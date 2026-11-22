//! Windows Servis Denetim Yöneticisi (SCM) ve süreç listesi için ince sarmalayıcılar.
//! Hatalar Win32 hata kodu (u32) olarak döner; anlamlandırmak çağıranın işidir.

use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, ERROR_SERVICE_ALREADY_RUNNING, ERROR_SERVICE_DOES_NOT_EXIST,
    ERROR_SERVICE_NOT_ACTIVE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
use windows_sys::Win32::Security::{DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Services::{
    ChangeServiceConfig2W, ChangeServiceConfigW, CloseServiceHandle, ControlService, CreateServiceW, DeleteService,
    OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, SetServiceObjectSecurity, StartServiceW, SC_HANDLE,
    SC_MANAGER_CONNECT, SC_MANAGER_CREATE_SERVICE, SC_STATUS_PROCESS_INFO, SERVICE_ALL_ACCESS, SERVICE_CONFIG_DESCRIPTION,
    SERVICE_CONTINUE_PENDING, SERVICE_CONTROL_STOP, SERVICE_DEMAND_START, SERVICE_DESCRIPTIONW, SERVICE_ERROR_NORMAL,
    SERVICE_PAUSE_PENDING, SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START, SERVICE_START_PENDING, SERVICE_STATUS,
    SERVICE_STATUS_PROCESS, SERVICE_STOP, SERVICE_STOP_PENDING, SERVICE_WIN32_OWN_PROCESS,
};
use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

/// Standart erişim hakkı: nesneyi silme.
const DELETE: u32 = 0x0001_0000;

pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn last_error() -> u32 {
    // SAFETY: yalnız iş parçacığının son hata kodunu okur.
    unsafe { GetLastError() }
}

struct Handle(SC_HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: tutamak bu yapıya aittir ve bir kez kapatılır.
        unsafe { CloseServiceHandle(self.0) };
    }
}

fn manager(access: u32) -> Result<Handle, u32> {
    // SAFETY: boş göstericiler "bu bilgisayar, etkin veritabanı" demektir.
    let handle = unsafe { OpenSCManagerW(null(), null(), access) };
    if handle.is_null() {
        Err(last_error())
    } else {
        Ok(Handle(handle))
    }
}

fn open(name: &str, access: u32) -> Result<Handle, u32> {
    let scm = manager(SC_MANAGER_CONNECT)?;
    let name = wide(name);
    // SAFETY: name sıfırla biten geçerli bir UTF-16 dizgisidir.
    let handle = unsafe { OpenServiceW(scm.0, name.as_ptr(), access) };
    if handle.is_null() {
        Err(last_error())
    } else {
        Ok(Handle(handle))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Missing,
    Stopped,
    Starting,
    Running,
    Stopping,
}

#[derive(Debug, Clone, Copy)]
pub struct Status {
    pub state: State,
    pub pid: u32,
    /// Servisin son çıkış kodu (Win32). ERROR_SERVICE_SPECIFIC_ERROR ise asıl kod `specific`tedir.
    pub exit_code: u32,
    pub specific: u32,
}

const MISSING: Status = Status { state: State::Missing, pid: 0, exit_code: 0, specific: 0 };

fn query_handle(service: &Handle) -> Option<Status> {
    // SAFETY: yapı düz veridir; boyutu ile birlikte verilir.
    let mut raw: SERVICE_STATUS_PROCESS = unsafe { zeroed() };
    let mut needed = 0u32;
    let ok = unsafe {
        QueryServiceStatusEx(
            service.0,
            SC_STATUS_PROCESS_INFO,
            (&raw mut raw).cast::<u8>(),
            size_of::<SERVICE_STATUS_PROCESS>() as u32,
            &mut needed,
        )
    };
    if ok == 0 {
        return None;
    }
    let state = match raw.dwCurrentState {
        SERVICE_RUNNING => State::Running,
        SERVICE_START_PENDING | SERVICE_CONTINUE_PENDING => State::Starting,
        SERVICE_STOP_PENDING | SERVICE_PAUSE_PENDING => State::Stopping,
        _ => State::Stopped,
    };
    Some(Status { state, pid: raw.dwProcessId, exit_code: raw.dwWin32ExitCode, specific: raw.dwServiceSpecificExitCode })
}

/// Servisin durumu. Servis yoksa ya da sorgulanamıyorsa `State::Missing`.
pub fn query(name: &str) -> Status {
    match open(name, SERVICE_QUERY_STATUS) {
        Ok(service) => query_handle(&service).unwrap_or(MISSING),
        Err(_) => MISSING,
    }
}

pub fn start(name: &str) -> Result<(), u32> {
    let service = open(name, SERVICE_START)?;
    // SAFETY: bağımsız değişken yok (0, null).
    if unsafe { StartServiceW(service.0, 0, null()) } == 0 {
        let error = last_error();
        if error != ERROR_SERVICE_ALREADY_RUNNING {
            return Err(error);
        }
    }
    Ok(())
}

pub fn stop(name: &str) -> Result<(), u32> {
    let service = match open(name, SERVICE_STOP) {
        Ok(service) => service,
        Err(ERROR_SERVICE_DOES_NOT_EXIST) => return Ok(()),
        Err(error) => return Err(error),
    };
    // SAFETY: status düz veridir.
    let mut status: SERVICE_STATUS = unsafe { zeroed() };
    if unsafe { ControlService(service.0, SERVICE_CONTROL_STOP, &mut status) } == 0 {
        let error = last_error();
        if error != ERROR_SERVICE_NOT_ACTIVE {
            return Err(error);
        }
    }
    Ok(())
}

/// Servis istenen duruma gelene kadar bekler. Süre dolarsa false.
pub fn wait_for(name: &str, wanted: State, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if query(name).state == wanted {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

fn set_dacl(service: &Handle, sddl: &str) -> Result<(), u32> {
    let sddl = wide(sddl);
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: descriptor başarılı çağrıda LocalAlloc ile ayrılır ve aşağıda LocalFree ile bırakılır.
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut descriptor, null_mut()) == 0 {
            return Err(last_error());
        }
        let ok = SetServiceObjectSecurity(service.0, DACL_SECURITY_INFORMATION, descriptor);
        let error = last_error();
        LocalFree(descriptor);
        if ok == 0 {
            return Err(error);
        }
    }
    Ok(())
}

/// Servisi kurar; zaten varsa komut satırını günceller. Elle başlatılan (demand) ayrı süreçli bir servistir.
/// `sddl`: servisin erişim listesi. Yönetici yetkisi gerekir.
pub fn install(name: &str, display: &str, description: &str, command_line: &str, sddl: &str) -> Result<(), u32> {
    let scm = manager(SC_MANAGER_CONNECT | SC_MANAGER_CREATE_SERVICE)?;
    let (wname, wdisplay, wcommand) = (wide(name), wide(display), wide(command_line));
    // SAFETY: bütün dizgiler sıfırla biten UTF-16'dır ve çağrı boyunca yaşar; kullanılmayan alanlar null'dır.
    let service = unsafe {
        let existing = OpenServiceW(scm.0, wname.as_ptr(), SERVICE_ALL_ACCESS);
        if existing.is_null() {
            let error = last_error();
            if error != ERROR_SERVICE_DOES_NOT_EXIST {
                return Err(error);
            }
            let created = CreateServiceW(
                scm.0,
                wname.as_ptr(),
                wdisplay.as_ptr(),
                SERVICE_ALL_ACCESS,
                SERVICE_WIN32_OWN_PROCESS,
                SERVICE_DEMAND_START,
                SERVICE_ERROR_NORMAL,
                wcommand.as_ptr(),
                null(),
                null_mut(),
                null(),
                null(),
                null(),
            );
            if created.is_null() {
                return Err(last_error());
            }
            Handle(created)
        } else {
            let service = Handle(existing);
            let ok = ChangeServiceConfigW(
                service.0,
                SERVICE_WIN32_OWN_PROCESS,
                SERVICE_DEMAND_START,
                SERVICE_ERROR_NORMAL,
                wcommand.as_ptr(),
                null(),
                null_mut(),
                null(),
                null(),
                null(),
                wdisplay.as_ptr(),
            );
            if ok == 0 {
                return Err(last_error());
            }
            service
        }
    };

    let mut wdescription = wide(description);
    let info = SERVICE_DESCRIPTIONW { lpDescription: wdescription.as_mut_ptr() };
    // SAFETY: info ve gösterdiği dizgi çağrı boyunca yaşar. Açıklama yazılamazsa servis yine çalışır.
    unsafe { ChangeServiceConfig2W(service.0, SERVICE_CONFIG_DESCRIPTION, (&raw const info).cast::<c_void>()) };

    set_dacl(&service, sddl)
}

/// Servisi durdurur ve siler. Servis yoksa başarılıdır. Yönetici yetkisi gerekir.
pub fn delete(name: &str) -> Result<(), u32> {
    let service = match open(name, DELETE | SERVICE_STOP | SERVICE_QUERY_STATUS) {
        Ok(service) => service,
        Err(ERROR_SERVICE_DOES_NOT_EXIST) => return Ok(()),
        Err(error) => return Err(error),
    };
    // SAFETY: status düz veridir; durdurma isteği başarısız olsa da (zaten duruyorsa) silmeye geçilir.
    unsafe {
        let mut status: SERVICE_STATUS = zeroed();
        ControlService(service.0, SERVICE_CONTROL_STOP, &mut status);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while query_handle(&service).is_some_and(|s| s.state != State::Stopped) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(150));
    }
    // SAFETY: tutamak DELETE hakkıyla açıldı.
    if unsafe { DeleteService(service.0) } == 0 {
        return Err(last_error());
    }
    Ok(())
}

/// Adı verilenlerden biri olan çalışan süreçler: (exe adı, süreç kimliği).
pub fn processes_named(names: &[&str]) -> Vec<(String, u32)> {
    let mut found = Vec::new();
    // SAFETY: anlık görüntü tutamağı aşağıda kapatılır; entry düz veridir ve boyutu yazılmıştır.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return found;
        }
        let mut entry: PROCESSENTRY32W = zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                let exe = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if names.iter().any(|name| name.eq_ignore_ascii_case(&exe)) {
                    found.push((exe, entry.th32ProcessID));
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
    }
    found
}

/// Süreci sonlandırır. Yönetici yetkisi gerekebilir.
pub fn terminate(pid: u32) -> Result<(), u32> {
    // SAFETY: tutamak yalnız sonlandırma hakkıyla açılır ve kapatılır.
    unsafe {
        let process = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if process.is_null() {
            return Err(last_error());
        }
        let ok = TerminateProcess(process, 1);
        let error = last_error();
        CloseHandle(process);
        if ok == 0 {
            return Err(error);
        }
    }
    Ok(())
}
