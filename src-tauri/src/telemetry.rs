//! Local Telemetry — privacy-first usage analytics that never leave the machine.
//!
//! All data stays in local SQLite. No external services. No aggregation.
//! Users can view their own usage patterns and delete at any time.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use tracing::info;

use crate::error::{FourDaError, Result};

// Session ID: generated once per app process lifetime from timestamp + PID
static SESSION_ID: LazyLock<String> = LazyLock::new(|| {
    use std::time::SystemTime;
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{}-{}", ts, std::process::id())
});

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    pub period_days: u32,
    pub total_events: u64,
    pub sessions: u64,
    pub view_usage: Vec<ViewUsage>,
    pub search_count: u64,
    pub synthesis_count: u64,
    pub ghost_preview_impressions: u64,
    pub ghost_preview_clicks: u64,
    pub ghost_click_rate: f64,
    pub avg_session_views: f64,
    pub most_active_day: Option<String>,
    pub feature_adoption: FeatureAdoption,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewUsage {
    pub view_id: String,
    pub count: u64,
    pub total_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureAdoption {
    pub has_configured_ollama: bool,
    pub has_searched: bool,
    pub has_used_synthesis: bool,
    pub has_given_feedback: bool,
    pub has_configured_sources: bool,
    pub has_created_watch: bool,
}

// ============================================================================
// Schema
// ============================================================================

/// Creates the user_events table and indexes if they don't exist.
/// Production uses db/migrations.rs (Phase 25). This is for test DBs.
#[cfg(test)]
fn ensure_telemetry_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS user_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL,
            view_id TEXT,
            metadata TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            session_id TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_user_events_type ON user_events(event_type);
        CREATE INDEX IF NOT EXISTS idx_user_events_created ON user_events(created_at);",
    )
    .map_err(FourDaError::Db)?;
    Ok(())
}

// ============================================================================
// Core Functions
// ============================================================================

/// Insert a telemetry event.
pub fn record_event(
    conn: &Connection,
    event_type: &str,
    view_id: Option<&str>,
    metadata: Option<&str>,
    session_id: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO user_events (event_type, view_id, metadata, session_id)
         VALUES (?1, ?2, ?3, ?4)",
        params![event_type, view_id, metadata, session_id],
    )
    .map_err(FourDaError::Db)?;
    Ok(())
}

/// Aggregate usage data for the given number of days.
pub fn get_usage_report(conn: &Connection, days: u32) -> Result<UsageReport> {
    let cutoff = format!("-{} days", days);

    let total_events: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM user_events WHERE created_at >= datetime('now', ?1)",
            params![cutoff],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let sessions: u64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT session_id) FROM user_events
             WHERE session_id IS NOT NULL AND created_at >= datetime('now', ?1)",
            params![cutoff],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // View usage sorted by count desc
    let view_usage = {
        let mut stmt = conn
            .prepare(
                "SELECT view_id, COUNT(*) as cnt
                 FROM user_events
                 WHERE view_id IS NOT NULL AND created_at >= datetime('now', ?1)
                 GROUP BY view_id ORDER BY cnt DESC",
            )
            .map_err(FourDaError::Db)?;
        let rows = stmt
            .query_map(params![cutoff], |row| {
                Ok(ViewUsage {
                    view_id: row.get(0)?,
                    count: row.get(1)?,
                    total_seconds: 0, // Duration tracking is a future enhancement
                })
            })
            .map_err(FourDaError::Db)?
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();
        rows
    };

    let search_count: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM user_events
             WHERE event_type = 'search_query' AND created_at >= datetime('now', ?1)",
            params![cutoff],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let synthesis_count: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM user_events
             WHERE event_type = 'synthesis_triggered' AND created_at >= datetime('now', ?1)",
            params![cutoff],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let ghost_preview_impressions: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM user_events
             WHERE event_type = 'ghost_preview_shown' AND created_at >= datetime('now', ?1)",
            params![cutoff],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let ghost_preview_clicks: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM user_events
             WHERE event_type = 'ghost_preview_clicked' AND created_at >= datetime('now', ?1)",
            params![cutoff],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let ghost_click_rate = if ghost_preview_impressions > 0 {
        ghost_preview_clicks as f64 / ghost_preview_impressions as f64
    } else {
        0.0
    };

    // Average views per session
    let avg_session_views = if sessions > 0 {
        let total_view_events: u64 = conn
            .query_row(
                "SELECT COUNT(*) FROM user_events
                 WHERE event_type LIKE 'view_open:%' AND created_at >= datetime('now', ?1)",
                params![cutoff],
                |row| row.get(0),
            )
            .unwrap_or(0);
        total_view_events as f64 / sessions as f64
    } else {
        0.0
    };

    // Most active day
    let most_active_day: Option<String> = conn
        .query_row(
            "SELECT date(created_at) as d FROM user_events
             WHERE created_at >= datetime('now', ?1)
             GROUP BY d ORDER BY COUNT(*) DESC LIMIT 1",
            params![cutoff],
            |row| row.get(0),
        )
        .ok();

    // Feature adoption (lifetime, not period-limited)
    let has = |event_type: &str| -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM user_events WHERE event_type = ?1",
            params![event_type],
            |row| row.get::<_, i64>(0).map(|c| c > 0),
        )
        .unwrap_or(false)
    };

    let feature_adoption = FeatureAdoption {
        has_configured_ollama: has("ollama_configured"),
        has_searched: has("search_query"),
        has_used_synthesis: has("synthesis_triggered"),
        has_given_feedback: has("feedback_given"),
        has_configured_sources: has("sources_configured"),
        has_created_watch: has("watch_created"),
    };

    Ok(UsageReport {
        period_days: days,
        total_events,
        sessions,
        view_usage,
        search_count,
        synthesis_count,
        ghost_preview_impressions,
        ghost_preview_clicks,
        ghost_click_rate,
        avg_session_views,
        most_active_day,
        feature_adoption,
    })
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn track_event(
    event_type: String,
    view_id: Option<String>,
    metadata: Option<String>,
) -> Result<()> {
    let conn = crate::open_db_connection()?;
    let session_id = SESSION_ID.as_str();
    record_event(
        &conn,
        &event_type,
        view_id.as_deref(),
        metadata.as_deref(),
        Some(session_id),
    )
}

#[tauri::command]
pub async fn get_usage_analytics(days: Option<u32>) -> Result<UsageReport> {
    let conn = crate::open_db_connection()?;
    get_usage_report(&conn, days.unwrap_or(30))
}

#[tauri::command]
pub async fn clear_telemetry() -> Result<()> {
    let conn = crate::open_db_connection()?;
    conn.execute("DELETE FROM user_events", [])
        .map_err(FourDaError::Db)?;
    info!(target: "4da::telemetry", "All telemetry data cleared by user");
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_telemetry_table(&conn).unwrap();
        conn
    }

    #[test]
    fn test_ensure_table_idempotent() {
        let conn = setup_test_db();
        // Calling twice should not error
        ensure_telemetry_table(&conn).unwrap();
    }

    #[test]
    fn test_record_and_count_events() {
        let conn = setup_test_db();
        record_event(&conn, "app_launch", None, None, Some("s1")).unwrap();
        record_event(
            &conn,
            "view_open:results",
            Some("results"),
            None,
            Some("s1"),
        )
        .unwrap();
        record_event(
            &conn,
            "search_query",
            None,
            Some(r#"{"q":"rust"}"#),
            Some("s1"),
        )
        .unwrap();

        let report = get_usage_report(&conn, 1).unwrap();
        assert_eq!(report.total_events, 3);
        assert_eq!(report.sessions, 1);
        assert_eq!(report.search_count, 1);
    }

    #[test]
    fn test_ghost_click_rate() {
        let conn = setup_test_db();
        for _ in 0..10 {
            record_event(&conn, "ghost_preview_shown", None, None, Some("s1")).unwrap();
        }
        for _ in 0..3 {
            record_event(&conn, "ghost_preview_clicked", None, None, Some("s1")).unwrap();
        }
        let report = get_usage_report(&conn, 1).unwrap();
        assert_eq!(report.ghost_preview_impressions, 10);
        assert_eq!(report.ghost_preview_clicks, 3);
        assert!((report.ghost_click_rate - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_empty_report() {
        let conn = setup_test_db();
        let report = get_usage_report(&conn, 7).unwrap();
        assert_eq!(report.total_events, 0);
        assert_eq!(report.sessions, 0);
        assert_eq!(report.ghost_click_rate, 0.0);
        assert_eq!(report.avg_session_views, 0.0);
        assert!(report.most_active_day.is_none());
    }
}
