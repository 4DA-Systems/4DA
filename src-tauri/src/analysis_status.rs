//! Cache-first analysis, status queries, and cancellation.

use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};

use futures::FutureExt;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::Ordering;

use crate::analysis_narration::{emit_narration, NarrationEvent};
use crate::error::Result;
use crate::scoring;
use crate::stacks;
use crate::{
    analysis_rerank, emit_progress, game_engine, get_analysis_abort, get_analysis_state,
    get_database, monitoring, open_db_connection, void_signal_analysis_complete, void_signal_error,
    AnalysisState, SourceRelevance, ANALYSIS_TIMEOUT_SECS,
};

use super::analysis_deep_scan::run_multi_source_analysis_impl;
use super::{is_aborted, SIGNAL_CLASSIFIER};

// ============================================================================
// Cache-First Analysis (Option D)
// ============================================================================

/// Cache-first analysis - analyzes items already in the database
/// This is INSTANT because it doesn't fetch from APIs, just scores cached items
#[tauri::command]
pub(crate) async fn run_cached_analysis(app: AppHandle) -> Result<()> {
    // Atomic check-and-set: prevents TOCTOU race from double-clicks
    {
        get_analysis_abort().store(false, Ordering::SeqCst);
        let state = get_analysis_state();
        let mut guard = state.lock();
        if guard.running {
            return Err("Analysis already running".into());
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
                // Compute near_misses while we have the guard, then release it
                // so downstream operations can acquire other locks freely.
                let near_misses = crate::types::extract_near_misses(&results);
                guard.completed = true;
                guard.near_misses = near_misses;
                guard.last_completed_at =
                    Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
                drop(guard);

                // Downstream operations all take &[SourceRelevance] — use references
                if let Err(e) = app.emit("analysis-complete", &results) {
                    tracing::warn!("Failed to emit 'analysis-complete': {e}");
                }
                analysis_rerank::maybe_save_digest(&results);

                let relevant_count = results.iter().filter(|r| r.relevant).count();
                if relevant_count > 0 {
                    monitoring::send_notification(&app, relevant_count, results.len());
                }

                void_signal_analysis_complete(&app, &results);

                // Run post-analysis innovation hooks (non-blocking)
                scoring::run_post_analysis_hooks(&results);

                // Record intelligence snapshot for growth tracking
                if let Ok(conn) = open_db_connection() {
                    let total = results.len() as f64;
                    let relevant = relevant_count as f64;
                    let accuracy = if total > 0.0 { relevant / total } else { 0.0 };
                    if let Err(e) = crate::intelligence_history::record_intelligence_snapshot(
                        &conn,
                        accuracy,
                        relevant_count as i64,
                        results.len() as i64,
                        relevant_count as i64,
                    ) {
                        warn!(target: "4da::analysis", error = %e, "Failed to record intelligence snapshot");
                    }
                }

                // GAME: track scan, discoveries, and source diversity
                if let Ok(db) = crate::get_database() {
                    for a in game_engine::increment_counter(db, "scans", 1) {
                        crate::events::emit_achievement_unlocked(&app, &a);
                    }
                    if relevant_count > 0 {
                        for a in
                            game_engine::increment_counter(db, "discoveries", relevant_count as u64)
                        {
                            crate::events::emit_achievement_unlocked(&app, &a);
                        }
                    }
                    let source_types: std::collections::HashSet<&str> =
                        results.iter().map(|r| r.source_type.as_str()).collect();
                    if source_types.len() >= 3 {
                        for a in
                            game_engine::increment_counter(db, "sources", source_types.len() as u64)
                        {
                            crate::events::emit_achievement_unlocked(&app, &a);
                        }
                    }
                }

                // Auto-detect stack profiles if none selected (first analysis)
                if let Ok(db) = open_db_connection() {
                    let selected = stacks::load_selected_stacks(&db);
                    if selected.is_empty() {
                        let ace_ctx = scoring::get_ace_context();
                        let detections = stacks::detection::detect_matching_profiles(&ace_ctx);
                        if !detections.is_empty() {
                            let top_ids: Vec<String> = detections
                                .iter()
                                .filter(|d| d.confidence >= 0.2)
                                .take(3)
                                .map(|d| d.profile_id.clone())
                                .collect();
                            if !top_ids.is_empty() {
                                info!(target: "4da::analysis",
                                    "Auto-selected stack profiles: {:?}",
                                    top_ids
                                );
                                if let Err(e) = stacks::save_selected_stacks(&db, &top_ids) {
                                    tracing::warn!("Failed to save selection: {e}");
                                }
                                if let Err(e) = app.emit("stacks-auto-detected", &top_ids) {
                                    tracing::warn!("Failed to emit 'stacks-auto-detected': {e}");
                                }
                            }
                        }
                    }
                }

                // Move results into shared state — no clone needed.
                // Done last so downstream ops (which hold &results) complete first.
                {
                    let state = get_analysis_state();
                    let mut guard = state.lock();
                    guard.results = Some(results);
                }
            }
            Ok(Err(e)) => {
                let err_str = e.to_string();
                guard.error = Some(err_str.clone());
                drop(guard);
                if let Err(e) = app.emit("analysis-error", &err_str) {
                    tracing::warn!("Failed to emit 'analysis-error': {e}");
                }
                void_signal_error(&app);
            }
            Err(_panic) => {
                let msg = "Analysis panicked (internal error)".to_string();
                error!(target: "4da::analysis", "Analysis task panicked — running flag cleared");
                guard.error = Some(msg.clone());
                drop(guard);
                if let Err(e) = app.emit("analysis-error", &msg) {
                    tracing::warn!("Failed to emit 'analysis-error': {e}");
                }
                void_signal_error(&app);
            }
        }
    });

    Ok(())
}

/// The actual cache-first analysis implementation
/// Uses differential analysis when previous results exist (only scores new items)
pub(crate) async fn analyze_cached_content_impl(app: &AppHandle) -> Result<Vec<SourceRelevance>> {
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
    // Use .take() to move results out of the guard instead of cloning the entire Vec.
    // This is safe: analysis is running, so old results will be replaced when it completes.
    let (last_completed_at, previous_results) = {
        let state = get_analysis_state();
        let mut guard = state.lock();
        (guard.last_completed_at.clone(), guard.results.take())
    };

    let is_differential = last_completed_at.is_some() && previous_results.is_some();

    if is_differential {
        // Safe: guarded by is_differential check above
        let since = last_completed_at.as_deref().unwrap_or("");
        info!(target: "4da::analysis", since = since, "Differential analysis - checking for new items since last run");

        let new_items = db
            .get_items_since_timestamp_tiered(since, 500)
            .map_err(|e| format!("Failed to load new items: {e}"))?;

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
            // Respects free-tier 30-day history gate via get_items_tiered
            let all_items = db
                .get_items_tiered(168, 1000)
                .map_err(|e| format!("Failed to load cached items: {e}"))?;

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

            return scoring::score_items_full(app, db, &all_items).await;
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
        let scoring_ctx = scoring::build_scoring_context(db).await.map_err(|e| {
            format!("Failed to build scoring context for differential analysis: {e}")
        })?;
        let trend_topics = crate::detect_trend_topics(
            new_items
                .iter()
                .map(|item| (item.title.as_str(), item.content.as_str())),
        );
        let options = scoring::ScoringOptions {
            apply_freshness: true,
            apply_signals: true,
            trend_topics,
        };

        let mut new_results: Vec<SourceRelevance> = Vec::new();
        let total_new = new_items.len();

        for (idx, item) in new_items.iter().enumerate() {
            if is_aborted() {
                return Err("Analysis cancelled".into());
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
        emit_narration(
            app,
            NarrationEvent {
                narration_type: "insight".into(),
                message: "Ranking items against your profile...".into(),
                source: None,
                relevance: None,
            },
        );
        analysis_rerank::apply_llm_reranking(app, &mut new_results, &scoring_ctx).await;

        // Merge: take previous results, update/add new ones by ID
        let mut prev = previous_results.unwrap_or_default();
        let existing_ids: std::collections::HashSet<u64> =
            new_results.iter().map(|r| r.id).collect();
        prev.retain(|r| !existing_ids.contains(&r.id));
        prev.extend(new_results);
        scoring::sort_results(&mut prev);

        let relevant_count = prev.iter().filter(|r| r.relevant && !r.excluded).count();
        let excluded_count = prev.iter().filter(|r| r.excluded).count();
        info!(target: "4da::analysis",
            "=== DIFFERENTIAL ANALYSIS COMPLETE === total={}, new={}, relevant={}",
            prev.len(), total_new, relevant_count
        );

        // Record rejection rate for verifiable metrics
        if let Err(e) = db.record_scoring_stats(
            "cached_differential",
            prev.len(),
            relevant_count,
            excluded_count,
        ) {
            tracing::warn!(target: "4da::analysis", error = %e, "Failed to record scoring stats");
        }

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
    // Respects free-tier 30-day history gate via get_items_tiered
    let cached_items = db
        .get_items_tiered(168, 1000)
        .map_err(|e| format!("Failed to load cached items: {e}"))?;

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

    scoring::score_items_full(app, db, &cached_items).await
}

/// Cancel a running analysis
#[tauri::command]
pub(crate) async fn cancel_analysis() -> Result<()> {
    get_analysis_abort().store(true, Ordering::SeqCst);
    info!(target: "4da::analysis", "Analysis cancellation requested");
    Ok(())
}

/// Get current analysis state (with timeout auto-recovery)
///
/// Applies free-tier history gate: non-Signal users only see items from the last 30 days.
/// The gate is enforced here as a defense-in-depth measure on top of the DB-level
/// filter in `get_items_tiered` / `get_items_since_timestamp_tiered`.
#[tauri::command]
pub(crate) async fn get_analysis_status() -> Result<AnalysisState> {
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
                guard.error = Some(format!("Analysis timed out after {elapsed}s"));
                guard.started_at = None;
            }
        }
    }

    let mut result = guard.clone();
    drop(guard);

    // Free-tier history gate: filter out items older than 30 days
    // Uses batch query to avoid N+1 per-item DB lookups
    if !crate::settings::is_signal() {
        if let Some(ref mut results) = result.results {
            let cutoff =
                chrono::Utc::now() - chrono::Duration::hours(crate::db::FREE_HISTORY_LIMIT_HOURS);
            if let Ok(db) = get_database() {
                let ids: Vec<i64> = results.iter().map(|item| item.id as i64).collect();
                if let Ok(created_dates) = db.get_created_at_batch(&ids) {
                    results.retain(|item| {
                        match created_dates.get(&(item.id as i64)) {
                            Some(created_at) => *created_at >= cutoff,
                            // If we can't look up the item, keep it (fail open for UX)
                            None => true,
                        }
                    });
                }
            }
        }
    }

    Ok(result)
}
/// Get scoring rejection rate statistics
#[tauri::command]
pub(crate) async fn get_scoring_stats() -> Result<crate::db::ScoringStatsAggregate> {
    let db = get_database()?;
    Ok(db
        .get_scoring_stats()
        .map_err(|e| format!("Failed to get scoring stats: {e}"))?)
}
// Settings and Context Engine commands are in settings_commands.rs
// ACE commands, PASIFA helpers, and auto-seeding are in ace_commands.rs
