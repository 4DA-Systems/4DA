//! Shared HTTP clients for outbound requests.
//!
//! Three pooled clients with distinct timeout profiles:
//! - `HTTP_CLIENT` — general-purpose, 30s timeout, 10s connect
//! - `PROBE_CLIENT` — health checks & API validation, 15s timeout, 5s connect
//! - `TEAM_CLIENT` — team relay operations, 15s timeout, TeamSync user-agent
//!
//! **When NOT to use these clients (keep purpose-built):**
//! - `embeddings.rs` — needs 90s timeout for large embedding batches
//! - `llm.rs` — needs dynamic timeouts (60s cloud, 120s Ollama cold start)
//! - `settings_commands_llm.rs::test_ollama_connection_impl` — 120s for cold model load
//! - `settings_commands_llm.rs::pull_ollama_model` — 600s for model downloads
//! - `settings_commands_llm.rs::detect_local_servers` — 3s intentionally fast probe
//! - `team_sync_scheduler.rs` — 30s timeout for background sync cycles
//! - `webhooks.rs::deliver_webhook` — 10s timeout for fire-and-forget
//! - `calibration_commands.rs` — 3s quick Ollama check
//! - `sources/mod.rs` — already has its own `SHARED_CLIENT` with identical config

use std::sync::LazyLock;
use std::time::Duration;

/// Global HTTP client with shared connection pool and TLS session cache.
/// Suitable for health checks, license validation, API probes, and other
/// general-purpose requests where the default 30s timeout is appropriate.
pub(crate) static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; desktop-app)")
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to build HTTP client: {e}, using default");
            reqwest::Client::new()
        })
});

/// Shared client for quick health checks, status probes, and API validation.
/// Tight timeouts prevent blocking on unresponsive services.
pub(crate) static PROBE_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; desktop-app)")
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to build probe client: {e}, using default");
            reqwest::Client::new()
        })
});

/// Shared client for team relay operations (sync, create, join).
/// Uses team-specific user-agent for relay identification.
#[allow(dead_code)]
pub(crate) static TEAM_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent("4DA-TeamSync/1.0")
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to build team client: {e}, using default");
            reqwest::Client::new()
        })
});
