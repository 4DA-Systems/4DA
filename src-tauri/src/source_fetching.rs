//! Source fetching functions extracted from lib.rs
//!
//! Contains: fetch_all_sources, fetch_all_sources_deep, fill_cache_background,
//! process_source_items, and settings loader helpers.

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
    build_embedding_text, embed_texts, emit_progress, get_database, get_settings_manager, sources,
    truncate_utf8, void_signal_cache_filled, void_signal_fetching, GenericSourceItem,
};

/// Fetch items from all sources (HN, arXiv, Reddit) directly
pub(crate) async fn fetch_all_sources(
    db: &Database,
    app: &AppHandle,
    max_items_per_source: usize,
) -> Result<Vec<(GenericSourceItem, Vec<f32>)>, String> {
    use sources::Source;

    // Phase 4: Network connectivity check before fetching
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .ok();

    let online = if let Some(c) = &client {
        c.head("https://httpbin.org/get").send().await.is_ok()
    } else {
        true // Assume online if client creation fails
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
                                source.scrape_content(&item).await.unwrap_or_default()
                            } else {
                                String::new()
                            }
                        } else {
                            item.content.clone()
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

/// Fill the cache with items from all sources (background operation)
/// This is the "write" side of the cache-first architecture
/// Does NOT emit progress events - runs silently in background
pub(crate) async fn fill_cache_background(app: &AppHandle) -> Result<usize, String> {
    use sources::Source;

    info!(target: "4da::cache", "=== BACKGROUND CACHE FILL STARTED ===");
    void_signal_fetching(app);

    let db = get_database()?;
    let rss_feeds = load_rss_feeds_from_settings();
    let (twitter_handles, x_api_key) = load_twitter_settings();
    let youtube_channels = load_youtube_channels_from_settings();

    // Use deep fetch for comprehensive coverage
    let hn_source = HackerNewsSource::new();
    let arxiv_source = ArxivSource::new();
    let reddit_source = RedditSource::new();
    let github_source = GitHubSource::with_languages(load_github_languages_from_settings());
    let rss_source = RssSource::with_feeds(rss_feeds);
    let twitter_source = TwitterSource::with_handles(twitter_handles).with_api_key(x_api_key);
    let youtube_source = YouTubeSource::with_channels(youtube_channels);
    let lobsters_source = LobstersSource::new();
    let devto_source = DevtoSource::new();

    let mut total_cached = 0;
    let mut new_items_to_embed: Vec<(String, String, Option<String>, String, String)> = Vec::new();

    // Fetch from all sources in parallel
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
        hn_source.fetch_items_deep(50),
        arxiv_source.fetch_items_deep(50),
        reddit_source.fetch_items_deep(50),
        github_source.fetch_items(),
        rss_source.fetch_items(),
        twitter_source.fetch_items_deep(50),
        youtube_source.fetch_items(),
        lobsters_source.fetch_items_deep(50),
        devto_source.fetch_items_deep(50),
    );

    // Process HN results
    match hn_result {
        Ok(items) => {
            info!(target: "4da::cache", source = "hackernews", count = items.len(), "Fetched HN items");
            for item in items {
                if db
                    .get_source_item("hackernews", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "hackernews".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("hackernews", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "hackernews", error = ?e, "Fetch failed"),
    }

    // Process arXiv results
    match arxiv_result {
        Ok(items) => {
            info!(target: "4da::cache", source = "arxiv", count = items.len(), "Fetched arXiv items");
            for item in items {
                if db
                    .get_source_item("arxiv", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "arxiv".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("arxiv", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "arxiv", error = ?e, "Fetch failed"),
    }

    // Process Reddit results
    match reddit_result {
        Ok(items) => {
            info!(target: "4da::cache", source = "reddit", count = items.len(), "Fetched Reddit items");
            for item in items {
                if db
                    .get_source_item("reddit", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "reddit".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("reddit", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "reddit", error = ?e, "Fetch failed"),
    }

    // Process GitHub results
    match github_result {
        Ok(items) => {
            info!(target: "4da::cache", source = "github", count = items.len(), "Fetched GitHub items");
            for item in items {
                if db
                    .get_source_item("github", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "github".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("github", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "github", error = ?e, "Fetch failed"),
    }

    // Process RSS results
    match rss_result {
        Ok(items) => {
            info!(target: "4da::cache", source = "rss", count = items.len(), "Fetched RSS items");
            for item in items {
                if db
                    .get_source_item("rss", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "rss".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("rss", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "rss", error = ?e, "Fetch failed"),
    }

    // Process Twitter results
    match twitter_result {
        Ok(items) => {
            info!(target: "4da::cache", source = "twitter", count = items.len(), "Fetched Twitter items");
            for item in items {
                if db
                    .get_source_item("twitter", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "twitter".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("twitter", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "twitter", error = ?e, "Fetch failed"),
    }

    // Process YouTube results
    match youtube_result {
        Ok(items) => {
            info!(target: "4da::cache", source = "youtube", count = items.len(), "Fetched YouTube items");
            for item in items {
                if db
                    .get_source_item("youtube", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "youtube".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("youtube", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "youtube", error = ?e, "Fetch failed"),
    }

    // Process Lobste.rs results
    match lobsters_result {
        Ok(items) => {
            info!(target: "4da::cache", source = "lobsters", count = items.len(), "Fetched Lobste.rs items");
            for item in items {
                if db
                    .get_source_item("lobsters", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "lobsters".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("lobsters", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "lobsters", error = ?e, "Fetch failed"),
    }

    // Process Dev.to results
    match devto_result {
        Ok(items) => {
            info!(target: "4da::cache", source = "devto", count = items.len(), "Fetched Dev.to items");
            for item in items {
                if db
                    .get_source_item("devto", &item.source_id)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    new_items_to_embed.push((
                        "devto".to_string(),
                        item.source_id,
                        item.url,
                        item.title,
                        item.content,
                    ));
                } else {
                    db.touch_source_item("devto", &item.source_id).ok();
                    total_cached += 1;
                }
            }
        }
        Err(e) => warn!(target: "4da::cache", source = "devto", error = ?e, "Fetch failed"),
    }

    // Embed and cache new items
    if !new_items_to_embed.is_empty() {
        info!(target: "4da::cache", new_items = new_items_to_embed.len(), "Embedding new items");

        // Decode HTML entities at ingestion time
        let new_items_to_embed: Vec<_> = new_items_to_embed
            .into_iter()
            .map(|(st, sid, url, title, content)| {
                (
                    st,
                    sid,
                    url,
                    crate::decode_html_entities(&title),
                    crate::decode_html_entities(&content),
                )
            })
            .collect();

        let texts: Vec<String> = new_items_to_embed
            .iter()
            .map(|(_, _, _, title, content)| build_embedding_text(title, content))
            .collect();

        match embed_texts(&texts).await {
            Ok(embeddings) => {
                // Batch upsert: collect non-fallback items for transaction
                #[allow(clippy::type_complexity)]
                let items_to_insert: Vec<(
                    String,
                    String,
                    Option<String>,
                    String,
                    String,
                    Vec<f32>,
                )> = new_items_to_embed
                    .into_iter()
                    .zip(embeddings.into_iter())
                    .filter(|(_, embedding)| !embedding.iter().all(|&v| v == 0.0))
                    .map(
                        |((source_type, source_id, url, title, content), embedding)| {
                            (source_type, source_id, url, title, content, embedding)
                        },
                    )
                    .collect();

                let count = items_to_insert.len();
                if !items_to_insert.is_empty() {
                    db.batch_upsert_source_items(&items_to_insert).ok();
                    total_cached += count;
                }
            }
            Err(e) => {
                warn!(target: "4da::cache", error = %e, "Embedding failed - items not cached");
            }
        }
    }

    // Emit cache update event
    let _ = app.emit("cache-updated", total_cached);
    void_signal_cache_filled(app);

    info!(target: "4da::cache", total = total_cached, "=== BACKGROUND CACHE FILL COMPLETE ===");
    Ok(total_cached)
}

/// Helper to process source items into cache/embed lists
pub(crate) fn process_source_items(
    db: &Database,
    all_items: &mut Vec<(GenericSourceItem, Vec<f32>)>,
    new_items_to_embed: &mut Vec<(GenericSourceItem, String)>,
    items: Vec<sources::SourceItem>,
    source_type: &str,
) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    for item in items {
        let id = {
            let mut hasher = DefaultHasher::new();
            format!("{}:{}", source_type, item.source_id).hash(&mut hasher);
            hasher.finish()
        };

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
            let generic = GenericSourceItem {
                id,
                source_id: item.source_id.clone(),
                source_type: source_type.to_string(),
                title: item.title.clone(),
                url: item.url.clone(),
                content: item.content.clone(),
            };

            let embed_text = build_embedding_text(&item.title, &item.content);
            new_items_to_embed.push((generic, embed_text));
        }
    }
}

/// Load RSS feed URLs from settings
pub(crate) fn load_rss_feeds_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let feeds = settings.get_rss_feeds();
    drop(settings);
    feeds
}

/// Load Twitter handles and X API key from settings
pub(crate) fn load_twitter_settings() -> (Vec<String>, String) {
    let settings = get_settings_manager().lock();
    let handles = settings.get_twitter_handles();
    let api_key = settings.get_x_api_key();
    drop(settings);
    (handles, api_key)
}

/// Load YouTube channel IDs from settings
pub(crate) fn load_youtube_channels_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let channels = settings.get_youtube_channels();
    drop(settings);
    channels
}

/// Load GitHub languages from settings (defaults if empty)
pub(crate) fn load_github_languages_from_settings() -> Vec<String> {
    let settings = get_settings_manager().lock();
    let langs = settings.get_github_languages();
    drop(settings);
    if langs.is_empty() {
        vec![
            "rust".to_string(),
            "typescript".to_string(),
            "python".to_string(),
        ]
    } else {
        langs
    }
}
