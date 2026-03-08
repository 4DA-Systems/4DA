//! Semantic Diff Engine for 4DA
//!
//! Tracks how the conversation around topics shifts over time.
//! Instead of "new post about X," surfaces "the narrative about X shifted from A to B."

use crate::error::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

// ============================================================================
// Types
// ============================================================================

#[allow(dead_code)] // Used by detect_shifts (reserved for MCP integration)
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
) -> Result<()> {
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
#[allow(dead_code)] // Reserved for MCP integration
pub fn detect_shifts(
    conn: &rusqlite::Connection,
    lookback_days: u32,
) -> Result<Vec<SemanticShift>> {
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

#[allow(dead_code)] // Used by detect_shifts
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
pub fn get_semantic_shifts(lookback_days: Option<u32>) -> Result<Vec<SemanticShift>> {
    crate::settings::require_pro_feature("get_semantic_shifts")?;
    let conn = crate::open_db_connection()?;
    detect_shifts(&conn, lookback_days.unwrap_or(7))
}

#[allow(dead_code)] // Reserved for MCP integration
pub fn get_topic_centroids(topic: Option<String>) -> Result<Vec<crate::temporal::TemporalEvent>> {
    let conn = crate::open_db_connection()?;
    if let Some(t) = topic {
        crate::temporal::query_events_by_subject(&conn, &t, 20).map_err(|e| e.into())
    } else {
        crate::temporal::query_events(&conn, "topic_centroid", None, 50).map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_shift_serde_roundtrip() {
        let shift = SemanticShift {
            topic: "react".to_string(),
            drift_magnitude: 0.72,
            direction: "Discussion about react is gaining relevance".to_string(),
            representative_items: vec![1, 2, 3],
            period: "last 7 days".to_string(),
            detected_at: "2026-02-28T10:00:00+00:00".to_string(),
        };
        let json = serde_json::to_string(&shift).unwrap();
        let deserialized: SemanticShift = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.topic, "react");
        assert!((deserialized.drift_magnitude - 0.72).abs() < 0.001);
        assert_eq!(deserialized.representative_items, vec![1, 2, 3]);
        assert_eq!(deserialized.period, "last 7 days");
    }

    #[test]
    fn compute_title_overlap_identical_sets() {
        let titles = vec!["Rust async programming guide".to_string()];
        let overlap = compute_title_overlap(&titles, &titles);
        assert!((overlap - 1.0).abs() < 0.001);
    }

    #[test]
    fn compute_title_overlap_empty_inputs() {
        assert!((compute_title_overlap(&[], &[]) - 0.0).abs() < 0.001);
        let titles = vec!["Something about testing".to_string()];
        assert!((compute_title_overlap(&titles, &[]) - 0.0).abs() < 0.001);
        assert!((compute_title_overlap(&[], &titles) - 0.0).abs() < 0.001);
    }

    #[test]
    fn compute_title_overlap_disjoint_sets() {
        let recent = vec!["Kubernetes deployment strategies".to_string()];
        let older = vec!["React component lifecycle hooks".to_string()];
        let overlap = compute_title_overlap(&recent, &older);
        assert!(
            overlap < 0.1,
            "Disjoint topics should have near-zero overlap: {}",
            overlap
        );
    }

    #[test]
    fn compute_title_overlap_partial_similarity() {
        let recent = vec!["Rust async runtime performance benchmarks".to_string()];
        let older = vec!["Rust memory safety performance guide".to_string()];
        let overlap = compute_title_overlap(&recent, &older);
        // "rust" and "performance" overlap (4+ chars), others differ
        assert!(
            overlap > 0.0 && overlap < 1.0,
            "Partial overlap expected: {}",
            overlap
        );
    }

    #[test]
    fn compute_title_overlap_filters_short_words() {
        // Words with 3 or fewer chars are filtered out
        let recent = vec!["is it a new way to do API".to_string()];
        let older = vec!["is it a new way to do API".to_string()];
        // "is", "it", "a", "new", "way", "to", "do", "API" — all <= 3 chars
        // No words > 3 chars remain → empty sets → should return 0.0
        let overlap = compute_title_overlap(&recent, &older);
        assert!((overlap - 0.0).abs() < 0.001);
    }

    #[test]
    fn compute_title_overlap_case_insensitive() {
        let recent = vec!["TYPESCRIPT MIGRATION Guide".to_string()];
        let older = vec!["typescript migration guide".to_string()];
        let overlap = compute_title_overlap(&recent, &older);
        // "typescript", "migration", "guide" all > 3 chars, identical after lowering
        assert!((overlap - 1.0).abs() < 0.001);
    }

    #[test]
    fn record_and_query_topic_centroid() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS temporal_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                data JSON NOT NULL,
                embedding BLOB,
                source_item_id INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT
            );",
        )
        .expect("create table");

        let titles = vec![
            "Rust 2026 roadmap".to_string(),
            "Async Rust patterns".to_string(),
        ];
        record_topic_centroid(&conn, "rust", 5, 0.72, &titles).unwrap();

        let events = crate::temporal::query_events(&conn, "topic_centroid", None, 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].subject, "rust");
        assert_eq!(events[0].data["item_count"], 5);
        assert!((events[0].data["avg_score"].as_f64().unwrap() - 0.72).abs() < 0.01);
        let stored_titles = events[0].data["top_titles"].as_array().unwrap();
        assert_eq!(stored_titles.len(), 2);
    }

    #[test]
    fn detect_shifts_empty_returns_empty() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS temporal_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                data JSON NOT NULL,
                embedding BLOB,
                source_item_id INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT
            );",
        )
        .expect("create table");

        let shifts = detect_shifts(&conn, 7).unwrap();
        assert!(shifts.is_empty());
    }

    #[test]
    fn semantic_shift_with_empty_representative_items() {
        let shift = SemanticShift {
            topic: "testing".to_string(),
            drift_magnitude: 0.6,
            direction: "Narrative changed".to_string(),
            representative_items: vec![],
            period: "last 7 days".to_string(),
            detected_at: "2026-03-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&shift).unwrap();
        let deserialized: SemanticShift = serde_json::from_str(&json).unwrap();
        assert!(deserialized.representative_items.is_empty());
    }
}
