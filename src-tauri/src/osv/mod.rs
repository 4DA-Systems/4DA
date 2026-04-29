// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Local OSV mirror — stores advisories locally for Tier 1 verified intelligence.
//!
//! Syncs advisories from the OSV API for the user's actual dependencies,
//! then cross-references with version matching to produce verified alerts.

pub(crate) mod matching;
pub(crate) mod sync;
pub(crate) mod types;

use crate::error::{Result, ResultExt};

// ============================================================================
// Tauri Commands
// ============================================================================

/// Trigger a manual OSV sync. Queries the OSV API for all user dependencies
/// and stores advisories locally.
#[tauri::command]
pub async fn osv_sync_now() -> Result<serde_json::Value> {
    let db = crate::get_database()?;
    let result = sync::sync(&db).await?;
    serde_json::to_value(&result).context("Failed to serialize sync result")
}

/// Get all advisories that match the user's installed dependencies.
/// Returns Tier 1 verified intelligence items.
#[tauri::command]
pub async fn osv_get_matches() -> Result<serde_json::Value> {
    let db = crate::get_database()?;
    let matches = matching::get_matched_advisories(&db)?;
    serde_json::to_value(&matches).context("Failed to serialize matched advisories")
}

/// Get the sync status for all ecosystems.
#[tauri::command]
pub async fn osv_get_sync_status() -> Result<serde_json::Value> {
    let db = crate::get_database()?;
    let statuses = db
        .get_osv_sync_statuses()
        .context("Failed to read sync status")?;
    serde_json::to_value(&statuses).context("Failed to serialize sync status")
}
