//! robzdpi-engine: Robz DPI'ın Linux'taki motoru ve yönetici yardımcısı.
//!
//!   robzdpi-engine --engine                     motor: systemd birimi olarak (root) paketleri işler
//!   robzdpi-engine --helper <işlem> <iş dosyası>  yönetici yardımcısı: arayüz bunu pkexec ile çalıştırır
//!
//! Kaynak dosyaları uygulamayla ortaktır (../../src); testleri de orada çalışır. Bu pakette arayüz tarafının
//! işlevleri (durum sorma, pkexec ile yükseltme) derlenir ama kullanılmaz; uyarıları bu yüzden kapalıdır.
#![allow(dead_code, unused_imports)]

#[cfg(not(target_os = "linux"))]
compile_error!("robzdpi-engine yalnız Linux içindir; Windows'ta motor uygulamanın kendi exe'sidir");

#[path = "../../src/catalog.rs"]
mod catalog;
#[path = "../../src/engine/mod.rs"]
mod engine;
#[path = "../../src/error.rs"]
mod error;
#[path = "../../src/plan.rs"]
mod plan;
#[path = "../../src/settings.rs"]
mod settings;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = match args.get(1).map(String::as_str) {
        Some("--helper") => engine::helper_main(&args[2..]),
        Some("--engine") => engine::engine_main(),
        _ => {
            eprintln!("robzdpi-engine {}: Robz DPI motoru. Robz DPI uygulaması tarafından çalıştırılır.", env!("CARGO_PKG_VERSION"));
            2
        }
    };
    std::process::exit(code);
}
