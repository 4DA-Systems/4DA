// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Bounded retention for the app's own bookkeeping tables.
//!
//! `engine_runs`, `security_audit_log`, `anomalies`, `scoring_churn` and
//! `ai_usage` had no pruner at all (audit 2026-11-11): every cycle, security
//! event, anomaly, scoring pass and model call added a row forever. Each gets
//! an age bound here plus, where a row stream can burst, a row cap that keeps
//! the newest. None of these tables holds the user's own content (Saved items,
//! feedback, decisions, settings), so nothing here reaches it; `source_items`
//! retention, which must keep Saved items, stays in `history.rs`.
//!
//! Runs inside the daily `run_maintenance` pass, which the GUI scheduler and
//! the headless engine (`headless::run_daily_retention`) both call.

use rusqlite::Connection;

/// Freshness receipts: 30 days of history, and never fewer than the newest
/// [`ENGINE_RUNS_KEEP_NEWEST`]. The newest receipt (what MCP and the CLI read
/// for freshness) and the newest scoring watermark (the next run's
/// differential baseline) are never deleted, however old.
pub(crate) const ENGINE_RUNS_DAYS: u32 = 30;
pub(crate) const ENGINE_RUNS_KEEP_NEWEST: u32 = 500;
/// Security events are kept longer: they are what someone reads after the fact.
pub(crate) const SECURITY_AUDIT_DAYS: u32 = 180;
pub(crate) const SECURITY_AUDIT_MAX_ROWS: u32 = 10_000;
/// Resolved anomalies are history after 30 days; open ones after 90.
pub(crate) const ANOMALIES_RESOLVED_DAYS: u32 = 30;
pub(crate) const ANOMALIES_DAYS: u32 = 90;
pub(crate) const SCORING_CHURN_DAYS: u32 = 90;
/// Model usage feeds monthly cost totals: a little over a year.
pub(crate) const AI_USAGE_DAYS: u32 = 400;

/// Rows each table lost in one pass.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryPruned {
    pub engine_runs: usize,
    pub security_audit_log: usize,
    pub anomalies: usize,
    pub scoring_churn: usize,
    pub ai_usage: usize,
}

#[cfg(test)]
impl TelemetryPruned {
    pub fn total(&self) -> usize {
        self.engine_runs
            + self.security_audit_log
            + self.anomalies
            + self.scoring_churn
            + self.ai_usage
    }
}

/// A DELETE against a table that may not exist on this database (an older
/// schema, or a table created lazily on first use) counts as nothing pruned.
fn prune(conn: &Connection, sql: &str) -> usize {
    match conn.execute(sql, []) {
        Ok(n) => n,
        Err(e) if e.to_string().contains("no such table") => 0,
        Err(e) => {
            tracing::warn!(target: "4da::db", error = %e, sql, "Retention prune failed (non-fatal)");
            0
        }
    }
}

/// Prune every bookkeeping table to its bound. `conn` may be a transaction.
pub(crate) fn prune_telemetry_on(conn: &Connection) -> TelemetryPruned {
    // `engine_runs.completed_at` is RFC3339; its first 10 characters are the
    // UTC date, which compares cleanly against `date()`.
    let engine_runs = prune(
        conn,
        &format!(
            "DELETE FROM engine_runs
             WHERE substr(completed_at, 1, 10) < date('now', '-{ENGINE_RUNS_DAYS} days')
               AND id NOT IN (SELECT id FROM engine_runs ORDER BY id DESC LIMIT {ENGINE_RUNS_KEEP_NEWEST})
               AND id NOT IN (SELECT COALESCE(MAX(id), -1) FROM engine_runs WHERE ok = 1 AND items_scored > 0)"
        ),
    );
    let security_audit_log = prune(
        conn,
        &format!(
            "DELETE FROM security_audit_log
             WHERE timestamp < datetime('now', '-{SECURITY_AUDIT_DAYS} days')
                OR id NOT IN (SELECT id FROM security_audit_log ORDER BY id DESC LIMIT {SECURITY_AUDIT_MAX_ROWS})"
        ),
    );
    let anomalies = prune(
        conn,
        &format!(
            "DELETE FROM anomalies
             WHERE detected_at < datetime('now', '-{ANOMALIES_DAYS} days')
                OR (resolved = 1 AND detected_at < datetime('now', '-{ANOMALIES_RESOLVED_DAYS} days'))"
        ),
    );
    let scoring_churn = prune(
        conn,
        &format!("DELETE FROM scoring_churn WHERE created_at < datetime('now', '-{SCORING_CHURN_DAYS} days')"),
    );
    let ai_usage = prune(
        conn,
        &format!(
            "DELETE FROM ai_usage WHERE created_at < datetime('now', '-{AI_USAGE_DAYS} days')"
        ),
    );
    TelemetryPruned {
        engine_runs,
        security_audit_log,
        anomalies,
        scoring_churn,
        ai_usage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    fn open() -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::new(&dir.path().join("4da.db")).expect("open db");
        {
            let conn = db.conn.lock();
            crate::engine_runs::ensure_table(&conn).unwrap();
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS anomalies (
                    id INTEGER PRIMARY KEY, anomaly_type TEXT NOT NULL, topic TEXT,
                    description TEXT NOT NULL, confidence REAL DEFAULT 0.5,
                    severity TEXT DEFAULT 'medium', evidence TEXT DEFAULT '[]',
                    detected_at TEXT DEFAULT (datetime('now')), resolved INTEGER DEFAULT 0);",
            )
            .unwrap();
        }
        (dir, db)
    }

    fn receipt(conn: &Connection, days_ago: i64, ok: i64, scored: i64) {
        conn.execute(
            "INSERT INTO engine_runs (trigger, started_at, completed_at, ok, items_scored)
             VALUES ('t', ?1, ?1, ?2, ?3)",
            rusqlite::params![
                (chrono::Utc::now() - chrono::Duration::days(days_ago)).to_rfc3339(),
                ok,
                scored
            ],
        )
        .unwrap();
    }

    /// Audit 2026-11-11: these five tables grew without bound. Old rows go,
    /// recent rows stay, and each bound is the documented one.
    #[test]
    fn old_bookkeeping_rows_are_pruned_and_recent_ones_kept() {
        let (_dir, db) = open();
        let conn = db.conn.lock();
        let old_ts = "datetime('now', '-500 days')";
        conn.execute_batch(&format!(
            "INSERT INTO security_audit_log (timestamp, event_type) VALUES ({old_ts}, 'old'), (datetime('now'), 'new');
             INSERT INTO anomalies (anomaly_type, description, detected_at, resolved) VALUES
                ('a', 'old open', {old_ts}, 0),
                ('a', 'resolved 40d', datetime('now', '-40 days'), 1),
                ('a', 'open 40d', datetime('now', '-40 days'), 0),
                ('a', 'new', datetime('now'), 0);
             INSERT INTO scoring_churn (created_at, path, pipeline_version, items_written, rescored,
                moved_up_gt_010, moved_down_gt_010, max_up, max_down, mean_abs_delta) VALUES
                ({old_ts}, 'p', 1, 0, 0, 0, 0, 0, 0, 0), (datetime('now'), 'p', 1, 0, 0, 0, 0, 0, 0, 0);
             INSERT INTO ai_usage (provider, model, task_type, created_at) VALUES
                ('x', 'm', 't', {old_ts}), ('x', 'm', 't', datetime('now'));"
        ))
        .unwrap();

        let pruned = prune_telemetry_on(&conn);
        assert_eq!(pruned.security_audit_log, 1);
        assert_eq!(pruned.anomalies, 2, "old open + resolved after 30 days");
        assert_eq!(pruned.scoring_churn, 1);
        assert_eq!(pruned.ai_usage, 1);
        assert_eq!(count(&conn, "security_audit_log"), 1);
        assert_eq!(count(&conn, "anomalies"), 2, "open 40d + new survive");
        assert_eq!(count(&conn, "scoring_churn"), 1);
        assert_eq!(count(&conn, "ai_usage"), 1);
        assert_eq!(prune_telemetry_on(&conn).total(), 0, "idempotent");
    }

    /// Receipts past 30 days go once more than the newest 500 exist, but the
    /// newest receipt and the scoring watermark survive at any age.
    #[test]
    fn engine_runs_keep_the_newest_and_the_watermark() {
        let (_dir, db) = open();
        let conn = db.conn.lock();
        receipt(&conn, 400, 1, 50); // the watermark: last ok run that scored
        for _ in 0..(ENGINE_RUNS_KEEP_NEWEST + 20) {
            receipt(&conn, 200, 1, 0);
        }
        receipt(&conn, 0, 1, 0);
        let watermark: i64 = conn
            .query_row(
                "SELECT MAX(id) FROM engine_runs WHERE ok = 1 AND items_scored > 0",
                [],
                |r| r.get(0),
            )
            .unwrap();

        let total = count(&conn, "engine_runs");
        let pruned = prune_telemetry_on(&conn);
        // Everything old goes except the newest 500 rows and the watermark.
        let kept = i64::from(ENGINE_RUNS_KEEP_NEWEST) + 1;
        assert_eq!(pruned.engine_runs as i64, total - kept);
        assert_eq!(count(&conn, "engine_runs"), kept);
        let still: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM engine_runs WHERE id = ?1",
                [watermark],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(still, 1, "the differential watermark is never pruned");
    }

    /// The daily deep clean (GUI scheduler and headless engine) applies the
    /// bounds: before, it never touched these tables.
    #[test]
    fn the_daily_deep_clean_prunes_bookkeeping_tables() {
        let (_dir, db) = open();
        {
            let conn = db.conn.lock();
            conn.execute_batch(
                "INSERT INTO scoring_churn (created_at, path, pipeline_version, items_written, rescored,
                    moved_up_gt_010, moved_down_gt_010, max_up, max_down, mean_abs_delta) VALUES
                    (datetime('now', '-500 days'), 'p', 1, 0, 0, 0, 0, 0, 0, 0);
                 INSERT INTO ai_usage (provider, model, task_type, created_at) VALUES
                    ('x', 'm', 't', datetime('now', '-500 days'));",
            )
            .unwrap();
        }
        db.run_maintenance(30).expect("deep clean");
        let conn = db.conn.lock();
        assert_eq!(count(&conn, "scoring_churn"), 0);
        assert_eq!(count(&conn, "ai_usage"), 0);
    }

    /// The security log also has a row cap for bursts inside the age window.
    #[test]
    fn security_log_is_capped_at_its_newest_rows() {
        let (_dir, db) = open();
        let conn = db.conn.lock();
        conn.execute_batch(&format!(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {})
             INSERT INTO security_audit_log (event_type) SELECT 'burst' FROM n;",
            SECURITY_AUDIT_MAX_ROWS + 5
        ))
        .unwrap();
        assert_eq!(prune_telemetry_on(&conn).security_audit_log, 5);
        assert_eq!(
            count(&conn, "security_audit_log"),
            i64::from(SECURITY_AUDIT_MAX_ROWS)
        );
    }
}
