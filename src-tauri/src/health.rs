// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Health Monitoring for 4DA
//!
//! Checks component status and provides graceful degradation info.
//! Simplified from _future/ace/health.rs - uses ACE engine's DB connection
//! rather than owning one.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::error::Result;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContextQuality {
    Excellent,  // All components healthy, rich context
    Good,       // Minor issues, mostly functional
    Acceptable, // Some components degraded
    Degraded,   // Multiple issues, reduced accuracy
    Minimal,    // Bare minimum functionality
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub last_check: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthReport {
    pub overall_status: HealthStatus,
    pub context_quality: ContextQuality,
    pub components: Vec<ComponentHealth>,
    /// 0=none, 1=reduced, 2=minimal, 3=emergency
    pub fallback_level: u8,
    pub timestamp: String,
}

// ============================================================================
// Public API
// ============================================================================

/// Check all system components and return a health report.
///
/// Accepts a borrowed rusqlite Connection (from ACE engine's conn.lock()).
pub fn check_all_components(conn: &Connection) -> Result<SystemHealthReport> {
    let now = chrono::Utc::now().to_rfc3339();
    let components = vec![
        check_scanner(conn, &now),
        check_watcher(conn, &now),
        check_git(conn, &now),
        check_database(conn, &now),
        check_embedding(&now),
    ];

    let failed_count = components
        .iter()
        .filter(|c| c.status == HealthStatus::Failed)
        .count();
    let degraded_count = components
        .iter()
        .filter(|c| c.status == HealthStatus::Degraded)
        .count();

    let overall_status = if failed_count >= 2 {
        HealthStatus::Failed
    } else if failed_count >= 1 || degraded_count >= 2 {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    let context_quality = compute_context_quality(&components, conn);
    let fallback_level = compute_fallback_level(&components);

    Ok(SystemHealthReport {
        overall_status,
        context_quality,
        components,
        fallback_level,
        timestamp: now,
    })
}

// ============================================================================
// Individual Component Checks
// ============================================================================

/// Check scanner health: detected_projects count > 0
fn check_scanner(conn: &Connection, now: &str) -> ComponentHealth {
    let result: std::result::Result<i64, _> =
        conn.query_row("SELECT COUNT(*) FROM detected_projects", [], |row| {
            row.get(0)
        });

    match result {
        Ok(count) if count > 0 => {
            debug!(target: "4da::health", count, "Scanner healthy: projects detected");
            ComponentHealth {
                name: "scanner".into(),
                status: HealthStatus::Healthy,
                last_check: now.into(),
                error_message: None,
            }
        }
        Ok(_) => {
            debug!(target: "4da::health", "Scanner degraded: no projects detected yet");
            ComponentHealth {
                name: "scanner".into(),
                status: HealthStatus::Failed,
                last_check: now.into(),
                error_message: Some("No projects detected".into()),
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
fn check_watcher(conn: &Connection, now: &str) -> ComponentHealth {
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
fn check_git(conn: &Connection, now: &str) -> ComponentHealth {
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
fn check_database(conn: &Connection, now: &str) -> ComponentHealth {
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
fn check_embedding(now: &str) -> ComponentHealth {
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
    let provider_has_ollama_fallback =
        matches!(llm.provider.as_str(), "anthropic" | "none" | "local" | "");

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

// ============================================================================
// Source Quality Analysis (ASCENT-PLAN Phase 4.4)
// ============================================================================

/// Per-source relevance quality report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceQualityReport {
    pub source_type: String,
    pub total_items: i64,
    pub relevant_items: i64,
    /// Ratio of relevant items to total (0.0-1.0)
    pub relevance_ratio: f64,
    /// Whether this source is below the 5% threshold
    pub below_threshold: bool,
}

/// Compute per-source relevance ratios from the last N analyses.
///
/// Returns sources that have been fetched, with their relevance ratios.
/// Sources below 5% relevance are flagged for potential replacement.
pub fn compute_source_quality(conn: &Connection, lookback_days: i64) -> Vec<SourceQualityReport> {
    let query = r"
        SELECT
            source_type,
            COUNT(*) as total,
            SUM(CASE WHEN relevance_score >= COALESCE(
                (SELECT CAST(value AS REAL) FROM settings_kv WHERE key = 'relevance_threshold'),
                0.35
            ) THEN 1 ELSE 0 END) as relevant
        FROM source_items
        WHERE fetched_at >= datetime('now', ? || ' days')
        GROUP BY source_type
        HAVING total >= 5
        ORDER BY relevant * 1.0 / total ASC
    ";

    let lookback = format!("-{lookback_days}");
    let mut stmt = match conn.prepare(query) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::health", "Failed to prepare source quality query: {e}");
            return Vec::new();
        }
    };

    let rows = match stmt.query_map([&lookback], |row| {
        let source_type: String = row.get(0)?;
        let total: i64 = row.get(1)?;
        let relevant: i64 = row.get(2)?;
        let ratio = if total > 0 {
            relevant as f64 / total as f64
        } else {
            0.0
        };
        Ok(SourceQualityReport {
            source_type,
            total_items: total,
            relevant_items: relevant,
            relevance_ratio: ratio,
            below_threshold: ratio < 0.05,
        })
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::health", "Failed to query source quality: {e}");
            return Vec::new();
        }
    };

    rows.filter_map(|r| match r {
        Ok(v) => Some(v),
        Err(e) => {
            warn!(target: "4da::health", "Row processing failed in source quality: {e}");
            None
        }
    })
    .collect()
}

// ============================================================================
// Quality & Fallback Computations
// ============================================================================

/// Compute context quality from component statuses.
///
/// Considers both component health and signal richness.
fn compute_context_quality(components: &[ComponentHealth], conn: &Connection) -> ContextQuality {
    let failed_count = components
        .iter()
        .filter(|c| c.status == HealthStatus::Failed)
        .count();
    let degraded_count = components
        .iter()
        .filter(|c| c.status == HealthStatus::Degraded)
        .count();

    if failed_count >= 2 {
        return ContextQuality::Minimal;
    }
    if failed_count >= 1 {
        return ContextQuality::Degraded;
    }
    if degraded_count >= 2 {
        return ContextQuality::Acceptable;
    }
    if degraded_count >= 1 {
        return ContextQuality::Good;
    }

    // All healthy - check signal richness for Excellent vs Good
    let total_signals = count_total_signals(conn);
    if total_signals > 50 {
        ContextQuality::Excellent
    } else {
        ContextQuality::Good
    }
}

/// Count total signals across all signal tables for richness check.
fn count_total_signals(conn: &Connection) -> i64 {
    let file_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM file_signals", [], |row| row.get(0))
        .unwrap_or(0);
    let git_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM git_signals", [], |row| row.get(0))
        .unwrap_or(0);
    file_count + git_count
}

/// Compute fallback level from component statuses.
///
/// - 0: All healthy
/// - 1: 1 component degraded/failed (reduced features)
/// - 2: 2+ components degraded/failed (minimal features)
/// - 3: Database or embedding failed (emergency)
fn compute_fallback_level(components: &[ComponentHealth]) -> u8 {
    let failed_count = components
        .iter()
        .filter(|c| c.status == HealthStatus::Failed)
        .count();
    let degraded_count = components
        .iter()
        .filter(|c| c.status == HealthStatus::Degraded)
        .count();

    // Emergency: database or embedding failed
    let db_failed = components
        .iter()
        .any(|c| c.name == "database" && c.status == HealthStatus::Failed);
    let embed_failed = components
        .iter()
        .any(|c| c.name == "embedding" && c.status == HealthStatus::Failed);

    if db_failed || embed_failed {
        return 3;
    }

    let unhealthy = failed_count + degraded_count;
    if unhealthy >= 2 {
        2
    } else {
        u8::from(unhealthy >= 1)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE detected_projects (id INTEGER PRIMARY KEY, path TEXT, name TEXT, detected_at TEXT);
            CREATE TABLE file_signals (path TEXT, timestamp TEXT, event_type TEXT, extracted_topics TEXT);
            CREATE TABLE git_signals (id INTEGER PRIMARY KEY, repo_path TEXT, timestamp TEXT, signal_type TEXT);
        "#,
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_scanner_healthy() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO detected_projects (path, name, detected_at) VALUES ('/test', 'test', datetime('now'))",
            [],
        )
        .unwrap();
        let result = check_scanner(&conn, "2026-01-01T00:00:00Z");
        assert_eq!(result.status, HealthStatus::Healthy);
        assert!(result.error_message.is_none());
    }

    #[test]
    fn test_scanner_empty() {
        let conn = setup_test_db();
        let result = check_scanner(&conn, "2026-01-01T00:00:00Z");
        assert_eq!(result.status, HealthStatus::Failed);
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_watcher_healthy() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO file_signals (path, timestamp, event_type) VALUES ('/test/file.rs', datetime('now'), 'modify')",
            [],
        )
        .unwrap();
        let result = check_watcher(&conn, "2026-01-01T00:00:00Z");
        assert_eq!(result.status, HealthStatus::Healthy);
    }

    #[test]
    fn test_watcher_stale() {
        let conn = setup_test_db();
        // Insert a signal from 2 hours ago
        conn.execute(
            "INSERT INTO file_signals (path, timestamp, event_type) VALUES ('/test/file.rs', datetime('now', '-2 hours'), 'modify')",
            [],
        )
        .unwrap();
        let result = check_watcher(&conn, "2026-01-01T00:00:00Z");
        assert_eq!(result.status, HealthStatus::Degraded);
    }

    #[test]
    fn test_watcher_empty() {
        let conn = setup_test_db();
        let result = check_watcher(&conn, "2026-01-01T00:00:00Z");
        assert_eq!(result.status, HealthStatus::Failed);
    }

    #[test]
    fn test_git_healthy() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO git_signals (repo_path, timestamp, signal_type) VALUES ('/test', datetime('now'), 'commit')",
            [],
        )
        .unwrap();
        let result = check_git(&conn, "2026-01-01T00:00:00Z");
        assert_eq!(result.status, HealthStatus::Healthy);
    }

    #[test]
    fn test_git_empty() {
        let conn = setup_test_db();
        let result = check_git(&conn, "2026-01-01T00:00:00Z");
        assert_eq!(result.status, HealthStatus::Degraded);
    }

    #[test]
    fn test_database_healthy() {
        let conn = setup_test_db();
        let result = check_database(&conn, "2026-01-01T00:00:00Z");
        assert_eq!(result.status, HealthStatus::Healthy);
    }

    #[test]
    fn test_all_healthy() {
        let conn = setup_test_db();
        // Add data to make scanner, watcher, git healthy
        conn.execute(
            "INSERT INTO detected_projects (path, name, detected_at) VALUES ('/test', 'test', datetime('now'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO file_signals (path, timestamp, event_type) VALUES ('/test/file.rs', datetime('now'), 'modify')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO git_signals (repo_path, timestamp, signal_type) VALUES ('/test', datetime('now'), 'commit')",
            [],
        )
        .unwrap();

        // check_all_components requires crate globals (settings manager) so we test
        // individual components + quality/fallback computations directly
        let components = [
            check_scanner(&conn, "now"),
            check_watcher(&conn, "now"),
            check_git(&conn, "now"),
            check_database(&conn, "now"),
        ];

        let failed = components
            .iter()
            .filter(|c| c.status == HealthStatus::Failed)
            .count();
        assert_eq!(failed, 0, "No component should be failed");
    }

    #[test]
    fn test_empty_db_degraded() {
        let conn = setup_test_db();
        let components = [
            check_scanner(&conn, "now"),
            check_watcher(&conn, "now"),
            check_git(&conn, "now"),
            check_database(&conn, "now"),
        ];

        // Scanner=Failed, Watcher=Failed, Git=Degraded, Database=Healthy
        let failed = components
            .iter()
            .filter(|c| c.status == HealthStatus::Failed)
            .count();
        assert!(
            failed >= 1,
            "Empty DB should have at least 1 failed component"
        );
    }

    #[test]
    fn test_context_quality_all_healthy() {
        let conn = setup_test_db();
        let components = vec![
            ComponentHealth {
                name: "scanner".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
            ComponentHealth {
                name: "watcher".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
            ComponentHealth {
                name: "git".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
            ComponentHealth {
                name: "database".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
            ComponentHealth {
                name: "embedding".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
        ];
        let quality = compute_context_quality(&components, &conn);
        // No signals = Good (need >50 for Excellent)
        assert_eq!(quality, ContextQuality::Good);
    }

    #[test]
    fn test_context_quality_excellent_with_signals() {
        let conn = setup_test_db();
        // Insert >50 signals
        for i in 0..30 {
            conn.execute(
                &format!(
                    "INSERT INTO file_signals (path, timestamp, event_type) VALUES ('/file{i}.rs', datetime('now'), 'modify')"
                ),
                [],
            )
            .unwrap();
        }
        for i in 0..25 {
            conn.execute(
                &format!(
                    "INSERT INTO git_signals (repo_path, timestamp, signal_type) VALUES ('/repo{i}', datetime('now'), 'commit')"
                ),
                [],
            )
            .unwrap();
        }

        let components = vec![
            ComponentHealth {
                name: "scanner".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
            ComponentHealth {
                name: "watcher".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
            ComponentHealth {
                name: "git".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
            ComponentHealth {
                name: "database".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
            ComponentHealth {
                name: "embedding".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
        ];
        let quality = compute_context_quality(&components, &conn);
        assert_eq!(quality, ContextQuality::Excellent);
    }

    #[test]
    fn test_context_quality_degraded() {
        let conn = setup_test_db();
        let components = vec![
            ComponentHealth {
                name: "scanner".into(),
                status: HealthStatus::Failed,
                last_check: String::new(),
                error_message: Some("failed".into()),
            },
            ComponentHealth {
                name: "watcher".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
        ];
        let quality = compute_context_quality(&components, &conn);
        assert_eq!(quality, ContextQuality::Degraded);
    }

    #[test]
    fn test_context_quality_minimal() {
        let conn = setup_test_db();
        let components = vec![
            ComponentHealth {
                name: "scanner".into(),
                status: HealthStatus::Failed,
                last_check: String::new(),
                error_message: Some("failed".into()),
            },
            ComponentHealth {
                name: "watcher".into(),
                status: HealthStatus::Failed,
                last_check: String::new(),
                error_message: Some("failed".into()),
            },
        ];
        let quality = compute_context_quality(&components, &conn);
        assert_eq!(quality, ContextQuality::Minimal);
    }

    #[test]
    fn test_fallback_level_healthy() {
        let components = vec![
            ComponentHealth {
                name: "scanner".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
            ComponentHealth {
                name: "watcher".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
            ComponentHealth {
                name: "database".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
        ];
        let level = compute_fallback_level(&components);
        assert_eq!(level, 0);
    }

    #[test]
    fn test_fallback_level_one_failed() {
        let components = vec![
            ComponentHealth {
                name: "scanner".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
            ComponentHealth {
                name: "watcher".into(),
                status: HealthStatus::Failed,
                last_check: String::new(),
                error_message: Some("not running".into()),
            },
            ComponentHealth {
                name: "database".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
        ];
        let level = compute_fallback_level(&components);
        assert!(
            level >= 1,
            "Should be at least fallback level 1 with a failed component"
        );
    }

    #[test]
    fn test_fallback_level_emergency_db() {
        let components = vec![ComponentHealth {
            name: "database".into(),
            status: HealthStatus::Failed,
            last_check: String::new(),
            error_message: Some("db error".into()),
        }];
        let level = compute_fallback_level(&components);
        assert_eq!(level, 3, "Database failure should be emergency level 3");
    }

    #[test]
    fn test_fallback_level_emergency_embedding() {
        let components = vec![ComponentHealth {
            name: "embedding".into(),
            status: HealthStatus::Failed,
            last_check: String::new(),
            error_message: Some("no key".into()),
        }];
        let level = compute_fallback_level(&components);
        assert_eq!(level, 3, "Embedding failure should be emergency level 3");
    }

    #[test]
    fn test_fallback_level_two_degraded() {
        let components = vec![
            ComponentHealth {
                name: "scanner".into(),
                status: HealthStatus::Degraded,
                last_check: String::new(),
                error_message: None,
            },
            ComponentHealth {
                name: "watcher".into(),
                status: HealthStatus::Degraded,
                last_check: String::new(),
                error_message: None,
            },
            ComponentHealth {
                name: "database".into(),
                status: HealthStatus::Healthy,
                last_check: String::new(),
                error_message: None,
            },
        ];
        let level = compute_fallback_level(&components);
        assert_eq!(level, 2, "2 degraded components should be fallback level 2");
    }

    #[test]
    fn test_count_total_signals() {
        let conn = setup_test_db();
        assert_eq!(count_total_signals(&conn), 0);

        conn.execute(
            "INSERT INTO file_signals (path, timestamp, event_type) VALUES ('/f.rs', datetime('now'), 'modify')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO git_signals (repo_path, timestamp, signal_type) VALUES ('/r', datetime('now'), 'commit')",
            [],
        )
        .unwrap();
        assert_eq!(count_total_signals(&conn), 2);
    }
}
