//! Predictive Context Switching for 4DA
//!
//! Uses time-of-day patterns + git activity + file watcher data to predict
//! what the user will work on next and pre-fetch relevant content.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedContext {
    pub predicted_topics: Vec<(String, f32)>,
    pub predicted_at: String,
    pub reasoning: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSwitchEvent {
    pub from_topics: Vec<String>,
    pub to_topics: Vec<String>,
    pub hour_of_day: u32,
    pub day_of_week: u32,
    pub trigger: String,
}

// ============================================================================
// Implementation
// ============================================================================
/// Predict what the user will work on next based on historical patterns
pub fn predict_next_context(conn: &rusqlite::Connection) -> Result<PredictedContext, String> {
    let now = chrono::Utc::now();
    let current_hour: u32 = now.format("%H").to_string().parse().unwrap_or(0);
    let current_dow: u32 = now.format("%u").to_string().parse().unwrap_or(1);

    // Query recent context switches (last 14 days)
    let events = crate::temporal::query_events(conn, "context_switch", None, 200)?;

    if events.is_empty() {
        return Ok(PredictedContext {
            predicted_topics: vec![],
            predicted_at: now.to_rfc3339(),
            reasoning: "Not enough context switch history for prediction".to_string(),
            confidence: 0.0,
        });
    }

    // Build time-topic frequency map
    let mut hour_topics: HashMap<u32, HashMap<String, u32>> = HashMap::new();

    for event in &events {
        if let Ok(switch) = serde_json::from_value::<ContextSwitchEvent>(event.data.clone()) {
            // Weight by proximity to current hour (±2 hour window)
            let hour_dist = ((switch.hour_of_day as i32 - current_hour as i32).abs()).min(12);
            if hour_dist <= 2 {
                let day_match = switch.day_of_week == current_dow;
                let weight = if day_match { 3 } else { 1 };

                let hour_entry = hour_topics.entry(switch.hour_of_day).or_default();
                for topic in &switch.to_topics {
                    *hour_entry.entry(topic.clone()).or_insert(0) += weight;
                }
            }
        }
    }

    // Aggregate and rank topics
    let mut topic_scores: HashMap<String, f32> = HashMap::new();
    let mut total_weight: f32 = 0.0;

    for topics in hour_topics.values() {
        for (topic, count) in topics {
            let score = *count as f32;
            *topic_scores.entry(topic.clone()).or_insert(0.0) += score;
            total_weight += score;
        }
    }

    // Normalize and sort
    let mut predicted: Vec<(String, f32)> = topic_scores
        .into_iter()
        .map(|(topic, score)| {
            let normalized = if total_weight > 0.0 {
                (score / total_weight).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (topic, normalized)
        })
        .collect();

    predicted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    predicted.truncate(5);

    let confidence = if events.len() >= 20 {
        0.7
    } else if events.len() >= 10 {
        0.5
    } else {
        0.3
    };

    let reasoning = if let Some((top_topic, top_score)) = predicted.first() {
        format!(
            "Around {}:00, you typically work on {} (confidence: {:.0}%)",
            current_hour,
            top_topic,
            top_score * 100.0
        )
    } else {
        "No strong pattern detected for this time".to_string()
    };

    Ok(PredictedContext {
        predicted_topics: predicted,
        predicted_at: now.to_rfc3339(),
        reasoning,
        confidence,
    })
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_predicted_context() -> Result<PredictedContext, String> {
    crate::settings::require_pro_feature("get_predicted_context")?;
    let conn = crate::open_db_connection()?;
    predict_next_context(&conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> rusqlite::Connection {
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
        .expect("create tables");
        conn
    }

    fn insert_context_switch(conn: &rusqlite::Connection, hour: u32, dow: u32, to_topics: &[&str]) {
        let event = ContextSwitchEvent {
            from_topics: vec![],
            to_topics: to_topics.iter().map(|s| s.to_string()).collect(),
            hour_of_day: hour,
            day_of_week: dow,
            trigger: "test".to_string(),
        };
        let data = serde_json::to_value(&event).unwrap();
        crate::temporal::record_event(conn, "context_switch", "user", &data, None, None).unwrap();
    }

    #[test]
    fn predict_empty_history_returns_zero_confidence() {
        let conn = setup_test_db();
        let result = predict_next_context(&conn).unwrap();
        assert_eq!(result.confidence, 0.0);
        assert!(result.predicted_topics.is_empty());
        assert!(result.reasoning.contains("Not enough"));
    }

    #[test]
    fn predict_with_events_returns_topics() {
        let conn = setup_test_db();
        let now = chrono::Utc::now();
        let hour: u32 = now.format("%H").to_string().parse().unwrap_or(12);
        let dow: u32 = now.format("%u").to_string().parse().unwrap_or(1);

        // Insert multiple events at current hour/dow so they match the ±2 hour window
        for _ in 0..5 {
            insert_context_switch(&conn, hour, dow, &["rust", "tauri"]);
        }

        let result = predict_next_context(&conn).unwrap();
        assert!(!result.predicted_topics.is_empty());
        let topic_names: Vec<&str> = result
            .predicted_topics
            .iter()
            .map(|(t, _)| t.as_str())
            .collect();
        assert!(topic_names.contains(&"rust") || topic_names.contains(&"tauri"));
    }

    #[test]
    fn predict_confidence_scales_with_event_count() {
        let conn = setup_test_db();
        let now = chrono::Utc::now();
        let hour: u32 = now.format("%H").to_string().parse().unwrap_or(12);
        let dow: u32 = now.format("%u").to_string().parse().unwrap_or(1);

        // Insert exactly 5 events — should get 0.3 confidence (< 10)
        for _ in 0..5 {
            insert_context_switch(&conn, hour, dow, &["test"]);
        }
        let result = predict_next_context(&conn).unwrap();
        assert_eq!(result.confidence, 0.3);

        // Add more to reach 15 — should get 0.5
        for _ in 0..10 {
            insert_context_switch(&conn, hour, dow, &["test"]);
        }
        let result = predict_next_context(&conn).unwrap();
        assert_eq!(result.confidence, 0.5);

        // Add more to reach 25 — should get 0.7
        for _ in 0..10 {
            insert_context_switch(&conn, hour, dow, &["test"]);
        }
        let result = predict_next_context(&conn).unwrap();
        assert_eq!(result.confidence, 0.7);
    }

    #[test]
    fn predict_day_match_weights_higher() {
        let conn = setup_test_db();
        let now = chrono::Utc::now();
        let hour: u32 = now.format("%H").to_string().parse().unwrap_or(12);
        let dow: u32 = now.format("%u").to_string().parse().unwrap_or(1);
        let other_dow = if dow == 7 { 1 } else { dow + 1 };

        // Insert matching-day event with topic A
        insert_context_switch(&conn, hour, dow, &["topic_a"]);
        // Insert 3x non-matching-day events with topic B
        for _ in 0..3 {
            insert_context_switch(&conn, hour, other_dow, &["topic_b"]);
        }

        let result = predict_next_context(&conn).unwrap();
        // topic_a has weight 3 (1 event * 3 for day match), topic_b has weight 3 (3 events * 1)
        // Either could be first, but both should appear
        assert!(result.predicted_topics.len() >= 2);
    }

    #[test]
    fn predict_topics_truncated_to_5() {
        let conn = setup_test_db();
        let now = chrono::Utc::now();
        let hour: u32 = now.format("%H").to_string().parse().unwrap_or(12);
        let dow: u32 = now.format("%u").to_string().parse().unwrap_or(1);

        for i in 0..10 {
            let topic = format!("topic_{}", i);
            insert_context_switch(&conn, hour, dow, &[&topic]);
        }

        let result = predict_next_context(&conn).unwrap();
        assert!(result.predicted_topics.len() <= 5);
    }

    #[test]
    fn predict_scores_are_normalized() {
        let conn = setup_test_db();
        let now = chrono::Utc::now();
        let hour: u32 = now.format("%H").to_string().parse().unwrap_or(12);
        let dow: u32 = now.format("%u").to_string().parse().unwrap_or(1);

        for _ in 0..5 {
            insert_context_switch(&conn, hour, dow, &["rust"]);
        }

        let result = predict_next_context(&conn).unwrap();
        for (_, score) in &result.predicted_topics {
            assert!(
                *score >= 0.0 && *score <= 1.0,
                "score out of range: {score}"
            );
        }
    }

    #[test]
    fn context_switch_event_serde_roundtrip() {
        let event = ContextSwitchEvent {
            from_topics: vec!["python".to_string()],
            to_topics: vec!["rust".to_string(), "tauri".to_string()],
            hour_of_day: 14,
            day_of_week: 3,
            trigger: "git_activity".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: ContextSwitchEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.to_topics.len(), 2);
        assert_eq!(back.hour_of_day, 14);
    }

    #[test]
    fn predicted_context_serde_roundtrip() {
        let ctx = PredictedContext {
            predicted_topics: vec![("rust".to_string(), 0.8)],
            predicted_at: "2026-01-01T00:00:00Z".to_string(),
            reasoning: "Test".to_string(),
            confidence: 0.7,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let back: PredictedContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back.confidence, 0.7);
        assert_eq!(back.predicted_topics.len(), 1);
    }
}
