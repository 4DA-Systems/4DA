//! Temporal Event Store for 4DA Innovation Features
//!
//! Provides recording and querying of temporal events, project dependencies,
//! and cross-item relationships used by predictive context, semantic diff,
//! signal chains, knowledge decay, and attention tracking.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::{Result, ResultExt};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalEvent {
    pub id: i64,
    pub event_type: String,
    pub subject: String,
    pub data: serde_json::Value,
    pub source_item_id: Option<i64>,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDependency {
    pub id: i64,
    pub project_path: String,
    pub manifest_type: String,
    pub package_name: String,
    pub version: Option<String>,
    pub is_dev: bool,
    /// Whether this is a direct dependency (listed in manifest) vs transitive
    /// (pulled in via lockfile). Direct deps default to true for backwards
    /// compatibility — existing rows without the column get is_direct=1.
    pub is_direct: bool,
    pub language: String,
    pub last_scanned: String,
}

// ============================================================================
// Project Dependencies
// ============================================================================

/// Upsert a project dependency.
///
/// `is_direct` indicates whether this dependency is declared directly in a
/// manifest file (`true`) or is a transitive dependency discovered from a
/// lockfile (`false`). On conflict the `is_direct` value is only *upgraded*
/// (transitive -> direct) but never downgraded, so a lockfile upsert cannot
/// demote a previously-seen direct dep.
///
/// `project_relevance` is a 0.0..1.0 score from ACE path/git analysis.
/// Example/demo/test directories get low scores (0.1x). The column defaults
/// to 1.0 in the schema, so existing rows and callers passing 1.0 are unaffected.
pub fn upsert_dependency(
    conn: &rusqlite::Connection,
    project_path: &str,
    manifest_type: &str,
    package_name: &str,
    version: Option<&str>,
    is_dev: bool,
    is_direct: bool,
    language: &str,
    project_relevance: f32,
) -> Result<()> {
    conn.execute(
        "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, is_direct, language, project_relevance, last_scanned)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))
         ON CONFLICT(project_path, package_name)
         DO UPDATE SET version = ?4, is_dev = ?5, is_direct = MAX(project_dependencies.is_direct, ?6), project_relevance = ?8, last_scanned = datetime('now')",
        params![project_path, manifest_type, package_name, version, is_dev as i32, is_direct as i32, language, project_relevance],
    )
    .context("Failed to upsert dependency")?;
    Ok(())
}

/// Get all dependencies for a project
pub fn get_project_dependencies(
    conn: &rusqlite::Connection,
    project_path: &str,
) -> Result<Vec<ProjectDependency>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, project_path, manifest_type, package_name, version, is_dev, is_direct, language, last_scanned
             FROM project_dependencies
             WHERE project_path = ?1
             ORDER BY package_name",
        )
        ?;

    let results: Vec<ProjectDependency> = stmt
        .query_map(params![project_path], map_project_dependency_row)?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in temporal: {e}");
                None
            }
        })
        .collect();

    Ok(results)
}

/// Map a row from the project_dependencies table to a `ProjectDependency`.
/// Column order must be: id, project_path, manifest_type, package_name,
///                        version, is_dev, is_direct, language, last_scanned.
fn map_project_dependency_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectDependency> {
    Ok(ProjectDependency {
        id: row.get(0)?,
        project_path: row.get(1)?,
        manifest_type: row.get(2)?,
        package_name: row.get(3)?,
        version: row.get(4)?,
        is_dev: row.get::<_, i32>(5)? != 0,
        is_direct: row.get::<_, i32>(6).unwrap_or(1) != 0,
        language: row.get(7)?,
        last_scanned: row.get(8)?,
    })
}

/// Get all tracked dependencies, scoped to projects with recent git activity.
/// Only includes deps from project trees that have commits in the last 60 days.
/// Falls back to all deps if no git signals exist (first run).
pub fn get_all_dependencies(conn: &rusqlite::Connection) -> Result<Vec<ProjectDependency>> {
    // Get active repo roots from git_signals (repos with recent commits)
    let active_roots: Vec<String> = conn
        .prepare(
            "SELECT DISTINCT repo_path FROM git_signals
             WHERE commit_hash IS NOT NULL AND commit_hash != ''
             AND timestamp > datetime('now', '-60 days')",
        )
        .and_then(|mut stmt| {
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            Ok(rows
                .filter_map(|r| match r {
                    Ok(v) => Some(v),
                    Err(e) => {
                        tracing::warn!("Row processing failed in temporal: {e}");
                        None
                    }
                })
                .collect())
        })
        .unwrap_or_default();

    let mut stmt = conn
        .prepare(
            "SELECT id, project_path, manifest_type, package_name, version, is_dev, is_direct, language, last_scanned
             FROM project_dependencies
             ORDER BY project_path, package_name",
        )
        ?;

    let all_deps: Vec<ProjectDependency> = stmt
        .query_map([], map_project_dependency_row)?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in temporal: {e}");
                None
            }
        })
        .collect();

    // Filter to deps from active project trees only
    if active_roots.is_empty() {
        // No git signals yet — return all deps (first run fallback)
        return Ok(all_deps);
    }

    let filtered: Vec<ProjectDependency> = all_deps
        .into_iter()
        .filter(|dep| {
            // Normalize for case-insensitive comparison on Windows
            let dep_path = dep.project_path.to_lowercase();
            active_roots.iter().any(|root| {
                let root_lower = root.to_lowercase();
                dep_path.starts_with(&root_lower) || root_lower.starts_with(&dep_path)
            })
        })
        .collect();

    // If filtering eliminated everything, fall back to all deps
    if filtered.is_empty() {
        return Ok(conn
            .prepare(
                "SELECT id, project_path, manifest_type, package_name, version, is_dev, is_direct, language, last_scanned
                 FROM project_dependencies ORDER BY project_path, package_name",
            )
            ?
            .query_map([], map_project_dependency_row)
            ?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in temporal: {e}");
                    None
                }
            })
            .collect());
    }

    Ok(filtered)
}

// ============================================================================
// Temporal Events
// ============================================================================

/// Record a new temporal event
pub fn record_event(
    conn: &rusqlite::Connection,
    event_type: &str,
    subject: &str,
    data: &serde_json::Value,
    source_item_id: Option<i64>,
    expires_at: Option<&str>,
) -> Result<i64> {
    let data_str = serde_json::to_string(data)?;
    conn.execute(
        "INSERT INTO temporal_events (event_type, subject, data, source_item_id, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![event_type, subject, data_str, source_item_id, expires_at],
    )
    .context("Failed to record temporal event")?;
    Ok(conn.last_insert_rowid())
}

/// Query temporal events by type and optional time range
pub fn query_events(
    conn: &rusqlite::Connection,
    event_type: &str,
    since: Option<&str>,
    limit: usize,
) -> Result<Vec<TemporalEvent>> {
    let query = if since.is_some() {
        "SELECT id, event_type, subject, data, source_item_id, created_at, expires_at
         FROM temporal_events
         WHERE event_type = ?1 AND created_at >= ?2
         ORDER BY created_at DESC LIMIT ?3"
    } else {
        "SELECT id, event_type, subject, data, source_item_id, created_at, expires_at
         FROM temporal_events
         WHERE event_type = ?1
         ORDER BY created_at DESC LIMIT ?2"
    };

    let mut stmt = conn.prepare(query)?;

    let results: Vec<TemporalEvent> = if let Some(since_val) = since {
        stmt.query_map(params![event_type, since_val, limit as i64], map_event)
    } else {
        stmt.query_map(params![event_type, limit as i64], map_event)
    }?
    .filter_map(|r| match r {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("Row processing failed in temporal: {e}");
            None
        }
    })
    .collect();

    Ok(results)
}

/// Query temporal events by subject
#[allow(dead_code)] // Reason: called by semantic_diff, reserved for MCP integration
pub fn query_events_by_subject(
    conn: &rusqlite::Connection,
    subject: &str,
    limit: usize,
) -> Result<Vec<TemporalEvent>> {
    let mut stmt = conn.prepare(
        "SELECT id, event_type, subject, data, source_item_id, created_at, expires_at
             FROM temporal_events
             WHERE subject = ?1
             ORDER BY created_at DESC LIMIT ?2",
    )?;

    let results: Vec<TemporalEvent> = stmt
        .query_map(params![subject, limit as i64], map_event)?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in temporal: {e}");
                None
            }
        })
        .collect();

    Ok(results)
}

/// Clean up expired temporal events
#[allow(dead_code)] // Reason: called by cleanup_temporal_events, reserved for scheduled maintenance
pub fn cleanup_expired(conn: &rusqlite::Connection) -> Result<usize> {
    let deleted = conn.execute(
        "DELETE FROM temporal_events WHERE expires_at IS NOT NULL AND expires_at < datetime('now')",
        [],
    )?;
    if deleted > 0 {
        debug!(target: "4da::temporal", deleted, "Cleaned up expired temporal events");
    }
    Ok(deleted)
}

fn map_event(row: &rusqlite::Row) -> rusqlite::Result<TemporalEvent> {
    let data_str: String = row.get(3)?;
    let data = serde_json::from_str(&data_str).unwrap_or(serde_json::Value::Null);
    Ok(TemporalEvent {
        id: row.get(0)?,
        event_type: row.get(1)?,
        subject: row.get(2)?,
        data,
        source_item_id: row.get(4)?,
        created_at: row.get(5)?,
        expires_at: row.get(6)?,
    })
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[allow(dead_code)] // Reason: reserved for MCP integration
pub fn get_temporal_events(
    event_type: String,
    since: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<TemporalEvent>> {
    let conn = crate::open_db_connection()?;
    query_events(&conn, &event_type, since.as_deref(), limit.unwrap_or(50))
}

#[allow(dead_code)] // Reason: reserved for MCP integration
pub fn get_temporal_event_count(event_type: String) -> Result<usize> {
    let conn = crate::open_db_connection()?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM temporal_events WHERE event_type = ?1",
        params![event_type],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

#[allow(dead_code)] // Reason: reserved for MCP integration
pub fn get_dependencies(project_path: Option<String>) -> Result<Vec<ProjectDependency>> {
    let conn = crate::open_db_connection()?;
    if let Some(path) = project_path {
        get_project_dependencies(&conn, &path)
    } else {
        get_all_dependencies(&conn)
    }
}

#[allow(dead_code)] // Reason: reserved for scheduled maintenance task
pub fn cleanup_temporal_events() -> Result<usize> {
    let conn = crate::open_db_connection()?;
    cleanup_expired(&conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS temporal_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                data JSON NOT NULL,
                embedding BLOB,
                source_item_id INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT
            );",
        )
        .expect("create tables");
        conn
    }

    #[test]
    fn record_and_query_event_roundtrip() {
        let conn = setup_test_db();
        let data = serde_json::json!({"key": "value", "count": 42});
        let id = record_event(&conn, "test_type", "test_subject", &data, Some(100), None).unwrap();
        assert!(id > 0);

        let events = query_events(&conn, "test_type", None, 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, id);
        assert_eq!(events[0].event_type, "test_type");
        assert_eq!(events[0].subject, "test_subject");
        assert_eq!(events[0].source_item_id, Some(100));
        assert_eq!(events[0].data["key"], "value");
        assert_eq!(events[0].data["count"], 42);
    }

    #[test]
    fn query_events_respects_limit() {
        let conn = setup_test_db();
        let data = serde_json::json!({});
        for i in 0..5 {
            record_event(&conn, "bulk", &format!("subj_{}", i), &data, None, None).unwrap();
        }
        let events = query_events(&conn, "bulk", None, 3).unwrap();
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn query_events_filters_by_type() {
        let conn = setup_test_db();
        let data = serde_json::json!({});
        record_event(&conn, "type_a", "s1", &data, None, None).unwrap();
        record_event(&conn, "type_b", "s2", &data, None, None).unwrap();
        record_event(&conn, "type_a", "s3", &data, None, None).unwrap();

        let a_events = query_events(&conn, "type_a", None, 10).unwrap();
        assert_eq!(a_events.len(), 2);
        let b_events = query_events(&conn, "type_b", None, 10).unwrap();
        assert_eq!(b_events.len(), 1);
    }

    #[test]
    fn query_events_by_subject_returns_matching() {
        let conn = setup_test_db();
        let data = serde_json::json!({"info": "test"});
        record_event(&conn, "ev1", "rust", &data, None, None).unwrap();
        record_event(&conn, "ev2", "rust", &data, None, None).unwrap();
        record_event(&conn, "ev3", "python", &data, None, None).unwrap();

        let rust_events = query_events_by_subject(&conn, "rust", 10).unwrap();
        assert_eq!(rust_events.len(), 2);
        for ev in &rust_events {
            assert_eq!(ev.subject, "rust");
        }

        let python_events = query_events_by_subject(&conn, "python", 10).unwrap();
        assert_eq!(python_events.len(), 1);
    }

    #[test]
    fn cleanup_expired_removes_old_events() {
        let conn = setup_test_db();
        let data = serde_json::json!({});
        // Insert an event that expired yesterday
        conn.execute(
            "INSERT INTO temporal_events (event_type, subject, data, expires_at)
             VALUES ('exp', 'old', '{}', datetime('now', '-1 day'))",
            [],
        )
        .unwrap();
        // Insert an event that expires tomorrow
        conn.execute(
            "INSERT INTO temporal_events (event_type, subject, data, expires_at)
             VALUES ('exp', 'future', '{}', datetime('now', '+1 day'))",
            [],
        )
        .unwrap();
        // Insert an event with no expiry
        record_event(&conn, "exp", "permanent", &data, None, None).unwrap();

        let deleted = cleanup_expired(&conn).unwrap();
        assert_eq!(deleted, 1);

        let remaining = query_events(&conn, "exp", None, 10).unwrap();
        assert_eq!(remaining.len(), 2);
    }

    #[test]
    fn temporal_event_serde_roundtrip() {
        let event = TemporalEvent {
            id: 1,
            event_type: "version_release".to_string(),
            subject: "react".to_string(),
            data: serde_json::json!({"version": "19.0.0", "breaking": true}),
            source_item_id: Some(42),
            created_at: "2026-02-28T10:00:00".to_string(),
            expires_at: Some("2026-03-28T10:00:00".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: TemporalEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type, "version_release");
        assert_eq!(deserialized.data["version"], "19.0.0");
        assert_eq!(deserialized.source_item_id, Some(42));
        assert!(deserialized.expires_at.is_some());
    }

    #[test]
    fn record_event_with_null_source_item_id() {
        let conn = setup_test_db();
        let data = serde_json::json!({"note": "no source"});
        let id = record_event(&conn, "manual", "user_action", &data, None, None).unwrap();
        let events = query_events(&conn, "manual", None, 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, id);
        assert_eq!(events[0].source_item_id, None);
    }
}
