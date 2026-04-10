//! Team notifications — in-app notification system for team events.
//!
//! Notifications are stored locally and never synced via the relay.
//! Other modules call `create_notification()` to generate notifications;
//! the frontend reads them via Tauri commands.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::info;
use ts_rs::TS;

// ============================================================================
// Schema
// ============================================================================

/// Ensure the team_notifications table and indices exist.
pub fn ensure_tables(conn: &rusqlite::Connection) -> crate::error::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS team_notifications (
            id TEXT PRIMARY KEY,
            team_id TEXT NOT NULL,
            notification_type TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT,
            severity TEXT DEFAULT 'info',
            read INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            metadata TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_team_notif_team
            ON team_notifications(team_id);
        CREATE INDEX IF NOT EXISTS idx_team_notif_read
            ON team_notifications(team_id, read);",
    )?;
    Ok(())
}

// ============================================================================
// Types
// ============================================================================

/// A single team notification.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TeamNotification {
    pub id: String,
    pub team_id: String,
    pub notification_type: String,
    pub title: String,
    pub body: Option<String>,
    pub severity: String,
    pub read: bool,
    pub created_at: String,
    #[ts(type = "any")]
    pub metadata: Option<serde_json::Value>,
}

/// Summary of unread notifications.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NotificationSummary {
    pub total_unread: u32,
    pub by_type: Vec<NotificationTypeCount>,
}

/// Count of notifications grouped by type.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NotificationTypeCount {
    pub notification_type: String,
    pub count: u32,
}

// ============================================================================
// Internal API (called by other modules)
// ============================================================================

/// Create a new notification and return its ID.
///
/// Called by other team modules (signal detection, decision proposals, etc.)
/// to generate in-app notifications for team events.
pub fn create_notification(
    conn: &rusqlite::Connection,
    team_id: &str,
    notification_type: &str,
    title: &str,
    body: Option<&str>,
    severity: &str,
    metadata: Option<&serde_json::Value>,
) -> crate::error::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let metadata_json = metadata.map(|m| serde_json::to_string(m)).transpose()?;

    conn.execute(
        "INSERT INTO team_notifications
            (id, team_id, notification_type, title, body, severity, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            team_id,
            notification_type,
            title,
            body,
            severity,
            metadata_json
        ],
    )?;

    info!(target: "4da::team_notifications",
        notification_id = %id,
        notification_type = %notification_type,
        severity = %severity,
        "Team notification created");

    Ok(id)
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get notifications for the current team, ordered by newest first.
#[tauri::command]
pub async fn get_team_notifications(
    limit: Option<u32>,
    unread_only: Option<bool>,
) -> crate::error::Result<Vec<TeamNotification>> {
    let team_id = get_team_id()?;
    let conn = crate::state::open_db_connection()?;
    ensure_tables(&conn)?;

    let effective_limit = limit.unwrap_or(50);
    let only_unread = unread_only.unwrap_or(false);

    let sql = if only_unread {
        "SELECT id, team_id, notification_type, title, body, severity, read, created_at, metadata
         FROM team_notifications
         WHERE team_id = ?1 AND read = 0
         ORDER BY created_at DESC
         LIMIT ?2"
    } else {
        "SELECT id, team_id, notification_type, title, body, severity, read, created_at, metadata
         FROM team_notifications
         WHERE team_id = ?1
         ORDER BY created_at DESC
         LIMIT ?2"
    };

    let mut stmt = conn.prepare(sql)?;
    let notifications = stmt
        .query_map(params![team_id, effective_limit], |row| {
            let metadata_str: Option<String> = row.get(8)?;
            let metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());
            Ok(TeamNotification {
                id: row.get(0)?,
                team_id: row.get(1)?,
                notification_type: row.get(2)?,
                title: row.get(3)?,
                body: row.get(4)?,
                severity: row.get(5)?,
                read: row.get::<_, i32>(6)? != 0,
                created_at: row.get(7)?,
                metadata,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(notifications)
}

/// Get a summary of unread notifications grouped by type.
#[tauri::command]
pub async fn get_notification_summary() -> crate::error::Result<NotificationSummary> {
    let team_id = get_team_id()?;
    let conn = crate::state::open_db_connection()?;
    ensure_tables(&conn)?;

    let total_unread: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM team_notifications WHERE team_id = ?1 AND read = 0",
            params![team_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT notification_type, COUNT(*) as cnt
         FROM team_notifications
         WHERE team_id = ?1 AND read = 0
         GROUP BY notification_type
         ORDER BY cnt DESC",
    )?;

    let by_type = stmt
        .query_map(params![team_id], |row| {
            Ok(NotificationTypeCount {
                notification_type: row.get(0)?,
                count: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(NotificationSummary {
        total_unread,
        by_type,
    })
}

/// Mark a single notification as read.
#[tauri::command]
pub async fn mark_notification_read(notification_id: String) -> crate::error::Result<()> {
    let team_id = get_team_id()?;
    let conn = crate::state::open_db_connection()?;
    ensure_tables(&conn)?;

    let updated = conn.execute(
        "UPDATE team_notifications SET read = 1 WHERE id = ?1 AND team_id = ?2",
        params![notification_id, team_id],
    )?;

    if updated == 0 {
        return Err("Notification not found".into());
    }

    info!(target: "4da::team_notifications",
        notification_id = %notification_id,
        "Notification marked as read");
    Ok(())
}

/// Mark all unread notifications as read for the current team.
#[tauri::command]
pub async fn mark_all_notifications_read() -> crate::error::Result<()> {
    let team_id = get_team_id()?;
    let conn = crate::state::open_db_connection()?;
    ensure_tables(&conn)?;

    conn.execute(
        "UPDATE team_notifications SET read = 1 WHERE team_id = ?1 AND read = 0",
        params![team_id],
    )?;

    info!(target: "4da::team_notifications",
        team_id = %team_id,
        "All notifications marked as read");
    Ok(())
}

/// Delete a single notification.
#[tauri::command]
pub async fn dismiss_notification(notification_id: String) -> crate::error::Result<()> {
    let team_id = get_team_id()?;
    let conn = crate::state::open_db_connection()?;
    ensure_tables(&conn)?;

    let deleted = conn.execute(
        "DELETE FROM team_notifications WHERE id = ?1 AND team_id = ?2",
        params![notification_id, team_id],
    )?;

    if deleted == 0 {
        return Err("Notification not found".into());
    }

    info!(target: "4da::team_notifications",
        notification_id = %notification_id,
        "Notification dismissed");
    Ok(())
}

// ============================================================================
// Helpers
// ============================================================================

fn get_team_id() -> crate::error::Result<String> {
    let settings = crate::state::get_settings_manager().lock();
    settings
        .get()
        .team_relay
        .as_ref()
        .and_then(|c| c.team_id.clone())
        .ok_or_else(|| "No team configured".into())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        ensure_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn test_create_notification() {
        let conn = setup_db();
        let id = create_notification(
            &conn,
            "team-1",
            "signal_detected",
            "New signal: Rust 2026 edition",
            Some("A significant signal was detected by 3 members."),
            "info",
            None,
        )
        .unwrap();
        assert!(!id.is_empty());

        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM team_notifications WHERE team_id = 'team-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_create_notification_with_metadata() {
        let conn = setup_db();
        let meta = serde_json::json!({"signal_id": "sig-123", "score": 0.95});
        let id = create_notification(
            &conn,
            "team-1",
            "signal_detected",
            "Critical CVE detected",
            None,
            "critical",
            Some(&meta),
        )
        .unwrap();

        let stored: String = conn
            .query_row(
                "SELECT metadata FROM team_notifications WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&stored).unwrap();
        assert_eq!(parsed["signal_id"], "sig-123");
    }

    #[test]
    fn test_notification_defaults() {
        let conn = setup_db();
        let id = create_notification(
            &conn,
            "team-1",
            "member_joined",
            "Alice joined the team",
            None,
            "info",
            None,
        )
        .unwrap();

        let (read, severity): (i32, String) = conn
            .query_row(
                "SELECT read, severity FROM team_notifications WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(read, 0);
        assert_eq!(severity, "info");
    }

    #[test]
    fn test_mark_read() {
        let conn = setup_db();
        let id = create_notification(
            &conn,
            "team-1",
            "decision_proposed",
            "Migrate to Axum?",
            None,
            "info",
            None,
        )
        .unwrap();

        conn.execute(
            "UPDATE team_notifications SET read = 1 WHERE id = ?1 AND team_id = ?2",
            params![id, "team-1"],
        )
        .unwrap();

        let read: i32 = conn
            .query_row(
                "SELECT read FROM team_notifications WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(read, 1);
    }

    #[test]
    fn test_mark_all_read() {
        let conn = setup_db();
        for i in 0..5 {
            create_notification(
                &conn,
                "team-1",
                "signal_detected",
                &format!("Signal {i}"),
                None,
                "info",
                None,
            )
            .unwrap();
        }

        conn.execute(
            "UPDATE team_notifications SET read = 1 WHERE team_id = ?1 AND read = 0",
            params!["team-1"],
        )
        .unwrap();

        let unread: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM team_notifications WHERE team_id = 'team-1' AND read = 0",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unread, 0);
    }

    #[test]
    fn test_dismiss_notification() {
        let conn = setup_db();
        let id = create_notification(
            &conn,
            "team-1",
            "member_left",
            "Bob left the team",
            None,
            "warning",
            None,
        )
        .unwrap();

        conn.execute(
            "DELETE FROM team_notifications WHERE id = ?1 AND team_id = ?2",
            params![id, "team-1"],
        )
        .unwrap();

        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM team_notifications WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_notification_summary_query() {
        let conn = setup_db();
        create_notification(&conn, "team-1", "signal_detected", "S1", None, "info", None).unwrap();
        create_notification(&conn, "team-1", "signal_detected", "S2", None, "info", None).unwrap();
        create_notification(&conn, "team-1", "member_joined", "M1", None, "info", None).unwrap();

        let total: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM team_notifications WHERE team_id = 'team-1' AND read = 0",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(total, 3);

        let mut stmt = conn
            .prepare(
                "SELECT notification_type, COUNT(*) as cnt
                 FROM team_notifications
                 WHERE team_id = 'team-1' AND read = 0
                 GROUP BY notification_type
                 ORDER BY cnt DESC",
            )
            .unwrap();
        let types: Vec<(String, u32)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(types.len(), 2);
        assert_eq!(types[0].0, "signal_detected");
        assert_eq!(types[0].1, 2);
    }

    #[test]
    fn test_ensure_tables_idempotent() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        ensure_tables(&conn).unwrap();
        ensure_tables(&conn).unwrap(); // Should not error on second call
    }
}
