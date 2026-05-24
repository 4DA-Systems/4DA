// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Individual component health checks for the 4DA health monitoring system.
//!
//! Extracted from `health.rs` — each function probes a single subsystem
//! (scanner, watcher, git, database, embedding) and returns a `ComponentHealth`.

use rusqlite::Connection;
use tracing::{debug, warn};

use super::{ComponentHealth, HealthStatus};

// ============================================================================
// Individual Component Checks
// ============================================================================

/// Check scanner health: detected_tech count > 0
pub(super) fn check_scanner(conn: &Connection, now: &str) -> ComponentHealth {
    let result: std::result::Result<i64, _> =
        conn.query_row("SELECT COUNT(*) FROM detected_tech", [], |row| row.get(0));

    match result {
        Ok(count) if count > 0 => {
            debug!(target: "4da::health", count, "Scanner healthy: technologies detected");
            ComponentHealth {
                name: "scanner".into(),
                status: HealthStatus::Healthy,
                last_check: now.into(),
                error_message: None,
            }
        }
        Ok(_) => {
            debug!(target: "4da::health", "Scanner degraded: no technologies detected yet");
            ComponentHealth {
                name: "scanner".into(),
                status: HealthStatus::Failed,
                last_check: now.into(),
                error_message: Some("No technologies detected".into()),
            }
        }
        Err(e) => {
            warn!(target: "4da::health", error = %e, "Scanner check failed");
            ComponentHealth {
                name: "scanner".into(),
                status: HealthStatus::Failed,
                last_check: now.into(),
                error_message: Some(format!("Query error: {e}")),
            }
        }
    }
}

/// Check file watcher health: recent file_signals
pub(super) fn check_watcher(conn: &Connection, now: &str) -> ComponentHealth {
    // Check for signals in the last hour
    let recent: std::result::Result<i64, _> = conn.query_row(
        "SELECT COUNT(*) FROM file_signals WHERE timestamp > datetime('now', '-1 hour')",
        [],
        |row| row.get(0),
    );

    match recent {
        Ok(count) if count > 0 => {
            debug!(target: "4da::health", count, "Watcher healthy: recent signals");
            ComponentHealth {
                name: "watcher".into(),
                status: HealthStatus::Healthy,
                last_check: now.into(),
                error_message: None,
            }
        }
        Ok(_) => {
            // No recent signals - check if any signals exist at all
            let total: std::result::Result<i64, _> =
                conn.query_row("SELECT COUNT(*) FROM file_signals", [], |row| row.get(0));

            match total {
                Ok(t) if t > 0 => {
                    debug!(target: "4da::health", total = t, "Watcher degraded: no recent signals");
                    ComponentHealth {
                        name: "watcher".into(),
                        status: HealthStatus::Degraded,
                        last_check: now.into(),
                        error_message: Some("No file signals in last hour".into()),
                    }
                }
                _ => {
                    debug!(target: "4da::health", "Watcher failed: no signals ever recorded");
                    ComponentHealth {
                        name: "watcher".into(),
                        status: HealthStatus::Failed,
                        last_check: now.into(),
                        error_message: Some("No file signals recorded".into()),
                    }
                }
            }
        }
        Err(e) => {
            warn!(target: "4da::health", error = %e, "Watcher check failed");
            ComponentHealth {
                name: "watcher".into(),
                status: HealthStatus::Failed,
                last_check: now.into(),
                error_message: Some(format!("Query error: {e}")),
            }
        }
    }
}

/// Check git analyzer health: git_signals exist
pub(super) fn check_git(conn: &Connection, now: &str) -> ComponentHealth {
    let result: std::result::Result<i64, _> =
        conn.query_row("SELECT COUNT(*) FROM git_signals", [], |row| row.get(0));

    match result {
        Ok(count) if count > 0 => {
            debug!(target: "4da::health", count, "Git analyzer healthy");
            ComponentHealth {
                name: "git".into(),
                status: HealthStatus::Healthy,
                last_check: now.into(),
                error_message: None,
            }
        }
        Ok(_) => {
            debug!(target: "4da::health", "Git analyzer degraded: no signals");
            ComponentHealth {
                name: "git".into(),
                status: HealthStatus::Degraded,
                last_check: now.into(),
                error_message: Some("No git signals recorded".into()),
            }
        }
        Err(e) => {
            warn!(target: "4da::health", error = %e, "Git check failed");
            ComponentHealth {
                name: "git".into(),
                status: HealthStatus::Failed,
                last_check: now.into(),
                error_message: Some(format!("Query error: {e}")),
            }
        }
    }
}

/// Check database health: simple SELECT 1
pub(super) fn check_database(conn: &Connection, now: &str) -> ComponentHealth {
    let result: std::result::Result<i64, _> = conn.query_row("SELECT 1", [], |row| row.get(0));

    match result {
        Ok(1) => ComponentHealth {
            name: "database".into(),
            status: HealthStatus::Healthy,
            last_check: now.into(),
            error_message: None,
        },
        Ok(other) => ComponentHealth {
            name: "database".into(),
            status: HealthStatus::Degraded,
            last_check: now.into(),
            error_message: Some(format!("Unexpected result: {other}")),
        },
        Err(e) => {
            warn!(target: "4da::health", error = %e, "Database check failed");
            ComponentHealth {
                name: "database".into(),
                status: HealthStatus::Failed,
                last_check: now.into(),
                error_message: Some(format!("Database error: {e}")),
            }
        }
    }
}

/// Check embedding availability.
///
/// Uses the capabilities system as ground truth when available (it tracks real
/// embedding success/failure at runtime). Falls back to a config-based heuristic
/// for the initial check before any embedding has been attempted.
///
/// The embedding pipeline always tries Ollama as a fallback for providers that
/// don't have a native embedding API (e.g. Anthropic) or when no cloud key is
/// configured. So the health check should not report "degraded" just because
/// there's no cloud API key — Ollama may be handling embeddings successfully.
pub(super) fn check_embedding(now: &str) -> ComponentHealth {
    // ----- 1. Consult the capabilities system (runtime ground truth) -----
    //
    // After the first real embedding attempt, the embedding pipeline updates
    // the EmbeddingSearch capability state. If it's been explicitly degraded
    // or restored, that's the authoritative answer.
    let cap_state = crate::capabilities::get_all_states()
        .get(&crate::capabilities::Capability::EmbeddingSearch)
        .cloned();

    match &cap_state {
        Some(crate::capabilities::CapabilityState::Degraded {
            reason, fallback, ..
        }) => {
            // The embedding system has explicitly reported degradation.
            return ComponentHealth {
                name: "embedding".into(),
                status: HealthStatus::Degraded,
                last_check: now.into(),
                error_message: Some(format!("{reason} ({fallback})")),
            };
        }
        Some(crate::capabilities::CapabilityState::Unavailable { reason, .. }) => {
            return ComponentHealth {
                name: "embedding".into(),
                status: HealthStatus::Failed,
                last_check: now.into(),
                error_message: Some(reason.clone()),
            };
        }
        // Full — either embeddings are working, or no attempt has been made yet.
        // Fall through to the config-based heuristic for a plausibility check.
        _ => {}
    }

    // ----- 2. Config-based heuristic (startup / pre-first-embed) -----
    let settings_mgr = crate::get_settings_manager();
    let settings = settings_mgr.lock();
    let llm = &settings.get().llm;

    let has_openai_key =
        !llm.openai_api_key.is_empty() || (llm.provider == "openai" && !llm.api_key.is_empty());
    let has_ollama = llm.provider == "ollama"
        || llm
            .base_url
            .as_ref()
            .is_some_and(|u| u.contains("localhost") || u.contains("127.0.0.1"));

    // The embedding pipeline always falls back to Ollama at localhost:11434 for
    // "anthropic", "none", and unknown providers. That fallback works without any
    // explicit configuration — Ollama just needs to be running. We can't verify
    // Ollama connectivity synchronously here, but we should not report "degraded"
    // for a config that has a valid fallback path.
    let provider_has_ollama_fallback = matches!(
        llm.provider.as_str(),
        "anthropic" | "none" | "local" | "builtin" | ""
    );

    if has_openai_key || has_ollama || provider_has_ollama_fallback {
        ComponentHealth {
            name: "embedding".into(),
            status: HealthStatus::Healthy,
            last_check: now.into(),
            error_message: None,
        }
    } else {
        // Provider is something unusual with no known embedding fallback and no
        // cloud key configured. Report degraded — the capabilities system will
        // correct this if Ollama turns out to be reachable.
        ComponentHealth {
            name: "embedding".into(),
            status: HealthStatus::Degraded,
            last_check: now.into(),
            error_message: Some(
                "No embedding provider available (configure API key or install Ollama)".into(),
            ),
        }
    }
}
