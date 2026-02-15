//! Attention Economy Dashboard for 4DA
//!
//! Tracks where user attention goes vs where code needs it.
//! Identifies blind spots between engagement patterns and codebase topics.

use serde::{Deserialize, Serialize};
use tracing::info;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionReport {
    pub period_days: u32,
    pub topic_engagement: Vec<TopicEngagement>,
    pub codebase_topics: Vec<CodebaseTopic>,
    pub blind_spots: Vec<BlindSpot>,
    pub attention_trend: Vec<TrendPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicEngagement {
    pub topic: String,
    pub interactions: u32,
    pub percent_of_total: f32,
    pub sentiment: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseTopic {
    pub topic: String,
    pub file_count: u32,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindSpot {
    pub topic: String,
    pub in_codebase: bool,
    pub engagement_level: f32,
    pub gap_description: String,
    pub risk_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendPoint {
    pub date: String,
    pub topic: String,
    pub engagement_level: f32,
}

// ============================================================================
// Implementation
// ============================================================================

/// Generate an attention report for the specified period
pub fn generate_report(period_days: u32) -> Result<AttentionReport, String> {
    let conn = crate::open_db_connection()?;

    let topic_engagement = compute_topic_engagement(&conn, period_days)?;
    let codebase_topics = get_codebase_topics()?;
    let blind_spots = identify_blind_spots(&topic_engagement, &codebase_topics);
    let attention_trend = compute_trend(&conn, period_days)?;

    Ok(AttentionReport {
        period_days,
        topic_engagement,
        codebase_topics,
        blind_spots,
        attention_trend,
    })
}

fn compute_topic_engagement(
    conn: &rusqlite::Connection,
    period_days: u32,
) -> Result<Vec<TopicEngagement>, String> {
    // Get interactions from feedback table within the period
    let mut stmt = conn
        .prepare(
            "SELECT si.title, f.relevant
             FROM feedback f
             JOIN source_items si ON si.id = f.source_item_id
             WHERE f.created_at >= datetime('now', ?1)
             ORDER BY f.created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let since = format!("-{} days", period_days);
    let rows: Vec<(String, bool)> = stmt
        .query_map(rusqlite::params![since], |row| {
            let title: String = row.get(0)?;
            let relevant: bool = row.get::<_, i32>(1)? != 0;
            Ok((title, relevant))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Extract topics from titles and aggregate
    let mut topic_counts: std::collections::HashMap<String, (u32, i32)> =
        std::collections::HashMap::new();

    for (title, relevant) in &rows {
        let topics = crate::extract_topics(title, "");
        for topic in topics {
            let entry = topic_counts.entry(topic).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += if *relevant { 1 } else { -1 };
        }
    }

    let total: u32 = topic_counts.values().map(|(c, _)| c).sum();
    let total = total.max(1) as f32;

    let mut engagement: Vec<TopicEngagement> = topic_counts
        .into_iter()
        .map(|(topic, (count, sentiment_raw))| TopicEngagement {
            topic,
            interactions: count,
            percent_of_total: (count as f32 / total) * 100.0,
            sentiment: (sentiment_raw as f32 / count.max(1) as f32).clamp(-1.0, 1.0),
        })
        .collect();

    engagement.sort_by(|a, b| {
        b.interactions
            .partial_cmp(&a.interactions)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    engagement.truncate(20);

    Ok(engagement)
}

fn get_codebase_topics() -> Result<Vec<CodebaseTopic>, String> {
    // Get detected tech from ACE
    let ace = match crate::get_ace_engine() {
        Ok(engine) => engine,
        Err(_) => {
            return Ok(vec![]);
        }
    };

    let detected_tech = ace.get_detected_tech().unwrap_or_default();
    let active_topics = ace.get_active_topics().unwrap_or_default();

    let mut topics: Vec<CodebaseTopic> = detected_tech
        .iter()
        .map(|dt| CodebaseTopic {
            topic: dt.name.clone(),
            file_count: 1, // We don't have exact file counts per tech
            source: format!("detected_{:?}", dt.category).to_lowercase(),
        })
        .collect();

    for at in active_topics {
        if !topics.iter().any(|t| t.topic == at.topic) {
            topics.push(CodebaseTopic {
                topic: at.topic,
                file_count: 1,
                source: "active_topic".to_string(),
            });
        }
    }

    Ok(topics)
}

fn identify_blind_spots(
    engagement: &[TopicEngagement],
    codebase: &[CodebaseTopic],
) -> Vec<BlindSpot> {
    let mut blind_spots = Vec::new();

    // Find codebase topics with low or no engagement
    for ct in codebase {
        let topic_lower = ct.topic.to_lowercase();
        let engagement_level = engagement
            .iter()
            .find(|e| e.topic.to_lowercase() == topic_lower)
            .map(|e| e.percent_of_total / 100.0)
            .unwrap_or(0.0);

        if engagement_level < 0.05 {
            let risk = if ct.source.contains("language") || ct.source.contains("framework") {
                "high"
            } else {
                "medium"
            };

            blind_spots.push(BlindSpot {
                topic: ct.topic.clone(),
                in_codebase: true,
                engagement_level,
                gap_description: format!(
                    "{} is in your codebase ({}) but you rarely engage with content about it",
                    ct.topic, ct.source
                ),
                risk_level: risk.to_string(),
            });
        }
    }

    blind_spots.sort_by(|a, b| a.risk_level.cmp(&b.risk_level));
    blind_spots.truncate(10);
    blind_spots
}

fn compute_trend(conn: &rusqlite::Connection, period_days: u32) -> Result<Vec<TrendPoint>, String> {
    let since = format!("-{} days", period_days);
    let mut stmt = conn
        .prepare(
            "SELECT date(f.created_at) as d, si.title
             FROM feedback f
             JOIN source_items si ON si.id = f.source_item_id
             WHERE f.created_at >= datetime('now', ?1)
             ORDER BY d",
        )
        .map_err(|e| e.to_string())?;

    let rows: Vec<(String, String)> = stmt
        .query_map(rusqlite::params![since], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Group by date and top topics
    let mut daily: std::collections::HashMap<String, std::collections::HashMap<String, u32>> =
        std::collections::HashMap::new();

    for (date, title) in &rows {
        let topics = crate::extract_topics(title, "");
        let day_entry = daily.entry(date.clone()).or_default();
        for topic in topics.into_iter().take(3) {
            *day_entry.entry(topic).or_insert(0) += 1;
        }
    }

    let mut trend: Vec<TrendPoint> = Vec::new();
    for (date, topics) in &daily {
        let total: u32 = topics.values().sum();
        for (topic, count) in topics {
            trend.push(TrendPoint {
                date: date.clone(),
                topic: topic.clone(),
                engagement_level: *count as f32 / total.max(1) as f32,
            });
        }
    }

    trend.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(trend)
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_attention_report(period_days: Option<u32>) -> Result<AttentionReport, String> {
    let days = period_days.unwrap_or(30);
    info!(target: "4da::attention", period_days = days, "Generating attention report");
    generate_report(days)
}
