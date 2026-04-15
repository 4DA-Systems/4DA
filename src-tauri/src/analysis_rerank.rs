//! Post-scoring quality processing — LLM reranking, digest generation, and dedup utilities.

use tracing::{debug, info, warn};

use crate::{emit_progress, get_database, get_settings_manager, scoring, SourceRelevance};

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
            .map(std::string::String::as_str)
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
            let mut anti_vec: Vec<&str> = anti.iter().map(std::string::String::as_str).collect();
            anti_vec.sort_unstable();
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
                            format!("- {truncated}")
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
                            format!("- {truncated}")
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
                format!("{source_label} {content_snippet}"),
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

    // Construct the advisory core. It carries its own ModelIdentity and
    // prompt_version so every AdvisorSignal and provenance row this rerank
    // pass writes share a single source of truth. Intelligence Mesh Phase 4
    // — the trait exists so Phases 5 (calibration wrap) and 6 (shadow arena)
    // can swap the impl without re-plumbing this loop.
    // See `docs/strategy/INTELLIGENCE-MESH.md` §2 Layer 2.
    let core: Box<dyn crate::intelligence_core::IntelligenceCore> =
        Box::new(crate::intelligence_core::LlmJudgeCore::new(llm_settings));
    let advisor_identity = core.identity();
    let advisor_prompt_version = core.prompt_version();
    let advisor_calibration_id = core.calibration_id();

    // Split into batches of 8 for better LLM accuracy
    const LLM_BATCH_SIZE: usize = 8;
    let batches: Vec<Vec<(String, String, String)>> = candidates
        .chunks(LLM_BATCH_SIZE)
        .map(
            <[(
                std::string::String,
                std::string::String,
                std::string::String,
            )]>::to_vec,
        )
        .collect();

    let total_batches = batches.len();
    let total_candidates = batches.iter().map(std::vec::Vec::len).sum::<usize>();
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

        let req = crate::intelligence_core::JudgeRequest {
            context_summary: context_summary.clone(),
            items: batch.clone(),
        };
        match core.judge(req).await {
            Ok(validated) => {
                total_input += validated.value.input_tokens;
                total_output += validated.value.output_tokens;
                all_judgments.extend(validated.value.judgments);
            }
            Err(e) => {
                warn!(target: "4da::rerank", batch = batch_idx, error = %e, "LLM batch failed, continuing");
            }
        }
    }

    if all_judgments.is_empty() {
        return None;
    }

    // Legacy-path counters (reconciler_enabled=false): judgment.relevant
    // hard-accepts/hard-rejects. Zero in the reconciler path.
    let mut confirmed = 0usize;
    let mut rejected = 0usize;
    // Reconciler-path counters (reconciler_enabled=true): honest breakdown
    // of how the bounded adjustment played out. Nothing is "rejected" in
    // this path — the worst an advisor can do is push an item down by the
    // ±ADVISOR_ADJUSTMENT_CAP (0.15).
    let mut reconciled_agreed = 0usize;
    let mut reconciled_skeptical = 0usize;
    let mut reconciled_enthusiastic = 0usize;
    let mut reconciled_internal = 0usize;

    // Collect provenance rows for batch-insert after the judgment loop.
    // This records one row per judged item so compound-learning, receipts,
    // and drift-detection can reason about which model/prompt produced each
    // rerank adjustment. Intelligence Mesh Phase 3.
    let mut provenance_rows: Vec<crate::provenance::Provenance> =
        Vec::with_capacity(all_judgments.len());

    for judgment in &all_judgments {
        if let Some(result) = results
            .iter_mut()
            .find(|r| r.id.to_string() == judgment.item_id)
        {
            // Store LLM score and reason in breakdown (legacy fields retained
            // for existing UI code; the authoritative source going forward is
            // the `advisor_signals` vector stamped below).
            //
            // `pipeline_score` is captured HERE (before any adjustment) so
            // the reconciler operates on the pure pipeline output, not a
            // mutated score. This matters because we may call this loop
            // multiple times in future (multi-advisor) and each advisor
            // must adjust off the same baseline.
            let pipeline_score = result.top_score;

            let advisor_signal = crate::types::AdvisorSignal {
                provider: advisor_identity.provider.clone(),
                model: advisor_identity.model.clone(),
                task: "judge".to_string(),
                raw_score: judgment.confidence,
                normalized_score: judgment.confidence, // No calibration yet (Phase 5)
                confidence: judgment.confidence,
                reason: if judgment.reasoning.is_empty() {
                    None
                } else {
                    Some(judgment.reasoning.clone())
                },
                prompt_version: Some(advisor_prompt_version.to_string()),
                calibration_id: advisor_calibration_id.clone(),
            };

            if let Some(ref mut breakdown) = result.score_breakdown {
                breakdown.llm_score = Some(judgment.confidence * 5.0); // Map back to 1-5
                breakdown.llm_reason = if judgment.reasoning.is_empty() {
                    None
                } else {
                    Some(judgment.reasoning.clone())
                };
                breakdown.advisor_signals.push(advisor_signal.clone());
            }

            // Queue a persisted provenance row for this judgment.
            provenance_rows.push(
                crate::provenance::Provenance::new(
                    crate::provenance::ArtifactKind::Rerank,
                    judgment.item_id.clone(),
                    &advisor_identity,
                    "judge",
                )
                .with_prompt_version(advisor_prompt_version),
            );

            if rerank_config.reconciler_enabled {
                // ── Phase 2 path: bounded reconciler ──────────────────
                // Pipeline is authoritative. Advisor can adjust by at most
                // ±ADVISOR_ADJUSTMENT_CAP (0.15). Disagreement becomes a
                // UI signal, never a score override. NO hard rejects.
                let signals = std::slice::from_ref(&advisor_signal);
                let reconciled = crate::reconciler::reconcile(pipeline_score, signals);
                result.top_score = reconciled.final_rank;

                if let Some(ref mut breakdown) = result.score_breakdown {
                    breakdown.disagreement = reconciled.disagreement;
                }

                if !judgment.reasoning.is_empty() {
                    result.explanation = Some(judgment.reasoning.clone());
                }

                // Honest telemetry: bucket by disagreement kind. No item is
                // "rejected" in this path — items the advisor dislikes are
                // surfaced at pipeline_score - 0.15, still visible, still
                // in the feed. Operators reading logs must not read a
                // "skeptical" count as "filtered out".
                match reconciled.disagreement {
                    None => reconciled_agreed += 1,
                    Some(crate::types::DisagreementKind::AdvisorSkeptical) => {
                        reconciled_skeptical += 1
                    }
                    Some(crate::types::DisagreementKind::AdvisorEnthusiastic) => {
                        reconciled_enthusiastic += 1
                    }
                    Some(crate::types::DisagreementKind::AdvisorsInternal) => {
                        reconciled_internal += 1
                    }
                }
            } else {
                // ── Legacy path: 50/50 blend + hard reject ────────────
                // Retained behind settings.rerank.reconciler_enabled=false
                // for debugging and A/B comparison during rollout.
                if judgment.relevant {
                    let blended =
                        (pipeline_score * 0.50 + judgment.confidence * 0.50).clamp(0.0, 1.0);
                    let signal_count = result
                        .score_breakdown
                        .as_ref()
                        .map_or(0, |b| b.signal_count);
                    result.top_score = if signal_count < 2 {
                        blended.min(0.55)
                    } else {
                        blended
                    };
                    if !judgment.reasoning.is_empty() {
                        result.explanation = Some(judgment.reasoning.clone());
                    }
                    confirmed += 1;
                } else {
                    result.relevant = false;
                    result.top_score *= 0.15;
                    result.explanation = Some(format!("Filtered: {}", judgment.reasoning));
                    rejected += 1;
                }
            }
        }
    }

    // Persist provenance rows. Non-fatal on failure: a DB error here should
    // not fail the rerank pass that already produced valid results.
    if !provenance_rows.is_empty() {
        let conn = db.conn.lock();
        match crate::provenance::record_batch(&conn, &provenance_rows) {
            Ok(ids) => {
                debug!(
                    target: "4da::rerank",
                    count = ids.len(),
                    "Recorded {} rerank provenance rows",
                    ids.len()
                );
            }
            Err(e) => {
                warn!(
                    target: "4da::rerank",
                    error = %e,
                    "Failed to record rerank provenance (non-fatal)"
                );
            }
        }
    }

    // Re-sort after LLM adjustments
    scoring::sort_results(results);

    // Track token usage for daily limits
    {
        let mut settings = get_settings_manager().lock();
        let cost = core.estimate_cost_cents(total_input, total_output);
        settings.record_usage(total_input + total_output, cost);
    }

    // Separate log shapes per path so downstream parsers don't need to
    // reconcile two different meanings for the same field. Every field is
    // zero in the path that doesn't apply — one line covers both cases.
    info!(target: "4da::rerank",
        judged = all_judgments.len(),
        reconciler_enabled = rerank_config.reconciler_enabled,
        // Reconciler-path buckets
        agreed = reconciled_agreed,
        skeptical = reconciled_skeptical,
        enthusiastic = reconciled_enthusiastic,
        internal_disagreement = reconciled_internal,
        // Legacy-path buckets
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
// Deduplication Utilities
// ============================================================================

/// Deduplicate items by normalized URL and normalized title.
/// Keeps the first occurrence (usually the oldest/original source).
pub(crate) fn dedup_stored_items(items: &[crate::db::StoredSourceItem]) -> Vec<usize> {
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
            detected_lang: "en".to_string(),
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
