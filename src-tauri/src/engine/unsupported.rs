//! Windows dışındaki sistemler: motor henüz yok (macOS desteği sonraki sürümde). Arayüz derlenir ve açılır,
//! motor işlemleri "unsupported" hatası verir.

use super::{Conflict, Status};
use crate::error::{DpiError, DpiResult};
use crate::plan::EngineRequest;
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

pub fn helper_main(_args: &[String]) -> i32 {
    1
}
