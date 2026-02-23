//! Pro Value Report for 4DA
//!
//! Computes measurable value delivered by Pro features: briefings generated,
//! signals detected, knowledge gaps caught, and estimated hours saved.
//! NOT Pro-gated — free users see what they're missing.

use serde::{Deserialize, Serialize};
use tracing::info;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProValueReport {
    pub period_days: u32,
    pub briefings_generated: u32,
    pub signals_detected: u32,
    pub knowledge_gaps_caught: u32,
    pub predictions_made: u32,
    pub queries_run: u32,
    pub items_surfaced: u32,
    pub attention_insights: u32,
    pub estimated_hours_saved: f32,
    pub data_age_days: u32,
    pub total_feedback_events: u32,
    pub active_since: Option<String>,
}

// ============================================================================
// Implementation
// ============================================================================

/// Count rows from a table/query, returning 0 if the table doesn't exist.
fn safe_count(conn: &rusqlite::Connection, sql: &str, p: &[&dyn rusqlite::types::ToSql]) -> u32 {
    conn.query_row(sql, p, |row| row.get::<_, u32>(0))
        .unwrap_or(0)
}

/// Compute Pro value metrics over the given number of days.
pub fn compute_pro_value(
    conn: &rusqlite::Connection,
    period_days: u32,
) -> Result<ProValueReport, String> {
    let days_param = format!("-{} days", period_days);

    let briefings_generated = safe_count(
        conn,
        "SELECT COUNT(*) FROM temporal_events WHERE event_type = 'briefing_generated' AND created_at >= datetime('now', ?1)",
        &[&days_param as &dyn rusqlite::types::ToSql],
    );

    let predictions_made = safe_count(
        conn,
        "SELECT COUNT(*) FROM temporal_events WHERE event_type = 'predicted_context' AND created_at >= datetime('now', ?1)",
        &[&days_param as &dyn rusqlite::types::ToSql],
    );

    let queries_run = safe_count(
        conn,
        "SELECT COUNT(*) FROM temporal_events WHERE event_type = 'nl_query' AND created_at >= datetime('now', ?1)",
        &[&days_param as &dyn rusqlite::types::ToSql],
    );

    let attention_insights = safe_count(
        conn,
        "SELECT COUNT(*) FROM temporal_events WHERE event_type = 'attention_blind_spot' AND created_at >= datetime('now', ?1)",
        &[&days_param as &dyn rusqlite::types::ToSql],
    );

    let signals_detected = safe_count(conn, "SELECT COUNT(*) FROM signal_chains", &[]);

    let knowledge_gaps_caught = safe_count(
        conn,
        "SELECT COUNT(DISTINCT package_name) FROM project_dependencies",
        &[],
    );

    let items_surfaced = safe_count(
        conn,
        "SELECT COUNT(*) FROM source_items WHERE created_at >= datetime('now', ?1)",
        &[&days_param as &dyn rusqlite::types::ToSql],
    );

    let total_feedback_events = safe_count(conn, "SELECT COUNT(*) FROM feedback", &[]);

    // Data age: earliest temporal event
    let active_since: Option<String> = conn
        .query_row("SELECT MIN(created_at) FROM temporal_events", [], |row| {
            row.get(0)
        })
        .unwrap_or(None);

    let data_age_days = active_since
        .as_deref()
        .and_then(|ts| {
            chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S"))
                .ok()
        })
        .map(|dt| {
            let now = chrono::Utc::now().naive_utc();
            now.signed_duration_since(dt).num_days().max(0) as u32
        })
        .unwrap_or(0);

    let estimated_hours_saved = briefings_generated as f32 * 0.5
        + signals_detected as f32 * 0.25
        + knowledge_gaps_caught as f32 * 0.1
        + queries_run as f32 * 0.15;

    let report = ProValueReport {
        period_days,
        briefings_generated,
        signals_detected,
        knowledge_gaps_caught,
        predictions_made,
        queries_run,
        items_surfaced,
        attention_insights,
        estimated_hours_saved,
        data_age_days,
        total_feedback_events,
        active_since,
    };

    info!(
        target: "4da::pro_value",
        briefings = briefings_generated,
        signals = signals_detected,
        gaps = knowledge_gaps_caught,
        hours_saved = %format!("{:.1}", estimated_hours_saved),
        data_age = data_age_days,
        "Pro value report computed"
    );

    Ok(report)
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get a Pro value report — NOT Pro-gated so free users see what they're missing.
#[tauri::command]
pub fn get_pro_value_report() -> Result<ProValueReport, String> {
    let conn = crate::open_db_connection()?;
    compute_pro_value(&conn, 30)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

    fn setup_test_db() -> Connection {
        crate::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE temporal_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                data JSON NOT NULL DEFAULT '{}',
                embedding BLOB,
                source_item_id INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT
            );
            CREATE TABLE signal_chains (
                id TEXT PRIMARY KEY,
                chain_name TEXT NOT NULL,
                links JSON NOT NULL DEFAULT '[]',
                overall_priority TEXT DEFAULT 'medium',
                resolution TEXT DEFAULT 'open',
                suggested_action TEXT DEFAULT '',
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            );
            CREATE TABLE project_dependencies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                manifest_type TEXT NOT NULL,
                package_name TEXT NOT NULL,
                version TEXT,
                is_dev INTEGER DEFAULT 0,
                language TEXT NOT NULL,
                last_scanned TEXT DEFAULT (datetime('now')),
                UNIQUE(project_path, package_name)
            );
            CREATE TABLE source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                url TEXT,
                title TEXT NOT NULL,
                content TEXT DEFAULT '',
                content_hash TEXT DEFAULT '',
                embedding BLOB DEFAULT x'00',
                created_at TEXT DEFAULT (datetime('now')),
                last_seen TEXT DEFAULT (datetime('now')),
                UNIQUE(source_type, source_id)
            );
            CREATE TABLE feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER NOT NULL,
                relevant INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_empty_db_returns_zeros() {
        let conn = setup_test_db();
        let report = compute_pro_value(&conn, 30).unwrap();
        assert_eq!(report.briefings_generated, 0);
        assert_eq!(report.signals_detected, 0);
        assert_eq!(report.total_feedback_events, 0);
        assert_eq!(report.estimated_hours_saved, 0.0);
        assert!(report.active_since.is_none());
    }

    #[test]
    fn test_counts_temporal_events_by_type() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO temporal_events (event_type, subject, data) VALUES ('briefing_generated', 'test', '{}')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO temporal_events (event_type, subject, data) VALUES ('briefing_generated', 'test2', '{}')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO temporal_events (event_type, subject, data) VALUES ('nl_query', 'q1', '{}')",
            [],
        ).unwrap();

        let report = compute_pro_value(&conn, 30).unwrap();
        assert_eq!(report.briefings_generated, 2);
        assert_eq!(report.queries_run, 1);
        assert_eq!(report.predictions_made, 0);
    }

    #[test]
    fn test_hours_saved_calculation() {
        let conn = setup_test_db();
        // 2 briefings (2*0.5=1.0) + 1 signal (0.25) + 3 deps (3*0.1=0.3) + 1 query (0.15)
        conn.execute(
            "INSERT INTO temporal_events (event_type, subject, data) VALUES ('briefing_generated', 'b1', '{}')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO temporal_events (event_type, subject, data) VALUES ('briefing_generated', 'b2', '{}')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO temporal_events (event_type, subject, data) VALUES ('nl_query', 'q1', '{}')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO signal_chains (id, chain_name) VALUES ('c1', 'chain1')",
            [],
        )
        .unwrap();
        for (name, lang) in [("react", "js"), ("serde", "rust"), ("tokio", "rust")] {
            conn.execute(
                "INSERT INTO project_dependencies (project_path, manifest_type, package_name, language) VALUES ('/test', 'cargo', ?1, ?2)",
                params![name, lang],
            ).unwrap();
        }

        let report = compute_pro_value(&conn, 30).unwrap();
        let expected = 2.0 * 0.5 + 1.0 * 0.25 + 3.0 * 0.1 + 1.0 * 0.15;
        assert!((report.estimated_hours_saved - expected).abs() < 0.01);
    }

    #[test]
    fn test_active_since_and_data_age() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO temporal_events (event_type, subject, data, created_at) VALUES ('briefing_generated', 'old', '{}', datetime('now', '-10 days'))",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO temporal_events (event_type, subject, data) VALUES ('nl_query', 'new', '{}')",
            [],
        ).unwrap();

        let report = compute_pro_value(&conn, 30).unwrap();
        assert!(report.active_since.is_some());
        assert!(report.data_age_days >= 10);
    }

    #[test]
    fn test_missing_tables_return_zeros() {
        crate::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        // No tables created at all
        let report = compute_pro_value(&conn, 30).unwrap();
        assert_eq!(report.briefings_generated, 0);
        assert_eq!(report.signals_detected, 0);
        assert_eq!(report.knowledge_gaps_caught, 0);
        assert_eq!(report.total_feedback_events, 0);
        assert_eq!(report.period_days, 30);
    }
}
