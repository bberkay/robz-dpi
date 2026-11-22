//! Yönetici yardımcısı: `RobzDPI.exe --helper <işlem> <iş dosyası>`.
//!
//! Arayüz süreci bu kipi Windows'un yönetici onayıyla (UAC) çalıştırır. Penceresi yoktur; işi yapar ve çıkar.
//! Çıkış kodu 0 başarıdır. Hata olursa ayrıntı korumalı klasördeki last-result.json'a yazılır.
//!
//! Yardımcı, arayüz sürecinden geleni güvenilir saymaz:
//!   * İstek (yöntem, DNS adresi, alan adları) yeniden doğrulanır; komut satırı buradaki sabitlerle kurulur.
//!   * Motor dosyaları, derleme anında gömülen SHA-256 özetleriyle tutuyorsa korumalı klasöre kopyalanır.
//!   * Yalnız korumalı klasöre ("Program Files\RobzDPI") yazılır.

use super::scm::{self, State};
use super::windows::{engine_id, protected_dir, source_dir, win_error, Applied, Job, JobResult, ENGINE_FILES, FOREIGN_ENGINES, SERVICE_NAME};
use crate::error::{DpiError, DpiResult};
use crate::plan::{command_line, EngineRequest};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::ptr::null;
use std::time::Duration;
use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};

const SERVICE_DISPLAY: &str = "Robz DPI";
const SERVICE_DESCRIPTION: &str = "Robz DPI motoru (GoodbyeDPI). Robz DPI uygulamasından açılır ve kapatılır.";

/// Servisin erişim listesi. SYSTEM ve yöneticiler: tam denetim. Oturum açmış kullanıcı (IU): sorgula, başlat (RP),
/// durdur (WP). Kullanıcı servisin ayarını (çalıştırdığı komutu) değiştiremez; onun için yönetici onayı gerekir.
const SERVICE_SDDL: &str =
    "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWRPWPLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)";

/// GoodbyeDPI-Turkey'in komut dosyalarının kurduğu servis.
const GOODBYEDPI_SERVICE: &str = "GoodbyeDPI";
const WINDIVERT_SERVICES: [&str; 2] = ["WinDivert", "WinDivert14"];

const MAX_JOB_BYTES: u64 = 1024 * 1024;

pub fn main(args: &[String]) -> i32 {
    let action = args.first().map(String::as_str).unwrap_or_default();
    let job = args.get(1).and_then(|path| read_job(Path::new(path))).unwrap_or_default();
    let result = match action {
        "apply" => job.request.as_ref().ok_or_else(|| DpiError::new("bad-request")).and_then(apply),
        "stop" => stop_engine(),
        "remove" => remove(),
        "conflicts" => stop_conflicts(),
        _ => Err(DpiError::new("bad-request")),
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            write_result(&job.nonce, error);
            1
        }
    }
}

fn read_job(path: &Path) -> Option<Job> {
    if std::fs::metadata(path).ok()?.len() > MAX_JOB_BYTES {
        return None;
    }
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn write_result(nonce: &str, error: DpiError) {
    let Some(dir) = protected_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    let result = JobResult { nonce: nonce.to_string(), error };
    let _ = std::fs::write(dir.join("last-result.json"), serde_json::to_vec(&result).unwrap_or_default());
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn io_error(code: &str) -> impl Fn(std::io::Error) -> DpiError + '_ {
    move |error| DpiError::with(code, error)
}

/// Servisi kurar ya da ayarını değiştirir, sonra başlatır.
fn apply(request: &EngineRequest) -> DpiResult<()> {
    request.validate()?;
    let dir = protected_dir().ok_or_else(|| DpiError::new("no-program-files"))?;
    let engine_dir = dir.join("engine");
    std::fs::create_dir_all(&engine_dir).map_err(io_error("write-failed"))?;

    // Çalışıyorsa durdur: dosyalar ve komut satırı değişecek.
    stop_engine()?;
    install_engine(&engine_dir)?;

    let blacklist = dir.join("blacklist.txt");
    match request.blacklist_text() {
        Some(text) => std::fs::write(&blacklist, text).map_err(io_error("write-failed"))?,
        None => {
            let _ = std::fs::remove_file(&blacklist);
        }
    }

    let command = command_line(&engine_dir.join(ENGINE_FILES[0].0), &request.args(&blacklist));
    scm::install(SERVICE_NAME, SERVICE_DISPLAY, SERVICE_DESCRIPTION, &command, SERVICE_SDDL).map_err(win_error)?;

    // Servis artık bu ayarla kurulu: başlatma başarısız olsa da kayıt doğrudur.
    let applied = Applied { fingerprint: request.fingerprint(&engine_id()) };
    std::fs::write(dir.join("applied.json"), serde_json::to_vec(&applied).unwrap_or_default()).map_err(io_error("write-failed"))?;

    scm::start(SERVICE_NAME).map_err(win_error)?;
    if !scm::wait_for(SERVICE_NAME, State::Running, Duration::from_secs(8)) {
        return Err(DpiError::with("engine-start-failed", scm::query(SERVICE_NAME).exit_code));
    }
    Ok(())
}

/// Uygulamayla gelen motor dosyalarını doğrulayıp korumalı klasöre kopyalar. Dosya bir kez okunur: özeti alınan
/// baytlar ile yazılan baytlar aynıdır.
fn install_engine(engine_dir: &Path) -> DpiResult<()> {
    let source = source_dir().ok_or_else(|| DpiError::new("engine-missing"))?;
    for (name, expected) in ENGINE_FILES {
        if expected.is_empty() {
            return Err(DpiError::new("engine-missing"));
        }
        let bytes = std::fs::read(source.join(name)).map_err(|_| DpiError::with("engine-missing", name))?;
        if sha256_hex(&bytes) != expected {
            return Err(DpiError::with("engine-tampered", name));
        }
        let target = engine_dir.join(name);
        if std::fs::read(&target).is_ok_and(|current| sha256_hex(&current) == expected) {
            continue;
        }
        std::fs::write(&target, &bytes).map_err(|error| DpiError::with("engine-busy", format!("{name}: {error}")))?;
    }
    Ok(())
}

fn stop_engine() -> DpiResult<()> {
    scm::stop(SERVICE_NAME).map_err(win_error)?;
    if scm::query(SERVICE_NAME).state != State::Missing && !scm::wait_for(SERVICE_NAME, State::Stopped, Duration::from_secs(10)) {
        return Err(DpiError::new("engine-stop-failed"));
    }
    Ok(())
}

/// Her şeyi kaldırır: servis, WinDivert sürücüsü (başka bir araç kullanmıyorsa) ve korumalı klasör.
fn remove() -> DpiResult<()> {
    scm::delete(SERVICE_NAME).map_err(win_error)?;

    // WinDivert sürücüsü ilk kullanımda yüklenir ve bilgisayar yeniden başlayana kadar kalır. Başka bir DPI aracı
    // çalışmıyorsa şimdi durdurulur (GoodbyeDPI'ın kendi kaldırma betiğinin yaptığı gibi).
    if scm::processes_named(&FOREIGN_ENGINES).is_empty() {
        for driver in WINDIVERT_SERVICES {
            let _ = scm::delete(driver);
        }
    }

    if let Some(dir) = protected_dir() {
        if std::fs::remove_dir_all(&dir).is_err() {
            // Sürücü dosyası hâlâ kullanımdaysa: kalanlar bilgisayar yeniden başlarken silinsin.
            delete_on_reboot(&dir);
        }
    }
    Ok(())
}

fn delete_on_reboot(dir: &Path) {
    let mut paths = Vec::new();
    for sub in [dir.join("engine"), dir.to_path_buf()] {
        if let Ok(entries) = std::fs::read_dir(&sub) {
            paths.extend(entries.flatten().map(|entry| entry.path()).filter(|path| path.is_file()));
        }
    }
    paths.push(dir.join("engine"));
    paths.push(dir.to_path_buf());
    for path in paths {
        if std::fs::remove_file(&path).is_ok() || std::fs::remove_dir(&path).is_ok() || !path.exists() {
            continue;
        }
        let wide = scm::wide(&path.to_string_lossy());
        // SAFETY: wide sıfırla biten UTF-16'dır; hedef null olduğunda dosya yeniden başlatmada silinir.
        unsafe { MoveFileExW(wide.as_ptr(), null(), MOVEFILE_DELAY_UNTIL_REBOOT) };
    }
}

/// Kullanıcının bilgisayarındaki GoodbyeDPI'ı durdurur: servis olarak kuruluysa siler, elle başlatılmışsa süreci
/// sonlandırır. Arayüz bunu yalnız kullanıcı onay verdikten sonra ister.
fn stop_conflicts() -> DpiResult<()> {
    scm::delete(GOODBYEDPI_SERVICE).map_err(win_error)?;
    let own = scm::query(SERVICE_NAME).pid;
    let mut failed = None;
    for (name, pid) in scm::processes_named(&["goodbyedpi.exe"]) {
        if pid == 0 || pid == own {
            continue;
        }
        if let Err(code) = scm::terminate(pid) {
            failed = Some(DpiError::with("conflict-stop-failed", format!("{name} ({pid}): {code}")));
        }
    }
    failed.map_or(Ok(()), Err)
}
