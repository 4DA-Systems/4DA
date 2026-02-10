//! Analysis functions extracted from lib.rs
//!
//! Contains: run_deep_initial_scan, run_cached_analysis, get_analysis_status,
//! get_actionable_signals, and their implementation helpers.

use once_cell::sync::Lazy;
use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};

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

    // Spawn background task
    tokio::spawn(async move {
        let result = run_deep_initial_scan_impl(&app).await;

        // Update state with result
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = false;

        match result {
            Ok(results) => {
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
            }
            Err(e) => {
                error!(target: "4da::analysis", error = %e, "Deep initial scan failed");
                guard.error = Some(e.clone());
                let _ = app.emit("analysis-error", &e);
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

    // Spawn background task
    tokio::spawn(async move {
        let result = analyze_cached_content_impl(&app).await;

        // Update state with result
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = false;

        match result {
            Ok(results) => {
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
            }
            Err(e) => {
                guard.error = Some(e.clone());
                let _ = app.emit("analysis-error", &e);
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
            // No new items - return previous results with freshness re-scored
            info!(target: "4da::analysis", "No new items since last analysis, re-scoring existing for freshness");
            emit_progress(
                app,
                "cache",
                0.5,
                "No new items, refreshing scores...",
                0,
                0,
            );

            // Re-score existing items for updated freshness/affinities
            let all_items = db
                .get_items_since_hours(48, 1000)
                .map_err(|e| format!("Failed to load cached items: {}", e))?;

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
    let cached_items = db
        .get_items_since_hours(48, 1000)
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

/// Score all items in a full analysis pass
async fn score_items_full(
    app: &AppHandle,
    db: &crate::db::Database,
    cached_items: &[crate::db::StoredSourceItem],
) -> Result<Vec<SourceRelevance>, String> {
    let total_cached = cached_items.len();

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

    for (idx, item) in cached_items.iter().enumerate() {
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
