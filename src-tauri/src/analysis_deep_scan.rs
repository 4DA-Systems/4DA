// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Deep scan and multi-source analysis implementations.

use tauri::AppHandle;
use tracing::{info, warn};

use crate::analysis_narration::{emit_narration, NarrationEvent};
use crate::error::Result;
use crate::scoring;

use crate::{
    analysis_rerank, emit_progress, get_database, open_db_connection, source_fetching,
    truncate_utf8, SourceRelevance,
};

use super::{is_aborted, SIGNAL_CLASSIFIER};

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
        .map_err(|e| format!("Failed to count context chunks: {e}"))?;

    if cached_context_count > 0 {
        info!(target: "4da::analysis", context_chunks = cached_context_count, "Context indexed (using KNN search)");
    } else {
        warn!(target: "4da::analysis", "No context indexed. Running without context-based scoring");
    }

    // Step 2: Fetch from all sources (50 items per source for comprehensive coverage)
    emit_progress(app, "fetch", 0.1, "Fetching from all sources...", 0, 0);
    let all_items = source_fetching::fetch_all_sources(db, app, 50)
        .await
        .map_err(|e| format!("Multi-source fetch failed: {e}"))?;
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
        .map_err(|e| format!("Failed to build scoring context: {e}"))?;
    let trend_topics = crate::detect_trend_topics(
        all_items
            .iter()
            .map(|(item, _)| (item.title.as_str(), item.content.as_str())),
    );
    let options = scoring::ScoringOptions {
        apply_freshness: false,
        apply_signals: true,
        trend_topics,
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
                detected_lang: "en",
                source_tags: &[],
                tags_json: None,
                feed_origin: item.feed_origin.as_deref(),
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
                        message: format!("High match: \"{title_preview}\""),
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
    scoring::apply_domain_diversity(&mut results);
    scoring::apply_source_topic_diversity(&mut results);

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

    // Layer 6: Concept Graph serendipity — inject items from 2-3 hop conceptual neighbors
    // This is ADDITIVE to the existing serendipity engine above. The concept graph discovers
    // content the user wouldn't find through direct interest matching by traversing
    // co-occurrence edges in the topic graph.
    {
        if let Ok(cg_conn) = open_db_connection() {
            let user_topics: Vec<String> = scoring_ctx.ace_ctx.active_topics.clone();
            if !user_topics.is_empty() {
                match crate::concept_graph::build_concept_graph(&cg_conn) {
                    Ok(graph) if !graph.is_empty() => {
                        let neighbors = crate::concept_graph::find_conceptual_neighbors(
                            &graph,
                            &user_topics,
                            3,
                        );
                        match crate::concept_graph::select_serendipity_item(&cg_conn, &neighbors) {
                            Ok(Some(item_id)) => {
                                // Load the item from DB and convert to SourceRelevance
                                if let Ok(db) = get_database() {
                                    if let Ok(Some(stored)) = db.get_source_item_by_id(item_id) {
                                        // Avoid injecting a duplicate already in results
                                        let already_present =
                                            results.iter().any(|r| r.id == stored.id as u64);
                                        if !already_present {
                                            let sr = SourceRelevance {
                                                id: stored.id as u64,
                                                title: stored.title,
                                                url: stored.url,
                                                top_score: 0.45, // moderate score — serendipity, not top-ranked
                                                matches: vec![],
                                                relevant: true,
                                                context_score: 0.0,
                                                interest_score: 0.0,
                                                excluded: false,
                                                excluded_by: None,
                                                source_type: stored.source_type,
                                                explanation: Some(
                                                    "Concept Graph: discovered via conceptual neighbors \
                                                     (2-3 hops from your interests)"
                                                        .to_string(),
                                                ),
                                                confidence: Some(0.4),
                                                score_breakdown: None,
                                                signal_type: None,
                                                signal_priority: None,
                                                signal_action: None,
                                                signal_triggers: None,
                                                signal_horizon: None,
                                                similar_count: 0,
                                                similar_titles: vec![],
                                                serendipity: true,
                                                streets_engine: None,
                                                decision_window_match: None,
                                                decision_boost_applied: 0.0,
                                                created_at: None,
                                                detected_lang: stored.detected_lang.clone(),
                                                is_critical_alert: false,
                                                applicability: None,
                                                advisory_id: None,
                                                primary_topic: None,
                                            };
                                            info!(
                                                target: "4da::analysis",
                                                item_id,
                                                title = %sr.title,
                                                "Concept graph: injecting serendipitous item"
                                            );
                                            results.push(sr);
                                            scoring::sort_results(&mut results);
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                tracing::debug!(
                                    target: "4da::analysis",
                                    "Concept graph: no suitable serendipity candidate found"
                                );
                            }
                            Err(e) => {
                                tracing::debug!(
                                    target: "4da::analysis",
                                    error = %e,
                                    "Concept graph: serendipity selection failed (non-fatal)"
                                );
                            }
                        }
                    }
                    Ok(_) => {
                        tracing::debug!(
                            target: "4da::analysis",
                            "Concept graph: empty graph (insufficient feedback data)"
                        );
                    }
                    Err(e) => {
                        tracing::debug!(
                            target: "4da::analysis",
                            error = %e,
                            "Concept graph: build failed (non-fatal)"
                        );
                    }
                }
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

    // Feed composition floors — Intelligence Mesh Gap 3. Reorders the
    // top-N items to guarantee minimum stretch + horizon representation
    // (prevents filter-bubble collapse). Pure reordering, no score
    // changes. Controlled by `settings.feed_composition.enabled`; when
    // disabled or when no stretch/horizon candidates exist, the function
    // is a no-op.
    {
        let settings = crate::get_settings_manager().lock();
        let fc = &settings.get().feed_composition;
        if fc.enabled {
            let cfg = scoring::FloorConfig {
                top_n: fc.top_n as usize,
                comfort_pct: fc.comfort_pct,
                stretch_pct: fc.stretch_pct,
                horizon_pct: fc.horizon_pct,
            };
            scoring::enforce_composition_floors(&mut results, &cfg);
        }
    }

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

    // Tier 2: queue unjudged items for LLM evaluation (non-blocking)
    if let Ok(db) = crate::get_database() {
        let db = db.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::llm_judgments::evaluate_pending_items(&db).await {
                tracing::warn!(target: "4da::llm_judgments", error = %e, "Post-scan LLM judgment failed");
            }
        });
    }

    Ok(results)
}
