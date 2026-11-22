//! Motor: paketleri değiştiren asıl program (Windows'ta GoodbyeDPI + WinDivert) ve onu yöneten Windows servisi.
//!
//! Güvenlik düzeni:
//!   * Arayüz süreci yönetici yetkisi olmadan çalışır. Servisi yalnız başlatabilir, durdurabilir ve sorgulayabilir.
//!   * Kurulum ve ayar değişikliği için aynı exe `--helper` ile, Windows'un yönetici onayı (UAC) alınarak yeniden
//!     çalıştırılır (helper.rs). Yardımcı isteği yeniden doğrular ve komut satırını kendisi kurar.
//!   * Motor dosyaları yalnız yöneticilerin yazabildiği "Program Files\RobzDPI" klasörüne kopyalanır; servis oradan
//!     çalışır. Kopyalamadan önce her dosyanın özeti derleme anında gömülen özetle karşılaştırılır.

#[cfg(windows)]
mod helper;
#[cfg(windows)]
mod scm;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::*;

#[cfg(not(windows))]
mod unsupported;
#[cfg(not(windows))]
pub use unsupported::*;

use serde::Serialize;

/// Servisin o anki durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Status {
    /// Servis kurulu mu (en az bir kez yönetici onayıyla uygulandı mı)?
    pub installed: bool,
    /// "stopped" | "starting" | "running" | "stopping"
    pub state: &'static str,
    pub pid: u32,
}

impl Status {
    pub const MISSING: Status = Status { installed: false, state: "stopped", pid: 0 };

    pub fn running(&self) -> bool {
        self.state == "running"
    }
}

/// Aynı anda çalışan başka bir DPI aracı (örneğin kullanıcının elle başlattığı goodbyedpi.exe).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Conflict {
    pub name: String,
    pub pid: u32,
}
