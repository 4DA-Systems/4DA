//! Tauri commands for the Decision Advantage module.
//!
//! Exposes window lifecycle and compound score to the frontend.

use crate::decision_advantage::{CompoundAdvantageScore, DecisionWindow};
use crate::error::{FourDaError, Result};
use crate::open_db_connection;

/// Get all open decision windows, ordered by urgency.
#[tauri::command]
pub async fn get_decision_windows() -> Result<Vec<DecisionWindow>> {
    let conn = open_db_connection().map_err(FourDaError::Internal)?;
    Ok(crate::decision_advantage::get_open_windows(&conn))
}

/// Mark a decision window as acted upon with an optional outcome note.
#[tauri::command]
pub async fn act_on_decision_window(window_id: i64, outcome: Option<String>) -> Result<()> {
    let conn = open_db_connection().map_err(FourDaError::Internal)?;
    crate::decision_advantage::transition_window(&conn, window_id, "acted", outcome.as_deref())
        .map_err(FourDaError::Internal)
}

/// Dismiss/close a decision window without acting on it.
#[tauri::command]
pub async fn close_decision_window(window_id: i64) -> Result<()> {
    let conn = open_db_connection().map_err(FourDaError::Internal)?;
    crate::decision_advantage::transition_window(&conn, window_id, "closed", None)
        .map_err(FourDaError::Internal)
}

/// Get the compound advantage score for the weekly period.
#[tauri::command]
pub async fn get_compound_advantage() -> Result<CompoundAdvantageScore> {
    let conn = open_db_connection().map_err(FourDaError::Internal)?;
    Ok(crate::decision_advantage::compute_compound_score(
        &conn, "weekly",
    ))
}

/// Trigger detection of new decision windows and expiration of stale ones.
/// Returns the newly detected windows.
#[tauri::command]
pub async fn detect_windows() -> Result<Vec<DecisionWindow>> {
    let conn = open_db_connection().map_err(FourDaError::Internal)?;

    // Expire stale windows first
    crate::decision_advantage::expire_stale_windows(&conn);

    // Detect new windows
    Ok(crate::decision_advantage::detect_decision_windows(&conn))
}
