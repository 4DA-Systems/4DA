//! Decision Advantage Tauri commands.
//!
//! Expose decision windows and compound advantage scoring to the frontend.
//! Core logic lives in `decision_advantage::windows` and `decision_advantage::compound_score`.

use crate::decision_advantage::{CompoundAdvantageScore, DecisionWindow};
use crate::error::Result;
use crate::open_db_connection;

/// Get all open decision windows, ordered by urgency descending.
#[tauri::command]
pub async fn get_decision_windows() -> Result<Vec<DecisionWindow>> {
    let conn = open_db_connection()?;
    Ok(crate::decision_advantage::get_open_windows(&conn))
}

/// Record an action on a decision window (transition to "acted" status).
#[tauri::command]
pub async fn act_on_decision_window(
    window_id: i64,
    outcome: Option<String>,
) -> Result<String> {
    let conn = open_db_connection()?;
    crate::decision_advantage::windows::transition_window(
        &conn,
        window_id,
        "acted",
        outcome.as_deref(),
    )?;
    Ok(format!("Window {} marked as acted", window_id))
}

/// Close/dismiss a decision window without acting on it.
#[tauri::command]
pub async fn close_decision_window(
    window_id: i64,
    outcome: Option<String>,
) -> Result<String> {
    let conn = open_db_connection()?;
    crate::decision_advantage::windows::transition_window(
        &conn,
        window_id,
        "closed",
        outcome.as_deref(),
    )?;
    Ok(format!("Window {} closed", window_id))
}

/// Calculate compound advantage score for a given period (daily, weekly, monthly).
#[tauri::command]
pub async fn get_compound_advantage(
    period: Option<String>,
) -> Result<CompoundAdvantageScore> {
    let conn = open_db_connection()?;
    let p = period.as_deref().unwrap_or("weekly");
    Ok(crate::decision_advantage::compute_compound_score(&conn, p))
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
