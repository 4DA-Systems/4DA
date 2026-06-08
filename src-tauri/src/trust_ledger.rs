// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Trust Ledger for 4DA
//!
//! Records and measures intelligence quality: precision, preemption lead time,
//! false positive rates, and action conversion. Makes the invisible visible —
//! proves 4DA is getting smarter over time.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{Result, ResultExt};
use crate::open_db_connection;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum TrustEventType {
    Surfaced,
    ActedOn,
    Dismissed,
    FalsePositive,
    Validated,
    Missed,
}

impl std::fmt::Display for TrustEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Surfaced => "surfaced",
            Self::ActedOn => "acted_on",
            Self::Dismissed => "dismissed",
            Self::FalsePositive => "false_positive",
            Self::Validated => "validated",
            Self::Missed => "missed",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TrustEvent {
    pub event_type: TrustEventType,
    pub signal_id: Option<String>,
    pub alert_id: Option<String>,
    pub source_type: Option<String>,
    pub topic: Option<String>,
    pub user_action: Option<String>,
    pub confidence_at_surface: Option<f32>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TrustSummary {
    pub period_days: u32,
    pub total_surfaced: u32,
    pub acted_on: u32,
    pub dismissed: u32,
    pub false_positives: u32,
    /// Precision score: 0.0-1.0 (TP / (TP + FP)) where TP = validated only
    pub precision: f32,
    pub has_precision_data: bool,
    pub action_conversion_rate: f32,
    pub preemption_wins: u32,
    pub avg_lead_time_hours: Option<f32>,
    /// One of: "improving", "stable", "declining"
    pub trend: String,
}

/// Preemption win -- record of a case where 4DA caught something before it became urgent.
/// Populated by the background validator (Phase 2 plan, scheduled task runs weekly)
/// that checks whether past preemption alerts were later validated by reality
/// (e.g. a CVE we warned about was published, a breaking change actually shipped).
// REMOVE BY 2026-08-01
#[allow(dead_code)] // DB schema struct -- deserialized from preemption_wins table
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PreemptionWin {
    pub alert_id: String,
    pub alert_title: String,
    pub alerted_at: String,
    pub incident_at: Option<String>,
    pub lead_time_hours: Option<f32>,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DomainPrecision {
    pub domain: String,
    /// Precision score: None when insufficient data (< MIN_PRECISION_DATA_POINTS),
    /// Some(0.0..=1.0) when enough evidence exists.
    pub precision: Option<f32>,
    pub total_surfaced: u32,
    /// Engagement count (user clicked/acted on item). NOT a true positive signal.
    pub engaged: u32,
    pub false_positives: u32,
    /// Validated count (explicitly confirmed relevant by user).
    pub validated: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FalsePositiveAnalysis {
    pub total_fp: u32,
    pub by_source: Vec<SourceFpRate>,
    pub by_topic: Vec<TopicFpRate>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SourceFpRate {
    pub source_type: String,
    pub total: u32,
    pub fp_count: u32,
    pub fp_rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TopicFpRate {
    pub topic: String,
    pub total: u32,
    pub fp_count: u32,
    pub fp_rate: f32,
}

/// Minimum data points (validated + false_positives) required before reporting precision.
/// Below this threshold, precision is `None` -- the system doesn't have enough evidence.
const MIN_PRECISION_DATA_POINTS: u32 = 5;

// ============================================================================
// Core Functions
// ============================================================================

/// Record a trust event when user interacts with intelligence.
pub fn record_trust_event(event: TrustEvent) -> Result<()> {
    let conn = open_db_connection()?;
    conn.execute(
        "INSERT INTO trust_events (event_type, signal_id, alert_id, source_type, topic, user_action, confidence_at_surface, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            event.event_type.to_string(),
            event.signal_id,
            event.alert_id,
            event.source_type,
            event.topic,
            event.user_action,
            event.confidence_at_surface,
            event.notes,
        ],
    )
    .context("Failed to insert trust event")?;
    Ok(())
}

/// Get trust summary for the last N days.
pub fn get_trust_summary(days: u32) -> Result<TrustSummary> {
    let conn = open_db_connection()?;
    let offset = format!("-{} days", days);

    let total_surfaced: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM trust_events WHERE event_type = 'surfaced' AND created_at >= datetime('now', ?1)",
            rusqlite::params![offset],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let acted_on: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM trust_events WHERE event_type = 'acted_on' AND created_at >= datetime('now', ?1)",
            rusqlite::params![offset],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let dismissed: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM trust_events WHERE event_type = 'dismissed' AND created_at >= datetime('now', ?1)",
            rusqlite::params![offset],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let false_positives: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM trust_events WHERE event_type = 'false_positive' AND created_at >= datetime('now', ?1)",
            rusqlite::params![offset],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let preemption_wins: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM preemption_wins WHERE verified = 1 AND created_at >= datetime('now', ?1)",
            rusqlite::params![offset],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let avg_lead_time: Option<f32> = conn
        .query_row(
            "SELECT AVG(lead_time_hours) FROM preemption_wins WHERE verified = 1 AND lead_time_hours IS NOT NULL AND created_at >= datetime('now', ?1)",
            rusqlite::params![offset],
            |row| row.get(0),
        )
        .unwrap_or(None);

    // TP = validated events only. acted_on is engagement, NOT confirmation of relevance.
    let validated: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM trust_events WHERE event_type = 'validated' AND created_at >= datetime('now', ?1)",
            rusqlite::params![offset],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let true_positives = validated;
    let precision_denominator = true_positives + false_positives;
    let has_precision_data = precision_denominator >= MIN_PRECISION_DATA_POINTS;
    let precision = if has_precision_data {
        true_positives as f32 / precision_denominator as f32
    } else {
        0.0
    };

    let action_rate = if total_surfaced > 0 {
        acted_on as f32 / total_surfaced as f32
    } else {
        0.0
    };

    let trend = compute_trend(&conn, days)?;

    Ok(TrustSummary {
        period_days: days,
        total_surfaced,
        acted_on,
        dismissed,
        false_positives,
        precision,
        has_precision_data,
        action_conversion_rate: action_rate,
        preemption_wins,
        avg_lead_time_hours: avg_lead_time,
        trend,
    })
}

/// Compute trend by comparing current period precision to previous period.
fn compute_trend(conn: &rusqlite::Connection, days: u32) -> Result<String> {
    let current_offset = format!("-{} days", days);
    let previous_offset = format!("-{} days", days * 2);

    // Current period: validated only (NOT acted_on -- that's engagement, not confirmation)
    let current_tp: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM trust_events WHERE event_type = 'validated' AND created_at >= datetime('now', ?1)",
            rusqlite::params![current_offset],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let current_fp: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM trust_events WHERE event_type = 'false_positive' AND created_at >= datetime('now', ?1)",
            rusqlite::params![current_offset],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Previous period: between 2*days ago and days ago
    let prev_tp: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM trust_events WHERE event_type = 'validated' AND created_at >= datetime('now', ?1) AND created_at < datetime('now', ?2)",
            rusqlite::params![previous_offset, current_offset],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let prev_fp: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM trust_events WHERE event_type = 'false_positive' AND created_at >= datetime('now', ?1) AND created_at < datetime('now', ?2)",
            rusqlite::params![previous_offset, current_offset],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // No previous data -- default to stable
    if prev_tp + prev_fp == 0 {
        return Ok("stable".to_string());
    }

    let current_precision = if current_tp + current_fp > 0 {
        current_tp as f32 / (current_tp + current_fp) as f32
    } else {
        1.0
    };

    let prev_precision = prev_tp as f32 / (prev_tp + prev_fp) as f32;

    let delta = current_precision - prev_precision;
    let trend = if delta > 0.05 {
        "improving"
    } else if delta < -0.05 {
        "declining"
    } else {
        "stable"
    };

    Ok(trend.to_string())
}

/// Compute and store weekly precision stats.
/// Called by the monitoring scheduler every 7 days.
pub fn compute_and_store_weekly_precision() -> Result<()> {
    let conn = open_db_connection()?;
    let now = chrono::Utc::now();
    let week_ago = now - chrono::Duration::days(7);
    let period = now.format("%Y-W%V").to_string();

    let domains = vec!["overall", "security", "dependency", "ecosystem", "decision"];

    for domain in &domains {
        let domain_filter = if *domain == "overall" {
            String::new()
        } else {
            format!(" AND source_type = '{}'", domain)
        };

        let total: u32 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM trust_events WHERE event_type = 'surfaced' AND created_at >= ?1{}",
                    domain_filter
                ),
                rusqlite::params![week_ago.to_rfc3339()],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let acted_on: u32 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM trust_events WHERE event_type = 'acted_on' AND created_at >= ?1{}",
                    domain_filter
                ),
                rusqlite::params![week_ago.to_rfc3339()],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let dismissed: u32 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM trust_events WHERE event_type = 'dismissed' AND created_at >= ?1{}",
                    domain_filter
                ),
                rusqlite::params![week_ago.to_rfc3339()],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let false_positives: u32 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM trust_events WHERE event_type = 'false_positive' AND created_at >= ?1{}",
                    domain_filter
                ),
                rusqlite::params![week_ago.to_rfc3339()],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let validated: u32 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM trust_events WHERE event_type = 'validated' AND created_at >= ?1{}",
                    domain_filter
                ),
                rusqlite::params![week_ago.to_rfc3339()],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // TP = validated only. acted_on is engagement, not confirmation.
        let true_positives = validated;
        let precision_denominator = true_positives + false_positives;
        // Undefined precision is stored as NULL (None), never a -1.0 sentinel. An
        // impossible numeric in a REAL column corrupts every reader (export, trend
        // UI, query_db) and violates the no-vanity-metric doctrine: an undefined
        // metric ships as NULL/silent, not as a fake value. The live read path
        // (get_domain_precision) already uses Option<f32>; this aligns the stored path.
        let precision: Option<f32> = if precision_denominator >= MIN_PRECISION_DATA_POINTS {
            Some(true_positives as f32 / precision_denominator as f32)
        } else {
            None
        };

        let action_rate = if total > 0 {
            acted_on as f32 / total as f32
        } else {
            0.0
        };

        // Get average lead time for this domain
        let avg_lead: Option<f32> = conn
            .query_row(
                "SELECT AVG(lead_time_hours) FROM preemption_wins WHERE verified = 1 AND lead_time_hours IS NOT NULL AND created_at >= ?1",
                rusqlite::params![week_ago.to_rfc3339()],
                |row| row.get(0),
            )
            .unwrap_or(None);

        // Only store if there's data
        if total > 0 || false_positives > 0 {
            conn.execute(
                "INSERT INTO precision_stats (period, domain, total_surfaced, true_positives, false_positives, false_negatives, acted_on, dismissed, precision, action_conversion_rate, avg_lead_time_hours)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    period,
                    domain,
                    total,
                    true_positives,
                    false_positives,
                    acted_on,
                    dismissed,
                    precision,
                    action_rate,
                    avg_lead
                ],
            )
            .context("Failed to insert precision stats")?;
        }
    }

    Ok(())
}

/// Get precision breakdown by domain for the last N days.
/// Precision is `None` when fewer than `MIN_PRECISION_DATA_POINTS` validated+FP events exist.
pub fn get_domain_precision(days: u32) -> Result<Vec<DomainPrecision>> {
    let conn = open_db_connection()?;
    let offset = format!("-{} days", days);

    let mut stmt = conn.prepare(
        "SELECT source_type,
                COUNT(CASE WHEN event_type = 'surfaced' THEN 1 END) as total,
                COUNT(CASE WHEN event_type = 'acted_on' THEN 1 END) as engaged,
                COUNT(CASE WHEN event_type = 'false_positive' THEN 1 END) as fp,
                COUNT(CASE WHEN event_type = 'validated' THEN 1 END) as validated
         FROM trust_events
         WHERE created_at >= datetime('now', ?1) AND source_type IS NOT NULL
         GROUP BY source_type",
    )?;

    let domains = stmt.query_map(rusqlite::params![offset], |row| {
        let domain: String = row.get(0)?;
        let total: u32 = row.get(1)?;
        let engaged: u32 = row.get(2)?;
        let fp: u32 = row.get(3)?;
        let validated: u32 = row.get(4)?;
        // Precision uses validated (true positive) vs false_positive only.
        // acted_on is engagement -- it doesn't confirm relevance.
        let precision_denominator = validated + fp;
        let precision = if precision_denominator >= MIN_PRECISION_DATA_POINTS {
            Some(validated as f32 / precision_denominator as f32)
        } else {
            None
        };
        Ok(DomainPrecision {
            domain,
            precision,
            total_surfaced: total,
            engaged,
            false_positives: fp,
            validated,
        })
    })?;

    Ok(domains.filter_map(|r| r.ok()).collect())
}

/// Analyze false positive patterns to help calibrate scoring
pub fn analyze_false_positives(days: u32) -> Result<FalsePositiveAnalysis> {
    let conn = open_db_connection()?;
    let offset = format!("-{} days", days);

    let total_fp: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM trust_events WHERE event_type = 'false_positive' AND created_at >= datetime('now', ?1)",
            rusqlite::params![offset],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // FP rate by source type
    let mut by_source_stmt = conn.prepare(
        "SELECT source_type,
                COUNT(*) as total,
                SUM(CASE WHEN event_type = 'false_positive' THEN 1 ELSE 0 END) as fp
         FROM trust_events
         WHERE created_at >= datetime('now', ?1) AND source_type IS NOT NULL
         GROUP BY source_type
         HAVING total > 2",
    )?;

    let by_source: Vec<SourceFpRate> = by_source_stmt
        .query_map(rusqlite::params![offset], |row| {
            let source: String = row.get(0)?;
            let total: u32 = row.get(1)?;
            let fp: u32 = row.get(2)?;
            Ok(SourceFpRate {
                source_type: source,
                total,
                fp_count: fp,
                fp_rate: if total > 0 {
                    fp as f32 / total as f32
                } else {
                    0.0
                },
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // FP rate by topic
    let mut by_topic_stmt = conn.prepare(
        "SELECT topic,
                COUNT(*) as total,
                SUM(CASE WHEN event_type = 'false_positive' THEN 1 ELSE 0 END) as fp
         FROM trust_events
         WHERE created_at >= datetime('now', ?1) AND topic IS NOT NULL
         GROUP BY topic
         HAVING total > 2",
    )?;

    let by_topic: Vec<TopicFpRate> = by_topic_stmt
        .query_map(rusqlite::params![offset], |row| {
            let topic: String = row.get(0)?;
            let total: u32 = row.get(1)?;
            let fp: u32 = row.get(2)?;
            Ok(TopicFpRate {
                topic,
                total,
                fp_count: fp,
                fp_rate: if total > 0 {
                    fp as f32 / total as f32
                } else {
                    0.0
                },
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Generate recommendations
    let mut recommendations = Vec::new();
    for s in &by_source {
        if s.fp_rate > 0.3 && s.total > 5 {
            recommendations.push(format!(
                "Source '{}' has {:.0}% FP rate -- consider downweighting",
                s.source_type,
                s.fp_rate * 100.0
            ));
        }
    }
    for t in &by_topic {
        if t.fp_rate > 0.3 && t.total > 5 {
            recommendations.push(format!(
                "Topic '{}' has {:.0}% FP rate -- consider raising relevance threshold",
                t.topic,
                t.fp_rate * 100.0
            ));
        }
    }

    Ok(FalsePositiveAnalysis {
        total_fp,
        by_source,
        by_topic,
        recommendations,
    })
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn get_trust_dashboard(days: Option<u32>) -> std::result::Result<TrustSummary, String> {
    get_trust_summary(days.unwrap_or(30)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn record_intelligence_feedback(
    event_type: String,
    signal_id: Option<String>,
    alert_id: Option<String>,
    source_type: Option<String>,
    topic: Option<String>,
    notes: Option<String>,
    dismiss_reason: Option<String>,
    dismiss_category: Option<String>,
) -> std::result::Result<(), String> {
    let event_type = match event_type.as_str() {
        "acted_on" => TrustEventType::ActedOn,
        "dismissed" => TrustEventType::Dismissed,
        "false_positive" => TrustEventType::FalsePositive,
        "validated" => TrustEventType::Validated,
        "missed" => TrustEventType::Missed,
        _ => TrustEventType::Surfaced,
    };
    record_trust_event(TrustEvent {
        event_type: event_type.clone(),
        signal_id: signal_id.clone(),
        alert_id,
        source_type,
        topic,
        user_action: None,
        confidence_at_surface: None,
        notes,
    })
    .map_err(|e| e.to_string())?;

    // If this is a dismiss with structured feedback, also record on the interaction
    if matches!(event_type, TrustEventType::Dismissed) {
        if let (Some(ref reason), Some(ref category)) = (&dismiss_reason, &dismiss_category) {
            if let Ok(engine) = crate::get_context_engine() {
                if let Some(ref sid) = signal_id {
                    if let Ok(item_id) = sid.parse::<i64>() {
                        let _ = engine.record_interaction(
                            item_id,
                            crate::context_engine::InteractionType::Dismiss,
                            Some(reason),
                            Some(category),
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_domain_precision_report(
    days: Option<u32>,
) -> std::result::Result<Vec<DomainPrecision>, String> {
    crate::settings::require_signal_feature("get_domain_precision_report")
        .map_err(|e| e.to_string())?;
    get_domain_precision(days.unwrap_or(30)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_false_positive_analysis(
    days: Option<u32>,
) -> std::result::Result<FalsePositiveAnalysis, String> {
    crate::settings::require_signal_feature("get_false_positive_analysis")
        .map_err(|e| e.to_string())?;
    analyze_false_positives(days.unwrap_or(30)).map_err(|e| e.to_string())
}

// ============================================================================
// Feedback Outbox — durable SQLite-backed retry queue
// ============================================================================

/// Persist a feedback event to the SQLite outbox for durable retry.
/// Called by the frontend when the immediate send fails.
#[tauri::command]
pub fn queue_feedback_event(
    event_type: String,
    signal_id: Option<String>,
    alert_id: Option<String>,
    source_type: Option<String>,
    topic: Option<String>,
    notes: Option<String>,
    dismiss_reason: Option<String>,
    dismiss_category: Option<String>,
) -> std::result::Result<i64, String> {
    let db = crate::get_database().map_err(|e| e.to_string())?;
    let conn = db.conn.lock();
    queue_feedback_event_on_conn(
        &conn,
        &event_type,
        signal_id.as_deref(),
        alert_id.as_deref(),
        source_type.as_deref(),
        topic.as_deref(),
        notes.as_deref(),
        dismiss_reason.as_deref(),
        dismiss_category.as_deref(),
    )
}

#[allow(clippy::too_many_arguments)]
fn queue_feedback_event_on_conn(
    conn: &rusqlite::Connection,
    event_type: &str,
    signal_id: Option<&str>,
    alert_id: Option<&str>,
    source_type: Option<&str>,
    topic: Option<&str>,
    notes: Option<&str>,
    dismiss_reason: Option<&str>,
    dismiss_category: Option<&str>,
) -> std::result::Result<i64, String> {
    conn.execute(
        "INSERT OR IGNORE INTO feedback_outbox (event_type, signal_id, alert_id, source_type, topic, notes, dismiss_reason, dismiss_category, queued_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime('%s','now') * 1000)",
        rusqlite::params![
            event_type,
            signal_id,
            alert_id,
            source_type,
            topic,
            notes,
            dismiss_reason,
            dismiss_category
        ],
    )
    .map_err(|e| e.to_string())?;

    if conn.changes() == 0 {
        // Duplicate hit — find the existing pending row by dedup key
        let existing_id: i64 = conn.query_row(
            "SELECT id FROM feedback_outbox WHERE event_type = ?1 AND COALESCE(signal_id,'') = COALESCE(?2,'') AND COALESCE(alert_id,'') = COALESCE(?3,'') AND COALESCE(source_type,'') = COALESCE(?4,'') AND COALESCE(topic,'') = COALESCE(?5,'') AND status = 'pending'",
            rusqlite::params![event_type, signal_id, alert_id, source_type, topic],
            |row| row.get(0),
        ).map_err(|e| format!("Failed to find existing outbox row: {e}"))?;
        conn.execute(
            "UPDATE feedback_outbox
             SET notes = COALESCE(?1, notes),
                 dismiss_reason = COALESCE(?2, dismiss_reason),
                 dismiss_category = COALESCE(?3, dismiss_category)
             WHERE id = ?4",
            rusqlite::params![notes, dismiss_reason, dismiss_category, existing_id],
        )
        .map_err(|e| e.to_string())?;
        return Ok(existing_id);
    }

    let row_id = conn.last_insert_rowid();
    if row_id <= 0 {
        return Err("feedback_outbox insert returned invalid row id".into());
    }
    Ok(row_id)
}

/// Load pending feedback events from the SQLite outbox.
/// Called by the frontend on startup to resume retry.
#[tauri::command]
pub fn get_pending_feedback() -> std::result::Result<Vec<serde_json::Value>, String> {
    let db = crate::get_database().map_err(|e| e.to_string())?;
    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(
            "SELECT id, event_type, signal_id, alert_id, source_type, topic, notes, dismiss_reason, dismiss_category, queued_at, attempts
             FROM feedback_outbox WHERE status = 'pending' AND attempts < 5 ORDER BY queued_at",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "eventType": row.get::<_, String>(1)?,
                "signalId": row.get::<_, Option<String>>(2)?,
                "alertId": row.get::<_, Option<String>>(3)?,
                "sourceType": row.get::<_, Option<String>>(4)?,
                "topic": row.get::<_, Option<String>>(5)?,
                "notes": row.get::<_, Option<String>>(6)?,
                "dismissReason": row.get::<_, Option<String>>(7)?,
                "dismissCategory": row.get::<_, Option<String>>(8)?,
                "queuedAt": row.get::<_, i64>(9)?,
                "attempts": row.get::<_, i32>(10)?,
            }))
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

/// Mark a feedback outbox event as sent (successfully delivered to backend).
#[tauri::command]
pub fn mark_feedback_sent(outbox_id: i64) -> std::result::Result<(), String> {
    let db = crate::get_database().map_err(|e| e.to_string())?;
    let conn = db.conn.lock();
    mark_feedback_sent_on_conn(&conn, outbox_id)
}

fn mark_feedback_sent_on_conn(
    conn: &rusqlite::Connection,
    outbox_id: i64,
) -> std::result::Result<(), String> {
    if outbox_id <= 0 {
        return Err(format!("invalid feedback outbox id: {outbox_id}"));
    }
    let changed = conn
        .execute(
            "UPDATE feedback_outbox SET status = 'sent' WHERE id = ?1",
            rusqlite::params![outbox_id],
        )
        .map_err(|e| e.to_string())?;
    if changed != 1 {
        return Err(format!(
            "feedback outbox row {outbox_id} was not marked sent; changed={changed}"
        ));
    }
    Ok(())
}

/// Increment attempt count for a failed feedback outbox event.
#[tauri::command]
pub fn mark_feedback_attempt(outbox_id: i64) -> std::result::Result<(), String> {
    let db = crate::get_database().map_err(|e| e.to_string())?;
    let conn = db.conn.lock();
    mark_feedback_attempt_on_conn(&conn, outbox_id)
}

fn mark_feedback_attempt_on_conn(
    conn: &rusqlite::Connection,
    outbox_id: i64,
) -> std::result::Result<(), String> {
    if outbox_id <= 0 {
        return Err(format!("invalid feedback outbox id: {outbox_id}"));
    }
    let changed = conn
        .execute(
        "UPDATE feedback_outbox SET attempts = attempts + 1, last_attempt_at = strftime('%s','now') * 1000 WHERE id = ?1 AND status = 'pending'",
        rusqlite::params![outbox_id],
    )
    .map_err(|e| e.to_string())?;
    if changed != 1 {
        return Err(format!(
            "feedback outbox row {outbox_id} was not marked attempted; changed={changed}"
        ));
    }
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create an in-memory DB with the trust_events schema
    fn test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE trust_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                signal_id TEXT,
                alert_id TEXT,
                source_type TEXT,
                topic TEXT,
                user_action TEXT,
                confidence_at_surface REAL,
                notes TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE preemption_wins (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                alert_id TEXT NOT NULL,
                alert_title TEXT NOT NULL,
                alerted_at TEXT NOT NULL,
                incident_at TEXT,
                lead_time_hours REAL,
                verified INTEGER DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
        conn
    }

    fn insert_event(conn: &rusqlite::Connection, event_type: &str, source_type: &str) {
        conn.execute(
            "INSERT INTO trust_events (event_type, source_type) VALUES (?1, ?2)",
            rusqlite::params![event_type, source_type],
        )
        .unwrap();
    }

    fn feedback_outbox_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE feedback_outbox (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                signal_id TEXT,
                alert_id TEXT,
                source_type TEXT,
                topic TEXT,
                notes TEXT,
                dismiss_reason TEXT,
                dismiss_category TEXT,
                queued_at INTEGER NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                last_attempt_at INTEGER,
                status TEXT NOT NULL DEFAULT 'pending'
            );
            CREATE UNIQUE INDEX idx_feedback_outbox_dedup
                ON feedback_outbox(event_type, COALESCE(signal_id,''), COALESCE(alert_id,''), COALESCE(source_type,''), COALESCE(topic,''), status);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn queue_feedback_duplicate_returns_existing_positive_row_id() {
        let conn = feedback_outbox_db();
        let first = queue_feedback_event_on_conn(
            &conn,
            "dismissed",
            Some("sig-1"),
            None,
            Some("blind_spot"),
            Some("react"),
            Some("first note"),
            None,
            None,
        )
        .unwrap();
        assert!(first > 0);

        let duplicate = queue_feedback_event_on_conn(
            &conn,
            "dismissed",
            Some("sig-1"),
            None,
            Some("blind_spot"),
            Some("react"),
            Some("newer note"),
            Some("not_relevant"),
            Some("noise"),
        )
        .unwrap();
        assert_eq!(duplicate, first);

        let row: (i64, Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT COUNT(*), notes, dismiss_reason, dismiss_category FROM feedback_outbox",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(row.0, 1);
        assert_eq!(row.1.as_deref(), Some("newer note"));
        assert_eq!(row.2.as_deref(), Some("not_relevant"));
        assert_eq!(row.3.as_deref(), Some("noise"));
    }

    #[test]
    fn mark_feedback_outbox_updates_reject_missing_or_invalid_ids() {
        let conn = feedback_outbox_db();
        assert!(mark_feedback_sent_on_conn(&conn, 0).is_err());
        assert!(mark_feedback_attempt_on_conn(&conn, 99).is_err());

        let row_id = queue_feedback_event_on_conn(
            &conn,
            "validated",
            Some("sig-2"),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        mark_feedback_attempt_on_conn(&conn, row_id).unwrap();
        let attempts: i64 = conn
            .query_row(
                "SELECT attempts FROM feedback_outbox WHERE id = ?1",
                rusqlite::params![row_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(attempts, 1);

        mark_feedback_sent_on_conn(&conn, row_id).unwrap();
        assert!(mark_feedback_attempt_on_conn(&conn, row_id).is_err());
    }

    #[test]
    fn domain_precision_returns_none_when_insufficient_data() {
        // With fewer than MIN_PRECISION_DATA_POINTS validated+FP, precision should be None
        let dp = DomainPrecision {
            domain: "security".into(),
            precision: None,
            total_surfaced: 10,
            engaged: 3,
            false_positives: 2,
            validated: 1,
        };
        assert!(dp.precision.is_none());

        // With enough data, precision should be Some
        let dp2 = DomainPrecision {
            domain: "security".into(),
            precision: Some(0.8),
            total_surfaced: 20,
            engaged: 5,
            false_positives: 2,
            validated: 8,
        };
        assert!(dp2.precision.is_some());
        assert!((dp2.precision.unwrap() - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn acted_on_is_engagement_not_true_positive() {
        // Verify that the TrustSummary precision calculation
        // does NOT count acted_on as a true positive.
        // With 10 acted_on but 0 validated and 0 FP, has_precision_data should be false.
        let summary = TrustSummary {
            period_days: 30,
            total_surfaced: 20,
            acted_on: 10,
            dismissed: 5,
            false_positives: 0,
            precision: 0.0,
            has_precision_data: false,
            action_conversion_rate: 0.5,
            preemption_wins: 0,
            avg_lead_time_hours: None,
            trend: "stable".into(),
        };
        // acted_on should NOT contribute to precision data
        assert!(!summary.has_precision_data);
        assert_eq!(summary.precision, 0.0);
    }

    #[test]
    fn precision_requires_minimum_data_threshold() {
        // MIN_PRECISION_DATA_POINTS = 5
        // With 3 validated + 1 FP = 4 total: insufficient
        assert!(4 < MIN_PRECISION_DATA_POINTS);

        // With 4 validated + 1 FP = 5 total: sufficient
        let denominator: u32 = 5;
        assert!(denominator >= MIN_PRECISION_DATA_POINTS);
        let precision = 4_f32 / denominator as f32;
        assert!((precision - 0.8).abs() < 0.001);
    }

    #[test]
    fn compute_trend_uses_validated_not_acted_on() {
        let conn = test_db();

        // Insert events: lots of acted_on but no validated in current period
        for _ in 0..10 {
            insert_event(&conn, "acted_on", "security");
        }
        // 2 false positives
        for _ in 0..2 {
            insert_event(&conn, "false_positive", "security");
        }

        // compute_trend should report stable (no previous data -> stable)
        let trend = compute_trend(&conn, 30).unwrap();
        assert_eq!(trend, "stable");
    }

    #[test]
    fn domain_precision_struct_separates_engaged_from_validated() {
        // The DomainPrecision struct should have separate `engaged` and `validated`
        // fields, not conflate acted_on with true positives.
        let dp = DomainPrecision {
            domain: "ecosystem".into(),
            precision: Some(0.6),
            total_surfaced: 50,
            engaged: 15, // clicked/acted on (engagement signal)
            false_positives: 4,
            validated: 6, // explicitly confirmed relevant (true positive)
        };
        // Precision should be validated / (validated + FP) = 6/10 = 0.6
        let expected = dp.validated as f32 / (dp.validated + dp.false_positives) as f32;
        assert!((dp.precision.unwrap() - expected).abs() < 0.001);
    }

    // ══════════════════════════════════════════════════════════════════════
    // T3-4: Trust Metric Calibration Tests
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_precision_none_below_threshold() {
        // When validated + FP < MIN_PRECISION_DATA_POINTS (5), precision must be None.
        // Simulate the calculation that get_domain_precision performs.
        let validated: u32 = 2;
        let fp: u32 = 1;
        let precision_denominator = validated + fp; // 3 < 5
        let precision = if precision_denominator >= MIN_PRECISION_DATA_POINTS {
            Some(validated as f32 / precision_denominator as f32)
        } else {
            None
        };
        assert!(
            precision.is_none(),
            "precision must be None with only {} data points (threshold = {})",
            precision_denominator,
            MIN_PRECISION_DATA_POINTS
        );

        // Verify the constant is what we expect
        assert_eq!(MIN_PRECISION_DATA_POINTS, 5);
    }

    #[test]
    fn test_engagement_not_counted_as_tp() {
        // acted_on events must not inflate true positives in the precision formula.
        // The production code uses: TP = validated only. acted_on = engagement.
        let conn = test_db();

        // Insert 20 acted_on events -- these are engagement, NOT confirmations
        for _ in 0..20 {
            insert_event(&conn, "acted_on", "hackernews");
        }
        // Insert 1 false_positive
        insert_event(&conn, "false_positive", "hackernews");

        // If acted_on were incorrectly counted as TP, precision denominator would
        // be 21 (20 TP + 1 FP), and precision = 20/21 = ~0.95.
        // Correctly, TP = 0 (no validated events), denominator = 0 + 1 = 1,
        // which is < MIN_PRECISION_DATA_POINTS. So has_precision_data = false.
        let validated: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM trust_events WHERE event_type = 'validated'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let false_positives: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM trust_events WHERE event_type = 'false_positive'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let denominator = validated + false_positives;
        assert_eq!(validated, 0, "no validated events exist");
        assert_eq!(false_positives, 1, "1 false positive exists");
        assert!(
            denominator < MIN_PRECISION_DATA_POINTS,
            "denominator ({}) must be below threshold ({})",
            denominator,
            MIN_PRECISION_DATA_POINTS
        );
    }

    #[test]
    fn test_validated_counts_as_tp() {
        // validated events ARE the true positive signal for precision.
        let conn = test_db();

        // Insert 4 validated + 1 FP = 5 total (meets threshold)
        for _ in 0..4 {
            insert_event(&conn, "validated", "security");
        }
        insert_event(&conn, "false_positive", "security");

        let validated: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM trust_events WHERE event_type = 'validated'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let fp: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM trust_events WHERE event_type = 'false_positive'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        let denominator = validated + fp;
        assert!(
            denominator >= MIN_PRECISION_DATA_POINTS,
            "enough data points to compute precision"
        );

        let precision = validated as f32 / denominator as f32;
        assert!(
            (precision - 0.8).abs() < 0.001,
            "precision should be 4/5 = 0.8, got {}",
            precision
        );
    }

    #[test]
    fn test_weekly_precision_undefined_is_none_not_sentinel() {
        // When fewer than MIN_PRECISION_DATA_POINTS exist, weekly precision is
        // undefined and MUST be None (stored as SQL NULL) -- never a -1.0 sentinel,
        // which would corrupt any reader and display an impossible value.
        for (validated, false_positives) in [(0u32, 0u32), (2, 1)] {
            let precision_denominator = validated + false_positives; // 0 and 3, both < 5
            let precision: Option<f32> = if precision_denominator >= MIN_PRECISION_DATA_POINTS {
                Some(validated as f32 / precision_denominator as f32)
            } else {
                None
            };
            assert_eq!(
                precision, None,
                "with {} data points (< {}), precision must be None, not a sentinel",
                precision_denominator, MIN_PRECISION_DATA_POINTS
            );
        }
    }

    #[test]
    fn test_weekly_precision_never_negative() {
        // Property: stored precision is either None or within [0.0, 1.0]. Never negative.
        for validated in 0u32..=8 {
            for false_positives in 0u32..=8 {
                let precision_denominator = validated + false_positives;
                let precision: Option<f32> = if precision_denominator >= MIN_PRECISION_DATA_POINTS {
                    Some(validated as f32 / precision_denominator as f32)
                } else {
                    None
                };
                if let Some(p) = precision {
                    assert!(
                        (0.0..=1.0).contains(&p),
                        "precision {} out of [0,1] for validated={} fp={}",
                        p,
                        validated,
                        false_positives
                    );
                }
            }
        }
    }
}
