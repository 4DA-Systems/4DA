//! Analysis functions extracted from lib.rs
//!
//! Contains: run_deep_initial_scan, run_cached_analysis, get_analysis_status,
//! get_actionable_signals, and their implementation helpers.

use once_cell::sync::Lazy;
use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};

use futures::FutureExt;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::Ordering;

use crate::scoring;
use crate::{
    analysis_rerank, emit_progress, get_analysis_abort, get_analysis_state, get_database,
    get_settings_manager, monitoring, signals, source_fetching, truncate_utf8,
    void_signal_analysis_complete, void_signal_error, AnalysisState, SourceRelevance,
    ANALYSIS_TIMEOUT_SECS,
};

// Singleton SignalClassifier - created once and reused across all analyses
static SIGNAL_CLASSIFIER: Lazy<signals::SignalClassifier> =
    Lazy::new(signals::SignalClassifier::new);

/// Get a reference to the singleton SignalClassifier (used by analysis_rerank)
pub(crate) fn signal_classifier() -> &'static signals::SignalClassifier {
    &SIGNAL_CLASSIFIER
}

/// Check if analysis has been aborted by the user
#[inline]
fn is_aborted() -> bool {
    get_analysis_abort().load(Ordering::SeqCst)
}

/// Deep initial scan - comprehensive first-time scan for new users
/// Fetches 300-500+ items from all sources using multiple endpoints
#[tauri::command]
pub(crate) async fn run_deep_initial_scan(app: AppHandle) -> Result<(), String> {
    // Check if already running
    {
        let state = get_analysis_state();
        let guard = state.lock();
        if guard.running {
            return Err("Analysis already running".to_string());
        }
    }

    // Set running state and reset abort flag
    {
        get_analysis_abort().store(false, Ordering::SeqCst);
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = true;
        guard.completed = false;
        guard.error = None;
        guard.results = None;
        guard.started_at = Some(chrono::Utc::now().timestamp());
    }

    info!(target: "4da::analysis", "=== DEEP INITIAL SCAN STARTING ===");
    info!(target: "4da::analysis", "This comprehensive scan will fetch 300-500+ items from multiple sources");

    // Spawn background task with panic recovery
    tokio::spawn(async move {
        let result = AssertUnwindSafe(run_deep_initial_scan_impl(&app))
            .catch_unwind()
            .await;

        // Update state with result — ALWAYS runs, even after panic
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = false;

        match result {
            Ok(Ok(results)) => {
                guard.completed = true;
                guard.results = Some(results.clone());
                guard.last_completed_at =
                    Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
                drop(guard);

                // Use original results for downstream operations
                let _ = app.emit("analysis-complete", &results);
                analysis_rerank::maybe_save_digest(&results);

                let relevant_count = results.iter().filter(|r| r.relevant).count();
                let top_picks = results.iter().filter(|r| r.top_score >= 0.6).count();
                info!(target: "4da::analysis",
                    "=== DEEP INITIAL SCAN COMPLETE ===\n  Total: {} items\n  Relevant: {}\n  Top Picks (60%+): {}",
                    results.len(), relevant_count, top_picks
                );
                if relevant_count > 0 {
                    monitoring::send_notification(&app, relevant_count, results.len());
                }

                // Run post-analysis innovation hooks (non-blocking)
                analysis_rerank::run_post_analysis_hooks(&results);
            }
            Ok(Err(e)) => {
                error!(target: "4da::analysis", error = %e, "Deep initial scan failed");
                guard.error = Some(e.clone());
                drop(guard);
                let _ = app.emit("analysis-error", &e);
            }
            Err(_panic) => {
                let msg = "Deep scan panicked (internal error)".to_string();
                error!(target: "4da::analysis", "Deep scan task panicked — running flag cleared");
                guard.error = Some(msg.clone());
                drop(guard);
                let _ = app.emit("analysis-error", &msg);
            }
        }
    });

    Ok(())
}

/// Deep initial scan implementation - comprehensive first-time intelligence gathering
pub(crate) async fn run_deep_initial_scan_impl(
    app: &AppHandle,
) -> Result<Vec<SourceRelevance>, String> {
    info!(target: "4da::analysis", "=== DEEP INITIAL SCAN STARTED ===");
    info!(target: "4da::analysis", "Fetching 300-500+ items from HN (5 categories), arXiv (16 categories), Reddit (40+ subreddits)...");

    emit_progress(
        app,
        "init",
        0.0,
        "Starting deep initial scan (this may take a few minutes)...",
        0,
        0,
    );

    let db = get_database()?;

    // Step 1: Check context
    emit_progress(app, "context", 0.02, "Checking context...", 0, 0);
    let cached_context_count = db.context_count().map_err(|e| e.to_string())?;

    if cached_context_count > 0 {
        info!(target: "4da::analysis", context_chunks = cached_context_count, "Context indexed");
    }

    // Step 2: DEEP fetch from all sources (100 items per category = 500-1500 total)
    emit_progress(
        app,
        "fetch",
        0.05,
        "Deep fetching from all sources (may take a few minutes)...",
        0,
        0,
    );
    let all_items = source_fetching::fetch_all_sources_deep(db, app, 100).await?;
    info!(target: "4da::analysis", items = all_items.len(), "Deep fetched items from all sources");

    emit_progress(
        app,
        "fetch",
        0.55,
        &format!("Fetched {} items, now scoring...", all_items.len()),
        all_items.len(),
        all_items.len(),
    );

    // Step 3-4: Load all scoring context (user interests, exclusions, ACE context)
    let scoring_ctx = scoring::build_scoring_context(db).await?;
    let interest_count = scoring_ctx.interest_count;
    let options = scoring::ScoringOptions {
        apply_freshness: false,
        apply_signals: true,
    };

    // Step 5: Score all items through unified pipeline
    emit_progress(
        app,
        "relevance",
        0.60,
        &format!("Scoring {} items...", all_items.len()),
        0,
        all_items.len(),
    );

    let mut results: Vec<SourceRelevance> = Vec::new();
    let total_items = all_items.len();

    for (idx, (item, item_embedding)) in all_items.iter().enumerate() {
        if is_aborted() {
            info!(target: "4da::analysis", scored = idx, "Deep scan aborted by user");
            return Err("Analysis cancelled".to_string());
        }

        if idx % 50 == 0 {
            let progress = 0.60 + (0.35 * (idx as f32 / total_items as f32));
            emit_progress(
                app,
                "relevance",
                progress,
                &format!("Scoring {} of {} items...", idx, total_items),
                idx,
                total_items,
            );
        }

        results.push(scoring::score_item(
            &scoring::ScoringInput {
                id: item.id,
                title: &item.title,
                url: item.url.as_deref(),
                content: &item.content,
                source_type: &item.source_type,
                embedding: item_embedding,
                created_at: None,
            },
            &scoring_ctx,
            db,
            &options,
            Some(&SIGNAL_CLASSIFIER),
        ));
    }

    scoring::sort_results(&mut results);

    emit_progress(
        app,
        "complete",
        1.0,
        &format!(
            "Deep scan complete! {} items, {} relevant, {} top picks",
            results.len(),
            results.iter().filter(|r| r.relevant).count(),
            results.iter().filter(|r| r.top_score >= 0.6).count()
        ),
        results.len(),
        results.len(),
    );

    let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
    let excluded_count = results.iter().filter(|r| r.excluded).count();
    info!(target: "4da::analysis", "=== DEEP INITIAL SCAN COMPLETE ===");
    info!(target: "4da::analysis",
        total = results.len(),
        relevant = relevant_count,
        excluded = excluded_count,
        interests = interest_count,
        "Deep scan summary"
    );

    Ok(results)
}

/// Multi-source analysis implementation
pub(crate) async fn run_multi_source_analysis_impl(
    app: &AppHandle,
) -> Result<Vec<SourceRelevance>, String> {
    info!(target: "4da::analysis", "=== MULTI-SOURCE ANALYSIS STARTED ===");

    emit_progress(
        app,
        "init",
        0.0,
        "Initializing multi-source analysis...",
        0,
        0,
    );

    let db = get_database()?;

    // Step 1: Check context (using sqlite-vec KNN)
    emit_progress(
        app,
        "context",
        0.05,
        "Checking context (KNN enabled)...",
        0,
        0,
    );
    let cached_context_count = db.context_count().map_err(|e| e.to_string())?;

    if cached_context_count > 0 {
        info!(target: "4da::analysis", context_chunks = cached_context_count, "Context indexed (using KNN search)");
    } else {
        warn!(target: "4da::analysis", "No context indexed. Running without context-based scoring");
    }

    // Step 2: Fetch from all sources (50 items per source for comprehensive coverage)
    emit_progress(app, "fetch", 0.1, "Fetching from all sources...", 0, 0);
    let all_items = source_fetching::fetch_all_sources(db, app, 50).await?;
    info!(target: "4da::analysis", items = all_items.len(), "Fetched items from all sources");

    // Step 3-4: Load all scoring context (user interests, exclusions, ACE context)
    emit_progress(
        app,
        "relevance",
        0.7,
        "Loading user context...",
        0,
        all_items.len(),
    );
    let scoring_ctx = scoring::build_scoring_context(db).await?;
    let options = scoring::ScoringOptions {
        apply_freshness: false,
        apply_signals: true,
    };

    // Step 5: Score all items through unified pipeline
    emit_progress(
        app,
        "relevance",
        0.75,
        "Computing relevance...",
        0,
        all_items.len(),
    );
    let mut results: Vec<SourceRelevance> = Vec::new();

    for (idx, (item, item_embedding)) in all_items.iter().enumerate() {
        if is_aborted() {
            info!(target: "4da::analysis", scored = idx, "Multi-source analysis aborted by user");
            return Err("Analysis cancelled".to_string());
        }

        if idx % 10 == 0 {
            let progress = 0.75 + (0.20 * (idx as f32 / all_items.len() as f32));
            emit_progress(
                app,
                "relevance",
                progress,
                &format!(
                    "[{}] {}",
                    &item.source_type,
                    &truncate_utf8(&item.title, 30)
                ),
                idx + 1,
                all_items.len(),
            );
        }

        results.push(scoring::score_item(
            &scoring::ScoringInput {
                id: item.id,
                title: &item.title,
                url: item.url.as_deref(),
                content: &item.content,
                source_type: &item.source_type,
                embedding: item_embedding,
                created_at: None,
            },
            &scoring_ctx,
            db,
            &options,
            Some(&SIGNAL_CLASSIFIER),
        ));
    }

    scoring::sort_results(&mut results);
    scoring::dedup_results(&mut results);
    scoring::topic_dedup_results(&mut results);

    // Serendipity Engine: inject anti-bubble items
    {
        let settings = crate::get_settings_manager().lock();
        let serendipity_config = &settings.get().serendipity;
        if serendipity_config.enabled {
            let candidates = scoring::compute_serendipity_candidates(
                &results,
                serendipity_config.budget_percent,
            );
            if !candidates.is_empty() {
                tracing::info!(target: "4da::analysis", count = candidates.len(), "Injecting serendipity items");
                results.extend(candidates);
                scoring::sort_results(&mut results);
            }
        }
    }

    // LLM Reranking (if enabled and within daily limits)
    analysis_rerank::apply_llm_reranking(app, &mut results, &scoring_ctx).await;

    emit_progress(
        app,
        "complete",
        1.0,
        "Multi-source analysis complete!",
        results.len(),
        results.len(),
    );

    let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
    let excluded_count = results.iter().filter(|r| r.excluded).count();
    info!(target: "4da::analysis", "=== MULTI-SOURCE ANALYSIS COMPLETE ===");
    info!(target: "4da::analysis",
        total = results.len(),
        relevant = relevant_count,
        excluded = excluded_count,
        "Analysis summary"
    );

    Ok(results)
}

// ============================================================================
// Cache-First Analysis (Option D)
// ============================================================================

/// Cache-first analysis - analyzes items already in the database
/// This is INSTANT because it doesn't fetch from APIs, just scores cached items
#[tauri::command]
pub(crate) async fn run_cached_analysis(app: AppHandle) -> Result<(), String> {
    // Atomic check-and-set: prevents TOCTOU race from double-clicks
    {
        get_analysis_abort().store(false, Ordering::SeqCst);
        let state = get_analysis_state();
        let mut guard = state.lock();
        if guard.running {
            return Err("Analysis already running".to_string());
        }
        guard.running = true;
        guard.completed = false;
        guard.error = None;
        guard.results = None;
        guard.started_at = Some(chrono::Utc::now().timestamp());
    }

    // Spawn background task with panic recovery
    tokio::spawn(async move {
        let result = AssertUnwindSafe(analyze_cached_content_impl(&app))
            .catch_unwind()
            .await;

        // Update state with result — ALWAYS runs, even after panic
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = false;

        match result {
            Ok(Ok(results)) => {
                guard.completed = true;
                guard.results = Some(results.clone());
                guard.last_completed_at =
                    Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
                drop(guard);

                // Use original results for downstream operations
                let _ = app.emit("analysis-complete", &results);
                analysis_rerank::maybe_save_digest(&results);

                let relevant_count = results.iter().filter(|r| r.relevant).count();
                if relevant_count > 0 {
                    monitoring::send_notification(&app, relevant_count, results.len());
                }

                void_signal_analysis_complete(&app, &results);

                // Run post-analysis innovation hooks (non-blocking)
                analysis_rerank::run_post_analysis_hooks(&results);
            }
            Ok(Err(e)) => {
                guard.error = Some(e.clone());
                drop(guard);
                let _ = app.emit("analysis-error", &e);
                void_signal_error(&app);
            }
            Err(_panic) => {
                let msg = "Analysis panicked (internal error)".to_string();
                error!(target: "4da::analysis", "Analysis task panicked — running flag cleared");
                guard.error = Some(msg.clone());
                drop(guard);
                let _ = app.emit("analysis-error", &msg);
                void_signal_error(&app);
            }
        }
    });

    Ok(())
}

/// The actual cache-first analysis implementation
/// Uses differential analysis when previous results exist (only scores new items)
pub(crate) async fn analyze_cached_content_impl(
    app: &AppHandle,
) -> Result<Vec<SourceRelevance>, String> {
    info!(target: "4da::analysis", "=== CACHE-FIRST ANALYSIS STARTED ===");

    emit_progress(app, "init", 0.0, "Loading cached items...", 0, 0);

    let db = get_database()?;

    // Attempt re-embedding of previously failed items
    match db.get_pending_embedding_items(100) {
        Ok(pending) if !pending.is_empty() => {
            info!(target: "4da::analysis", count = pending.len(), "Attempting re-embedding of pending items");
            emit_progress(
                app,
                "init",
                0.05,
                &format!("Re-embedding {} pending items...", pending.len()),
                0,
                0,
            );
            let texts: Vec<String> = pending.iter().map(|(_, _, _, t)| t.clone()).collect();
            if let Ok(embeddings) = crate::embed_texts(&texts).await {
                let mut upgraded = 0;
                for ((id, _, _, _), embedding) in pending.iter().zip(embeddings.iter()) {
                    let is_fallback = embedding.iter().all(|&v| v == 0.0);
                    if !is_fallback && db.upgrade_pending_to_complete(*id, embedding).is_ok() {
                        upgraded += 1;
                    }
                }
                if upgraded > 0 {
                    info!(target: "4da::analysis", upgraded, total = pending.len(), "Re-embedded previously pending items");
                }
            }
        }
        _ => {} // No pending items or DB error - continue normally
    }

    // Fetch fresh content from all sources before scoring
    // Without this, manual analysis only re-scores stale cached items
    emit_progress(app, "fetch", 0.08, "Fetching fresh content...", 0, 0);
    match crate::source_fetching::fill_cache_background(app).await {
        Ok(count) => {
            if count > 0 {
                info!(target: "4da::analysis", new_items = count, "Fetched fresh content before scoring");
            }
        }
        Err(e) => {
            warn!(target: "4da::analysis", error = %e, "Cache fill failed, continuing with existing cache");
        }
    }

    // Check if we can do differential analysis (have previous results + timestamp)
    let (last_completed_at, previous_results) = {
        let state = get_analysis_state();
        let guard = state.lock();
        (guard.last_completed_at.clone(), guard.results.clone())
    };

    let is_differential = last_completed_at.is_some() && previous_results.is_some();

    if is_differential {
        // Safe: guarded by is_differential check above
        let since = last_completed_at.as_deref().unwrap_or("");
        info!(target: "4da::analysis", since = since, "Differential analysis - checking for new items since last run");

        let new_items = db
            .get_items_since_timestamp(since, 500)
            .map_err(|e| format!("Failed to load new items: {}", e))?;

        if new_items.is_empty() {
            // No new items since last analysis — try re-scoring recent cache (7 days)
            info!(target: "4da::analysis", "No new items since last analysis, re-scoring existing for freshness");
            emit_progress(
                app,
                "cache",
                0.5,
                "No new items, refreshing scores...",
                0,
                0,
            );

            // Re-score existing items for updated freshness/affinities (7-day window)
            let all_items = db
                .get_items_since_hours(168, 1000)
                .map_err(|e| format!("Failed to load cached items: {}", e))?;

            if all_items.is_empty() {
                // Cache is stale — fetch fresh content
                warn!(target: "4da::analysis", "No items in 7-day window, fetching fresh content");
                emit_progress(
                    app,
                    "fetch",
                    0.1,
                    "Cache stale, fetching fresh items...",
                    0,
                    0,
                );
                return run_multi_source_analysis_impl(app).await;
            }

            return analysis_rerank::score_items_full(app, db, &all_items).await;
        }

        info!(target: "4da::analysis", new_items = new_items.len(), "Found new items for differential scoring");
        emit_progress(
            app,
            "cache",
            0.1,
            &format!("Scoring {} new items (differential)...", new_items.len()),
            0,
            new_items.len(),
        );

        // Score only new items
        let scoring_ctx = scoring::build_scoring_context(db).await?;
        let options = scoring::ScoringOptions {
            apply_freshness: true,
            apply_signals: true,
        };

        let mut new_results: Vec<SourceRelevance> = Vec::new();
        let total_new = new_items.len();

        for (idx, item) in new_items.iter().enumerate() {
            if is_aborted() {
                return Err("Analysis cancelled".to_string());
            }

            if idx % 20 == 0 {
                let progress = 0.2 + (0.7 * (idx as f32 / total_new as f32));
                emit_progress(
                    app,
                    "relevance",
                    progress,
                    &format!("Scoring new item {} of {}...", idx + 1, total_new),
                    idx + 1,
                    total_new,
                );
            }

            new_results.push(scoring::score_item(
                &scoring::ScoringInput {
                    id: item.id as u64,
                    title: &item.title,
                    url: item.url.as_deref(),
                    content: &item.content,
                    source_type: &item.source_type,
                    embedding: &item.embedding,
                    created_at: Some(&item.created_at),
                },
                &scoring_ctx,
                db,
                &options,
                Some(&SIGNAL_CLASSIFIER),
            ));
        }

        // LLM Reranking on new items only (if enabled)
        analysis_rerank::apply_llm_reranking(app, &mut new_results, &scoring_ctx).await;

        // Merge: take previous results, update/add new ones by ID
        let mut prev = previous_results.unwrap_or_default();
        let existing_ids: std::collections::HashSet<u64> =
            new_results.iter().map(|r| r.id).collect();
        prev.retain(|r| !existing_ids.contains(&r.id));
        prev.extend(new_results);
        scoring::sort_results(&mut prev);

        let relevant_count = prev.iter().filter(|r| r.relevant && !r.excluded).count();
        info!(target: "4da::analysis",
            "=== DIFFERENTIAL ANALYSIS COMPLETE === total={}, new={}, relevant={}",
            prev.len(), total_new, relevant_count
        );

        emit_progress(
            app,
            "complete",
            1.0,
            &format!(
                "Differential: {} new items scored, {} total",
                total_new,
                prev.len()
            ),
            prev.len(),
            prev.len(),
        );

        return Ok(prev);
    }

    // Full analysis path (no previous results or first run)
    // Use 7-day window to include items from recent fetches
    let cached_items = db
        .get_items_since_hours(168, 1000)
        .map_err(|e| format!("Failed to load cached items: {}", e))?;

    let total_cached = cached_items.len();
    info!(target: "4da::analysis", cached_items = total_cached, "Loaded items from cache");

    if total_cached == 0 {
        warn!(target: "4da::analysis", "Cache empty, falling back to fetch");
        emit_progress(
            app,
            "fetch",
            0.1,
            "Cache empty, fetching fresh items...",
            0,
            0,
        );
        return run_multi_source_analysis_impl(app).await;
    }

    analysis_rerank::score_items_full(app, db, &cached_items).await
}

/// Cancel a running analysis
#[tauri::command]
pub(crate) async fn cancel_analysis() -> Result<(), String> {
    get_analysis_abort().store(true, Ordering::SeqCst);
    info!(target: "4da::analysis", "Analysis cancellation requested");
    Ok(())
}

/// Get current analysis state (with timeout auto-recovery)
#[tauri::command]
pub(crate) async fn get_analysis_status() -> Result<AnalysisState, String> {
    let state = get_analysis_state();
    let mut guard = state.lock();

    // Auto-recover from stuck analysis: if running for too long, force reset
    if guard.running {
        if let Some(started) = guard.started_at {
            let elapsed = chrono::Utc::now().timestamp() - started;
            if elapsed > ANALYSIS_TIMEOUT_SECS {
                warn!(target: "4da::analysis",
                    elapsed_secs = elapsed,
                    timeout = ANALYSIS_TIMEOUT_SECS,
                    "Analysis timed out, auto-resetting state"
                );
                guard.running = false;
                guard.error = Some(format!("Analysis timed out after {}s", elapsed));
                guard.started_at = None;
            }
        }
    }

    Ok(guard.clone())
}
// Settings and Context Engine commands are in settings_commands.rs
// ACE commands, PASIFA helpers, and auto-seeding are in ace_commands.rs
