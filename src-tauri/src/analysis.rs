//! Analysis functions extracted from lib.rs
//!
//! Contains: start_background_analysis, run_multi_source_analysis,
//! run_deep_initial_scan, run_cached_analysis, get_analysis_status,
//! get_actionable_signals, and their implementation helpers.

use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};

use crate::llm::RelevanceJudge;
use crate::scoring;
use crate::{
    build_embedding_text, check_exclusions, embed_texts, emit_progress, extract_topics,
    get_analysis_state, get_context_engine, get_database, get_settings_manager, monitoring,
    scrape_article_content, signals, source_fetching, truncate_utf8, void_signal_analysis_complete,
    void_signal_error, AnalysisState, HNItem, HNRelevance, HNStory, RelevanceMatch, ScoreBreakdown,
    RELEVANCE_THRESHOLD,
};

/// Start background analysis - returns immediately, emits progress events
#[tauri::command]
pub(crate) async fn start_background_analysis(app: AppHandle) -> Result<(), String> {
    // Check if already running
    {
        let state = get_analysis_state();
        let guard = state.lock();
        if guard.running {
            return Err("Analysis already running".to_string());
        }
    }

    // Mark as running
    {
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = true;
        guard.completed = false;
        guard.error = None;
        guard.results = None;
    }

    // Spawn background task
    tokio::spawn(async move {
        let result = run_background_analysis(&app).await;

        // Update state with result
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = false;

        match result {
            Ok(results) => {
                guard.completed = true;
                guard.results = Some(results.clone());

                // Emit completion event
                let _ = app.emit("analysis-complete", &results);

                // Save digest if enabled
                maybe_save_digest(&results);

                // Send notification if relevant items found
                let relevant_count = results.iter().filter(|r| r.relevant).count();
                if relevant_count > 0 {
                    monitoring::send_notification(&app, relevant_count, results.len());
                }

                // Update void engine heartbeat
                void_signal_analysis_complete(&app, &results);
            }
            Err(e) => {
                guard.error = Some(e.clone());

                // Emit error event
                let _ = app.emit("analysis-error", &e);
                void_signal_error(&app);
            }
        }
    });

    Ok(())
}

/// Generate and save digest from analysis results (if enabled)
pub(crate) fn maybe_save_digest(results: &[HNRelevance]) {
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

/// The actual background analysis work
pub(crate) async fn run_background_analysis(app: &AppHandle) -> Result<Vec<HNRelevance>, String> {
    info!(target: "4da::analysis", "=== BACKGROUND ANALYSIS STARTED ===");

    emit_progress(app, "init", 0.0, "Initializing...", 0, 0);

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
        emit_progress(
            app,
            "context",
            0.1,
            &format!("{} chunks indexed (KNN enabled)", cached_context_count),
            0,
            0,
        );
    } else {
        warn!(target: "4da::analysis", "No context indexed. Running without context-based scoring");
        emit_progress(
            app,
            "context",
            0.1,
            "No context indexed - add files to context directory",
            0,
            0,
        );
    }

    // Step 2: Fetch HN story IDs
    emit_progress(app, "fetch", 0.15, "Fetching story IDs...", 0, 30);

    let client = reqwest::Client::new();
    let top_ids: Vec<u64> = client
        .get("https://hacker-news.firebaseio.com/v0/topstories.json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch top stories: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse top stories: {}", e))?;

    let total_items = 30.min(top_ids.len());
    emit_progress(
        app,
        "fetch",
        0.2,
        &format!("Processing {} stories...", total_items),
        0,
        total_items,
    );

    // Step 3: Process items incrementally with progress updates
    let mut cached_items: Vec<(HNItem, Vec<f32>)> = Vec::new();
    let mut new_items: Vec<HNItem> = Vec::new();

    for (idx, id) in top_ids.into_iter().take(total_items).enumerate() {
        let id_str = id.to_string();
        let progress = 0.2 + (0.5 * (idx as f32 / total_items as f32));

        // Check cache first
        if let Ok(Some(cached)) = db.get_source_item("hackernews", &id_str) {
            emit_progress(
                app,
                "fetch",
                progress,
                &format!("Cached: {}", &truncate_utf8(&cached.title, 35)),
                idx + 1,
                total_items,
            );
            db.touch_source_item("hackernews", &id_str).ok();
            cached_items.push((
                HNItem {
                    id,
                    title: cached.title,
                    url: cached.url,
                    content: cached.content,
                },
                cached.embedding,
            ));
        } else {
            // Fetch from API
            let url = format!("https://hacker-news.firebaseio.com/v0/item/{}.json", id);
            if let Ok(response) = client.get(&url).send().await {
                if let Ok(story) = response.json::<HNStory>().await {
                    let title = story.title.unwrap_or_else(|| "[No title]".to_string());
                    emit_progress(
                        app,
                        "fetch",
                        progress,
                        &format!("Fetching: {}", &truncate_utf8(&title, 35)),
                        idx + 1,
                        total_items,
                    );

                    let content = if let Some(text) = story.text {
                        text
                    } else if let Some(ref article_url) = story.url {
                        emit_progress(
                            app,
                            "scrape",
                            progress,
                            &format!("Scraping: {}", &truncate_utf8(&title, 35)),
                            idx + 1,
                            total_items,
                        );
                        scrape_article_content(article_url)
                            .await
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };

                    new_items.push(HNItem {
                        id: story.id,
                        title,
                        url: story.url,
                        content,
                    });
                }
            }
        }
    }

    // Step 4: Embed new items
    let new_embeddings = if !new_items.is_empty() {
        emit_progress(
            app,
            "embed",
            0.75,
            &format!("Embedding {} new items...", new_items.len()),
            cached_items.len(),
            total_items,
        );

        let new_texts: Vec<String> = new_items
            .iter()
            .map(|item| build_embedding_text(&item.title, &item.content))
            .collect();
        let embeddings = embed_texts(&new_texts)?;

        for (item, embedding) in new_items.iter().zip(embeddings.iter()) {
            db.upsert_source_item(
                "hackernews",
                &item.id.to_string(),
                item.url.as_deref(),
                &item.title,
                &item.content,
                embedding,
            )
            .ok();
        }

        embeddings
    } else {
        emit_progress(
            app,
            "embed",
            0.75,
            "All items cached!",
            total_items,
            total_items,
        );
        vec![]
    };

    db.update_source_fetch_time("hackernews").ok();

    // Combine all items
    let mut all_items_with_embeddings: Vec<(HNItem, Vec<f32>)> = cached_items;
    for (item, embedding) in new_items.into_iter().zip(new_embeddings.into_iter()) {
        all_items_with_embeddings.push((item, embedding));
    }

    // Step 5: Load user context for personalized scoring
    emit_progress(
        app,
        "relevance",
        0.82,
        "Loading user context...",
        0,
        all_items_with_embeddings.len(),
    );

    let context_engine = get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .map_err(|e| format!("Failed to load context: {}", e))?;

    let interest_count = static_identity.interests.len();
    let exclusion_count = static_identity.exclusions.len();
    debug!(target: "4da::analysis", interests = interest_count, exclusions = exclusion_count, "User context loaded");

    // Step 6: Compute personalized relevance
    emit_progress(
        app,
        "relevance",
        0.85,
        "Computing personalized relevance...",
        0,
        all_items_with_embeddings.len(),
    );

    let mut results: Vec<HNRelevance> = Vec::new();
    let mut excluded_count = 0;

    for (idx, (item, item_embedding)) in all_items_with_embeddings.iter().enumerate() {
        let progress = 0.85 + (0.10 * (idx as f32 / all_items_with_embeddings.len() as f32));

        // Extract topics for exclusion checking
        let topics = extract_topics(&item.title, &item.content);

        // Check exclusions FIRST (hard filter)
        let excluded_by = check_exclusions(&topics, &static_identity.exclusions);

        if let Some(ref exclusion) = excluded_by {
            excluded_count += 1;
            emit_progress(
                app,
                "relevance",
                progress,
                &format!(
                    "Excluded: {} ({})",
                    &truncate_utf8(&item.title, 30),
                    exclusion
                ),
                idx + 1,
                all_items_with_embeddings.len(),
            );

            results.push(HNRelevance {
                id: item.id,
                title: item.title.clone(),
                url: item.url.clone(),
                top_score: 0.0,
                matches: vec![],
                relevant: false,
                context_score: 0.0,
                interest_score: 0.0,
                excluded: true,
                excluded_by: Some(exclusion.clone()),
                source_type: "hackernews".to_string(),
                explanation: None,     // Excluded items don't need explanations
                confidence: Some(0.0), // Excluded items have zero confidence
                score_breakdown: None,
                signal_type: None,
                signal_priority: None,
                signal_action: None,
                signal_triggers: None,
            });
            continue;
        }

        emit_progress(
            app,
            "relevance",
            progress,
            &format!("Scoring: {}", &truncate_utf8(&item.title, 35)),
            idx + 1,
            all_items_with_embeddings.len(),
        );

        // Compute context file score using sqlite-vec KNN search (O(log n))
        // With graceful fallback to empty matches if KNN fails
        let matches: Vec<RelevanceMatch> = if cached_context_count > 0 {
            match db.find_similar_contexts(item_embedding, 3) {
                Ok(results) => results
                    .into_iter()
                    .map(|result| {
                        let similarity = 1.0 / (1.0 + result.distance);
                        // Safely truncate text at character boundary
                        let matched_text = if result.text.len() > 100 {
                            let truncated: String = result.text.chars().take(100).collect();
                            format!("{}...", truncated)
                        } else {
                            result.text
                        };
                        RelevanceMatch {
                            source_file: result.source_file,
                            matched_text,
                            similarity,
                        }
                    })
                    .collect(),
                Err(e) => {
                    warn!(target: "4da::knn", error = %e, "KNN search failed - using interest-only scoring");
                    vec![]
                }
            }
        } else {
            vec![]
        };

        let context_score = matches.first().map(|m| m.similarity).unwrap_or(0.0);

        // Compute interest score
        let interest_score =
            scoring::compute_interest_score(item_embedding, &static_identity.interests);

        // Combined score - adjust weights based on available data
        let combined_score = if cached_context_count > 0 && interest_count > 0 {
            context_score * 0.5 + interest_score * 0.5
        } else if interest_count > 0 {
            // No context indexed: rely on interests (full weight)
            interest_score * 0.7
        } else if cached_context_count > 0 {
            context_score
        } else {
            0.0
        };

        let relevant = combined_score >= RELEVANCE_THRESHOLD;

        // Generate simple explanation for relevant items (legacy path without ACE)
        let explanation = if relevant {
            let mut reasons: Vec<String> = Vec::new();
            if context_score > 0.2 {
                if let Some(first_match) = matches.first() {
                    let file_name = first_match
                        .source_file
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(&first_match.source_file);
                    reasons.push(format!("Matches your current work ({})", file_name));
                } else {
                    reasons.push("Relates to your active projects".to_string());
                }
            }
            if interest_score > 0.2 {
                reasons.push("Matches your declared interests".to_string());
            }
            if reasons.is_empty() {
                Some("Matches your overall profile".to_string())
            } else if reasons.len() == 1 {
                Some(reasons[0].clone())
            } else {
                Some(format!("{}; {}", reasons[0], reasons[1]))
            }
        } else {
            None
        };

        // Calculate simple confidence for legacy path (no ACE)
        let confidence = if cached_context_count > 0 && interest_count > 0 {
            (context_score + interest_score) / 2.0
        } else if interest_count > 0 {
            interest_score * 0.8
        } else if cached_context_count > 0 {
            context_score * 0.7
        } else {
            0.3
        };

        let mut confidence_by_signal = std::collections::HashMap::new();
        if cached_context_count > 0 {
            confidence_by_signal.insert("context".to_string(), context_score);
        }
        if interest_count > 0 {
            confidence_by_signal.insert("interest".to_string(), interest_score);
        }

        let score_breakdown = ScoreBreakdown {
            context_score,
            interest_score,
            ace_boost: 0.0,
            affinity_mult: 1.0,
            anti_penalty: 0.0,
            freshness_mult: 1.0,
            confidence_by_signal,
        };

        results.push(HNRelevance {
            id: item.id,
            title: item.title.clone(),
            url: item.url.clone(),
            top_score: combined_score,
            matches,
            relevant,
            context_score,
            interest_score,
            excluded: false,
            excluded_by: None,
            source_type: "hackernews".to_string(),
            explanation,
            confidence: Some(confidence),
            score_breakdown: Some(score_breakdown),
            signal_type: None,
            signal_priority: None,
            signal_action: None,
            signal_triggers: None,
        });
    }

    // Sort: excluded items last, then by score
    results.sort_by(|a, b| {
        if a.excluded && !b.excluded {
            return std::cmp::Ordering::Greater;
        }
        if !a.excluded && b.excluded {
            return std::cmp::Ordering::Less;
        }
        b.top_score
            .partial_cmp(&a.top_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if excluded_count > 0 {
        info!(target: "4da::analysis", excluded = excluded_count, "Items excluded by user preferences");
    }

    // Step 6: LLM Re-ranking (if enabled)
    let settings_manager = get_settings_manager();
    let (rerank_enabled, llm_settings, rerank_config) = {
        let mut guard = settings_manager.lock();
        let is_enabled = guard.is_rerank_enabled() && guard.within_daily_limits();
        let llm = guard.get().llm.clone();
        let rerank = guard.get().rerank.clone();
        (is_enabled, llm, rerank)
    };

    if rerank_enabled {
        emit_progress(
            app,
            "rerank",
            0.92,
            "LLM re-ranking enabled, filtering candidates...",
            0,
            0,
        );

        // Get items that pass the minimum embedding score
        let candidates: Vec<&mut HNRelevance> = results
            .iter_mut()
            .filter(|r| r.top_score >= rerank_config.min_embedding_score)
            .take(rerank_config.max_items_per_batch)
            .collect();

        let candidate_count = candidates.len();

        if candidate_count > 0 {
            info!(target: "4da::llm", candidates = candidate_count, threshold = rerank_config.min_embedding_score, "LLM Re-ranking candidates");

            emit_progress(
                app,
                "rerank",
                0.93,
                &format!("Sending {} items to LLM for re-ranking...", candidate_count),
                0,
                candidate_count,
            );

            // Build comprehensive context summary from database
            // This gives the LLM a complete picture of user's interests
            let context_summary: String = if cached_context_count > 0 {
                db.get_all_contexts()
                    .map_err(|e| format!("Failed to get contexts for LLM: {}", e))?
                    .iter()
                    .take(20) // Limit to avoid token overflow
                    .map(|c| {
                        format!(
                            "[{}]\n{}",
                            c.source_file,
                            c.text.chars().take(600).collect::<String>()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n---\n\n")
            } else {
                String::new()
            };

            // Prepare items for LLM with more context
            // Include all top matches to give LLM full picture
            let items_for_llm: Vec<(String, String, String)> = candidates
                .iter()
                .map(|r| {
                    // Combine all match texts for richer context
                    let content_snippet = r
                        .matches
                        .iter()
                        .map(|m| format!("Matched '{}': {}", m.source_file, m.matched_text))
                        .collect::<Vec<_>>()
                        .join(" | ");
                    (r.id.to_string(), r.title.clone(), content_snippet)
                })
                .collect();

            // Create judge and run
            let judge = RelevanceJudge::new(llm_settings);

            match judge.judge_batch(&context_summary, items_for_llm).await {
                Ok((judgments, input_tokens, output_tokens)) => {
                    let cost_cents = judge.estimate_cost_cents(input_tokens, output_tokens);

                    info!(target: "4da::llm",
                        judgments = judgments.len(),
                        tokens = input_tokens + output_tokens,
                        cost_cents = cost_cents,
                        "LLM Re-ranking complete"
                    );

                    // Record usage
                    {
                        let mut guard = settings_manager.lock();
                        guard.record_usage(input_tokens + output_tokens, cost_cents);
                    }

                    // Apply LLM judgments with confidence threshold
                    // Only demote if LLM is confident (>= 0.7) to avoid over-filtering
                    const DEMOTION_CONFIDENCE_THRESHOLD: f32 = 0.7;
                    let mut llm_relevant_count = 0;
                    let mut demoted_count = 0;
                    let mut kept_by_low_confidence = 0;

                    let mut no_match_count = 0;
                    for result in results.iter_mut() {
                        let result_id_str = result.id.to_string();
                        if let Some(judgment) =
                            judgments.iter().find(|j| j.item_id == result_id_str)
                        {
                            if judgment.relevant {
                                // LLM confirms relevance
                                llm_relevant_count += 1;
                                debug!(target: "4da::llm", title = %&truncate_utf8(&result.title, 40), confidence = judgment.confidence, "LLM confirmed");
                            } else if result.relevant {
                                // LLM says not relevant - check confidence before demoting
                                if judgment.confidence >= DEMOTION_CONFIDENCE_THRESHOLD {
                                    debug!(target: "4da::llm",
                                        title = %&truncate_utf8(&result.title, 35),
                                        confidence = judgment.confidence,
                                        reason = %&truncate_utf8(&judgment.reasoning, 50),
                                        "LLM demoted"
                                    );
                                    result.relevant = false;
                                    demoted_count += 1;
                                } else {
                                    // Low confidence - keep as relevant (benefit of doubt)
                                    debug!(target: "4da::llm", title = %&truncate_utf8(&result.title, 40), confidence = judgment.confidence, "LLM uncertain, keeping");
                                    llm_relevant_count += 1;
                                    kept_by_low_confidence += 1;
                                }
                            }
                        } else if result.relevant {
                            // No matching judgment found - item keeps embedding relevance
                            no_match_count += 1;
                            if no_match_count <= 3 {
                                debug!(target: "4da::llm", title = %&truncate_utf8(&result.title, 40), id = %result_id_str, "No LLM judgment");
                            }
                        }
                    }

                    if no_match_count > 0 {
                        warn!(target: "4da::llm", count = no_match_count, "Items had no matching LLM judgment");
                    }

                    info!(target: "4da::llm",
                        confirmed = llm_relevant_count - kept_by_low_confidence,
                        demoted = demoted_count,
                        kept_low_confidence = kept_by_low_confidence,
                        "LLM summary"
                    );

                    emit_progress(
                        app,
                        "rerank",
                        0.98,
                        &format!(
                            "LLM kept {} of {} as relevant",
                            llm_relevant_count, candidate_count
                        ),
                        candidate_count,
                        candidate_count,
                    );
                }
                Err(e) => {
                    warn!(target: "4da::llm", error = %e, "LLM Re-ranking failed, using embedding scores only");
                    emit_progress(
                        app,
                        "rerank",
                        0.98,
                        "LLM re-ranking failed, using embeddings only",
                        0,
                        0,
                    );
                }
            }
        } else {
            debug!(target: "4da::llm", "LLM Re-ranking: No candidates above threshold, skipping");
            emit_progress(
                app,
                "rerank",
                0.98,
                "No candidates for LLM re-ranking",
                0,
                0,
            );
        }
    } else {
        debug!(target: "4da::llm", "LLM Re-ranking: Disabled or limit reached");
    }

    emit_progress(
        app,
        "complete",
        1.0,
        "Analysis complete!",
        results.len(),
        results.len(),
    );

    let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
    let final_excluded = results.iter().filter(|r| r.excluded).count();
    info!(target: "4da::analysis", "=== PERSONALIZED ANALYSIS COMPLETE ===");
    info!(target: "4da::analysis",
        total = results.len(),
        relevant = relevant_count,
        excluded = final_excluded,
        interests = interest_count,
        exclusions = exclusion_count,
        "Analysis summary"
    );

    Ok(results)
}

/// Multi-source analysis - fetches from all enabled sources (HN, arXiv, Reddit)
#[tauri::command]
pub(crate) async fn run_multi_source_analysis(app: AppHandle) -> Result<(), String> {
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

    // Spawn background task
    tokio::spawn(async move {
        let result = run_multi_source_analysis_impl(&app).await;

        // Update state with result
        let state = get_analysis_state();
        let mut guard = state.lock();
        guard.running = false;

        match result {
            Ok(results) => {
                guard.completed = true;
                guard.results = Some(results.clone());
                let _ = app.emit("analysis-complete", &results);

                // Save digest if enabled
                maybe_save_digest(&results);

                // Send notification if relevant items found
                let relevant_count = results.iter().filter(|r| r.relevant).count();
                if relevant_count > 0 {
                    monitoring::send_notification(&app, relevant_count, results.len());
                }
            }
            Err(e) => {
                guard.error = Some(e.clone());
                let _ = app.emit("analysis-error", &e);
            }
        }
    });

    Ok(())
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
                let _ = app.emit("analysis-complete", &results);

                // Save digest
                maybe_save_digest(&results);

                // Send notification
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
) -> Result<Vec<HNRelevance>, String> {
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

    // Step 3: Load user context
    let context_engine = get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .map_err(|e| format!("Failed to load context: {}", e))?;

    let interest_count = static_identity.interests.len();

    // Step 4: Load ACE context
    let ace_ctx = scoring::get_ace_context();
    let topic_embeddings = scoring::get_topic_embeddings(&ace_ctx);
    info!(target: "4da::ace",
        topics = ace_ctx.active_topics.len(),
        tech = ace_ctx.detected_tech.len(),
        embeddings = topic_embeddings.len(),
        "ACE context loaded for scoring"
    );

    // Step 5: Compute relevance for all items
    emit_progress(
        app,
        "relevance",
        0.60,
        &format!("Scoring {} items...", all_items.len()),
        0,
        all_items.len(),
    );

    let mut results: Vec<HNRelevance> = Vec::new();
    let mut excluded_count = 0;
    let total_items = all_items.len();

    for (idx, (item, item_embedding)) in all_items.iter().enumerate() {
        // Progress feedback every 50 items
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

        let topics = extract_topics(&item.title, &item.content);

        // Check exclusions
        let excluded_by = check_exclusions(&topics, &static_identity.exclusions)
            .or_else(|| scoring::check_ace_exclusions(&topics, &ace_ctx));

        if let Some(ref exclusion) = excluded_by {
            excluded_count += 1;
            results.push(HNRelevance {
                id: item.id,
                title: item.title.clone(),
                url: item.url.clone(),
                top_score: 0.0,
                matches: vec![],
                relevant: false,
                context_score: 0.0,
                interest_score: 0.0,
                excluded: true,
                excluded_by: Some(exclusion.clone()),
                source_type: item.source_type.clone(),
                explanation: None,
                confidence: Some(0.0),
                score_breakdown: None,
                signal_type: None,
                signal_priority: None,
                signal_action: None,
                signal_triggers: None,
            });
            continue;
        }

        // Compute context score using KNN
        let matches: Vec<RelevanceMatch> = if cached_context_count > 0 {
            db.find_similar_contexts(item_embedding, 3)
                .unwrap_or_default()
                .into_iter()
                .map(|result| {
                    let similarity = 1.0 / (1.0 + result.distance);
                    let matched_text = if result.text.len() > 100 {
                        let truncated: String = result.text.chars().take(100).collect();
                        format!("{}...", truncated)
                    } else {
                        result.text
                    };
                    RelevanceMatch {
                        source_file: result.source_file,
                        matched_text,
                        similarity,
                    }
                })
                .collect()
        } else {
            vec![]
        };

        let context_score = matches.first().map(|m| m.similarity).unwrap_or(0.0);
        let interest_score =
            scoring::compute_interest_score(item_embedding, &static_identity.interests);

        // PASIFA: Compute semantic boost for topic/tech matching
        let semantic_boost =
            scoring::compute_semantic_ace_boost(item_embedding, &ace_ctx, &topic_embeddings)
                .unwrap_or_else(|| {
                    // Fallback to keyword matching
                    let mut boost: f32 = 0.0;
                    for topic in &topics {
                        let topic_lower = topic.to_lowercase();
                        for active in &ace_ctx.active_topics {
                            if topic_lower.contains(active) || active.contains(&topic_lower) {
                                boost += 0.15
                                    * ace_ctx.topic_confidence.get(active).copied().unwrap_or(0.5);
                                break;
                            }
                        }
                        for tech in &ace_ctx.detected_tech {
                            if topic_lower.contains(tech) || tech.contains(&topic_lower) {
                                boost += 0.12;
                                break;
                            }
                        }
                    }
                    boost.clamp(0.0, 0.3)
                });

        // Compute base score
        let base_score = if cached_context_count > 0 && interest_count > 0 {
            (context_score * 0.5 + interest_score * 0.5 + semantic_boost).min(1.0)
        } else if interest_count > 0 {
            (interest_score * 0.7 + semantic_boost * 1.5).min(1.0)
        } else if cached_context_count > 0 {
            (context_score + semantic_boost).min(1.0)
        } else {
            (semantic_boost * 2.0).min(1.0)
        };

        // Apply unified scoring
        let combined_score = scoring::compute_unified_relevance(base_score, &topics, &ace_ctx);
        let relevant = combined_score >= RELEVANCE_THRESHOLD;

        let affinity_mult = scoring::compute_affinity_multiplier(&topics, &ace_ctx);
        let anti_penalty = scoring::compute_anti_penalty(&topics, &ace_ctx);

        // Generate explanation
        let explanation = if relevant || combined_score >= 0.3 {
            Some(scoring::generate_relevance_explanation(
                &item.title,
                context_score,
                interest_score,
                &matches,
                &ace_ctx,
                &topics,
            ))
        } else {
            None
        };

        // Calculate confidence
        let confidence = scoring::calculate_confidence(
            context_score,
            interest_score,
            semantic_boost,
            &ace_ctx,
            &topics,
            cached_context_count,
            interest_count as i64,
        );

        let mut confidence_by_signal = std::collections::HashMap::new();
        if cached_context_count > 0 {
            confidence_by_signal.insert("context".to_string(), context_score);
        }
        if interest_count > 0 {
            confidence_by_signal.insert("interest".to_string(), interest_score);
        }
        if semantic_boost > 0.0 {
            confidence_by_signal.insert("ace_boost".to_string(), semantic_boost);
        }

        let breakdown = ScoreBreakdown {
            context_score,
            interest_score,
            ace_boost: semantic_boost,
            affinity_mult,
            anti_penalty,
            freshness_mult: 1.0,
            confidence_by_signal,
        };

        results.push(HNRelevance {
            id: item.id,
            title: item.title.clone(),
            url: item.url.clone(),
            top_score: combined_score,
            matches,
            relevant,
            context_score,
            interest_score,
            excluded: false,
            excluded_by: None,
            source_type: item.source_type.clone(),
            explanation,
            confidence: Some(confidence),
            score_breakdown: Some(breakdown),
            signal_type: None,
            signal_priority: None,
            signal_action: None,
            signal_triggers: None,
        });
    }

    // Sort by score
    results.sort_by(|a, b| {
        b.top_score
            .partial_cmp(&a.top_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

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
) -> Result<Vec<HNRelevance>, String> {
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

    // Step 3: Load user context
    emit_progress(
        app,
        "relevance",
        0.7,
        "Loading user context...",
        0,
        all_items.len(),
    );
    let context_engine = get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .map_err(|e| format!("Failed to load context: {}", e))?;

    let interest_count = static_identity.interests.len();
    let _exclusion_count = static_identity.exclusions.len();

    // Step 4: Load ACE context
    let ace_ctx = scoring::get_ace_context();
    // PASIFA: Pre-compute topic embeddings for semantic matching
    let topic_embeddings = scoring::get_topic_embeddings(&ace_ctx);
    info!(target: "4da::ace",
        topics = ace_ctx.active_topics.len(),
        tech = ace_ctx.detected_tech.len(),
        anti_topics = ace_ctx.anti_topics.len(),
        embeddings = topic_embeddings.len(),
        "ACE context loaded"
    );

    // Step 5: Compute relevance
    emit_progress(
        app,
        "relevance",
        0.75,
        "Computing relevance...",
        0,
        all_items.len(),
    );
    let mut results: Vec<HNRelevance> = Vec::new();
    let mut excluded_count = 0;

    for (idx, (item, item_embedding)) in all_items.iter().enumerate() {
        let progress = 0.75 + (0.20 * (idx as f32 / all_items.len() as f32));
        let topics = extract_topics(&item.title, &item.content);

        // Check exclusions
        let excluded_by = check_exclusions(&topics, &static_identity.exclusions)
            .or_else(|| scoring::check_ace_exclusions(&topics, &ace_ctx));

        if let Some(ref exclusion) = excluded_by {
            excluded_count += 1;
            results.push(HNRelevance {
                id: item.id,
                title: item.title.clone(),
                url: item.url.clone(),
                top_score: 0.0,
                matches: vec![],
                relevant: false,
                context_score: 0.0,
                interest_score: 0.0,
                excluded: true,
                excluded_by: Some(exclusion.clone()),
                source_type: item.source_type.clone(),
                explanation: None,     // Excluded items don't need explanations
                confidence: Some(0.0), // Excluded items have zero confidence
                score_breakdown: None,
                signal_type: None,
                signal_priority: None,
                signal_action: None,
                signal_triggers: None,
            });
            continue;
        }

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

        // Compute context file score using sqlite-vec KNN search (O(log n))
        // With graceful fallback to empty matches if KNN fails
        let matches: Vec<RelevanceMatch> = if cached_context_count > 0 {
            match db.find_similar_contexts(item_embedding, 3) {
                Ok(results) => results
                    .into_iter()
                    .map(|result| {
                        let similarity = 1.0 / (1.0 + result.distance);
                        // Safely truncate text at character boundary
                        let matched_text = if result.text.len() > 100 {
                            let truncated: String = result.text.chars().take(100).collect();
                            format!("{}...", truncated)
                        } else {
                            result.text
                        };
                        RelevanceMatch {
                            source_file: result.source_file,
                            matched_text,
                            similarity,
                        }
                    })
                    .collect(),
                Err(e) => {
                    warn!(target: "4da::knn", error = %e, "KNN search failed - using interest-only scoring");
                    vec![]
                }
            }
        } else {
            vec![]
        };

        let context_score = matches.first().map(|m| m.similarity).unwrap_or(0.0);
        let interest_score =
            scoring::compute_interest_score(item_embedding, &static_identity.interests);

        // PASIFA: Compute semantic boost for topic/tech matching
        let semantic_boost =
            scoring::compute_semantic_ace_boost(item_embedding, &ace_ctx, &topic_embeddings)
                .unwrap_or_else(|| {
                    // Fall back to keyword matching for active topics and tech
                    let mut boost: f32 = 0.0;
                    for topic in &topics {
                        let topic_lower = topic.to_lowercase();
                        for active_topic in &ace_ctx.active_topics {
                            if topic_lower.contains(active_topic)
                                || active_topic.contains(&topic_lower)
                            {
                                let conf = ace_ctx
                                    .topic_confidence
                                    .get(active_topic)
                                    .copied()
                                    .unwrap_or(0.5);
                                boost += 0.15 * conf;
                                break;
                            }
                        }
                        for tech in &ace_ctx.detected_tech {
                            if topic_lower.contains(tech) || tech.contains(&topic_lower) {
                                boost += 0.12;
                                break;
                            }
                        }
                    }
                    boost.clamp(0.0, 0.3)
                });

        // Compute base score - adjust weights based on available data
        let base_score = if cached_context_count > 0 && interest_count > 0 {
            // Full mode: 50% context + 50% interests + ACE boost
            (context_score * 0.5 + interest_score * 0.5 + semantic_boost).min(1.0)
        } else if interest_count > 0 {
            // No context indexed: rely on interests + ACE boost (full weight)
            (interest_score * 0.7 + semantic_boost * 1.5).min(1.0)
        } else if cached_context_count > 0 {
            // No interests: rely on context + ACE boost
            (context_score + semantic_boost).min(1.0)
        } else {
            // Neither context nor interests: pure ACE topic matching
            (semantic_boost * 2.0).min(1.0)
        };

        // PASIFA: Apply unified multiplicative scoring
        let combined_score = scoring::compute_unified_relevance(base_score, &topics, &ace_ctx);
        let relevant = combined_score >= RELEVANCE_THRESHOLD;

        // Compute debug info
        let affinity_mult = scoring::compute_affinity_multiplier(&topics, &ace_ctx);
        let anti_penalty = scoring::compute_anti_penalty(&topics, &ace_ctx);

        // Generate explanation for relevant items
        let explanation = if relevant {
            Some(scoring::generate_relevance_explanation(
                &item.title,
                context_score,
                interest_score,
                &matches,
                &ace_ctx,
                &topics,
            ))
        } else {
            None
        };

        // Calculate confidence and score breakdown
        let confidence = scoring::calculate_confidence(
            context_score,
            interest_score,
            semantic_boost,
            &ace_ctx,
            &topics,
            cached_context_count,
            interest_count as i64,
        );

        let mut confidence_by_signal = std::collections::HashMap::new();
        if cached_context_count > 0 {
            confidence_by_signal.insert("context".to_string(), context_score);
        }
        if interest_count > 0 {
            confidence_by_signal.insert("interest".to_string(), interest_score);
        }
        if semantic_boost > 0.0 {
            confidence_by_signal.insert("ace_boost".to_string(), semantic_boost);
        }

        let score_breakdown = ScoreBreakdown {
            context_score,
            interest_score,
            ace_boost: semantic_boost,
            affinity_mult,
            anti_penalty,
            freshness_mult: 1.0,
            confidence_by_signal,
        };

        results.push(HNRelevance {
            id: item.id,
            title: item.title.clone(),
            url: item.url.clone(),
            top_score: combined_score,
            matches,
            relevant,
            context_score,
            interest_score,
            excluded: false,
            excluded_by: None,
            source_type: item.source_type.clone(),
            explanation,
            confidence: Some(confidence),
            score_breakdown: Some(score_breakdown),
            signal_type: None,
            signal_priority: None,
            signal_action: None,
            signal_triggers: None,
        });
    }

    // Sort results
    results.sort_by(|a, b| {
        if a.excluded && !b.excluded {
            return std::cmp::Ordering::Greater;
        }
        if !a.excluded && b.excluded {
            return std::cmp::Ordering::Less;
        }
        b.top_score
            .partial_cmp(&a.top_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    emit_progress(
        app,
        "complete",
        1.0,
        "Multi-source analysis complete!",
        results.len(),
        results.len(),
    );

    let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
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
    // Check if already running
    {
        let state = get_analysis_state();
        let guard = state.lock();
        if guard.running {
            return Err("Analysis already running".to_string());
        }
    }

    // Mark as running
    {
        let state = get_analysis_state();
        let mut guard = state.lock();
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

                // Emit completion event
                let _ = app.emit("analysis-complete", &results);

                // Save digest if enabled
                maybe_save_digest(&results);

                // Send notification if relevant items found
                let relevant_count = results.iter().filter(|r| r.relevant).count();
                if relevant_count > 0 {
                    monitoring::send_notification(&app, relevant_count, results.len());
                }

                // Update void engine heartbeat
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
) -> Result<Vec<HNRelevance>, String> {
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

    // Load context
    let cached_context_count = db.context_count().map_err(|e| e.to_string())?;
    info!(target: "4da::analysis", context_chunks = cached_context_count, "Context loaded");

    // Load user interests
    let context_engine = get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .map_err(|e| format!("Failed to load context: {}", e))?;
    let interest_count = static_identity.interests.len();

    // Load ACE context
    let ace_ctx = scoring::get_ace_context();
    let topic_embeddings = scoring::get_topic_embeddings(&ace_ctx);
    info!(target: "4da::ace",
        topics = ace_ctx.active_topics.len(),
        tech = ace_ctx.detected_tech.len(),
        "ACE context loaded"
    );

    emit_progress(
        app,
        "relevance",
        0.2,
        "Scoring cached items...",
        0,
        total_cached,
    );

    // Score all cached items
    let signal_classifier = signals::SignalClassifier::new();
    let mut results: Vec<HNRelevance> = Vec::new();
    let mut excluded_count = 0;

    for (idx, item) in cached_items.iter().enumerate() {
        let progress = 0.2 + (0.75 * (idx as f32 / total_cached as f32));
        let topics = extract_topics(&item.title, &item.content);

        // Check exclusions
        let excluded_by = check_exclusions(&topics, &static_identity.exclusions)
            .or_else(|| scoring::check_ace_exclusions(&topics, &ace_ctx));

        if let Some(ref exclusion) = excluded_by {
            excluded_count += 1;
            results.push(HNRelevance {
                id: item.id as u64,
                title: item.title.clone(),
                url: item.url.clone(),
                top_score: 0.0,
                matches: vec![],
                relevant: false,
                context_score: 0.0,
                interest_score: 0.0,
                excluded: true,
                excluded_by: Some(exclusion.clone()),
                source_type: item.source_type.clone(),
                explanation: None,
                confidence: Some(0.0),
                score_breakdown: None,
                signal_type: None,
                signal_priority: None,
                signal_action: None,
                signal_triggers: None,
            });
            continue;
        }

        if idx % 50 == 0 {
            // Truncate title safely (UTF-8 aware)
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

        // Compute context file score using KNN
        let item_embedding = &item.embedding;
        let matches: Vec<RelevanceMatch> = if cached_context_count > 0 && !item_embedding.is_empty()
        {
            match db.find_similar_contexts(item_embedding, 3) {
                Ok(results) => results
                    .into_iter()
                    .map(|result| {
                        let similarity = 1.0 / (1.0 + result.distance);
                        let matched_text = if result.text.len() > 100 {
                            let truncated: String = result.text.chars().take(100).collect();
                            format!("{}...", truncated)
                        } else {
                            result.text
                        };
                        RelevanceMatch {
                            source_file: result.source_file,
                            matched_text,
                            similarity,
                        }
                    })
                    .collect(),
                Err(_) => vec![],
            }
        } else {
            vec![]
        };

        let context_score = matches.first().map(|m| m.similarity).unwrap_or(0.0);
        let interest_score =
            scoring::compute_interest_score(item_embedding, &static_identity.interests);

        // PASIFA semantic boost
        let semantic_boost =
            scoring::compute_semantic_ace_boost(item_embedding, &ace_ctx, &topic_embeddings)
                .unwrap_or_else(|| {
                    let mut boost: f32 = 0.0;
                    for topic in &topics {
                        let topic_lower = topic.to_lowercase();
                        for active_topic in &ace_ctx.active_topics {
                            if topic_lower.contains(active_topic)
                                || active_topic.contains(&topic_lower)
                            {
                                let conf = ace_ctx
                                    .topic_confidence
                                    .get(active_topic)
                                    .copied()
                                    .unwrap_or(0.5);
                                boost += 0.15 * conf;
                                break;
                            }
                        }
                        for tech in &ace_ctx.detected_tech {
                            if topic_lower.contains(tech) || tech.contains(&topic_lower) {
                                boost += 0.12;
                                break;
                            }
                        }
                    }
                    boost.clamp(0.0, 0.3)
                });

        // Compute base score
        let base_score = if cached_context_count > 0 && interest_count > 0 {
            (context_score * 0.5 + interest_score * 0.5 + semantic_boost).min(1.0)
        } else if interest_count > 0 {
            (interest_score * 0.7 + semantic_boost * 1.5).min(1.0)
        } else if cached_context_count > 0 {
            (context_score + semantic_boost).min(1.0)
        } else {
            (semantic_boost * 2.0).min(1.0)
        };

        // Apply temporal freshness: recent items get slight boost, older items decay
        let freshness = scoring::compute_temporal_freshness(&item.created_at);
        let base_score = (base_score * freshness).clamp(0.0, 1.0);

        let combined_score = scoring::compute_unified_relevance(base_score, &topics, &ace_ctx);
        let relevant = combined_score >= RELEVANCE_THRESHOLD;

        let affinity_mult = scoring::compute_affinity_multiplier(&topics, &ace_ctx);
        let anti_penalty = scoring::compute_anti_penalty(&topics, &ace_ctx);

        let explanation = if relevant {
            Some(scoring::generate_relevance_explanation(
                &item.title,
                context_score,
                interest_score,
                &matches,
                &ace_ctx,
                &topics,
            ))
        } else {
            None
        };

        let confidence = scoring::calculate_confidence(
            context_score,
            interest_score,
            semantic_boost,
            &ace_ctx,
            &topics,
            cached_context_count,
            interest_count as i64,
        );

        let mut confidence_by_signal = std::collections::HashMap::new();
        if cached_context_count > 0 {
            confidence_by_signal.insert("context".to_string(), context_score);
        }
        if interest_count > 0 {
            confidence_by_signal.insert("interest".to_string(), interest_score);
        }
        if semantic_boost > 0.0 {
            confidence_by_signal.insert("ace_boost".to_string(), semantic_boost);
        }

        let score_breakdown = ScoreBreakdown {
            context_score,
            interest_score,
            ace_boost: semantic_boost,
            affinity_mult,
            anti_penalty,
            freshness_mult: freshness,
            confidence_by_signal,
        };

        // Signal classification
        let content_text = &item.content;
        let classification = signal_classifier.classify(
            &item.title,
            content_text,
            combined_score,
            &ace_ctx.detected_tech,
        );

        let (sig_type, sig_priority, sig_action, sig_triggers) = match classification {
            Some(c) => (
                Some(c.signal_type.slug().to_string()),
                Some(c.priority.label().to_string()),
                Some(c.action),
                Some(c.triggers),
            ),
            None => (None, None, None, None),
        };

        results.push(HNRelevance {
            id: item.id as u64,
            title: item.title.clone(),
            url: item.url.clone(),
            top_score: combined_score,
            matches,
            relevant,
            context_score,
            interest_score,
            excluded: false,
            excluded_by: None,
            source_type: item.source_type.clone(),
            explanation,
            confidence: Some(confidence),
            score_breakdown: Some(score_breakdown),
            signal_type: sig_type,
            signal_priority: sig_priority,
            signal_action: sig_action,
            signal_triggers: sig_triggers,
        });
    }

    // Sort by relevance
    results.sort_by(|a, b| {
        if a.excluded && !b.excluded {
            return std::cmp::Ordering::Greater;
        }
        if !a.excluded && b.excluded {
            return std::cmp::Ordering::Less;
        }
        b.top_score
            .partial_cmp(&a.top_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    emit_progress(
        app,
        "complete",
        1.0,
        &format!("Analyzed {} cached items!", total_cached),
        results.len(),
        results.len(),
    );

    let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
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
