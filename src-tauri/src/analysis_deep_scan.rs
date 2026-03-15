//! Deep scan and multi-source analysis implementations.

use tauri::{AppHandle, Emitter};
use tracing::{info, warn};

use crate::analysis_narration::{emit_narration, NarrationEvent};
use crate::error::Result;
use crate::scoring;
use crate::stacks;
use futures::FutureExt;

use crate::{
    analysis_rerank, emit_progress, game_engine, get_analysis_abort, get_analysis_state,
    get_database, monitoring, open_db_connection, source_fetching, truncate_utf8, SourceRelevance,
};

use std::sync::atomic::Ordering;

use super::{is_aborted, SIGNAL_CLASSIFIER};

/// Deep initial scan - comprehensive first-time scan for new users
/// Fetches 300-500+ items from all sources using multiple endpoints
#[tauri::command]
pub(crate) async fn run_deep_initial_scan(app: AppHandle) -> Result<()> {
    // Check if already running
    {
        let state = get_analysis_state();
        let guard = state.lock();
        if guard.running {
            return Err("Analysis already running".into());
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
        let result = std::panic::AssertUnwindSafe(run_deep_initial_scan_impl(&app))
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
                let top_picks = results.iter().filter(|r| r.top_score >= 0.6).count();
                info!(target: "4da::analysis",
                    "=== DEEP INITIAL SCAN COMPLETE ===\n  Total: {} items\n  Relevant: {}\n  Top Picks (60%+): {}",
                    results.len(), relevant_count, top_picks
                );
                if relevant_count > 0 {
                    monitoring::send_notification(&app, relevant_count, results.len());
                }

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
                    // Increment scan counter
                    for a in game_engine::increment_counter(db, "scans", 1) {
                        crate::events::emit_achievement_unlocked(&app, &a);
                    }
                    // Increment discoveries counter
                    if relevant_count > 0 {
                        for a in
                            game_engine::increment_counter(db, "discoveries", relevant_count as u64)
                        {
                            crate::events::emit_achievement_unlocked(&app, &a);
                        }
                    }
                    // Track source diversity
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
                tracing::error!(target: "4da::analysis", error = %e, "Deep initial scan failed");
                let err_str = e.to_string();
                // Emit first (borrows), then move into guard (no clone needed)
                if let Err(e) = app.emit("analysis-error", &err_str) {
                    tracing::warn!("Failed to emit 'analysis-error': {e}");
                }
                guard.error = Some(err_str);
            }
            Err(_panic) => {
                tracing::error!(target: "4da::analysis", "Deep scan task panicked — running flag cleared");
                let msg = "Deep scan panicked (internal error)".to_string();
                if let Err(e) = app.emit("analysis-error", &msg) {
                    tracing::warn!("Failed to emit 'analysis-error': {e}");
                }
                guard.error = Some(msg);
            }
        }
    });

    Ok(())
}

/// Deep initial scan implementation - comprehensive first-time intelligence gathering
pub(crate) async fn run_deep_initial_scan_impl(app: &AppHandle) -> Result<Vec<SourceRelevance>> {
    info!(target: "4da::analysis", "=== DEEP INITIAL SCAN STARTED ===");
    info!(target: "4da::analysis", "Fetching 300-500+ items from HN (5 categories), arXiv (16 categories), Reddit (40+ subreddits)...");

    // Narration: analysis start
    emit_narration(
        app,
        NarrationEvent {
            narration_type: "discovery".into(),
            message: "Scanning sources for intelligence...".into(),
            source: None,
            relevance: None,
        },
    );

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
    let cached_context_count = db
        .context_count()
        .map_err(|e| format!("Failed to count context chunks: {}", e))?;

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
    let all_items = source_fetching::fetch_all_sources_deep(db, app, 100)
        .await
        .map_err(|e| format!("Deep fetch failed: {}", e))?;
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
    let scoring_ctx = scoring::build_scoring_context(db)
        .await
        .map_err(|e| format!("Failed to build scoring context for deep scan: {}", e))?;
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

    // Narration: scoring start
    emit_narration(
        app,
        NarrationEvent {
            narration_type: "insight".into(),
            message: format!("Scoring {} items against your profile...", total_items),
            source: None,
            relevance: None,
        },
    );

    let mut high_match_count: usize = 0;

    for (idx, (item, item_embedding)) in all_items.iter().enumerate() {
        if is_aborted() {
            info!(target: "4da::analysis", scored = idx, "Deep scan aborted by user");
            return Err("Analysis cancelled".into());
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

            // Emit partial results for progressive rendering
            if !results.is_empty() {
                let batch_end = results.len();
                let batch_start = batch_end.saturating_sub(50);
                if let Err(e) = app.emit("partial-results", &results[batch_start..batch_end]) {
                    tracing::warn!("Failed to emit 'partial-results': {e}");
                }
            }
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

        // Narration: high-relevance match (max 3)
        if let Some(scored) = results.last() {
            if scored.top_score > 0.7 && high_match_count < 3 {
                high_match_count += 1;
                let title_preview: String = item.title.chars().take(60).collect();
                emit_narration(
                    app,
                    NarrationEvent {
                        narration_type: "match".into(),
                        message: format!("High match: \"{}\"", title_preview),
                        source: Some(item.source_type.clone()),
                        relevance: Some(scored.top_score),
                    },
                );
            }
        }
    }

    scoring::sort_results(&mut results);

    // Narration: scoring complete
    {
        let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
        emit_narration(
            app,
            NarrationEvent {
                narration_type: "insight".into(),
                message: format!(
                    "{} items scored, {} above your threshold",
                    results.len(),
                    relevant_count
                ),
                source: None,
                relevance: None,
            },
        );
    }

    // Narration: top signal
    if let Some(top) = results.first() {
        let title_preview: String = top.title.chars().take(50).collect();
        emit_narration(
            app,
            NarrationEvent {
                narration_type: "insight".into(),
                message: format!(
                    "Top signal: \"{}\" ({:.0}%)",
                    title_preview,
                    top.top_score * 100.0
                ),
                source: Some(top.source_type.clone()),
                relevance: Some(top.top_score),
            },
        );
    }

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

    // Record rejection rate for verifiable metrics
    if let Err(e) = db.record_scoring_stats(
        "deep_initial",
        results.len(),
        relevant_count,
        excluded_count,
    ) {
        tracing::warn!(target: "4da::analysis", error = %e, "Failed to record scoring stats");
    }

    Ok(results)
}

/// Multi-source analysis implementation
pub(crate) async fn run_multi_source_analysis_impl(
    app: &AppHandle,
) -> Result<Vec<SourceRelevance>> {
    info!(target: "4da::analysis", "=== MULTI-SOURCE ANALYSIS STARTED ===");

    // Narration: analysis start
    emit_narration(
        app,
        NarrationEvent {
            narration_type: "discovery".into(),
            message: "Scanning sources for intelligence...".into(),
            source: None,
            relevance: None,
        },
    );

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
    let cached_context_count = db
        .context_count()
        .map_err(|e| format!("Failed to count context chunks: {}", e))?;

    if cached_context_count > 0 {
        info!(target: "4da::analysis", context_chunks = cached_context_count, "Context indexed (using KNN search)");
    } else {
        warn!(target: "4da::analysis", "No context indexed. Running without context-based scoring");
    }

    // Step 2: Fetch from all sources (50 items per source for comprehensive coverage)
    emit_progress(app, "fetch", 0.1, "Fetching from all sources...", 0, 0);
    let all_items = source_fetching::fetch_all_sources(db, app, 50)
        .await
        .map_err(|e| format!("Multi-source fetch failed: {}", e))?;
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
    let scoring_ctx = scoring::build_scoring_context(db)
        .await
        .map_err(|e| format!("Failed to build scoring context: {}", e))?;
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

    // Narration: scoring start
    emit_narration(
        app,
        NarrationEvent {
            narration_type: "insight".into(),
            message: format!("Scoring {} items against your profile...", all_items.len()),
            source: None,
            relevance: None,
        },
    );

    let mut high_match_count: usize = 0;

    for (idx, (item, item_embedding)) in all_items.iter().enumerate() {
        if is_aborted() {
            info!(target: "4da::analysis", scored = idx, "Multi-source analysis aborted by user");
            return Err("Analysis cancelled".into());
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

        // Narration: high-relevance match (max 3)
        if let Some(scored) = results.last() {
            if scored.top_score > 0.7 && high_match_count < 3 {
                high_match_count += 1;
                let title_preview: String = item.title.chars().take(60).collect();
                emit_narration(
                    app,
                    NarrationEvent {
                        narration_type: "match".into(),
                        message: format!("High match: \"{}\"", title_preview),
                        source: Some(item.source_type.clone()),
                        relevance: Some(scored.top_score),
                    },
                );
            }
        }
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
    emit_narration(
        app,
        NarrationEvent {
            narration_type: "insight".into(),
            message: "Ranking items against your profile...".into(),
            source: None,
            relevance: None,
        },
    );
    analysis_rerank::apply_llm_reranking(app, &mut results, &scoring_ctx).await;

    // Narration: scoring complete
    {
        let narr_relevant = results.iter().filter(|r| r.relevant && !r.excluded).count();
        emit_narration(
            app,
            NarrationEvent {
                narration_type: "insight".into(),
                message: format!(
                    "{} items scored, {} above your threshold",
                    results.len(),
                    narr_relevant
                ),
                source: None,
                relevance: None,
            },
        );
    }

    // Narration: top signal
    if let Some(top) = results.first() {
        let title_preview: String = top.title.chars().take(50).collect();
        emit_narration(
            app,
            NarrationEvent {
                narration_type: "insight".into(),
                message: format!(
                    "Top signal: \"{}\" ({:.0}%)",
                    title_preview,
                    top.top_score * 100.0
                ),
                source: Some(top.source_type.clone()),
                relevance: Some(top.top_score),
            },
        );
    }

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

    // Record rejection rate for verifiable metrics
    if let Err(e) = db.record_scoring_stats(
        "multi_source",
        results.len(),
        relevant_count,
        excluded_count,
    ) {
        tracing::warn!(target: "4da::analysis", error = %e, "Failed to record scoring stats");
    }

    Ok(results)
}
