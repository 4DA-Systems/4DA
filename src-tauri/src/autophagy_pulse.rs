//! Intelligence Pulse subsystem for the autophagy dashboard.
//!
//! Provides a rich 7-day snapshot of how the scoring/relevance system is performing,
//! including calibration insights, source quality, and learning narratives.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

use serde::Serialize;
use ts_rs::TS;

use crate::error::{FourDaError, Result};

// 60-second TTL cache for intelligence pulse (avoids 7 SQL queries on repeat views)
static PULSE_CACHE: LazyLock<Mutex<Option<(IntelligencePulse, Instant)>>> =
    LazyLock::new(|| Mutex::new(None));
const PULSE_TTL_SECS: u64 = 60;

/// A single calibration insight: how far off the system was for a topic.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct CalibrationInsight {
    pub topic: String,
    /// positive = under-scored (should rank higher), negative = over-scored
    pub delta: f64,
    pub sample_size: i64,
    pub confidence: f64,
}

/// Per-source engagement quality for the intelligence pulse.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct SourceQuality {
    pub source_type: String,
    pub items_surfaced: i64,
    pub items_engaged: i64,
    pub engagement_rate: f64,
}

/// Rich intelligence pulse: a 7-day snapshot of how the system is performing.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct IntelligencePulse {
    /// Total items seen (last_seen) in the last 7 days
    pub items_analyzed_7d: i64,
    /// Total items that received positive feedback in the last 7 days (proxy for surfaced)
    pub items_surfaced_7d: i64,
    /// Rejection rate as a percentage (e.g. 99.2)
    pub rejection_rate: f64,
    /// Calibration accuracy (0.0–1.0) — average confidence of active calibrations
    pub calibration_accuracy: f64,
    /// Top calibration deltas by absolute value (up to 3)
    pub top_calibrations: Vec<CalibrationInsight>,
    /// Source quality rankings from stored autopsies
    pub source_quality: Vec<SourceQuality>,
    /// Total anti-patterns currently detected (non-superseded)
    pub anti_patterns_detected: i64,
    /// Total autophagy cycles ever run
    pub total_cycles: i64,
    /// Human-readable narrative insights generated from autophagy data
    pub learning_narratives: Vec<String>,
}

/// Return a rich intelligence pulse for the frontend dashboard.
///
/// Pulls from `scoring_stats`, `digested_intelligence`, and `autophagy_cycles`
/// to give the frontend a single, pre-computed snapshot of system health.
#[tauri::command]
pub async fn get_intelligence_pulse() -> Result<IntelligencePulse> {
    // Check cache before running 7 SQL queries
    {
        let cache = PULSE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((ref result, ref instant)) = *cache {
            if instant.elapsed().as_secs() < PULSE_TTL_SECS {
                return Ok(result.clone());
            }
        }
    }

    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;

    // ── 1. Items analyzed in the last 7 days (rows seen / fetched) ──────────
    let items_analyzed_7d: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM source_items
             WHERE last_seen >= datetime('now', '-7 days')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // ── 2. Items surfaced: received positive feedback in the last 7 days ────
    // Score is not persisted to the DB, so positive feedback is the best proxy
    // for "the system surfaced this item and the user found it relevant".
    let items_surfaced_7d: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT si.id)
             FROM feedback f
             JOIN source_items si ON f.source_item_id = si.id
             WHERE f.relevant = 1
               AND si.last_seen >= datetime('now', '-7 days')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // ── 3. Rejection rate ────────────────────────────────────────────────────
    // Compute from items_analyzed_7d vs items_surfaced_7d.
    // If we have no analyzed items, fall back to the lifetime scoring_stats.
    let rejection_rate: f64 = if items_analyzed_7d > 0 {
        let rejected = items_analyzed_7d - items_surfaced_7d;
        (rejected as f64 / items_analyzed_7d as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        // Fall back to the aggregate rejection rate from scoring_stats
        let (total_scored, total_relevant): (i64, i64) = conn
            .query_row(
                "SELECT COALESCE(SUM(total_scored), 0), COALESCE(SUM(relevant_count), 0)
                 FROM scoring_stats",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or((0, 0));
        if total_scored > 0 {
            ((total_scored - total_relevant) as f64 / total_scored as f64 * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        }
    };

    // ── 4. Calibration accuracy: avg confidence of active calibrations ───────
    let calibration_accuracy: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(confidence), 0.0)
             FROM digested_intelligence
             WHERE digest_type = 'calibration' AND superseded_by IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    // ── 5. Top calibrations: up to 3 by abs(delta) ───────────────────────────
    let top_calibrations = {
        let mut stmt = conn
            .prepare(
                "SELECT subject, data, confidence, sample_size
                 FROM digested_intelligence
                 WHERE digest_type = 'calibration' AND superseded_by IS NULL
                 ORDER BY created_at DESC",
            )
            .map_err(FourDaError::Db)?;

        let mut rows: Vec<CalibrationInsight> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(FourDaError::Db)?
            .filter_map(|r| r.ok())
            .filter_map(|(topic, data_json, confidence, sample_size)| {
                let data: serde_json::Value = serde_json::from_str(&data_json).ok()?;
                let delta = data.get("delta")?.as_f64()?;
                Some(CalibrationInsight {
                    topic,
                    delta,
                    sample_size,
                    confidence,
                })
            })
            .collect();

        // Sort by absolute delta descending, take top 3
        rows.sort_by(|a, b| {
            b.delta
                .abs()
                .partial_cmp(&a.delta.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        rows.truncate(3);
        rows
    };

    // ── 6. Source quality: from stored autopsies, deduped by source_type ────
    let source_quality = {
        let mut stmt = conn
            .prepare(
                "SELECT data
                 FROM digested_intelligence
                 WHERE digest_type = 'source_autopsy' AND superseded_by IS NULL
                 ORDER BY created_at DESC",
            )
            .map_err(FourDaError::Db)?;

        // Collect all non-superseded autopsies, merge by source_type (keep latest)
        let mut by_source: HashMap<String, SourceQuality> = HashMap::new();

        let rows: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(FourDaError::Db)?
            .filter_map(|r| r.ok())
            .collect();

        for data_json in rows {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&data_json) {
                let source_type = data
                    .get("source_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let items_surfaced = data
                    .get("items_surfaced")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let items_engaged = data
                    .get("items_engaged")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let engagement_rate = data
                    .get("engagement_rate")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                // Keep first (most recent) entry for each source_type
                by_source
                    .entry(source_type.clone())
                    .or_insert(SourceQuality {
                        source_type,
                        items_surfaced,
                        items_engaged,
                        engagement_rate,
                    });
            }
        }

        // Sort by engagement_rate descending
        let mut quality: Vec<SourceQuality> = by_source.into_values().collect();
        quality.sort_by(|a, b| {
            b.engagement_rate
                .partial_cmp(&a.engagement_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        quality
    };

    // ── 7. Anti-patterns detected (active, non-superseded) ───────────────────
    let anti_patterns_detected: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digested_intelligence
             WHERE digest_type = 'anti_pattern' AND superseded_by IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // ── 8. Total autophagy cycles ─────────────────────────────────────────────
    let total_cycles: i64 = conn
        .query_row("SELECT COUNT(*) FROM autophagy_cycles", [], |r| r.get(0))
        .unwrap_or(0);

    // ── 9. Generate human-readable learning narratives ────────────────────
    let mut learning_narratives = Vec::new();

    for cal in &top_calibrations {
        let direction = if cal.delta > 0.0 {
            "increasing"
        } else {
            "decreasing"
        };
        let pct = (cal.delta.abs() * 100.0).round();
        learning_narratives.push(format!(
            "Your {} relevance is {} by {:.0}% \u{2014} based on {} interactions",
            cal.topic, direction, pct, cal.sample_size
        ));
    }

    if let (Some(best), Some(worst)) = (source_quality.first(), source_quality.last()) {
        if source_quality.len() >= 2 && best.engagement_rate > worst.engagement_rate * 2.0 {
            learning_narratives.push(format!(
                "{} delivers {:.0}x more relevant content than {} for your stack",
                best.source_type,
                best.engagement_rate / worst.engagement_rate.max(0.01),
                worst.source_type
            ));
        }
    }

    if rejection_rate > 95.0 {
        learning_narratives.push(format!(
            "Processed {} items, surfaced {} ({:.1}% rejection rate) \u{2014} your filter is sharp",
            items_analyzed_7d, items_surfaced_7d, rejection_rate
        ));
    }

    if anti_patterns_detected > 0 {
        learning_narratives.push(format!(
            "Detected {} scoring anti-pattern{} \u{2014} self-correcting",
            anti_patterns_detected,
            if anti_patterns_detected > 1 { "s" } else { "" }
        ));
    }

    // ── 10. Cold-start fallback narratives ────────────────────────────────
    if learning_narratives.is_empty() {
        if total_cycles == 0 {
            learning_narratives.push(
                "Analyzing your first interactions \u{2014} patterns will emerge as you save and dismiss content"
                    .to_string(),
            );
        } else if items_analyzed_7d > 0 {
            learning_narratives.push(format!(
                "Processed {} items this week \u{2014} interact with results to sharpen your profile",
                items_analyzed_7d
            ));
        }
    }

    let pulse = IntelligencePulse {
        items_analyzed_7d,
        items_surfaced_7d,
        rejection_rate,
        calibration_accuracy,
        top_calibrations,
        source_quality,
        anti_patterns_detected,
        total_cycles,
        learning_narratives,
    };

    // Store in cache
    {
        let mut cache = PULSE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        *cache = Some((pulse.clone(), Instant::now()));
    }

    Ok(pulse)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intelligence_pulse_construction() {
        let pulse = IntelligencePulse {
            items_analyzed_7d: 1500,
            items_surfaced_7d: 12,
            rejection_rate: 99.2,
            calibration_accuracy: 0.75,
            top_calibrations: vec![
                CalibrationInsight {
                    topic: "rust".to_string(),
                    delta: -0.7,
                    sample_size: 20,
                    confidence: 0.9,
                },
                CalibrationInsight {
                    topic: "ai".to_string(),
                    delta: 0.3,
                    sample_size: 10,
                    confidence: 0.5,
                },
            ],
            source_quality: vec![SourceQuality {
                source_type: "hackernews".to_string(),
                items_surfaced: 800,
                items_engaged: 8,
                engagement_rate: 0.01,
            }],
            anti_patterns_detected: 2,
            total_cycles: 7,
            learning_narratives: vec!["Test narrative".to_string()],
        };

        assert_eq!(pulse.items_analyzed_7d, 1500);
        assert_eq!(pulse.items_surfaced_7d, 12);
        assert!((pulse.rejection_rate - 99.2).abs() < f64::EPSILON);
        assert!((pulse.calibration_accuracy - 0.75).abs() < f64::EPSILON);
        assert_eq!(pulse.top_calibrations.len(), 2);
        assert_eq!(pulse.top_calibrations[0].topic, "rust");
        assert!((pulse.top_calibrations[0].delta - (-0.7)).abs() < f64::EPSILON);
        assert_eq!(pulse.source_quality.len(), 1);
        assert_eq!(pulse.source_quality[0].source_type, "hackernews");
        assert_eq!(pulse.anti_patterns_detected, 2);
        assert_eq!(pulse.total_cycles, 7);
    }

    #[test]
    fn test_intelligence_pulse_serialization() {
        let pulse = IntelligencePulse {
            items_analyzed_7d: 500,
            items_surfaced_7d: 5,
            rejection_rate: 99.0,
            calibration_accuracy: 0.6,
            top_calibrations: vec![],
            source_quality: vec![],
            anti_patterns_detected: 0,
            total_cycles: 3,
            learning_narratives: vec![],
        };
        let json = serde_json::to_string(&pulse).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed["items_analyzed_7d"], 500);
        assert_eq!(parsed["items_surfaced_7d"], 5);
        assert_eq!(parsed["total_cycles"], 3);
        assert!(parsed["top_calibrations"].is_array());
        assert!(parsed["source_quality"].is_array());
    }

    #[test]
    fn test_cold_start_narrative_zero_cycles() {
        // When total_cycles is 0 and no calibrations exist, the system should
        // generate a cold-start narrative guiding the user toward first interactions.
        let pulse = IntelligencePulse {
            items_analyzed_7d: 0,
            items_surfaced_7d: 0,
            rejection_rate: 0.0,
            calibration_accuracy: 0.0,
            top_calibrations: vec![],
            source_quality: vec![],
            anti_patterns_detected: 0,
            total_cycles: 0,
            learning_narratives: vec![
                "Analyzing your first interactions \u{2014} patterns will emerge as you save and dismiss content"
                    .to_string(),
            ],
        };
        assert_eq!(pulse.learning_narratives.len(), 1);
        assert!(pulse.learning_narratives[0].contains("first interactions"));
    }

    #[test]
    fn test_cold_start_narrative_items_but_no_calibrations() {
        // When items have been analyzed but no calibrations exist yet,
        // the system should acknowledge processing and prompt engagement.
        let pulse = IntelligencePulse {
            items_analyzed_7d: 250,
            items_surfaced_7d: 0,
            rejection_rate: 100.0,
            calibration_accuracy: 0.0,
            top_calibrations: vec![],
            source_quality: vec![],
            anti_patterns_detected: 0,
            total_cycles: 1,
            learning_narratives: vec![format!(
                "Processed {} items this week \u{2014} interact with results to sharpen your profile",
                250
            )],
        };
        assert_eq!(pulse.learning_narratives.len(), 1);
        assert!(pulse.learning_narratives[0].contains("250 items"));
        assert!(pulse.learning_narratives[0].contains("sharpen your profile"));
    }
}
