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
    emit_progress, get_analysis_abort, get_analysis_state, get_database, get_settings_manager,
    monitoring, signals, source_fetching, truncate_utf8, void_signal_analysis_complete,
    void_signal_error, AnalysisState, SourceRelevance, ANALYSIS_TIMEOUT_SECS,
};

// Singleton SignalClassifier - created once and reused across all analyses
static SIGNAL_CLASSIFIER: Lazy<signals::SignalClassifier> =
    Lazy::new(signals::SignalClassifier::new);

/// Build a rich context summary for LLM reranking.
/// Provides the LLM with everything it needs to judge genuine usefulness.
fn build_rerank_context_summary(ctx: &scoring::ScoringContext) -> String {
    let mut parts = Vec::new();

    // 1. Primary tech stack (declared by user, not the 95 auto-detected items)
    if !ctx.declared_tech.is_empty() {
        parts.push(format!("Primary tech: {}", ctx.declared_tech.join(", ")));
    } else if !ctx.ace_ctx.detected_tech.is_empty() {
        // Fallback to detected, but limit to top 8
        let top: Vec<&str> = ctx
            .ace_ctx
            .detected_tech
            .iter()
            .take(8)
            .map(|s| s.as_str())
            .collect();
        parts.push(format!("Tech stack: {}", top.join(", ")));
    }

    // 2. Key dependencies (non-dev, notable packages)
    if !ctx.ace_ctx.dependency_info.is_empty() {
        let notable_deps: Vec<&str> = ctx
            .ace_ctx
            .dependency_info
            .values()
            .filter(|d| !d.is_dev)
            .take(15)
            .map(|d| d.package_name.as_str())
            .collect();
        if !notable_deps.is_empty() {
            parts.push(format!("Key dependencies: {}", notable_deps.join(", ")));
        }
    }

    // 3. Current work focus (work topics from recent git activity)
    if !ctx.work_topics.is_empty() {
        parts.push(format!(
            "Currently working on: {}",
            ctx.work_topics.join(", ")
        ));
    }

    // 4. Anti-technologies (competing tech the user has chosen NOT to use)
    if !ctx.domain_profile.primary_stack.is_empty() {
        let anti = crate::competing_tech::get_anti_dependencies(&ctx.domain_profile.primary_stack);
        if !anti.is_empty() {
            let mut anti_vec: Vec<&str> = anti.iter().map(|s| s.as_str()).collect();
            anti_vec.sort();
            anti_vec.truncate(10);
            parts.push(format!(
                "Does NOT use (chose alternatives): {}",
                anti_vec.join(", ")
            ));
        }
    }

    // 5. Anti-topics (learned from behavior)
    if !ctx.ace_ctx.anti_topics.is_empty() {
        parts.push(format!(
            "Consistently rejects: {}",
            ctx.ace_ctx.anti_topics.join(", ")
        ));
    }

    // 6. Declared interests
    if !ctx.interests.is_empty() {
        let names: Vec<&str> = ctx
            .interests
            .iter()
            .take(10)
            .map(|i| i.topic.as_str())
            .collect();
        parts.push(format!("Interests: {}", names.join(", ")));
    }

    // 7. Recent git commits (from DB)
    if let Ok(db) = crate::open_db_connection() {
        // Recent commit messages
        if let Ok(mut stmt) = db.prepare(
            "SELECT commit_message FROM git_signals WHERE commit_message IS NOT NULL ORDER BY timestamp DESC LIMIT 5",
        ) {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                let commits: Vec<String> = rows.flatten().collect();
                if !commits.is_empty() {
                    let commit_lines: Vec<String> = commits
                        .iter()
                        .map(|c| {
                            let truncated: String = c.chars().take(80).collect();
                            format!("- {}", truncated)
                        })
                        .collect();
                    parts.push(format!("Recent commits:\n{}", commit_lines.join("\n")));
                }
            }
        }

        // Recently engaged topics (from feedback/interactions)
        if let Ok(mut stmt) = db.prepare(
            "SELECT DISTINCT si.title FROM feedback f JOIN source_items si ON si.id = f.source_item_id WHERE f.relevant = 1 ORDER BY f.created_at DESC LIMIT 5",
        ) {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                let saved: Vec<String> = rows.flatten().collect();
                if !saved.is_empty() {
                    let titles: Vec<String> = saved
                        .iter()
                        .map(|t| {
                            let truncated: String = t.chars().take(60).collect();
                            format!("- {}", truncated)
                        })
                        .collect();
                    parts.push(format!("Recently saved:\n{}", titles.join("\n")));
                }
            }
        }
    }

    parts.join("\n")
}

/// Apply LLM reranking to scored results if enabled and within limits.
/// Uses smaller batches (8 items) with real article content for accurate judging.
/// Returns the number of items judged, or None if skipped.
async fn apply_llm_reranking(
    app: &AppHandle,
    results: &mut [SourceRelevance],
    scoring_ctx: &scoring::ScoringContext,
) -> Option<usize> {
    let (rerank_enabled, rerank_config) = {
        let mut settings = get_settings_manager().lock();
        let enabled = settings.is_rerank_enabled() && settings.within_daily_limits();
        let config = settings.get().rerank.clone();
        (enabled, config)
    };

    if !rerank_enabled {
        return None;
    }

    let context_summary = build_rerank_context_summary(scoring_ctx);
    if context_summary.is_empty() {
        info!(target: "4da::rerank", "No context available for reranking, skipping");
        return None;
    }

    // Get database for content snippets
    let db = match get_database() {
        Ok(db) => db,
        Err(_) => return None,
    };

    // Select candidates with ACTUAL content from the database
    let candidates: Vec<(String, String, String)> = results
        .iter()
        .filter(|r| r.top_score >= rerank_config.min_embedding_score && !r.excluded)
        .take(rerank_config.max_items_per_batch)
        .map(|r| {
            let content_snippet = db
                .get_item_content_snippet(r.id as i64, 300)
                .unwrap_or_default();
            let source_label = format!("[{}]", r.source_type);
            (
                r.id.to_string(),
                r.title.clone(),
                format!("{} {}", source_label, content_snippet),
            )
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    let llm_settings = {
        let settings = get_settings_manager().lock();
        settings.get().llm.clone()
    };
    let judge = crate::llm::RelevanceJudge::new(llm_settings);

    // Split into batches of 8 for better LLM accuracy
    const LLM_BATCH_SIZE: usize = 8;
    let batches: Vec<Vec<(String, String, String)>> = candidates
        .chunks(LLM_BATCH_SIZE)
        .map(|c| c.to_vec())
        .collect();

    let total_batches = batches.len();
    let total_candidates = batches.iter().map(|b| b.len()).sum::<usize>();
    let mut all_judgments = Vec::new();
    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;

    for (batch_idx, batch) in batches.iter().enumerate() {
        emit_progress(
            app,
            "rerank",
            0.90 + (batch_idx as f32 / total_batches as f32) * 0.08,
            &format!(
                "LLM judging batch {}/{} ({} items)...",
                batch_idx + 1,
                total_batches,
                batch.len()
            ),
            all_judgments.len(),
            total_candidates,
        );

        match judge.judge_batch(&context_summary, batch.clone()).await {
            Ok((judgments, input_tokens, output_tokens)) => {
                total_input += input_tokens;
                total_output += output_tokens;
                all_judgments.extend(judgments);
            }
            Err(e) => {
                warn!(target: "4da::rerank", batch = batch_idx, error = %e, "LLM batch failed, continuing");
            }
        }
    }

    if all_judgments.is_empty() {
        return None;
    }

    let mut confirmed = 0usize;
    let mut rejected = 0usize;

    for judgment in &all_judgments {
        if let Some(result) = results
            .iter_mut()
            .find(|r| r.id.to_string() == judgment.item_id)
        {
            // Store LLM score and reason in breakdown
            if let Some(ref mut breakdown) = result.score_breakdown {
                breakdown.llm_score = Some(judgment.confidence * 5.0); // Map back to 1-5
                breakdown.llm_reason = if judgment.reasoning.is_empty() {
                    None
                } else {
                    Some(judgment.reasoning.clone())
                };
            }

            if !judgment.relevant {
                // LLM says not relevant — hard reject
                result.relevant = false;
                result.top_score *= 0.15;
                result.explanation = Some(format!("Filtered: {}", judgment.reasoning));
                rejected += 1;
            } else {
                // LLM confirms — LLM score dominates (70/30 blend)
                result.top_score =
                    (result.top_score * 0.3 + judgment.confidence * 0.7).clamp(0.0, 1.0);
                if !judgment.reasoning.is_empty() {
                    result.explanation = Some(judgment.reasoning.clone());
                }
                confirmed += 1;
            }
        }
    }

    // Re-sort after LLM adjustments
    scoring::sort_results(results);

    // Track token usage for daily limits
    {
        let mut settings = get_settings_manager().lock();
        let cost = judge.estimate_cost_cents(total_input, total_output);
        settings.record_usage(total_input + total_output, cost);
    }

    info!(target: "4da::rerank",
        judged = all_judgments.len(),
        confirmed = confirmed,
        rejected = rejected,
        batches = total_batches,
        tokens = total_input + total_output,
        "LLM reranking complete"
    );

    Some(all_judgments.len())
}

/// Check if analysis has been aborted by the user
#[inline]
fn is_aborted() -> bool {
    get_analysis_abort().load(Ordering::SeqCst)
}

/// Generate and save digest from analysis results (if enabled)
pub(crate) fn maybe_save_digest(results: &[SourceRelevance]) {
    use crate::digest::{Digest, DigestItem, DigestManager};
    use chrono::{Duration, Utc};

    let settings = get_settings_manager().lock();
    let config = settings.get().digest.clone();
    drop(settings);

    // Check if digest is enabled and save_local is true
    if !config.enabled || !config.save_local {
        return;
    }

    // Filter to only relevant items above min_score
    let relevant_items: Vec<DigestItem> = results
        .iter()
        .filter(|r| r.relevant && r.top_score as f64 >= config.min_score)
        .take(config.max_items)
        .map(|r| DigestItem {
            id: r.id as i64,
            title: r.title.clone(),
            url: r.url.clone(),
            source: r.source_type.clone(),
            relevance_score: r.top_score as f64,
            matched_topics: r.matches.iter().map(|m| m.source_file.clone()).collect(),
            discovered_at: Utc::now(),
            summary: None,
            signal_type: r.signal_type.clone(),
            signal_priority: r.signal_priority.clone(),
            signal_action: r.signal_action.clone(),
        })
        .collect();

    if relevant_items.is_empty() {
        info!(target: "4da::digest", "No relevant items for digest, skipping");
        return;
    }

    // Create digest
    let period_end = Utc::now();
    let period_start = period_end - Duration::hours(24);
    let digest = Digest::new(relevant_items, period_start, period_end);

    // Save using DigestManager
    let manager = DigestManager::new(config);
    match manager.save_local(&digest) {
        Ok(path) => {
            info!(target: "4da::digest",
                path = %path.display(),
                items = digest.summary.total_items,
                "Digest saved successfully"
            );
        }
        Err(e) => {
            warn!(target: "4da::digest", error = %e, "Failed to save digest");
        }
    }
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
                maybe_save_digest(&results);

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
                run_post_analysis_hooks(&results);
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
    apply_llm_reranking(app, &mut results, &scoring_ctx).await;

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
                maybe_save_digest(&results);

                let relevant_count = results.iter().filter(|r| r.relevant).count();
                if relevant_count > 0 {
                    monitoring::send_notification(&app, relevant_count, results.len());
                }

                void_signal_analysis_complete(&app, &results);

                // Run post-analysis innovation hooks (non-blocking)
                run_post_analysis_hooks(&results);
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

/// Run innovation feature hooks after analysis completes
/// These populate temporal data needed by SignalChains, KnowledgeGaps, etc.
fn run_post_analysis_hooks(results: &[SourceRelevance]) {
    if let Ok(conn) = crate::open_db_connection() {
        // 1. Record attention events for engagement tracking
        let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
        let _ = crate::temporal::record_event(
            &conn,
            "attention_event",
            "analysis_complete",
            &serde_json::json!({
                "total_items": results.len(),
                "relevant_count": relevant_count,
                "source_types": results.iter()
                    .map(|r| r.source_type.as_str())
                    .collect::<std::collections::HashSet<_>>(),
            }),
            None,
            Some(&(chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339()),
        );

        // 2. Record topic centroids for semantic drift detection
        let mut topic_scores: std::collections::HashMap<String, Vec<f32>> =
            std::collections::HashMap::new();
        let mut topic_titles: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for item in results.iter().filter(|r| r.relevant) {
            let topics = crate::extract_topics(&item.title, "");
            for topic in topics {
                topic_scores
                    .entry(topic.clone())
                    .or_default()
                    .push(item.top_score);
                topic_titles
                    .entry(topic)
                    .or_default()
                    .push(item.title.clone());
            }
        }
        for (topic, scores) in &topic_scores {
            let avg = scores.iter().sum::<f32>() / scores.len() as f32;
            let titles = topic_titles
                .get(topic)
                .map(|t| t.iter().take(5).cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            let _ = crate::semantic_diff::record_topic_centroid(
                &conn,
                topic,
                scores.len() as u32,
                avg,
                &titles,
            );
        }

        // 3. Scan for reverse mentions of user's projects
        let _ = crate::reverse_relevance::scan_for_mentions(&conn, 24);

        info!(target: "4da::analysis", "Post-analysis innovation hooks complete");
    }
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
        let since = last_completed_at.as_deref().unwrap();
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

            return score_items_full(app, db, &all_items).await;
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
        apply_llm_reranking(app, &mut new_results, &scoring_ctx).await;

        // Merge: take previous results, update/add new ones by ID
        let mut prev = previous_results.unwrap();
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

    score_items_full(app, db, &cached_items).await
}

/// Deduplicate items by normalized URL and normalized title.
/// Keeps the first occurrence (usually the oldest/original source).
fn dedup_stored_items(items: &[crate::db::StoredSourceItem]) -> Vec<usize> {
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_titles: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut keep_indices = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        // URL-based dedup (normalized)
        if let Some(ref url) = item.url {
            let normalized = normalize_url(url);
            if !normalized.is_empty() && !seen_urls.insert(normalized) {
                continue; // duplicate URL
            }
        }
        // Title-based dedup (aggressive normalization)
        let title_key = normalize_title_for_dedup(&item.title);
        if !title_key.is_empty() && !seen_titles.insert(title_key) {
            continue; // duplicate title
        }
        keep_indices.push(idx);
    }

    keep_indices
}

/// Normalize a title for dedup: decode entities, strip prefixes, remove punctuation
fn normalize_title_for_dedup(title: &str) -> String {
    // Decode HTML entities first so "&amp;" == "&"
    let decoded = crate::decode_html_entities(title);

    // Strip common source prefixes
    let stripped = decoded
        .trim()
        .trim_start_matches("[HN]")
        .trim_start_matches("Show HN:")
        .trim_start_matches("Ask HN:")
        .trim_start_matches("Tell HN:")
        .trim_start_matches("Launch HN:")
        .trim_start_matches("[D]") // Reddit discussion tag
        .trim_start_matches("[R]")
        .trim_start_matches("[P]")
        .trim();

    // Keep only alphanumeric + whitespace, normalize spaces, lowercase
    stripped
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Normalize a URL for dedup: strip www, trailing slash, query params, fragments, protocol
fn normalize_url(url: &str) -> String {
    let url = url.trim();
    let base = url
        .split('#')
        .next()
        .unwrap_or(url)
        .split('?')
        .next()
        .unwrap_or(url);
    base.replace("http://", "https://")
        .replace("://www.", "://")
        .trim_end_matches('/')
        .to_lowercase()
}

/// Score all items in a full analysis pass
async fn score_items_full(
    app: &AppHandle,
    db: &crate::db::Database,
    cached_items: &[crate::db::StoredSourceItem],
) -> Result<Vec<SourceRelevance>, String> {
    // Deduplicate before scoring to avoid wasting compute on duplicates
    let keep_indices = dedup_stored_items(cached_items);
    let deduped_count = cached_items.len() - keep_indices.len();
    if deduped_count > 0 {
        info!(target: "4da::analysis", removed = deduped_count, kept = keep_indices.len(), "Cross-source deduplication");
    }
    let total_cached = keep_indices.len();

    emit_progress(
        app,
        "cache",
        0.1,
        &format!("Analyzing {} cached items (no API calls)...", total_cached),
        0,
        total_cached,
    );

    let scoring_ctx = scoring::build_scoring_context(db).await?;
    let options = scoring::ScoringOptions {
        apply_freshness: true,
        apply_signals: true,
    };

    emit_progress(
        app,
        "relevance",
        0.2,
        "Scoring cached items...",
        0,
        total_cached,
    );

    let mut results: Vec<SourceRelevance> = Vec::new();

    for (idx, &item_idx) in keep_indices.iter().enumerate() {
        let item = &cached_items[item_idx];
        if is_aborted() {
            info!(target: "4da::analysis", scored = idx, "Cached analysis aborted by user");
            return Err("Analysis cancelled".to_string());
        }

        if idx % 50 == 0 {
            let progress = 0.2 + (0.75 * (idx as f32 / total_cached as f32));
            let truncated_title: String = item.title.chars().take(30).collect();
            emit_progress(
                app,
                "relevance",
                progress,
                &format!("[{}] {}", &item.source_type, truncated_title),
                idx + 1,
                total_cached,
            );
        }

        results.push(scoring::score_item(
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
                tracing::info!(target: "4da::analysis", count = candidates.len(), "Injecting serendipity items (cached)");
                results.extend(candidates);
                scoring::sort_results(&mut results);
            }
        }
    }

    // LLM Reranking (if enabled and within daily limits)
    apply_llm_reranking(app, &mut results, &scoring_ctx).await;

    emit_progress(
        app,
        "complete",
        1.0,
        &format!("Analyzed {} cached items!", total_cached),
        results.len(),
        results.len(),
    );

    let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
    let excluded_count = results.iter().filter(|r| r.excluded).count();
    info!(target: "4da::analysis", "=== CACHE-FIRST ANALYSIS COMPLETE ===");
    info!(target: "4da::analysis",
        total = results.len(),
        relevant = relevant_count,
        excluded = excluded_count,
        "Cache analysis summary"
    );

    Ok(results)
}

/// Background analysis for scheduled monitoring
/// Silently scores new items since last analysis, emits results, sends notifications
pub(crate) async fn run_background_analysis<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &std::sync::Arc<monitoring::MonitoringState>,
) {
    let db = match get_database() {
        Ok(db) => db,
        Err(e) => {
            warn!(target: "4da::monitor", error = %e, "Background analysis: DB unavailable");
            state
                .is_checking
                .store(false, std::sync::atomic::Ordering::Relaxed);
            return;
        }
    };

    // Get last_completed_at for differential
    let last_completed_at = {
        let analysis_state = get_analysis_state();
        let guard = analysis_state.lock();
        guard.last_completed_at.clone()
    };

    // Determine items to score
    let items = if let Some(ref since) = last_completed_at {
        match db.get_items_since_timestamp(since, 500) {
            Ok(items) if !items.is_empty() => items,
            _ => {
                // No new items
                info!(target: "4da::monitor", "Background check: no new items");
                state
                    .is_checking
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                return;
            }
        }
    } else {
        // No previous analysis, do a full cache analysis
        match db.get_items_since_hours(48, 500) {
            Ok(items) if !items.is_empty() => items,
            _ => {
                state
                    .is_checking
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                return;
            }
        }
    };

    let new_count = items.len();
    info!(target: "4da::monitor", items = new_count, "Background analysis: scoring items");

    // Score items
    let scoring_ctx = match scoring::build_scoring_context(db).await {
        Ok(ctx) => ctx,
        Err(e) => {
            warn!(target: "4da::monitor", error = %e, "Background analysis: scoring context failed");
            state
                .is_checking
                .store(false, std::sync::atomic::Ordering::Relaxed);
            return;
        }
    };

    let options = scoring::ScoringOptions {
        apply_freshness: true,
        apply_signals: true,
    };

    let mut new_results: Vec<SourceRelevance> = Vec::new();
    for item in &items {
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

    scoring::sort_results(&mut new_results);

    let relevant_count = new_results
        .iter()
        .filter(|r| r.relevant && !r.excluded)
        .count();

    // Log signal_count distribution for scoring diagnostics
    {
        let mut dist = [0usize; 5]; // 0..4 signals
        let mut rel_by_sig = [0usize; 5];
        for r in &new_results {
            if let Some(ref bd) = r.score_breakdown {
                let idx = (bd.signal_count as usize).min(4);
                dist[idx] += 1;
                if r.relevant && !r.excluded {
                    rel_by_sig[idx] += 1;
                }
            }
        }
        info!(target: "4da::scoring",
            sig0 = dist[0], sig1 = dist[1], sig2 = dist[2], sig3 = dist[3], sig4 = dist[4],
            rel0 = rel_by_sig[0], rel1 = rel_by_sig[1], rel2 = rel_by_sig[2],
            rel3 = rel_by_sig[3], rel4 = rel_by_sig[4],
            "Confirmation gate distribution (total / relevant)"
        );
    }

    // Build signal summary for notifications
    let signal_summary = {
        let critical = new_results
            .iter()
            .filter(|r| r.signal_priority.as_deref() == Some("critical"))
            .count();
        let high = new_results
            .iter()
            .filter(|r| r.signal_priority.as_deref() == Some("high"))
            .count();
        let top_signal = new_results
            .iter()
            .filter(|r| r.signal_type.is_some())
            .max_by(|a, b| {
                a.top_score
                    .partial_cmp(&b.top_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|r| {
                Some((
                    r.signal_type.clone()?,
                    r.signal_action.clone().unwrap_or_default(),
                ))
            });
        monitoring::SignalSummary {
            critical_count: critical,
            high_count: high,
            top_signal,
        }
    };

    // Merge into existing results
    {
        let analysis_state = get_analysis_state();
        let mut guard = analysis_state.lock();

        if let Some(ref mut existing) = guard.results {
            let existing_ids: std::collections::HashSet<u64> =
                new_results.iter().map(|r| r.id).collect();
            existing.retain(|r| !existing_ids.contains(&r.id));
            existing.extend(new_results.clone());
            scoring::sort_results(existing);
        } else {
            guard.results = Some(new_results.clone());
        }
        guard.last_completed_at = Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
    }

    // Emit background results event to frontend (silent - no UI progress)
    let _ = app.emit("background-results", &new_results);

    // Complete the scheduled check (handles notifications)
    monitoring::complete_scheduled_check(
        app,
        state,
        relevant_count,
        new_count,
        Some(signal_summary),
    );

    // Run post-analysis innovation hooks for background analysis too
    run_post_analysis_hooks(&new_results);

    info!(target: "4da::monitor",
        new_items = new_count,
        relevant = relevant_count,
        "Background analysis complete"
    );
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

/// Get actionable signals from the latest analysis
/// Filters to items with signal classifications, sorted by priority
#[tauri::command]
pub(crate) async fn get_actionable_signals(
    priority_filter: Option<String>,
) -> Result<serde_json::Value, String> {
    let state = get_analysis_state();
    let guard = state.lock();

    let results = match &guard.results {
        Some(r) => r,
        None => return Ok(serde_json::json!({ "signals": [], "total": 0 })),
    };

    let priority_order = |p: &str| -> u8 {
        match p {
            "critical" => 4,
            "high" => 3,
            "medium" => 2,
            "low" => 1,
            _ => 0,
        }
    };

    let mut signals: Vec<serde_json::Value> = results
        .iter()
        .filter(|r| r.signal_type.is_some())
        .filter(|r| {
            if let Some(ref filter) = priority_filter {
                r.signal_priority.as_deref() == Some(filter.as_str())
            } else {
                true
            }
        })
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "title": r.title,
                "url": r.url,
                "score": r.top_score,
                "source_type": r.source_type,
                "signal_type": r.signal_type,
                "signal_priority": r.signal_priority,
                "signal_action": r.signal_action,
                "signal_triggers": r.signal_triggers,
            })
        })
        .collect();

    // Sort by priority (critical first), then by score
    signals.sort_by(|a, b| {
        let pa = priority_order(a["signal_priority"].as_str().unwrap_or(""));
        let pb = priority_order(b["signal_priority"].as_str().unwrap_or(""));
        pb.cmp(&pa).then_with(|| {
            let sa = a["score"].as_f64().unwrap_or(0.0);
            let sb = b["score"].as_f64().unwrap_or(0.0);
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    let total = signals.len();
    Ok(serde_json::json!({
        "signals": signals,
        "total": total,
    }))
}

// Settings and Context Engine commands are in settings_commands.rs
// ACE commands, PASIFA helpers, and auto-seeding are in ace_commands.rs
