// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Item processing logic: fill_cache_background, process_source_items,
//! embedding generation, deduplication, validation.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use futures::FutureExt;

use tauri::{AppHandle, Emitter};
use tokio::task::JoinSet;
use tracing::{debug, info, warn};

use crate::db::Database;
use crate::error::Result;
use crate::sources::rate_limiter::rate_limiter;
use crate::{
    build_embedding_text, embed_texts, get_database, sources, void_signal_cache_filled,
    void_signal_fetch_progress, void_signal_fetching, GenericSourceItem,
};

use super::{fetch_with_retry, AdapterFailureTracker};

type FetchResult = std::result::Result<
    (String, String, Vec<crate::sources::SourceItem>),
    (String, super::RetryExhaustedError),
>;

/// A newly fetched item awaiting embed + persist:
/// `(source_type, source_id, url, title, content, feed_origin, published_at, tags)`.
/// `tags` is the serialized tags-column object (topics + engagement keys,
/// `extract_source_tags`) — computed at fetch time while the adapter's
/// metadata is still in hand.
type RawNewItem = (
    String,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// A prepared item (HTML entities decoded, language detected):
/// `(source_type, source_id, url, title, content, detected_lang, feed_origin, published_at, tags)`.
type PreparedItem = (
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Row shape for [`Database::batch_upsert_source_items`].
type InsertRow = (
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
    Option<String>,
    Option<String>,
);

/// Row shape for [`Database::batch_upsert_pending_source_items`]:
/// `(source_type, source_id, url, title, content, embed_text)`.
type PendingRow = (String, String, Option<String>, String, String, String);

/// Outcome counters for one source's ingestion, folded into the cycle totals:
/// items fully upserted, items parked in the `embedding_status = 'pending'`
/// retry queue, and DB batch writes that FAILED (logged and counted, never
/// `.ok()`-swallowed).
#[derive(Debug, Default, PartialEq, Eq)]
struct IngestCounts {
    new_items: usize,
    pending_items: usize,
    db_errors: usize,
}

fn fetched_recently(db: &Database, source_type: &str, cooldown_secs: i64) -> bool {
    let Ok(Some(last_fetch_str)) = db.get_source_last_fetch(source_type) else {
        return false;
    };
    let Ok(last_fetch) =
        chrono::NaiveDateTime::parse_from_str(&last_fetch_str, "%Y-%m-%d %H:%M:%S")
    else {
        return false;
    };
    let elapsed = chrono::Utc::now().naive_utc() - last_fetch;
    elapsed.num_seconds() < cooldown_secs
}

/// Fill the cache with items from all sources (background operation).
/// Sources are fetched in parallel, bounded by the rate limiter's 6-permit
/// semaphore. Results stream in as they complete — fast sources don't wait
/// behind slow ones.
///
/// Ingestion is per-source incremental (2026-08-23 scoring audit): each
/// source's new items are embedded and persisted as soon as that source's
/// fetch completes. Before this, the whole cycle accumulated into ONE embed
/// batch and ONE `.ok()`-swallowed upsert at the end, so a single embed
/// failure, one DB error, or an engine death mid-cycle discarded every
/// source's new items — permanently, because source windows scroll forward.
/// Anything that cannot be fully ingested immediately is parked in the
/// `embedding_status = 'pending'` retry queue instead of being dropped.
pub(crate) async fn fill_cache_background(app: &AppHandle) -> Result<super::FetchSummary> {
    info!(target: "4da::cache", "=== BACKGROUND CACHE FILL STARTED (parallel) ===");
    void_signal_fetching(app);

    let db = get_database()?;
    let mut summary = super::FetchSummary::default();
    let mut pending_items_total = 0usize;
    let mut db_errors_total = 0usize;

    let all_sources = crate::sources::build_all_sources();
    let source_count = all_sources.len();
    let cache_tracker = AdapterFailureTracker::new();

    // Track how many sources have been skipped up front. This covers disabled
    // sources, open source-level circuits, and very recent successful fetches.
    let mut enabled_sources = Vec::new();
    for source in all_sources {
        let st = source.source_type();
        if !db.is_source_enabled(st) {
            summary.skipped_disabled += 1;
        } else if db.is_circuit_open(st) {
            info!(target: "4da::cache", source = st, "Skipping source with open circuit breaker");
            let _ = app.emit(
                "source-circuit-break",
                serde_json::json!({
                    "source": st,
                    "status": "open",
                    "message": "Temporarily disabled after repeated failures",
                }),
            );
            summary.skipped_disabled += 1;
        } else if fetched_recently(db, st, 300) {
            info!(target: "4da::cache", source = st, "Skipping source fetched recently");
            summary.skipped_disabled += 1;
        } else {
            enabled_sources.push(source);
        }
    }

    let enabled_count = enabled_sources.len();
    let completed = Arc::new(AtomicUsize::new(summary.skipped_disabled));

    // Language / translation settings are cycle-constant; resolve once.
    let user_lang = crate::i18n::get_user_language();
    let auto_translate = crate::get_settings_manager()
        .lock()
        .get()
        .translation
        .auto_translate;

    // Adaptive yield throttle (see yield_throttle): low-yield sources get a smaller
    // fetch budget so we stop pulling+embedding a firehose of noise. Capping the
    // fetch_items_deep count here saves the fetch AND the embed for throttled sources.
    let source_yields = db
        .get_source_relevance_yields(30, super::RELEVANCE_FLOOR_PUB)
        .unwrap_or_default();
    const CACHE_FILL_BASE: usize = 50;

    // Spawn all enabled sources as concurrent tasks
    let mut join_set: JoinSet<FetchResult> = JoinSet::new();

    for source in enabled_sources {
        let tracker = cache_tracker.clone();
        let cap = super::fetch_cap(
            CACHE_FILL_BASE,
            source_yields
                .get(source.source_type())
                .map(|&(scored, hit_rate)| super::SourceYield { scored, hit_rate })
                .as_ref(),
            source.source_type(),
        );
        join_set.spawn(async move {
            let st = source.source_type().to_string();
            let name = source.name().to_string();

            rate_limiter().wait_for_rate_limit(&st).await;

            let result = fetch_with_retry(&name, &tracker, || source.fetch_items_deep(cap)).await;

            match result {
                Ok(raw_items) => {
                    let manifest = source.manifest();
                    let items = sources::apply_source_quality_gate(raw_items, &manifest);
                    Ok((st, name, items))
                }
                Err(e) => Err((st, e)),
            }
        });
    }

    info!(
        target: "4da::cache",
        enabled = enabled_count,
        disabled = summary.skipped_disabled,
        total = source_count,
        "Spawned parallel fetch for {enabled_count} sources"
    );

    // Collect results as they complete. Each source's batch is embedded and
    // persisted HERE, so already-ingested sources survive whatever happens later.
    while let Some(join_result) = join_set.join_next().await {
        let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
        void_signal_fetch_progress(app, done, source_count);

        let fetch_result = match join_result {
            Ok(r) => r,
            Err(join_err) => {
                warn!(target: "4da::cache", error = %join_err, "Fetch task panicked");
                summary.failed += 1;
                continue;
            }
        };

        match fetch_result {
            Ok((st, name, items)) => {
                let filtered = items.len();
                info!(target: "4da::cache", source = %st, fetched = filtered, "Fetched {name} items (quality-gated)");
                summary.succeeded += 1;

                db.record_source_health(&st, true, filtered as i64, 0, None)
                    .ok();
                // I-5: stamp sources.last_fetch on the ACTIVE ingestion path. The legacy
                // source_fetching/fetcher.rs path stamps it, but this parallel processor (the
                // path that actually runs) did not — so sources.last_fetch went 25d+ stale while
                // items kept arriving. The fetch-interval gate (fetcher.rs:133) and any "last
                // updated" UI read this column, so a stale value misreports freshness.
                db.update_source_fetch_time(&st).ok();

                // Record per-feed health so DataFreshness.source_checks_last_24h updates
                let mut feed_origins_seen = std::collections::HashSet::new();
                for item in &items {
                    if let Some(origin) = super::extract_feed_origin(item) {
                        feed_origins_seen.insert(origin);
                    }
                }
                if feed_origins_seen.is_empty() && !items.is_empty() {
                    // Source doesn't emit per-feed metadata — record at source level
                    db.record_feed_success(&st, &st).ok();
                } else {
                    for origin in &feed_origins_seen {
                        db.record_feed_success(origin, &st).ok();
                    }
                }

                let mut source_new_items: Vec<RawNewItem> = Vec::new();
                for item in items {
                    // `source_item_exists`, NOT `get_source_item`: the getter filters
                    // out `embedding_status = 'pending'` rows, so a pending item still
                    // inside the source window would look "new" and be re-embedded
                    // every cycle (and its enriched content clobbered). The repair
                    // loop owns re-embedding; the fetch path only refreshes last_seen.
                    if db.source_item_exists(&st, &item.source_id).unwrap_or(false) {
                        db.touch_source_item(&st, &item.source_id).ok();
                        // Re-see = the only moment engagement counts refresh
                        // (favourites/score/likes grow after first ingest).
                        if let Some(tags) = super::extract_source_tags(&item) {
                            if let Err(e) = db.update_source_item_tags(&st, &item.source_id, &tags)
                            {
                                debug!(target: "4da::cache", source = %st, error = %e, "Tags refresh failed on re-seen item");
                            }
                        }
                        summary.cached_touches += 1;
                    } else {
                        let feed_origin = super::extract_feed_origin(&item);
                        let published_at = super::extract_published_at(&item);
                        // Tags (topics + engagement keys) come from adapter
                        // metadata, which is dropped at the DB boundary —
                        // serialize them NOW or the community-signal reader
                        // never sees engagement (2026-08-23 audit: the tags
                        // column was NULL across the entire corpus).
                        let tags = super::extract_source_tags(&item);
                        source_new_items.push((
                            st.to_string(),
                            item.source_id,
                            item.url,
                            item.title,
                            item.content,
                            feed_origin,
                            published_at,
                            tags,
                        ));
                    }
                }

                if !source_new_items.is_empty() {
                    let counts =
                        ingest_source_batch(db, &st, source_new_items, &user_lang, auto_translate)
                            .await;
                    summary.new_items += counts.new_items;
                    pending_items_total += counts.pending_items;
                    db_errors_total += counts.db_errors;
                }
            }
            Err((st, e)) => {
                warn!(target: "4da::cache", source = %st, error = %e, "Fetch failed after retries");
                summary.failed += 1;
                let err_msg = e.to_string();
                db.record_source_health(&st, false, 0, 0, Some(&err_msg))
                    .ok();
                // Record per-feed failure so circuit breaker and stale detection work
                db.record_feed_failure(&st, &st, &err_msg).ok();
            }
        }
    }

    for (name, count) in cache_tracker.persistent_failures() {
        warn!(target: "4da::cache", adapter = %name, consecutive_failures = count, "Persistent failure during cache fill");
    }

    // Link newly ingested items to known dependencies
    if let Err(e) = crate::dep_linker::link_recent_items(db) {
        warn!(target: "4da::dep_linker", "Failed to link source items to deps: {e}");
    }

    void_signal_cache_filled(app);

    // Sources just fetched over the network, so the network works: clear any
    // stale SourceFetching degradation. The sleep/wake detector reports
    // degraded ("Network state uncertain after sleep/wake") but only the
    // deep-scan path (`fetcher::fetch_all_sources`) ever restored — this
    // everyday cache-fill path never did, so the flag stuck at degraded for
    // days while every source fetched healthily (live 2026-07-21 audit).
    if summary.succeeded > 0 {
        crate::capabilities::report_restored(crate::capabilities::Capability::SourceFetching);
    }

    info!(
        target: "4da::cache",
        succeeded = summary.succeeded,
        failed = summary.failed,
        new_items = summary.new_items,
        pending_items = pending_items_total,
        db_errors = db_errors_total,
        "=== BACKGROUND CACHE FILL COMPLETE ==="
    );
    Ok(summary)
}

// ============================================================================
// Per-source incremental ingestion
// ============================================================================

/// Embed and persist ONE source's new items. Never drops the batch:
/// - embed call fails        -> whole batch parked as embedding-pending
/// - one item embeds to zero -> that item parked as embedding-pending
///   (osv/cve keep their zero vector: version-grounded, not similarity-grounded)
/// - DB batch upsert fails   -> logged + counted, batch re-queued as pending
///
/// Pending rows are re-embedded by the repair loop (`get_pending_embedding_items`
/// / `upgrade_pending_to_complete`) on later background cycles.
async fn ingest_source_batch(
    db: &Database,
    source_type: &str,
    raw_items: Vec<RawNewItem>,
    user_lang: &str,
    auto_translate: bool,
) -> IngestCounts {
    let mut counts = IngestCounts::default();

    let prepared = prepare_source_batch(source_type, raw_items, user_lang, auto_translate);
    if prepared.is_empty() {
        return counts;
    }

    spawn_title_translation_warmup(&prepared, user_lang);

    debug!(target: "4da::cache", source = %source_type, count = prepared.len(), "Embedding new items for source");

    let texts: Vec<String> = prepared.iter().map(|(_, text)| text.clone()).collect();
    match embed_texts(&texts).await {
        Ok(embeddings) => {
            let (insert_rows, pending_rows) = partition_embedded(prepared, embeddings);
            persist_source_batch(db, source_type, insert_rows, pending_rows, &mut counts);
        }
        Err(e) => {
            // Source windows scroll forward: an item dropped here may never
            // be fetched again. Park the batch for the repair loop instead.
            warn!(
                target: "4da::cache",
                source = %source_type,
                items = prepared.len(),
                error = %e,
                "Embedding failed - storing source batch as embedding-pending for retry"
            );
            let pending_rows: Vec<PendingRow> = prepared
                .into_iter()
                .map(|((st, sid, url, title, content, _, _, _, _), embed_text)| {
                    (st, sid, url, title, content, embed_text)
                })
                .collect();
            persist_pending_rows(db, source_type, pending_rows, &mut counts);
        }
    }
    counts
}

/// Decode HTML entities, detect language, and apply the foreign-language filter
/// to one source's raw items; pairs each retained item with its embed text.
fn prepare_source_batch(
    source_type: &str,
    raw_items: Vec<RawNewItem>,
    user_lang: &str,
    auto_translate: bool,
) -> Vec<(PreparedItem, String)> {
    let before_filter = raw_items.len();
    let prepared: Vec<(PreparedItem, String)> = raw_items
        .into_iter()
        .map(|(st, sid, url, title, content, feed_origin, published_at, tags)| {
            // Decode HTML entities at ingestion time; detect language from the
            // decoded title text (before embedding).
            let title = crate::decode_html_entities(&title);
            let content = crate::decode_html_entities(&content);
            let detected_lang =
                crate::language_detect::detect_language_with_content(&title, &content);
            (
                st,
                sid,
                url,
                title,
                content,
                detected_lang,
                feed_origin,
                published_at,
                tags,
            )
        })
        // Drop foreign-language items that won't be translated into the user's
        // language. Two signals are combined: the detected language, and a
        // script-ratio check that catches predominantly non-Latin titles the
        // detector misclassifies as English (e.g. a mostly-CJK title with a few
        // ASCII tokens). Non-English users have foreign titles translated, so
        // those are retained for them.
        .filter(|(st, _, _, title, _, detected, _, _, _)| {
            // Security advisories (OSV/CVE) are version-matched to a pinned dependency — they
            // are relevant regardless of the advisory text's DETECTED language (the title
            // carries an "[id] pkg:" prefix that skews short-title detection, so an English
            // advisory like "Next.js Cache Poisoning" can be misclassified and wrongly dropped,
            // silently losing a real exposure). Never language-filter a security source.
            if st == "osv" || st == "cve" {
                return true;
            }
            let foreign_by_detect = detected != user_lang;
            let foreign_by_script =
                user_lang == "en" && crate::language_detect::is_predominantly_non_latin(title);
            // Non-English users get foreign-detected titles translated.
            let will_translate = auto_translate && user_lang != "en" && foreign_by_detect;
            let keep = (!foreign_by_detect && !foreign_by_script) || will_translate;
            if !keep {
                debug!(target: "4da::ingest", source = %st, lang = %detected, "Filtered foreign-language item at ingestion");
            }
            keep
        })
        .map(|item| {
            let (st, _, _, title, content, ..) = &item;
            let compressed = crate::compression_rules::compress(st, content);
            let embed_text = build_embedding_text(title, &compressed);
            (item, embed_text)
        })
        .collect();

    let filtered_out = before_filter - prepared.len();
    if filtered_out > 0 {
        info!(target: "4da::ingest", source = %source_type, filtered_out, user_lang = %user_lang, "Dropped foreign-language items at ingestion");
    }
    prepared
}

/// Warm the translation cache for foreign-detected titles (non-English users).
/// Non-blocking: content displays immediately; the next view hits a warm cache.
fn spawn_title_translation_warmup(prepared: &[(PreparedItem, String)], user_lang: &str) {
    if user_lang == "en" {
        return;
    }
    let translation_requests: Vec<crate::content_translation::TranslationRequest> = prepared
        .iter()
        .filter(|((_, _, _, _, _, detected, _, _, _), _)| detected != user_lang)
        .map(|((_, sid, _, title, _, _, _, _, _), _)| {
            crate::content_translation::TranslationRequest {
                id: sid.clone(),
                text: title.clone(),
                source_lang: "en".to_string(),
            }
        })
        .collect();

    if translation_requests.is_empty() {
        return;
    }
    let total_chars: usize = translation_requests.iter().map(|r| r.text.len()).sum();
    if !crate::content_translation::check_ingest_budget(total_chars) {
        debug!(target: "4da::cache", "Ingest translation budget exhausted - skipping until tomorrow");
        return;
    }
    let count = translation_requests.len();
    let lang = user_lang.to_string();
    debug!(target: "4da::cache", count, lang = %lang, "Spawning background title translation");
    tokio::spawn(async move {
        let result = std::panic::AssertUnwindSafe(
            crate::content_translation::translate_content_batch(&translation_requests, &lang),
        )
        .catch_unwind()
        .await;
        match result {
            Ok(results) => {
                let translated = results.iter().filter(|r| r.provider != "none").count();
                info!(target: "4da::cache", translated, total = count, lang = %lang, "Background ingest translation complete");
            }
            Err(_) => {
                warn!(target: "4da::cache", "Background translation panicked - caught and ignored");
            }
        }
    });
}

/// Split embedded items into insertable rows and embedding-pending rows.
/// Items whose embedding failed (all-zero vector) are NOT dropped: they become
/// pending rows so the repair loop re-embeds them on a later cycle. The
/// exception is manifest-grounded security advisories (OSV/CVE): their
/// relevance is the version-match to a pinned dependency, NOT embedding
/// similarity, so deferring one for a failed embedding would silently lose a
/// real exposure. A zero vector is inert in cosine similarity — insert it.
fn partition_embedded(
    prepared: Vec<(PreparedItem, String)>,
    embeddings: Vec<Vec<f32>>,
) -> (Vec<InsertRow>, Vec<PendingRow>) {
    let mut insert_rows: Vec<InsertRow> = Vec::new();
    let mut pending_rows: Vec<PendingRow> = Vec::new();
    let mut shortfall = 0usize;

    let mut embeddings = embeddings.into_iter();
    for (item, embed_text) in prepared {
        let (
            source_type,
            source_id,
            url,
            title,
            content,
            detected_lang,
            feed_origin,
            published_at,
            tags,
        ) = item;
        let Some(embedding) = embeddings.next() else {
            // Embedder returned fewer vectors than texts: park the tail, never truncate.
            shortfall += 1;
            pending_rows.push((source_type, source_id, url, title, content, embed_text));
            continue;
        };

        let is_zero = embedding.iter().all(|&v| v == 0.0);
        let security = source_type == "osv" || source_type == "cve";
        if is_zero && !security {
            pending_rows.push((source_type, source_id, url, title, content, embed_text));
            continue;
        }
        if is_zero {
            debug!(target: "4da::ingest", source = %source_type, id = %source_id, "Retaining zero-embedding security advisory (version-grounded)");
        }

        let content_type =
            crate::entity_extraction::classify_for_storage(&title, &content, &source_type);
        let cve_ids = crate::entity_extraction::extract_cve_ids(&title, &content);
        insert_rows.push((
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
            tags,
            published_at,
        ));
    }

    if shortfall > 0 {
        warn!(target: "4da::ingest", shortfall, "Embedding batch returned fewer vectors than texts - overflow items parked as pending");
    }
    (insert_rows, pending_rows)
}

/// Persist one source's batch. Every DB error is logged with counts and
/// surfaced in `IngestCounts::db_errors` — never `.ok()`-swallowed — and a
/// failed main upsert re-queues its rows as embedding-pending so the items
/// survive to the next cycle. (The vec-table write path has failed before while
/// the plain pending path kept working; see `upgrade_pending_to_complete`.)
fn persist_source_batch(
    db: &Database,
    source_type: &str,
    insert_rows: Vec<InsertRow>,
    mut pending_rows: Vec<PendingRow>,
    counts: &mut IngestCounts,
) {
    if !insert_rows.is_empty() {
        match db.batch_upsert_source_items(&insert_rows) {
            Ok(upserted) => {
                counts.new_items += upserted;
            }
            Err(e) => {
                counts.db_errors += 1;
                warn!(
                    target: "4da::cache",
                    source = %source_type,
                    items = insert_rows.len(),
                    error = %e,
                    "Batch upsert failed - re-queueing this source's items as embedding-pending"
                );
                pending_rows.extend(insert_rows.into_iter().map(
                    |(st, sid, url, title, content, _, _, _, _, _, _, _)| {
                        // Rebuild the embed text exactly as the embed attempt built it.
                        let compressed = crate::compression_rules::compress(&st, &content);
                        let embed_text = build_embedding_text(&title, &compressed);
                        (st, sid, url, title, content, embed_text)
                    },
                ));
            }
        }
    }
    persist_pending_rows(db, source_type, pending_rows, counts);
}

/// Store rows in the `embedding_status = 'pending'` retry queue, counting
/// (never swallowing) failures. Items that cannot be stored even here are
/// genuinely lost once the source window scrolls — the log says so.
fn persist_pending_rows(
    db: &Database,
    source_type: &str,
    pending_rows: Vec<PendingRow>,
    counts: &mut IngestCounts,
) {
    if pending_rows.is_empty() {
        return;
    }
    match db.batch_upsert_pending_source_items(&pending_rows) {
        Ok(stored) => {
            counts.pending_items += stored;
            info!(target: "4da::cache", source = %source_type, count = stored, "Stored items as embedding-pending for retry");
        }
        Err(e) => {
            counts.db_errors += 1;
            warn!(
                target: "4da::cache",
                source = %source_type,
                items = pending_rows.len(),
                error = %e,
                "Failed to store pending items - this source's batch is lost if its window scrolls"
            );
        }
    }
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
                    tags: cached.tags,
                    published_at: cached
                        .published_at
                        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()),
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
                tags: super::extract_source_tags(&item),
                published_at: super::extract_published_at(&item),
            };

            let compressed = crate::compression_rules::compress(source_type, &item.content);
            let embed_text = build_embedding_text(&item.title, &compressed);
            new_items_to_embed.push((generic, embed_text));
        }
    }
}

#[cfg(test)]
#[path = "processor_tests.rs"]
mod tests;
