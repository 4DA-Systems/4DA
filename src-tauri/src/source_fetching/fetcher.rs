//! Network fetching logic: fetch_all_sources, fetch_all_sources_deep

use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};

use crate::db::Database;
use crate::sources::arxiv::ArxivSource;
use crate::sources::devto::DevtoSource;
use crate::sources::github::GitHubSource;
use crate::sources::hackernews::HackerNewsSource;
use crate::sources::lobsters::LobstersSource;
use crate::sources::reddit::RedditSource;
use crate::sources::rss::RssSource;
use crate::sources::twitter::TwitterSource;
use crate::sources::youtube::YouTubeSource;
use crate::{
    build_embedding_text, embed_texts, emit_progress, sources, truncate_utf8, GenericSourceItem,
};

use super::processor::process_source_items;
use super::{
    load_github_languages_from_settings, load_rss_feeds_from_settings, load_twitter_settings,
    load_youtube_channels_from_settings,
};

/// Fetch items from all sources (HN, arXiv, Reddit) directly
pub(crate) async fn fetch_all_sources(
    db: &Database,
    app: &AppHandle,
    max_items_per_source: usize,
) -> Result<Vec<(GenericSourceItem, Vec<f32>)>, String> {
    use sources::Source;

    // Phase 4: Network connectivity check before fetching
    // Multi-target parallel race: resolves on first successful response
    let client = sources::shared_client();
    let timeout = std::time::Duration::from_secs(4);
    let online = tokio::select! {
        r = client.head("https://1.1.1.1/cdn-cgi/trace").timeout(timeout).send() => r.is_ok(),
        r = client.head("https://dns.google/resolve?name=example.com").timeout(timeout).send() => r.is_ok(),
        r = client.head("https://httpbin.org/get").timeout(timeout).send() => r.is_ok(),
    };

    if !online {
        warn!(target: "4da::sources", "Network unavailable - using cached content only");
        let _ = app.emit("network-offline", ());
        return Ok(Vec::new()); // Return empty; caller falls back to cache
    }

    // Create sources directly (avoid holding MutexGuard across await)
    // Filter by enabled status from DB (Phase 2: source enable/disable enforcement)
    let rss_feeds = load_rss_feeds_from_settings();
    let (twitter_handles, x_api_key) = load_twitter_settings();
    let youtube_channels = load_youtube_channels_from_settings();

    let all_sources: Vec<(&str, Box<dyn Source>)> = vec![
        (
            "hackernews",
            Box::new(HackerNewsSource::new()) as Box<dyn Source>,
        ),
        ("arxiv", Box::new(ArxivSource::new())),
        ("reddit", Box::new(RedditSource::new())),
        (
            "github",
            Box::new(GitHubSource::with_languages(
                load_github_languages_from_settings(),
            )),
        ),
        ("rss", Box::new(RssSource::with_feeds(rss_feeds))),
        (
            "twitter",
            Box::new(TwitterSource::with_handles(twitter_handles).with_api_key(x_api_key)),
        ),
        (
            "youtube",
            Box::new(YouTubeSource::with_channels(youtube_channels)),
        ),
        (
            "lobsters",
            Box::new(LobstersSource::new()) as Box<dyn Source>,
        ),
        ("devto", Box::new(DevtoSource::new()) as Box<dyn Source>),
        (
            "producthunt",
            Box::new(crate::sources::producthunt::ProductHuntSource::new()) as Box<dyn Source>,
        ),
    ];

    let sources: Vec<Box<dyn Source>> = all_sources
        .into_iter()
        .filter(|(source_type, _)| {
            let enabled = db.is_source_enabled(source_type);
            if !enabled {
                info!(target: "4da::sources", source = source_type, "Skipping disabled source");
            }
            enabled
        })
        .map(|(_, source)| source)
        .collect();

    info!(target: "4da::sources", count = sources.len(), "Fetching from enabled sources");

    let mut all_items: Vec<(GenericSourceItem, Vec<f32>)> = Vec::new();
    let mut new_items_to_embed: Vec<(GenericSourceItem, String)> = Vec::new();

    for source in &sources {
        let source_type = source.source_type();
        let source_name = source.name();

        debug!(target: "4da::sources", source = source_name, "Fetching from source");
        emit_progress(
            app,
            "fetch",
            0.2,
            &format!("Fetching from {}...", source_name),
            all_items.len(),
            max_items_per_source * 3,
        );

        // Phase 7: Fetch interval enforcement (skip if fetched recently, default 300s)
        if let Ok(Some(last_fetch_str)) = db.get_source_last_fetch(source_type) {
            if let Ok(last_fetch) =
                chrono::NaiveDateTime::parse_from_str(&last_fetch_str, "%Y-%m-%d %H:%M:%S")
            {
                let elapsed = chrono::Utc::now().naive_utc() - last_fetch;
                if elapsed.num_seconds() < 300 {
                    debug!(target: "4da::sources", source = source_name, elapsed_secs = elapsed.num_seconds(), "Skipping - fetched recently");
                    continue;
                }
            }
        }

        // Fetch items from this source with exponential backoff retry
        let fetch_start = std::time::Instant::now();

        // Circuit breaker: skip sources with 5+ consecutive failures
        if db.is_circuit_open(source_type) {
            warn!(target: "4da::sources", source = source_name, "Circuit breaker OPEN - skipping (too many failures)");
            let _ = app.emit("source-error", serde_json::json!({
                "source": source_type, "error": "Circuit breaker open (5+ failures)", "retry_count": 0
            }));
            continue;
        }

        let fetch_result = {
            let mut attempts = 0;
            let max_attempts = 3;
            let backoff_ms = [500u64, 1000, 2000];
            loop {
                attempts += 1;
                match source.fetch_items().await {
                    Ok(items) => break Ok(items),
                    Err(e) if attempts < max_attempts => {
                        let delay = backoff_ms.get(attempts - 1).copied().unwrap_or(2000);
                        warn!(target: "4da::sources", source = source_name, attempt = attempts, error = ?e, delay_ms = delay, "Fetch failed, retrying...");
                        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    }
                    Err(e) => break Err(e),
                }
            }
        };

        let elapsed_ms = fetch_start.elapsed().as_millis() as i64;

        match fetch_result {
            Ok(items) => {
                let item_count = items.len();
                info!(target: "4da::sources", source = source_name, count = item_count, ms = elapsed_ms, "Fetched items from source");
                db.record_source_health(source_type, true, item_count as i64, elapsed_ms, None)
                    .ok();
                db.update_source_fetch_time(source_type).ok();
                let _ = app.emit(
                    "source-fetched",
                    serde_json::json!({
                        "source": source_type, "count": item_count
                    }),
                );

                for (idx, item) in items.into_iter().take(max_items_per_source).enumerate() {
                    // Generate a numeric ID from source_id hash
                    let id = {
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut hasher = DefaultHasher::new();
                        format!("{}:{}", source_type, item.source_id).hash(&mut hasher);
                        hasher.finish()
                    };

                    // Check cache first
                    if let Ok(Some(cached)) = db.get_source_item(source_type, &item.source_id) {
                        if let Err(e) = db.touch_source_item(source_type, &item.source_id) {
                            warn!(target: "4da::sources", source_type, source_id = %item.source_id, error = %e, "Failed to touch source item");
                        }
                        all_items.push((
                            GenericSourceItem {
                                id,
                                source_id: item.source_id,
                                source_type: source_type.to_string(),
                                title: cached.title,
                                url: cached.url,
                                content: cached.content,
                            },
                            cached.embedding,
                        ));
                    } else {
                        // Need to scrape and embed
                        let content = if item.content.is_empty() {
                            if let Some(ref _url) = item.url {
                                emit_progress(
                                    app,
                                    "scrape",
                                    0.3,
                                    &format!("Scraping: {}", &truncate_utf8(&item.title, 30)),
                                    idx,
                                    max_items_per_source,
                                );
                                let scraped =
                                    source.scrape_content(&item).await.unwrap_or_default();
                                // Cap scraped content to 500KB to prevent memory exhaustion
                                if scraped.len() > 500_000 {
                                    scraped[..500_000].to_string()
                                } else {
                                    scraped
                                }
                            } else {
                                String::new()
                            }
                        } else {
                            // Cap item content too
                            let c = item.content.clone();
                            if c.len() > 500_000 {
                                c[..500_000].to_string()
                            } else {
                                c
                            }
                        };

                        let generic = GenericSourceItem {
                            id,
                            source_id: item.source_id.clone(),
                            source_type: source_type.to_string(),
                            title: item.title.clone(),
                            url: item.url.clone(),
                            content: content.clone(),
                        };

                        let embed_text = build_embedding_text(&item.title, &content);
                        new_items_to_embed.push((generic, embed_text));
                    }
                }
            }
            Err(e) => {
                error!(target: "4da::sources", source = source_name, error = ?e, "Source fetch failed after retries - continuing with other sources");
                db.record_source_health(source_type, false, 0, elapsed_ms, Some(&format!("{}", e)))
                    .ok();
                let _ = app.emit(
                    "source-error",
                    serde_json::json!({
                        "source": source_type, "error": format!("{}", e), "retry_count": 3
                    }),
                );
            }
        }
    }

    // Log summary of fetch results
    if all_items.is_empty() && new_items_to_embed.is_empty() {
        warn!(target: "4da::sources", "No items fetched from any source - check network connectivity");
    }

    // Embed new items with graceful degradation
    if !new_items_to_embed.is_empty() {
        debug!(target: "4da::embed", count = new_items_to_embed.len(), "Embedding new items");
        emit_progress(
            app,
            "embed",
            0.6,
            &format!("Embedding {} new items...", new_items_to_embed.len()),
            all_items.len(),
            all_items.len() + new_items_to_embed.len(),
        );

        let texts: Vec<String> = new_items_to_embed
            .iter()
            .map(|(_, text)| text.clone())
            .collect();

        // Try to embed, with fallback to zero vectors (keyword-only scoring)
        let embeddings = match embed_texts(&texts).await {
            Ok(emb) => {
                let _ = app.emit("embedding-mode", serde_json::json!({ "mode": "semantic" }));
                emb
            }
            Err(e) => {
                warn!(target: "4da::embed", error = %e, count = texts.len(),
                    "Embedding service unavailable - using fallback (keyword-only scoring)");
                let _ = app.emit(
                    "embedding-mode",
                    serde_json::json!({ "mode": "keyword-only", "reason": e.to_string() }),
                );
                // Create zero vectors as fallback - items will score via keyword matching only
                vec![vec![0.0f32; 384]; texts.len()]
            }
        };

        // Batch upsert: separate successful and failed embeddings
        let mut items_to_insert = Vec::new();
        let mut pending_items = Vec::new();
        for ((item, embed_text), embedding) in
            new_items_to_embed.into_iter().zip(embeddings.into_iter())
        {
            // Decode HTML entities at ingestion time so DB always has clean text.
            // This ensures dedup, embeddings, and display all see the same clean text.
            let clean_title = crate::decode_html_entities(&item.title);
            let clean_content = crate::decode_html_entities(&item.content);

            let is_fallback = embedding.iter().all(|&v| v == 0.0);
            if is_fallback {
                // Store as pending for re-embedding on next analysis
                pending_items.push((
                    item.source_type.clone(),
                    item.source_id.clone(),
                    item.url.clone(),
                    clean_title.clone(),
                    clean_content.clone(),
                    embed_text,
                ));
            } else {
                items_to_insert.push((
                    item.source_type.clone(),
                    item.source_id.clone(),
                    item.url.clone(),
                    clean_title.clone(),
                    clean_content.clone(),
                    embedding.clone(),
                ));
            }
            all_items.push((item, embedding));
        }

        // Batch insert successful embeddings
        if !items_to_insert.is_empty() {
            db.batch_upsert_source_items(&items_to_insert).ok();
        }

        // Store pending items for retry on next analysis
        if !pending_items.is_empty() {
            info!(target: "4da::sources", count = pending_items.len(), "Storing pending items for embedding retry");
            db.batch_upsert_pending_source_items(&pending_items).ok();
        }
    }

    info!(target: "4da::sources", total = all_items.len(), "Total items from all sources");
    Ok(all_items)
}

/// Deep fetch from all sources - used for comprehensive initial scans
/// Fetches 5-10x more items from multiple endpoints per source
pub(crate) async fn fetch_all_sources_deep(
    db: &Database,
    app: &AppHandle,
    items_per_category: usize,
) -> Result<Vec<(GenericSourceItem, Vec<f32>)>, String> {
    use sources::Source;

    info!(target: "4da::sources", items_per_category, "DEEP SCAN: Fetching from all sources comprehensively");

    // Create sources directly (avoid holding MutexGuard across await)
    let rss_feeds = load_rss_feeds_from_settings();
    let (twitter_handles, x_api_key) = load_twitter_settings();
    let youtube_channels = load_youtube_channels_from_settings();

    // Note: HN, arXiv, Reddit, Lobsters, Dev.to have fetch_items_deep implementations
    // GitHub, RSS, YouTube use default (regular fetch). Twitter has deep fetch.
    let hn_source = HackerNewsSource::new();
    let arxiv_source = ArxivSource::new();
    let reddit_source = RedditSource::new();
    let github_source = GitHubSource::with_languages(load_github_languages_from_settings());
    let rss_source = RssSource::with_feeds(rss_feeds);
    let twitter_source = TwitterSource::with_handles(twitter_handles).with_api_key(x_api_key);
    let youtube_source = YouTubeSource::with_channels(youtube_channels);
    let lobsters_source = LobstersSource::new();
    let devto_source = DevtoSource::new();

    let mut all_items: Vec<(GenericSourceItem, Vec<f32>)> = Vec::new();
    let mut new_items_to_embed: Vec<(GenericSourceItem, String)> = Vec::new();

    // Fetch from all sources in parallel using tokio::join!
    emit_progress(
        app,
        "fetch",
        0.12,
        "Deep fetching from all sources in parallel...",
        0,
        0,
    );

    let (
        hn_result,
        arxiv_result,
        reddit_result,
        github_result,
        rss_result,
        twitter_result,
        youtube_result,
        lobsters_result,
        devto_result,
    ) = tokio::join!(
        hn_source.fetch_items_deep(items_per_category),
        arxiv_source.fetch_items_deep(items_per_category),
        reddit_source.fetch_items_deep(items_per_category),
        github_source.fetch_items(),
        rss_source.fetch_items(),
        twitter_source.fetch_items_deep(items_per_category),
        youtube_source.fetch_items(),
        lobsters_source.fetch_items_deep(items_per_category),
        devto_source.fetch_items_deep(items_per_category),
    );

    // Process HN results
    match hn_result {
        Ok(items) => {
            info!(target: "4da::sources", source = "hackernews", count = items.len(), "Deep fetched HN items");
            process_source_items(
                db,
                &mut all_items,
                &mut new_items_to_embed,
                items,
                "hackernews",
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "hackernews", error = ?e, "Deep fetch failed");
        }
    }

    // Process arXiv results
    match arxiv_result {
        Ok(items) => {
            info!(target: "4da::sources", source = "arxiv", count = items.len(), "Deep fetched arXiv papers");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "arxiv");
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "arxiv", error = ?e, "Deep fetch failed");
        }
    }

    // Process Reddit results
    match reddit_result {
        Ok(items) => {
            info!(target: "4da::sources", source = "reddit", count = items.len(), "Deep fetched Reddit posts");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "reddit");
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "reddit", error = ?e, "Deep fetch failed");
        }
    }

    // Process GitHub results
    match github_result {
        Ok(items) => {
            info!(target: "4da::sources", source = "github", count = items.len(), "Fetched GitHub repos");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "github");
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "github", error = ?e, "Fetch failed");
        }
    }

    // Process RSS results
    match rss_result {
        Ok(items) => {
            info!(target: "4da::sources", source = "rss", count = items.len(), "Fetched RSS items");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "rss");
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "rss", error = ?e, "Fetch failed");
        }
    }

    // Process Twitter results
    match twitter_result {
        Ok(items) => {
            info!(target: "4da::sources", source = "twitter", count = items.len(), "Deep fetched Twitter items");
            process_source_items(
                db,
                &mut all_items,
                &mut new_items_to_embed,
                items,
                "twitter",
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "twitter", error = ?e, "Fetch failed");
        }
    }

    // Process YouTube results
    match youtube_result {
        Ok(items) => {
            info!(target: "4da::sources", source = "youtube", count = items.len(), "Fetched YouTube videos");
            process_source_items(
                db,
                &mut all_items,
                &mut new_items_to_embed,
                items,
                "youtube",
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "youtube", error = ?e, "Fetch failed");
        }
    }

    // Process Lobste.rs results
    match lobsters_result {
        Ok(items) => {
            info!(target: "4da::sources", source = "lobsters", count = items.len(), "Deep fetched Lobste.rs stories");
            process_source_items(
                db,
                &mut all_items,
                &mut new_items_to_embed,
                items,
                "lobsters",
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "lobsters", error = ?e, "Deep fetch failed");
        }
    }

    // Process Dev.to results
    match devto_result {
        Ok(items) => {
            info!(target: "4da::sources", source = "devto", count = items.len(), "Deep fetched Dev.to articles");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "devto");
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "devto", error = ?e, "Deep fetch failed");
        }
    }

    info!(target: "4da::sources",
        total_cached = all_items.len(),
        new_to_embed = new_items_to_embed.len(),
        "Deep fetch complete, now embedding new items"
    );

    // Embed new items in batches for better progress feedback
    if !new_items_to_embed.is_empty() {
        let total_to_embed = new_items_to_embed.len();
        let batch_size = 20; // Smaller batches for better progress feedback

        for (batch_idx, chunk) in new_items_to_embed.chunks(batch_size).enumerate() {
            let start_idx = batch_idx * batch_size;
            let progress = 0.55 + (0.35 * (start_idx as f32 / total_to_embed as f32));

            emit_progress(
                app,
                "embed",
                progress,
                &format!(
                    "Embedding batch {}/{} ({} items)...",
                    batch_idx + 1,
                    total_to_embed.div_ceil(batch_size),
                    chunk.len()
                ),
                all_items.len() + start_idx,
                all_items.len() + total_to_embed,
            );

            let texts: Vec<String> = chunk.iter().map(|(_, text)| text.clone()).collect();

            let embeddings = match embed_texts(&texts).await {
                Ok(emb) => emb,
                Err(e) => {
                    warn!(target: "4da::embed", error = %e, batch = batch_idx, "Embedding batch failed - using fallback");
                    vec![vec![0.0f32; 384]; texts.len()]
                }
            };

            for ((item, _), embedding) in chunk.iter().cloned().zip(embeddings.into_iter()) {
                let is_fallback = embedding.iter().all(|&v| v == 0.0);
                if !is_fallback {
                    db.upsert_source_item(
                        &item.source_type,
                        &item.source_id,
                        item.url.as_deref(),
                        &item.title,
                        &item.content,
                        &embedding,
                    )
                    .ok();
                }
                all_items.push((item, embedding));
            }
        }
    }

    info!(target: "4da::sources", total = all_items.len(), "DEEP SCAN: Total items from all sources");
    Ok(all_items)
}

#[cfg(test)]
mod tests {
    // ========================================================================
    // Content capping logic (500KB limit mirrors fetch_all_sources inline logic)
    // ========================================================================

    const CONTENT_CAP: usize = 500_000;

    #[test]
    fn test_content_cap_short_content_unchanged() {
        let content = "Short content that is well under the limit.".to_string();
        let capped = if content.len() > CONTENT_CAP {
            content[..CONTENT_CAP].to_string()
        } else {
            content.clone()
        };
        assert_eq!(
            capped, content,
            "Short content should pass through unchanged"
        );
    }

    #[test]
    fn test_content_cap_exactly_at_limit() {
        let content = "x".repeat(CONTENT_CAP);
        let capped = if content.len() > CONTENT_CAP {
            content[..CONTENT_CAP].to_string()
        } else {
            content.clone()
        };
        assert_eq!(
            capped.len(),
            CONTENT_CAP,
            "Exact-limit content should not be truncated"
        );
    }

    #[test]
    fn test_content_cap_over_limit_truncated() {
        let content = "y".repeat(CONTENT_CAP + 1000);
        let capped = if content.len() > CONTENT_CAP {
            content[..CONTENT_CAP].to_string()
        } else {
            content.clone()
        };
        assert_eq!(
            capped.len(),
            CONTENT_CAP,
            "Over-limit content should be truncated to 500KB"
        );
    }

    // ========================================================================
    // Fetch interval logic (300s cooldown mirrors fetch_all_sources)
    // ========================================================================

    #[test]
    fn test_fetch_interval_skip_logic() {
        // Simulates the 300-second fetch interval check
        let fetch_interval_secs = 300i64;

        // Recently fetched (10s ago) - should be skipped
        let recent_elapsed = 10i64;
        assert!(
            recent_elapsed < fetch_interval_secs,
            "10s ago should be within interval (should skip)"
        );

        // Long ago (600s) - should be fetched
        let old_elapsed = 600i64;
        assert!(
            old_elapsed >= fetch_interval_secs,
            "600s ago should be past interval (should fetch)"
        );

        // Exactly at boundary
        let boundary_elapsed = 300i64;
        assert!(
            boundary_elapsed >= fetch_interval_secs,
            "Exactly 300s should trigger fetch (not less than)"
        );
    }

    // ========================================================================
    // Retry backoff pattern (mirrors fetch_all_sources retry logic)
    // ========================================================================

    #[test]
    fn test_fetch_retry_backoff_pattern() {
        let backoff_ms = [500u64, 1000, 2000];
        let max_attempts = 3;

        // Attempt 1 (attempts=1, index 0) -> 500ms
        let mut attempts = 1;
        assert_eq!(backoff_ms.get(attempts - 1).copied().unwrap_or(2000), 500);

        // Attempt 2 -> 1000ms
        attempts = 2;
        assert_eq!(backoff_ms.get(attempts - 1).copied().unwrap_or(2000), 1000);

        // The loop breaks at max_attempts, so attempt 3 never indexes into backoff
        assert_eq!(max_attempts, 3);
    }

    // ========================================================================
    // Deep fetch batch progress calculation
    // ========================================================================

    #[test]
    fn test_deep_fetch_batch_progress_calculation() {
        // Mirrors the progress calculation in fetch_all_sources_deep
        let total_to_embed: usize = 100;
        let batch_size: usize = 20;

        for batch_idx in 0..total_to_embed.div_ceil(batch_size) {
            let start_idx = batch_idx * batch_size;
            let progress = 0.55 + (0.35 * (start_idx as f32 / total_to_embed as f32));

            // Progress should be between 0.55 and 0.90
            assert!(
                (0.55..0.91).contains(&progress),
                "Progress {} at batch {} should be in [0.55, 0.90)",
                progress,
                batch_idx
            );
        }

        // First batch starts at 0.55
        let first_progress = 0.55 + (0.35 * (0.0 / total_to_embed as f32));
        assert!(
            (first_progress - 0.55).abs() < f32::EPSILON,
            "First batch progress should be exactly 0.55"
        );
    }

    // ========================================================================
    // GenericSourceItem ID generation via hash
    // ========================================================================

    #[test]
    fn test_generic_item_id_from_source_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let source_type = "hackernews";
        let source_id = "12345";

        let id1 = {
            let mut hasher = DefaultHasher::new();
            format!("{}:{}", source_type, source_id).hash(&mut hasher);
            hasher.finish()
        };

        let id2 = {
            let mut hasher = DefaultHasher::new();
            format!("{}:{}", source_type, source_id).hash(&mut hasher);
            hasher.finish()
        };

        assert_eq!(id1, id2, "Same source should produce same ID");

        // Different source_type should produce different ID
        let id3 = {
            let mut hasher = DefaultHasher::new();
            format!("{}:{}", "reddit", source_id).hash(&mut hasher);
            hasher.finish()
        };
        assert_ne!(
            id1, id3,
            "Different source_type should produce different ID"
        );
    }
}
