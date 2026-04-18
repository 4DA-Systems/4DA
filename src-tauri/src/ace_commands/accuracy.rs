// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! ACE accuracy metrics and calibration feedback commands.

use crate::error::{Result, ResultExt};

/// Get accuracy metrics calculated from interactions
#[tauri::command]
pub async fn ace_get_accuracy_metrics() -> Result<serde_json::Value> {
    let ace = crate::get_ace_engine()?;
    let conn = ace.get_conn().lock();

    // Calculate from interactions table
    // The ACE schema uses action_type (not interaction_type)
    let total_interactions: i64 = conn
        .query_row("SELECT COUNT(*) FROM interactions", [], |row| row.get(0))
        .unwrap_or(0);

    let positive_interactions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM interactions WHERE action_type IN ('click', 'save', 'share')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let negative_interactions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM interactions WHERE action_type IN ('dismiss', 'mark_irrelevant')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let engagement_rate = if total_interactions > 0 {
        positive_interactions as f64 / total_interactions as f64
    } else {
        0.0
    };

    let precision = if (positive_interactions + negative_interactions) > 0 {
        positive_interactions as f64 / (positive_interactions + negative_interactions) as f64
    } else {
        0.0
    };

    // Calculate calibration error from accuracy feedback entries
    let calibration_error: f64 = conn
        .query_row(
            "SELECT AVG(json_extract(action_data, '$.calibration_error')) FROM interactions WHERE action_type = 'accuracy_feedback' AND action_data IS NOT NULL",
            [],
            |row| row.get::<_, Option<f64>>(0),
        )
        .unwrap_or(None)
        .unwrap_or(0.0);

    Ok(serde_json::json!({
        "precision": precision,
        "engagement_rate": engagement_rate,
        "calibration_error": calibration_error
    }))
}

/// Record accuracy feedback for a scored item (predicted vs actual relevance)
#[tauri::command]
pub async fn ace_record_accuracy_feedback(
    item_id: u64,
    predicted_score: f64,
    feedback_type: String,
) -> Result<()> {
    let ace = crate::get_ace_engine()?;
    let conn = ace.get_conn().lock();

    // Map feedback to actual relevance score
    let actual_score: f64 = match feedback_type.as_str() {
        "save" => 1.0,
        "click" => 0.7,
        "dismiss" => 0.2,
        "thumbs_down" => 0.0,
        _ => 0.5,
    };

    let action_data = serde_json::json!({
        "predicted_score": predicted_score,
        "actual_score": actual_score,
        "calibration_error": (predicted_score - actual_score).abs(),
    });

    conn.execute(
        "INSERT INTO interactions (item_id, action_type, action_data, signal_strength) VALUES (?1, 'accuracy_feedback', ?2, ?3)",
        rusqlite::params![item_id as i64, action_data.to_string(), actual_score],
    )
    .context("Failed to record accuracy feedback")?;

    Ok(())
}
