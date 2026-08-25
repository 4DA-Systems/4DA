// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Cache-first analysis, status queries, and cancellation.

use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};

use futures::FutureExt;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::analysis_narration::NarrationEvent;
use crate::error::Result;
use crate::scoring;
use crate::stacks;
use crate::{
    achievement_engine, analysis_rerank, get_analysis_abort, get_analysis_state, get_database,
    monitoring, open_db_connection, void_signal_analysis_complete, void_signal_error,
    AnalysisState, SourceRelevance, ANALYSIS_TIMEOUT_SECS,
};

use super::analysis_cycle::{
    merge_freshness_refresh_batch, merge_stale_drain_batch, persist_cycle_results, CycleResults,
    RankProvenance,
};
use super::analysis_deep_scan::run_multi_source_analysis_impl;
use super::analysis_fast_path::{elapsed_ms, spawn_post_foreground_cache_fill, CachedAnalysisRun};
use super::{is_aborted, SIGNAL_CLASSIFIER};

// ============================================================================
// Cache-First Analysis (Option D)
// ============================================================================

/// Cache-first analysis - analyzes items already in the database
/// This is INSTANT because it doesn't fetch from APIs, just scores cached items
#[tauri::command]
pub(crate) async fn run_cached_analysis(app: AppHandle) -> Result<()> {
    crate::ipc_rate_limit::check_rate_limit("run_cached_analysis", 5)?;

    // Atomic check-and-set: prevents TOCTOU race from double-clicks.
    // If already running, return Ok (not error) — the user sees progress, not a failure.
    {
        get_analysis_abort().store(false, Ordering::SeqCst);
        let state = get_analysis_state();
        let mut guard = state.lock();
        if guard.running {
            info!(target: "4da::analysis", "Analysis already in progress — skipping duplicate request");
            return Ok(());
        }
        guard.running = true;
        guard.completed = false;
        guard.error = None;
        // Deliberately KEEP guard.results — the previous run's feed stays
        // visible while this run works. Blanking it here meant that during a
        // pipeline-version drain (every run slow, scheduled runs churning)
        // the state spent most of its time EMPTY: each new run wiped the
        // last completion's results before producing its own (2026-07-14
        // live incident). Results are replaced atomically on completion.
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
                // Store results INTO state BEFORE marking completed.
                // This ensures the frontend can always read results from state
                // even if the event emission below fails or races.
                let near_misses = crate::types::extract_near_misses(&results);
                guard.results = Some(results.clone());
                guard.near_misses = near_misses;
                // A completion supersedes any stale watchdog verdict: if
                // get_analysis_status auto-reset this run as "timed out"
                // while it was still (slowly) progressing, the error must
                // not survive next to real results.
                guard.error = None;
                guard.last_completed_at =
                    Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
                guard.completed = true; // Mark completed LAST — after all data is stored
                drop(guard);

                // Relevance scores, pipeline-version stamps, feed verdicts, and the
                // scoring-event telemetry row are persisted once at the shared
                // analysis boundary (analyze_cached_content_inner ->
                // persist_cycle_results) so foreground, scheduled, and headless runs
                // all curate the corpus identically. Do NOT re-add a per-wrapper
                // persist here — that split is the regression this fix removed.

                if let Err(e) = app.emit("analysis-complete", &results) {
                    tracing::warn!("Failed to emit 'analysis-complete': {e}");
                }
                analysis_rerank::maybe_save_digest(&results);

                let relevant_count = results.iter().filter(|r| r.relevant).count();
                if relevant_count > 0 {
                    monitoring::send_notification(&app, relevant_count, results.len());
                }

                void_signal_analysis_complete(&app, &results);

                // Invalidate blind spot cache so next tab switch recomputes
                // with fresh analysis data.
                crate::blind_spots::invalidate_blind_spot_cache();

                // Run post-analysis innovation hooks (non-blocking)
                scoring::run_post_analysis_hooks(&results);

                // Synthetic topic-affinity seeding removed in v19 (AD-029):
                // it fabricated engagement rows (positive_signals=3) that
                // the learned axis could not distinguish from real behavior.
                // retired-ok: documents the AD-029 demotion itself
                // With behavioral learning demoted from scoring authority,
                // the seed served no purpose and only polluted the
                // preferences/radar surfaces that still read affinities.

                // Manual analysis is now genuinely cache-first: finish scoring
                // visible cached data before touching the network. Refresh sources
                // afterward on a separate task so the next scheduled/manual pass has
                // fresh data without making this foreground completion wait on slow
                // or broken adapters.
                spawn_post_foreground_cache_fill(app.clone());

                // Background content enrichment for ambiguous-zone items.
                // Fetches page body for title-only items scoring 0.20–0.55,
                // so the next analysis cycle can re-score with richer signal.
                tokio::spawn(async move {
                    if let Ok(db) = crate::get_database() {
                        let count = crate::content_enrichment::enrich_ambiguous_items(db).await;
                        if count > 0 {
                            tracing::info!(
                                target: "4da::enrichment",
                                enriched = count,
                                "Post-analysis enrichment complete"
                            );
                        }
                    }
                });

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
                    for a in achievement_engine::increment_counter(db, "scans", 1) {
                        crate::events::emit_achievement_unlocked(&app, &a);
                    }
                    if relevant_count > 0 {
                        for a in achievement_engine::increment_counter(
                            db,
                            "discoveries",
                            relevant_count as u64,
                        ) {
                            crate::events::emit_achievement_unlocked(&app, &a);
                        }
                    }
                    let source_types: std::collections::HashSet<&str> =
                        results.iter().map(|r| r.source_type.as_str()).collect();
                    if source_types.len() >= 3 {
                        for a in achievement_engine::increment_counter(
                            db,
                            "sources",
                            source_types.len() as u64,
                        ) {
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

/// Uses differential analysis when previous results exist (only scores new items)
pub(crate) async fn analyze_cached_content_impl(app: &AppHandle) -> Result<Vec<SourceRelevance>> {
    Ok(
        analyze_cached_content_inner(app, CachedAnalysisRun::foreground_fast())
            .await?
            .results,
    )
}

/// Cache-first analysis with control over user-facing progress emission.
///
/// `silent = true` is used by the background/scheduled scheduler so a background
/// refresh does not hijack the user's visible progress bar. The work (fetch,
/// score, persist) still runs from the caller's orchestration path; only the
/// intermediate `emit_progress`/`emit_narration` surface events are suppressed.
pub(crate) async fn analyze_cached_content_silent(app: &AppHandle) -> Result<CycleResults> {
    let cycle = analyze_cached_content_inner(app, CachedAnalysisRun::background_deep()).await?;
    // Item 9 state-restore fix: the scheduled path used to TAKE state.results
    // for the differential merge and never put them back, so the next run's
    // merge base (and the frontend's get_analysis_status hydration) was empty.
    // Mirror the foreground completion handler — but only with a full-fidelity
    // corpus (a partial differential set must never shrink the shared feed),
    // and never over a foreground run that started mid-cycle.
    if cycle.full_display {
        let state = get_analysis_state();
        let mut guard = state.lock();
        if !guard.running {
            guard.near_misses = crate::types::extract_near_misses(&cycle.results);
            guard.results = Some(cycle.results.clone());
            guard.last_completed_at =
                Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
        }
    }
    Ok(cycle)
}

/// Persistence boundary over `analyze_cached_content_inner_impl`. EVERY analysis
/// path (foreground / scheduled / headless) reaches the scorer through here, so
/// persisting the cycle's results at this single point guarantees the curated
/// corpus (`feed_relevant` + `scoring_events`) advances for all of them. Do NOT
/// move persistence back into the individual caller wrappers — that split is the
/// regression this consolidation fixed (see `persist_cycle_results`).
///
/// Persists ONLY what this run actually scored (`CycleResults::scored_ids`):
/// on a differential run the carried-over previous results are display state,
/// and re-writing them would stamp hours-old in-memory scores/verdicts over
/// whatever the backfill, drain, and repair passes wrote since.
async fn analyze_cached_content_inner(
    app: &AppHandle,
    run: CachedAnalysisRun,
) -> Result<CycleResults> {
    let cycle = analyze_cached_content_inner_impl(app, run).await?;
    if let Ok(db) = get_database() {
        match &cycle.scored_ids {
            None => persist_cycle_results(db, &cycle.results),
            Some(ids) => {
                let scored: Vec<SourceRelevance> = cycle
                    .results
                    .iter()
                    .filter(|r| ids.contains(&r.id))
                    .cloned()
                    .collect();
                persist_cycle_results(db, &scored);
            }
        }
    }
    // Converge the verdicts this cycle did NOT touch. The cycle only re-judges
    // what `get_items_tiered` selects, so an item that ages out of that window
    // keeps whatever verdict a superseded pipeline version gave it — forever,
    // and invisibly, because the drain only tracks stale SCORES. Running here
    // (after persistence, on the single boundary every foreground / scheduled /
    // headless path reaches) means the guard converges on end-user machines
    // without an operator-run drain. No-op probe when nothing is stale.
    crate::analysis_backfill::reconcile_stale_verdicts_logged().await;
    Ok(cycle)
}

async fn analyze_cached_content_inner_impl(
    app: &AppHandle,
    run: CachedAnalysisRun,
) -> Result<CycleResults> {
    let analysis_started = Instant::now();
    let silent = run.silent;
    info!(
        target: "4da::analysis",
        silent,
        run_type = run.run_type,
        repair_pending_embeddings = run.repair_pending_embeddings,
        drain_stale_backlog = run.drain_stale_backlog,
        llm_rerank = run.llm_rerank,
        "=== CACHE-FIRST ANALYSIS STARTED ==="
    );

    // Gated emitters: when `silent`, suppress user-facing progress/narration so a
    // background refresh doesn't move the foreground progress bar. Call sites stay
    // unchanged; these shadow the free functions (which are reached via fully
    // qualified paths inside the closures).
    let emit_progress = |app: &AppHandle,
                         stage: &str,
                         progress: f32,
                         message: &str,
                         processed: usize,
                         total: usize| {
        if !silent {
            crate::emit_progress(app, stage, progress, message, processed, total);
        }
    };
    let emit_narration = |app: &AppHandle, ev: NarrationEvent| {
        if !silent {
            crate::analysis_narration::emit_narration(app, ev);
        }
    };

    emit_progress(app, "init", 0.0, "Loading cached items...", 0, 0);

    let db = get_database()?;

    // Keep foreground manual analysis fast. Pending-embedding repair is a
    // maintenance loop, so background/headless cycles own it; manual analysis
    // scores the best cache it has and does not wait on repair work.
    if run.repair_pending_embeddings {
        let repair_started = Instant::now();
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
                match crate::embed_texts(&texts).await {
                    Ok(embeddings) => {
                        let (mut upgraded, mut fallback, mut failed) = (0usize, 0usize, 0usize);
                        let mut first_error: Option<String> = None;
                        for ((id, _, _, _), embedding) in pending.iter().zip(embeddings.iter()) {
                            if embedding.iter().all(|&v| v == 0.0) {
                                fallback += 1;
                                continue;
                            }
                            match db.upgrade_pending_to_complete(*id, embedding) {
                                Ok(()) => upgraded += 1,
                                Err(e) => {
                                    failed += 1;
                                    first_error.get_or_insert_with(|| e.to_string());
                                }
                            }
                        }
                        // ALWAYS report the outcome. A repair loop that upgrades nothing
                        // must never again look identical to one that had no work to do:
                        // the previous `if upgraded > 0` gate hid 624 consecutive total
                        // failures for three months (vec0 rejecting `INSERT OR REPLACE`),
                        // while 887 items accumulated stale embeddings.
                        if upgraded > 0 {
                            info!(
                                target: "4da::analysis",
                                upgraded,
                                failed,
                                fallback,
                                total = pending.len(),
                                elapsed_ms = elapsed_ms(repair_started),
                                "Re-embedded previously pending items"
                            );
                        } else {
                            warn!(
                                target: "4da::analysis",
                                total = pending.len(), failed, fallback,
                                elapsed_ms = elapsed_ms(repair_started),
                                error = first_error.as_deref().unwrap_or("none"),
                                "Re-embed cycle upgraded NOTHING — the repair pipeline is not making progress"
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            target: "4da::analysis",
                            error = %e,
                            count = pending.len(),
                            elapsed_ms = elapsed_ms(repair_started),
                            "Re-embed batch could not be embedded"
                        );
                    }
                }
            }
            _ => {} // No pending items or DB error - continue normally
        }
    } else {
        info!(target: "4da::analysis", "Skipping pending-embedding repair on foreground fast path");
    }

    // Previous in-memory results are ONLY a merge/display optimization now.
    // Use .take() to move results out of the guard instead of cloning the entire Vec.
    // This is safe: analysis is running, so old results will be replaced when it completes.
    let previous_results = {
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.results.take()
    };

    // ── DB-watermark differential gate (2026-08-23 audit, item 9) ────────
    // The old gate lived in in-memory state (`last_completed_at` + previous
    // results), which the headless engine — a fresh process every 30 minutes —
    // NEVER had, and which the scheduled path drained and never restored. So
    // differential scoring was structurally dead: every background run
    // re-scored the full window (~660 items to write ~30 new ones, measured
    // live). The watermark now comes from `engine_runs`, which every scoring
    // cycle of every trigger records against the shared database. The
    // admission rule (freshness, pending-drain bound, display safety) lives in
    // `engine_runs::differential_since`.
    let watermark = crate::engine_runs::last_scoring_watermark();
    let stale_backlog = if watermark.is_some() {
        // Probe failure counts as "unbounded backlog" → full window (safe).
        db.count_stale_scored_items(scoring::PIPELINE_VERSION)
            .unwrap_or(i64::MAX)
    } else {
        0
    };
    let differential_since = crate::engine_runs::differential_since(
        watermark.as_ref(),
        stale_backlog,
        previous_results.is_some(),
        run.silent,
    );

    if let Some(since) = differential_since {
        info!(
            target: "4da::analysis",
            since = %since,
            stale_backlog,
            merge_base = previous_results.is_some(),
            "Differential analysis (DB watermark) - checking for new items since the last successful scoring run"
        );

        let select_started = Instant::now();
        let mut new_items = db
            .get_items_since_timestamp_tiered(&since, 500)
            .map_err(|e| format!("Failed to load new items: {e}"))?;
        let changed_count = new_items.len();
        info!(
            target: "4da::analysis",
            items = changed_count,
            elapsed_ms = elapsed_ms(select_started),
            "Selected differential candidates"
        );

        // Deep/background cycles drain a batch of items scored under an older
        // pipeline version. Foreground manual analysis skips this maintenance
        // work so a user click scores visible cache instead of inheriting a
        // bounded-but-slow backlog repair.
        let drained = if run.drain_stale_backlog {
            let stale_started = Instant::now();
            let drained = merge_stale_drain_batch(db, &mut new_items);
            if drained > 0 {
                info!(
                    target: "4da::analysis",
                    stale = drained,
                    elapsed_ms = elapsed_ms(stale_started),
                    "Re-scoring stale items from an older pipeline version (merged with new items)"
                );
                emit_progress(
                    app,
                    "cache",
                    0.5,
                    &format!("Re-scoring {drained} items (pipeline updated)..."),
                    0,
                    drained,
                );
            }
            drained
        } else {
            info!(target: "4da::analysis", "Skipping stale-version backlog drain on foreground fast path");
            0
        };

        // ── Rolling freshness refresh (2026-08-25 tightening T1) ─────────
        // The differential selection now keys on CHANGE (created_at /
        // content_updated_at), no longer on last_seen touches — so unchanged
        // items are never re-selected, and the quiet-cycle full re-score
        // below (which fires only when this set is EMPTY) almost never runs.
        // Merge the stalest-scored slice of the 7-day window each background
        // cycle so freshness-tier decay keeps its cadence on a bounded
        // budget (FRESHNESS_REFRESH_PER_CYCLE = 100 → the whole window every
        // ~7 cycles ≈ 3.5 h). Foreground fast path skips it like the other
        // maintenance work; the scheduled/headless cycles carry the cadence.
        let freshness = if run.drain_stale_backlog {
            merge_freshness_refresh_batch(db, &mut new_items)
        } else {
            0
        };
        info!(
            target: "4da::analysis",
            changed = changed_count,
            drain = drained,
            freshness,
            total = new_items.len(),
            "Differential batch composition"
        );

        if new_items.is_empty() {
            // Nothing changed, nothing stale, AND the freshness batch found
            // nothing (background: the 7-day window is empty; foreground: a
            // quiet cycle with maintenance skipped) — fall back to the full
            // 7-day re-score. Before tightening T1 this was the ONLY
            // freshness-decay path, and it almost never fired because the
            // touch-based differential was never empty; the rolling refresh
            // above now carries the cadence, and this stays as the fallback.
            info!(target: "4da::analysis", "No new or stale items, re-scoring existing for freshness");
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
                return Ok(CycleResults::full(
                    run_multi_source_analysis_impl(app, silent).await?,
                ));
            }

            let results =
                scoring::score_items_full(app, db, &all_items, silent, run.llm_rerank).await?;
            info!(
                target: "4da::analysis",
                run_type = run.run_type,
                elapsed_ms = elapsed_ms(analysis_started),
                "Cache-first analysis finished"
            );
            return Ok(CycleResults::full(results));
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

        // Score only new items (10s timeout on context build — it's local DB + ACE, should be fast)
        let scoring_ctx = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            scoring::build_scoring_context(db),
        )
        .await
        .map_err(|_| String::from("Scoring context build timed out after 10s"))?
        .map_err(|e| format!("Failed to build scoring context for differential analysis: {e}"))?;
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

            // Path parity with the analyzer path: parse topic tags (§3.5).
            let parsed_tags = scoring::parse_tags_topics(item.tags.as_deref());
            new_results.push(scoring::score_item(
                &scoring::ScoringInput {
                    id: item.id as u64,
                    title: &item.title,
                    url: item.url.as_deref(),
                    content: &item.content,
                    source_type: &item.source_type,
                    embedding: &item.embedding,
                    // Effective publication date: honest freshness (falls back to first-seen)
                    created_at: Some(item.published_at.as_ref().unwrap_or(&item.created_at)),
                    detected_lang: &item.detected_lang,
                    source_tags: &parsed_tags,
                    tags_json: item.tags.as_deref(),
                    feed_origin: item.feed_origin.as_deref(),
                    source_id: Some(&item.source_id),
                },
                &scoring_ctx,
                db,
                &options,
                Some(&SIGNAL_CLASSIFIER),
            ));
        }

        // Rank provenance (audit items 12+26): on the differential path only
        // the LLM advisor can move `top_score` away from `evidence_score`
        // (which score_item set at construction). Recording it keeps the
        // persisted rank layer honest about why differential ranks differ
        // from full-pass ranks.
        let mut rank_prov = RankProvenance::begin(&new_results);
        if run.llm_rerank {
            // LLM Reranking on new items only (if enabled)
            // 120s timeout: LLM API calls can hang on provider outages
            emit_narration(
                app,
                NarrationEvent {
                    narration_type: "insight".into(),
                    message: "Ranking items against your profile...".into(),
                    source: None,
                    relevance: None,
                },
            );
            let rerank_started = Instant::now();
            match tokio::time::timeout(
                std::time::Duration::from_mins(2),
                analysis_rerank::apply_llm_reranking(app, &mut new_results, &scoring_ctx),
            )
            .await
            {
                Ok(outcome) => {
                    outcome.log(elapsed_ms(rerank_started), "differential");
                }
                Err(_) => {
                    warn!(target: "4da::analysis", "LLM reranking timed out after 120s, using pipeline scores only");
                }
            }
        } else {
            info!(target: "4da::analysis", "Skipping LLM rerank on foreground fast path");
        }
        rank_prov.record(&new_results, "llm");
        rank_prov.finish(&mut new_results);

        // Merge: take previous results (display optimization), update/add new
        // ones by ID. Without a merge base (headless / cold process) the set
        // stays partial — flagged via `full_display` so it can never REPLACE
        // the shared feed, only merge into it frontend-side.
        let scored_ids: std::collections::HashSet<u64> = new_results.iter().map(|r| r.id).collect();
        let full_display = previous_results.is_some();
        let mut prev = previous_results.unwrap_or_default();
        prev.retain(|r| !scored_ids.contains(&r.id));
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

        info!(
            target: "4da::analysis",
            run_type = run.run_type,
            elapsed_ms = elapsed_ms(analysis_started),
            "Cache-first analysis finished"
        );
        return Ok(CycleResults {
            results: prev,
            scored_ids: Some(scored_ids),
            full_display,
        });
    }

    // Full analysis path (no previous results or first run)
    // Use 7-day window to include items from recent fetches
    // Respects free-tier 30-day history gate via get_items_tiered
    let select_started = Instant::now();
    let mut cached_items = db
        .get_items_tiered(168, 1000)
        .map_err(|e| format!("Failed to load cached items: {e}"))?;
    info!(
        target: "4da::analysis",
        items = cached_items.len(),
        elapsed_ms = elapsed_ms(select_started),
        "Selected full-analysis candidates"
    );

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
        return Ok(CycleResults::full(
            run_multi_source_analysis_impl(app, silent).await?,
        ));
    }

    // Background/headless full passes also drain stale pipeline-version backlog.
    // Foreground manual passes skip it: correctness still converges in background,
    // while the visible user action returns after scoring current cache.
    if run.drain_stale_backlog {
        let stale_started = Instant::now();
        let drained = merge_stale_drain_batch(db, &mut cached_items);
        if drained > 0 {
            info!(
                target: "4da::analysis",
                stale = drained,
                elapsed_ms = elapsed_ms(stale_started),
                "Re-scoring stale backlog items from an older pipeline version (full path)"
            );
        }
    } else {
        info!(target: "4da::analysis", "Skipping stale-version backlog drain on foreground fast path");
    }

    let results = scoring::score_items_full(app, db, &cached_items, silent, run.llm_rerank).await?;
    info!(
        target: "4da::analysis",
        run_type = run.run_type,
        elapsed_ms = elapsed_ms(analysis_started),
        "Cache-first analysis finished"
    );
    Ok(CycleResults::full(results))
}

/// Cancel a running analysis
#[tauri::command]
pub(crate) async fn cancel_analysis() -> Result<()> {
    get_analysis_abort().store(true, Ordering::SeqCst);
    info!(target: "4da::analysis", "Analysis cancellation requested");

    // Give the analysis task up to 5 seconds to observe the abort flag and stop
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Force cleanup if the task is still marked as running
    let state = get_analysis_state();
    let mut guard = state.lock();
    if guard.running {
        warn!(target: "4da::analysis", "Analysis still running after cancellation timeout — forcing cleanup");
        guard.running = false;
        guard.error = Some("Analysis cancelled by user".to_string());
    }
    drop(guard);

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
