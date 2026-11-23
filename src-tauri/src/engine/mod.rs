//! Motor: paketleri değiştiren program ve onu yöneten Windows servisi.
//!
//! Aynı exe üç kipte çalışır:
//!   * (bağımsız değişkensiz)  arayüz: yönetici yetkisi yoktur; servisi yalnız başlatır, durdurur ve sorgular.
//!   * `--helper …`            yönetici yardımcısı: Windows'un onayıyla (UAC) çalışır, servisi kurar (helper.rs).
//!   * `--engine`              motorun kendisi: Windows servisi olarak SYSTEM yetkisiyle çalışır (service.rs).
//!
//! Katmanlar:
//!   packet, tls, dnsmsg   ham baytları okuyan ve yazan güvenli kod (unsafe yok, her platformda test edilir)
//!   logic                 karar: bu pakete ne yapılacak? (saf mantık)
//!   filter                sürücüye verilen süzgeç: motora hangi paketler gelsin?
//!   divert                WinDivert.dll bağlayıcısı (paketleri çekirdekten alan imzalı sürücü)
//!   service               servis döngüsü: paketi al, karar ver, geri bırak
//!
//! Güvenlik düzeni:
//!   * Motor yalnız yöneticilerin yazabildiği "Program Files\RobzDPI" klasöründen çalışır. Oraya kopyalanan exe,
//!     yönetici onayı verilmiş sürecin kendi görüntüsüdür; WinDivert dosyaları derleme anında gömülen SHA-256
//!     özetleriyle doğrulanır.
//!   * Yardımcı ve motor, arayüzden gelen isteği yeniden doğrular (`EngineRequest::validate`).

pub mod dnsmsg;
#[cfg(windows)]
pub mod filter;
pub mod logic;
pub mod packet;
pub mod tls;

#[cfg(any(windows, target_os = "linux"))]
mod common;

#[cfg(windows)]
pub mod divert;
#[cfg(windows)]
mod helper;
#[cfg(windows)]
mod scm;
#[cfg(windows)]
mod service;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::*;

// Linux: WinDivert'in yerinde çekirdeğin paket kuyruğu (nfqueue), Windows servisinin yerinde systemd birimi,
// UAC'nin yerinde polkit (pkexec) vardır.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod linux_helper;
#[cfg(target_os = "linux")]
mod linux_service;
#[cfg(target_os = "linux")]
mod nfqueue;

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(not(any(windows, target_os = "linux")))]
mod unsupported;
#[cfg(not(any(windows, target_os = "linux")))]
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
