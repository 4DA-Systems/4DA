// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Local OSV mirror — stores advisories locally for Tier 1 verified intelligence.
//!
//! Syncs advisories from the OSV API for the user's actual dependencies,
//! then cross-references with version matching to produce verified alerts.

pub(crate) mod cache;
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
    // The sync recomputed matches — refresh the Preemption feed cache in the
    // background so a manual refresh is reflected on the next tab open without
    // paying the 30-40s recompute (live matching + adversarial deliberation).
    tauri::async_runtime::spawn(async {
        crate::preemption::warm_preemption_cache().await;
    });
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

/// Update the local OSV cache by downloading ecosystem ZIP files.
/// Falls back to cached data when the network is unavailable.
#[tauri::command]
pub async fn osv_update_cache() -> Result<serde_json::Value> {
    let db = crate::get_database()?;
    let result = cache::update_all_caches(&db).await?;
    serde_json::to_value(&result).context("Failed to serialize cache update result")
}

/// Get the status of all cached ecosystem ZIP files.
#[tauri::command]
pub async fn osv_cache_status() -> Result<serde_json::Value> {
    let statuses = cache::get_all_cache_statuses()?;
    serde_json::to_value(&statuses).context("Failed to serialize cache statuses")
}
