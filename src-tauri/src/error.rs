//! Unified error type for 4DA.
//!
//! Replaces ad-hoc `Result<_, String>` with a typed enum.
//! Implements `serde::Serialize` for Tauri command compatibility.

use serde::Serialize;

pub type Result<T> = std::result::Result<T, FourDaError>;

#[derive(Debug, thiserror::Error)]
pub enum FourDaError {
    /// SQLite / database errors
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    /// JSON serialization / deserialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP request errors
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Filesystem I/O errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration or settings errors
    #[error("Config error: {0}")]
    Config(String),

    /// Resource not initialized (e.g., ACE engine, database)
    #[error("{0}")]
    NotInitialized(String),

    /// Analysis pipeline errors
    #[error("Analysis error: {0}")]
    Analysis(String),

    /// LLM / embedding provider errors
    #[error("LLM error: {0}")]
    Llm(String),

    /// Generic internal error (catch-all, bridges legacy `Result<_, String>`)
    #[error("{0}")]
    Internal(String),
}

// Tauri v2: commands returning Result<T, E> require E: Serialize
impl Serialize for FourDaError {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// Bridge: lets `?` work on functions still returning Result<_, String>
impl From<String> for FourDaError {
    fn from(s: String) -> Self {
        FourDaError::Internal(s)
    }
}

impl From<&str> for FourDaError {
    fn from(s: &str) -> Self {
        FourDaError::Internal(s.to_string())
    }
}
