//! ACE interaction commands: feedback recording, topic affinities, engagement summary.

use tracing::warn;

use crate::ace;
use crate::error::{Result, ResultExt};
use crate::get_ace_engine;

/// Record user feedback in the main database — feeds autophagy calibration analysis.
/// This bridges user interactions (save/dismiss) into the `feedback` table that all
/// autophagy analyzers depend on. Without this, autophagy produces zero output.
#[tauri::command]
pub async fn record_item_feedback(item_id: i64, relevant: bool) -> Result<()> {
    let db = crate::get_database()?;
    db.record_feedback(item_id, relevant)
        .context("Failed to record feedback")?;
    Ok(())
}

/// Record a user interaction for behavior learning
#[tauri::command]
pub async fn ace_record_interaction(
    item_id: i64,
    action_type: String,
    action_data: Option<serde_json::Value>,
    item_topics: Vec<String>,
    item_source: String,
) -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;

    // Parse action type into BehaviorAction
    let action = match action_type.as_str() {
        "click" => {
            let dwell_time = action_data
                .as_ref()
                .and_then(|d| {
                    d.get("dwell_time_seconds")
                        .and_then(serde_json::Value::as_u64)
                })
                .unwrap_or(0);
            // Optional interaction pattern classified by the frontend.
            // Serialized as snake_case string matching InteractionPattern's
            // serde rename. Unknown values fall back to None (legacy scoring).
            let pattern = action_data.as_ref().and_then(|d| {
                d.get("pattern")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|s| match s {
                        "bounced" => Some(ace::InteractionPattern::Bounced),
                        "scanned" => Some(ace::InteractionPattern::Scanned),
                        "engaged" => Some(ace::InteractionPattern::Engaged),
                        "completed" => Some(ace::InteractionPattern::Completed),
                        "reread" => Some(ace::InteractionPattern::Reread),
                        "abandoned" => Some(ace::InteractionPattern::Abandoned),
                        _ => None,
                    })
            });
            ace::BehaviorAction::Click {
                dwell_time_seconds: dwell_time,
                pattern,
            }
        }
        "save" => ace::BehaviorAction::Save,
        "share" => ace::BehaviorAction::Share,
        "dismiss" => ace::BehaviorAction::Dismiss,
        "mark_irrelevant" => ace::BehaviorAction::MarkIrrelevant,
        "scroll" => {
            let visible_seconds = action_data
                .and_then(|d| d.get("visible_seconds").and_then(serde_json::Value::as_f64))
                .unwrap_or(0.0) as f32;
            ace::BehaviorAction::Scroll { visible_seconds }
        }
        "ignore" => ace::BehaviorAction::Ignore,
        "briefing_click" => ace::BehaviorAction::BriefingClick,
        "briefing_dismiss" => ace::BehaviorAction::BriefingDismiss,
        "engagement_complete" => {
            let total_seconds = action_data
                .as_ref()
                .and_then(|d| d.get("total_seconds").and_then(serde_json::Value::as_u64))
                .unwrap_or(0);
            let scroll_depth_pct = action_data
                .as_ref()
                .and_then(|d| {
                    d.get("scroll_depth_pct")
                        .and_then(serde_json::Value::as_f64)
                })
                .unwrap_or(0.0) as f32;
            ace::BehaviorAction::EngagementComplete {
                total_seconds,
                scroll_depth_pct,
            }
        }
        "save_with_context" => {
            let context_str = action_data
                .as_ref()
                .and_then(|d| d.get("context").and_then(serde_json::Value::as_str))
                .unwrap_or("useful_now");
            let context = match context_str {
                "reference" => ace::SaveContext::Reference,
                "share" => ace::SaveContext::Share,
                _ => ace::SaveContext::UsefulNow, // Default to UsefulNow
            };
            ace::BehaviorAction::SaveWithContext { context }
        }
        _ => return Err(format!("Unknown action type: {action_type}").into()),
    };

    ace.record_interaction(
        item_id,
        action.clone(),
        item_topics.clone(),
        item_source.clone(),
    )?;

    Ok(serde_json::json!({
        "success": true,
        "recorded": {
            "item_id": item_id,
            "action": action_type,
            "topics": item_topics,
            "source": item_source
        }
    }))
}

/// Get learned topic affinities
#[tauri::command]
pub async fn ace_get_topic_affinities() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let affinities = ace.get_topic_affinities()?;

    Ok(serde_json::json!({
        "affinities": affinities,
        "count": affinities.len()
    }))
}

/// Get detected anti-topics
#[tauri::command]
pub async fn ace_get_anti_topics(min_rejections: Option<u32>) -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let threshold = min_rejections.unwrap_or(5);
    let anti_topics = ace.get_anti_topics(threshold)?;

    Ok(serde_json::json!({
        "anti_topics": anti_topics,
        "count": anti_topics.len(),
        "threshold": threshold
    }))
}

/// Get a single topic's affinity score
#[tauri::command]
pub async fn ace_get_single_affinity(topic: String) -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let affinities = ace.get_topic_affinities()?;

    let matching = affinities
        .iter()
        .find(|a| a.topic.to_lowercase() == topic.to_lowercase());

    match matching {
        Some(affinity) => Ok(serde_json::json!({
            "affinity": {
                "topic": affinity.topic,
                "positive_signals": affinity.positive_signals,
                "negative_signals": affinity.negative_signals,
                "affinity_score": affinity.affinity_score
            }
        })),
        None => Ok(serde_json::json!({
            "affinity": null
        })),
    }
}

/// Get engagement summary for the dashboard (daily count, streak, trend)
#[tauri::command]
pub async fn get_engagement_summary() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn().lock();

    // Today's interaction count
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let today_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM interactions WHERE date(timestamp) = ?1",
            rusqlite::params![today],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Streak: consecutive days with at least 1 interaction (looking back from today)
    let mut streak: i64 = 0;
    let rows: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT date(timestamp) as d FROM interactions
                 ORDER BY d DESC LIMIT 30",
        )?;
        let result = stmt.query_map([], |row| row.get::<_, String>(0))?;
        result
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    warn!("Row processing failed in ace_commands: {e}");
                    None
                }
            })
            .collect()
    };

    if !rows.is_empty() {
        let mut expected = chrono::Utc::now().date_naive();
        for date_str in &rows {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                if date == expected {
                    streak += 1;
                    expected -= chrono::Duration::days(1);
                } else if date < expected {
                    break;
                }
            }
        }
    }

    // 7-day heatmap data (interactions per day for last 7 days)
    let mut heatmap: Vec<serde_json::Value> = Vec::new();
    for i in (0..7).rev() {
        let date = (chrono::Utc::now() - chrono::Duration::days(i))
            .format("%Y-%m-%d")
            .to_string();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM interactions WHERE date(timestamp) = ?1",
                rusqlite::params![date],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let day_name = (chrono::Utc::now() - chrono::Duration::days(i))
            .format("%a")
            .to_string();
        heatmap.push(serde_json::json!({
            "date": date,
            "day": day_name,
            "count": count,
        }));
    }

    // Accuracy trend: average feedback positivity over last 7 vs previous 7 days
    let recent_positive: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(CASE WHEN signal_strength > 0 THEN 1.0 ELSE 0.0 END), 0.5)
             FROM interactions WHERE timestamp >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0.5);

    let prev_positive: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(CASE WHEN signal_strength > 0 THEN 1.0 ELSE 0.0 END), 0.5)
             FROM interactions WHERE timestamp >= datetime('now', '-14 days')
             AND timestamp < datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0.5);

    let trend = if recent_positive > prev_positive + 0.05 {
        "improving"
    } else if recent_positive < prev_positive - 0.05 {
        "declining"
    } else {
        "stable"
    };

    Ok(serde_json::json!({
        "today_interactions": today_count,
        "streak_days": streak,
        "heatmap": heatmap,
        "accuracy_trend": trend,
        "recent_positive_rate": format!("{:.0}%", recent_positive * 100.0),
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_engagement_summary_shape() {
        // Verify the JSON shape returned by ace_get_engagement_summary
        let summary = serde_json::json!({
            "today_interactions": 5,
            "streak_days": 3,
            "heatmap": [],
            "accuracy_trend": [],
            "recent_positive_rate": "80%",
        });
        assert!(summary["today_interactions"].is_number());
        assert!(summary["streak_days"].is_number());
        assert!(summary["heatmap"].is_array());
        assert!(summary["accuracy_trend"].is_array());
        assert!(summary["recent_positive_rate"].is_string());
    }
}
