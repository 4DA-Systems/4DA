//! Analysis orchestration — full scoring pipeline and background analysis.
//!
//! Contains: score_items_full (cache-first analysis), run_background_analysis (scheduled),
//! and post-analysis hooks (temporal events, topic centroids, reverse mentions).

use tauri::Emitter;
use tracing::{info, warn};

use crate::analysis_narration::{emit_narration, NarrationEvent};
use crate::error::Result;
use crate::scoring;
use crate::{emit_progress, get_analysis_state, get_database, monitoring, SourceRelevance};

// ============================================================================
// Full Scoring Pipeline
// ============================================================================

/// Score all items in a full analysis pass
pub(crate) async fn score_items_full(
    app: &tauri::AppHandle,
    db: &crate::db::Database,
    cached_items: &[crate::db::StoredSourceItem],
) -> Result<Vec<SourceRelevance>> {
    use std::sync::atomic::Ordering;

    // Deduplicate before scoring to avoid wasting compute on duplicates
    let keep_indices = crate::analysis_rerank::dedup_stored_items(cached_items);
    let deduped_count = cached_items.len() - keep_indices.len();
    if deduped_count > 0 {
        info!(target: "4da::analysis", removed = deduped_count, kept = keep_indices.len(), "Cross-source deduplication");
    }
    let total_cached = keep_indices.len();

    emit_progress(
        app,
        "cache",
        0.1,
        &format!("Analyzing {total_cached} cached items (no API calls)..."),
        0,
        total_cached,
    );

    let scoring_ctx = scoring::build_scoring_context(db).await?;
    let trend_topics = crate::detect_trend_topics(keep_indices.iter().map(|&i| {
        (
            cached_items[i].title.as_str(),
            cached_items[i].content.as_str(),
        )
    }));
    let options = scoring::ScoringOptions {
        apply_freshness: true,
        apply_signals: true,
        trend_topics,
    };

    emit_progress(
        app,
        "relevance",
        0.2,
        "Scoring cached items...",
        0,
        total_cached,
    );

    let classifier = crate::analysis::signal_classifier();
    let mut results: Vec<SourceRelevance> = Vec::new();

    for (idx, &item_idx) in keep_indices.iter().enumerate() {
        let item = &cached_items[item_idx];
        if crate::get_analysis_abort().load(Ordering::SeqCst) {
            info!(target: "4da::analysis", scored = idx, "Cached analysis aborted by user");
            return Err("Analysis cancelled".into());
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
            Some(classifier),
        ));
    }

    scoring::sort_results(&mut results);
    scoring::dedup_results(&mut results);
    scoring::topic_dedup_results(&mut results);
    scoring::temporal_cluster_results(&mut results);

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
    emit_narration(
        app,
        NarrationEvent {
            narration_type: "insight".into(),
            message: "Ranking items against your profile...".into(),
            source: None,
            relevance: None,
        },
    );
    crate::analysis_rerank::apply_llm_reranking(app, &mut results, &scoring_ctx).await;

    emit_progress(
        app,
        "complete",
        1.0,
        &format!("Analyzed {total_cached} cached items!"),
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

    // Record rejection rate for verifiable metrics
    if let Err(e) =
        db.record_scoring_stats("cached_full", results.len(), relevant_count, excluded_count)
    {
        tracing::warn!(target: "4da::analysis", error = %e, "Failed to record scoring stats");
    }

    Ok(results)
}

// ============================================================================
// Background Analysis
// ============================================================================

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

    // Determine items to score (respects free-tier 30-day history gate)
    let items = if let Some(ref since) = last_completed_at {
        match db.get_items_since_timestamp_tiered(since, 500) {
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
        match db.get_items_tiered(48, 500) {
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

    let trend_topics = crate::detect_trend_topics(
        items
            .iter()
            .map(|item| (item.title.as_str(), item.content.as_str())),
    );
    let options = scoring::ScoringOptions {
        apply_freshness: true,
        apply_signals: true,
        trend_topics,
    };

    // Use the singleton classifier from analysis module
    let classifier = crate::analysis::signal_classifier();

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
            Some(classifier),
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
            top_item_id: None,
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
            guard.near_misses = crate::types::extract_near_misses(existing);
        } else {
            guard.near_misses = crate::types::extract_near_misses(&new_results);
            guard.results = Some(new_results.clone());
        }
        guard.last_completed_at = Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());
    }

    // Emit background results event to frontend (silent - no UI progress)
    if let Err(e) = app.emit("background-results", &new_results) {
        tracing::warn!("Failed to emit 'background-results': {e}");
    }

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

// ============================================================================
// Post-Analysis Hooks
// ============================================================================

/// Run post-analysis innovation hooks: temporal events, topic centroids, reverse mentions.
/// These populate temporal data needed by SignalChains, KnowledgeGaps, etc.
pub(crate) fn run_post_analysis_hooks(results: &[SourceRelevance]) {
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

        info!(target: "4da::analysis", "Post-analysis innovation hooks complete");
    }
}

#[cfg(test)]
#[path = "analyzer_tests.rs"]
mod tests;
