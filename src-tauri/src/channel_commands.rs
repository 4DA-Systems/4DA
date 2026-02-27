//! Tauri commands for Information Channels.
//!
//! All commands follow the pattern: clone data out of mutex, release lock, then await.
//! Commands bridge the frontend Zustand store (channels-slice.ts) to the Rust render pipeline.

use tracing::{error, info};

use crate::channels::{
    ChannelChangelog, ChannelRender, ChannelSourceMatch, ChannelSummary, RenderProvenance,
};
use crate::error::Result;
use crate::get_database;

// ============================================================================
// Channel List & Detail
// ============================================================================

/// List all active channels with freshness indicators.
#[tauri::command]
pub async fn list_channels() -> Result<Vec<ChannelSummary>> {
    let db = get_database()?;
    let channels = db
        .list_channels()
        .map_err(|e| format!("Failed to list channels: {}", e))?;
    Ok(channels)
}

/// Get full channel details by ID.
#[tauri::command]
pub async fn get_channel(channel_id: i64) -> Result<crate::channels::Channel> {
    let db = get_database()?;
    let channel = db
        .get_channel(channel_id)
        .map_err(|e| format!("Channel not found: {}", e))?;
    Ok(channel)
}

// ============================================================================
// Render Operations
// ============================================================================

/// Get the latest render for a channel. If never rendered, triggers first render.
#[tauri::command]
pub async fn get_channel_content(channel_id: i64) -> Result<Option<ChannelRender>> {
    let db = get_database()?;
    match db.get_latest_render(channel_id) {
        Ok(Some(render)) => Ok(Some(render)),
        Ok(None) => {
            // No render exists -- trigger first render
            info!(target: "4da::channels", channel_id, "No render exists, triggering first render");
            match crate::channel_render::render_channel(channel_id).await {
                Ok(render) => Ok(Some(render)),
                Err(e) => {
                    error!(target: "4da::channels", error = %e, "First render failed");
                    Ok(None)
                }
            }
        }
        Err(e) => Err(format!("Failed to get channel content: {}", e).into()),
    }
}

/// Force re-render a channel now.
#[tauri::command]
pub async fn render_channel_now(channel_id: i64) -> Result<ChannelRender> {
    info!(target: "4da::channels", channel_id, "Force re-rendering channel");
    let render = crate::channel_render::render_channel(channel_id)
        .await
        .map_err(|e| -> crate::error::FourDaError { e.into() })?;
    Ok(render)
}

// ============================================================================
// Provenance & Changelog
// ============================================================================

/// Get provenance (source citations) for a specific render.
#[tauri::command]
pub async fn get_channel_provenance(render_id: i64) -> Result<Vec<RenderProvenance>> {
    let db = get_database()?;
    let provenance = db
        .get_render_provenance(render_id)
        .map_err(|e| format!("Failed to get provenance: {}", e))?;
    Ok(provenance)
}

/// Get changelog (diff) between latest and previous render.
#[tauri::command]
pub async fn get_channel_changelog(channel_id: i64) -> Result<Option<ChannelChangelog>> {
    let db = get_database()?;
    let renders = db
        .get_render_history(channel_id, 2)
        .map_err(|e| format!("Failed to get render history: {}", e))?;

    if renders.len() < 2 {
        return Ok(None);
    }

    // renders are ordered newest first
    let new_render = &renders[0];
    let old_render = &renders[1];

    let changelog = crate::channel_render::compute_changelog(channel_id, old_render, new_render);
    Ok(Some(changelog))
}

// ============================================================================
// Source Management
// ============================================================================

/// Get matched source items for a channel.
#[tauri::command]
pub async fn get_channel_sources(
    channel_id: i64,
    limit: Option<usize>,
) -> Result<Vec<ChannelSourceMatch>> {
    let db = get_database()?;
    let limit = limit.unwrap_or(20);
    let sources = db
        .get_channel_source_items(channel_id, limit)
        .map_err(|e| format!("Failed to get channel sources: {}", e))?;
    Ok(sources)
}

/// Re-scan sources for a channel and return the new source count.
#[tauri::command]
pub async fn refresh_channel_sources(channel_id: i64) -> Result<i64> {
    let db = get_database()?;
    let channel = db
        .get_channel(channel_id)
        .map_err(|e| format!("Channel not found: {}", e))?;

    info!(target: "4da::channels", channel = %channel.slug, "Refreshing channel sources");

    let items = crate::channel_render::gather_channel_sources(db, &channel)
        .map_err(|e| -> crate::error::FourDaError { e.into() })?;

    let count = db
        .refresh_channel_source_count(channel_id)
        .map_err(|e| format!("Failed to refresh count: {}", e))?;

    info!(
        target: "4da::channels",
        channel = %channel.slug,
        sources = items.len(),
        count,
        "Channel sources refreshed"
    );
    Ok(count)
}
