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
    /// Precision score: 0.0–1.0 (TP / (TP + FP))
    pub precision: f32,
    pub action_conversion_rate: f32,
    pub preemption_wins: u32,
    pub avg_lead_time_hours: Option<f32>,
    /// One of: "improving", "stable", "declining"
    pub trend: String,
}

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

    // TP = acted_on + validated events
    let validated: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM trust_events WHERE event_type = 'validated' AND created_at >= datetime('now', ?1)",
            rusqlite::params![offset],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let true_positives = acted_on + validated;
    let precision = if true_positives + false_positives > 0 {
        true_positives as f32 / (true_positives + false_positives) as f32
    } else {
        1.0 // No data yet — assume perfect until proven otherwise
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
        action_conversion_rate: action_rate,
        preemption_wins,
        avg_lead_time_hours: avg_lead_time,
        trend,
    })
}

/// Record a preemption win (4DA caught something before it became urgent).
pub fn record_preemption_win(win: PreemptionWin) -> Result<()> {
    let conn = open_db_connection()?;
    conn.execute(
        "INSERT INTO preemption_wins (alert_id, alert_title, alerted_at, incident_at, lead_time_hours, user_acted, verified)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
        rusqlite::params![
            win.alert_id,
            win.alert_title,
            win.alerted_at,
            win.incident_at,
            win.lead_time_hours,
            win.verified as i32,
        ],
    )
    .context("Failed to insert preemption win")?;
    Ok(())
}

/// Compute trend by comparing current period precision to previous period.
fn compute_trend(conn: &rusqlite::Connection, days: u32) -> Result<String> {
    let current_offset = format!("-{} days", days);
    let previous_offset = format!("-{} days", days * 2);

    // Current period: acted_on + validated vs false_positives
    let current_tp: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM trust_events WHERE event_type IN ('acted_on', 'validated') AND created_at >= datetime('now', ?1)",
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
            "SELECT COUNT(*) FROM trust_events WHERE event_type IN ('acted_on', 'validated') AND created_at >= datetime('now', ?1) AND created_at < datetime('now', ?2)",
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

    // No previous data — default to stable
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

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn get_trust_dashboard(
    days: Option<u32>,
) -> std::result::Result<TrustSummary, String> {
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
        event_type,
        signal_id,
        alert_id,
        source_type,
        topic,
        user_action: None,
        confidence_at_surface: None,
        notes,
    })
    .map_err(|e| e.to_string())
}
