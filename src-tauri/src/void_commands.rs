//! Void Engine Tauri commands for 3D spatial visualization.
//!
//! Extracted from lib.rs. Provides commands for the heartbeat signal,
//! universe building, particle details, and neighbor finding.

use crate::void_engine;
use crate::{get_context_engine, get_database, get_monitoring_state};

// ============================================================================
// Void Engine Commands
// ============================================================================

/// Get the current void signal state (for initial mount)
#[tauri::command]
pub fn get_void_signal() -> Result<void_engine::VoidSignal, String> {
    let db = get_database()?;
    let monitoring = get_monitoring_state();
    Ok(void_engine::compute_signal(db, monitoring))
}

/// Build the complete VoidUniverse for 3D rendering
#[tauri::command]
pub fn void_get_universe(
    max_particles: Option<usize>,
) -> Result<void_engine::VoidUniverse, String> {
    let db = get_database()?;
    let ctx = get_context_engine()?;
    let projection_version = 1i64; // Increment when embedding model changes
    void_engine::build_universe(db, ctx, max_particles, projection_version)
}

/// Get full detail for a single particle (on selection)
#[tauri::command]
pub fn void_get_particle_detail(id: i64, layer: String) -> Result<serde_json::Value, String> {
    let db = get_database()?;
    match layer.as_str() {
        "source" => {
            let item = db
                .get_source_item_by_id(id)
                .map_err(|e| format!("DB error: {e}"))?
                .ok_or_else(|| format!("Source item {id} not found"))?;
            serde_json::to_value(serde_json::json!({
                "id": item.id,
                "layer": "source",
                "source_type": item.source_type,
                "title": item.title,
                "url": item.url,
                "content_preview": &item.content[..item.content.len().min(500)],
                "created_at": item.created_at.to_rfc3339(),
                "last_seen": item.last_seen.to_rfc3339(),
            }))
            .map_err(|e| format!("Serialization error: {e}"))
        }
        "context" => {
            // Context chunks are internal - return basic info
            Ok(serde_json::json!({
                "id": id,
                "layer": "context",
                "message": "Context chunk from local files",
            }))
        }
        _ => Err(format!("Unknown layer: {layer}")),
    }
}

/// Find K nearest neighbors for a particle in the universe
#[tauri::command]
pub fn void_get_neighbors(
    id: i64,
    layer: String,
    k: Option<usize>,
) -> Result<Vec<void_engine::VoidParticle>, String> {
    // Build universe first, then find neighbors within it
    let db = get_database()?;
    let ctx = get_context_engine()?;
    let universe = void_engine::build_universe(db, ctx, None, 1)?;
    let neighbors = void_engine::find_neighbors(id, &layer, &universe.particles, k.unwrap_or(10));
    Ok(neighbors)
}
