//! Linux: arayüz sürecinin (root yetkisi olmadan) kullandığı işlemler ve yönetici onayıyla (polkit / pkexec)
//! yardımcıyı çalıştırma. Yönetici tarafı linux_helper.rs'te, motorun kendisi linux_service.rs'tedir.
//!
//! Windows'taki karşılıkları:
//!   Windows servisi "RobzDPI"            systemd birimi "robzdpi.service"
//!   Program Files\RobzDPI                /opt/robzdpi (motor) ve /var/lib/robzdpi (ayar, günlük, sayaçlar)
//!   UAC                                  polkit (pkexec)
//!   servisin erişim listesi              polkit kuralı: oturum açmış kullanıcı birimi başlatıp durdurabilir

use super::common::{nonce, sha256_hex, tail, Applied, Job, JobResult};
use super::{linux_helper, linux_service, Conflict, Status};
use crate::error::{DpiError, DpiResult};
use crate::plan::EngineRequest;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

pub const SUPPORTED: bool = true;

/// systemd biriminin adı (`systemctl status robzdpi`).
pub(super) const UNIT: &str = "robzdpi.service";
pub(super) const UNIT_FILE: &str = "/etc/systemd/system/robzdpi.service";
/// Oturum açmış kullanıcının birimi yönetici onayı olmadan başlatıp durdurmasına izin veren polkit kuralı.
pub(super) const POLKIT_RULE: &str = "/etc/polkit-1/rules.d/49-robzdpi.rules";
/// Motorun kopyalandığı klasör ve adı: bu programın kopyası, `--engine` ile çalışır. Yalnız root yazabilir.
pub(super) const ENGINE_DIR: &str = "/opt/robzdpi";
pub(super) const ENGINE_BIN: &str = "robzdpi-engine";
/// Motorun hata ile çıktığında kodunu yazdığı dosya (durum klasöründe).
pub(super) const EXIT_FILE: &str = "exit-code";

/// Aynı anda çalışırsa paketleri iki kez değiştirecek başka araçlar.
pub(super) const FOREIGN_ENGINES: [&str; 5] = ["nfqws", "tpws", "ciadpi", "byedpi", "spoofdpi"];

/// Motorun ayarı, günlüğü ve sayaçları. Yalnız root yazabilir; herkes okuyabilir.
pub(super) fn state_dir() -> PathBuf {
    PathBuf::from("/var/lib/robzdpi")
}

/// Bu sürecin çalışan programının yolu (/proc/self/exe'nin gösterdiği dosya).
pub(super) fn image_path() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

/// Bir programı bilinen sistem klasörlerinde arar (PATH kullanıcıya aittir; ona güvenilmez).
pub(super) fn system_binary(name: &str) -> Option<PathBuf> {
    ["/usr/bin", "/bin", "/usr/sbin", "/sbin"].iter().map(|dir| Path::new(dir).join(name)).find(|path| path.is_file())
}

/// Motorun kimliği: bu programın özeti. Uygulama güncellenince değişir; arayüz o zaman "yeniden uygula" ister ve
/// /opt/robzdpi'deki motor da yenilenir.
pub fn engine_id() -> String {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| image_path().and_then(|path| std::fs::read(path).ok()).map(|bytes| sha256_hex(&bytes)).unwrap_or_default())
        .clone()
}

/// Motorun gereksindiği sistem araçları var mı: systemd, nftables ve polkit (pkexec).
pub fn available() -> bool {
    system_binary("systemctl").is_some() && super::nfqueue::nft_path().is_some() && system_binary("pkexec").is_some()
}

fn systemctl(arguments: &[&str]) -> Option<std::process::Output> {
    Command::new(system_binary("systemctl")?).args(arguments).stdin(Stdio::null()).output().ok()
}

/// `systemctl show` çıktısından bir alan ("ActiveState=active" -> "active").
fn property(name: &str) -> Option<String> {
    let output = systemctl(&["show", UNIT, "--property", name, "--value"])?;
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn status() -> Status {
    if !Path::new(UNIT_FILE).is_file() {
        return Status::MISSING;
    }
    let state = match property("ActiveState").as_deref() {
        Some("active") => "running",
        Some("activating") | Some("reloading") => "starting",
        Some("deactivating") => "stopping",
        _ => "stopped",
    };
    let pid = property("MainPID").and_then(|pid| pid.parse().ok()).unwrap_or(0);
    Status { installed: true, state, pid }
}

/// Motor kendiliğinden durduysa nedeni: motorun yazdığı çıkış kodu (linux_service.rs). İstenerek durduysa None.
pub fn stop_reason() -> Option<u32> {
    if property("ActiveState").as_deref() != Some("failed") {
        return None;
    }
    let code = std::fs::read_to_string(state_dir().join(EXIT_FILE)).ok().and_then(|text| text.trim().parse().ok());
    Some(code.unwrap_or(1))
}

/// En son uygulanan ayarın parmak izi (yardımcının durum klasörüne yazdığı kayıt).
pub fn applied_fingerprint() -> Option<String> {
    let text = std::fs::read_to_string(state_dir().join("applied.json")).ok()?;
    serde_json::from_str::<Applied>(&text).ok().map(|applied| applied.fingerprint)
}

/// Motorun sayaçları (stats.json): işlenen bağlantılar, yönlendirilen sorgular, son alan adları.
pub fn stats() -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(state_dir().join("stats.json")).ok()?).ok()
}

/// Motor günlüğünün (engine.log) son `max_bytes` baytı.
pub fn log_tail(max_bytes: usize) -> String {
    tail(&state_dir().join("engine.log"), max_bytes)
}

/// Kurulu birimi başlatır. Yönetici onayı gerekmez (polkit kuralı buna izin verir). Birim "çalışıyor" dediğinde
/// kuyruk açılmış ve kurallar yüklenmiş demektir (Type=notify).
pub fn start() -> DpiResult<()> {
    let output = systemctl(&["start", UNIT]).ok_or_else(|| DpiError::new("engine-missing"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(match stop_reason() {
        Some(code) => DpiError::with("engine-start-failed", code),
        None => DpiError::with("engine-start-failed", String::from_utf8_lossy(&output.stderr).trim()),
    })
}

pub fn stop() -> DpiResult<()> {
    if !Path::new(UNIT_FILE).is_file() {
        return Ok(());
    }
    match systemctl(&["stop", UNIT]) {
        Some(output) if output.status.success() => Ok(()),
        _ => Err(DpiError::new("engine-stop-failed")),
    }
}

/// Adı verilen programlardan çalışanlar: (ad, süreç numarası). /proc/<pid>/comm okunur.
pub(super) fn processes_named(names: &[&str]) -> Vec<(String, u32)> {
    let Ok(entries) = std::fs::read_dir("/proc") else { return Vec::new() };
    entries
        .flatten()
        .filter_map(|entry| {
            let pid: u32 = entry.file_name().to_str()?.parse().ok()?;
            let name = std::fs::read_to_string(entry.path().join("comm")).ok()?.trim().to_string();
            names.contains(&name.as_str()).then_some((name, pid))
        })
        .collect()
}

/// Bizim motorumuz dışında çalışan DPI araçları.
pub fn conflicts(own_pid: u32) -> Vec<Conflict> {
    processes_named(&FOREIGN_ENGINES).into_iter().filter(|(_, pid)| *pid != own_pid).map(|(name, pid)| Conflict { name, pid }).collect()
}

/// Yardımcıyı yönetici onayıyla (polkit / pkexec) çalıştırır ve bitmesini bekler.
/// `action`: "apply" (request gerekir) | "stop" | "remove" | "conflicts". Kullanıcı onayı reddederse "cancelled".
pub fn elevated(action: &str, request: Option<&EngineRequest>, jobs_dir: &Path) -> DpiResult<()> {
    let nonce = nonce();
    let job = Job { nonce: nonce.clone(), request: request.cloned() };
    let job_path = jobs_dir.join("job.json");
    std::fs::create_dir_all(jobs_dir).map_err(|e| DpiError::with("write-failed", e))?;
    std::fs::write(&job_path, serde_json::to_vec(&job).unwrap_or_default()).map_err(|e| DpiError::with("write-failed", e))?;

    let outcome = run_elevated(action, &job_path);
    let _ = std::fs::remove_file(&job_path);

    match outcome? {
        0 => Ok(()),
        code => Err(read_result(&nonce).unwrap_or_else(|| DpiError::with("helper-failed", code))),
    }
}

/// Yardımcının bıraktığı hata; yalnız bu işe aitse (nonce tutuyorsa) kullanılır.
fn read_result(nonce: &str) -> Option<DpiError> {
    let text = std::fs::read_to_string(linux_helper::RESULT_FILE).ok()?;
    let result: JobResult = serde_json::from_str(&text).ok()?;
    (result.nonce == nonce).then_some(result.error)
}

/// Bu programı pkexec ile root olarak başlatır, bitene kadar bekler ve çıkış kodunu döndürür.
/// pkexec'in kendi kodları: 126 kullanıcı onay vermedi, 127 kimlik doğrulanamadı ya da pkexec çalışamadı.
fn run_elevated(action: &str, job_path: &Path) -> DpiResult<i32> {
    let pkexec = system_binary("pkexec").ok_or_else(|| DpiError::with("elevation-failed", "pkexec"))?;
    let exe = image_path().ok_or_else(|| DpiError::new("elevation-failed"))?;
    let status = Command::new(pkexec)
        .arg(exe)
        .args(["--helper", action])
        .arg(job_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| DpiError::with("elevation-failed", e))?;
    match status.code() {
        Some(126) => Err(DpiError::new("cancelled")),
        Some(127) | None => Err(DpiError::new("elevation-failed")),
        Some(code) => Ok(code),
    }
}

pub fn helper_main(args: &[String]) -> i32 {
    linux_helper::main(args)
}

pub fn engine_main() -> i32 {
    linux_service::main()
}
