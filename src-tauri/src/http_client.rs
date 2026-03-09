//! Shared HTTP client for general-purpose outbound requests.
//!
//! Provides a global `reqwest::Client` with sensible defaults for most use cases.
//!
//! **When NOT to use this client:**
//! - `embeddings.rs` — needs 90s timeout for large embedding batches
//! - `llm.rs` — needs dynamic timeouts (60s cloud, 120s Ollama cold start)
//! - `settings_commands.rs::pull_ollama_model` — needs 600s timeout for model downloads
//! - `sources/mod.rs` — already has its own `SHARED_CLIENT` with identical config
//!
//! For those cases, keep the purpose-built client and document why.

use std::sync::LazyLock;
use std::time::Duration;

/// Global HTTP client with shared connection pool and TLS session cache.
/// Suitable for health checks, license validation, API probes, and other
/// general-purpose requests where the default 30s timeout is appropriate.
pub(crate) static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent("4DA/1.0")
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to build HTTP client: {e}, using default");
            reqwest::Client::new()
        })
});
