// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Post-scoring quality processing — LLM reranking and digest generation.

#[path = "analysis_dedup.rs"]
mod analysis_dedup;
pub(crate) use analysis_dedup::*;

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

/// Fraction of the daily budget released up-front, before the day has elapsed,
/// so the first pass after the UTC rollover can actually run.
const BUDGET_PACE_HEADROOM: f64 = 0.05;

/// How much of the daily budget may be spent by this point in the UTC day.
///
/// Without pacing the budget is consumed as fast as the scheduler can spend it.
/// Measured on the live app 2026-08-14: the analysis loop runs every ~10.5min at
/// ~11.4k tokens per rerank, so a 100k/day budget was exhausted by 01:36Z — the
/// day resets at 00:00Z, meaning the flagship reranker was DEAD for 22.4 of
/// every 24 hours while the logs still said "LLM rerank phase complete".
/// Pacing converts "9 passes clustered in 96 minutes, then nothing" into
/// "~9 passes spread evenly across the day".
fn budget_allowance_by_now(limit: u64, secs_into_utc_day: u64) -> u64 {
    if limit == 0 {
        return u64::MAX; // 0 means "no limit" throughout this module
    }
    let elapsed_fraction = (secs_into_utc_day as f64 / 86_400.0).clamp(0.0, 1.0);
    let share = (elapsed_fraction + BUDGET_PACE_HEADROOM).min(1.0);
    (limit as f64 * share) as u64
}

fn secs_into_utc_day() -> u64 {
    use chrono::Timelike;
    let now = chrono::Utc::now();
    u64::from(now.hour()) * 3600 + u64::from(now.minute()) * 60 + u64::from(now.second())
}

/// Why a rerank pass did not run.
///
/// Every variant is logged by the caller. Previously ALL of these paths returned
/// a bare `None` — several with no log line whatsoever — and the caller printed
/// "LLM rerank phase complete elapsed_ms=0", which reads as success. A skipped
/// rerank is now indistinguishable from a real one only if you don't read the
/// logs, which is the opposite of the old behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RerankSkip {
    /// Disabled in settings, or no usable LLM provider/key configured.
    Disabled,
    /// Daily token or cost ceiling already reached.
    BudgetExhausted {
        tokens_today: u64,
        token_limit: u64,
        cost_today_cents: u64,
        cost_limit_cents: u64,
    },
    /// Budget intact, but spending now would burn the day's allowance early.
    BudgetPaced {
        tokens_today: u64,
        allowed_by_now: u64,
    },
    NoContext,
    NoDatabase,
    /// No item cleared `rerank.min_embedding_score`.
    NoCandidates,
    UnsupportedTier(String),
    /// Every LLM batch failed.
    NoJudgments,
    /// Judge returned one identical score for every item (2026-08-11 incident).
    NonDiscriminating,
}

impl RerankSkip {
    /// Stable machine-readable tag for log filtering.
    pub(crate) fn reason(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::BudgetExhausted { .. } => "budget_exhausted",
            Self::BudgetPaced { .. } => "budget_paced",
            Self::NoContext => "no_context",
            Self::NoDatabase => "no_database",
            Self::NoCandidates => "no_candidates",
            Self::UnsupportedTier(_) => "unsupported_model_tier",
            Self::NoJudgments => "all_batches_failed",
            Self::NonDiscriminating => "non_discriminating_judge",
        }
    }

    /// Human-readable detail, including the numbers that explain the decision.
    pub(crate) fn detail(&self) -> String {
        match self {
            Self::Disabled => "rerank disabled or no LLM configured".to_string(),
            Self::BudgetExhausted {
                tokens_today,
                token_limit,
                cost_today_cents,
                cost_limit_cents,
            } => format!(
                "daily budget spent: {tokens_today}/{token_limit} tokens, {cost_today_cents}/{cost_limit_cents} cents — resets at 00:00 UTC"
            ),
            Self::BudgetPaced {
                tokens_today,
                allowed_by_now,
            } => format!(
                "pacing the daily budget: {tokens_today} tokens used, {allowed_by_now} allowed by this point in the UTC day"
            ),
            Self::NoContext => "no user context available to rank against".to_string(),
            Self::NoDatabase => "database unavailable".to_string(),
            Self::NoCandidates => {
                "no item cleared rerank.min_embedding_score this pass".to_string()
            }
            Self::UnsupportedTier(tier) => {
                format!("model tier '{tier}' cannot produce structured judgments")
            }
            Self::NoJudgments => "every LLM batch failed".to_string(),
            Self::NonDiscriminating => {
                "judge returned an identical score for every item — pass discarded".to_string()
            }
        }
    }
}

/// Result of a rerank attempt. `Skipped` carries WHY, so no caller can report a
/// no-op as a completed phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RerankOutcome {
    Reranked { judged: usize },
    Skipped(RerankSkip),
}

impl RerankOutcome {
    /// Emit the single honest log line for this outcome.
    pub(crate) fn log(&self, elapsed_ms: u128, phase: &str) {
        match self {
            Self::Reranked { judged } => {
                info!(
                    target: "4da::rerank",
                    phase,
                    judged,
                    elapsed_ms,
                    "LLM rerank applied"
                );
            }
            Self::Skipped(skip) => {
                warn!(
                    target: "4da::rerank",
                    phase,
                    reason = skip.reason(),
                    detail = %skip.detail(),
                    elapsed_ms,
                    "LLM rerank SKIPPED — items carry pipeline scores only, no LLM judgment"
                );
            }
        }
    }
}

/// Apply LLM reranking to scored results if enabled and within limits.
/// Uses smaller batches (8 items) with real article content for accurate judging.
pub(crate) async fn apply_llm_reranking(
    app: &tauri::AppHandle,
    results: &mut [SourceRelevance],
    scoring_ctx: &scoring::ScoringContext,
) -> RerankOutcome {
    let (enabled, within_limits, usage, rerank_config) = {
        let mut settings = get_settings_manager().lock();
        let enabled = settings.is_rerank_enabled();
        // Call unconditionally: this also performs the UTC day-rollover reset.
        let within_limits = settings.within_daily_limits();
        let usage = settings.get_usage().clone();
        let config = settings.get().rerank.clone();
        (enabled, within_limits, usage, config)
    };

    if !enabled {
        return RerankOutcome::Skipped(RerankSkip::Disabled);
    }

    if !within_limits {
        return RerankOutcome::Skipped(RerankSkip::BudgetExhausted {
            tokens_today: usage.tokens_today,
            token_limit: rerank_config.daily_token_limit,
            cost_today_cents: usage.cost_today_cents,
            cost_limit_cents: rerank_config.daily_cost_limit_cents,
        });
    }

    let allowed_by_now =
        budget_allowance_by_now(rerank_config.daily_token_limit, secs_into_utc_day());
    if usage.tokens_today >= allowed_by_now {
        return RerankOutcome::Skipped(RerankSkip::BudgetPaced {
            tokens_today: usage.tokens_today,
            allowed_by_now,
        });
    }

    let context_summary = build_rerank_context_summary(scoring_ctx);
    if context_summary.is_empty() {
        return RerankOutcome::Skipped(RerankSkip::NoContext);
    }

    // Get database for content snippets
    let db = match get_database() {
        Ok(db) => db,
        Err(_) => return RerankOutcome::Skipped(RerankSkip::NoDatabase),
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
        return RerankOutcome::Skipped(RerankSkip::NoCandidates);
    }

    let llm_settings = {
        let settings = get_settings_manager().lock();
        settings.get().llm.clone()
    };

    // Gate: skip reranking for Basic-tier models (small local models that
    // can't reliably produce structured JSON judgments). They still get
    // pipeline scoring and heuristic explanations from scoring/explanation.rs.
    let tier = crate::llm_capability::get_model_tier(&llm_settings);
    if !tier.supports_reranking() {
        return RerankOutcome::Skipped(RerankSkip::UnsupportedTier(tier.to_string()));
    }

    // Construct the advisory core. It carries its own ModelIdentity and
    // prompt_version so every AdvisorSignal and provenance row this rerank
    // pass writes share a single source of truth. Intelligence Mesh Phase 4
    // — the trait exists so Phases 5 (calibration wrap) and 6 (shadow arena)
    // can swap the impl without re-plumbing this loop.
    // See `docs/strategy/INTELLIGENCE-MESH.md` §2 Layer 2.
    // Phase 5b.1 — calibration persistence wire-through.
    //
    // Construction order:
    //   1. Build the raw LlmJudgeCore (wraps the legacy RelevanceJudge)
    //   2. Compute its identity hash + look up a persisted curve for
    //      (identity_hash, "judge")
    //   3. Wrap with CalibratedCore. When no curve exists, CalibratedCore
    //      is a transparent pass-through — matches pre-mesh behavior
    //      exactly, so zero risk. When a curve exists, it applies and
    //      overrides calibration_id on each Validated response.
    //
    // The fitter that PRODUCES curves (Phase 5b.2) is not yet built, so in
    // practice every rerank today is pass-through. This commit lands the
    // architectural wire so 5b.2 becomes "add a fitter", not "wire a
    // fitter + refactor the whole rerank loop."
    let inner_core: Box<dyn crate::intelligence_core::IntelligenceCore> =
        Box::new(crate::intelligence_core::LlmJudgeCore::new(llm_settings));
    let identity_hash = inner_core.identity().hash();
    // Drift-aware load: returns None-curve (pass-through) if the stored
    // curve's prompt_version no longer matches the core's current
    // prompt_version. Model-hash drift is handled implicitly by the
    // identity_hash filename — a swapped model looks up a different file.
    //
    // Phase 5b.2+ drift alarm: when drift is detected, emit a Tauri
    // event so the UI can toast the user ("Your llama3.2 calibration
    // expired — recalibrating from next 50 feedback events"). Without
    // this, drift invalidation is silent and users never know their
    // model's scoring semantics changed underneath them.
    let curve_load = crate::calibration_store::load_current_curve_detailed(
        &identity_hash,
        "judge",
        inner_core.prompt_version(),
    );
    if let Some(drift) = &curve_load.drift {
        if let Err(e) = tauri::Emitter::emit(app, "calibration-drift", drift) {
            tracing::debug!(
                target: "4da::rerank",
                error = %e,
                "Failed to emit calibration-drift event (non-fatal)"
            );
        }
        tracing::warn!(
            target: "4da::rerank",
            curve_id = %drift.curve_id,
            task = %drift.task,
            stored_prompt = %drift.stored_prompt_version,
            current_prompt = %drift.current_prompt_version,
            "Calibration curve invalidated by prompt drift — emitted UI event"
        );
    }
    let core: Box<dyn crate::intelligence_core::IntelligenceCore> = Box::new(
        crate::calibration::CalibratedCore::new(inner_core, curve_load.curve),
    );
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
        return RerankOutcome::Skipped(RerankSkip::NoJudgments);
    }

    // Circuit breaker (2026-08-11 incident): a judge whose scores are ALL
    // identical across a full pass is not discriminating — either the model
    // output is broken or a transform (e.g. a degenerate calibration curve)
    // flattened it. Applying ±0.15 adjustments from a non-discriminating
    // advisor moves the whole feed uniformly, and persisting its samples
    // poisons the next curve fit. Discard the pass entirely.
    if rerank_pass_is_uniform(&all_judgments) {
        warn!(
            target: "4da::rerank",
            judged = all_judgments.len(),
            uniform_confidence = all_judgments[0].confidence,
            "LLM judge returned one identical score for every item — discarding rerank pass (non-discriminating advisor)"
        );
        return RerankOutcome::Skipped(RerankSkip::NonDiscriminating);
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

    // Phase 5b.2: calibration samples — one row per (source_item, signal).
    // Paired later with interactions/feedback by the fitter to derive
    // binary labels and fit a calibration curve.
    let mut calibration_samples_batch: Vec<(i64, crate::types::AdvisorSignal)> =
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
                identity_hash: Some(identity_hash.clone()),
                task: "judge".to_string(),
                // raw_score must be the model's PRE-curve confidence: the
                // fitter pairs raw_score with outcomes to fit the NEXT
                // curve, and feeding it post-curve values trains the curve
                // on its own output (2026-08-11 incident).
                raw_score: judgment.raw_confidence.unwrap_or(judgment.confidence),
                normalized_score: judgment.confidence,
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

            // Phase 5b.2: queue a calibration sample. Provenance records
            // WHICH model judged the item; this table records WHAT score
            // it gave — the fitter needs the score to produce a curve.
            // We use `result.id` (the source_items row id) rather than
            // `judgment.item_id` (the string form) so the fitter can
            // join directly to `interactions.source_item_id` without
            // parsing.
            calibration_samples_batch.push((result.id as i64, advisor_signal.clone()));

            if rerank_config.reconciler_enabled {
                // ── Phase 2 path: bounded reconciler ──────────────────
                // Pipeline is authoritative. Advisor can adjust by at most
                // ±ADVISOR_ADJUSTMENT_CAP (0.15). Disagreement becomes a
                // UI signal, never a score override. NO hard rejects.
                let signals = std::slice::from_ref(&advisor_signal);
                let reconciled = crate::reconciler::reconcile(pipeline_score, signals);
                result.top_score = reconciled.final_rank;

                // LLM hard floor: when the advisor strongly disagrees AND the item lacks
                // genuine user-facing signal (high interest or context), the pipeline's
                // mechanical dep matching shouldn't override human-calibrated judgment.
                // This is NOT a general override — only fires when LLM confidence < 0.30
                // (score < 1.5/5) AND the item relies primarily on dependency matching.
                if !judgment.relevant && judgment.confidence < 0.30 {
                    let has_strong_user_signal = result
                        .score_breakdown
                        .as_ref()
                        .map_or(false, |b| b.interest_score > 0.50 || b.context_score > 0.30);
                    if !has_strong_user_signal {
                        result.relevant = false;
                    }
                }

                if let Some(ref mut breakdown) = result.score_breakdown {
                    breakdown.disagreement = reconciled.disagreement;
                }

                if !judgment.reasoning.is_empty()
                    && judgment.reasoning != "No judgment provided by LLM"
                {
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
                    if !judgment.reasoning.is_empty()
                        && judgment.reasoning != "No judgment provided by LLM"
                    {
                        result.explanation = Some(judgment.reasoning.clone());
                    }
                    confirmed += 1;
                } else {
                    result.relevant = false;
                    result.top_score *= 0.15;
                    if judgment.reasoning != "No judgment provided by LLM" {
                        result.explanation = Some(format!("Filtered: {}", judgment.reasoning));
                    }
                    rejected += 1;
                }
            }
        }
    }

    // Persist provenance rows + calibration samples under a single lock
    // acquisition. Non-fatal on failure: DB errors here should not fail
    // the rerank pass that already produced valid results.
    if !provenance_rows.is_empty() || !calibration_samples_batch.is_empty() {
        let conn = db.conn.lock();

        if !provenance_rows.is_empty() {
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

        // Stamp calibration samples grouped by source_item_id so the
        // helper transaction covers each item's signals atomically. In
        // practice today the vector is 1-signal-per-item (single advisor);
        // this grouping is future-proof for the multi-advisor case.
        if !calibration_samples_batch.is_empty() {
            let identity_hash = advisor_identity.hash();
            let mut total_stamped = 0usize;
            let mut errored = 0usize;

            // Partition by source_item_id.
            let mut by_item: std::collections::BTreeMap<i64, Vec<crate::types::AdvisorSignal>> =
                std::collections::BTreeMap::new();
            for (item_id, sig) in calibration_samples_batch.drain(..) {
                by_item.entry(item_id).or_default().push(sig);
            }

            for (item_id, sigs) in by_item {
                match crate::calibration_samples::stamp_signals(
                    &conn,
                    item_id,
                    &identity_hash,
                    &sigs,
                ) {
                    Ok(n) => total_stamped += n,
                    Err(e) => {
                        errored += 1;
                        warn!(
                            target: "4da::rerank",
                            source_item_id = item_id,
                            error = %e,
                            "Failed to stamp calibration samples (non-fatal)"
                        );
                    }
                }
            }

            if total_stamped > 0 || errored > 0 {
                debug!(
                    target: "4da::rerank",
                    stamped = total_stamped,
                    errored,
                    "Stamped calibration samples"
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

    RerankOutcome::Reranked {
        judged: all_judgments.len(),
    }
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

/// True when a full rerank pass produced one identical confidence for every
/// judgment — a non-discriminating advisor. 2026-08-11 incident: a degenerate
/// calibration curve flattened every honest judge score to 1.0; the
/// reconciler then inflated all 48 judged items by +0.15 every cycle and the
/// pass re-persisted its own output as training samples. Small passes (< 8)
/// are exempt: genuine uniformity is plausible there.
pub(crate) fn rerank_pass_is_uniform(judgments: &[crate::llm::RelevanceJudgment]) -> bool {
    if judgments.len() < 8 {
        return false;
    }
    let mut lo = f32::MAX;
    let mut hi = f32::MIN;
    for j in judgments {
        lo = lo.min(j.confidence);
        hi = hi.max(j.confidence);
    }
    (hi - lo) < 1e-6
}

#[cfg(test)]
#[path = "analysis_rerank_tests.rs"]
mod rerank_breaker_tests;
