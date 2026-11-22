//! WinDivert.dll bağlayıcısı. Kitaplık çalışma anında, tam yoluyla yüklenir; böylece arayüz süreci ona hiç
//! bağımlı olmaz ve Windows'un DLL arama sırası devreye girmez.
//!
//! WinDivert (basil00, LGPL-3.0) paketleri çekirdekte yakalayıp kullanıcı kipine veren imzalı bir sürücüdür.
//! Buradaki bütün `unsafe`, o kitaplığın C işlevlerini çağırmak içindir; paketlerin içeriğini okuyan kod
//! (packet.rs, tls.rs, dnsmsg.rs, logic.rs) güvenli Rust'tır.

use std::ffi::{c_char, c_void, CString};
use std::mem::transmute;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{GetLastError, HANDLE, HMODULE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_WITH_ALTERED_SEARCH_PATH};

/// WINDIVERT_LAYER_NETWORK: IP paketleri
const LAYER_NETWORK: i32 = 0;
/// WINDIVERT_SHUTDOWN_BOTH
const SHUTDOWN_BOTH: i32 = 3;

const FLAG_OUTBOUND: u8 = 0x02;
/// IPChecksum | TCPChecksum | UDPChecksum: "sağlama toplamları geçerli, yeniden hesaplatma".
const FLAG_CHECKSUMS: u8 = 0xe0;

/// WINDIVERT_ADDRESS (WinDivert 2.x, 80 bayt). Paketin yönü ve geldiği ağ arabirimi; paketi geri gönderirken
/// aynı yapı verilir.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Address {
    timestamp: i64,
    layer: u8,
    event: u8,
    /// bit 0 Sniffed, 1 Outbound, 2 Loopback, 3 Impostor, 4 IPv6, 5 IPChecksum, 6 TCPChecksum, 7 UDPChecksum
    flags: u8,
    reserved1: u8,
    reserved2: u32,
    data: [u8; 64],
}

const _: () = assert!(size_of::<Address>() == 80);

impl Address {
    pub fn new(outbound: bool) -> Self {
        Self {
            timestamp: 0,
            layer: 0,
            event: 0,
            flags: if outbound { FLAG_OUTBOUND } else { 0 },
            reserved1: 0,
            reserved2: 0,
            data: [0; 64],
        }
    }

    pub fn outbound(&self) -> bool {
        self.flags & FLAG_OUTBOUND != 0
    }

    /// Motor sağlama toplamlarını kendisi hesaplar (ya da bilerek bozar): sürücü ve ağ kartı dokunmasın.
    pub fn checksums_done(mut self) -> Self {
        self.flags |= FLAG_CHECKSUMS;
        self
    }
}

type OpenFn = unsafe extern "C" fn(*const c_char, i32, i16, u64) -> HANDLE;
type RecvFn = unsafe extern "C" fn(HANDLE, *mut c_void, u32, *mut u32, *mut Address) -> i32;
type SendFn = unsafe extern "C" fn(HANDLE, *const c_void, u32, *mut u32, *const Address) -> i32;
type ShutdownFn = unsafe extern "C" fn(HANDLE, i32) -> i32;
type CloseFn = unsafe extern "C" fn(HANDLE) -> i32;
type CompileFn = unsafe extern "C" fn(*const c_char, i32, *mut c_char, u32, *mut *const c_char, *mut u32) -> i32;
#[cfg(test)]
type EvalFn = unsafe extern "C" fn(*const c_char, *const c_void, u32, *const Address) -> i32;
/// GetProcAddress'in döndürdüğü, imzası henüz bilinmeyen işlev adresi.
type Symbol = unsafe extern "system" fn() -> isize;

/// Yüklenmiş WinDivert.dll. Süreç kapanana kadar bellekte kalır.
pub struct Divert {
    open: OpenFn,
    recv: RecvFn,
    send: SendFn,
    shutdown: ShutdownFn,
    close: CloseFn,
    compile: CompileFn,
    #[cfg(test)]
    eval: EvalFn,
}

/// Açık bir WinDivert tutamağı (sürücüdeki bir süzgeç).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Filter(HANDLE);

// SAFETY: tutamak çekirdek nesnesine bir başvurudur; WinDivert aynı tutamağın bir iş parçacığında okunurken
// başka bir iş parçacığından kapatılmasına (WinDivertShutdown) izin verir.
unsafe impl Send for Filter {}
unsafe impl Sync for Filter {}

impl Filter {
    pub fn as_raw(&self) -> *mut c_void {
        self.0
    }

    pub fn from_raw(raw: *mut c_void) -> Self {
        Self(raw)
    }
}

unsafe fn symbol(module: HMODULE, name: &[u8]) -> Result<Symbol, u32> {
    GetProcAddress(module, name.as_ptr()).ok_or_else(|| GetLastError())
}

impl Divert {
    /// Kitaplığı tam yoluyla yükler. Hata: Win32 hata kodu.
    pub fn load(dll: &Path) -> Result<Self, u32> {
        let path: Vec<u16> = dll.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        // SAFETY: path sıfırla biten UTF-16'dır. Dönen işlev adresleri WinDivert 2.2'nin belgelenmiş imzalarına
        // dönüştürülür (windivert.h); x64'te tek çağrı düzeni vardır.
        unsafe {
            let module = LoadLibraryExW(path.as_ptr(), null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH);
            if module.is_null() {
                return Err(GetLastError());
            }
            Ok(Self {
                open: transmute::<Symbol, OpenFn>(symbol(module, b"WinDivertOpen\0")?),
                recv: transmute::<Symbol, RecvFn>(symbol(module, b"WinDivertRecv\0")?),
                send: transmute::<Symbol, SendFn>(symbol(module, b"WinDivertSend\0")?),
                shutdown: transmute::<Symbol, ShutdownFn>(symbol(module, b"WinDivertShutdown\0")?),
                close: transmute::<Symbol, CloseFn>(symbol(module, b"WinDivertClose\0")?),
                compile: transmute::<Symbol, CompileFn>(symbol(module, b"WinDivertHelperCompileFilter\0")?),
                #[cfg(test)]
                eval: transmute::<Symbol, EvalFn>(symbol(module, b"WinDivertHelperEvalFilter\0")?),
            })
        }
    }

    /// Süzgeci sürücüde açar. İlk çağrıda WinDivert sürücüsü yüklenir (yönetici yetkisi gerekir).
    pub fn open(&self, filter: &str) -> Result<Filter, u32> {
        let filter = CString::new(filter).map_err(|_| 87u32)?;
        // SAFETY: filter çağrı boyunca yaşayan, sıfırla biten bir dizgidir.
        let handle = unsafe { (self.open)(filter.as_ptr(), LAYER_NETWORK, 0, 0) };
        if handle == INVALID_HANDLE_VALUE || handle.is_null() {
            return Err(unsafe { GetLastError() });
        }
        Ok(Filter(handle))
    }

    /// Bir paket bekler ve `buffer`a yazar: (uzunluk, adres). Tutamak kapatıldıysa ya da hata olduysa Err.
    pub fn recv(&self, filter: Filter, buffer: &mut [u8]) -> Result<(usize, Address), u32> {
        let mut length = 0u32;
        let mut address = Address::new(false);
        // SAFETY: buffer ve boyu birlikte verilir; length ve address geçerli yazılabilir adreslerdir.
        let ok = unsafe { (self.recv)(filter.0, buffer.as_mut_ptr().cast(), buffer.len() as u32, &mut length, &mut address) };
        if ok == 0 {
            return Err(unsafe { GetLastError() });
        }
        Ok(((length as usize).min(buffer.len()), address))
    }

    /// Paketi ağa geri bırakır.
    pub fn send(&self, filter: Filter, packet: &[u8], address: &Address) -> bool {
        // SAFETY: packet ve boyu birlikte verilir; address geçerli bir yapıdır.
        unsafe { (self.send)(filter.0, packet.as_ptr().cast(), packet.len() as u32, null_mut(), address) != 0 }
    }

    /// Bekleyen `recv` çağrısını sonlandırır (durdurma isteği başka bir iş parçacığından gelir).
    pub fn shutdown(&self, filter: Filter) {
        // SAFETY: tutamak `open`dan gelmiştir.
        unsafe { (self.shutdown)(filter.0, SHUTDOWN_BOTH) };
    }

    pub fn close(&self, filter: Filter) {
        // SAFETY: tutamak `open`dan gelmiştir ve bir kez kapatılır.
        unsafe { (self.close)(filter.0) };
    }

    /// Süzgeç metnini derlemeyi dener (sürücü gerekmez). Hata: WinDivert'in açıklaması.
    pub fn compile(&self, filter: &str) -> Result<(), String> {
        let text = CString::new(filter).map_err(|_| "içinde sıfır bayt var".to_string())?;
        let mut message: *const c_char = std::ptr::null();
        let mut position = 0u32;
        // SAFETY: çıktı nesnesi istenmez (null, 0); message kitaplığın içindeki sabit bir dizgiyi gösterir.
        let ok = unsafe { (self.compile)(text.as_ptr(), LAYER_NETWORK, null_mut(), 0, &mut message, &mut position) };
        if ok != 0 {
            return Ok(());
        }
        let reason = if message.is_null() {
            String::new()
        } else {
            unsafe { std::ffi::CStr::from_ptr(message) }.to_string_lossy().into_owned()
        };
        Err(format!("{reason} (konum {position})"))
    }

    /// Paketin süzgece uyup uymadığını kullanıcı kipinde hesaplar (sürücü gerekmez). Yalnız testlerde kullanılır.
    #[cfg(test)]
    pub fn eval(&self, filter: &str, packet: &[u8], address: &Address) -> bool {
        let Ok(text) = CString::new(filter) else { return false };
        // SAFETY: packet ve boyu birlikte verilir.
        unsafe { (self.eval)(text.as_ptr(), packet.as_ptr().cast(), packet.len() as u32, address) != 0 }
    }
}
