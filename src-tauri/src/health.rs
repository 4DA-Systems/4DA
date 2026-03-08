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
/// EMBEDDING_CLIENT is a Lazy<reqwest::Client> so it's always available.
/// The real question is whether API keys are configured for embedding.
fn check_embedding(now: &str) -> ComponentHealth {
    // EMBEDDING_CLIENT (Lazy<reqwest::Client>) is always available once accessed.
    // Check if settings have an embedding-capable API key configured.
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

    if has_openai_key {
        ComponentHealth {
            name: "embedding".into(),
            status: HealthStatus::Healthy,
            last_check: now.into(),
            error_message: None,
        }
    } else if has_ollama {
        // Ollama can do embeddings but we can't verify connectivity here synchronously
        ComponentHealth {
            name: "embedding".into(),
            status: HealthStatus::Healthy,
            last_check: now.into(),
            error_message: None,
        }
    } else {
        ComponentHealth {
            name: "embedding".into(),
            status: HealthStatus::Degraded,
            last_check: now.into(),
            error_message: Some("No embedding API key configured".into()),
        }
    }
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
    } else if unhealthy >= 1 {
        1
    } else {
        0
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
