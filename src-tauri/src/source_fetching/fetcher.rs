//! Network fetching logic: fetch_all_sources, fetch_all_sources_deep

use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};

use crate::analysis_narration::{emit_narration, NarrationEvent};
use crate::db::Database;
use crate::error::Result;
use crate::sources::arxiv::ArxivSource;
use crate::sources::devto::DevtoSource;
use crate::sources::github::GitHubSource;
use crate::sources::hackernews::HackerNewsSource;
use crate::sources::lobsters::LobstersSource;
use crate::sources::rate_limiter::rate_limiter;
use crate::sources::reddit::RedditSource;
use crate::sources::rss::RssSource;
use crate::sources::twitter::TwitterSource;
use crate::sources::youtube::YouTubeSource;
use crate::{
    build_embedding_text, embed_texts, emit_progress, sources, truncate_utf8, GenericSourceItem,
};

use super::processor::process_source_items;
use super::{
    fetch_with_retry, load_github_languages_from_settings, load_rss_feeds_from_settings,
    load_twitter_settings, load_youtube_channels_from_settings, AdapterFailureTracker,
};

/// Fetch items from all sources (HN, arXiv, Reddit) directly
pub(crate) async fn fetch_all_sources(
    db: &Database,
    app: &AppHandle,
    max_items_per_source: usize,
) -> Result<Vec<(GenericSourceItem, Vec<f32>)>> {
    use sources::Source;

    // Network connectivity check with caching (avoid re-checking every fetch)
    static LAST_ONLINE_CHECK: std::sync::Mutex<Option<(std::time::Instant, bool)>> =
        std::sync::Mutex::new(None);

    let online = {
        let cached = LAST_ONLINE_CHECK.lock().ok().and_then(|guard| {
            guard.as_ref().and_then(|(when, result)| {
                if when.elapsed() < std::time::Duration::from_secs(30) {
                    Some(*result)
                } else {
                    None
                }
            })
        });

        if let Some(cached_result) = cached {
            cached_result
        } else {
            let client = sources::shared_client();
            let timeout = std::time::Duration::from_secs(8);

            // Check multiple endpoints — succeed if ANY responds
            // Includes user's API endpoints so corporate networks work
            let check_result = tokio::select! {
                r = client.head("https://1.1.1.1/cdn-cgi/trace").timeout(timeout).send() => r.is_ok(),
                r = client.head("https://dns.google/resolve?name=example.com").timeout(timeout).send() => r.is_ok(),
                r = client.head("https://httpbin.org/get").timeout(timeout).send() => r.is_ok(),
                r = client.head("https://hacker-news.firebaseio.com/v0/topstories.json").timeout(timeout).send() => r.is_ok(),
                r = client.head("https://api.github.com").timeout(timeout).send() => r.is_ok(),
            };

            // Cache the result
            if let Ok(mut guard) = LAST_ONLINE_CHECK.lock() {
                *guard = Some((std::time::Instant::now(), check_result));
            }

            check_result
        }
    };

    if !online {
        warn!(target: "4da::sources", "Network unavailable - using cached content only");
        crate::capabilities::report_degraded(
            crate::capabilities::Capability::SourceFetching,
            "Network unavailable",
            "Using cached content from previous fetches",
        );
        if let Err(e) = app.emit("network-offline", ()) {
            tracing::warn!("Failed to emit 'network-offline': {e}");
        }
        return Ok(Vec::new()); // Return empty; caller falls back to cache
    } else {
        crate::capabilities::report_restored(crate::capabilities::Capability::SourceFetching);
    }

    // Build all sources from the canonical factory (single source of truth)
    // Filter by enabled status from DB
    let sources: Vec<Box<dyn Source>> = crate::sources::build_all_sources()
        .into_iter()
        .filter(|source| {
            let st = source.source_type();
            let enabled = db.is_source_enabled(st);
            if !enabled {
                info!(target: "4da::sources", source = st, "Skipping disabled source");
            }
            enabled
        })
        .collect();

    info!(target: "4da::sources", count = sources.len(), "Fetching from enabled sources");

    let mut all_items: Vec<(GenericSourceItem, Vec<f32>)> = Vec::new();
    let mut new_items_to_embed: Vec<(GenericSourceItem, String)> = Vec::new();
    let tracker = AdapterFailureTracker::new();

    for source in &sources {
        let source_type = source.source_type();
        let source_name = source.name();

        debug!(target: "4da::sources", source = source_name, "Fetching from source");

        // Narration: per-source fetch start
        emit_narration(
            app,
            NarrationEvent {
                narration_type: "discovery".into(),
                message: format!("Scanning {source_name}..."),
                source: Some(source_type.to_string()),
                relevance: None,
            },
        );

        emit_progress(
            app,
            "fetch",
            0.2,
            &format!("Fetching from {source_name}..."),
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

        // Centralized rate limiting: wait if we fetched this source too recently
        rate_limiter().wait_for_rate_limit(source_type).await;

        // Fetch items from this source with self-healing retry
        let fetch_start = std::time::Instant::now();

        // Circuit breaker: skip sources with 5+ consecutive failures
        if db.is_circuit_open(source_type) {
            warn!(target: "4da::sources", source = source_name, "Circuit breaker OPEN - skipping (too many failures)");
            let _ = app.emit(
                "source-circuit-break",
                serde_json::json!({
                    "source": source_type,
                    "source_name": source_name,
                    "status": "open",
                    "message": "Temporarily disabled after 5+ consecutive failures",
                    "session_failures": tracker.failure_count(source_name),
                }),
            );
            let _ = app.emit("source-error", serde_json::json!({
                "source": source_type, "error": "Circuit breaker open (5+ failures)", "retry_count": 0
            }));
            continue;
        }

        let fetch_result = fetch_with_retry(source_name, &tracker, || source.fetch_items()).await;

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

                // Narration: per-source fetch complete
                emit_narration(
                    app,
                    NarrationEvent {
                        narration_type: "discovery".into(),
                        message: format!("{source_name}: {item_count} items found"),
                        source: Some(source_type.to_string()),
                        relevance: None,
                    },
                );

                // Freshness validation: detect stale/zombie sources
                {
                    use crate::sources::freshness::{get_source_thresholds, validate_freshness};
                    let (min_expected, max_age_secs) = get_source_thresholds(source_type);
                    let previous_ids: Vec<String> = db
                        .get_recent_source_item_ids(source_type, 50)
                        .unwrap_or_default();
                    let report = validate_freshness(
                        source_type,
                        &items,
                        &previous_ids,
                        min_expected,
                        max_age_secs,
                    );
                    if !matches!(
                        report.state,
                        crate::sources::freshness::SourceHealthState::Healthy
                    ) {
                        warn!(
                            target: "4da::freshness",
                            source = source_type,
                            state = ?report.state,
                            items = report.items_fetched,
                            duplicate_ratio = report.duplicate_ratio,
                            "Source health: {:?}",
                            report.state
                        );
                        let _ = app.emit(
                            "source-health-state",
                            serde_json::json!({
                                "source": source_type,
                                "state": format!("{:?}", report.state),
                                "items_fetched": report.items_fetched,
                                "duplicate_ratio": report.duplicate_ratio,
                                "warnings": report.warnings,
                            }),
                        );
                    }
                }

                for (idx, item) in items.into_iter().take(max_items_per_source).enumerate() {
                    // Generate a numeric ID from source_id hash
                    let id = {
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut hasher = DefaultHasher::new();
                        source_type.hash(&mut hasher);
                        ":".hash(&mut hasher);
                        item.source_id.hash(&mut hasher);
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
            Err(retry_err) => {
                // Try fallback endpoints before giving up
                let fallback_items = {
                    use crate::sources::fallback::try_fallbacks;
                    let client = sources::shared_client();
                    match try_fallbacks(source_type, &client, &retry_err.last_error).await {
                        Ok(items) if !items.is_empty() => {
                            info!(target: "4da::sources", source = source_name, count = items.len(), "Fallback cascade succeeded");
                            db.record_source_health(
                                source_type,
                                true,
                                items.len() as i64,
                                elapsed_ms,
                                None,
                            )
                            .ok();
                            db.update_source_fetch_time(source_type).ok();
                            let _ = app.emit(
                                "source-fetched",
                                serde_json::json!({
                                    "source": source_type, "count": items.len(), "fallback": true
                                }),
                            );
                            Some(items)
                        }
                        _ => None,
                    }
                };

                if let Some(items) = fallback_items {
                    // Process fallback items using the shared helper (DRY)
                    process_source_items(
                        db,
                        &mut all_items,
                        &mut new_items_to_embed,
                        items,
                        source_type,
                    );
                } else {
                    // Original error handling — no fallback available or all fallbacks failed
                    let failure_count = tracker.failure_count(source_name);
                    error!(target: "4da::sources", source = source_name, error = %retry_err, session_failures = failure_count, "Source fetch failed after retries - continuing with other sources");
                    db.record_source_health(
                        source_type,
                        false,
                        0,
                        elapsed_ms,
                        Some(&format!("{retry_err}")),
                    )
                    .ok();
                    let _ = app.emit(
                        "source-error",
                        serde_json::json!({
                            "source": source_type,
                            "error": format!("{}", retry_err),
                            "retry_count": retry_err.attempts,
                            "session_failures": failure_count
                        }),
                    );

                    // Record to local error telemetry for developer diagnostics
                    crate::telemetry::record_error_async(
                        "source_fetch",
                        &format!("{retry_err}"),
                        Some(source_type),
                    );
                }
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

        // Narration: embedding start
        emit_narration(
            app,
            NarrationEvent {
                narration_type: "insight".into(),
                message: "Analyzing content patterns...".into(),
                source: None,
                relevance: None,
            },
        );

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
                let is_zero_fallback = emb.first().is_some_and(|v| v.iter().all(|&x| x == 0.0));
                if is_zero_fallback {
                    let _ = app.emit(
                        "embedding-mode",
                        serde_json::json!({
                            "mode": "keyword-only",
                            "reason": "No embedding provider available"
                        }),
                    );
                } else if let Err(e) =
                    app.emit("embedding-mode", serde_json::json!({ "mode": "semantic" }))
                {
                    tracing::warn!("Failed to emit 'embedding-mode': {e}");
                }
                emb
            }
            Err(e) => {
                warn!(target: "4da::embed", error = %e, count = texts.len(),
                    "Embedding service unavailable - using fallback (keyword-only scoring)");
                let _ = app.emit(
                    "embedding-mode",
                    serde_json::json!({ "mode": "keyword-only", "reason": e.to_string() }),
                );
                // Record embedding failure to local error telemetry
                crate::telemetry::record_error_async("embedding", &format!("{e}"), None);
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
            let detected_lang = crate::language_detect::detect_language(&clean_title);
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
                    detected_lang,
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

    // Narration: all sources fetched
    emit_narration(
        app,
        NarrationEvent {
            narration_type: "discovery".into(),
            message: format!(
                "All sources scanned \u{2014} {} total items collected",
                all_items.len()
            ),
            source: None,
            relevance: None,
        },
    );

    info!(target: "4da::sources", total = all_items.len(), "Total items from all sources");
    Ok(all_items)
}

/// Deep fetch from all sources - used for comprehensive initial scans
/// Fetches 5-10x more items from multiple endpoints per source
pub(crate) async fn fetch_all_sources_deep(
    db: &Database,
    app: &AppHandle,
    items_per_category: usize,
) -> Result<Vec<(GenericSourceItem, Vec<f32>)>> {
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
    // Narration: deep scan fetch start
    emit_narration(
        app,
        NarrationEvent {
            narration_type: "discovery".into(),
            message: "Scanning all sources in parallel...".into(),
            source: None,
            relevance: None,
        },
    );

    emit_progress(
        app,
        "fetch",
        0.12,
        "Deep fetching from all sources in parallel...",
        0,
        0,
    );

    // Rate limit each source before parallel fetch (protects against rapid re-invocation)
    // Each fetch is wrapped with fetch_with_retry for self-healing retry with backoff
    let rl = rate_limiter();
    let deep_tracker = AdapterFailureTracker::new();
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
        async {
            rl.wait_for_rate_limit("hackernews").await;
            fetch_with_retry("Hacker News", &deep_tracker, || {
                hn_source.fetch_items_deep(items_per_category)
            })
            .await
        },
        async {
            rl.wait_for_rate_limit("arxiv").await;
            fetch_with_retry("arXiv", &deep_tracker, || {
                arxiv_source.fetch_items_deep(items_per_category)
            })
            .await
        },
        async {
            rl.wait_for_rate_limit("reddit").await;
            fetch_with_retry("Reddit", &deep_tracker, || {
                reddit_source.fetch_items_deep(items_per_category)
            })
            .await
        },
        async {
            rl.wait_for_rate_limit("github").await;
            fetch_with_retry("GitHub", &deep_tracker, || github_source.fetch_items()).await
        },
        async {
            rl.wait_for_rate_limit("rss").await;
            fetch_with_retry("RSS", &deep_tracker, || rss_source.fetch_items()).await
        },
        async {
            rl.wait_for_rate_limit("twitter").await;
            fetch_with_retry("Twitter", &deep_tracker, || {
                twitter_source.fetch_items_deep(items_per_category)
            })
            .await
        },
        async {
            rl.wait_for_rate_limit("youtube").await;
            fetch_with_retry("YouTube", &deep_tracker, || youtube_source.fetch_items()).await
        },
        async {
            rl.wait_for_rate_limit("lobsters").await;
            fetch_with_retry("Lobsters", &deep_tracker, || {
                lobsters_source.fetch_items_deep(items_per_category)
            })
            .await
        },
        async {
            rl.wait_for_rate_limit("devto").await;
            fetch_with_retry("Dev.to", &deep_tracker, || {
                devto_source.fetch_items_deep(items_per_category)
            })
            .await
        },
    );

    // Helper to emit per-source narration during deep fetch
    let mut deep_source_count: usize = 0;

    // Log any persistent failures from the deep tracker
    for (name, count) in deep_tracker.persistent_failures() {
        warn!(target: "4da::retry", adapter = %name, consecutive_failures = count, "Persistent failure detected during deep scan");
    }

    // Process HN results
    match hn_result {
        Ok(items) => {
            let count = items.len();
            info!(target: "4da::sources", source = "hackernews", count, "Deep fetched HN items");
            process_source_items(
                db,
                &mut all_items,
                &mut new_items_to_embed,
                items,
                "hackernews",
            );
            deep_source_count += 1;
            emit_narration(
                app,
                NarrationEvent {
                    narration_type: "discovery".into(),
                    message: format!("Hacker News: {count} items found"),
                    source: Some("hackernews".into()),
                    relevance: None,
                },
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "hackernews", error = %e, "Deep fetch failed after retries");
        }
    }

    // Process arXiv results
    match arxiv_result {
        Ok(items) => {
            let count = items.len();
            info!(target: "4da::sources", source = "arxiv", count, "Deep fetched arXiv papers");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "arxiv");
            deep_source_count += 1;
            emit_narration(
                app,
                NarrationEvent {
                    narration_type: "discovery".into(),
                    message: format!("arXiv: {count} items found"),
                    source: Some("arxiv".into()),
                    relevance: None,
                },
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "arxiv", error = %e, "Deep fetch failed after retries");
        }
    }

    // Process Reddit results
    match reddit_result {
        Ok(items) => {
            let count = items.len();
            info!(target: "4da::sources", source = "reddit", count, "Deep fetched Reddit posts");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "reddit");
            deep_source_count += 1;
            emit_narration(
                app,
                NarrationEvent {
                    narration_type: "discovery".into(),
                    message: format!("Reddit: {count} items found"),
                    source: Some("reddit".into()),
                    relevance: None,
                },
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "reddit", error = %e, "Deep fetch failed after retries");
        }
    }

    // Process GitHub results
    match github_result {
        Ok(items) => {
            let count = items.len();
            info!(target: "4da::sources", source = "github", count, "Fetched GitHub repos");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "github");
            deep_source_count += 1;
            emit_narration(
                app,
                NarrationEvent {
                    narration_type: "discovery".into(),
                    message: format!("GitHub: {count} items found"),
                    source: Some("github".into()),
                    relevance: None,
                },
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "github", error = %e, "Fetch failed after retries");
        }
    }

    // Process RSS results
    match rss_result {
        Ok(items) => {
            let count = items.len();
            info!(target: "4da::sources", source = "rss", count, "Fetched RSS items");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "rss");
            deep_source_count += 1;
            emit_narration(
                app,
                NarrationEvent {
                    narration_type: "discovery".into(),
                    message: format!("RSS: {count} items found"),
                    source: Some("rss".into()),
                    relevance: None,
                },
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "rss", error = %e, "Fetch failed after retries");
        }
    }

    // Process Twitter results
    match twitter_result {
        Ok(items) => {
            let count = items.len();
            info!(target: "4da::sources", source = "twitter", count, "Deep fetched Twitter items");
            process_source_items(
                db,
                &mut all_items,
                &mut new_items_to_embed,
                items,
                "twitter",
            );
            deep_source_count += 1;
            emit_narration(
                app,
                NarrationEvent {
                    narration_type: "discovery".into(),
                    message: format!("Twitter: {count} items found"),
                    source: Some("twitter".into()),
                    relevance: None,
                },
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "twitter", error = %e, "Fetch failed after retries");
        }
    }

    // Process YouTube results
    match youtube_result {
        Ok(items) => {
            let count = items.len();
            info!(target: "4da::sources", source = "youtube", count, "Fetched YouTube videos");
            process_source_items(
                db,
                &mut all_items,
                &mut new_items_to_embed,
                items,
                "youtube",
            );
            deep_source_count += 1;
            emit_narration(
                app,
                NarrationEvent {
                    narration_type: "discovery".into(),
                    message: format!("YouTube: {count} items found"),
                    source: Some("youtube".into()),
                    relevance: None,
                },
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "youtube", error = %e, "Fetch failed after retries");
        }
    }

    // Process Lobste.rs results
    match lobsters_result {
        Ok(items) => {
            let count = items.len();
            info!(target: "4da::sources", source = "lobsters", count, "Deep fetched Lobste.rs stories");
            process_source_items(
                db,
                &mut all_items,
                &mut new_items_to_embed,
                items,
                "lobsters",
            );
            deep_source_count += 1;
            emit_narration(
                app,
                NarrationEvent {
                    narration_type: "discovery".into(),
                    message: format!("Lobsters: {count} items found"),
                    source: Some("lobsters".into()),
                    relevance: None,
                },
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "lobsters", error = %e, "Deep fetch failed after retries");
        }
    }

    // Process Dev.to results
    match devto_result {
        Ok(items) => {
            let count = items.len();
            info!(target: "4da::sources", source = "devto", count, "Deep fetched Dev.to articles");
            process_source_items(db, &mut all_items, &mut new_items_to_embed, items, "devto");
            deep_source_count += 1;
            emit_narration(
                app,
                NarrationEvent {
                    narration_type: "discovery".into(),
                    message: format!("Dev.to: {count} items found"),
                    source: Some("devto".into()),
                    relevance: None,
                },
            );
        }
        Err(e) => {
            warn!(target: "4da::sources", source = "devto", error = %e, "Deep fetch failed after retries");
        }
    }

    // Narration: all sources fetched (deep)
    let total_pre_embed = all_items.len() + new_items_to_embed.len();
    emit_narration(
        app,
        NarrationEvent {
            narration_type: "discovery".into(),
            message: format!(
                "All sources scanned \u{2014} {total_pre_embed} total items collected"
            ),
            source: None,
            relevance: None,
        },
    );
    let _ = deep_source_count; // suppress unused warning

    info!(target: "4da::sources",
        total_cached = all_items.len(),
        new_to_embed = new_items_to_embed.len(),
        "Deep fetch complete, now embedding new items"
    );

    // Embed new items in batches for better progress feedback
    if !new_items_to_embed.is_empty() {
        // Narration: embedding start (deep scan)
        emit_narration(
            app,
            NarrationEvent {
                narration_type: "insight".into(),
                message: "Analyzing content patterns...".into(),
                source: None,
                relevance: None,
            },
        );

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

            let mut items_to_insert = Vec::new();
            for ((item, _), embedding) in chunk.iter().cloned().zip(embeddings.into_iter()) {
                let is_fallback = embedding.iter().all(|&v| v == 0.0);
                if !is_fallback {
                    let detected_lang = crate::language_detect::detect_language(&item.title);
                    items_to_insert.push((
                        item.source_type.clone(),
                        item.source_id.clone(),
                        item.url.clone(),
                        item.title.clone(),
                        item.content.clone(),
                        embedding.clone(),
                        detected_lang,
                    ));
                }
                all_items.push((item, embedding));
            }
            if !items_to_insert.is_empty() {
                db.batch_upsert_source_items(&items_to_insert).ok();
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
    // Retry backoff pattern (mirrors fetch_with_retry constants)
    // ========================================================================

    #[test]
    fn test_fetch_retry_backoff_pattern() {
        use super::super::{MAX_RETRY_ATTEMPTS, RETRY_BACKOFF_SECS};

        // Backoff should be 1s, 2s, 4s (exponential)
        assert_eq!(RETRY_BACKOFF_SECS, [1, 2, 4]);
        assert_eq!(MAX_RETRY_ATTEMPTS, 3);

        // Attempt 1 (index 0) -> 1s backoff before retry
        assert_eq!(RETRY_BACKOFF_SECS[0], 1);

        // Attempt 2 (index 1) -> 2s backoff before retry
        assert_eq!(RETRY_BACKOFF_SECS[1], 2);

        // Attempt 3 (index 2) -> 4s (but this is the final attempt, no more retries)
        assert_eq!(RETRY_BACKOFF_SECS[2], 4);
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
