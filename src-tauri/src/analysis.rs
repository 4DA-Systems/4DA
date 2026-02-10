//! Analysis functions extracted from lib.rs
//!
//! Contains: run_deep_initial_scan, run_cached_analysis, get_analysis_status,
//! get_actionable_signals, and their implementation helpers.

use once_cell::sync::Lazy;
use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};

use crate::scoring;
use crate::{
    emit_progress, get_analysis_state, get_database, get_settings_manager, monitoring, signals,
    source_fetching, truncate_utf8, void_signal_analysis_complete, void_signal_error,
    AnalysisState, SourceRelevance,
};

// Singleton SignalClassifier - created once and reused across all analyses
static SIGNAL_CLASSIFIER: Lazy<signals::SignalClassifier> =
    Lazy::new(signals::SignalClassifier::new);

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

    // Set running state
    {
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = true;
        guard.completed = false;
        guard.error = None;
        guard.results = None;
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
        apply_signals: false,
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
            None,
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
        apply_signals: false,
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
            None,
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
        let state = get_analysis_state();
        let mut guard = state.lock();
        if guard.running {
            return Err("Analysis already running".to_string());
        }
        guard.running = true;
        guard.completed = false;
        guard.error = None;
        guard.results = None;
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
/// Scores ALL cached items without any API fetching
pub(crate) async fn analyze_cached_content_impl(
    app: &AppHandle,
) -> Result<Vec<SourceRelevance>, String> {
    info!(target: "4da::analysis", "=== CACHE-FIRST ANALYSIS STARTED ===");

    emit_progress(app, "init", 0.0, "Loading cached items...", 0, 0);

    let db = get_database()?;

    // Get cached items from last 48 hours (or all recent if less)
    // This is INSTANT - no API calls
    let cached_items = db
        .get_items_since_hours(48, 1000)
        .map_err(|e| format!("Failed to load cached items: {}", e))?;

    let total_cached = cached_items.len();
    info!(target: "4da::analysis", cached_items = total_cached, "Loaded items from cache");

    if total_cached == 0 {
        // Fall back to fetching if cache is empty
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

    emit_progress(
        app,
        "cache",
        0.1,
        &format!("Analyzing {} cached items (no API calls)...", total_cached),
        0,
        total_cached,
    );

    // Load all scoring context (user interests, exclusions, ACE context)
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

    // Score all cached items through unified pipeline
    let mut results: Vec<SourceRelevance> = Vec::new();

    for (idx, item) in cached_items.iter().enumerate() {
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

/// Get current analysis state
#[tauri::command]
pub(crate) async fn get_analysis_status() -> Result<AnalysisState, String> {
    let state = get_analysis_state();
    let guard = state.lock();
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
