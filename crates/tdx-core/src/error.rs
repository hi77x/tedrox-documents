//! Error type shared by every engine crate.

use std::path::Path;

/// Stable, log-safe error categories. Logs may include the category and the
/// operation id, but never document contents, passwords or full private paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Io,
    Unsupported,
    InvalidInput,
    Encrypted,
    WrongPassword,
    NotFound,
    Permission,
    InsufficientSpace,
    Cancelled,
    Corrupt,
    AdapterRequired,
    Internal,
}

impl ErrorCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCategory::Io => "io",
            ErrorCategory::Unsupported => "unsupported",
            ErrorCategory::InvalidInput => "invalid_input",
            ErrorCategory::Encrypted => "encrypted",
            ErrorCategory::WrongPassword => "wrong_password",
            ErrorCategory::NotFound => "not_found",
            ErrorCategory::Permission => "permission",
            ErrorCategory::InsufficientSpace => "insufficient_space",
            ErrorCategory::Cancelled => "cancelled",
            ErrorCategory::Corrupt => "corrupt",
            ErrorCategory::AdapterRequired => "adapter_required",
            ErrorCategory::Internal => "internal",
        }
    }
}

/// Unified error type. Messages are user-facing and written in plain English;
/// localization happens in the UI layer through [`TdxError::category`].
#[derive(Debug, thiserror::Error)]
pub enum TdxError {
    #[error("File not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    Permission(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unsupported input: {0}")]
    Unsupported(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("This document is encrypted or password protected")]
    Encrypted,

    #[error("Incorrect password for this document")]
    WrongPassword,

    #[error("Not enough disk space: about {needed_mb} MB required, {available_mb} MB available")]
    InsufficientSpace { needed_mb: u64, available_mb: u64 },

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Corrupt or malformed file: {0}")]
    Corrupt(String),

    #[error("This operation requires an optional external adapter: {0}")]
    AdapterRequired(String),

    #[error("{0}")]
    Other(String),
}

impl TdxError {
    pub fn category(&self) -> ErrorCategory {
        match self {
            TdxError::NotFound(_) => ErrorCategory::NotFound,
            TdxError::Permission(_) => ErrorCategory::Permission,
            TdxError::Io(_) => ErrorCategory::Io,
            TdxError::Unsupported(_) => ErrorCategory::Unsupported,
            TdxError::InvalidInput(_) => ErrorCategory::InvalidInput,
            TdxError::Encrypted => ErrorCategory::Encrypted,
            TdxError::WrongPassword => ErrorCategory::WrongPassword,
            TdxError::InsufficientSpace { .. } => ErrorCategory::InsufficientSpace,
            TdxError::Cancelled => ErrorCategory::Cancelled,
            TdxError::Corrupt(_) => ErrorCategory::Corrupt,
            TdxError::AdapterRequired(_) => ErrorCategory::AdapterRequired,
            TdxError::Other(_) => ErrorCategory::Internal,
        }
    }

    /// A short log-safe description. Never includes document content.
    pub fn sanitized(&self) -> String {
        let category = self.category().as_str();
        match self {
            TdxError::Io(err) => format!("{category}: {}", err.kind()),
            TdxError::NotFound(_) | TdxError::Permission(_) => category.to_string(),
            TdxError::Encrypted | TdxError::WrongPassword | TdxError::Cancelled => {
                category.to_string()
            }
            TdxError::InsufficientSpace {
                needed_mb,
                available_mb,
            } => {
                format!("{category}: need={needed_mb}MB available={available_mb}MB")
            }
            TdxError::Unsupported(_) => "unsupported".to_string(),
            TdxError::InvalidInput(_) => "invalid_input".to_string(),
            TdxError::Corrupt(_) => "corrupt".to_string(),
            TdxError::AdapterRequired(name) => format!("{category}: {name}"),
            TdxError::Other(_) => "internal".to_string(),
        }
    }

    pub fn not_found(path: &Path) -> Self {
        TdxError::NotFound(path.display().to_string())
    }

    pub fn other(message: impl Into<String>) -> Self {
        TdxError::Other(message.into())
    }

    pub fn corrupt(message: impl Into<String>) -> Self {
        TdxError::Corrupt(message.into())
    }
}

pub type Result<T> = std::result::Result<T, TdxError>;
