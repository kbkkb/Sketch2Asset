use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorCode {
    InvalidInput,
    NotFound,
    MissingApiKey,
    ProviderAuth,
    ProviderRateLimited,
    ProviderBadResponse,
    ProviderUnavailable,
    Network,
    ImageDecode,
    MattingFailed,
    ModelMissing,
    Io,
    Database,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct AppError {
    pub code: AppErrorCode,
    pub message: String,
    pub details: Option<String>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: AppErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(code: AppErrorCode, message: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: Some(redact(&details.into())),
        }
    }
}

pub fn redact(input: &str) -> String {
    let mut out = input.to_string();
    if let Ok(re) = regex::Regex::new(r"sk-[A-Za-z0-9_\-]{8,}") {
        out = re.replace_all(&out, "sk-***REDACTED***").to_string();
    }
    if out.chars().count() > 500 {
        out.chars().take(500).collect::<String>() + "..."
    } else {
        out
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::new(AppErrorCode::Io, e.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        Self::new(AppErrorCode::Database, e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        Self::new(AppErrorCode::ProviderUnavailable, e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::new(AppErrorCode::Internal, e.to_string())
    }
}
