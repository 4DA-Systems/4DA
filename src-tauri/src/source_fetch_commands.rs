// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Immediate fetch commands for individual RSS feeds and YouTube channels.
//!
//! These commands fetch, embed, and store items from a single source on demand,
//! bypassing the normal scheduler cycle. Extracted from source_config to keep
//! file sizes within limits.

use tracing::info;

use crate::error::Result;
use crate::source_config::validate_input_length;
use crate::sources::Source;

// ============================================================================
// Immediate Fetch: RSS
// ============================================================================

/// Fetch a single RSS feed immediately, embed items, and store in database
#[tauri::command]
pub async fn fetch_single_feed(
    url: String,
    app: tauri::AppHandle,
) -> Result<serde_json::Value> {
    use tauri::Emitter;

    validate_input_length(&url, "Feed URL", 2000)?;
    crate::url_validation::validate_not_internal(&url)?;

    let source = crate::sources::rss::RssSource::with_feeds(vec![url.clone()]);
    let items = source
        .fetch_items()
        .await
        .map_err(|e| format!("Fetch failed: {}", e))?;

    if items.is_empty() {
        return Ok(serde_json::json!({ "success": true, "items_added": 0 }));
    }

    let added = embed_and_store_items(&items).await?;

    let _ = app.emit(
        "source-fetched",
        serde_json::json!({
            "source": "rss",
            "count": added,
            "immediate": true,
            "feed_url": url,
        }),
    );

    Ok(serde_json::json!({
        "success": true,
        "items_added": added,
    }))
}

// ============================================================================
// Immediate Fetch: YouTube
// ============================================================================

/// Fetch a single YouTube channel immediately, embed items, and store in database
#[tauri::command]
pub async fn fetch_single_youtube_channel(
    channel_id: String,
    app: tauri::AppHandle,
) -> Result<serde_json::Value> {
    use tauri::Emitter;

    validate_input_length(&channel_id, "Channel ID", 100)?;

    let source =
        crate::sources::youtube::YouTubeSource::with_channels(vec![channel_id.clone()]);
    let items = source
        .fetch_items()
        .await
        .map_err(|e| format!("Fetch failed: {}", e))?;

    if items.is_empty() {
        return Ok(serde_json::json!({ "success": true, "items_added": 0 }));
    }

    let added = embed_and_store_items(&items).await?;

    let _ = app.emit(
        "source-fetched",
        serde_json::json!({
            "source": "youtube",
            "count": added,
            "immediate": true,
            "channel_id": channel_id,
        }),
    );

    Ok(serde_json::json!({
        "success": true,
        "items_added": added,
    }))
}

// ============================================================================
// Shared embedding + storage pipeline
// ============================================================================

/// Embed and store source items in the database. Returns number of items added.
///
/// This follows the same pipeline as `source_fetching::fetcher` but for
/// single-source on-demand fetches: build embedding text, detect language,
/// classify content, extract CVE IDs, extract feed origin, embed, upsert.
async fn embed_and_store_items(items: &[crate::sources::SourceItem]) -> Result<usize> {
    // Build texts for embedding
    let texts_to_embed: Vec<String> = items
        .iter()
        .map(|item| crate::build_embedding_text(&item.title, &item.content))
        .collect();

    // Embed all texts
    let embeddings = crate::embed_texts(&texts_to_embed).await.unwrap_or_default();

    // Build upsert tuples matching batch_upsert_source_items signature
    let upsert_items: Vec<_> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let detected_lang = crate::language_detect::detect_language(&item.title);
            let content_type = crate::entity_extraction::classify_for_storage(
                &item.title,
                &item.content,
                &item.source_type,
            );
            let cve_ids =
                crate::entity_extraction::extract_cve_ids(&item.title, &item.content);
            let feed_origin = crate::source_fetching::extract_feed_origin(item);
            let embedding = embeddings
                .get(i)
                .cloned()
                .unwrap_or_else(|| vec![0.0f32; 384]);

            (
                item.source_type.clone(),
                item.source_id.clone(),
                item.url.clone(),
                item.title.clone(),
                item.content.clone(),
                embedding,
                detected_lang,
                content_type,
                cve_ids,
                feed_origin,
            )
        })
        .collect();

    let db = crate::get_database()?;
    let added = db.batch_upsert_source_items(&upsert_items).unwrap_or(0);

    info!(
        target: "4da::sources",
        items_fetched = items.len(),
        items_stored = added,
        "Immediate fetch complete"
    );

    Ok(added)
}

// ============================================================================
// Feed Health: Reset circuit breaker
// ============================================================================

/// Reset the circuit breaker for a specific feed/channel/handle
#[tauri::command]
pub async fn reset_feed_health(
    feed_origin: String,
    source_type: String,
) -> Result<serde_json::Value> {
    validate_input_length(&feed_origin, "Feed origin", 2000)?;
    validate_input_length(&source_type, "Source type", 50)?;

    let db = crate::get_database()?;
    db.reset_feed_health(&feed_origin, &source_type)
        .map_err(|e| crate::error::FourDaError::Internal(format!("Failed to reset feed health: {e}")))?;

    info!(
        target: "4da::sources",
        feed = %feed_origin,
        source = %source_type,
        "Feed health reset"
    );

    Ok(serde_json::json!({
        "success": true,
        "feed_origin": feed_origin,
        "source_type": source_type,
    }))
}
