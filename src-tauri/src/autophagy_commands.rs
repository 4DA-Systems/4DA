#![allow(dead_code)]
//! Tauri commands for the Intelligent Autophagy system.
//!
//! Exposes autophagy status, history, and manual trigger to the frontend.

use serde::Serialize;
use ts_rs::TS;

use crate::autophagy::AutophagyCycleResult;
use crate::error::{FourDaError, Result};

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
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;

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
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;
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
                duration_ms: row.get(6)?,
            })
        })
        .map_err(FourDaError::Db)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(FourDaError::Db)?;

    Ok(results)
}
