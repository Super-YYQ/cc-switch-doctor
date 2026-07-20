use serde::Serialize;
use thiserror::Error;

/// Public errors safe to send across the Tauri boundary (already redacted).
#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum PublicError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    UnsupportedSchema(String),
    #[error("{0}")]
    Database(String),
    #[error("{0}")]
    InvalidRequest(String),
    #[error("{0}")]
    Busy(String),
    #[error("{0}")]
    Cancelled(String),
    #[error("{0}")]
    Network(String),
    #[error("{0}")]
    Internal(String),
}

impl PublicError {
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

pub type PublicResult<T> = Result<T, PublicError>;
