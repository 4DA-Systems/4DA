//! Local Telemetry — privacy-first usage analytics that never leave the machine.
//!
//! All data stays in local SQLite. No external services. No aggregation.
//! Users can view their own usage patterns and delete at any time.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use tracing::{info, warn};

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

/// Creates the user_events and error_telemetry tables and indexes if they don't exist.
/// Production uses db/migrations.rs (Phase 25 for user_events, Phase 33 for error_telemetry).
/// This is for test DBs.
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
    ensure_error_telemetry_table(conn)?;
    Ok(())
}

/// Creates the error_telemetry table for test DBs.
/// Production uses db/migrations.rs (Phase 33).
#[cfg(test)]
fn ensure_error_telemetry_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS error_telemetry (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            category TEXT NOT NULL,
            message TEXT NOT NULL,
            context TEXT,
            count INTEGER DEFAULT 1,
            first_seen TEXT NOT NULL,
            last_seen TEXT NOT NULL,
            UNIQUE(category, message)
        );
        CREATE INDEX IF NOT EXISTS idx_error_telemetry_category ON error_telemetry(category);
        CREATE INDEX IF NOT EXISTS idx_error_telemetry_last_seen ON error_telemetry(last_seen);",
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
// Error Telemetry — local error tracking for developer visibility
// ============================================================================

/// A single error record from the error_telemetry table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub id: i64,
    pub category: String,
    pub message: String,
    pub context: Option<String>,
    pub count: i64,
    pub first_seen: String,
    pub last_seen: String,
}

/// Summary of error telemetry: counts by category, top errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSummary {
    pub total_errors: i64,
    pub total_occurrences: i64,
    pub by_category: Vec<CategoryCount>,
    pub top_errors: Vec<ErrorRecord>,
}

/// Error count for a single category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryCount {
    pub category: String,
    pub unique_errors: i64,
    pub total_occurrences: i64,
}

/// Record an error to the local error_telemetry table.
///
/// Uses upsert: if the same category+message already exists, increments count
/// and updates last_seen. Otherwise inserts a new row.
pub fn record_error(
    conn: &Connection,
    category: &str,
    message: &str,
    context: Option<&str>,
) -> Result<()> {
    // Truncate message to 1000 chars to prevent bloat from stack traces
    let msg = if message.len() > 1000 {
        &message[..1000]
    } else {
        message
    };

    conn.execute(
        "INSERT INTO error_telemetry (category, message, context, count, first_seen, last_seen)
         VALUES (?1, ?2, ?3, 1, datetime('now'), datetime('now'))
         ON CONFLICT(category, message) DO UPDATE SET
           count = count + 1,
           last_seen = datetime('now'),
           context = COALESCE(?3, context)",
        params![category, msg, context],
    )
    .map_err(FourDaError::Db)?;
    Ok(())
}

/// Retrieve recent errors, ordered by last_seen descending.
pub fn get_recent_errors(conn: &Connection, limit: u32) -> Result<Vec<ErrorRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, category, message, context, count, first_seen, last_seen
             FROM error_telemetry
             ORDER BY last_seen DESC
             LIMIT ?1",
        )
        .map_err(FourDaError::Db)?;

    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(ErrorRecord {
                id: row.get(0)?,
                category: row.get(1)?,
                message: row.get(2)?,
                context: row.get(3)?,
                count: row.get(4)?,
                first_seen: row.get(5)?,
                last_seen: row.get(6)?,
            })
        })
        .map_err(FourDaError::Db)?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();

    Ok(rows)
}

/// Get a summary of all errors: counts by category, most frequent errors.
pub fn get_error_summary(conn: &Connection) -> Result<ErrorSummary> {
    let total_errors: i64 = conn
        .query_row("SELECT COUNT(*) FROM error_telemetry", [], |row| row.get(0))
        .unwrap_or(0);

    let total_occurrences: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(count), 0) FROM error_telemetry",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Counts grouped by category
    let by_category = {
        let mut stmt = conn
            .prepare(
                "SELECT category, COUNT(*) as unique_errors, SUM(count) as total_occurrences
                 FROM error_telemetry
                 GROUP BY category
                 ORDER BY total_occurrences DESC",
            )
            .map_err(FourDaError::Db)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(CategoryCount {
                    category: row.get(0)?,
                    unique_errors: row.get(1)?,
                    total_occurrences: row.get(2)?,
                })
            })
            .map_err(FourDaError::Db)?
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();
        rows
    };

    // Top 10 most frequent errors
    let top_errors = {
        let mut stmt = conn
            .prepare(
                "SELECT id, category, message, context, count, first_seen, last_seen
                 FROM error_telemetry
                 ORDER BY count DESC
                 LIMIT 10",
            )
            .map_err(FourDaError::Db)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ErrorRecord {
                    id: row.get(0)?,
                    category: row.get(1)?,
                    message: row.get(2)?,
                    context: row.get(3)?,
                    count: row.get(4)?,
                    first_seen: row.get(5)?,
                    last_seen: row.get(6)?,
                })
            })
            .map_err(FourDaError::Db)?
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();
        rows
    };

    Ok(ErrorSummary {
        total_errors,
        total_occurrences,
        by_category,
        top_errors,
    })
}

/// Delete errors older than the specified number of days.
pub fn clear_old_errors(conn: &Connection, days: u32) -> Result<u64> {
    let cutoff = format!("-{} days", days);
    let deleted = conn
        .execute(
            "DELETE FROM error_telemetry WHERE last_seen < datetime('now', ?1)",
            params![cutoff],
        )
        .map_err(FourDaError::Db)?;
    if deleted > 0 {
        info!(target: "4da::telemetry", deleted, days, "Pruned old error telemetry records");
    }
    Ok(deleted as u64)
}

/// Fire-and-forget error recording. Opens its own connection and logs failures
/// instead of propagating — error telemetry should never crash the caller.
pub fn record_error_async(category: &str, message: &str, context: Option<&str>) {
    // Capture owned copies for the non-async path
    let cat = category.to_string();
    let msg = message.to_string();
    let ctx = context.map(|s| s.to_string());

    // Best-effort: silently absorb failures so error tracking never disrupts the app
    match crate::open_db_connection() {
        Ok(conn) => {
            if let Err(e) = record_error(&conn, &cat, &msg, ctx.as_deref()) {
                warn!(target: "4da::telemetry", error = %e, "Failed to record error telemetry");
            }
        }
        Err(e) => {
            warn!(target: "4da::telemetry", error = %e, "Failed to open DB for error telemetry");
        }
    }
}

// ============================================================================
// Error Telemetry — Tauri Commands
// ============================================================================

/// Returns recent errors from the local error telemetry table.
/// Frontend can display these in a developer diagnostics panel.
#[tauri::command]
pub async fn get_error_telemetry(limit: Option<u32>) -> Result<Vec<ErrorRecord>> {
    let conn = crate::open_db_connection()?;
    get_recent_errors(&conn, limit.unwrap_or(50))
}

/// Returns a summary of error counts by category and top errors.
#[tauri::command]
pub async fn get_error_summary_cmd() -> Result<ErrorSummary> {
    let conn = crate::open_db_connection()?;
    get_error_summary(&conn)
}

/// Prune error telemetry records older than N days (default: 30).
#[tauri::command]
pub async fn clear_error_telemetry(days: Option<u32>) -> Result<u64> {
    let conn = crate::open_db_connection()?;
    clear_old_errors(&conn, days.unwrap_or(30))
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

    // ========================================================================
    // Error Telemetry Tests
    // ========================================================================

    #[test]
    fn test_record_error_basic() {
        let conn = setup_test_db();
        record_error(
            &conn,
            "source_fetch",
            "Connection timeout",
            Some("hackernews"),
        )
        .unwrap();

        let errors = get_recent_errors(&conn, 10).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].category, "source_fetch");
        assert_eq!(errors[0].message, "Connection timeout");
        assert_eq!(errors[0].context.as_deref(), Some("hackernews"));
        assert_eq!(errors[0].count, 1);
    }

    #[test]
    fn test_record_error_upsert_increments_count() {
        let conn = setup_test_db();
        // Same category+message recorded three times
        record_error(&conn, "llm", "Rate limit exceeded", None).unwrap();
        record_error(&conn, "llm", "Rate limit exceeded", None).unwrap();
        record_error(&conn, "llm", "Rate limit exceeded", None).unwrap();

        let errors = get_recent_errors(&conn, 10).unwrap();
        assert_eq!(errors.len(), 1); // Single row via upsert
        assert_eq!(errors[0].count, 3); // Count incremented
    }

    #[test]
    fn test_record_error_different_messages_separate_rows() {
        let conn = setup_test_db();
        record_error(&conn, "source_fetch", "Connection timeout", None).unwrap();
        record_error(&conn, "source_fetch", "DNS resolution failed", None).unwrap();
        record_error(&conn, "llm", "Connection timeout", None).unwrap();

        let errors = get_recent_errors(&conn, 10).unwrap();
        assert_eq!(errors.len(), 3); // Three distinct category+message combos
    }

    #[test]
    fn test_error_summary() {
        let conn = setup_test_db();
        record_error(&conn, "source_fetch", "Connection timeout", None).unwrap();
        record_error(&conn, "source_fetch", "Connection timeout", None).unwrap();
        record_error(&conn, "source_fetch", "DNS failure", None).unwrap();
        record_error(&conn, "llm", "API key invalid", None).unwrap();

        let summary = get_error_summary(&conn).unwrap();
        assert_eq!(summary.total_errors, 3); // 3 unique errors
        assert_eq!(summary.total_occurrences, 4); // 4 total occurrences
        assert_eq!(summary.by_category.len(), 2); // source_fetch and llm

        // source_fetch should be first (most occurrences)
        assert_eq!(summary.by_category[0].category, "source_fetch");
        assert_eq!(summary.by_category[0].unique_errors, 2);
        assert_eq!(summary.by_category[0].total_occurrences, 3);
    }

    #[test]
    fn test_clear_old_errors() {
        let conn = setup_test_db();
        // Insert an error with old timestamp
        conn.execute(
            "INSERT INTO error_telemetry (category, message, count, first_seen, last_seen)
             VALUES ('old_cat', 'old error', 1, datetime('now', '-60 days'), datetime('now', '-60 days'))",
            [],
        )
        .unwrap();
        // Insert a fresh error
        record_error(&conn, "fresh_cat", "fresh error", None).unwrap();

        let deleted = clear_old_errors(&conn, 30).unwrap();
        assert_eq!(deleted, 1);

        let remaining = get_recent_errors(&conn, 10).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].category, "fresh_cat");
    }

    #[test]
    fn test_error_summary_empty() {
        let conn = setup_test_db();
        let summary = get_error_summary(&conn).unwrap();
        assert_eq!(summary.total_errors, 0);
        assert_eq!(summary.total_occurrences, 0);
        assert!(summary.by_category.is_empty());
        assert!(summary.top_errors.is_empty());
    }

    #[test]
    fn test_error_message_truncation() {
        let conn = setup_test_db();
        let long_message = "x".repeat(2000);
        record_error(&conn, "test", &long_message, None).unwrap();

        let errors = get_recent_errors(&conn, 10).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message.len(), 1000); // Truncated to 1000 chars
    }

    #[test]
    fn test_error_telemetry_table_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_error_telemetry_table(&conn).unwrap();
        // Second call should not error
        ensure_error_telemetry_table(&conn).unwrap();
    }

    #[test]
    fn test_recent_errors_respects_limit() {
        let conn = setup_test_db();
        for i in 0..20 {
            record_error(&conn, "test", &format!("error {}", i), None).unwrap();
        }
        let errors = get_recent_errors(&conn, 5).unwrap();
        assert_eq!(errors.len(), 5);
    }

    #[test]
    fn test_upsert_updates_context() {
        let conn = setup_test_db();
        record_error(&conn, "source_fetch", "timeout", None).unwrap();
        record_error(&conn, "source_fetch", "timeout", Some("reddit")).unwrap();

        let errors = get_recent_errors(&conn, 10).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].count, 2);
        assert_eq!(errors[0].context.as_deref(), Some("reddit")); // Context updated
    }
}
