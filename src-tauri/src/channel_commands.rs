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
use tauri::AppHandle;

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

    let changelog = crate::channel_changelog::compute_changelog(channel_id, old_render, new_render);
    Ok(Some(changelog))
}

/// Auto-render all stale or never-rendered channels.
/// Called after onboarding and on each monitoring cycle.
#[tauri::command]
pub async fn auto_render_all_channels() -> Result<()> {
    info!(target: "4da::channels", "Auto-rendering all stale channels");
    crate::channel_render::auto_render_stale_channels()
        .await
        .map_err(|e| -> crate::error::FourDaError { e.into() })?;
    Ok(())
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

// ============================================================================
// Channel Creation & Deletion
// ============================================================================

/// Maximum custom channels allowed on the free tier.
const FREE_TIER_MAX_CHANNELS: i64 = 3;

/// Create a custom channel.
#[tauri::command]
pub async fn create_custom_channel(
    app: AppHandle,
    slug: String,
    title: String,
    description: String,
    topic_query: Vec<String>,
) -> Result<i64> {
    let db = get_database()?;

    // Free tier gate: limit custom channels
    if !crate::settings::is_pro() {
        let custom_count = db
            .count_custom_channels()
            .map_err(|e| format!("Failed to count channels: {}", e))?;
        if custom_count >= FREE_TIER_MAX_CHANNELS {
            return Err(format!(
                "Free tier limited to {} custom channels. Upgrade to Pro for unlimited channels.",
                FREE_TIER_MAX_CHANNELS
            )
            .into());
        }
    }
    let id = db
        .create_channel(&slug, &title, &description, &topic_query)
        .map_err(|e| format!("Failed to create channel: {}", e))?;
    info!(target: "4da::channels", slug = %slug, id, "Created custom channel");

    // GAME: track channel creation
    for a in crate::game_engine::increment_counter(db, "channels", 1) {
        crate::events::emit_achievement_unlocked(&app, &a);
    }

    // Trigger async render for the new channel
    let channel_id = id;
    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::channel_render::render_channel(channel_id).await {
            tracing::warn!(target: "4da::channels", error = %e, "Initial render failed for new channel");
        }
    });

    Ok(id)
}

/// Preview how many sources match given topics (without creating a channel).
#[tauri::command]
pub async fn preview_channel_sources(topics: Vec<String>) -> Result<serde_json::Value> {
    let db = get_database()?;
    let (count, top_titles) = crate::channel_render::preview_channel_sources(db, &topics)
        .map_err(|e| -> crate::error::FourDaError { e.into() })?;
    Ok(serde_json::json!({
        "count": count,
        "topTitles": top_titles
    }))
}

/// Soft-delete a channel by archiving it.
#[tauri::command]
pub async fn delete_channel(channel_id: i64) -> Result<()> {
    let db = get_database()?;
    db.update_channel_status(channel_id, &crate::channels::ChannelStatus::Archived)
        .map_err(|e| format!("Failed to archive channel: {}", e))?;
    info!(target: "4da::channels", channel_id, "Channel archived");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::channels::{ChannelFreshness, ChannelSummary, RenderProvenance};
    use crate::test_utils::test_db;

    #[test]
    fn list_channels_returns_seeded_channels() {
        let db = test_db();
        let channels = db.list_channels().unwrap();
        assert_eq!(channels.len(), 3);
        for ch in &channels {
            assert!(matches!(ch.freshness, ChannelFreshness::NeverRendered));
        }
    }

    #[test]
    fn get_channel_returns_correct_data() {
        let db = test_db();
        let id = db
            .create_channel("cmd-test", "Command Test", "Test", &["rust".to_string()])
            .unwrap();
        let ch = db.get_channel(id).unwrap();
        assert_eq!(ch.slug, "cmd-test");
        assert_eq!(ch.source_count, 0);
    }

    #[test]
    fn get_channel_fails_for_nonexistent_id() {
        let db = test_db();
        assert!(db.get_channel(999_999).is_err());
    }

    #[test]
    fn get_latest_render_returns_none_when_never_rendered() {
        let db = test_db();
        let id = db
            .create_channel("no-render", "No Render", "Test", &[])
            .unwrap();
        let result = db.get_latest_render(id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn get_latest_render_returns_newest_version() {
        let db = test_db();
        let id = db
            .create_channel("versioned", "Versioned", "Test", &[])
            .unwrap();
        db.save_channel_render(id, "# V1", &[], None, None, None)
            .unwrap();
        db.save_channel_render(id, "# V2", &[], None, None, None)
            .unwrap();
        let render = db.get_latest_render(id).unwrap().unwrap();
        assert_eq!(render.version, 2);
        assert_eq!(render.content_markdown, "# V2");
    }

    #[test]
    fn get_render_provenance_returns_empty_for_no_provenance() {
        let db = test_db();
        let id = db
            .create_channel("prov-test", "Prov Test", "Test", &[])
            .unwrap();
        let render = db
            .save_channel_render(id, "# Content", &[], None, None, None)
            .unwrap();
        let provenance = db.get_render_provenance(render.id).unwrap();
        assert!(provenance.is_empty());
    }

    #[test]
    fn get_render_provenance_returns_saved_entries() {
        let db = test_db();
        let id = db
            .create_channel("prov-pop", "Prov Pop", "Test", &[])
            .unwrap();
        let render = db
            .save_channel_render(id, "# Content", &[], None, None, None)
            .unwrap();
        let entries = vec![RenderProvenance {
            render_id: render.id,
            claim_index: 0,
            claim_text: "Test claim".to_string(),
            source_item_ids: vec![1],
            source_titles: vec!["Test Article".to_string()],
            source_urls: vec!["https://example.com".to_string()],
        }];
        db.save_render_provenance(&entries).unwrap();
        let result = db.get_render_provenance(render.id).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].claim_text, "Test claim");
    }

    #[test]
    fn get_render_history_needs_minimum_two_for_changelog() {
        let db = test_db();
        let id = db
            .create_channel("changelog", "Changelog", "Test", &[])
            .unwrap();
        db.save_channel_render(id, "# V1", &[], None, None, None)
            .unwrap();
        let history = db.get_render_history(id, 2).unwrap();
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn channel_summary_serializes_to_json() {
        let summary = ChannelSummary {
            id: 1,
            slug: "test".to_string(),
            title: "Test Channel".to_string(),
            description: "A test channel".to_string(),
            source_count: 5,
            render_count: 2,
            freshness: ChannelFreshness::Fresh,
            last_rendered_at: Some("2026-01-01".to_string()),
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"slug\":\"test\""));
        assert!(json.contains("\"source_count\":5"));
    }

    #[test]
    fn channel_render_stores_source_item_ids() {
        let db = test_db();
        let id = db
            .create_channel("ids-test", "IDs Test", "Test", &[])
            .unwrap();
        let ids = vec![10, 20, 30];
        let render = db
            .save_channel_render(id, "# With Sources", &ids, Some("test-model"), None, None)
            .unwrap();
        assert_eq!(render.source_item_ids, ids);
        assert_eq!(render.model.as_deref(), Some("test-model"));
    }
}
