// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Feedback-loop liveness — is the human feedback loop alive at all?
//!
//! The calibration monitor stays silent until it has >= 10 feedback rows, the
//! `feedback` table held ONE row ever, and `interactions` stopped on
//! 2026-08-24 — while the Signal tab kept surfacing hundreds of items a week
//! and nothing on screen said the scores were uncalibrated to this user. This
//! is the one query that turns that into a sentence the UI can show: how much
//! was surfaced in the window, and how much of anything came back.
//!
//! Read-only, cheap (three COUNTs and two MAXes on indexed timestamp columns),
//! and deliberately not a metric dashboard: the frontend renders a single line
//! when surfaced is high and both feedback channels are at zero.

use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use crate::open_db_connection;

/// The look-back window. Two weeks: long enough that a holiday does not trip
/// the banner, short enough that a dead loop is called out the same month.
pub(crate) const LIVENESS_WINDOW_DAYS: i64 = 14;

/// Interaction kinds the frontend never records deliberately. `scroll` and
/// `ignore` are visibility telemetry, not the "clicks" the banner talks about;
/// counting them would let a user who only ever scrolled read as engaged.
const PASSIVE_INTERACTIONS: &str = "('scroll', 'ignore')";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct FeedbackLiveness {
    /// `trust_events` rows of type `surfaced` inside the window.
    pub surfaced_14d: i64,
    /// Explicit relevant / not-relevant ratings (`feedback` rows) in the window.
    pub feedback_14d: i64,
    /// Active `interactions` rows (click, save, engagement_complete, ...) in
    /// the window; passive `scroll` / `ignore` telemetry is excluded.
    pub interactions_14d: i64,
    /// All-time latest rating, DB timestamp format (`YYYY-MM-DD HH:MM:SS`, UTC).
    pub last_feedback_at: Option<String>,
    /// All-time latest active interaction, same format.
    pub last_interaction_at: Option<String>,
}

fn table_exists(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n > 0)
}

/// Compute liveness over the last [`LIVENESS_WINDOW_DAYS`].
///
/// `interactions` is created lazily by the ACE layer, not by the core
/// migrations, so a fresh install can legitimately lack it — that reads as
/// zero interactions, which is also the truth.
pub(crate) fn compute_feedback_liveness(conn: &Connection) -> rusqlite::Result<FeedbackLiveness> {
    let window = format!("-{LIVENESS_WINDOW_DAYS} days");

    let surfaced_14d: i64 = conn.query_row(
        "SELECT COUNT(*) FROM trust_events
         WHERE event_type = 'surfaced' AND created_at >= datetime('now', ?1)",
        [&window],
        |row| row.get(0),
    )?;

    let feedback_14d: i64 = conn.query_row(
        "SELECT COUNT(*) FROM feedback WHERE created_at >= datetime('now', ?1)",
        [&window],
        |row| row.get(0),
    )?;
    let last_feedback_at: Option<String> =
        conn.query_row("SELECT MAX(created_at) FROM feedback", [], |row| row.get(0))?;

    let (interactions_14d, last_interaction_at) = if table_exists(conn, "interactions")? {
        let count: i64 = conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM interactions
                 WHERE timestamp >= datetime('now', ?1)
                   AND COALESCE(action_type, '') NOT IN {PASSIVE_INTERACTIONS}"
            ),
            [&window],
            |row| row.get(0),
        )?;
        let last: Option<String> = conn.query_row(
            &format!(
                "SELECT MAX(timestamp) FROM interactions
                 WHERE COALESCE(action_type, '') NOT IN {PASSIVE_INTERACTIONS}"
            ),
            [],
            |row| row.get(0),
        )?;
        (count, last)
    } else {
        (0, None)
    };

    Ok(FeedbackLiveness {
        surfaced_14d,
        feedback_14d,
        interactions_14d,
        last_feedback_at,
        last_interaction_at,
    })
}

/// Signal-tab banner input: surfaced volume vs. feedback volume over the last
/// two weeks. The frontend decides whether that combination warrants a line.
#[tauri::command]
pub async fn get_feedback_liveness() -> std::result::Result<FeedbackLiveness, String> {
    let conn = open_db_connection().map_err(|e| e.to_string())?;
    compute_feedback_liveness(&conn).map_err(|e| format!("feedback liveness query failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db;

    /// `interactions` is ACE-owned (see `ace/db.rs`); the migrated test DB
    /// may not carry it, so the tests create the canonical shape themselves.
    fn ensure_interactions(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS interactions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER,
                item_id INTEGER,
                action TEXT,
                action_type TEXT,
                action_data TEXT,
                item_topics TEXT,
                item_source TEXT,
                signal_strength REAL DEFAULT 0.5,
                timestamp TEXT DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
    }

    fn surfaced(conn: &Connection, age: &str) {
        conn.execute(
            "INSERT INTO trust_events (event_type, created_at) VALUES ('surfaced', datetime('now', ?1))",
            [age],
        )
        .unwrap();
    }

    #[test]
    fn empty_db_is_all_zero_and_none() {
        let db = test_db();
        let conn = db.conn.lock();
        let l = compute_feedback_liveness(&conn).unwrap();
        assert_eq!(
            l,
            FeedbackLiveness {
                surfaced_14d: 0,
                feedback_14d: 0,
                interactions_14d: 0,
                last_feedback_at: None,
                last_interaction_at: None,
            }
        );
    }

    #[test]
    fn missing_interactions_table_reads_as_zero_not_error() {
        let db = test_db();
        let conn = db.conn.lock();
        conn.execute_batch("DROP TABLE IF EXISTS interactions")
            .unwrap();
        let l = compute_feedback_liveness(&conn).unwrap();
        assert_eq!(l.interactions_14d, 0);
        assert_eq!(l.last_interaction_at, None);
    }

    #[test]
    fn counts_only_the_window_but_reports_the_all_time_last() {
        let db = test_db();
        // Item row so the feedback FK (if enforced) has something to point at.
        // Inserted BEFORE taking the writer lock: upsert takes it too.
        let id = crate::test_utils::insert_test_item(&db, "hackernews", "hn_1", "t", "c");
        let conn = db.conn.lock();
        ensure_interactions(&conn);

        surfaced(&conn, "-1 days");
        surfaced(&conn, "-13 days");
        surfaced(&conn, "-15 days"); // outside the window

        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant, created_at)
             VALUES (?1, 1, datetime('now', '-20 days'))",
            [id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO interactions (item_id, action_type, timestamp)
             VALUES (?1, 'click', datetime('now', '-30 days'))",
            [id],
        )
        .unwrap();

        let l = compute_feedback_liveness(&conn).unwrap();
        assert_eq!(
            l.surfaced_14d, 2,
            "the 15-day-old surface is outside the window"
        );
        assert_eq!(
            l.feedback_14d, 0,
            "a 20-day-old rating is outside the window"
        );
        assert_eq!(
            l.interactions_14d, 0,
            "a 30-day-old click is outside the window"
        );
        assert!(
            l.last_feedback_at.is_some(),
            "…but the all-time last rating is reported"
        );
        assert!(
            l.last_interaction_at.is_some(),
            "…and so is the all-time last click"
        );
    }

    #[test]
    fn passive_scroll_and_ignore_do_not_count_as_interactions() {
        let db = test_db();
        let conn = db.conn.lock();
        ensure_interactions(&conn);
        for kind in ["scroll", "ignore", "scroll"] {
            conn.execute(
                "INSERT INTO interactions (item_id, action_type) VALUES (1, ?1)",
                [kind],
            )
            .unwrap();
        }
        let l = compute_feedback_liveness(&conn).unwrap();
        assert_eq!(l.interactions_14d, 0);
        assert_eq!(l.last_interaction_at, None);

        conn.execute(
            "INSERT INTO interactions (item_id, action_type) VALUES (1, 'click')",
            [],
        )
        .unwrap();
        let l = compute_feedback_liveness(&conn).unwrap();
        assert_eq!(l.interactions_14d, 1);
        assert!(l.last_interaction_at.is_some());
    }

    #[test]
    fn a_fresh_rating_and_click_inside_the_window_count() {
        let db = test_db();
        let id = crate::test_utils::insert_test_item(&db, "hackernews", "hn_2", "t", "c");
        let conn = db.conn.lock();
        ensure_interactions(&conn);
        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 0)",
            [id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO interactions (item_id, action_type) VALUES (?1, 'engagement_complete')",
            [id],
        )
        .unwrap();
        let l = compute_feedback_liveness(&conn).unwrap();
        assert_eq!(l.feedback_14d, 1);
        assert_eq!(l.interactions_14d, 1);
    }
}
