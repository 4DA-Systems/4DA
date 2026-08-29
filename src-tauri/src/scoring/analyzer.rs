// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Analysis orchestration — full scoring pipeline and post-analysis hooks.
//!
//! Contains: score_items_full (cache-first analysis) and post-analysis hooks
//! (temporal events, topic centroids, reverse mentions). All analysis paths
//! (foreground / scheduled / headless) score through `score_items_full` via
//! `analysis_status::analyze_cached_content_inner`, which persists the cycle's
//! curation verdict once (`analysis_status::persist_cycle_results`).
//!
//! ## Architectural invariant
//!
//! Every source item that reaches the user MUST pass through
//! `scoring::score_item` in this module (via `score_items_full`). New sources
//! are added by implementing the
//! `Source` trait and letting the existing DB → `get_items_tiered` →
//! `score_item` path carry them through the full PASIFA V2 pipeline.
//!
//! Do NOT construct `SourceRelevance` values directly in source adapters or
//! bypass the pipeline. The only sanctioned bypass is the concept-graph
//! serendipity injection in `analysis_deep_scan.rs`, which is explicitly
//! capped at 0.45 and marked `serendipity: true` so it cannot masquerade as a
//! scored signal.

use std::time::Instant;

use tauri::Emitter;
use tracing::{info, warn};

use crate::analysis_narration::NarrationEvent;
use crate::error::Result;
use crate::scoring;
use crate::SourceRelevance;

// ============================================================================
// Brief rejection demotion
// ============================================================================

/// Feed lookback for Brief rejection verdicts: a verdict older than a week is
/// stale context, not a standing judgment.
const BRIEF_REJECTION_LOOKBACK_DAYS: u32 = 7;

/// Load the Brief's rejection verdicts from the last 7 days. Errors degrade
/// to an empty map — a failed read must never block or distort scoring.
fn load_recent_brief_rejections(
    db: &crate::db::Database,
) -> std::collections::HashMap<i64, String> {
    match db.get_recent_brief_rejections(BRIEF_REJECTION_LOOKBACK_DAYS) {
        Ok(map) => map,
        Err(e) => {
            tracing::debug!(target: "4da::scoring", error = %e, "Brief rejections unavailable — no demotions applied");
            std::collections::HashMap::new()
        }
    }
}

// ============================================================================
// Full Scoring Pipeline
// ============================================================================

/// Score all items in a full analysis pass
pub(crate) async fn score_items_full(
    app: &tauri::AppHandle,
    db: &crate::db::Database,
    cached_items: &[crate::db::StoredSourceItem],
    silent: bool,
    llm_rerank: bool,
) -> Result<crate::analysis::analysis_cycle::ScoredBatch> {
    use std::sync::atomic::Ordering;

    let scoring_started = Instant::now();

    // Gated emitters: when `silent` (background/scheduled run), suppress
    // user-facing progress/narration so a background refresh doesn't move the
    // foreground progress bar. Call sites below are unchanged; these shadow the
    // free functions (reached via fully qualified paths inside the closures).
    let emit_progress = |app: &tauri::AppHandle,
                         stage: &str,
                         progress: f32,
                         message: &str,
                         processed: usize,
                         total: usize| {
        if !silent {
            crate::emit_progress(app, stage, progress, message, processed, total);
        }
    };
    let emit_narration = |app: &tauri::AppHandle, ev: NarrationEvent| {
        if !silent {
            crate::analysis_narration::emit_narration(app, ev);
        }
    };

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

    crate::diagnostics::log_rss("scoring:before_build_context");
    let context_started = Instant::now();
    let scoring_ctx = scoring::build_scoring_context_with_timeout(db).await?;
    info!(
        target: "4da::analysis",
        elapsed_ms = context_started.elapsed().as_millis(),
        "Scoring context build complete"
    );
    crate::diagnostics::log_rss("scoring:after_build_context");
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
    let scoring_loop_started = Instant::now();

    for (idx, &item_idx) in keep_indices.iter().enumerate() {
        let item = &cached_items[item_idx];
        if crate::get_analysis_abort().load(Ordering::SeqCst) {
            info!(target: "4da::analysis", scored = idx, "Cached analysis aborted by user");
            return Err("Analysis cancelled".into());
        }

        if idx % 200 == 0 {
            crate::diagnostics::log_rss(&format!("scoring:loop@{idx}/{total_cached}"));
        }

        if idx % 50 == 0 {
            let progress = 0.2 + (0.75 * (idx as f32 / total_cached as f32));
            let truncated_title: String = item.title.chars().take(30).collect();
            emit_progress(
                app,
                "relevance",
                progress,
                &format!("[{}] {}", item.source_type, truncated_title),
                idx + 1,
                total_cached,
            );

            // Emit partial results for progressive rendering (foreground only)
            if !silent && !results.is_empty() {
                let batch_end = results.len();
                let batch_start = batch_end.saturating_sub(50);
                if let Err(e) = app.emit("partial-results", &results[batch_start..batch_end]) {
                    tracing::warn!("Failed to emit 'partial-results': {e}");
                }
            }
        }

        let parsed_tags: Vec<String> = scoring::parse_tags_topics(item.tags.as_deref());

        results.push(scoring::score_item(
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
            Some(classifier),
        ));
    }
    info!(
        target: "4da::analysis",
        items = results.len(),
        elapsed_ms = scoring_loop_started.elapsed().as_millis(),
        "PASIFA scoring loop complete"
    );

    // ── Evaluated snapshot — the persistence boundary's second input ──────
    // Everything below this line is the batch-relative layer, and four of its
    // stages DELETE entries from `results` (cross-source dedup, fuzzy title,
    // topic, temporal clustering — 831 of 1,458 per cycle, measured on the live
    // corpus 2026-08-27). `evidence_score` is fixed at construction by
    // `score_item` and no batch stage touches it (`finalize_scores` shapes
    // `top_score` only), so this snapshot carries the FINAL evidence for every
    // item the scorer evaluated — including the ones about to be deleted.
    //
    // Without it, `persist_cycle_results` stamped only survivors and the drain
    // re-selected the rest for ever: a 500-item drain batch converted 5.
    let pre_batch: Vec<crate::analysis::analysis_cycle::EvaluatedItem> = results
        .iter()
        .map(crate::analysis::analysis_cycle::EvaluatedItem::from)
        .collect();

    // Candidate-selection instrumentation for THIS pass.
    //
    // This used to be logged as "Pre-score coverage — items not selected this
    // pass were never scored", with a `coverage_pct` of `total_cached / corpus`.
    // That claim is false and the metric was actively misleading: items not
    // selected this pass retain the score and pipeline-version stamp written by
    // an EARLIER pass. Measured against the live DB on 2026-08-14, the log
    // reported coverage_pct=9.5 / not_scored=9,171 while every one of the
    // 10,172 corpus items was in fact scored and stamped at the current
    // PIPELINE_VERSION — i.e. real coverage was 100%, not 9.5%. Anyone acting
    // on the old line would go hunting a recall crisis that does not exist.
    //
    // What this ratio genuinely measures is selector THROUGHPUT per pass. The
    // honest measure of corpus coverage is the pipeline-version histogram,
    // surfaced by `get_scoring_coverage` (triage_audit_commands.rs).
    if let Ok(corpus) = db.count_embedded_source_items() {
        let not_selected = (corpus - total_cached as i64).max(0);
        let selection_pct = if corpus > 0 {
            (total_cached as f64 / corpus as f64) * 100.0
        } else {
            0.0
        };
        info!(
            target: "4da::analysis",
            candidates_this_pass = total_cached,
            corpus_embedded = corpus,
            not_selected_this_pass = not_selected,
            selection_pct = format!("{selection_pct:.1}"),
            "Candidate selection for this pass — NOT corpus coverage; unselected items keep their score from an earlier pass"
        );
    }

    // Collect scoring telemetry
    let mut telemetry = scoring::ScoringTelemetry {
        total_scored: results.len(),
        ..Default::default()
    };
    for item in &results {
        if item.excluded {
            telemetry.excluded_count += 1;
        } else if item.relevant {
            telemetry.relevant_count += 1;
        }
        // Gate distribution from score breakdown
        if let Some(ref bd) = item.score_breakdown {
            let sig_count = (bd.signal_count as usize).min(5);
            telemetry.gate_distribution[sig_count] += 1;
        }
        // Source breakdown
        let entry = telemetry
            .source_breakdown
            .entry(item.source_type.clone())
            .or_insert((0, 0));
        entry.0 += 1;
        if item.relevant && !item.excluded {
            entry.1 += 1;
        }
    }

    crate::diagnostics::log_rss("scoring:loop_done_before_cross_encoder");
    let post_score_started = Instant::now();
    // Everything below this line is the BATCH-RELATIVE layer: it may reorder
    // and rewrite `top_score` (the rank value) but never `evidence_score`
    // (the pure score_item output, set at construction), which is what
    // persists as `relevance_score`. RankProvenance diffs `top_score` around
    // each stage so the persisted rank carries honest provenance.
    let mut rank_prov = crate::analysis::RankProvenance::begin(&results);
    crate::cross_encoder_rerank::apply_cross_encoder_reranking(&mut results, &scoring_ctx);
    rank_prov.record(&results, "ce");
    crate::diagnostics::log_rss("scoring:after_cross_encoder");

    scoring::sort_results(&mut results);
    let pre_dedup = results.len();
    scoring::dedup_results(&mut results);
    telemetry.dedup_removed = pre_dedup - results.len();
    let pre_fuzzy = results.len();
    scoring::fuzzy_dedup_results(&mut results);
    telemetry.fuzzy_dedup_removed = pre_fuzzy - results.len();
    let pre_topic = results.len();
    scoring::topic_dedup_results(&mut results);
    telemetry.topic_dedup_removed = pre_topic - results.len();
    scoring::temporal_cluster_results(&mut results);
    // Dedup/cluster stages can BOOST a surviving representative
    // (topic-corroboration) — a batch-relative move worth naming.
    rank_prov.record(&results, "corroboration");
    telemetry.domain_diversity_adjusted = scoring::apply_domain_diversity(&mut results);
    scoring::apply_source_topic_diversity(&mut results);
    scoring::apply_source_share_diversity(&mut results);
    rank_prov.record(&results, "diversity");

    // Per-source score normalization: blend raw score with source-relative
    // percentile so high-volume sources don't crowd out niche sources
    crate::source_tiers::normalize_scores_by_source(&mut results);
    rank_prov.record(&results, "percentile");
    scoring::sort_results(&mut results); // Re-sort after normalization

    // Serendipity Engine: inject anti-bubble items
    {
        let settings = crate::get_settings_manager().lock();
        let serendipity_config = &settings.get().serendipity;
        if serendipity_config.enabled {
            // In-place swap: the picks REPLACE the scorer-rejected originals
            // they were cloned from. The previous `extend` left the originals
            // behind, so every pick's id persisted twice (once relevant=false
            // / "score", once relevant=true / "serendipity") with the stored
            // verdict decided by write order — see the injector's doc.
            let injected = scoring::dedup::inject_serendipity_candidates(
                &mut results,
                serendipity_config.budget_percent,
            );
            if injected > 0 {
                telemetry.serendipity_injected = injected;
                tracing::info!(target: "4da::analysis", count = injected, "Injecting serendipity items (cached)");
                scoring::sort_results(&mut results);
            }
        }
    }

    telemetry.log_summary();
    info!(
        target: "4da::analysis",
        elapsed_ms = post_score_started.elapsed().as_millis(),
        "Post-score rerank/dedup/diversity phase complete"
    );
    crate::diagnostics::log_rss("scoring:after_dedup_diversity");

    if llm_rerank {
        // LLM Reranking (if enabled and within daily limits)
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
        let llm_started = Instant::now();
        match tokio::time::timeout(
            std::time::Duration::from_mins(2),
            crate::analysis_rerank::apply_llm_reranking(app, &mut results, &scoring_ctx),
        )
        .await
        {
            Ok(outcome) => {
                // Log what ACTUALLY happened. This previously printed
                // "LLM rerank phase complete" for every outcome including the
                // silent skips, so a rerank that never ran was indistinguishable
                // from one that did — the live app reported
                // `elapsed_ms=0` as success while the daily budget had been
                // exhausted 30 minutes earlier.
                outcome.log(llm_started.elapsed().as_millis(), "cached_full");
            }
            Err(_) => {
                warn!(target: "4da::analysis", "LLM reranking timed out after 120s, using pipeline scores only");
            }
        }
    } else {
        info!(target: "4da::analysis", "LLM rerank skipped for this run");
    }
    rank_prov.record(&results, "llm");
    crate::diagnostics::log_rss("scoring:after_llm_rerank");

    // Feed consumes the Brief's verdicts: items the narrated Brief rejected
    // in the last 7 days are demoted (excluded, not deleted) so the canonical
    // sort pushes them to the bottom — "yesterday's noise becomes tomorrow's
    // signal". Dep-grounded items are immune (see apply_brief_rejection_demotions).
    {
        let rejections = load_recent_brief_rejections(db);
        let demoted =
            crate::brief_rejections::apply_brief_rejection_demotions(&mut results, &rejections);
        if demoted > 0 {
            info!(target: "4da::analysis", demoted, "Demoted feed items rejected by the Brief");
        }
    }

    // Final top-end de-saturation on the RANK value. The cross-encoder
    // (and LLM reconciler) overwrite `top_score` AFTER score_item, so its
    // soft-ceiling no longer governs the batch-layer output — top matches land
    // near ~0.99 and tie. Re-apply the canonical cap here, downstream of every
    // score mutation, so rank_score honors the 0.95 invariant and the top stays
    // rankable. (The persisted relevance_score is `evidence_score`, which
    // already honors score_item's own ceilings.) Re-sort since values shifted
    // (stable order preserved).
    scoring::finalize_scores(&mut results);
    rank_prov.record(&results, "cap");
    rank_prov.finish(&mut results);
    scoring::sort_results(&mut results);

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
    // `results` has SHRUNK since the PASIFA loop: fuzzy-title dedup, topic-level
    // dedup and temporal clustering all remove entries. Reporting only the final
    // length next to a rejection rate invited the reading "we scored 654 items",
    // when the scorer actually saw `total_cached`. Log both, so the funnel is
    // legible and the denominator of the rejection rate is unambiguous.
    let survivors = results.len();
    info!(target: "4da::analysis",
        scored = total_cached,
        survivors,
        removed_by_dedup = total_cached.saturating_sub(survivors),
        relevant = relevant_count,
        excluded = excluded_count,
        elapsed_ms = scoring_started.elapsed().as_millis(),
        "Cache analysis summary"
    );

    // Record rejection rate for verifiable metrics.
    //
    // NOTE: `total_scored` here is the POST-dedup survivor count, not the number
    // of items the scorer evaluated — the two differ by the dedup/clustering
    // drop above. The column keeps this meaning deliberately: 179 historical
    // rows were written under it, and silently redefining the denominator would
    // make every stored `rejection_rate` incomparable with its own history.
    // Read it as "of the items that survived dedup, how many were relevant".
    if let Err(e) =
        db.record_scoring_stats("cached_full", survivors, relevant_count, excluded_count)
    {
        tracing::warn!(target: "4da::analysis", error = %e, "Failed to record scoring stats");
    }

    // Report the gap the batch layer opened, so a dedup stage that starts
    // eating the whole batch is visible rather than silent.
    let batch = crate::analysis::analysis_cycle::ScoredBatch::new(results, pre_batch);
    info!(
        target: "4da::analysis",
        evaluated = batch.evaluated.len(),
        survivors,
        dropped_by_batch_layer = batch.evaluated.len().saturating_sub(survivors),
        "Scoring pass complete — evidence + version stamped for every evaluated item"
    );

    Ok(batch)
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

        // 2. Record topic centroids for semantic drift detection + topic hotness
        let mut topic_scores: std::collections::HashMap<String, Vec<f32>> =
            std::collections::HashMap::new();
        let mut topic_titles: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for item in results.iter().filter(|r| r.relevant) {
            let topics = crate::extract_topics(&item.title, "", &[]);
            for topic in &topics {
                crate::topic_hotness::record_topic_mention(&conn, topic, &item.source_type);
            }
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

        // 3. Feed stability detector from high-scoring results (recurrence signal)
        for item in results.iter().filter(|r| r.relevant && r.top_score >= 0.6) {
            let topics = crate::extract_topics(&item.title, "", &[]);
            for topic in &topics {
                crate::stability_detector::record_evidence(
                    &conn,
                    crate::stability_detector::FacetClass::TopicAffinity,
                    topic,
                    "surfaced",
                    crate::stability_detector::CueFamily::Recurrence,
                    "analysis_surface",
                    (item.top_score as f64).min(1.0) * 0.5,
                );
            }
            crate::stability_detector::record_evidence(
                &conn,
                crate::stability_detector::FacetClass::SourcePref,
                &item.source_type,
                "producing",
                crate::stability_detector::CueFamily::Recurrence,
                "analysis_surface",
                0.3,
            );
        }

        // 4. Rebuild stability scores if enough new evidence accumulated
        crate::engagement_telemetry::rebuild_if_needed(&conn);

        info!(target: "4da::analysis", "Post-analysis hooks complete (including stability rebuild)");
    }
}

#[cfg(test)]
#[path = "analyzer_tests.rs"]
mod tests;
