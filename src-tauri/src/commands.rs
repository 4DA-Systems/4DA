// Copyright (c) 2025-2026 4DA Systems. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1). See LICENSE file.

use tracing::{debug, info, warn};

use crate::{
    anomaly, build_embedding_text, check_exclusions, decode_html_entities, developer_dna,
    embed_texts, extract_topics, get_ace_engine, get_analysis_state, get_context_engine,
    get_database, get_db_path, get_relevance_threshold, get_source_registry, health, scoring,
    scrape_article_content, set_relevance_threshold, truncate_utf8, FetchedItem, HNStory,
    RelevanceMatch, ScoreBreakdown, SourceRelevance,
};

// ============================================================================
// Commands
// ============================================================================

#[tauri::command]
pub(crate) async fn get_hn_top_stories() -> Result<Vec<FetchedItem>, String> {
    info!(target: "4da::sources", "Fetching HN top stories");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let top_ids: Vec<u64> = client
        .get("https://hacker-news.firebaseio.com/v0/topstories.json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch top stories: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse top stories: {}", e))?;

    debug!(target: "4da::sources", count = top_ids.len(), "Got story IDs, fetching top 30");

    let mut items = Vec::new();
    for id in top_ids.into_iter().take(30) {
        let url = format!("https://hacker-news.firebaseio.com/v0/item/{}.json", id);
        match client.get(&url).send().await {
            Ok(response) => match response.json::<HNStory>().await {
                Ok(story) => {
                    let title = story.title.unwrap_or_else(|| "[No title]".to_string());

                    // Get content: prefer HN text field, otherwise scrape URL
                    let content = if let Some(text) = story.text {
                        // Ask HN / Show HN / text posts have content directly
                        debug!(target: "4da::sources", id = id, title = %title, "HN story has text");
                        text
                    } else if let Some(ref article_url) = story.url {
                        // Link posts - scrape the article
                        debug!(target: "4da::sources", id = id, title = %title, "Scraping HN story");
                        match scrape_article_content(article_url).await {
                            Some(scraped) => {
                                debug!(target: "4da::sources", id = id, chars = scraped.len(), "Scraped content");
                                scraped
                            }
                            None => {
                                debug!(target: "4da::sources", id = id, "Scrape failed, using title only");
                                String::new()
                            }
                        }
                    } else {
                        debug!(target: "4da::sources", id = id, title = %title, "HN story has no content");
                        String::new()
                    };

                    items.push(FetchedItem {
                        id: story.id,
                        title,
                        url: story.url,
                        content,
                    });
                }
                Err(e) => {
                    warn!(target: "4da::sources", id = id, error = %e, "Failed to parse story")
                }
            },
            Err(e) => warn!(target: "4da::sources", id = id, error = %e, "Failed to fetch story"),
        }
    }

    info!(target: "4da::sources", count = items.len(), "Loaded HN stories");
    Ok(items)
}

#[tauri::command]
pub(crate) async fn compute_relevance() -> Result<Vec<SourceRelevance>, String> {
    info!(target: "4da::analysis", "=== COMPUTING RELEVANCE SCORES (Phase 1 - with persistence) ===");

    let db = get_database()?;

    // Step 1: Check context availability (using sqlite-vec KNN search)
    debug!(target: "4da::analysis", "Step 1: Checking context (sqlite-vec KNN enabled)");
    let cached_context_count = db.context_count().map_err(|e| e.to_string())?;

    if cached_context_count > 0 {
        info!(target: "4da::analysis", context_chunks = cached_context_count, "Context indexed (using KNN search)");
    } else {
        warn!(target: "4da::analysis", "No context indexed. Scores will be 0 without context");
    }

    // Step 2: Fetch HN story IDs and process incrementally
    debug!(target: "4da::analysis", "Step 2: Fetching HN stories (incremental)");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let top_ids: Vec<u64> = client
        .get("https://hacker-news.firebaseio.com/v0/topstories.json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch top stories: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse top stories: {}", e))?;

    debug!(target: "4da::analysis", story_ids = top_ids.len(), "Processing top 30 stories");

    // Categorize: cached vs new
    let mut cached_items: Vec<(FetchedItem, Vec<f32>)> = Vec::new();
    let mut new_items: Vec<FetchedItem> = Vec::new();

    for id in top_ids.into_iter().take(30) {
        let id_str = id.to_string();

        // Check cache first
        if let Ok(Some(cached)) = db.get_source_item("hackernews", &id_str) {
            debug!(target: "4da::analysis", id = id, title = %&truncate_utf8(&cached.title, 40), "HN story (cached)");
            db.touch_source_item("hackernews", &id_str).ok();
            cached_items.push((
                FetchedItem {
                    id,
                    title: cached.title,
                    url: cached.url,
                    content: cached.content,
                },
                cached.embedding,
            ));
        } else {
            // Need to fetch from API
            let url = format!("https://hacker-news.firebaseio.com/v0/item/{}.json", id);
            match client.get(&url).send().await {
                Ok(response) => match response.json::<HNStory>().await {
                    Ok(story) => {
                        let title = story.title.unwrap_or_else(|| "[No title]".to_string());

                        // Get content: prefer HN text field, otherwise scrape URL
                        let content = if let Some(text) = story.text {
                            debug!(target: "4da::analysis", id = id, title = %&truncate_utf8(&title, 40), "HN story NEW - has text");
                            text
                        } else if let Some(ref article_url) = story.url {
                            debug!(target: "4da::analysis", id = id, title = %&truncate_utf8(&title, 35), "HN story NEW - scraping");
                            match scrape_article_content(article_url).await {
                                Some(scraped) => {
                                    debug!(target: "4da::analysis", id = id, chars = scraped.len(), "Scraped content");
                                    scraped
                                }
                                None => {
                                    debug!(target: "4da::analysis", id = id, "Scrape failed");
                                    String::new()
                                }
                            }
                        } else {
                            debug!(target: "4da::analysis", id = id, title = %&truncate_utf8(&title, 40), "HN story NEW - no content");
                            String::new()
                        };

                        new_items.push(FetchedItem {
                            id: story.id,
                            title,
                            url: story.url,
                            content,
                        });
                    }
                    Err(e) => {
                        warn!(target: "4da::analysis", id = id, error = %e, "Failed to parse story")
                    }
                },
                Err(e) => {
                    warn!(target: "4da::analysis", id = id, error = %e, "Failed to fetch story")
                }
            }
        }
    }

    info!(target: "4da::analysis", cached = cached_items.len(), new = new_items.len(), "Found items");

    // Step 3: Generate embeddings only for NEW items
    let new_embeddings = if !new_items.is_empty() {
        debug!(target: "4da::analysis", count = new_items.len(), "Step 3: Generating embeddings for NEW items");
        let with_content = new_items.iter().filter(|i| !i.content.is_empty()).count();
        debug!(target: "4da::analysis", with_content = with_content, "Items have scraped content");

        let new_texts: Vec<String> = new_items
            .iter()
            .map(|item| build_embedding_text(&item.title, &item.content))
            .collect();
        let embeddings = embed_texts(&new_texts).await?;

        // Cache new items in database
        debug!(target: "4da::analysis", count = new_items.len(), "Caching new items in database");
        for (item, embedding) in new_items.iter().zip(embeddings.iter()) {
            db.upsert_source_item(
                "hackernews",
                &item.id.to_string(),
                item.url.as_deref(),
                &decode_html_entities(&item.title),
                &decode_html_entities(&item.content),
                embedding,
            )
            .ok();
        }

        embeddings
    } else {
        debug!(target: "4da::analysis", "Step 3: All items cached, no embedding needed");
        vec![]
    };

    db.update_source_fetch_time("hackernews").ok();

    // Combine cached and new items
    let mut all_items_with_embeddings: Vec<(FetchedItem, Vec<f32>)> = cached_items;
    for (item, embedding) in new_items.into_iter().zip(new_embeddings.into_iter()) {
        all_items_with_embeddings.push((item, embedding));
    }

    if all_items_with_embeddings.is_empty() {
        return Err("No HN stories fetched".to_string());
    }

    // Step 4: Load user context for personalized scoring
    debug!(target: "4da::analysis", "Step 4: Loading user context");
    let context_engine = get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .map_err(|e| format!("Failed to load context: {}", e))?;

    let interest_count = static_identity.interests.len();
    let exclusion_count = static_identity.exclusions.len();
    info!(target: "4da::analysis", interests = interest_count, exclusions = exclusion_count, "User context loaded");

    if !static_identity.exclusions.is_empty() {
        debug!(target: "4da::analysis", exclusions = %static_identity.exclusions.join(", "), "Active exclusions");
    }
    if !static_identity.interests.is_empty() {
        let topics: Vec<&str> = static_identity
            .interests
            .iter()
            .map(|i| i.topic.as_str())
            .collect();
        debug!(target: "4da::analysis", interests = %topics.join(", "), "Active interests");
    }

    // Step 4b: Load ACE-discovered context
    debug!(target: "4da::ace", "Step 4b: Loading ACE discovered context");
    let ace_ctx = scoring::get_ace_context();
    // PASIFA: Pre-compute topic embeddings for semantic matching
    let topic_embeddings = scoring::get_topic_embeddings(&ace_ctx).await;
    info!(target: "4da::ace",
        active_topics = ace_ctx.active_topics.len(),
        detected_tech = ace_ctx.detected_tech.len(),
        anti_topics = ace_ctx.anti_topics.len(),
        affinities = ace_ctx.topic_affinities.len(),
        embeddings = topic_embeddings.len(),
        "ACE context loaded"
    );

    if !ace_ctx.active_topics.is_empty() {
        debug!(target: "4da::ace", topics = %ace_ctx.active_topics.iter().take(5).cloned().collect::<Vec<_>>().join(", "), "ACE Topics");
    }
    if !ace_ctx.detected_tech.is_empty() {
        debug!(target: "4da::ace", tech = %ace_ctx.detected_tech.iter().take(5).cloned().collect::<Vec<_>>().join(", "), "ACE Tech");
    }

    // Step 5: Compute similarity scores with context integration
    debug!(target: "4da::analysis", items = all_items_with_embeddings.len(), "Step 5: Computing personalized relevance");
    let mut results: Vec<SourceRelevance> = Vec::new();
    let mut excluded_count = 0;

    for (item, item_embedding) in &all_items_with_embeddings {
        // Extract topics from this item
        let topics = extract_topics(&item.title, &item.content);

        // Check exclusions FIRST (hard filter)
        let excluded_by = check_exclusions(&topics, &static_identity.exclusions)
            .or_else(|| scoring::check_ace_exclusions(&topics, &ace_ctx));

        if let Some(ref exclusion) = excluded_by {
            debug!(target: "4da::analysis", title = %&truncate_utf8(&item.title, 50), exclusion = %exclusion, "EXCLUDED");
            excluded_count += 1;

            // Still add to results but marked as excluded
            results.push(SourceRelevance {
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
                similar_count: 0,
                similar_titles: vec![],
                serendipity: false,
            });
            continue;
        }

        // Compute context file score using sqlite-vec KNN search (O(log n))
        // With graceful fallback to empty matches if KNN fails
        let matches: Vec<RelevanceMatch> = if cached_context_count > 0 {
            match db.find_similar_contexts(item_embedding, 3) {
                Ok(results) => results
                    .into_iter()
                    .map(|result| {
                        // Convert L2 distance to similarity: 1/(1+d) gives [0,1] range
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

        // Compute interest score (what you care about)
        let interest_score =
            scoring::compute_interest_score(item_embedding, &static_identity.interests);

        // Compute semantic ACE boost for topic/tech matching
        // PASIFA: Use semantic matching when embeddings available, fall back to keywords
        let semantic_boost =
            scoring::compute_semantic_ace_boost(item_embedding, &ace_ctx, &topic_embeddings)
                .unwrap_or_else(|| {
                    // Fall back to keyword matching for active topics and tech only (not affinities)
                    let mut boost: f32 = 0.0;
                    for topic in &topics {
                        let topic_lower = topic.to_lowercase();
                        // Active topics boost
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
                        // Tech stack boost
                        for tech in &ace_ctx.detected_tech {
                            if topic_lower.contains(tech) || tech.contains(&topic_lower) {
                                boost += 0.12;
                                break;
                            }
                        }
                    }
                    boost.clamp(0.0, 0.3)
                });

        // Keyword interest matching (with specificity weighting)
        let keyword_score = scoring::compute_keyword_interest_score_pub(
            &item.title,
            &item.content,
            &static_identity.interests,
        );

        // Combined score: weighted average of context, interest scores, plus semantic boost
        // Dynamically adjust weights based on what data is available
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

        // Multi-signal confirmation gate (same logic as cached path)
        let affinity_mult = scoring::compute_affinity_multiplier(&topics, &ace_ctx);
        let (gated_score, signal_count, confirmation_mult, confirmed_signals) =
            scoring::apply_confirmation_gate(
                base_score,
                context_score,
                interest_score,
                keyword_score,
                semantic_boost,
                &ace_ctx,
                &topics,
                0.0, // No feedback boost in fresh-fetch path
                affinity_mult,
                0.0, // No dep matching in fresh-fetch path (scored in cached analysis)
            );

        // PASIFA: Apply unified multiplicative scoring on gated score
        let combined_score = scoring::compute_unified_relevance(gated_score, &topics, &ace_ctx);

        let relevant = combined_score >= get_relevance_threshold();

        let anti_penalty = scoring::compute_anti_penalty(&topics, &ace_ctx);

        // Log scoring details
        if relevant {
            info!(target: "4da::analysis",
                id = item.id,
                title = %item.title,
                combined = combined_score,
                base = base_score,
                gated = gated_score,
                context = context_score,
                interest = interest_score,
                keyword = keyword_score,
                semantic_boost = semantic_boost,
                affinity_mult = affinity_mult,
                anti_penalty = anti_penalty,
                signal_count = signal_count,
                "RELEVANT"
            );
        } else {
            debug!(target: "4da::analysis",
                id = item.id,
                title = %item.title,
                combined = combined_score,
                gated = gated_score,
                context = context_score,
                interest = interest_score,
                signal_count = signal_count,
                "not relevant"
            );
        }
        if !topics.is_empty() {
            debug!(target: "4da::analysis", id = item.id, topics = %topics.iter().take(5).cloned().collect::<Vec<_>>().join(", "), "Extracted topics");
        }

        // Generate explanation for relevant items
        let declared_tech: Vec<String> = static_identity
            .tech_stack
            .iter()
            .map(|t| t.to_lowercase())
            .collect();
        let explanation = if relevant {
            Some(scoring::generate_relevance_explanation(
                &item.title,
                context_score,
                interest_score,
                &matches,
                &ace_ctx,
                &topics,
                &static_identity.interests,
                &declared_tech,
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
            signal_count,
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
            keyword_score,
            ace_boost: semantic_boost,
            affinity_mult,
            anti_penalty,
            freshness_mult: 1.0,
            feedback_boost: 0.0,
            source_quality_boost: 0.0,
            confidence_by_signal,
            signal_count,
            confirmed_signals,
            confirmation_mult,
            dep_match_score: 0.0,
            matched_deps: vec![],
            domain_relevance: 1.0,
            content_quality_mult: 1.0,
            novelty_mult: 1.0,
            intent_boost: 0.0,
            content_type: None,
            content_dna_mult: 1.0,
            competing_mult: 1.0,
            llm_score: None,
            llm_reason: None,
        };

        results.push(SourceRelevance {
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
            similar_count: 0,
            similar_titles: vec![],
            serendipity: false,
        });
    }

    // Sort by relevance score descending (excluded items go to bottom)
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

    // Summary
    let relevant_count = results.iter().filter(|r| r.relevant && !r.excluded).count();
    let db_item_count = db.total_item_count().unwrap_or(0);
    info!(target: "4da::analysis", "=== PERSONALIZED ANALYSIS COMPLETE ===");
    info!(target: "4da::analysis",
        total = results.len(),
        relevant = relevant_count,
        excluded = excluded_count,
        interests = interest_count,
        exclusions = exclusion_count,
        threshold = get_relevance_threshold(),
        db_cached = db_item_count,
        "Analysis summary"
    );

    Ok(results)
}

// ============================================================================
// Background Job Functions (called by monitoring scheduler)
// ============================================================================

/// Run background health check - called every 5 minutes by scheduler
pub async fn run_background_health_check() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn().lock();
    let report = health::check_all_components(&conn)?;

    info!(
        target: "4da::health",
        status = ?report.overall_status,
        quality = ?report.context_quality,
        fallback = report.fallback_level,
        "Background health check complete"
    );

    serde_json::to_value(&report).map_err(|e| e.to_string())
}

/// Run background anomaly detection - called every hour by scheduler
pub async fn run_background_anomaly_detection() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn().lock();
    let anomalies = anomaly::detect_all(&conn)?;

    // Store any new anomalies
    let mut new_count = 0;
    for a in &anomalies {
        if anomaly::store_anomaly(&conn, a).is_ok() {
            new_count += 1;
        }
    }

    info!(target: "4da::anomaly", found = anomalies.len(), stored = new_count, "Background anomaly detection complete");

    Ok(serde_json::json!({
        "anomalies_found": anomalies.len(),
        "new_stored": new_count,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Run background behavior decay - called daily by scheduler
pub async fn run_background_behavior_decay() -> Result<serde_json::Value, String> {
    let ace = get_ace_engine()?;

    // Apply decay to behavior signals
    let decayed_count = ace.apply_behavior_decay()?;

    info!(
        target: "4da::decay",
        signals_decayed = decayed_count,
        "Background behavior decay applied"
    );

    // Auto-tune relevance threshold based on engagement rate
    let threshold_adjusted = {
        let current = get_relevance_threshold();
        if let Some(new_threshold) = ace.compute_threshold_adjustment(current) {
            set_relevance_threshold(new_threshold);
            ace.store_threshold(new_threshold);
            info!(
                target: "4da::threshold",
                old = current,
                new = new_threshold,
                "Auto-tuned relevance threshold"
            );
            Some(new_threshold)
        } else {
            None
        }
    };

    Ok(serde_json::json!({
        "signals_decayed": decayed_count,
        "threshold_adjusted": threshold_adjusted,
        "current_threshold": get_relevance_threshold(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

// Update, Digest, and AI Briefing commands are in digest_commands.rs

// ============================================================================
// MCP Score Autopsy Command
// ============================================================================

/// Execute score autopsy - native implementation using AnalysisState data
/// Provides deep breakdown of why an item scored the way it did
#[tauri::command]
pub(crate) async fn mcp_score_autopsy(
    item_id: u64,
    source_type: String,
    _synthesize: bool,
    _compact: bool,
) -> Result<serde_json::Value, String> {
    info!(
        target: "4da::autopsy",
        item_id = item_id,
        source_type = %source_type,
        "Score autopsy requested"
    );

    // Find the item in analysis results
    let state = get_analysis_state();
    let guard = state.lock();
    let results = guard
        .results
        .as_ref()
        .ok_or("No analysis results available. Run an analysis first.")?;

    let item = results
        .iter()
        .find(|r| r.id == item_id)
        .ok_or_else(|| format!("Item {} not found in analysis results", item_id))?;

    // Get item metadata from DB
    let db = get_database()?;
    let db_item = db.get_source_item_by_id(item_id as i64).ok().flatten();

    let age_hours = db_item
        .as_ref()
        .map(|i| (chrono::Utc::now() - i.created_at).num_minutes() as f64 / 60.0)
        .unwrap_or(0.0);

    let created_at = db_item
        .as_ref()
        .map(|i| i.created_at.to_rfc3339())
        .unwrap_or_default();

    // Build component breakdown from ScoreBreakdown
    let mut components = Vec::new();
    if let Some(ref bd) = item.score_breakdown {
        components.push(serde_json::json!({
            "name": "Context Match",
            "raw_value": bd.context_score,
            "weight": 0.5,
            "contribution": bd.context_score * 0.5,
            "explanation": if bd.context_score > 0.2 {
                format!("Strong match with your project files ({:.0}% similarity)", bd.context_score * 100.0)
            } else if bd.context_score > 0.05 {
                format!("Weak match with project context ({:.0}%)", bd.context_score * 100.0)
            } else {
                "No significant match with indexed project files".to_string()
            }
        }));

        components.push(serde_json::json!({
            "name": "Interest Match",
            "raw_value": bd.interest_score,
            "weight": 0.5,
            "contribution": bd.interest_score * 0.5,
            "explanation": if bd.interest_score > 0.3 {
                format!("Closely matches your declared interests ({:.0}%)", bd.interest_score * 100.0)
            } else if bd.interest_score > 0.1 {
                format!("Partial interest match ({:.0}%)", bd.interest_score * 100.0)
            } else {
                "Low alignment with declared interests".to_string()
            }
        }));

        if bd.ace_boost > 0.01 {
            components.push(serde_json::json!({
                "name": "ACE Semantic Boost",
                "raw_value": bd.ace_boost,
                "weight": 1.0,
                "contribution": bd.ace_boost,
                "explanation": format!("Boosted by ACE context engine topics/tech (+{:.0}%)", bd.ace_boost * 100.0)
            }));
        }

        if (bd.affinity_mult - 1.0).abs() > 0.01 {
            let direction = if bd.affinity_mult > 1.0 {
                "boosted"
            } else {
                "reduced"
            };
            components.push(serde_json::json!({
                "name": "Learned Affinity",
                "raw_value": bd.affinity_mult,
                "weight": 1.0,
                "contribution": bd.affinity_mult - 1.0,
                "explanation": format!("Score {} by learned topic preferences (x{:.2})", direction, bd.affinity_mult)
            }));
        }

        if bd.anti_penalty > 0.01 {
            components.push(serde_json::json!({
                "name": "Anti-Topic Penalty",
                "raw_value": bd.anti_penalty,
                "weight": 1.0,
                "contribution": -bd.anti_penalty,
                "explanation": format!("Penalized by anti-topic filter (-{:.0}%)", bd.anti_penalty * 100.0)
            }));
        }

        if (bd.freshness_mult - 1.0).abs() > 0.01 {
            let label = if bd.freshness_mult > 1.0 {
                "Freshness bonus"
            } else {
                "Staleness decay"
            };
            components.push(serde_json::json!({
                "name": "Temporal Freshness",
                "raw_value": bd.freshness_mult,
                "weight": 1.0,
                "contribution": bd.freshness_mult - 1.0,
                "explanation": format!("{}: item is {:.0}h old (x{:.2})", label, age_hours, bd.freshness_mult)
            }));
        }
    }

    // Build matching context from ACE
    let ace_ctx = scoring::get_ace_context();
    let topics = extract_topics(&item.title, "");

    let matching_interests: Vec<String> = {
        let ctx_engine = get_context_engine().ok();
        ctx_engine
            .and_then(|ce| ce.get_static_identity().ok())
            .map(|id| {
                id.interests
                    .iter()
                    .filter(|i| {
                        let int_lower = i.topic.to_lowercase();
                        topics.iter().any(|t| {
                            let tl = t.to_lowercase();
                            tl.contains(&int_lower) || int_lower.contains(&tl)
                        })
                    })
                    .map(|i| i.topic.clone())
                    .collect()
            })
            .unwrap_or_default()
    };

    let matching_tech: Vec<String> = ace_ctx
        .detected_tech
        .iter()
        .filter(|t| {
            let tl = t.to_lowercase();
            topics.iter().any(|topic| {
                let topic_lower = topic.to_lowercase();
                topic_lower.contains(&tl) || tl.contains(&topic_lower)
            })
        })
        .cloned()
        .collect();

    let matching_active: Vec<String> = ace_ctx
        .active_topics
        .iter()
        .filter(|t| {
            topics.iter().any(|topic| {
                let topic_lower = topic.to_lowercase();
                topic_lower.contains(t.as_str()) || t.contains(&topic_lower)
            })
        })
        .cloned()
        .collect();

    let matching_affinities: Vec<String> = ace_ctx
        .topic_affinities
        .iter()
        .filter(|(_, (score, _))| *score > 0.3)
        .filter(|(topic, _)| {
            topics.iter().any(|t| {
                let tl = t.to_lowercase();
                tl.contains(topic.as_str()) || topic.contains(&tl)
            })
        })
        .map(|(topic, (score, _))| format!("{} ({:+.0}%)", topic, score * 100.0))
        .collect();

    // Find similar items for comparison (items with close scores)
    let similar_items: Vec<serde_json::Value> = results
        .iter()
        .filter(|r| r.id != item_id && r.relevant)
        .map(|r| {
            let diff = r.top_score - item.top_score;
            let key_diff = if diff.abs() < 0.05 {
                "Very similar score - different content matched".to_string()
            } else if diff > 0.0 {
                if r.context_score > item.context_score + 0.1 {
                    "Higher context match with your project files".to_string()
                } else if r.interest_score > item.interest_score + 0.1 {
                    "Better alignment with declared interests".to_string()
                } else {
                    "Stronger overall relevance signals".to_string()
                }
            } else if item.context_score > r.context_score + 0.1 {
                "This item has stronger project context match".to_string()
            } else {
                "This item has stronger interest alignment".to_string()
            };
            (r, diff, key_diff)
        })
        .take(3)
        .map(|(r, diff, key_diff)| {
            serde_json::json!({
                "id": r.id,
                "title": r.title,
                "score": r.top_score,
                "score_difference": diff,
                "key_difference": key_diff
            })
        })
        .collect();

    // Generate recommendations
    let mut recommendations = Vec::new();
    if item.context_score < 0.1 {
        recommendations.push("Index more project files to improve context matching. Add directories in Settings > Context.".to_string());
    }
    if item.interest_score < 0.1 {
        recommendations.push(
            "Add more interests in Settings > Interests to improve matching for this topic area."
                .to_string(),
        );
    }
    if matching_tech.is_empty() {
        recommendations.push("This item doesn't match your detected tech stack. If it's relevant, the ACE engine will learn from your interaction.".to_string());
    }
    if item.top_score < 0.35 && !item.relevant {
        recommendations.push("This item fell below the relevance threshold. Save items like this to train the system to surface similar content.".to_string());
    }

    // Build narrative
    let narrative = build_autopsy_narrative(item, &matching_tech, &matching_active, age_hours);

    Ok(serde_json::json!({
        "item": {
            "id": item.id,
            "title": item.title,
            "url": item.url,
            "source_type": item.source_type,
            "created_at": created_at,
            "age_hours": age_hours
        },
        "final_score": item.top_score,
        "components": components,
        "matching_context": {
            "interests": matching_interests,
            "tech_stack": matching_tech,
            "active_topics": matching_active,
            "learned_affinities": matching_affinities,
            "exclusions_hit": item.excluded_by.as_ref().map(|e| vec![e.clone()]).unwrap_or_else(Vec::<String>::new)
        },
        "similar_items": similar_items,
        "recommendations": recommendations,
        "narrative": narrative
    }))
}

/// Build a human-readable narrative for the score autopsy
fn build_autopsy_narrative(
    item: &SourceRelevance,
    matching_tech: &[String],
    matching_active: &[String],
    age_hours: f64,
) -> String {
    let mut parts = Vec::new();

    // Score assessment
    let score_pct = (item.top_score * 100.0) as u32;
    if item.top_score >= 0.6 {
        parts.push(format!("This item scored {}% - a strong match.", score_pct));
    } else if item.top_score >= 0.35 {
        parts.push(format!(
            "This item scored {}% - above the relevance threshold.",
            score_pct
        ));
    } else {
        parts.push(format!(
            "This item scored {}% - below the relevance threshold of 35%.",
            score_pct
        ));
    }

    // Context explanation
    if item.context_score > 0.3 {
        parts.push("It closely matches code you're actively working on.".to_string());
    } else if item.context_score > 0.1 {
        parts.push("It has some overlap with your project files.".to_string());
    }

    // Interest explanation
    if item.interest_score > 0.3 {
        parts.push("It strongly aligns with your declared interests.".to_string());
    } else if item.interest_score > 0.1 {
        parts.push("It partially matches your interests.".to_string());
    }

    // Tech stack
    if !matching_tech.is_empty() {
        parts.push(format!(
            "It mentions {} which is in your tech stack.",
            matching_tech.join(", ")
        ));
    }

    // Active topics
    if !matching_active.is_empty() {
        parts.push(format!(
            "It relates to topics you've been active in: {}.",
            matching_active.join(", ")
        ));
    }

    // Freshness
    if age_hours < 2.0 {
        parts.push("It was discovered very recently and received a freshness boost.".to_string());
    } else if age_hours > 36.0 {
        parts.push(format!(
            "It's {:.0} hours old, so its score was slightly reduced for staleness.",
            age_hours
        ));
    }

    // Signal info
    if let Some(ref sig) = item.signal_type {
        let label = match sig.as_str() {
            "security_alert" => "a security alert",
            "breaking_change" => "a breaking change notification",
            "tool_discovery" => "a new tool/library discovery",
            "tech_trend" => "a technology trend",
            "learning" => "a learning resource",
            "competitive_intel" => "competitive intelligence",
            _ => "a classified signal",
        };
        parts.push(format!(
            "It was classified as {} with {} priority.",
            label,
            item.signal_priority.as_deref().unwrap_or("unknown")
        ));
    }

    parts.join(" ")
}

// ============================================================================
// Product Hardening Commands
// ============================================================================

/// Get registered sources
#[tauri::command]
pub(crate) async fn get_sources() -> Result<Vec<serde_json::Value>, String> {
    let registry = get_source_registry();
    let guard = registry.lock();

    let sources: Vec<serde_json::Value> = guard
        .sources()
        .iter()
        .map(|s| {
            serde_json::json!({
                "type": s.source_type(),
                "name": s.name(),
                "enabled": s.config().enabled,
                "max_items": s.config().max_items,
                "fetch_interval_secs": s.config().fetch_interval_secs
            })
        })
        .collect();

    Ok(sources)
}

/// Get database statistics
#[tauri::command]
pub(crate) async fn get_database_stats() -> Result<serde_json::Value, String> {
    let db = get_database()?;

    let context_count = db.context_count().map_err(|e| e.to_string())?;
    let hn_count = db
        .source_item_count("hackernews")
        .map_err(|e| e.to_string())?;
    let total_count = db.total_item_count().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "context_chunks": context_count,
        "hackernews_items": hn_count,
        "total_items": total_count
    }))
}

// Analysis functions (start_background_analysis, run_multi_source_analysis, etc.) are in analysis.rs
// Settings and Context Engine commands are in settings_commands.rs
// ACE commands, PASIFA helpers, and auto-seeding are in ace_commands.rs

/// Toggle a source's enabled/disabled status
#[tauri::command]
pub(crate) async fn toggle_source_enabled(
    source_type: String,
    enabled: bool,
) -> Result<(), String> {
    let db = get_database()?;
    db.toggle_source_enabled(&source_type, enabled)
        .map_err(|e| format!("Failed to toggle source: {}", e))
}

/// Get all sources with their enabled status and health
#[tauri::command]
pub(crate) async fn get_all_sources_status() -> Result<serde_json::Value, String> {
    let db = get_database()?;
    let sources = db.get_all_sources().map_err(|e| e.to_string())?;
    let health = db.get_source_health().unwrap_or_default();

    let result: Vec<serde_json::Value> = sources
        .iter()
        .map(|(source_type, name, enabled, last_fetch)| {
            let h = health.iter().find(|h| h.source_type == *source_type);
            serde_json::json!({
                "source_type": source_type,
                "name": name,
                "enabled": enabled,
                "last_fetch": last_fetch,
                "status": h.map(|h| h.status.as_str()).unwrap_or("unknown"),
                "error_count": h.map(|h| h.error_count).unwrap_or(0),
                "consecutive_failures": h.map(|h| h.consecutive_failures).unwrap_or(0),
                "items_fetched": h.map(|h| h.items_fetched).unwrap_or(0),
                "response_time_ms": h.map(|h| h.response_time_ms).unwrap_or(0),
                "last_success": h.and_then(|h| h.last_success.clone()),
                "last_error": h.and_then(|h| h.last_error.clone()),
            })
        })
        .collect();

    Ok(serde_json::json!({ "sources": result }))
}

/// Get source health data
#[tauri::command]
pub(crate) async fn get_source_health() -> Result<serde_json::Value, String> {
    let db = get_database()?;
    let health = db.get_source_health().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "health": health }))
}

/// Check network connectivity
#[tauri::command]
pub(crate) async fn check_network_status() -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let online = client.head("https://httpbin.org/get").send().await.is_ok();

    Ok(serde_json::json!({
        "online": online,
        "checked_at": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Run database maintenance (cleanup old items, vacuum)
#[tauri::command]
pub(crate) async fn run_db_maintenance() -> Result<serde_json::Value, String> {
    let db = get_database()?;
    let result = db.run_maintenance(90).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "deleted_items": result.deleted_items,
        "deleted_feedback": result.deleted_feedback,
        "deleted_void": result.deleted_void,
    }))
}

/// Get database statistics
#[tauri::command]
pub(crate) async fn get_db_stats_detailed() -> Result<serde_json::Value, String> {
    let db = get_database()?;
    let stats = db.get_db_stats().map_err(|e| e.to_string())?;

    // Get DB file size
    let db_path = get_db_path();
    let file_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    Ok(serde_json::json!({
        "source_items": stats.source_items,
        "context_chunks": stats.context_chunks,
        "feedback_count": stats.feedback_count,
        "sources_count": stats.sources_count,
        "file_size_bytes": file_size,
        "file_size_mb": format!("{:.1}", file_size as f64 / 1_048_576.0),
    }))
}

/// Export current analysis results in specified format
#[tauri::command]
pub(crate) async fn export_results(format: String) -> Result<String, String> {
    let state = get_analysis_state();
    let guard = state.lock();

    let results = match &guard.results {
        Some(r) => r,
        None => return Err("No analysis results to export".to_string()),
    };

    let relevant: Vec<&SourceRelevance> = results.iter().filter(|r| r.relevant).collect();

    match format.as_str() {
        "markdown" => {
            let mut md = String::from("# 4DA Analysis Results\n\n");
            md.push_str(&format!(
                "**Generated:** {}\n",
                chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
            ));
            md.push_str(&format!(
                "**Total items:** {} ({} relevant)\n\n",
                results.len(),
                relevant.len()
            ));
            md.push_str("---\n\n");
            for item in &relevant {
                let score_pct = (item.top_score * 100.0) as u32;
                md.push_str(&format!("### {} ({}%)\n", item.title, score_pct));
                if let Some(ref url) = item.url {
                    md.push_str(&format!("- **URL:** {}\n", url));
                }
                md.push_str(&format!("- **Source:** {}\n", item.source_type));
                if let Some(ref explanation) = item.explanation {
                    md.push_str(&format!("- **Why:** {}\n", explanation));
                }
                md.push('\n');
            }
            Ok(md)
        }
        "text" => {
            let mut text = format!(
                "4DA Analysis Results ({})\n",
                chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
            );
            text.push_str(&format!(
                "{} items, {} relevant\n\n",
                results.len(),
                relevant.len()
            ));
            for item in &relevant {
                let score_pct = (item.top_score * 100.0) as u32;
                text.push_str(&format!(
                    "[{}%] {} ({})\n",
                    score_pct, item.title, item.source_type
                ));
                if let Some(ref url) = item.url {
                    text.push_str(&format!("  {}\n", url));
                }
            }
            Ok(text)
        }
        "html" => {
            let mut html = String::from("<html><head><title>4DA Analysis Results</title></head><body style='font-family:sans-serif;background:#0A0A0A;color:#fff;padding:2rem'>");
            html.push_str(&format!(
                "<h1>4DA Analysis Results</h1><p>{} items, {} relevant</p><hr>",
                results.len(),
                relevant.len()
            ));
            for item in &relevant {
                let score_pct = (item.top_score * 100.0) as u32;
                html.push_str("<div style='margin:1rem 0;padding:1rem;background:#141414;border-radius:8px;border:1px solid #2A2A2A'>");
                html.push_str(&format!("<strong>{}%</strong> ", score_pct));
                if let Some(ref url) = item.url {
                    html.push_str(&format!(
                        "<a href='{}' style='color:#D4AF37'>{}</a>",
                        url, item.title
                    ));
                } else {
                    html.push_str(&item.title);
                }
                html.push_str(&format!(
                    " <span style='color:#666'>({})</span>",
                    item.source_type
                ));
                html.push_str("</div>");
            }
            html.push_str("</body></html>");
            Ok(html)
        }
        "digest" => {
            // Rich shareable digest with Developer DNA context
            let dna = developer_dna::generate_dna().ok();
            let now = chrono::Utc::now();
            let date_str = now.format("%B %d, %Y").to_string();

            let mut md = format!("# 4DA Intelligence Digest — {}\n\n", date_str);

            // Developer identity header
            if let Some(ref dna) = dna {
                md.push_str(&format!("> **{}**", dna.identity_summary));
                if !dna.primary_stack.is_empty() {
                    md.push_str(&format!(" | Stack: {}", dna.primary_stack.join(", ")));
                }
                md.push('\n');
            }
            md.push('\n');

            // Stats bar
            let high_signal = relevant.iter().filter(|r| r.top_score >= 0.5).count();
            let rejection_pct = if !results.is_empty() {
                ((1.0 - relevant.len() as f64 / results.len() as f64) * 100.0) as u32
            } else {
                0
            };
            md.push_str(&format!(
                "**{}** items scored | **{}** relevant | **{}** high-signal | **{}%** filtered as noise\n\n",
                results.len(), relevant.len(), high_signal, rejection_pct
            ));
            md.push_str("---\n\n");

            // High-signal section
            let high: Vec<&&SourceRelevance> =
                relevant.iter().filter(|r| r.top_score >= 0.5).collect();
            if !high.is_empty() {
                md.push_str("## High-Signal\n\n");
                for item in &high {
                    let score_pct = (item.top_score * 100.0) as u32;
                    if let Some(ref url) = item.url {
                        md.push_str(&format!("- **[{}]({})** — {}%", item.title, url, score_pct));
                    } else {
                        md.push_str(&format!("- **{}** — {}%", item.title, score_pct));
                    }
                    if let Some(ref explanation) = item.explanation {
                        md.push_str(&format!(" — {}", explanation));
                    }
                    md.push('\n');
                }
                md.push('\n');
            }

            // Group remaining by source
            let mut by_source: std::collections::HashMap<&str, Vec<&&SourceRelevance>> =
                std::collections::HashMap::new();
            for item in &relevant {
                if item.top_score < 0.5 {
                    by_source
                        .entry(item.source_type.as_str())
                        .or_default()
                        .push(item);
                }
            }

            if !by_source.is_empty() {
                fn source_label(s: &str) -> &str {
                    match s {
                        "hackernews" => "Hacker News",
                        "reddit" => "Reddit",
                        "arxiv" => "arXiv",
                        "github" => "GitHub",
                        "producthunt" => "Product Hunt",
                        "youtube" => "YouTube",
                        "twitter" => "Twitter/X",
                        "rss" => "RSS",
                        "devto" => "Dev.to",
                        "lobsters" => "Lobsters",
                        other => other,
                    }
                }

                // Sort sources by item count descending
                let mut sources: Vec<_> = by_source.into_iter().collect();
                sources.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

                for (source, items) in &sources {
                    md.push_str(&format!(
                        "## {} ({})\n\n",
                        source_label(source),
                        items.len()
                    ));
                    for item in items.iter().take(8) {
                        let score_pct = (item.top_score * 100.0) as u32;
                        if let Some(ref url) = item.url {
                            md.push_str(&format!("- [{}]({}) — {}%\n", item.title, url, score_pct));
                        } else {
                            md.push_str(&format!("- {} — {}%\n", item.title, score_pct));
                        }
                    }
                    if items.len() > 8 {
                        md.push_str(&format!("- *...+{} more*\n", items.len() - 8));
                    }
                    md.push('\n');
                }
            }

            // Footer
            md.push_str("---\n\n");
            md.push_str("*Generated by [4DA](https://github.com/4da-dev/4da-home) — The internet, scored for you.*\n");

            Ok(md)
        }
        _ => Err(format!(
            "Unknown format: {}. Use 'markdown', 'text', 'html', or 'digest'",
            format
        )),
    }
}
