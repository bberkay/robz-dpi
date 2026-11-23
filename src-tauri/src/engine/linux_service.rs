//! Linux'ta motorun kendisi: `robzdpi-engine --engine`, systemd birimi olarak (root) çalışır.
//!
//! Durum klasöründeki (/var/lib/robzdpi, yalnız root yazabilir) engine.json'u okur, paket kuyruğunu açar,
//! nftables kurallarını yükler ve paket döngüsüne girer. SIGTERM gelince kuralları siler ve çıkar. Dışarıya üç dosya
//! yazar: engine.log (ne oldu), stats.json (sayaçlar) ve hata olursa exit-code (motorun çıkış kodu; arayüz okur).

use super::common::{describe, seed, Log, StatsFile, EXIT_BAD_CONFIG, EXIT_NO_QUEUE, EXIT_NO_RULES};
use super::linux::{state_dir, EXIT_FILE};
use super::logic::{Engine, Verdict};
use super::nfqueue::{self, Capture};
use crate::plan::EngineRequest;
use std::fs;
use std::os::unix::net::UnixDatagram;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_signal: libc::c_int) {
    STOP.store(true, Ordering::SeqCst);
}

pub fn main() -> i32 {
    let dir = state_dir();
    let _ = fs::remove_file(dir.join(EXIT_FILE));
    match run() {
        Ok(()) => 0,
        Err(code) => {
            let _ = fs::write(dir.join(EXIT_FILE), code.to_string());
            1
        }
    }
}

fn run() -> Result<(), u32> {
    let dir = state_dir();
    let mut log = Log::open(&dir);
    log.line(&format!("Robz DPI motoru {} başlıyor", env!("CARGO_PKG_VERSION")));

    let request = fs::read(dir.join("engine.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<EngineRequest>(&bytes).ok())
        .filter(|request| request.validate().is_ok());
    let Some(request) = request else {
        log.line("engine.json okunamadı ya da geçersiz");
        return Err(EXIT_BAD_CONFIG);
    };

    // SAFETY: işleyici yalnız bir atomik bayrağı kurar.
    unsafe {
        let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
        libc::signal(libc::SIGTERM, handler);
        libc::signal(libc::SIGINT, handler);
    }

    // Önce kuyruk açılır, sonra kurallar yüklenir: kurallar varken kuyruk yoksa paketler yine geçer (bypass).
    let mut capture = Capture::open().map_err(|error| {
        log.line(&format!("paket kuyruğu açılamadı: {error}"));
        EXIT_NO_QUEUE
    })?;
    let rules = nfqueue::rules(&request);
    log.line(&format!("kurallar: {}", rules.split_whitespace().collect::<Vec<_>>().join(" ")));
    if let Err(reason) = nfqueue::install_rules(&rules) {
        log.line(&format!("kurallar yüklenemedi: {reason}"));
        return Err(EXIT_NO_RULES);
    }

    // Daha önce alınmış yanıtlar sistemin DNS önbelleğinde kalmasın; hedeflerin adresleri yeniden sorulup öğrenilsin.
    flush_dns_cache();
    notify_ready();
    log.line(&describe(&request));

    let mut engine = Engine::new(&request, seed(), Instant::now());
    let mut stats = StatsFile::new(&dir);
    stats.write(&engine.stats);

    let result = loop {
        if STOP.load(Ordering::SeqCst) {
            break Ok(());
        }
        let mut message = match capture.recv(Duration::from_millis(500)) {
            Ok(Some(message)) => message,
            Ok(None) => {
                stats.maybe_write(&engine.stats);
                continue;
            }
            Err(error) => {
                log.line(&format!("paket okunamadı: {error}"));
                break Err(error.raw_os_error().map_or(1, |code| code as u32));
            }
        };
        let outbound = nfqueue::outbound(&message);
        match engine.handle(message.get_payload(), outbound, Instant::now()) {
            Verdict::Pass => message.set_verdict(nfq::Verdict::Accept),
            Verdict::Drop => message.set_verdict(nfq::Verdict::Drop),
            // Giden yön: motorun paketleri ham yuvadan sırayla gönderilir, yakalanan asıl paket bırakılmaz.
            Verdict::Send(packets) if outbound => {
                let sent = packets.iter().all(|packet| capture.send(packet).is_ok());
                // Gönderilemediyse asıl paket olduğu gibi geçer: bağlantı kesilmez, yalnız yöntem uygulanmamış olur.
                message.set_verdict(if sent { nfq::Verdict::Drop } else { nfq::Verdict::Accept });
            }
            // Gelen yön (yönlendirilmiş DNS yanıtı): paket yerinde değiştirilir.
            Verdict::Send(mut packets) => {
                if let (1, Some(packet)) = (packets.len(), packets.pop()) {
                    message.set_payload(packet);
                }
                message.set_verdict(nfq::Verdict::Accept);
            }
        }
        if let Err(error) = capture.verdict(message) {
            log.line(&format!("karar bildirilemedi: {error}"));
        }
        stats.maybe_write(&engine.stats);
    };

    nfqueue::remove_rules();
    stats.write(&engine.stats);
    let s = &engine.stats;
    log.line(&format!("durdu: {} TLS, {} HTTP, {} DNS, {} QUIC", s.tls, s.http, s.dns, s.quic));
    result
}

/// systemd'ye "hazırım" der (Type=notify): `systemctl start` ancak kuyruk açılıp kurallar yüklenince döner.
fn notify_ready() {
    let Some(path) = std::env::var_os("NOTIFY_SOCKET") else { return };
    let Ok(socket) = UnixDatagram::unbound() else { return };
    let name = path.to_string_lossy();
    let _ = match name.strip_prefix('@') {
        // '@' ile başlayan ad soyut yuvadır (dosya sisteminde yeri yoktur).
        Some(abstract_name) => {
            use std::os::linux::net::SocketAddrExt;
            std::os::unix::net::SocketAddr::from_abstract_name(abstract_name).and_then(|address| socket.send_to_addr(b"READY=1", &address))
        }
        None => socket.send_to(b"READY=1", &path),
    };
}

/// systemd-resolved kullanılıyorsa DNS önbelleğini boşaltır; yoksa sessizce geçer.
fn flush_dns_cache() {
    for path in ["/usr/bin/resolvectl", "/bin/resolvectl"] {
        if std::path::Path::new(path).is_file() {
            let _ = std::process::Command::new(path).arg("flush-caches").output();
            return;
        }
    }
}
