//! Intelligence History — tracks how the system's accuracy evolves over time.
//!
//! Records snapshots of accuracy, topics learned, items analyzed, and relevant items found.
//! Powers the intelligence growth trajectory visualization.

use serde::Serialize;
use ts_rs::TS;

use crate::error::{FourDaError, Result};

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
