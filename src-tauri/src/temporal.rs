//! Temporal Event Store for 4DA Innovation Features
//!
//! Provides recording and querying of temporal events, project dependencies,
//! and cross-item relationships used by predictive context, semantic diff,
//! signal chains, knowledge decay, and attention tracking.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::debug;

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
    pub language: String,
    pub last_scanned: String,
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
) -> Result<i64, String> {
    let data_str = serde_json::to_string(data).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO temporal_events (event_type, subject, data, source_item_id, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![event_type, subject, data_str, source_item_id, expires_at],
    )
    .map_err(|e| format!("Failed to record temporal event: {}", e))?;
    Ok(conn.last_insert_rowid())
}

/// Query temporal events by type and optional time range
pub fn query_events(
    conn: &rusqlite::Connection,
    event_type: &str,
    since: Option<&str>,
    limit: usize,
) -> Result<Vec<TemporalEvent>, String> {
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

    let mut stmt = conn.prepare(query).map_err(|e| e.to_string())?;

    let results: Vec<TemporalEvent> = if let Some(since_val) = since {
        stmt.query_map(params![event_type, since_val, limit as i64], map_event)
    } else {
        stmt.query_map(params![event_type, limit as i64], map_event)
    }
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

    Ok(results)
}

/// Query temporal events by subject
#[allow(dead_code)] // Used by semantic_diff (reserved for MCP integration)
pub fn query_events_by_subject(
    conn: &rusqlite::Connection,
    subject: &str,
    limit: usize,
) -> Result<Vec<TemporalEvent>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, event_type, subject, data, source_item_id, created_at, expires_at
             FROM temporal_events
             WHERE subject = ?1
             ORDER BY created_at DESC LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;

    let results: Vec<TemporalEvent> = stmt
        .query_map(params![subject, limit as i64], map_event)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

/// Clean up expired temporal events
#[allow(dead_code)] // Used by cleanup_temporal_events (reserved for scheduled maintenance)
pub fn cleanup_expired(conn: &rusqlite::Connection) -> Result<usize, String> {
    let deleted = conn
        .execute(
            "DELETE FROM temporal_events WHERE expires_at IS NOT NULL AND expires_at < datetime('now')",
            [],
        )
        .map_err(|e| e.to_string())?;
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

#[allow(dead_code)] // Reserved for MCP integration
pub fn get_temporal_events(
    event_type: String,
    since: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<TemporalEvent>, String> {
    let conn = crate::open_db_connection()?;
    query_events(&conn, &event_type, since.as_deref(), limit.unwrap_or(50))
}

#[allow(dead_code)] // Reserved for MCP integration
pub fn get_temporal_event_count(event_type: String) -> Result<usize, String> {
    let conn = crate::open_db_connection()?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM temporal_events WHERE event_type = ?1",
            params![event_type],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count as usize)
}

#[allow(dead_code)] // Reserved for MCP integration
pub fn get_dependencies(project_path: Option<String>) -> Result<Vec<ProjectDependency>, String> {
    let conn = crate::open_db_connection()?;
    if let Some(path) = project_path {
        crate::temporal_dependencies::get_project_dependencies(&conn, &path)
    } else {
        crate::temporal_dependencies::get_all_dependencies(&conn)
    }
}

#[allow(dead_code)] // Reserved for scheduled maintenance task
pub fn cleanup_temporal_events() -> Result<usize, String> {
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
