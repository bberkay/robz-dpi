//! Motorun kendisi: `RobzDPI-engine.exe --engine`, Windows servisi olarak (SYSTEM) çalışır.
//!
//! Bulunduğu klasördeki (Program Files\RobzDPI, yalnız yöneticiler yazabilir) engine.json'u okur, WinDivert'i
//! açar ve paket döngüsüne girer. Durdurma isteği gelince döngüden çıkar. Penceresi ve ağ sunucusu yoktur;
//! dışarıya yalnız iki dosya yazar: engine.log (ne oldu) ve stats.json (sayaçlar).
//!
//! Çıkış kodları (servise özel hata kodu olarak Windows'a bildirilir; arayüz okur):
//!   0                      istenerek durdu
//!   EXIT_BAD_CONFIG        engine.json yok ya da geçersiz
//!   EXIT_NO_DLL            WinDivert.dll yüklenemedi
//!   diğer                  WinDivert'in verdiği Windows hata kodu (5 yetki yok, 577 imza, 1275 engellendi …)

use super::common::{describe, seed, Log, StatsFile, EXIT_BAD_CONFIG, EXIT_BAD_FILTER, EXIT_NO_DLL};
use super::divert::{Divert, Filter};
use super::filter;
use super::logic::{Engine, Verdict};
use super::scm::wide;
use super::windows::SERVICE_NAME;
use crate::plan::EngineRequest;
use std::ffi::c_void;
use std::fs;
use std::path::Path;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::OnceLock;
use std::time::Instant;
use windows_sys::Win32::Foundation::{ERROR_CALL_NOT_IMPLEMENTED, ERROR_SERVICE_SPECIFIC_ERROR};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32};
use windows_sys::Win32::System::Services::{
    RegisterServiceCtrlHandlerExW, SetServiceStatus, StartServiceCtrlDispatcherW, SERVICE_ACCEPT_SHUTDOWN, SERVICE_ACCEPT_STOP,
    SERVICE_CONTROL_INTERROGATE, SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP, SERVICE_RUNNING, SERVICE_START_PENDING,
    SERVICE_STATUS, SERVICE_STOPPED, SERVICE_STOP_PENDING, SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS,
};

/// En büyük IP paketi (ağ kartına bölme devredildiğinde sürücü böyle büyük paketler verebilir).
const BUFFER: usize = 40 + 0xffff;

static STOP: AtomicBool = AtomicBool::new(false);
static STATUS: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static FILTER: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static DIVERT: OnceLock<Divert> = OnceLock::new();

/// Servis olarak çalışır; servis durana kadar dönmez. Servis Denetim Yöneticisi başlatmadıysa hemen 1 döner.
pub fn main() -> i32 {
    let mut name = wide(SERVICE_NAME);
    let table = [
        SERVICE_TABLE_ENTRYW { lpServiceName: name.as_mut_ptr(), lpServiceProc: Some(service_main) },
        SERVICE_TABLE_ENTRYW { lpServiceName: null_mut(), lpServiceProc: None },
    ];
    // SAFETY: tablo sıfır kayıtla biter ve çağrı boyunca yaşar.
    if unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) } == 0 {
        return 1;
    }
    0
}

unsafe extern "system" fn service_main(_argc: u32, _argv: *mut *mut u16) {
    let name = wide(SERVICE_NAME);
    let handle = RegisterServiceCtrlHandlerExW(name.as_ptr(), Some(control), null_mut());
    if handle.is_null() {
        return;
    }
    STATUS.store(handle, Ordering::SeqCst);
    report(SERVICE_START_PENDING, 0);
    let code = run().err().unwrap_or(0);
    report(SERVICE_STOPPED, code);
}

unsafe extern "system" fn control(code: u32, _event: u32, _data: *mut c_void, _context: *mut c_void) -> u32 {
    match code {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
            STOP.store(true, Ordering::SeqCst);
            report(SERVICE_STOP_PENDING, 0);
            // Paket bekleyen döngüyü uyandır.
            let raw = FILTER.load(Ordering::SeqCst);
            if let (Some(divert), false) = (DIVERT.get(), raw.is_null()) {
                divert.shutdown(Filter::from_raw(raw));
            }
            0
        }
        SERVICE_CONTROL_INTERROGATE => 0,
        _ => ERROR_CALL_NOT_IMPLEMENTED,
    }
}

fn report(state: u32, exit: u32) {
    let handle = STATUS.load(Ordering::SeqCst);
    if handle.is_null() {
        return;
    }
    let status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: if state == SERVICE_RUNNING { SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN } else { 0 },
        dwWin32ExitCode: if exit == 0 { 0 } else { ERROR_SERVICE_SPECIFIC_ERROR },
        dwServiceSpecificExitCode: exit,
        dwCheckPoint: 0,
        dwWaitHint: if state == SERVICE_START_PENDING || state == SERVICE_STOP_PENDING { 5000 } else { 0 },
    };
    // SAFETY: handle RegisterServiceCtrlHandlerExW'den gelmiştir; status çağrı boyunca yaşar.
    unsafe { SetServiceStatus(handle, &status) };
}

fn run() -> Result<(), u32> {
    let dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf)).ok_or(EXIT_BAD_CONFIG)?;
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

    let divert = match Divert::load(&dir.join("WinDivert.dll")) {
        Ok(divert) => DIVERT.get_or_init(|| divert),
        Err(code) => {
            log.line(&format!("WinDivert.dll yüklenemedi (Windows hatası {code})"));
            return Err(EXIT_NO_DLL);
        }
    };

    let text = filter::build(&request);
    log.line(&format!("süzgeç: {text}"));
    if let Err(reason) = divert.compile(&text) {
        log.line(&format!("süzgeç derlenemedi: {reason}"));
        return Err(EXIT_BAD_FILTER);
    }
    let filter = divert
        .open(&text)
        .inspect_err(|code| log.line(&format!("WinDivert açılamadı (Windows hatası {code})")))?;
    FILTER.store(filter.as_raw(), Ordering::SeqCst);

    // Daha önce alınmış (yanlış) yanıtlar Windows'un DNS önbelleğinde kalmasın; hedeflerin adresleri de
    // yeniden sorulup öğrenilsin.
    flush_dns_cache();
    report(SERVICE_RUNNING, 0);
    log.line(&describe(&request));

    let started = Instant::now();
    let mut engine = Engine::new(&request, seed(), started);
    let mut stats = StatsFile::new(&dir);
    stats.write(&engine.stats);
    let mut buffer = vec![0u8; BUFFER];

    let result = loop {
        let (length, address) = match divert.recv(filter, &mut buffer) {
            Ok(received) => received,
            Err(_) if STOP.load(Ordering::SeqCst) => break Ok(()),
            Err(code) => {
                log.line(&format!("paket okunamadı (Windows hatası {code})"));
                break Err(code);
            }
        };
        let packet = &buffer[..length];
        match engine.handle(packet, address.outbound(), Instant::now()) {
            Verdict::Pass => {
                divert.send(filter, packet, &address);
            }
            Verdict::Drop => {}
            Verdict::Send(packets) => {
                let address = address.checksums_done();
                for packet in &packets {
                    divert.send(filter, packet, &address);
                }
            }
        }
        stats.maybe_write(&engine.stats);
    };

    FILTER.store(null_mut(), Ordering::SeqCst);
    divert.close(filter);
    stats.write(&engine.stats);
    let s = &engine.stats;
    log.line(&format!("durdu: {} TLS, {} HTTP, {} DNS, {} QUIC", s.tls, s.http, s.dns, s.quic));
    result
}

/// Windows'un DNS önbelleğini boşaltır (`ipconfig /flushdns` ile aynı çağrı).
fn flush_dns_cache() {
    let library = wide("dnsapi.dll");
    // SAFETY: kitaplık yalnız System32'den yüklenir; DnsFlushResolverCache bağımsız değişken almaz.
    unsafe {
        let module = LoadLibraryExW(library.as_ptr(), null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32);
        if module.is_null() {
            return;
        }
        if let Some(flush) = GetProcAddress(module, c"DnsFlushResolverCache".as_ptr().cast()) {
            flush();
        }
    }
}
