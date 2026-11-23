//! Derleme anında WinDivert dosyalarının SHA-256 özetlerini ikili dosyaya gömer.
//!
//! Yönetici yardımcısı (src/engine/helper.rs) sürücüyü korumalı klasöre kopyalamadan önce her dosyayı bu özetlerle
//! karşılaştırır: kurulum klasöründeki bir dosya sonradan değiştirilmişse yönetici yetkisiyle yüklenmez.

use sha2::{Digest, Sha256};
use std::path::Path;

const ENGINE_FILES: [(&str, &str); 2] = [("WinDivert.dll", "ROBZDPI_SHA_DLL"), ("WinDivert64.sys", "ROBZDPI_SHA_SYS")];

fn main() {
    println!("cargo:rerun-if-changed=engine");
    let engine = Path::new(env!("CARGO_MANIFEST_DIR")).join("engine");
    // WinDivert yalnız Windows'ta kullanılır; Linux çekirdeğin kendi paket kuyruğunu kullanır.
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    for (name, variable) in ENGINE_FILES {
        let digest = match std::fs::read(engine.join(name)) {
            Ok(bytes) => hex::encode(Sha256::digest(&bytes)),
            Err(_) => {
                if windows {
                    println!("cargo:warning=WinDivert dosyası yok: engine/{name} (önce `npm run engine`)");
                }
                String::new()
            }
        };
        println!("cargo:rustc-env={variable}={digest}");
    }
    tauri_build::build()
}
