//! Windows motoru: arayüz sürecinin (yönetici yetkisi olmadan) kullandığı işlemler ve yönetici onayıyla
//! yardımcıyı çalıştırma. Yönetici tarafı helper.rs'tedir.

use super::scm::{self, State};
use super::{helper, Conflict, Status};
use crate::error::{DpiError, DpiResult};
use crate::plan::{quote_arg, EngineRequest};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ACCESS_DENIED, ERROR_CANCELLED, ERROR_SERVICE_DOES_NOT_EXIST};
use windows_sys::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
use windows_sys::Win32::UI::Shell::{
    FOLDERID_ProgramFiles, SHGetKnownFolderPath, ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC,
    SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

pub const SUPPORTED: bool = true;

/// Windows servisinin adı (`sc query RobzDPI`).
pub(super) const SERVICE_NAME: &str = "RobzDPI";

/// Motor dosyaları ve derleme anında gömülen SHA-256 özetleri (build.rs).
pub(super) const ENGINE_FILES: [(&str, &str); 3] = [
    ("goodbyedpi.exe", env!("ROBZDPI_SHA_EXE")),
    ("WinDivert.dll", env!("ROBZDPI_SHA_DLL")),
    ("WinDivert64.sys", env!("ROBZDPI_SHA_SYS")),
];

/// Aynı anda çalışırsa paketleri iki kez değiştirecek başka araçlar.
pub(super) const FOREIGN_ENGINES: [&str; 2] = ["goodbyedpi.exe", "winws.exe"];

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

pub fn engine_id() -> String {
    ENGINE_FILES.map(|(_, digest)| digest).join(":")
}

/// Motor dosyaları derlemeye girmiş ve uygulamanın yanında duruyor mu?
pub fn available() -> bool {
    ENGINE_FILES.iter().all(|(_, digest)| !digest.is_empty()) && source_dir().is_some()
}

/// Uygulamayla gelen motor dosyalarının klasörü (kurulum klasöründeki "engine").
pub(super) fn source_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().as_deref().and_then(Path::parent) {
        candidates.push(dir.join("engine"));
    }
    #[cfg(debug_assertions)]
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("engine"));
    candidates.into_iter().find(|dir| dir.join(ENGINE_FILES[0].0).is_file())
}

/// Servisin çalıştığı korumalı klasör: "C:\Program Files\RobzDPI". Yalnız yöneticiler yazabilir.
/// Yol ortam değişkeninden değil Windows'un kendi kaydından okunur (ortam değişkeni kullanıcıya aittir).
pub(super) fn protected_dir() -> Option<PathBuf> {
    let mut raw: *mut u16 = null_mut();
    // SAFETY: başarılı çağrıda raw sıfırla biten bir UTF-16 dizgisini gösterir ve CoTaskMemFree ile bırakılır.
    unsafe {
        let result = SHGetKnownFolderPath(&FOLDERID_ProgramFiles, 0, null_mut(), &mut raw);
        if raw.is_null() {
            return None;
        }
        let mut len = 0;
        while *raw.add(len) != 0 {
            len += 1;
        }
        let path = PathBuf::from(OsString::from_wide(std::slice::from_raw_parts(raw, len)));
        CoTaskMemFree(raw.cast());
        (result == 0 && path.is_absolute()).then(|| path.join("RobzDPI"))
    }
}

pub(super) fn win_error(code: u32) -> DpiError {
    match code {
        ERROR_ACCESS_DENIED => DpiError::new("access-denied"),
        ERROR_SERVICE_DOES_NOT_EXIST => DpiError::new("not-installed"),
        _ => DpiError::with("windows-error", code),
    }
}

pub fn status() -> Status {
    let raw = scm::query(SERVICE_NAME);
    let state = match raw.state {
        State::Missing => return Status::MISSING,
        State::Stopped => "stopped",
        State::Starting => "starting",
        State::Running => "running",
        State::Stopping => "stopping",
    };
    Status { installed: true, state, pid: raw.pid }
}

/// En son uygulanan ayarın parmak izi (yardımcının korumalı klasöre yazdığı kayıt).
pub fn applied_fingerprint() -> Option<String> {
    let text = std::fs::read_to_string(protected_dir()?.join("applied.json")).ok()?;
    serde_json::from_str::<Applied>(&text).ok().map(|applied| applied.fingerprint)
}

/// Kurulu servisi başlatır. Yönetici yetkisi gerekmez (servisin erişim listesi buna izin verir).
pub fn start() -> DpiResult<()> {
    scm::start(SERVICE_NAME).map_err(win_error)?;
    if scm::wait_for(SERVICE_NAME, State::Running, Duration::from_secs(8)) {
        Ok(())
    } else {
        Err(DpiError::with("engine-start-failed", scm::query(SERVICE_NAME).exit_code))
    }
}

pub fn stop() -> DpiResult<()> {
    scm::stop(SERVICE_NAME).map_err(win_error)?;
    if scm::query(SERVICE_NAME).state == State::Missing || scm::wait_for(SERVICE_NAME, State::Stopped, Duration::from_secs(10)) {
        Ok(())
    } else {
        Err(DpiError::new("engine-stop-failed"))
    }
}

/// Bizim servisimiz dışında çalışan DPI araçları.
pub fn conflicts(own_pid: u32) -> Vec<Conflict> {
    scm::processes_named(&FOREIGN_ENGINES)
        .into_iter()
        .filter(|(_, pid)| *pid != own_pid)
        .map(|(name, pid)| Conflict { name: name.to_lowercase(), pid })
        .collect()
}

/// Yardımcıyı Windows'un yönetici onayıyla (UAC) çalıştırır ve bitmesini bekler.
/// `action`: "apply" (request gerekir) | "stop" | "remove" | "conflicts". Kullanıcı onayı reddederse "cancelled".
pub fn elevated(action: &str, request: Option<&EngineRequest>, jobs_dir: &Path) -> DpiResult<()> {
    let nonce = format!(
        "{:x}-{:x}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default()
    );
    let job = Job { nonce: nonce.clone(), request: request.cloned() };
    let job_path = jobs_dir.join("job.json");
    std::fs::create_dir_all(jobs_dir).map_err(|e| DpiError::with("write-failed", e))?;
    std::fs::write(&job_path, serde_json::to_vec(&job).unwrap_or_default()).map_err(|e| DpiError::with("write-failed", e))?;

    let parameters = format!("--helper {action} {}", quote_arg(&job_path.to_string_lossy()));
    let outcome = run_elevated(&parameters);
    let _ = std::fs::remove_file(&job_path);

    match outcome? {
        0 => Ok(()),
        code => Err(read_result(&nonce).unwrap_or_else(|| DpiError::with("helper-failed", code))),
    }
}

/// Yardımcının bıraktığı hata; yalnız bu işe aitse (nonce tutuyorsa) kullanılır.
fn read_result(nonce: &str) -> Option<DpiError> {
    let text = std::fs::read_to_string(protected_dir()?.join("last-result.json")).ok()?;
    let result: JobResult = serde_json::from_str(&text).ok()?;
    (result.nonce == nonce).then_some(result.error)
}

/// Bu exe'yi "runas" ile başlatır, bitene kadar bekler ve çıkış kodunu döndürür.
fn run_elevated(parameters: &str) -> DpiResult<u32> {
    let exe = std::env::current_exe().map_err(|e| DpiError::with("elevation-failed", e))?;
    let file: Vec<u16> = exe.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let verb = scm::wide("runas");
    let parameters = scm::wide(parameters);

    // SAFETY: yapı sıfırlanır ve yalnız çağrı boyunca yaşayan dizgileri gösterir. SEE_MASK_NOCLOSEPROCESS ile
    // dönen süreç tutamağı aşağıda kapatılır. ShellExecuteEx kabuk uzantılarına COM ile ulaşabildiği için bu iş
    // parçacığında COM açılır ve (yalnız biz açtıysak) kapatılır.
    unsafe {
        let com = CoInitializeEx(std::ptr::null(), (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32);

        let mut info: SHELLEXECUTEINFOW = zeroed();
        info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = parameters.as_ptr();
        info.nShow = SW_HIDE;

        let outcome = if ShellExecuteExW(&mut info) == 0 {
            Err(match GetLastError() {
                ERROR_CANCELLED => DpiError::new("cancelled"),
                code => DpiError::with("elevation-failed", code),
            })
        } else if info.hProcess.is_null() {
            Err(DpiError::new("elevation-failed"))
        } else {
            WaitForSingleObject(info.hProcess, INFINITE);
            let mut code = 1u32;
            GetExitCodeProcess(info.hProcess, &mut code);
            CloseHandle(info.hProcess);
            Ok(code)
        };

        if com >= 0 {
            CoUninitialize();
        }
        outcome
    }
}

pub fn helper_main(args: &[String]) -> i32 {
    helper::main(args)
}
