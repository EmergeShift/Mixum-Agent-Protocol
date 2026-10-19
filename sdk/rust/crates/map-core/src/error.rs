//! Wire error codes (envelope.md). Stable, programmatic identifiers.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    AlreadyExists,
    ReplayDetected,
    Busy,
    Internal,
    Unsupported,
    RateLimited,
    Timeout,
    Unavailable,
    SnapshotInvalid,
}

/// Client-side error: either a wire `.err` reply or a transport failure.
#[derive(Debug, thiserror::Error)]
pub enum MapError {
    #[error("[{code:?}] {message}")]
    Wire { code: ErrorCode, message: String },
    #[error("transport: {0}")]
    Transport(String),
    #[error("request timed out")]
    Timeout,
}

impl MapError {
    pub fn code(&self) -> Option<ErrorCode> {
        match self {
            MapError::Wire { code, .. } => Some(*code),
            _ => None,
        }
    }
}
