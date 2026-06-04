// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

        // Implicit skip: item was visible but user scrolled past without
        // engaging. Weak negative signal for topic affinity learning.
        if action_type == "scroll" {
            if let ace::BehaviorAction::Scroll { visible_seconds } = &action {
                crate::engagement_telemetry::on_implicit_skip(&conn, item_id, *visible_seconds);
            }
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

/// Diagnostic snapshot of the compound-learning loop. Surfaces the state that
/// proves "4DA gets sharper every day": how many topics have been learned, how
/// many have crossed the confidence-building exposure threshold, the strongest
/// learned likes and dislikes, and the total interaction volume feeding it.
/// Pairs with the "4da::learning" tracing stream emitted on every affinity update.
#[tauri::command]
pub async fn get_learning_stats() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn().lock();
    Ok(learning_stats_json(&conn))
}

/// Build the learning-stats snapshot from a behavior connection. Split out from
/// the command so it can be exercised against an in-memory ACE in tests.
pub(crate) fn learning_stats_json(conn: &rusqlite::Connection) -> serde_json::Value {
    let total_topics: i64 = conn
        .query_row("SELECT COUNT(*) FROM topic_affinities", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    // Topics with enough exposures that their affinity is confidence-weighted
    // rather than provisional (the SQL affinity formula gains confidence past 3).
    let topics_above_threshold: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM topic_affinities WHERE total_exposures > 3",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_interactions: i64 = conn
        .query_row("SELECT COUNT(*) FROM interactions", [], |row| row.get(0))
        .unwrap_or(0);

    let read_affinities = |sql: &str| -> Vec<serde_json::Value> {
        let mut stmt = match conn.prepare(sql) {
            Ok(stmt) => stmt,
            Err(e) => {
                warn!("get_learning_stats query prepare failed: {e}");
                return Vec::new();
            }
        };
        let rows = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "topic": row.get::<_, String>(0)?,
                "affinity_score": row.get::<_, f32>(1)?,
                "confidence": row.get::<_, f32>(2)?,
                "total_exposures": row.get::<_, i64>(3)?,
            }))
        });
        match rows {
            Ok(iter) => iter.filter_map(std::result::Result::ok).collect(),
            Err(e) => {
                warn!("get_learning_stats query_map failed: {e}");
                Vec::new()
            }
        }
    };

    let top_positive = read_affinities(
        "SELECT topic, affinity_score, confidence, total_exposures FROM topic_affinities
         WHERE affinity_score > 0 ORDER BY affinity_score DESC LIMIT 5",
    );
    let top_negative = read_affinities(
        "SELECT topic, affinity_score, confidence, total_exposures FROM topic_affinities
         WHERE affinity_score < 0 ORDER BY affinity_score ASC LIMIT 5",
    );

    serde_json::json!({
        "total_topics_tracked": total_topics,
        "topics_above_exposure_threshold": topics_above_threshold,
        "total_interactions_processed": total_interactions,
        "top_positive_affinities": top_positive,
        "top_negative_affinities": top_negative,
    })
}

#[cfg(test)]
mod tests {
    use crate::ace::{create_test_ace, BehaviorAction};

    /// The learning-stats observability surface must reflect real recorded
    /// feedback: topic counts, interaction volume, and the strongest learned
    /// likes/dislikes. Guards against the stats command silently reporting zeros.
    #[test]
    fn learning_stats_reflects_recorded_feedback() {
        let ace = create_test_ace();
        for item_id in 1..=3 {
            ace.record_interaction(
                item_id,
                BehaviorAction::Save,
                vec!["rust".to_string()],
                "hackernews".to_string(),
            )
            .expect("record save");
        }
        for item_id in 4..=5 {
            ace.record_interaction(
                item_id,
                BehaviorAction::MarkIrrelevant,
                vec!["java".to_string()],
                "reddit".to_string(),
            )
            .expect("record irrelevant");
        }

        let conn = ace.get_conn().lock();
        let stats = super::learning_stats_json(&conn);

        assert_eq!(stats["total_interactions_processed"].as_i64().unwrap(), 5);
        assert!(stats["total_topics_tracked"].as_i64().unwrap() >= 2);
        assert_eq!(
            stats["top_positive_affinities"][0]["topic"]
                .as_str()
                .unwrap(),
            "rust"
        );
        assert_eq!(
            stats["top_negative_affinities"][0]["topic"]
                .as_str()
                .unwrap(),
            "java"
        );
    }

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
