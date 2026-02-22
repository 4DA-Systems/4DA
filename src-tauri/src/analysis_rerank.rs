//! Post-scoring quality processing — LLM reranking, digest generation, and analysis hooks.

use tauri::Emitter;
use tracing::{info, warn};

use crate::scoring;
use crate::{
    emit_progress, get_analysis_state, get_database, get_settings_manager, monitoring,
    SourceRelevance,
};

// ============================================================================
// LLM Reranking
// ============================================================================

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
pub(crate) async fn apply_llm_reranking(
    app: &tauri::AppHandle,
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

// ============================================================================
// Digest Generation
// ============================================================================

/// Generate and save digest from analysis results (if enabled)
pub(crate) fn maybe_save_digest(results: &[SourceRelevance]) {
    use crate::digest::{Digest, DigestItem, DigestManager};
    use chrono::{Duration, Utc};

    let settings = get_settings_manager().lock();
    let config = settings.get().digest.clone();
    drop(settings);

    if !config.enabled || !config.save_local {
        return;
    }

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

    let period_end = Utc::now();
    let period_start = period_end - Duration::hours(24);
    let digest = Digest::new(relevant_items, period_start, period_end);

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

// ============================================================================
// Post-Analysis Hooks
// ============================================================================

// ============================================================================
// Deduplication Utilities
// ============================================================================

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

// ============================================================================
// Full Scoring Pipeline
// ============================================================================

/// Score all items in a full analysis pass
pub(crate) async fn score_items_full(
    app: &tauri::AppHandle,
    db: &crate::db::Database,
    cached_items: &[crate::db::StoredSourceItem],
) -> Result<Vec<SourceRelevance>, String> {
    use std::sync::atomic::Ordering;

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

    let classifier = crate::analysis::signal_classifier();
    let mut results: Vec<SourceRelevance> = Vec::new();

    for (idx, &item_idx) in keep_indices.iter().enumerate() {
        let item = &cached_items[item_idx];
        if crate::get_analysis_abort().load(Ordering::SeqCst) {
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
            Some(classifier),
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

        // 3. Scan for reverse mentions of user's projects
        let _ = crate::reverse_relevance::scan_for_mentions(&conn, 24);

        info!(target: "4da::analysis", "Post-analysis innovation hooks complete");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_url_strips_protocol_www_trailing_slash_query_fragment() {
        assert_eq!(
            normalize_url("http://example.com/page"),
            "https://example.com/page"
        );

        // www stripping
        assert_eq!(
            normalize_url("https://www.example.com/page"),
            "https://example.com/page"
        );

        // Trailing slash removal
        assert_eq!(
            normalize_url("https://example.com/page/"),
            "https://example.com/page"
        );

        // Query parameter stripping
        assert_eq!(
            normalize_url("https://example.com/page?utm_source=twitter&ref=123"),
            "https://example.com/page"
        );

        // Fragment stripping
        assert_eq!(
            normalize_url("https://example.com/page#section-2"),
            "https://example.com/page"
        );

        // All combined: http + www + trailing slash + query + fragment
        assert_eq!(
            normalize_url("http://www.example.com/article/?ref=hn#comments"),
            "https://example.com/article"
        );

        // Lowercase normalization
        assert_eq!(
            normalize_url("HTTPS://Example.COM/Path"),
            "https://example.com/path"
        );

        // Empty and whitespace
        assert_eq!(normalize_url(""), "");
        assert_eq!(
            normalize_url("  https://example.com  "),
            "https://example.com"
        );
    }

    #[test]
    fn test_normalize_title_strips_prefixes_and_normalizes() {
        assert_eq!(
            normalize_title_for_dedup("Show HN: My Cool Project"),
            "my cool project"
        );
        assert_eq!(
            normalize_title_for_dedup("Ask HN: Best Rust framework?"),
            "best rust framework"
        );
        assert_eq!(
            normalize_title_for_dedup("Tell HN: I built a thing"),
            "i built a thing"
        );
        assert_eq!(
            normalize_title_for_dedup("Launch HN: NewStartup"),
            "newstartup"
        );

        // Reddit prefixes
        assert_eq!(
            normalize_title_for_dedup("[D] Discussion about transformers"),
            "discussion about transformers"
        );
        assert_eq!(
            normalize_title_for_dedup("[R] New paper on attention"),
            "new paper on attention"
        );

        // HTML entity decoding (via decode_html_entities)
        assert_eq!(
            normalize_title_for_dedup("Rust &amp; WebAssembly"),
            "rust webassembly"
        );
        assert_eq!(normalize_title_for_dedup("5 &gt; 3 &lt; 10"), "5 3 10");

        // Punctuation removal and whitespace normalization
        assert_eq!(
            normalize_title_for_dedup("  Hello,   World!  (2024)  "),
            "hello world 2024"
        );

        // Empty string
        assert_eq!(normalize_title_for_dedup(""), "");
    }

    #[test]
    fn test_normalize_title_dedup_equivalence() {
        // Two titles that differ only by source prefix should be equal after normalization
        let hn_title = normalize_title_for_dedup("Show HN: Building a Rust CLI tool");
        let raw_title = normalize_title_for_dedup("Building a Rust CLI tool");
        assert_eq!(hn_title, raw_title);

        // Same title with different HTML encoding
        let encoded = normalize_title_for_dedup("React &amp; Next.js Guide");
        let decoded = normalize_title_for_dedup("React & Next.js Guide");
        assert_eq!(encoded, decoded);
    }

    fn make_item(id: i64, title: &str, url: Option<&str>) -> crate::db::StoredSourceItem {
        crate::db::StoredSourceItem {
            id,
            source_type: "hackernews".to_string(),
            source_id: format!("test-{}", id),
            url: url.map(String::from),
            title: title.to_string(),
            content: String::new(),
            content_hash: format!("hash-{}", id),
            embedding: vec![],
            created_at: chrono::Utc::now(),
            last_seen: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_dedup_stored_items_removes_url_duplicates() {
        let items = vec![
            make_item(1, "First Article", Some("https://example.com/article")),
            make_item(
                2,
                "Different Title",
                Some("https://www.example.com/article/"),
            ),
            make_item(3, "Third Article", Some("https://other.com/post")),
        ];

        let kept = dedup_stored_items(&items);
        // Item 2 has the same normalized URL as item 1, so only items 1 and 3 should remain
        assert_eq!(kept, vec![0, 2]);
    }

    #[test]
    fn test_dedup_stored_items_removes_title_duplicates() {
        let items = vec![
            make_item(1, "Show HN: My Cool Tool", None),
            make_item(2, "My Cool Tool", None),
            make_item(3, "Completely Different Article", None),
        ];

        let kept = dedup_stored_items(&items);
        // Item 2 normalizes to same title as item 1 after prefix stripping
        assert_eq!(kept, vec![0, 2]);
    }

    #[test]
    fn test_dedup_stored_items_keeps_unique_items() {
        let items = vec![
            make_item(1, "Rust async runtime", Some("https://blog.com/rust")),
            make_item(2, "Go concurrency patterns", Some("https://blog.com/go")),
            make_item(
                3,
                "Python type hints guide",
                Some("https://blog.com/python"),
            ),
        ];

        let kept = dedup_stored_items(&items);
        assert_eq!(kept, vec![0, 1, 2]);
    }
}
