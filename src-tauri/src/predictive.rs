//! Predictive Context Switching for 4DA
//!
//! Uses time-of-day patterns + git activity + file watcher data to predict
//! what the user will work on next and pre-fetch relevant content.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

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

/// Record a context switch event in temporal store
pub fn record_context_switch(
    conn: &rusqlite::Connection,
    from_topics: &[String],
    to_topics: &[String],
    trigger: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now();
    let event = ContextSwitchEvent {
        from_topics: from_topics.to_vec(),
        to_topics: to_topics.to_vec(),
        hour_of_day: now.format("%H").to_string().parse().unwrap_or(0),
        day_of_week: now.format("%u").to_string().parse().unwrap_or(1),
        trigger: trigger.to_string(),
    };

    let data = serde_json::to_value(&event).map_err(|e| e.to_string())?;
    let subject = to_topics
        .first()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());

    crate::temporal::record_event(
        conn,
        "context_switch",
        &subject,
        &data,
        None,
        Some(&(now + chrono::Duration::days(14)).to_rfc3339()),
    )?;

    debug!(target: "4da::predictive", trigger, to = ?to_topics, "Recorded context switch");
    Ok(())
}

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

    for (_hour, topics) in &hour_topics {
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
    let conn = crate::open_db_connection()?;
    predict_next_context(&conn)
}

#[tauri::command]
pub fn record_context_switch_event(
    from_topics: Vec<String>,
    to_topics: Vec<String>,
    trigger: String,
) -> Result<(), String> {
    let conn = crate::open_db_connection()?;
    record_context_switch(&conn, &from_topics, &to_topics, &trigger)
}

#[tauri::command]
pub fn get_context_switch_history(
    limit: Option<usize>,
) -> Result<Vec<crate::temporal::TemporalEvent>, String> {
    let conn = crate::open_db_connection()?;
    crate::temporal::query_events(&conn, "context_switch", None, limit.unwrap_or(20))
}
