//! Windows dışındaki sistemler: motorun sürücü katmanı henüz yok (macOS desteği sonraki sürümde). Arayüz derlenir
//! ve açılır, motor işlemleri "unsupported" hatası verir. Paket mantığı (logic.rs) her platformda derlenir ve
//! test edilir.

use super::{Conflict, Status};
use crate::error::{DpiError, DpiResult};
use crate::plan::EngineRequest;
use serde_json::Value;
use std::path::Path;

pub const SUPPORTED: bool = false;

pub fn engine_id() -> String {
    String::new()
}

pub fn available() -> bool {
    false
}

pub fn status() -> Status {
    Status::MISSING
}

pub fn applied_fingerprint() -> Option<String> {
    None
}

pub fn start() -> DpiResult<()> {
    Err(DpiError::new("unsupported"))
}

pub fn stop() -> DpiResult<()> {
    Err(DpiError::new("unsupported"))
}

pub fn elevated(_action: &str, _request: Option<&EngineRequest>, _jobs_dir: &Path) -> DpiResult<()> {
    Err(DpiError::new("unsupported"))
}

pub fn conflicts(_own_pid: u32) -> Vec<Conflict> {
    Vec::new()
}

pub fn stats() -> Option<Value> {
    None
}

pub fn log_tail(_max_bytes: usize) -> String {
    String::new()
}

pub fn stop_reason() -> Option<u32> {
    None
}

pub fn helper_main(_args: &[String]) -> i32 {
    1
}

pub fn engine_main() -> i32 {
    1
}
