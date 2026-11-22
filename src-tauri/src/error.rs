//! Arayüze giden hata: kısa bir kod ("cancelled", "engine-missing" …) ve isteğe bağlı ayrıntı.
//! Kodun kullanıcıya gösterilecek metni arayüzün dil dosyalarındadır (ui/js/locales).

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DpiError {
    pub code: String,
    pub detail: Option<String>,
}

impl DpiError {
    pub fn new(code: &str) -> Self {
        Self { code: code.to_string(), detail: None }
    }

    pub fn with(code: &str, detail: impl fmt::Display) -> Self {
        Self { code: code.to_string(), detail: Some(detail.to_string()) }
    }

    pub fn is(&self, code: &str) -> bool {
        self.code == code
    }
}

impl fmt::Display for DpiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.detail {
            Some(detail) => write!(f, "{}: {}", self.code, detail),
            None => write!(f, "{}", self.code),
        }
    }
}

impl std::error::Error for DpiError {}

pub type DpiResult<T> = Result<T, DpiError>;
