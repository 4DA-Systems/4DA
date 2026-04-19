// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Developer Temporal Graph — tracks how interests and skills evolve over time.
//!
//! Weekly snapshots capture the developer's tech stack, interests, and engagement.
//! Enables technology adoption curves, knowledge decay detection, and interest trends.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TimelineSnapshot {
    pub id: i64,
    pub period: String,
    pub tech_snapshot: Vec<TechEntry>,
    pub interest_snapshot: Vec<InterestEntry>,
    pub decision_count: u32,
    pub feedback_count: u32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TechEntry {
    pub name: String,
    pub confidence: f32,
    pub engagement_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InterestEntry {
    pub topic: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TechAdoptionCurve {
    pub tech_name: String,
    pub first_seen: String,
    pub weeks_active: u32,
    pub current_confidence: f32,
    pub stage: String,
    pub engagement_history: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct KnowledgeDecay {
    pub tech_name: String,
    pub last_engagement: String,
    pub weeks_since_engagement: u32,
    pub risk_level: String,
    pub recommendation: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InterestTrend {
    pub topic: String,
    pub direction: String,
    pub delta: f32,
    pub current_score: f32,
}

// ============================================================================
// SQL Schema
// ============================================================================

#[allow(dead_code)]
pub(crate) const TEMPORAL_GRAPH_SQL: &str = "
CREATE TABLE IF NOT EXISTS developer_timeline (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    period TEXT NOT NULL UNIQUE,
    tech_snapshot TEXT NOT NULL,
    interest_snapshot TEXT NOT NULL,
    decision_count INTEGER DEFAULT 0,
    feedback_count INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_timeline_period ON developer_timeline(period);
";

// ============================================================================
// Core Functions
// ============================================================================

/// Determine adoption stage based on weeks of engagement and confidence.
pub(crate) fn adoption_stage(weeks_active: u32, current_confidence: f32) -> &'static str {
    match (
        weeks_active,
        current_confidence >= 0.7,
        current_confidence >= 0.85,
    ) {
        (0..=2, _, _) => "exploring",
        (3..=8, _, _) => "learning",
        (9..=20, true, _) => "productive",
        (9..=20, false, _) => "learning",
        (_, _, true) => "expert",
        (_, true, false) => "productive",
        _ => "learning",
    }
}

/// Detect knowledge decay based on time since last engagement.
pub(crate) fn detect_decay(tech_name: &str, weeks_ago: u32) -> KnowledgeDecay {
    let (risk_level, recommendation) = match weeks_ago {
        0..=4 => (
            "fresh",
            format!("Active engagement with {tech_name}"),
        ),
        5..=12 => (
            "aging",
            format!("Consider reviewing recent {tech_name} developments"),
        ),
        13..=26 => (
            "stale",
            format!("{tech_name} knowledge may be outdated — 3+ months since engagement"),
        ),
        _ => (
            "decayed",
            format!("{tech_name} knowledge likely outdated — 6+ months. Major changes may have occurred."),
        ),
    };

    KnowledgeDecay {
        tech_name: tech_name.to_string(),
        last_engagement: format!("{weeks_ago} weeks ago"),
        weeks_since_engagement: weeks_ago,
        risk_level: risk_level.to_string(),
        recommendation,
    }
}

/// Compute interest trends by comparing current and previous snapshots.
#[allow(dead_code)]
pub(crate) fn compute_interest_trends(
    current: &[InterestEntry],
    previous: &[InterestEntry],
) -> Vec<InterestTrend> {
    let prev_map: HashMap<&str, f32> = previous
        .iter()
        .map(|e| (e.topic.as_str(), e.score))
        .collect();

    current
        .iter()
        .map(|entry| {
            let prev_score = prev_map.get(entry.topic.as_str()).copied().unwrap_or(0.0);
            let delta = entry.score - prev_score;
            let direction = if delta > 0.05 {
                "increasing"
            } else if delta < -0.05 {
                "decreasing"
            } else {
                "stable"
            };

            InterestTrend {
                topic: entry.topic.clone(),
                direction: direction.to_string(),
                delta,
                current_score: entry.score,
            }
        })
        .collect()
}

/// Build adoption curves from a series of timeline snapshots.
pub(crate) fn build_adoption_curves(snapshots: &[TimelineSnapshot]) -> Vec<TechAdoptionCurve> {
    if snapshots.is_empty() {
        return vec![];
    }

    let mut tech_history: HashMap<String, Vec<(String, f32, f32)>> = HashMap::new();

    for snapshot in snapshots {
        for tech in &snapshot.tech_snapshot {
            tech_history.entry(tech.name.clone()).or_default().push((
                snapshot.period.clone(),
                tech.confidence,
                tech.engagement_score,
            ));
        }
    }

    tech_history
        .into_iter()
        .map(|(name, history)| {
            let first_seen = history
                .first()
                .map(|(p, _, _)| p.clone())
                .unwrap_or_default();
            let weeks_active = history.len() as u32;
            let current_confidence = history.last().map_or(0.0, |(_, c, _)| *c);
            let engagement_history: Vec<f32> = history.iter().map(|(_, _, e)| *e).collect();
            let stage = adoption_stage(weeks_active, current_confidence).to_string();

            TechAdoptionCurve {
                tech_name: name,
                first_seen,
                weeks_active,
                current_confidence,
                stage,
                engagement_history,
            }
        })
        .collect()
}

// ============================================================================
// Weekly Timeline Snapshot (Background Job)
// ============================================================================

/// Compute and store a developer timeline snapshot for the current week.
/// Called by the monitoring background job on a weekly cadence.
/// Pulls tech data from ACE's detected_tech and interest data from active_topics.
pub(crate) fn record_weekly_timeline(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    let now = chrono::Utc::now();
    let period = format!("{}-W{:02}", now.format("%Y"), now.format("%W"));

    // Build tech snapshot from ACE detected technologies
    let tech_snapshot: Vec<TechEntry> = match crate::state::get_ace_engine() {
        Ok(ace) => ace
            .get_detected_tech()
            .unwrap_or_default()
            .into_iter()
            .map(|t| TechEntry {
                name: t.name,
                confidence: t.confidence,
                engagement_score: t.confidence, // Use confidence as engagement proxy
            })
            .collect(),
        Err(_) => vec![],
    };

    // Build interest snapshot from ACE active topics
    let interest_snapshot: Vec<InterestEntry> = match crate::state::get_ace_engine() {
        Ok(ace) => ace
            .get_active_topics()
            .unwrap_or_default()
            .into_iter()
            .map(|t| InterestEntry {
                topic: t.topic,
                score: t.weight,
            })
            .collect(),
        Err(_) => vec![],
    };

    // Count decisions and feedback from the past week
    let decision_count: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM developer_decisions WHERE created_at >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let feedback_count: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM feedback WHERE created_at >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let tech_json = serde_json::to_string(&tech_snapshot).unwrap_or_else(|_| "[]".to_string());
    let interest_json =
        serde_json::to_string(&interest_snapshot).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT OR REPLACE INTO developer_timeline (period, tech_snapshot, interest_snapshot, decision_count, feedback_count)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![period, tech_json, interest_json, decision_count, feedback_count],
    )?;

    tracing::info!(
        target: "4da::temporal",
        period = %period,
        techs = tech_snapshot.len(),
        interests = interest_snapshot.len(),
        decisions = decision_count,
        feedback = feedback_count,
        "Weekly timeline snapshot recorded"
    );

    Ok(())
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_temporal_snapshot(period: Option<String>) -> crate::error::Result<serde_json::Value> {
    let p = period.unwrap_or_else(|| {
        let now = chrono::Utc::now();
        format!("{}-W{:02}", now.format("%Y"), now.format("%W"))
    });
    let conn = crate::get_database()?.conn.lock();

    let snapshot_opt: Option<(String, String, u32, u32, String)> = conn
        .query_row(
            "SELECT tech_snapshot, interest_snapshot, decision_count, feedback_count, created_at \
             FROM developer_timeline WHERE period = ?1",
            rusqlite::params![p],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .ok();

    match snapshot_opt {
        Some((tech_json, interest_json, dec_count, fb_count, created_at)) => {
            let tech: Vec<TechEntry> = serde_json::from_str(&tech_json).unwrap_or_default();
            let interests: Vec<InterestEntry> =
                serde_json::from_str(&interest_json).unwrap_or_default();
            Ok(serde_json::json!({
                "period": p,
                "tech_snapshot": tech,
                "interest_snapshot": interests,
                "decision_count": dec_count,
                "feedback_count": fb_count,
                "created_at": created_at,
            }))
        }
        None => Ok(serde_json::json!({
            "period": p,
            "tech_snapshot": [],
            "interest_snapshot": [],
            "message": "No snapshot for this period"
        })),
    }
}

#[tauri::command]
pub fn get_adoption_curves() -> crate::error::Result<serde_json::Value> {
    let conn = crate::get_database()?.conn.lock();

    let mut stmt = conn.prepare(
        "SELECT id, period, tech_snapshot, interest_snapshot, decision_count, feedback_count, created_at \
         FROM developer_timeline ORDER BY period ASC",
    )?;
    let snapshots: Vec<TimelineSnapshot> = stmt
        .query_map([], |row| {
            let tech_json: String = row.get(2)?;
            let interest_json: String = row.get(3)?;
            Ok(TimelineSnapshot {
                id: row.get(0)?,
                period: row.get(1)?,
                tech_snapshot: serde_json::from_str(&tech_json).unwrap_or_default(),
                interest_snapshot: serde_json::from_str(&interest_json).unwrap_or_default(),
                decision_count: row.get(4)?,
                feedback_count: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?
        .filter_map(std::result::Result::ok)
        .collect();

    let curves = build_adoption_curves(&snapshots);
    Ok(serde_json::to_value(curves)?)
}

#[tauri::command]
pub fn get_knowledge_decay_report() -> crate::error::Result<serde_json::Value> {
    let conn = crate::get_database()?.conn.lock();

    // Get the most recent snapshot
    let latest_opt: Option<String> = conn
        .query_row(
            "SELECT tech_snapshot FROM developer_timeline ORDER BY period DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    let tech: Vec<TechEntry> = match latest_opt {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => return Ok(serde_json::json!([])),
    };

    // For each tech, find how many weeks since last seen with engagement > 0
    let mut decays: Vec<KnowledgeDecay> = Vec::new();
    for entry in &tech {
        // Count weeks since last significant engagement
        let weeks_ago: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM developer_timeline WHERE period < (SELECT MAX(period) FROM developer_timeline)",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if entry.engagement_score < 0.1 {
            decays.push(detect_decay(&entry.name, weeks_ago.min(52)));
        }
    }

    Ok(serde_json::to_value(decays)?)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adoption_stages() {
        assert_eq!(adoption_stage(1, 0.3), "exploring");
        assert_eq!(adoption_stage(5, 0.4), "learning");
        assert_eq!(adoption_stage(12, 0.8), "productive");
        assert_eq!(adoption_stage(30, 0.9), "expert");
        assert_eq!(adoption_stage(15, 0.5), "learning");
    }

    #[test]
    fn test_knowledge_decay_levels() {
        assert_eq!(detect_decay("Rust", 2).risk_level, "fresh");
        assert_eq!(detect_decay("Go", 8).risk_level, "aging");
        assert_eq!(detect_decay("Docker", 15).risk_level, "stale");
        assert_eq!(detect_decay("jQuery", 30).risk_level, "decayed");
    }

    #[test]
    fn test_interest_trends() {
        let current = vec![
            InterestEntry {
                topic: "Rust".into(),
                score: 0.8,
            },
            InterestEntry {
                topic: "React".into(),
                score: 0.5,
            },
            InterestEntry {
                topic: "Go".into(),
                score: 0.6,
            },
        ];
        let previous = vec![
            InterestEntry {
                topic: "Rust".into(),
                score: 0.5,
            },
            InterestEntry {
                topic: "React".into(),
                score: 0.7,
            },
            InterestEntry {
                topic: "Go".into(),
                score: 0.58,
            },
        ];

        let trends = compute_interest_trends(&current, &previous);
        assert_eq!(trends.len(), 3);
        assert_eq!(trends[0].direction, "increasing"); // Rust 0.5 -> 0.8
        assert_eq!(trends[1].direction, "decreasing"); // React 0.7 -> 0.5
        assert_eq!(trends[2].direction, "stable"); // Go 0.58 -> 0.6
    }

    #[test]
    fn test_adoption_curves_empty() {
        let curves = build_adoption_curves(&[]);
        assert!(curves.is_empty());
    }

    #[test]
    fn test_adoption_curves_single_snapshot() {
        let snapshots = vec![TimelineSnapshot {
            id: 1,
            period: "2026-W10".into(),
            tech_snapshot: vec![TechEntry {
                name: "Rust".into(),
                confidence: 0.7,
                engagement_score: 0.8,
            }],
            interest_snapshot: vec![],
            decision_count: 3,
            feedback_count: 15,
            created_at: "2026-03-01".into(),
        }];

        let curves = build_adoption_curves(&snapshots);
        assert_eq!(curves.len(), 1);
        assert_eq!(curves[0].weeks_active, 1);
    }
}
