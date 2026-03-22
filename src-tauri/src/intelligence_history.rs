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

/// Get the diff between the two most recent intelligence snapshots.
/// Returns new items/relevant counts and time elapsed since last session.
#[tauri::command]
pub async fn get_session_diff() -> Result<SessionDiff> {
    let conn = crate::open_db_connection()?;

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

    Ok(SessionDiff {
        new_items: current.0 - previous.0,
        new_relevant: current.1 - previous.1,
        hours_since_last: hours,
        has_previous: true,
    })
}
