// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Item processing logic: fill_cache_background, process_source_items,
//! embedding generation, deduplication, validation.

use tauri::AppHandle;
use tracing::{debug, info, warn};

use crate::db::Database;
use crate::error::Result;
use crate::sources::rate_limiter::rate_limiter;
use crate::{
    build_embedding_text, embed_texts, get_database, sources, void_signal_cache_filled,
    void_signal_fetch_progress, void_signal_fetching, GenericSourceItem,
};

use super::{fetch_with_retry, AdapterFailureTracker};

/// Fill the cache with items from all sources (background operation)
/// This is the "write" side of the cache-first architecture
/// Does NOT emit progress events - runs silently in background
pub(crate) async fn fill_cache_background(app: &AppHandle) -> Result<usize> {
    info!(target: "4da::cache", "=== BACKGROUND CACHE FILL STARTED ===");
    void_signal_fetching(app);

    let db = get_database()?;
    let mut total_cached = 0;
    let mut new_items_to_embed: Vec<(String, String, Option<String>, String, String, Option<String>)> = Vec::new();

    // Build ALL sources from the canonical factory (single source of truth)
    let all_sources = crate::sources::build_all_sources();
    let source_count = all_sources.len();
    let rl = rate_limiter();
    let cache_tracker = AdapterFailureTracker::new();

    // Fetch from each source sequentially with rate limiting
    // (previous tokio::join! approach required hardcoded variable names)
    for (idx, source) in all_sources.into_iter().enumerate() {
        let st = source.source_type();
        let name = source.name();

        // Skip disabled sources
        if !db.is_source_enabled(st) {
            continue;
        }

        rl.wait_for_rate_limit(st).await;

        let result = fetch_with_retry(name, &cache_tracker, || source.fetch_items_deep(50)).await;

        match result {
            Ok(raw_items) => {
                // Apply algorithmic quality gate before caching
                let manifest = source.manifest();
                let items = sources::apply_source_quality_gate(raw_items, &manifest);
                let filtered = items.len();
                info!(target: "4da::cache", source = st, fetched = filtered, "Fetched {name} items (quality-gated)");
                for item in items {
                    if db
                        .get_source_item(st, &item.source_id)
                        .ok()
                        .flatten()
                        .is_none()
                    {
                        let feed_origin = super::extract_feed_origin(&item);
                        new_items_to_embed.push((
                            st.to_string(),
                            item.source_id,
                            item.url,
                            item.title,
                            item.content,
                            feed_origin,
                        ));
                    } else {
                        db.touch_source_item(st, &item.source_id).ok();
                        total_cached += 1;
                    }
                }
            }
            Err(e) => {
                warn!(target: "4da::cache", source = st, error = %e, "Fetch failed after retries");
            }
        }

        void_signal_fetch_progress(app, idx + 1, source_count);
    }

    // Log persistent failures from cache fill
    for (name, count) in cache_tracker.persistent_failures() {
        warn!(target: "4da::cache", adapter = %name, consecutive_failures = count, "Persistent failure during cache fill");
    }

    // Embed and cache new items
    if !new_items_to_embed.is_empty() {
        info!(target: "4da::cache", new_items = new_items_to_embed.len(), "Embedding new items");

        // Decode HTML entities at ingestion time
        let new_items_to_embed: Vec<_> = new_items_to_embed
            .into_iter()
            .map(|(st, sid, url, title, content, feed_origin)| {
                (
                    st,
                    sid,
                    url,
                    crate::decode_html_entities(&title),
                    crate::decode_html_entities(&content),
                    feed_origin,
                )
            })
            .collect();

        // Detect language from title text (before embedding)
        let new_items_to_embed: Vec<_> = new_items_to_embed
            .into_iter()
            .map(|(st, sid, url, title, content, feed_origin)| {
                let detected_lang =
                    crate::language_detect::detect_language_with_content(&title, &content);
                (st, sid, url, title, content, detected_lang, feed_origin)
            })
            .collect();

        // Pre-translate titles for non-English users (warms translation cache)
        let user_lang = crate::i18n::get_user_language();
        if user_lang != "en" {
            let translation_requests: Vec<crate::content_translation::TranslationRequest> =
                new_items_to_embed
                    .iter()
                    .filter(|(_, _, _, _, _, detected, _)| detected != &user_lang)
                    .map(|(_, sid, _, title, _, _, _)| {
                        crate::content_translation::TranslationRequest {
                            id: sid.clone(),
                            text: title.clone(),
                            source_lang: "en".to_string(),
                        }
                    })
                    .collect();

            if !translation_requests.is_empty() {
                let total_chars: usize = translation_requests.iter().map(|r| r.text.len()).sum();
                if crate::content_translation::check_ingest_budget(total_chars) {
                    let count = translation_requests.len();
                    let lang = user_lang.clone();
                    debug!(target: "4da::cache", count, lang = %lang, "Spawning background title translation");
                    // Non-blocking: translation warms cache asynchronously.
                    // Content displays immediately; next view hits warm cache.
                    tokio::spawn(async move {
                        let results = crate::content_translation::translate_content_batch(
                            &translation_requests,
                            &lang,
                        )
                        .await;
                        let translated = results.iter().filter(|r| r.provider != "none").count();
                        info!(target: "4da::cache", translated, total = count, lang = %lang, "Background ingest translation complete");
                    });
                } else {
                    debug!(target: "4da::cache", "Ingest translation budget exhausted - skipping until tomorrow");
                }
            }
        }

        let texts: Vec<String> = new_items_to_embed
            .iter()
            .map(|(_, _, _, title, content, _, _)| build_embedding_text(title, content))
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
                    Option<String>,
                    Option<String>,
                    Option<String>,
                )> = new_items_to_embed
                    .into_iter()
                    .zip(embeddings.into_iter())
                    .filter(|(_, embedding)| !embedding.iter().all(|&v| v == 0.0))
                    .map(
                        |(
                            (source_type, source_id, url, title, content, detected_lang, feed_origin),
                            embedding,
                        )| {
                            let content_type = crate::entity_extraction::classify_for_storage(
                                &title,
                                &content,
                                &source_type,
                            );
                            let cve_ids =
                                crate::entity_extraction::extract_cve_ids(&title, &content);
                            (
                                source_type,
                                source_id,
                                url,
                                title,
                                content,
                                embedding,
                                detected_lang,
                                content_type,
                                cve_ids,
                                feed_origin,
                            )
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
                    feed_origin: cached.feed_origin,
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
                feed_origin: super::extract_feed_origin(&item),
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
