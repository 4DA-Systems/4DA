//! Tauri commands for the Decision Advantage module.
//!
//! Exposes window lifecycle and compound score to the frontend.

use crate::decision_advantage::{CompoundAdvantageScore, DecisionWindow};
use crate::error::{FourDaError, Result};
use crate::open_db_connection;
use tauri::{AppHandle, Emitter};

/// Get all open decision windows, ordered by urgency.
#[tauri::command]
pub async fn get_decision_windows() -> Result<Vec<DecisionWindow>> {
    let conn = open_db_connection().map_err(FourDaError::Internal)?;
    Ok(crate::decision_advantage::get_open_windows(&conn))
}

/// Mark a decision window as acted upon with an optional outcome note.
#[tauri::command]
pub async fn act_on_decision_window(
    app: AppHandle,
    window_id: i64,
    outcome: Option<String>,
) -> Result<()> {
    let conn = open_db_connection().map_err(FourDaError::Internal)?;
    crate::decision_advantage::transition_window(&conn, window_id, "acted", outcome.as_deref())
        .map_err(FourDaError::Internal)?;

    // GAME: track decisions
    if let Ok(db) = crate::get_database() {
        for a in crate::game_engine::increment_counter(db, "decisions", 1) {
            crate::events::emit_achievement_unlocked(&app, &a);
        }
    }

    Ok(())
}

/// Dismiss/close a decision window without acting on it.
#[tauri::command]
pub async fn close_decision_window(app: AppHandle, window_id: i64) -> Result<()> {
    let conn = open_db_connection().map_err(FourDaError::Internal)?;
    crate::decision_advantage::transition_window(&conn, window_id, "closed", None)
        .map_err(FourDaError::Internal)?;

    // GAME: track decisions (closing is still a decision)
    if let Ok(db) = crate::get_database() {
        for a in crate::game_engine::increment_counter(db, "decisions", 1) {
            crate::events::emit_achievement_unlocked(&app, &a);
        }
    }

    Ok(())
}

/// Get the decision journal: acted and closed windows, ordered most recent first (up to 50).
#[tauri::command]
pub async fn get_decision_journal() -> Result<Vec<DecisionWindow>> {
    let conn = open_db_connection().map_err(FourDaError::Internal)?;
    Ok(crate::decision_advantage::get_decision_journal(&conn))
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::decision_advantage::{CompoundAdvantageScore, DecisionWindow};

    #[test]
    fn test_decision_window_default_construction() {
        let window = DecisionWindow {
            id: 0,
            window_type: "adoption".to_string(),
            title: "Consider Bun runtime".to_string(),
            description: "Bun 1.2 brings Node compat".to_string(),
            urgency: 0.5,
            relevance: 0.7,
            dependency: None,
            status: "open".to_string(),
            opened_at: "2025-06-01 12:00:00".to_string(),
            expires_at: None,
            lead_time_hours: None,
            streets_engine: None,
        };
        assert_eq!(window.window_type, "adoption");
        assert!(window.dependency.is_none());
        assert!(window.expires_at.is_none());
    }

    #[test]
    fn test_compound_advantage_score_zero_state() {
        let score = CompoundAdvantageScore {
            score: 0.0,
            period: "weekly".to_string(),
            items_surfaced: 0,
            avg_lead_time_hours: 0.0,
            windows_opened: 0,
            windows_acted: 0,
            windows_expired: 0,
            knowledge_gaps_closed: 0,
            calibration_accuracy: 0.0,
            trend: 0.0,
        };
        let json = serde_json::to_string(&score).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed["score"], 0.0);
        assert_eq!(parsed["period"], "weekly");
    }
}
