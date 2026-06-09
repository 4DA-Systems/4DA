// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Relevance accuracy tracking — measures how well PASIFA predictions match user behavior.
//!
//! Records weekly accuracy metrics so users can see their 4DA getting smarter over time.
//! Powers the "Your Intelligence This Month" card in the Briefing view.

use serde::{Deserialize, Serialize};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AccuracyRecord {
    pub id: i64,
    pub period: String,
    pub total_scored: u32,
    pub total_relevant: u32,
    pub user_confirmed: u32,
    pub user_rejected: u32,
    pub accuracy_pct: f32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IntelligenceReport {
    pub period: String,
    pub accuracy_current: f32,
    pub accuracy_previous: f32,
    pub accuracy_delta: f32,
    pub topics_tracked: u32,
    pub topics_added: u32,
    pub noise_rejected: u32,
    pub noise_rejection_pct: f32,
    pub time_saved_hours: f32,
    pub security_alerts: u32,
    pub security_acted_on: u32,
    pub decisions_recorded: u32,
    pub feedback_signals: u32,
}

// ============================================================================
// SQL Schema
// ============================================================================

#[allow(dead_code)]
pub(crate) const ACCURACY_SQL: &str = "
CREATE TABLE IF NOT EXISTS accuracy_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    period TEXT NOT NULL UNIQUE,
    total_scored INTEGER NOT NULL DEFAULT 0,
    total_relevant INTEGER NOT NULL DEFAULT 0,
    user_confirmed INTEGER DEFAULT 0,
    user_rejected INTEGER DEFAULT 0,
    accuracy_pct REAL DEFAULT 0.0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
";

// ============================================================================
// Core Functions
// ============================================================================

/// Calculate accuracy for a given period based on feedback data.
/// accuracy = confirmed / (confirmed + rejected) if either exists,
/// else fallback to relevant / scored.
pub(crate) fn calculate_accuracy(
    total_scored: u32,
    total_relevant: u32,
    user_confirmed: u32,
    user_rejected: u32,
) -> f32 {
    let feedback_total = user_confirmed + user_rejected;
    if feedback_total > 0 {
        user_confirmed as f32 / feedback_total as f32
    } else if total_scored > 0 {
        total_relevant as f32 / total_scored as f32
    } else {
        0.0
    }
}

/// Estimate time saved based on noise rejected.
/// Assumes average developer spends ~8 seconds evaluating each piece of content.
/// If 4DA rejects 2000 items, that's ~267 minutes = ~4.4 hours saved.
pub(crate) fn estimate_time_saved(noise_rejected: u32) -> f32 {
    let seconds_per_item = 8.0_f32;
    let total_seconds = noise_rejected as f32 * seconds_per_item;
    (total_seconds / 3600.0 * 10.0).round() / 10.0 // Round to 1 decimal
}

/// Generate a monthly intelligence report from accuracy history.
pub(crate) fn generate_report(
    period: &str,
    current: &AccuracyRecord,
    previous: Option<&AccuracyRecord>,
    topics_tracked: u32,
    topics_previous: u32,
    noise_rejected: u32,
    total_scored: u32,
    security_alerts: u32,
    security_acted_on: u32,
    decisions_recorded: u32,
    feedback_signals: u32,
) -> IntelligenceReport {
    let prev_accuracy = previous.map_or(0.0, |p| p.accuracy_pct);

    IntelligenceReport {
        period: period.to_string(),
        accuracy_current: current.accuracy_pct,
        accuracy_previous: prev_accuracy,
        accuracy_delta: current.accuracy_pct - prev_accuracy,
        topics_tracked,
        topics_added: topics_tracked.saturating_sub(topics_previous),
        noise_rejected,
        noise_rejection_pct: if total_scored > 0 {
            (noise_rejected as f32 / total_scored as f32) * 100.0
        } else {
            0.0
        },
        time_saved_hours: estimate_time_saved(noise_rejected),
        security_alerts,
        security_acted_on,
        decisions_recorded,
        feedback_signals,
    }
}

// ============================================================================
// Weekly Accuracy Computation (Background Job)
// ============================================================================

/// Compute and store accuracy for the current week.
/// Called by the monitoring background job on a weekly cadence.
/// Uses feedback table data: relevant=1 is confirmed, relevant=0 is rejected.
pub(crate) fn record_weekly_accuracy(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    let period = chrono::Utc::now().format("%Y-W%W").to_string();

    // Count items scored this week from the authoritative scoring_stats table.
    // Falls back to source_items count if no scoring runs recorded yet.
    let total_scored: u32 = conn
        .query_row(
            "SELECT COALESCE(SUM(total_scored), 0) FROM scoring_stats WHERE created_at >= datetime('now', '-7 days')",
            [],
            |row| row.get::<_, u32>(0),
        )
        .unwrap_or(0);
    let total_scored: u32 = if total_scored > 0 {
        total_scored
    } else {
        conn.query_row(
            "SELECT COUNT(*) FROM source_items WHERE created_at >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
    };

    // Count items that actually passed the scoring threshold this week.
    // Uses scoring_stats (written by the scoring pipeline with real relevant_count)
    // rather than the old "has embedding" proxy which was wildly inaccurate.
    let total_relevant: u32 = conn
        .query_row(
            "SELECT COALESCE(SUM(relevant_count), 0) FROM scoring_stats WHERE created_at >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Count explicit user feedback this week
    let user_confirmed: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM feedback WHERE relevant = 1 AND created_at >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let user_rejected: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM feedback WHERE relevant = 0 AND created_at >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let accuracy_pct =
        calculate_accuracy(total_scored, total_relevant, user_confirmed, user_rejected);

    conn.execute(
        "INSERT OR REPLACE INTO accuracy_history (period, total_scored, total_relevant, user_confirmed, user_rejected, accuracy_pct)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![period, total_scored, total_relevant, user_confirmed, user_rejected, accuracy_pct],
    )?;

    tracing::info!(target: "4da::accuracy", period = %period, accuracy = accuracy_pct, confirmed = user_confirmed, rejected = user_rejected, "Weekly accuracy recorded");

    Ok(())
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_accuracy_report(period: Option<String>) -> crate::error::Result<serde_json::Value> {
    crate::settings::require_signal_feature("get_accuracy_report")?;
    let p = period.unwrap_or_else(|| chrono::Utc::now().format("%Y-%m").to_string());
    let conn = crate::get_database()?.conn.lock();

    // Get current period record
    let current_opt: Option<AccuracyRecord> = conn
        .query_row(
            "SELECT id, period, total_scored, total_relevant, user_confirmed, user_rejected, accuracy_pct, created_at \
             FROM accuracy_history WHERE period = ?1",
            rusqlite::params![p],
            |row| {
                Ok(AccuracyRecord {
                    id: row.get(0)?,
                    period: row.get(1)?,
                    total_scored: row.get(2)?,
                    total_relevant: row.get(3)?,
                    user_confirmed: row.get(4)?,
                    user_rejected: row.get(5)?,
                    accuracy_pct: row.get(6)?,
                    created_at: row.get(7)?,
                })
            },
        )
        .ok();

    match current_opt {
        Some(current) => Ok(serde_json::to_value(current)?),
        None => Ok(serde_json::json!({
            "period": p,
            "total_scored": 0,
            "total_relevant": 0,
            "user_confirmed": 0,
            "user_rejected": 0,
            "accuracy_pct": 0.0,
            "message": "No data for this period yet"
        })),
    }
}

#[tauri::command]
pub fn get_intelligence_report(period: Option<String>) -> crate::error::Result<serde_json::Value> {
    crate::settings::require_signal_feature("get_intelligence_report")?;
    let p = period.unwrap_or_else(|| chrono::Utc::now().format("%Y-%m").to_string());
    let conn = crate::get_database()?.conn.lock();

    // Get current and previous period
    let mut stmt = conn.prepare(
        "SELECT id, period, total_scored, total_relevant, user_confirmed, user_rejected, accuracy_pct, created_at \
         FROM accuracy_history ORDER BY period DESC LIMIT 2",
    )?;
    let records: Vec<AccuracyRecord> = stmt
        .query_map([], |row| {
            Ok(AccuracyRecord {
                id: row.get(0)?,
                period: row.get(1)?,
                total_scored: row.get(2)?,
                total_relevant: row.get(3)?,
                user_confirmed: row.get(4)?,
                user_rejected: row.get(5)?,
                accuracy_pct: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .filter_map(std::result::Result::ok)
        .collect();

    if records.is_empty() {
        return Ok(serde_json::json!({
            "period": p,
            "accuracy_current": 0.0,
            "accuracy_previous": 0.0,
            "accuracy_delta": 0.0,
            "message": "No accuracy data yet"
        }));
    }

    let current = &records[0];
    let previous = records.get(1);
    let report = generate_report(
        &p,
        current,
        previous,
        0,
        0, // topics tracked/previous — placeholder
        current.total_scored.saturating_sub(current.total_relevant),
        current.total_scored,
        0,
        0, // security alerts — placeholder
        0, // decisions
        (current.user_confirmed + current.user_rejected) as u32,
    );
    Ok(serde_json::to_value(report)?)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accuracy_with_feedback() {
        let acc = calculate_accuracy(100, 80, 70, 10);
        assert!((acc - 0.875).abs() < 0.001); // 70 / (70 + 10)
    }

    #[test]
    fn test_accuracy_without_feedback() {
        let acc = calculate_accuracy(100, 80, 0, 0);
        assert!((acc - 0.80).abs() < 0.001); // 80 / 100
    }

    #[test]
    fn test_accuracy_zero_scored() {
        let acc = calculate_accuracy(0, 0, 0, 0);
        assert_eq!(acc, 0.0);
    }

    #[test]
    fn test_time_saved_estimate() {
        let hours = estimate_time_saved(2400);
        assert!((hours - 5.3).abs() < 0.1); // 2400 * 8s = 19200s = 5.3h
    }

    #[test]
    fn test_report_generation() {
        let current = AccuracyRecord {
            id: 2,
            period: "2026-03".to_string(),
            total_scored: 500,
            total_relevant: 100,
            user_confirmed: 85,
            user_rejected: 15,
            accuracy_pct: 0.85,
            created_at: "2026-03-31".to_string(),
        };
        let previous = AccuracyRecord {
            id: 1,
            period: "2026-02".to_string(),
            total_scored: 400,
            total_relevant: 70,
            user_confirmed: 50,
            user_rejected: 20,
            accuracy_pct: 0.714,
            created_at: "2026-02-28".to_string(),
        };

        let report = generate_report(
            "2026-03",
            &current,
            Some(&previous),
            19,
            14,
            2847,
            3000,
            3,
            2,
            7,
            142,
        );

        assert_eq!(report.period, "2026-03");
        assert!(report.accuracy_delta > 0.0); // Improved
        assert_eq!(report.topics_added, 5);
        assert_eq!(report.noise_rejected, 2847);
        assert_eq!(report.security_alerts, 3);
        assert_eq!(report.feedback_signals, 142);
    }
}
