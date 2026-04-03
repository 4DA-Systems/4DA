//! Item processing logic: fill_cache_background, process_source_items,
//! embedding generation, deduplication, validation.

use tauri::AppHandle;
use tracing::{debug, info, warn};

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
    build_embedding_text, embed_texts, get_database, sources, void_signal_cache_filled,
    void_signal_fetch_progress, void_signal_fetching, GenericSourceItem,
};

use super::{
    fetch_with_retry, load_github_languages_from_settings, load_rss_feeds_from_settings,
    load_twitter_settings, load_youtube_channels_from_settings, AdapterFailureTracker,
};

/// Fill the cache with items from all sources (background operation)
/// This is the "write" side of the cache-first architecture
/// Does NOT emit progress events - runs silently in background
pub(crate) async fn fill_cache_background(app: &AppHandle) -> Result<usize> {
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

    // Fetch from all sources in parallel (with per-source rate limiting + retry)
    let rl = rate_limiter();
    let cache_tracker = AdapterFailureTracker::new();
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
            fetch_with_retry("Hacker News", &cache_tracker, || {
                hn_source.fetch_items_deep(50)
            })
            .await
        },
        async {
            rl.wait_for_rate_limit("arxiv").await;
            fetch_with_retry("arXiv", &cache_tracker, || {
                arxiv_source.fetch_items_deep(50)
            })
            .await
        },
        async {
            rl.wait_for_rate_limit("reddit").await;
            fetch_with_retry("Reddit", &cache_tracker, || {
                reddit_source.fetch_items_deep(50)
            })
            .await
        },
        async {
            rl.wait_for_rate_limit("github").await;
            fetch_with_retry("GitHub", &cache_tracker, || github_source.fetch_items()).await
        },
        async {
            rl.wait_for_rate_limit("rss").await;
            fetch_with_retry("RSS", &cache_tracker, || rss_source.fetch_items()).await
        },
        async {
            rl.wait_for_rate_limit("twitter").await;
            fetch_with_retry("Twitter", &cache_tracker, || {
                twitter_source.fetch_items_deep(50)
            })
            .await
        },
        async {
            rl.wait_for_rate_limit("youtube").await;
            fetch_with_retry("YouTube", &cache_tracker, || youtube_source.fetch_items()).await
        },
        async {
            rl.wait_for_rate_limit("lobsters").await;
            fetch_with_retry("Lobsters", &cache_tracker, || {
                lobsters_source.fetch_items_deep(50)
            })
            .await
        },
        async {
            rl.wait_for_rate_limit("devto").await;
            fetch_with_retry("Dev.to", &cache_tracker, || {
                devto_source.fetch_items_deep(50)
            })
            .await
        },
    );

    // Log persistent failures from cache fill
    for (name, count) in cache_tracker.persistent_failures() {
        warn!(target: "4da::cache", adapter = %name, consecutive_failures = count, "Persistent failure during cache fill");
    }

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
        Err(e) => {
            warn!(target: "4da::cache", source = "hackernews", error = %e, "Fetch failed after retries");
        }
    }
    void_signal_fetch_progress(app, 1, 9);

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
        Err(e) => {
            warn!(target: "4da::cache", source = "arxiv", error = %e, "Fetch failed after retries");
        }
    }
    void_signal_fetch_progress(app, 2, 9);

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
        Err(e) => {
            warn!(target: "4da::cache", source = "reddit", error = %e, "Fetch failed after retries");
        }
    }
    void_signal_fetch_progress(app, 3, 9);

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
        Err(e) => {
            warn!(target: "4da::cache", source = "github", error = %e, "Fetch failed after retries");
        }
    }
    void_signal_fetch_progress(app, 4, 9);

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
        Err(e) => {
            warn!(target: "4da::cache", source = "rss", error = %e, "Fetch failed after retries");
        }
    }
    void_signal_fetch_progress(app, 5, 9);

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
        Err(e) => {
            warn!(target: "4da::cache", source = "twitter", error = %e, "Fetch failed after retries");
        }
    }
    void_signal_fetch_progress(app, 6, 9);

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
        Err(e) => {
            warn!(target: "4da::cache", source = "youtube", error = %e, "Fetch failed after retries");
        }
    }
    void_signal_fetch_progress(app, 7, 9);

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
        Err(e) => {
            warn!(target: "4da::cache", source = "lobsters", error = %e, "Fetch failed after retries");
        }
    }
    void_signal_fetch_progress(app, 8, 9);

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
        Err(e) => {
            warn!(target: "4da::cache", source = "devto", error = %e, "Fetch failed after retries");
        }
    }
    void_signal_fetch_progress(app, 9, 9);

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

        // Detect language from title text (before embedding)
        let new_items_to_embed: Vec<_> = new_items_to_embed
            .into_iter()
            .map(|(st, sid, url, title, content)| {
                let detected_lang = crate::language_detect::detect_language(&title);
                (st, sid, url, title, content, detected_lang)
            })
            .collect();

        // Pre-translate titles for non-English users (warms translation cache)
        let user_lang = crate::i18n::get_user_language();
        if user_lang != "en" {
            let translation_requests: Vec<crate::content_translation::TranslationRequest> =
                new_items_to_embed
                    .iter()
                    .filter(|(_, _, _, _, _, detected)| detected != &user_lang)
                    .map(|(_, sid, _, title, _, _)| {
                        crate::content_translation::TranslationRequest {
                            id: sid.clone(),
                            text: title.clone(),
                            source_lang: "en".to_string(),
                        }
                    })
                    .collect();

            if !translation_requests.is_empty() {
                let total_chars: usize =
                    translation_requests.iter().map(|r| r.text.len()).sum();
                if crate::content_translation::check_ingest_budget(total_chars) {
                    debug!(target: "4da::cache", count = translation_requests.len(), lang = %user_lang, "Pre-translating titles at ingest");
                    let results = crate::content_translation::translate_content_batch(
                        &translation_requests,
                        &user_lang,
                    )
                    .await;
                    let translated =
                        results.iter().filter(|r| r.provider != "none").count();
                    info!(target: "4da::cache", translated, total = translation_requests.len(), lang = %user_lang, "Ingest translation complete");
                } else {
                    debug!(target: "4da::cache", "Ingest translation budget exhausted - skipping until tomorrow");
                }
            }
        }

        let texts: Vec<String> = new_items_to_embed
            .iter()
            .map(|(_, _, _, title, content, _)| build_embedding_text(title, content))
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
                    String,
                )> = new_items_to_embed
                    .into_iter()
                    .zip(embeddings.into_iter())
                    .filter(|(_, embedding)| !embedding.iter().all(|&v| v == 0.0))
                    .map(
                        |((source_type, source_id, url, title, content, detected_lang), embedding)| {
                            (source_type, source_id, url, title, content, embedding, detected_lang)
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    /// Helper: replicate the ID hashing logic used in fetch_all_sources and process_source_items
    fn hash_source_id(source_type: &str, source_id: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        format!("{}:{}", source_type, source_id).hash(&mut hasher);
        hasher.finish()
    }

    // ---------- Test 1: ID hashing is deterministic and collision-resistant ----------

    #[test]
    fn test_source_id_hashing_deterministic_and_distinct() {
        // Same inputs must produce the same hash (deterministic)
        let id_a = hash_source_id("hackernews", "12345");
        let id_b = hash_source_id("hackernews", "12345");
        assert_eq!(
            id_a, id_b,
            "Same source_type + source_id should yield same hash"
        );

        // Different source_id should produce different hash
        let id_c = hash_source_id("hackernews", "99999");
        assert_ne!(
            id_a, id_c,
            "Different source_ids should produce different hashes"
        );

        // Different source_type with same source_id should produce different hash
        let id_d = hash_source_id("reddit", "12345");
        assert_ne!(
            id_a, id_d,
            "Different source_types should produce different hashes"
        );
    }

    // ---------- Test 2: process_source_items routes uncached items to embed list ----------

    #[test]
    fn test_process_source_items_new_items_go_to_embed_list() {
        let db = test_db();
        let mut all_items: Vec<(GenericSourceItem, Vec<f32>)> = Vec::new();
        let mut new_items_to_embed: Vec<(GenericSourceItem, String)> = Vec::new();

        let items = vec![
            sources::SourceItem::new("hackernews", "hn_001", "Rust is great")
                .with_url(Some("https://example.com/rust".to_string()))
                .with_content("Rust offers memory safety without a GC.".to_string()),
            sources::SourceItem::new("hackernews", "hn_002", "TypeScript 6.0 Released")
                .with_content("Major new features in TS 6.".to_string()),
        ];

        process_source_items(
            &db,
            &mut all_items,
            &mut new_items_to_embed,
            items,
            "hackernews",
        );

        // Nothing in DB, so all items should be in the embed list
        assert_eq!(
            all_items.len(),
            0,
            "No cached items should appear in all_items"
        );
        assert_eq!(
            new_items_to_embed.len(),
            2,
            "Both items should need embedding"
        );

        // Verify GenericSourceItem fields
        let (ref item, ref embed_text) = new_items_to_embed[0];
        assert_eq!(item.source_type, "hackernews");
        assert_eq!(item.source_id, "hn_001");
        assert_eq!(item.title, "Rust is great");
        assert_eq!(item.url, Some("https://example.com/rust".to_string()));
        assert_eq!(item.content, "Rust offers memory safety without a GC.");

        // Embedding text should contain both title and content
        assert!(
            embed_text.contains("Rust is great"),
            "Embed text should contain the title"
        );
        assert!(
            embed_text.contains("memory safety"),
            "Embed text should contain the content"
        );
    }

    // ---------- Test 3: process_source_items assigns correct source_type ----------

    #[test]
    fn test_process_source_items_tags_with_source_type() {
        let db = test_db();
        let mut all_items: Vec<(GenericSourceItem, Vec<f32>)> = Vec::new();
        let mut embed_list: Vec<(GenericSourceItem, String)> = Vec::new();

        let reddit_items = vec![sources::SourceItem::new(
            "reddit",
            "t3_abc",
            "Show Reddit: My CLI tool",
        )];
        let arxiv_items = vec![sources::SourceItem::new(
            "arxiv",
            "2401.00001",
            "Attention Is Still All You Need",
        )];

        process_source_items(&db, &mut all_items, &mut embed_list, reddit_items, "reddit");
        process_source_items(&db, &mut all_items, &mut embed_list, arxiv_items, "arxiv");

        assert_eq!(embed_list.len(), 2);
        assert_eq!(embed_list[0].0.source_type, "reddit");
        assert_eq!(embed_list[1].0.source_type, "arxiv");

        // IDs should differ because source_type differs
        assert_ne!(embed_list[0].0.id, embed_list[1].0.id);
    }

    // ---------- Test 4: Fallback zero-vector detection ----------

    #[test]
    fn test_fallback_zero_vector_detection() {
        // This tests the pattern used to detect fallback embeddings
        let real_embedding = [0.1f32, -0.5, 0.3, 0.0, 0.8];
        let zero_embedding = vec![0.0f32; 384];
        let empty_embedding: Vec<f32> = vec![];

        let is_fallback_real = real_embedding.iter().all(|&v| v == 0.0);
        let is_fallback_zero = zero_embedding.iter().all(|&v| v == 0.0);
        let is_fallback_empty = empty_embedding.iter().all(|&v| v == 0.0);

        assert!(
            !is_fallback_real,
            "Real embedding should not be detected as fallback"
        );
        assert!(
            is_fallback_zero,
            "All-zeros vector should be detected as fallback"
        );
        assert!(
            is_fallback_empty,
            "Empty vector should satisfy all() vacuously (edge case)"
        );
    }

    // ---------- Test 5: Retry backoff constants from fetch_with_retry ----------

    #[test]
    fn test_retry_backoff_delays() {
        use crate::source_fetching::{MAX_RETRY_ATTEMPTS, RETRY_BACKOFF_SECS};

        // fetch_with_retry uses exponential backoff: 1s, 2s, 4s
        assert_eq!(RETRY_BACKOFF_SECS[0], 1, "First retry: 1s");
        assert_eq!(RETRY_BACKOFF_SECS[1], 2, "Second retry: 2s");
        assert_eq!(RETRY_BACKOFF_SECS[2], 4, "Third retry: 4s");
        assert_eq!(MAX_RETRY_ATTEMPTS, 3, "Maximum 3 attempts");

        // Beyond array bounds should fallback to 4
        assert_eq!(
            RETRY_BACKOFF_SECS.get(3).copied().unwrap_or(4),
            4,
            "Out-of-bounds: fallback 4s"
        );
    }
}
