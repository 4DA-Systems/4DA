// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Intelligence History — tracks how the system's accuracy evolves over time.
//!
//! Records snapshots of accuracy, topics learned, items analyzed, and relevant items found.
//! Powers the intelligence growth trajectory visualization.

use serde::Serialize;
use ts_rs::TS;

use crate::error::{FourDaError, Result};

/// Diff between the two most recent intelligence snapshots.
/// Powers the "what changed since last session" display.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct SessionDiff {
    pub new_items: i64,
    pub new_relevant: i64,
    pub hours_since_last: f64,
    pub has_previous: bool,
}

/// A single point in the intelligence growth trajectory.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct IntelligenceSnapshot {
    pub recorded_at: String,
    pub accuracy: f64,
    pub topics_learned: i64,
    pub items_analyzed: i64,
    pub relevant_found: i64,
}

/// Intelligence growth data returned to the frontend.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct IntelligenceGrowth {
    pub snapshots: Vec<IntelligenceSnapshot>,
    pub current_accuracy: f64,
    pub total_topics: i64,
    pub total_analyzed: i64,
    pub total_relevant: i64,
}

/// Record a snapshot of intelligence metrics after analysis completes.
/// Called automatically after each successful analysis run.
pub fn record_intelligence_snapshot(
    conn: &rusqlite::Connection,
    accuracy: f64,
    topics_learned: i64,
    items_analyzed: i64,
    relevant_found: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO intelligence_history (accuracy, topics_learned, items_analyzed, relevant_found)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![accuracy, topics_learned, items_analyzed, relevant_found],
    )
    .map_err(FourDaError::Db)?;
    Ok(())
}

/// Get the intelligence growth trajectory from recorded history.
#[tauri::command]
pub async fn get_intelligence_growth() -> Result<IntelligenceGrowth> {
    let conn = crate::open_db_connection()?;

    let mut stmt = conn
        .prepare(
            "SELECT recorded_at, accuracy, topics_learned, items_analyzed, relevant_found
             FROM intelligence_history
             ORDER BY recorded_at ASC",
        )
        .map_err(FourDaError::Db)?;

    let snapshots: Vec<IntelligenceSnapshot> = stmt
        .query_map([], |row| {
            Ok(IntelligenceSnapshot {
                recorded_at: row.get(0)?,
                accuracy: row.get(1)?,
                topics_learned: row.get(2)?,
                items_analyzed: row.get(3)?,
                relevant_found: row.get(4)?,
            })
        })
        .map_err(FourDaError::Db)?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in intelligence_history: {e}");
                None
            }
        })
        .collect();

    let (current_accuracy, total_topics, total_analyzed, total_relevant) =
        if let Some(last) = snapshots.last() {
            (
                last.accuracy,
                last.topics_learned,
                last.items_analyzed,
                last.relevant_found,
            )
        } else {
            (0.0, 0, 0, 0)
        };

    Ok(IntelligenceGrowth {
        snapshots,
        current_accuracy,
        total_topics,
        total_analyzed,
        total_relevant,
    })
}

/// Count the genuine FLOW of items since `since`: how many arrived, and how many
/// of those are relevant at the current threshold.
///
/// This exists because the caller used to SUBTRACT two `intelligence_history`
/// rows. Those rows are PER-RUN counts — `analysis_status.rs` records
/// `results.len()` and `relevant_count` for the analysis run that just finished,
/// not a running total — so differencing them measured "how much bigger was this
/// batch than the previous one", a quantity that is routinely negative and was
/// rendered to the user verbatim as "N newly relevant". Observed live:
/// `relevant_found` 79 -> 7 produced `{"new_items":176,"new_relevant":-72}` on
/// the Brief tab.
///
/// `intelligence_history.recorded_at` and `source_items.created_at` are both
/// written by SQLite's `datetime('now')` (UTC, `YYYY-MM-DD HH:MM:SS`) — neither
/// is ever supplied by the caller — so a lexicographic `>` on the text column is
/// a correct chronological comparison.
///
/// Documented limit: "became relevant" here means "arrived since `since` and is
/// relevant now". `source_items` carries no `scored_at` column (only
/// `scored_pipeline_version`), so an item that was already in the corpus and
/// only crossed the threshold on this run cannot be distinguished from one that
/// was always above it. That undercounts rather than inventing a number, and —
/// unlike the stock difference it replaces — both components are COUNTs, so both
/// are non-negative by construction.
fn count_new_since(
    conn: &rusqlite::Connection,
    since: &str,
    relevance_threshold: f64,
) -> Result<(i64, i64)> {
    conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN relevance_score >= ?2 THEN 1 ELSE 0 END), 0)
         FROM source_items
         WHERE created_at > ?1",
        rusqlite::params![since, relevance_threshold],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .map_err(FourDaError::Db)
}

/// Get the diff between the two most recent intelligence snapshots.
/// Returns new items/relevant counts and time elapsed since last session.
#[tauri::command]
pub async fn get_session_diff() -> Result<SessionDiff> {
    let conn = crate::open_db_connection()?;
    compute_session_diff(&conn, crate::get_relevance_threshold() as f64)
}

/// Testable core of [`get_session_diff`] — takes the connection and threshold
/// explicitly so the flow arithmetic can be exercised without global state.
fn compute_session_diff(
    conn: &rusqlite::Connection,
    relevance_threshold: f64,
) -> Result<SessionDiff> {
    let mut stmt = conn
        .prepare(
            "SELECT items_analyzed, relevant_found, recorded_at
             FROM intelligence_history
             ORDER BY id DESC
             LIMIT 2",
        )
        .map_err(FourDaError::Db)?;

    let snapshots: Vec<(i64, i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(FourDaError::Db)?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in session_diff: {e}");
                None
            }
        })
        .collect();

    if snapshots.len() < 2 {
        // No previous session to measure a flow against. Report the only run's
        // own counts (both are COUNTs, so both are non-negative); the frontend
        // ignores them when `has_previous` is false.
        return Ok(SessionDiff {
            new_items: snapshots.first().map_or(0, |s| s.0),
            new_relevant: snapshots.first().map_or(0, |s| s.1),
            hours_since_last: 0.0,
            has_previous: false,
        });
    }

    let current = &snapshots[0];
    let previous = &snapshots[1];

    let hours = chrono::NaiveDateTime::parse_from_str(&current.2, "%Y-%m-%d %H:%M:%S")
        .ok()
        .and_then(|curr| {
            chrono::NaiveDateTime::parse_from_str(&previous.2, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|prev| (curr - prev).num_minutes() as f64 / 60.0)
        })
        .unwrap_or(0.0);

    // FLOW, not a difference of stocks: count what actually arrived since the
    // previous snapshot. `current` is still used for the elapsed-time reading.
    let (new_items, new_relevant) = count_new_since(conn, &previous.2, relevance_threshold)
        .unwrap_or_else(|e| {
            tracing::warn!(
                target: "4da::intelligence",
                error = %e,
                "Session-diff flow query failed — reporting zero rather than a stock difference"
            );
            (0, 0)
        });

    Ok(SessionDiff {
        new_items,
        new_relevant,
        hours_since_last: hours,
        has_previous: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal schema: only the columns the session-diff path reads.
    fn test_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE intelligence_history (
                 id INTEGER PRIMARY KEY,
                 recorded_at TEXT NOT NULL,
                 accuracy REAL NOT NULL,
                 topics_learned INTEGER NOT NULL,
                 items_analyzed INTEGER NOT NULL,
                 relevant_found INTEGER NOT NULL
             );
             CREATE TABLE source_items (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 created_at TEXT NOT NULL,
                 relevance_score REAL DEFAULT NULL
             );",
        )
        .expect("create tables");
        conn
    }

    fn snapshot(conn: &rusqlite::Connection, recorded_at: &str, analyzed: i64, relevant: i64) {
        conn.execute(
            "INSERT INTO intelligence_history
                 (recorded_at, accuracy, topics_learned, items_analyzed, relevant_found)
             VALUES (?1, 0.5, 0, ?2, ?3)",
            rusqlite::params![recorded_at, analyzed, relevant],
        )
        .expect("insert snapshot");
    }

    fn item(conn: &rusqlite::Connection, created_at: &str, score: Option<f64>) {
        conn.execute(
            "INSERT INTO source_items (created_at, relevance_score) VALUES (?1, ?2)",
            rusqlite::params![created_at, score],
        )
        .expect("insert item");
    }

    /// Regression for the live Brief-tab defect: two snapshots whose per-run
    /// `relevant_found` SHRANK (79 -> 7) produced `new_relevant: -72`, which the
    /// frontend rendered verbatim as "-72 newly relevant".
    #[test]
    fn session_diff_is_never_negative_when_relevance_shrinks() {
        let conn = test_conn();
        // The exact live shape: the later run analysed a smaller batch and found
        // far fewer relevant items than the earlier one.
        snapshot(&conn, "2026-08-14 08:00:00", 900, 79);
        snapshot(&conn, "2026-08-15 08:00:00", 120, 7);

        // Three items arrived after the previous snapshot; two are relevant.
        item(&conn, "2026-08-14 09:00:00", Some(0.80));
        item(&conn, "2026-08-14 10:00:00", Some(0.61));
        item(&conn, "2026-08-14 11:00:00", Some(0.05));
        // …and one arrived BEFORE it, so it is not part of the flow.
        item(&conn, "2026-08-13 23:00:00", Some(0.95));

        let diff = compute_session_diff(&conn, 0.50).expect("diff");

        assert!(diff.has_previous);
        assert!(
            diff.new_items >= 0 && diff.new_relevant >= 0,
            "session diff must never report a negative count, got {diff:?}"
        );
        assert_eq!(diff.new_items, 3, "flow counts arrivals since last session");
        assert_eq!(
            diff.new_relevant, 2,
            "flow counts arrivals that are relevant now"
        );
    }

    /// The stock difference also mis-reported `new_items` whenever a run
    /// analysed fewer items than its predecessor — the same bug, one field over.
    #[test]
    fn session_diff_new_items_is_a_flow_not_a_batch_size_difference() {
        let conn = test_conn();
        snapshot(&conn, "2026-08-14 08:00:00", 1000, 50);
        snapshot(&conn, "2026-08-15 08:00:00", 10, 1);
        item(&conn, "2026-08-14 09:00:00", None);

        let diff = compute_session_diff(&conn, 0.50).expect("diff");

        assert_eq!(
            diff.new_items, 1,
            "one item arrived since the previous snapshot"
        );
        assert_eq!(diff.new_relevant, 0, "an unscored item is not relevant");
    }

    /// A single snapshot has no previous session to measure against; the values
    /// are the run's own counts and must still be non-negative.
    #[test]
    fn session_diff_without_previous_snapshot() {
        let conn = test_conn();
        snapshot(&conn, "2026-08-15 08:00:00", 42, 7);

        let diff = compute_session_diff(&conn, 0.50).expect("diff");

        assert!(!diff.has_previous);
        assert_eq!(diff.new_items, 42);
        assert_eq!(diff.new_relevant, 7);
    }
}
