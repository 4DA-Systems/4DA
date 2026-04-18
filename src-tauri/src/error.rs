// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Unified error type for 4DA.
//!
//! Replaces ad-hoc `Result<_, String>` with a typed enum.
//! Implements `serde::Serialize` for Tauri command compatibility.

use serde::Serialize;

pub type Result<T> = std::result::Result<T, FourDaError>;

/// Structured error information for the frontend.
/// Every error shown to users goes through this — no raw backend strings.
#[derive(Debug, Clone, Serialize)]
pub struct UserError {
    /// Stable error code (e.g., "E2001") — can be documented and googled
    pub code: &'static str,
    /// User-friendly title (short, translatable)
    pub title: String,
    /// Detailed explanation of what went wrong
    pub detail: String,
    /// Actionable steps the user can take to fix the issue
    pub remediation: Vec<String>,
    /// Severity level
    pub severity: ErrorSeverity,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

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

    /// Tauri runtime errors (menus, tray, windows)
    #[error("Tauri error: {0}")]
    Tauri(#[from] tauri::Error),

    /// Generic internal error (catch-all, bridges legacy `Result<_, String>`)
    #[error("{0}")]
    Internal(String),

    /// Input validation errors (URL, path, or data format rejection)
    #[error("Validation error: {0}")]
    Validation(String),
}

// Tauri v2: commands returning Result<T, E> require E: Serialize
// Serializes as a structured UserError object so the frontend receives
// { code, title, detail, remediation, severity } instead of a raw string.
impl Serialize for FourDaError {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let user_error = self.to_user_error();
        // Serialize as a JSON object with code, title, detail, remediation, severity
        user_error.serialize(serializer)
    }
}

impl FourDaError {
    /// Convert an internal error to a user-facing error with actionable guidance.
    pub fn to_user_error(&self) -> UserError {
        match self {
            // Database errors
            FourDaError::Db(e) => {
                let msg = e.to_string();
                if msg.contains("database is locked") {
                    UserError {
                        code: "E3001",
                        title: "Database busy".into(),
                        detail: "The database is temporarily locked by another operation.".into(),
                        remediation: vec![
                            "Wait a moment and try again.".into(),
                            "If this persists, restart 4DA.".into(),
                        ],
                        severity: ErrorSeverity::Warning,
                    }
                } else if msg.contains("disk I/O error") || msg.contains("disk full") {
                    UserError {
                        code: "E3002",
                        title: "Storage error".into(),
                        detail: "Cannot write to the database. Your disk may be full.".into(),
                        remediation: vec![
                            "Free up disk space and try again.".into(),
                            "Check that 4DA's data directory is writable.".into(),
                        ],
                        severity: ErrorSeverity::Critical,
                    }
                } else {
                    UserError {
                        code: "E3000",
                        title: "Database error".into(),
                        detail: "An unexpected database error occurred.".into(),
                        remediation: vec![
                            "Try restarting 4DA.".into(),
                            "If this persists, the database may need to be reset in Settings."
                                .into(),
                        ],
                        severity: ErrorSeverity::Error,
                    }
                }
            }

            // HTTP / network errors
            FourDaError::Http(e) => {
                let msg = e.to_string();
                if e.is_timeout() {
                    UserError {
                        code: "E4001",
                        title: "Connection timed out".into(),
                        detail: "The request took too long to complete.".into(),
                        remediation: vec![
                            "Check your internet connection.".into(),
                            "If using a VPN, try disconnecting temporarily.".into(),
                            "Try again in a moment.".into(),
                        ],
                        severity: ErrorSeverity::Warning,
                    }
                } else if e.is_connect() {
                    UserError {
                        code: "E4002",
                        title: "Connection failed".into(),
                        detail: "Could not connect to the server.".into(),
                        remediation: vec![
                            "Check your internet connection.".into(),
                            "If behind a firewall or proxy, ensure API endpoints are accessible."
                                .into(),
                        ],
                        severity: ErrorSeverity::Warning,
                    }
                } else if msg.contains("429") || msg.contains("rate limit") {
                    UserError {
                        code: "E4003",
                        title: "Rate limited".into(),
                        detail: "Too many requests — the API is asking us to slow down.".into(),
                        remediation: vec![
                            "Wait a few minutes before trying again.".into(),
                            "4DA will automatically retry with backoff.".into(),
                        ],
                        severity: ErrorSeverity::Warning,
                    }
                } else {
                    UserError {
                        code: "E4000",
                        title: "Network error".into(),
                        detail: format!(
                            "A network request failed: {}",
                            truncate_error_msg(&msg, 120)
                        ),
                        remediation: vec![
                            "Check your internet connection.".into(),
                            "Try again in a moment.".into(),
                        ],
                        severity: ErrorSeverity::Error,
                    }
                }
            }

            // IO errors
            FourDaError::Io(e) => {
                let kind = e.kind();
                match kind {
                    std::io::ErrorKind::PermissionDenied => UserError {
                        code: "E3003",
                        title: "Permission denied".into(),
                        detail: "4DA doesn't have permission to access a required file.".into(),
                        remediation: vec![
                            "Check file permissions on 4DA's data directory.".into(),
                            "On Linux/macOS: ensure your user owns the data directory.".into(),
                            "On Windows: try running 4DA as administrator.".into(),
                        ],
                        severity: ErrorSeverity::Error,
                    },
                    std::io::ErrorKind::NotFound => UserError {
                        code: "E3004",
                        title: "File not found".into(),
                        detail: "A required file or directory is missing.".into(),
                        remediation: vec![
                            "Try restarting 4DA — it will recreate missing files.".into()
                        ],
                        severity: ErrorSeverity::Warning,
                    },
                    _ => UserError {
                        code: "E3005",
                        title: "File system error".into(),
                        detail: format!(
                            "File operation failed: {}",
                            truncate_error_msg(&e.to_string(), 120)
                        ),
                        remediation: vec![
                            "Check available disk space.".into(),
                            "Ensure 4DA's data directory is accessible.".into(),
                        ],
                        severity: ErrorSeverity::Error,
                    },
                }
            }

            // LLM errors
            FourDaError::Llm(msg) => {
                if msg.contains("not configured")
                    || msg.contains("no API key")
                    || msg.contains("API key not")
                {
                    UserError {
                        code: "E5001",
                        title: "AI provider not configured".into(),
                        detail: "No AI provider is set up for this operation.".into(),
                        remediation: vec![
                            "Open Settings and configure an AI provider (OpenAI, Anthropic, or Ollama).".into(),
                            "For local AI: install Ollama and run 'ollama pull nomic-embed-text'.".into(),
                        ],
                        severity: ErrorSeverity::Warning,
                    }
                } else if msg.contains("401")
                    || msg.contains("Unauthorized")
                    || msg.contains("invalid")
                {
                    UserError {
                        code: "E5002",
                        title: "Invalid API key".into(),
                        detail: "Your API key was rejected by the provider.".into(),
                        remediation: vec![
                            "Check that your API key is correct in Settings.".into(),
                            "Verify the key hasn't expired on your provider's dashboard.".into(),
                            "Make sure you're using the right key for the selected provider."
                                .into(),
                        ],
                        severity: ErrorSeverity::Error,
                    }
                } else if msg.contains("ModelNotFound") || msg.contains("model") {
                    UserError {
                        code: "E5003",
                        title: "Model not available".into(),
                        detail: "The configured AI model could not be found.".into(),
                        remediation: vec![
                            "If using Ollama: run 'ollama pull <model-name>' to download it."
                                .into(),
                            "Check Settings to verify the model name is correct.".into(),
                        ],
                        severity: ErrorSeverity::Error,
                    }
                } else {
                    UserError {
                        code: "E5000",
                        title: "AI provider error".into(),
                        detail: format!(
                            "The AI provider returned an error: {}",
                            truncate_error_msg(msg, 120)
                        ),
                        remediation: vec![
                            "Check your API key and provider settings.".into(),
                            "If using Ollama, make sure it's running.".into(),
                            "Try again in a moment.".into(),
                        ],
                        severity: ErrorSeverity::Error,
                    }
                }
            }

            // Config errors
            FourDaError::Config(msg) => UserError {
                code: "E2001",
                title: "Configuration error".into(),
                detail: format!("Settings issue: {}", truncate_error_msg(msg, 120)),
                remediation: vec![
                    "Open Settings and check your configuration.".into(),
                    "If settings are corrupted, try resetting to defaults.".into(),
                ],
                severity: ErrorSeverity::Warning,
            },

            // Analysis errors
            FourDaError::Analysis(msg) => UserError {
                code: "E5010",
                title: "Analysis failed".into(),
                detail: format!(
                    "The analysis could not complete: {}",
                    truncate_error_msg(msg, 120)
                ),
                remediation: vec![
                    "Try running the analysis again.".into(),
                    "Check that at least one content source is enabled.".into(),
                    "If the issue persists, check your AI provider settings.".into(),
                ],
                severity: ErrorSeverity::Error,
            },

            // Not initialized
            FourDaError::NotInitialized(msg) => UserError {
                code: "E1001",
                title: "Component not ready".into(),
                detail: format!("{} is still initializing.", truncate_error_msg(msg, 80)),
                remediation: vec![
                    "Wait a moment for 4DA to finish starting up.".into(),
                    "If this persists, restart the application.".into(),
                ],
                severity: ErrorSeverity::Warning,
            },

            // Validation errors (input rejection)
            FourDaError::Validation(msg) => UserError {
                code: "E6001",
                title: "Invalid input".into(),
                detail: truncate_error_msg(msg, 150),
                remediation: vec!["Check the input and try again.".into()],
                severity: ErrorSeverity::Warning,
            },

            // Catch-all
            _ => UserError {
                code: "E9999",
                title: "Unexpected error".into(),
                detail: truncate_error_msg(&self.to_string(), 150),
                remediation: vec![
                    "Try the operation again.".into(),
                    "If this persists, restart 4DA.".into(),
                ],
                severity: ErrorSeverity::Error,
            },
        }
    }
}

/// Truncate an error message to a maximum length, appending "..." if truncated.
fn truncate_error_msg(msg: &str, max_len: usize) -> String {
    if msg.len() <= max_len {
        msg.to_string()
    } else {
        format!("{}...", &msg[..max_len])
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

/// Extension trait for adding context to Results (like anyhow::Context).
///
/// Use `.context("message")` instead of `.map_err(|e| format!("message: {e}"))`.
/// Preserves the error message while providing a consistent, concise pattern.
pub trait ResultExt<T> {
    /// Add static context to an error.
    fn context(self, msg: &str) -> Result<T>;

    /// Add dynamic context to an error (evaluated lazily on failure).
    fn with_context<F: FnOnce() -> String>(self, f: F) -> Result<T>;
}

impl<T, E: std::fmt::Display> ResultExt<T> for std::result::Result<T, E> {
    fn context(self, msg: &str) -> Result<T> {
        self.map_err(|e| FourDaError::Internal(format!("{msg}: {e}")))
    }

    fn with_context<F: FnOnce() -> String>(self, f: F) -> Result<T> {
        self.map_err(|e| FourDaError::Internal(format!("{}: {e}", f())))
    }
}
