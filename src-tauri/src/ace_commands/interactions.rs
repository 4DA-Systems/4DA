// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! ACE interaction commands: feedback recording and engagement summary.

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

    // Feed stability detector — explicit feedback is the strongest signal
    if let Ok(conn) = crate::open_db_connection() {
        // Look up the item's topics from source_items
        let topics: Vec<String> = conn
            .prepare("SELECT si.title FROM source_items si WHERE si.id = ?1")
            .ok()
            .and_then(|mut stmt| {
                stmt.query_row(rusqlite::params![item_id], |row| row.get::<_, String>(0))
                    .ok()
            })
            .map(|title| crate::extract_topics(&title, "", &[]))
            .unwrap_or_default();

        for topic in &topics {
            let class = if relevant {
                crate::stability_detector::FacetClass::Interest
            } else {
                crate::stability_detector::FacetClass::Veto
            };
            let value = if relevant { "confirmed" } else { "rejected" };
            crate::stability_detector::record_evidence(
                &conn,
                class,
                topic,
                value,
                crate::stability_detector::CueFamily::Explicit,
                "feedback",
                1.0,
            );
        }
    }

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

    // Frontend call sites pass `actionData: JSON.stringify({...})` — a JSON
    // STRING, which deserializes to Value::String, so every `.get(...)` below
    // silently returned None. Live impact (2026-07-13 audit): all 819 scroll
    // interactions persisted visible_seconds 0.0 → signal_strength 0 — the
    // feed's dominant POSITIVE signal was discarded wholesale while passive
    // ignores kept their full negative weight, driving stack-topic affinities
    // hard negative. Accept both shapes.
    let action_data = action_data.map(|v| match v {
        serde_json::Value::String(s) => {
            serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s))
        }
        other => other,
    });

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

    ace.record_interaction(item_id, action, item_topics.clone(), item_source.clone())?;

    // Feed stability detector with engagement evidence
    if let Ok(conn) = crate::open_db_connection() {
        let (cue, etype, conf) = match action_type.as_str() {
            "save" | "save_with_context" => (
                crate::stability_detector::CueFamily::Structural,
                "bookmark",
                0.9,
            ),
            "dismiss" | "mark_irrelevant" => (
                crate::stability_detector::CueFamily::Behavioral,
                "dismiss",
                0.7,
            ),
            "click" | "briefing_click" => (
                crate::stability_detector::CueFamily::Behavioral,
                "engagement",
                0.6,
            ),
            "engagement_complete" => (
                crate::stability_detector::CueFamily::Behavioral,
                "dwell_time",
                0.8,
            ),
            _ => (
                crate::stability_detector::CueFamily::Recurrence,
                "interaction",
                0.5,
            ),
        };

        let is_negative = matches!(action_type.as_str(), "dismiss" | "mark_irrelevant");

        for topic in &item_topics {
            let class = if is_negative {
                crate::stability_detector::FacetClass::Veto
            } else {
                crate::stability_detector::FacetClass::Interest
            };
            let value = if is_negative { "rejected" } else { "engaged" };
            crate::stability_detector::record_evidence(
                &conn, class, topic, value, cue, etype, conf,
            );
        }

        // Source preference signal
        if !item_source.is_empty() {
            let value = if is_negative { "low" } else { "high" };
            crate::stability_detector::record_evidence(
                &conn,
                crate::stability_detector::FacetClass::SourcePref,
                &item_source,
                value,
                cue,
                etype,
                conf * 0.5, // halved confidence for source-level signal
            );
        }
    }

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

    // Accuracy trend: average feedback positivity over last 7 vs previous 7 days.
    // AVG over zero rows is SQL NULL -> Option::None, so a first-week user with no
    // feedback gets an honest null/"none" instead of a fabricated 50% / "stable".
    let recent_positive: Option<f64> = conn
        .query_row(
            "SELECT AVG(CASE WHEN signal_strength > 0 THEN 1.0 ELSE 0.0 END)
             FROM interactions WHERE timestamp >= datetime('now', '-7 days')",
            [],
            |row| row.get::<_, Option<f64>>(0),
        )
        .ok()
        .flatten();

    let prev_positive: Option<f64> = conn
        .query_row(
            "SELECT AVG(CASE WHEN signal_strength > 0 THEN 1.0 ELSE 0.0 END)
             FROM interactions WHERE timestamp >= datetime('now', '-14 days')
             AND timestamp < datetime('now', '-7 days')",
            [],
            |row| row.get::<_, Option<f64>>(0),
        )
        .ok()
        .flatten();

    // A trend is only real when both windows have data to compare.
    let trend = match (recent_positive, prev_positive) {
        (Some(r), Some(p)) if r > p + 0.05 => "improving",
        (Some(r), Some(p)) if r < p - 0.05 => "declining",
        (Some(_), Some(_)) => "stable",
        _ => "none",
    };

    Ok(serde_json::json!({
        "today_interactions": today_count,
        "streak_days": streak,
        "heatmap": heatmap,
        "accuracy_trend": trend,
        "recent_positive_rate": recent_positive.map(|r| format!("{:.0}%", r * 100.0)),
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_engagement_summary_shape() {
        // Populated shape: a user with recent feedback.
        let summary = serde_json::json!({
            "today_interactions": 5,
            "streak_days": 3,
            "heatmap": [],
            "accuracy_trend": "improving",
            "recent_positive_rate": "80%",
        });
        assert!(summary["today_interactions"].is_number());
        assert!(summary["streak_days"].is_number());
        assert!(summary["heatmap"].is_array());
        assert!(summary["accuracy_trend"].is_string());
        assert!(summary["recent_positive_rate"].is_string());

        // No-data shape (first-week user): rate is null and trend is "none" — never a
        // fabricated 50% / "stable".
        let cold = serde_json::json!({
            "accuracy_trend": "none",
            "recent_positive_rate": serde_json::Value::Null,
        });
        assert!(cold["recent_positive_rate"].is_null());
        assert_eq!(cold["accuracy_trend"], "none");
    }
}
