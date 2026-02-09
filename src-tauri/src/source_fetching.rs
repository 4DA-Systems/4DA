//! Source fetching functions extracted from lib.rs
//!
//! Contains: fetch_all_sources, fetch_all_sources_deep, fill_cache_background,
//! process_source_items, and settings loader helpers.

use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};

use crate::db::Database;
use crate::sources::arxiv::ArxivSource;
use crate::sources::github::GitHubSource;
use crate::sources::hackernews::HackerNewsSource;
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

    // Create sources directly (avoid holding MutexGuard across await)
    let rss_feeds = load_rss_feeds_from_settings();
    let (twitter_handles, x_api_key) = load_twitter_settings();
    let youtube_channels = load_youtube_channels_from_settings();
    let sources: Vec<Box<dyn Source>> = vec![
        Box::new(HackerNewsSource::new()),
        Box::new(ArxivSource::new()),
        Box::new(RedditSource::new()),
        Box::new(GitHubSource::with_languages(
            load_github_languages_from_settings(),
        )),
        Box::new(RssSource::with_feeds(rss_feeds)),
        Box::new(TwitterSource::with_handles(twitter_handles).with_api_key(x_api_key)),
        Box::new(YouTubeSource::with_channels(youtube_channels)),
    ];

    info!(target: "4da::sources", count = sources.len(), "Fetching from sources");

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

        // Fetch items from this source with retry
        let fetch_result = {
            let mut attempts = 0;
            let max_attempts = 2;
            loop {
                attempts += 1;
                match source.fetch_items().await {
                    Ok(items) => break Ok(items),
                    Err(e) if attempts < max_attempts => {
                        warn!(target: "4da::sources", source = source_name, attempt = attempts, error = ?e, "Fetch failed, retrying...");
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                    Err(e) => break Err(e),
                }
            }
        };

        match fetch_result {
            Ok(items) => {
                info!(target: "4da::sources", source = source_name, count = items.len(), "Fetched items from source");

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
                        db.touch_source_item(source_type, &item.source_id).ok();
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
        let embeddings = match embed_texts(&texts) {
            Ok(emb) => emb,
            Err(e) => {
                warn!(target: "4da::embed", error = %e, count = texts.len(),
                    "Embedding service unavailable - using fallback (keyword-only scoring)");
                // Create zero vectors as fallback - items will score via keyword matching only
                vec![vec![0.0f32; 384]; texts.len()]
            }
        };

        for ((item, _), embedding) in new_items_to_embed.into_iter().zip(embeddings.into_iter()) {
            // Cache in database (skip if embedding failed - zero vector)
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

    // Note: HN, arXiv, and Reddit have fetch_items_deep implementations
    // GitHub, RSS, YouTube use default (regular fetch). Twitter has deep fetch.
    let hn_source = HackerNewsSource::new();
    let arxiv_source = ArxivSource::new();
    let reddit_source = RedditSource::new();
    let github_source = GitHubSource::with_languages(load_github_languages_from_settings());
    let rss_source = RssSource::with_feeds(rss_feeds);
    let twitter_source = TwitterSource::with_handles(twitter_handles).with_api_key(x_api_key);
    let youtube_source = YouTubeSource::with_channels(youtube_channels);

    let mut all_items: Vec<(GenericSourceItem, Vec<f32>)> = Vec::new();
    let mut new_items_to_embed: Vec<(GenericSourceItem, String)> = Vec::new();

    // Fetch from each source using deep fetch where available
    // HN deep fetch (top + new + best + ask + show = ~200+ unique items)
    emit_progress(
        app,
        "fetch",
        0.12,
        "Deep fetching Hacker News (5 categories)...",
        0,
        0,
    );
    match hn_source.fetch_items_deep(items_per_category).await {
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

    // arXiv deep fetch (16 categories = ~100+ papers)
    emit_progress(
        app,
        "fetch",
        0.25,
        "Deep fetching arXiv (16 categories)...",
        all_items.len(),
        0,
    );
    match arxiv_source.fetch_items_deep(items_per_category).await {
        Ok(items) => {
            info!(target: "4da::sources", source = "arxiv", count = items.len(), "Deep fetched arXiv papers");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "arxiv");
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "arxiv", error = ?e, "Deep fetch failed");
        }
    }

    // Reddit deep fetch (40+ subreddits = ~200+ posts)
    emit_progress(
        app,
        "fetch",
        0.35,
        "Deep fetching Reddit (40+ subreddits)...",
        all_items.len(),
        0,
    );
    match reddit_source.fetch_items_deep(items_per_category).await {
        Ok(items) => {
            info!(target: "4da::sources", source = "reddit", count = items.len(), "Deep fetched Reddit posts");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "reddit");
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "reddit", error = ?e, "Deep fetch failed");
        }
    }

    // GitHub (regular fetch - trending is already comprehensive)
    emit_progress(
        app,
        "fetch",
        0.45,
        "Fetching GitHub trending...",
        all_items.len(),
        0,
    );
    match github_source.fetch_items().await {
        Ok(items) => {
            info!(target: "4da::sources", source = "github", count = items.len(), "Fetched GitHub repos");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "github");
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "github", error = ?e, "Fetch failed");
        }
    }

    // RSS (regular fetch)
    emit_progress(
        app,
        "fetch",
        0.45,
        "Fetching RSS feeds...",
        all_items.len(),
        0,
    );
    match rss_source.fetch_items().await {
        Ok(items) => {
            info!(target: "4da::sources", source = "rss", count = items.len(), "Fetched RSS items");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "rss");
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "rss", error = ?e, "Fetch failed");
        }
    }

    // Twitter/X deep fetch (timeline + search)
    emit_progress(
        app,
        "fetch",
        0.55,
        "Fetching Twitter/X...",
        all_items.len(),
        0,
    );
    match twitter_source.fetch_items_deep(items_per_category).await {
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

    // YouTube (regular fetch - RSS feeds)
    emit_progress(
        app,
        "fetch",
        0.60,
        "Fetching YouTube feeds...",
        all_items.len(),
        0,
    );
    match youtube_source.fetch_items().await {
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

            let embeddings = match embed_texts(&texts) {
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

    let mut total_cached = 0;
    let mut new_items_to_embed: Vec<(String, String, Option<String>, String, String)> = Vec::new();

    // HN deep fetch
    match hn_source.fetch_items_deep(50).await {
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

    // arXiv deep fetch
    match arxiv_source.fetch_items_deep(50).await {
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

    // Reddit deep fetch
    match reddit_source.fetch_items_deep(50).await {
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

    // GitHub fetch
    match github_source.fetch_items().await {
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

    // RSS fetch
    match rss_source.fetch_items().await {
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

    // Twitter/X deep fetch
    match twitter_source.fetch_items_deep(50).await {
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

    // YouTube fetch
    match youtube_source.fetch_items().await {
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

    // Embed and cache new items
    if !new_items_to_embed.is_empty() {
        info!(target: "4da::cache", new_items = new_items_to_embed.len(), "Embedding new items");

        let texts: Vec<String> = new_items_to_embed
            .iter()
            .map(|(_, _, _, title, content)| build_embedding_text(title, content))
            .collect();

        match embed_texts(&texts) {
            Ok(embeddings) => {
                for ((source_type, source_id, url, title, content), embedding) in
                    new_items_to_embed.into_iter().zip(embeddings.into_iter())
                {
                    if !embedding.iter().all(|&v| v == 0.0) {
                        db.upsert_source_item(
                            &source_type,
                            &source_id,
                            url.as_deref(),
                            &title,
                            &content,
                            &embedding,
                        )
                        .ok();
                        total_cached += 1;
                    }
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
            db.touch_source_item(source_type, &item.source_id).ok();
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
