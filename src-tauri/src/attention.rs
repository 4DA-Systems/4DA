//! Attention Economy Dashboard for 4DA
//!
//! Tracks where user attention goes vs where code needs it.
//! Identifies blind spots between engagement patterns and codebase topics.

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::error::Result;

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
pub fn generate_report(period_days: u32) -> Result<AttentionReport> {
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
) -> Result<Vec<TopicEngagement>> {
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

fn get_codebase_topics() -> Result<Vec<CodebaseTopic>> {
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

fn compute_trend(conn: &rusqlite::Connection, period_days: u32) -> Result<Vec<TrendPoint>> {
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
pub fn get_attention_report(period_days: Option<u32>) -> Result<AttentionReport> {
    crate::settings::require_pro_feature("get_attention_report")?;
    let days = period_days.unwrap_or(30);
    info!(target: "4da::attention", period_days = days, "Generating attention report");
    generate_report(days)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engagement(topic: &str, interactions: u32, percent: f32) -> TopicEngagement {
        TopicEngagement {
            topic: topic.to_string(),
            interactions,
            percent_of_total: percent,
            sentiment: 0.5,
        }
    }

    fn make_codebase_topic(topic: &str, source: &str) -> CodebaseTopic {
        CodebaseTopic {
            topic: topic.to_string(),
            file_count: 1,
            source: source.to_string(),
        }
    }

    #[test]
    fn attention_report_serde_roundtrip() {
        let report = AttentionReport {
            period_days: 30,
            topic_engagement: vec![TopicEngagement {
                topic: "rust".to_string(),
                interactions: 15,
                percent_of_total: 45.0,
                sentiment: 0.8,
            }],
            codebase_topics: vec![CodebaseTopic {
                topic: "rust".to_string(),
                file_count: 50,
                source: "detected_language".to_string(),
            }],
            blind_spots: vec![],
            attention_trend: vec![TrendPoint {
                date: "2026-02-28".to_string(),
                topic: "rust".to_string(),
                engagement_level: 0.65,
            }],
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: AttentionReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.period_days, 30);
        assert_eq!(deserialized.topic_engagement.len(), 1);
        assert_eq!(deserialized.topic_engagement[0].topic, "rust");
        assert!((deserialized.topic_engagement[0].sentiment - 0.8).abs() < 0.001);
    }

    #[test]
    fn blind_spot_serde_roundtrip() {
        let bs = BlindSpot {
            topic: "python".to_string(),
            in_codebase: true,
            engagement_level: 0.02,
            gap_description: "Python is in your codebase but rarely engaged".to_string(),
            risk_level: "high".to_string(),
        };
        let json = serde_json::to_string(&bs).unwrap();
        let deserialized: BlindSpot = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.topic, "python");
        assert!(deserialized.in_codebase);
        assert_eq!(deserialized.risk_level, "high");
    }

    #[test]
    fn identify_blind_spots_finds_low_engagement_codebase_topics() {
        let engagement = vec![
            make_engagement("rust", 20, 60.0),
            make_engagement("react", 10, 30.0),
        ];
        let codebase = vec![
            make_codebase_topic("rust", "detected_language"),
            make_codebase_topic("python", "detected_language"),
            make_codebase_topic("docker", "detected_tool"),
        ];
        let spots = identify_blind_spots(&engagement, &codebase);
        // python and docker have 0 engagement → both should be blind spots
        assert_eq!(spots.len(), 2);
        let topics: Vec<&str> = spots.iter().map(|s| s.topic.as_str()).collect();
        assert!(topics.contains(&"python"));
        assert!(topics.contains(&"docker"));
    }

    #[test]
    fn identify_blind_spots_skips_high_engagement_topics() {
        let engagement = vec![make_engagement("rust", 20, 80.0)];
        let codebase = vec![make_codebase_topic("rust", "detected_language")];
        // rust has 80% engagement → should NOT be a blind spot
        let spots = identify_blind_spots(&engagement, &codebase);
        assert!(spots.is_empty());
    }

    #[test]
    fn identify_blind_spots_assigns_high_risk_to_language_framework() {
        let engagement = vec![];
        let codebase = vec![
            make_codebase_topic("python", "detected_language"),
            make_codebase_topic("redis", "detected_tool"),
        ];
        let spots = identify_blind_spots(&engagement, &codebase);
        let python_spot = spots.iter().find(|s| s.topic == "python").unwrap();
        let redis_spot = spots.iter().find(|s| s.topic == "redis").unwrap();
        assert_eq!(python_spot.risk_level, "high");
        assert_eq!(redis_spot.risk_level, "medium");
    }

    #[test]
    fn identify_blind_spots_threshold_at_5_percent() {
        // 4.9% engagement → blind spot (below 5%)
        let engagement = vec![make_engagement("go", 5, 4.9)];
        let codebase = vec![make_codebase_topic("go", "detected_language")];
        let spots = identify_blind_spots(&engagement, &codebase);
        assert_eq!(spots.len(), 1);
        assert_eq!(spots[0].topic, "go");

        // 5.1% engagement → NOT a blind spot
        let engagement = vec![make_engagement("go", 5, 5.1)];
        let spots2 = identify_blind_spots(&engagement, &codebase);
        assert!(spots2.is_empty());
    }

    #[test]
    fn identify_blind_spots_truncates_to_10() {
        let engagement = vec![];
        let codebase: Vec<CodebaseTopic> = (0..15)
            .map(|i| make_codebase_topic(&format!("topic_{}", i), "detected_tool"))
            .collect();
        let spots = identify_blind_spots(&engagement, &codebase);
        assert_eq!(spots.len(), 10);
    }

    #[test]
    fn identify_blind_spots_case_insensitive_matching() {
        let engagement = vec![make_engagement("Rust", 20, 80.0)];
        let codebase = vec![make_codebase_topic("rust", "detected_language")];
        // "Rust" in engagement should match "rust" in codebase (case-insensitive)
        let spots = identify_blind_spots(&engagement, &codebase);
        assert!(spots.is_empty());
    }

    #[test]
    fn identify_blind_spots_sets_in_codebase_true() {
        let engagement = vec![];
        let codebase = vec![make_codebase_topic("typescript", "detected_language")];
        let spots = identify_blind_spots(&engagement, &codebase);
        assert_eq!(spots.len(), 1);
        assert!(spots[0].in_codebase);
        assert_eq!(spots[0].engagement_level, 0.0);
    }

    #[test]
    fn identify_blind_spots_gap_description_includes_topic_and_source() {
        let engagement = vec![];
        let codebase = vec![make_codebase_topic("tailwind", "detected_framework")];
        let spots = identify_blind_spots(&engagement, &codebase);
        assert_eq!(spots.len(), 1);
        assert!(spots[0].gap_description.contains("tailwind"));
        assert!(spots[0].gap_description.contains("detected_framework"));
    }

    #[test]
    fn identify_blind_spots_framework_source_is_high_risk() {
        let engagement = vec![];
        let codebase = vec![make_codebase_topic("react", "detected_framework")];
        let spots = identify_blind_spots(&engagement, &codebase);
        assert_eq!(spots.len(), 1);
        assert_eq!(spots[0].risk_level, "high");
    }

    #[test]
    fn topic_engagement_serde_preserves_float_precision() {
        let te = TopicEngagement {
            topic: "testing".to_string(),
            interactions: 7,
            percent_of_total: 33.33,
            sentiment: -0.5,
        };
        let json = serde_json::to_string(&te).unwrap();
        let deserialized: TopicEngagement = serde_json::from_str(&json).unwrap();
        assert!((deserialized.percent_of_total - 33.33).abs() < 0.01);
        assert!((deserialized.sentiment - (-0.5)).abs() < 0.001);
    }

    #[test]
    fn trend_point_serde_roundtrip() {
        let tp = TrendPoint {
            date: "2026-03-01".to_string(),
            topic: "kubernetes".to_string(),
            engagement_level: 0.42,
        };
        let json = serde_json::to_string(&tp).unwrap();
        let deserialized: TrendPoint = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.date, "2026-03-01");
        assert_eq!(deserialized.topic, "kubernetes");
        assert!((deserialized.engagement_level - 0.42).abs() < 0.001);
    }
}
