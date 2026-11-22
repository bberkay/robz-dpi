//! Derleme anında motor dosyalarının SHA-256 özetlerini ikili dosyaya gömer.
//!
//! Yönetici yardımcısı (src/engine/helper.rs) motoru korumalı klasöre kopyalamadan önce her dosyayı bu özetlerle
//! karşılaştırır: kurulum klasöründeki bir dosya sonradan değiştirilmişse yönetici yetkisiyle çalıştırılmaz.

use sha2::{Digest, Sha256};
use std::path::Path;

const ENGINE_FILES: [(&str, &str); 3] = [
    ("goodbyedpi.exe", "ROBZDPI_SHA_EXE"),
    ("WinDivert.dll", "ROBZDPI_SHA_DLL"),
    ("WinDivert64.sys", "ROBZDPI_SHA_SYS"),
];

fn main() {
    println!("cargo:rerun-if-changed=engine");
    let engine = Path::new(env!("CARGO_MANIFEST_DIR")).join("engine");
    for (name, variable) in ENGINE_FILES {
        let digest = match std::fs::read(engine.join(name)) {
            Ok(bytes) => hex::encode(Sha256::digest(&bytes)),
            Err(_) => {
                println!("cargo:warning=motor dosyası yok: engine/{name} (önce `npm run engine`)");
                String::new()
            }
        };
        println!("cargo:rustc-env={variable}={digest}");
    }
    tauri_build::build()
}
