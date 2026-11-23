//! Windows ve Linux motorunun ortak parçaları: yardımcıyla paylaşılan kayıtlar, çıkış kodları, günlük ve sayaç
//! dosyaları. Buradaki hiçbir şey paketlere dokunmaz.

use super::logic::Stats;
use crate::error::DpiError;
use crate::plan::EngineRequest;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Motorun kendi çıkış kodları. Arayüz bunları "engineCode.<kod>" metniyle gösterir.
/// engine.json yok ya da geçersiz.
pub const EXIT_BAD_CONFIG: u32 = 9001;
/// Windows: WinDivert.dll yüklenemedi.
#[cfg(windows)]
pub const EXIT_NO_DLL: u32 = 9002;
/// Windows: süzgeç derlenemedi (programlama hatası; nedeni günlükte yazar).
#[cfg(windows)]
pub const EXIT_BAD_FILTER: u32 = 9003;
/// Linux: çekirdeğin paket kuyruğu açılamadı (nfnetlink_queue yok ya da yetki yok).
#[cfg(target_os = "linux")]
pub const EXIT_NO_QUEUE: u32 = 9004;
/// Linux: nftables kuralları yüklenemedi (nft yok ya da çekirdek desteklemiyor).
#[cfg(target_os = "linux")]
pub const EXIT_NO_RULES: u32 = 9005;

const LOG_LIMIT: u64 = 512 * 1024;
const STATS_EVERY: Duration = Duration::from_secs(2);

/// Yönetici yardımcısının yazdığı kayıt: en son hangi ayar uygulandı.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Applied {
    pub fingerprint: String,
}

/// Arayüz sürecinden yardımcıya giden iş.
#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct Job {
    pub nonce: String,
    pub request: Option<EngineRequest>,
}

/// Yardımcının başarısız olduğunda bıraktığı sonuç.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct JobResult {
    pub nonce: String,
    pub error: DpiError,
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Her iş için ayrı bir kimlik: yardımcının bıraktığı sonucun bu işe ait olduğu bununla anlaşılır.
pub(super) fn nonce() -> String {
    format!(
        "{:x}-{:x}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default()
    )
}

/// Sahte paketlerdeki rastgele alanların tohumu.
pub(super) fn seed() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) ^ u64::from(std::process::id())
}

/// Günlüğe yazılan "çalışıyor" satırı: neye uygulandığı ve DNS'in nereye gittiği.
pub(super) fn describe(request: &EngineRequest) -> String {
    format!(
        "çalışıyor: {}, DNS {}",
        match &request.domains {
            Some(domains) => format!("{} alan adı", domains.len()),
            None => "bütün siteler".to_string(),
        },
        match &request.dns {
            Some(dns) => format!("{}:{}{}", dns.v4, dns.port, if dns.all { " (bütün sorgular)" } else { " (yalnız hedefler)" }),
            None => "değiştirilmiyor".to_string(),
        }
    )
}

/// Bir metin dosyasının son `max_bytes` baytı (motor günlüğü için).
pub(super) fn tail(path: &Path, max_bytes: usize) -> String {
    let Ok(mut file) = fs::File::open(path) else { return String::new() };
    let length = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    let _ = file.seek(SeekFrom::Start(length.saturating_sub(max_bytes as u64)));
    let mut bytes = Vec::new();
    let _ = file.take(max_bytes as u64).read_to_end(&mut bytes);
    let text = String::from_utf8_lossy(&bytes);
    // Ortadan başladıysak yarım kalan ilk satırı at.
    match text.split_once('\n') {
        Some((_, rest)) if length > max_bytes as u64 => rest.to_string(),
        _ => text.into_owned(),
    }
}

// ---- günlük ve sayaç dosyaları --------------------------------------------------------------------------------------

/// Motorun günlüğü (engine.log): ne zaman başladı, hangi kurallarla çalışıyor, durduysa neden.
pub(super) struct Log {
    path: PathBuf,
}

impl Log {
    pub fn open(dir: &Path) -> Self {
        let path = dir.join("engine.log");
        // Büyüdüyse eskisini bir kez sakla, yenisine başla.
        if fs::metadata(&path).is_ok_and(|meta| meta.len() > LOG_LIMIT) {
            let _ = fs::rename(&path, dir.join("engine.old.log"));
        }
        Self { path }
    }

    pub fn line(&mut self, text: &str) {
        let (year, month, day, hour, minute, second) = local_time();
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = writeln!(file, "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}  {text}");
        }
    }
}

/// Yerel saat: (yıl, ay, gün, saat, dakika, saniye).
#[cfg(windows)]
fn local_time() -> (u32, u32, u32, u32, u32, u32) {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: time düz veridir.
    let time = unsafe {
        let mut time = std::mem::zeroed();
        GetLocalTime(&mut time);
        time
    };
    (time.wYear.into(), time.wMonth.into(), time.wDay.into(), time.wHour.into(), time.wMinute.into(), time.wSecond.into())
}

#[cfg(target_os = "linux")]
fn local_time() -> (u32, u32, u32, u32, u32, u32) {
    // SAFETY: tm düz veridir; localtime_r yalnız verilen yapıyı doldurur.
    let tm = unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm
    };
    ((tm.tm_year + 1900) as u32, (tm.tm_mon + 1) as u32, tm.tm_mday as u32, tm.tm_hour as u32, tm.tm_min as u32, tm.tm_sec as u32)
}

/// Motorun sayaçları (stats.json): arayüz bunu okuyup gösterir.
pub(super) struct StatsFile {
    path: PathBuf,
    since: u64,
    written: Stats,
    at: Instant,
}

impl StatsFile {
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("stats.json"),
            since: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
            written: Stats::default(),
            at: Instant::now(),
        }
    }

    pub fn write(&mut self, stats: &Stats) {
        let mut value = json!(stats);
        value["since"] = json!(self.since);
        let _ = fs::write(&self.path, value.to_string());
        self.written = stats.clone();
        self.at = Instant::now();
    }

    /// Sayaçlar değiştiyse ve son yazımın üstünden biraz geçtiyse dosyayı yeniler.
    pub fn maybe_write(&mut self, stats: &Stats) {
        if self.at.elapsed() >= STATS_EVERY && *stats != self.written {
            self.write(stats);
        }
    }
}
