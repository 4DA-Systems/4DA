//! MUSE Tauri Command Handlers
//!
//! IPC bridge for MUSE operations. These commands are registered in the
//! invoke_handler but not exposed in the frontend (MUSE is a private
//! parallel development track, revealed post-launch).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::Result;
use crate::muse;

// ============================================================================
// Pack Management Commands
// ============================================================================

/// Create a new MUSE context pack
#[tauri::command]
pub async fn muse_create_pack(name: String, description: Option<String>) -> Result<muse::MusePack> {
    let conn = crate::open_db_connection()?;
    muse::pack::create_pack(&conn, &name, description.as_deref())
}

/// List all MUSE context packs
#[tauri::command]
pub async fn muse_list_packs(active_only: Option<bool>) -> Result<Vec<muse::MusePack>> {
    let conn = crate::open_db_connection()?;
    muse::pack::list_packs(&conn, active_only.unwrap_or(false))
}

/// Get a single pack by ID
#[tauri::command]
pub async fn muse_get_pack(pack_id: String) -> Result<Option<muse::MusePack>> {
    let conn = crate::open_db_connection()?;
    muse::pack::get_pack(&conn, &pack_id)
}

/// Activate or deactivate a pack
#[tauri::command]
pub async fn muse_set_pack_active(pack_id: String, active: bool) -> Result<()> {
    let conn = crate::open_db_connection()?;
    muse::pack::set_pack_active(&conn, &pack_id, active)
}

/// Delete a pack and all its sources
#[tauri::command]
pub async fn muse_delete_pack(pack_id: String) -> Result<()> {
    let conn = crate::open_db_connection()?;
    muse::pack::delete_pack(&conn, &pack_id)
}

// ============================================================================
// Source Management Commands
// ============================================================================

/// Add source files to a pack
#[tauri::command]
pub async fn muse_add_sources(pack_id: String, file_paths: Vec<String>) -> Result<MuseAddResult> {
    let conn = crate::open_db_connection()?;
    let mut added = 0u32;
    let mut skipped = 0u32;

    for path in &file_paths {
        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if let Some(media_type) = muse::classify_media_type(ext) {
            muse::pack::add_source(&conn, &pack_id, path, media_type, None)?;
            added += 1;
        } else {
            skipped += 1;
        }
    }

    Ok(MuseAddResult {
        added,
        skipped,
        total: file_paths.len() as u32,
    })
}

/// List sources for a pack
#[tauri::command]
pub async fn muse_list_sources(pack_id: String) -> Result<Vec<muse::MusePackSource>> {
    let conn = crate::open_db_connection()?;
    muse::pack::list_sources(&conn, &pack_id)
}

// ============================================================================
// Influence Commands
// ============================================================================

/// Enrich a generation prompt with the active pack's context
#[tauri::command]
pub async fn muse_enrich_prompt(prompt: String, pack_id: Option<String>) -> Result<String> {
    let conn = crate::open_db_connection()?;

    // Use specified pack or find the first active pack
    let pack = if let Some(id) = pack_id {
        muse::pack::get_pack(&conn, &id)?
    } else {
        let active = muse::pack::list_packs(&conn, true)?;
        active.into_iter().next()
    };

    match pack {
        Some(p) => Ok(muse::influence::enrich_prompt(&p, &prompt)),
        None => Ok(prompt), // No active pack — return original
    }
}

// ============================================================================
// Stats Commands
// ============================================================================

/// Get MUSE system statistics
#[tauri::command]
pub async fn muse_get_stats() -> Result<muse::pack::PackStats> {
    let conn = crate::open_db_connection()?;
    muse::pack::pack_stats(&conn)
}

// ============================================================================
// Extraction Commands
// ============================================================================

/// Extract creative signals from all pending source files in a pack.
/// Runs image analysis (color, composition, texture) on each file,
/// then aggregates results into the pack's unified visual profile.
#[tauri::command]
pub async fn muse_extract_pack(pack_id: String) -> Result<muse::extract::ExtractionReport> {
    let conn = crate::open_db_connection()?;
    muse::extract::extract_pack(&conn, &pack_id)
}

/// Analyze a single image file and return its visual profile.
/// Useful for preview before adding to a pack.
#[tauri::command]
pub async fn muse_analyze_image(file_path: String) -> Result<muse::VisualProfile> {
    let path = std::path::Path::new(&file_path);
    muse::image_analysis::analyze_image(path)
}

// ============================================================================
// Response Types
// ============================================================================

/// Result of adding sources to a pack
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MuseAddResult {
    pub added: u32,
    pub skipped: u32,
    pub total: u32,
}
