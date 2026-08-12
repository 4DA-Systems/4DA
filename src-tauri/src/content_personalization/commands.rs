// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tauri commands for the Sovereign Content Engine.

use crate::error::Result;

use super::cache;

/// Prune stale cache entries. Called on app startup and periodically.
#[tauri::command]
pub async fn prune_personalization_cache() -> Result<serde_json::Value> {
    let conn = crate::open_db_connection()?;
    let deleted = cache::prune_cache(&conn);
    let stats = cache::cache_stats(&conn);
    Ok(serde_json::json!({
        "deleted": deleted,
        "remaining": stats.cache_entries,
        "read_states": stats.read_state_entries,
        "cache_size_bytes": stats.cache_size_bytes,
    }))
}
