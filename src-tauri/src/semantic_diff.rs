#![allow(dead_code)]
//! Semantic Diff Engine for 4DA
//!
//! Tracks how the conversation around topics shifts over time.
//! Instead of "new post about X," surfaces "the narrative about X shifted from A to B."

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticShift {
    pub topic: String,
    pub drift_magnitude: f32,
    pub direction: String,
    pub representative_items: Vec<i64>,
    pub period: String,
    pub detected_at: String,
}

// ============================================================================
// Implementation
// ============================================================================

/// Record a topic centroid snapshot after scoring
pub fn record_topic_centroid(
    conn: &rusqlite::Connection,
    topic: &str,
    item_count: u32,
    avg_score: f32,
    top_titles: &[String],
) -> Result<(), String> {
    let data = serde_json::json!({
        "item_count": item_count,
        "avg_score": avg_score,
        "top_titles": top_titles,
    });

    crate::temporal::record_event(
        conn,
        "topic_centroid",
        topic,
        &data,
        None,
        Some(&(chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339()),
    )?;

    debug!(target: "4da::semantic_diff", topic, items = item_count, "Recorded topic centroid");
    Ok(())
}

/// Detect semantic shifts by comparing current topic centroids with historical ones
pub fn detect_shifts(
    conn: &rusqlite::Connection,
    lookback_days: u32,
) -> Result<Vec<SemanticShift>, String> {
    let since = format!(
        "{}",
        (chrono::Utc::now() - chrono::Duration::days(lookback_days as i64))
            .format("%Y-%m-%d %H:%M:%S")
    );

    // Get recent centroids (last 24h)
    let recent_events = crate::temporal::query_events(conn, "topic_centroid", None, 100)?;

    // Get older centroids for comparison
    let older_events = crate::temporal::query_events(conn, "topic_centroid", Some(&since), 500)?;

    if recent_events.is_empty() || older_events.is_empty() {
        return Ok(vec![]);
    }

    // Group by topic
    let mut recent_by_topic: std::collections::HashMap<
        String,
        Vec<&crate::temporal::TemporalEvent>,
    > = std::collections::HashMap::new();
    let mut older_by_topic: std::collections::HashMap<
        String,
        Vec<&crate::temporal::TemporalEvent>,
    > = std::collections::HashMap::new();

    // Recent = first 24h worth of events
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
    let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S").to_string();

    for event in &recent_events {
        if event.created_at >= cutoff_str {
            recent_by_topic
                .entry(event.subject.clone())
                .or_default()
                .push(event);
        }
    }

    for event in &older_events {
        if event.created_at < cutoff_str {
            older_by_topic
                .entry(event.subject.clone())
                .or_default()
                .push(event);
        }
    }

    let mut shifts = Vec::new();

    for (topic, recent) in &recent_by_topic {
        if let Some(older) = older_by_topic.get(topic) {
            // Compare title sets to detect narrative shift
            let recent_titles: Vec<String> = recent
                .iter()
                .filter_map(|e| {
                    e.data
                        .get("top_titles")
                        .and_then(|t| t.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                })
                .flatten()
                .collect();

            let older_titles: Vec<String> = older
                .iter()
                .filter_map(|e| {
                    e.data
                        .get("top_titles")
                        .and_then(|t| t.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                })
                .flatten()
                .collect();

            // Compute title overlap as drift proxy
            let overlap = compute_title_overlap(&recent_titles, &older_titles);
            let drift = 1.0 - overlap;

            if drift > 0.5 {
                // Significant drift detected
                let recent_avg: f32 = recent
                    .iter()
                    .filter_map(|e| e.data.get("avg_score").and_then(|v| v.as_f64()))
                    .map(|v| v as f32)
                    .sum::<f32>()
                    / recent.len().max(1) as f32;

                let older_avg: f32 = older
                    .iter()
                    .filter_map(|e| e.data.get("avg_score").and_then(|v| v.as_f64()))
                    .map(|v| v as f32)
                    .sum::<f32>()
                    / older.len().max(1) as f32;

                let direction = if recent_avg > older_avg + 0.05 {
                    format!(
                        "Discussion about {} is gaining relevance with new angles",
                        topic
                    )
                } else if recent_avg < older_avg - 0.05 {
                    format!(
                        "Discussion about {} is shifting to less relevant areas",
                        topic
                    )
                } else {
                    format!("The narrative around {} has changed significantly", topic)
                };

                shifts.push(SemanticShift {
                    topic: topic.clone(),
                    drift_magnitude: drift,
                    direction,
                    representative_items: vec![],
                    period: format!("last {} days", lookback_days),
                    detected_at: chrono::Utc::now().to_rfc3339(),
                });
            }
        }
    }

    shifts.sort_by(|a, b| {
        b.drift_magnitude
            .partial_cmp(&a.drift_magnitude)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    shifts.truncate(10);

    info!(target: "4da::semantic_diff", shifts = shifts.len(), "Semantic shift detection complete");
    Ok(shifts)
}

fn compute_title_overlap(recent: &[String], older: &[String]) -> f32 {
    if recent.is_empty() || older.is_empty() {
        return 0.0;
    }

    // Compute word-level Jaccard similarity between title sets
    let recent_words: std::collections::HashSet<String> = recent
        .iter()
        .flat_map(|t| {
            t.to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .collect();

    let older_words: std::collections::HashSet<String> = older
        .iter()
        .flat_map(|t| {
            t.to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .collect();

    let intersection = recent_words.intersection(&older_words).count();
    let union = recent_words.union(&older_words).count();

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_semantic_shifts(lookback_days: Option<u32>) -> Result<Vec<SemanticShift>, String> {
    crate::settings::require_pro_feature("get_semantic_shifts")?;
    let conn = crate::open_db_connection()?;
    detect_shifts(&conn, lookback_days.unwrap_or(7))
}

#[tauri::command]
pub fn get_topic_centroids(
    topic: Option<String>,
) -> Result<Vec<crate::temporal::TemporalEvent>, String> {
    let conn = crate::open_db_connection()?;
    if let Some(t) = topic {
        crate::temporal::query_events_by_subject(&conn, &t, 20)
    } else {
        crate::temporal::query_events(&conn, "topic_centroid", None, 50)
    }
}
