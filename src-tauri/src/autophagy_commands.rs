// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tauri commands for the Intelligent Autophagy system.
//!
//! Exposes autophagy status, history, and manual trigger to the frontend.

use serde::Serialize;
use tracing::info;
use ts_rs::TS;

use crate::autophagy::AutophagyCycleResult;
use crate::error::{FourDaError, Result, ResultExt};

/// Autophagy system status overview.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AutophagyStatus {
    pub last_cycle: Option<AutophagyCycleResult>,
    pub total_cycles: i64,
    pub total_calibrations: i64,
    pub total_anti_patterns: i64,
}

/// Get current autophagy status: latest cycle result and aggregate stats.
#[tauri::command]
pub async fn get_autophagy_status() -> Result<AutophagyStatus> {
    let conn = crate::open_db_connection()?;

    // Get the latest cycle result
    let last_cycle = conn
        .query_row(
            "SELECT items_analyzed, items_pruned, calibrations_produced,
                    topic_decay_rates_updated, source_autopsies_produced,
                    anti_patterns_detected, duration_ms
             FROM autophagy_cycles
             ORDER BY created_at DESC
             LIMIT 1",
            [],
            |row| {
                Ok(AutophagyCycleResult {
                    items_analyzed: row.get(0)?,
                    items_pruned: row.get(1)?,
                    calibrations_produced: row.get(2)?,
                    topic_decay_rates_updated: row.get(3)?,
                    source_autopsies_produced: row.get(4)?,
                    anti_patterns_detected: row.get(5)?,
                    decision_outcomes_analyzed: 0, // Not stored in autophagy_cycles table
                    duration_ms: row.get(6)?,
                })
            },
        )
        .ok();

    // Count total cycles
    let total_cycles: i64 = conn
        .query_row("SELECT COUNT(*) FROM autophagy_cycles", [], |r| r.get(0))
        .unwrap_or(0);

    // Count active (non-superseded) calibrations
    let total_calibrations: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digested_intelligence
             WHERE digest_type = 'calibration' AND superseded_by IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // Count active (non-superseded) anti-patterns
    let total_anti_patterns: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digested_intelligence
             WHERE digest_type = 'anti_pattern' AND superseded_by IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    Ok(AutophagyStatus {
        last_cycle,
        total_cycles,
        total_calibrations,
        total_anti_patterns,
    })
}

/// Get autophagy cycle history (most recent first).
#[tauri::command]
pub async fn get_autophagy_history(limit: Option<i64>) -> Result<Vec<AutophagyCycleResult>> {
    let conn = crate::open_db_connection()?;
    let limit = limit.unwrap_or(10).min(100);

    let mut stmt = conn
        .prepare(
            "SELECT items_analyzed, items_pruned, calibrations_produced,
                    topic_decay_rates_updated, source_autopsies_produced,
                    anti_patterns_detected, duration_ms
             FROM autophagy_cycles
             ORDER BY created_at DESC
             LIMIT ?1",
        )
        .map_err(FourDaError::Db)?;

    let results = stmt
        .query_map(rusqlite::params![limit], |row| {
            Ok(AutophagyCycleResult {
                items_analyzed: row.get(0)?,
                items_pruned: row.get(1)?,
                calibrations_produced: row.get(2)?,
                topic_decay_rates_updated: row.get(3)?,
                source_autopsies_produced: row.get(4)?,
                anti_patterns_detected: row.get(5)?,
                decision_outcomes_analyzed: 0, // Not stored in autophagy_cycles table
                duration_ms: row.get(6)?,
            })
        })
        .map_err(FourDaError::Db)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(FourDaError::Db)?;

    Ok(results)
}

/// Manually trigger an autophagy cycle. Returns the cycle result.
/// Mirrors the logic in monitoring.rs but exposed for on-demand use.
#[tauri::command]
pub async fn trigger_autophagy_cycle() -> Result<AutophagyCycleResult> {
    let max_age_days = {
        let sm = crate::get_settings_manager().lock();
        sm.get().monitoring.cleanup_max_age_days.unwrap_or(30)
    };

    let conn = crate::open_db_connection()?;

    let cycle = crate::autophagy::run_autophagy_cycle(&conn, max_age_days as i64)?;

    info!(
        target: "4da::autophagy",
        items_analyzed = cycle.items_analyzed,
        calibrations = cycle.calibrations_produced,
        anti_patterns = cycle.anti_patterns_detected,
        duration_ms = cycle.duration_ms,
        "Manual autophagy cycle completed"
    );

    // GAME: track calibrations produced
    if cycle.calibrations_produced > 0 {
        if let Ok(db) = crate::get_database() {
            let _unlocked = crate::achievement_engine::increment_counter(
                db,
                "calibrations",
                cycle.calibrations_produced as u64,
            );
        }
    }

    // Bridge accuracy feedback from ACE
    if let Ok(ace) = crate::state::get_ace_engine() {
        let ace_conn = ace.get_conn().lock();
        if let Err(e) =
            crate::autophagy::bridge_accuracy_feedback(&ace_conn, &conn, max_age_days as i64)
        {
            tracing::warn!(target: "4da::autophagy", error = %e, "Accuracy feedback bridge failed");
        }
    }

    Ok(cycle)
}

// ============================================================================
// Data Health
// ============================================================================

/// Data health overview: DB stats + retention settings.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DataHealth {
    pub stats: crate::db::DbStats,
    pub retention_days: u32,
    pub db_size_mb: f64,
    pub health_status: String, // "healthy", "growing", "needs_attention"
}

/// Get data health overview.
#[tauri::command]
pub async fn get_data_health() -> Result<DataHealth> {
    let db = crate::get_database().context("database not initialized")?;
    let stats = db.get_db_stats().map_err(FourDaError::Db)?;

    let retention_days = {
        let sm = crate::get_settings_manager().lock();
        sm.get().monitoring.cleanup_max_age_days.unwrap_or(30)
    };

    let db_size_mb = stats.db_size_bytes as f64 / (1024.0 * 1024.0);

    let health_status = if db_size_mb > 500.0 || stats.source_items > 100_000 {
        "needs_attention".to_string()
    } else if db_size_mb > 200.0 || stats.source_items > 50_000 {
        "growing".to_string()
    } else {
        "healthy".to_string()
    };

    Ok(DataHealth {
        stats,
        retention_days,
        db_size_mb: (db_size_mb * 10.0).round() / 10.0,
        health_status,
    })
}

/// Run deep database clean: prune all tables + VACUUM.
#[tauri::command]
pub async fn run_deep_clean() -> Result<crate::db::MaintenanceResult> {
    let retention_days = {
        let sm = crate::get_settings_manager().lock();
        sm.get().monitoring.cleanup_max_age_days.unwrap_or(30)
    };

    let db = crate::get_database().context("database not initialized")?;
    let result = db
        .run_maintenance(retention_days as i64)
        .map_err(FourDaError::Db)?;

    info!(
        target: "4da::autophagy",
        deleted_items = result.deleted_items,
        deleted_feedback = result.deleted_feedback,
        deleted_intelligence = result.deleted_intelligence,
        deleted_windows = result.deleted_windows,
        vacuumed = result.vacuumed,
        "Deep clean completed"
    );

    Ok(result)
}

/// Update the data retention period (days).
#[tauri::command]
pub async fn set_cleanup_retention(days: u32) -> Result<()> {
    if !(7..=365).contains(&days) {
        return Err(FourDaError::Internal(
            "Retention must be between 7 and 365 days".into(),
        ));
    }
    let sm = crate::get_settings_manager();
    let mut guard = sm.lock();
    guard.get_mut().monitoring.cleanup_max_age_days = Some(days);
    guard.save().context("failed to save retention settings")?;
    info!(target: "4da::autophagy", days, "Data retention period updated");
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autophagy::AutophagyCycleResult;

    #[test]
    fn test_autophagy_status_construction() {
        let status = AutophagyStatus {
            last_cycle: None,
            total_cycles: 0,
            total_calibrations: 0,
            total_anti_patterns: 0,
        };
        assert_eq!(status.total_cycles, 0);
        assert!(status.last_cycle.is_none());
    }

    #[test]
    fn test_autophagy_status_with_cycle() {
        let cycle = AutophagyCycleResult {
            items_analyzed: 100,
            items_pruned: 15,
            calibrations_produced: 3,
            topic_decay_rates_updated: 5,
            source_autopsies_produced: 2,
            anti_patterns_detected: 1,
            decision_outcomes_analyzed: 0,
            duration_ms: 450,
        };
        let status = AutophagyStatus {
            last_cycle: Some(cycle.clone()),
            total_cycles: 7,
            total_calibrations: 12,
            total_anti_patterns: 3,
        };
        assert_eq!(status.total_cycles, 7);
        assert_eq!(
            status
                .last_cycle
                .as_ref()
                .expect("has cycle")
                .items_analyzed,
            100
        );
    }

    #[test]
    fn test_autophagy_status_serialization() {
        let status = AutophagyStatus {
            last_cycle: Some(AutophagyCycleResult {
                items_analyzed: 50,
                items_pruned: 5,
                calibrations_produced: 1,
                topic_decay_rates_updated: 2,
                source_autopsies_produced: 0,
                anti_patterns_detected: 0,
                decision_outcomes_analyzed: 0,
                duration_ms: 200,
            }),
            total_cycles: 3,
            total_calibrations: 1,
            total_anti_patterns: 0,
        };
        let json = serde_json::to_string(&status).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed["total_cycles"], 3);
        assert_eq!(parsed["last_cycle"]["items_analyzed"], 50);
    }
}
