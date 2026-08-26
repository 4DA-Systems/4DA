// SPDX-License-Identifier: FSL-1.1-Apache-2.0

//! LLM judgment engine — evaluates source items at ingestion time for Tier 2 intelligence.
//!
//! After ingestion, items scoring above a threshold are sent to the user's configured LLM
//! for relevance evaluation with full ACE context. Judgments are stored in `llm_judgments`
//! and later read by preemption/blind_spots feeds.
//!
//! Prompt v2 (2026-08-24, scoring-audit fix queue item 25) widens the SAME batched
//! call to also yield the deep-content-analysis fields the scoring pipeline's read
//! side has consumed since Phase 41 without any writer ever existing
//! (`content_analysis::get_cached_analysis` → `analysis_to_multiplier`, plus the
//! Stretch bucket in feed composition). One call, two lanes lit.
//!
//! [`run_post_cycle_llm_passes`] is the scheduled/headless entry point:
//! judge → content-analysis upsert → demote-only verdict feedback, all gated by
//! the BYOK provider and the daily token/cost budget. No provider or budget
//! exhausted → silent no-op with a debug log — Accurate-first doctrine: never
//! fake intelligence the system can't stand behind.

use crate::db::{Database, VerdictReason, VerdictSource};
use crate::error::Result;
use crate::llm::{LLMClient, Message};
use crate::settings::LLMProvider;
use serde::Deserialize;
use tracing::{debug, info, warn};

/// Bumped v1 → v2 when the batch schema gained the content-analysis fields
/// (`technical_depth`/`novelty`/`audience_level`/`key_insight`). v1 judgments
/// remain valid (`get_unjudged_item_ids` joins on ANY judgment, so nothing is
/// re-judged and re-billed on upgrade); only v2 judgments drive demotions.
const PROMPT_VERSION: &str = "v2";
const INGESTION_THRESHOLD: f64 = 0.25;
const BATCH_SIZE: usize = 5;

/// Demote-only verdict feedback: judged relevance strictly below this…
const DEMOTION_RELEVANCE_BELOW: f64 = 0.25;
/// …with judge confidence at or above this…
const DEMOTION_CONFIDENCE_MIN: f64 = 0.7;
/// …capped per pass, so a systematically mis-calibrated judge cannot gut the
/// curated feed in a single run (10 ≈ 2% of the measured 532-member live feed).
const DEMOTION_CAP_PER_RUN: usize = 10;
/// Only judgments this fresh may demote. A judgment is written once and never
/// re-made, so without a window an old low read would fight every future
/// pipeline brain that re-curates the item, forever. Mirrors the 7-day
/// ingestion window in `get_unjudged_item_ids`.
const DEMOTION_JUDGMENT_WINDOW_DAYS: u32 = 7;

/// Diagnostic probe bounds — deliberately NOT demotion thresholds. Nothing is
/// demoted at these values; they exist only to answer the question a zero
/// cannot: did the gate match nothing because the judge AGREES with the feed,
/// or because it is calibrated past the judge's actual output distribution?
///
/// Measured 2026-08-27, and the reason this probe exists: the gate above had
/// demoted ZERO items on a 520-item feed while the judge disputed 162 of the
/// 324 feed items it had assessed. Both thresholds miss independently — the
/// judge's rejections cluster at EXACTLY 0.25 and 0.30 against a strict
/// `< 0.25`, and it emits low confidence precisely BECAUSE it is rejecting
/// (every one of ten axios advisories it scored 0.22-0.35 carried confidence
/// 0.45-0.60). At these probe bounds, 61 feed items would have qualified.
///
/// Retuning the real thresholds is a feed-visibility decision for the operator,
/// not something to infer from one corpus. Making the silence audible is not.
const DEMOTION_PROBE_RELEVANCE_BELOW: f64 = 0.40;
const DEMOTION_PROBE_CONFIDENCE_MIN: f64 = 0.6;

#[derive(Debug, Deserialize)]
struct JudgmentResponse {
    relevance: Option<f64>,
    explanation: Option<String>,
    actions: Option<Vec<String>>,
    confidence: Option<f64>,
    /// Content-analysis fields (prompt v2). All optional: the model is
    /// instructed to OMIT them when the content preview is too thin to judge,
    /// and parsing must never fail because a field is missing.
    technical_depth: Option<f64>,
    novelty: Option<f64>,
    audience_level: Option<String>,
    key_insight: Option<String>,
}

/// Evaluate a batch of source items and store judgments.
/// Called after ingestion when new items arrive (deep-scan path), and by
/// [`run_post_cycle_llm_passes`] on the scheduled/headless cadence.
pub(crate) async fn evaluate_pending_items(db: &Database) -> Result<usize> {
    if crate::state::is_llm_limit_reached() {
        debug!(target: "4da::llm_judgments", "LLM daily limit reached, skipping judgment batch");
        return Ok(0);
    }

    let Some(provider) = get_llm_settings() else {
        debug!(target: "4da::llm_judgments", "No LLM provider configured, skipping judgments");
        return Ok(0);
    };

    Ok(evaluate_with_provider(db, provider).await?.judged)
}

// ============================================================================
// Post-cycle passes (scheduled + headless entry point)
// ============================================================================

/// Outcome of one post-cycle LLM pass. Consumed by log lines and tests only —
/// no UI surface (Intelligence Doctrine rule 3: a number nobody can act on is
/// not displayed).
#[derive(Debug, Default)]
pub(crate) struct PostCycleLlmSummary {
    pub judged: usize,
    pub analyses_stored: usize,
    pub demoted: usize,
    /// Why the pass was a no-op (`"llm_budget_reached"` / `"no_llm_provider"`),
    /// if it was.
    pub skipped: Option<&'static str>,
}

/// Post-cycle LLM passes: judge the top-band unjudged items (one batched call
/// per 5 items yields the judgment AND the content-analysis fields), then
/// apply demote-only verdict feedback for curated items the judge confidently
/// rejects. Budget-safe and BYOK-gated: daily token/cost limit reached or no
/// configured provider → silent no-op (debug log only).
///
/// Wire-in sites (seams, mirroring the deep-scan Tier-2 pattern at
/// `analysis_deep_scan.rs:468-476`): the scheduled 30-min cycle
/// (`app_setup::run_scheduled_analysis`, spawned non-blocking) and the
/// headless engine (`headless::run_cycle`, awaited inline — a `--once`
/// process exits before a detached task would run).
// REMOVE the allow when the app_setup/headless seams land (fix-queue item 25
// wiring; the seam lines are in this function's doc above).
pub(crate) async fn run_post_cycle_llm_passes(db: &Database) -> PostCycleLlmSummary {
    run_post_cycle_with(db, crate::state::is_llm_limit_reached(), get_llm_settings()).await
}

/// Gate-injectable inner pass. Hermetic tests drive the budget/BYOK gates
/// directly instead of mutating the process-global daily counters or the
/// settings manager (both shared across the parallel test harness).
async fn run_post_cycle_with(
    db: &Database,
    llm_limit_reached: bool,
    provider: Option<LLMProvider>,
) -> PostCycleLlmSummary {
    let mut summary = PostCycleLlmSummary::default();

    if llm_limit_reached {
        debug!(target: "4da::llm_judgments", "LLM daily limit reached — skipping post-cycle LLM passes");
        summary.skipped = Some("llm_budget_reached");
        return summary;
    }
    let Some(provider) = provider else {
        debug!(target: "4da::llm_judgments", "No LLM provider configured — skipping post-cycle LLM passes");
        summary.skipped = Some("no_llm_provider");
        return summary;
    };

    match evaluate_with_provider(db, provider).await {
        Ok(outcome) => {
            summary.judged = outcome.judged;
            summary.analyses_stored = outcome.analyses_stored;
        }
        Err(e) => {
            warn!(target: "4da::llm_judgments", error = %e, "Post-cycle judgment pass failed")
        }
    }

    match apply_judgment_demotions(db, DEMOTION_CAP_PER_RUN) {
        Ok(n) => summary.demoted = n,
        Err(e) => {
            warn!(target: "4da::llm_judgments", error = %e, "Post-cycle demotion pass failed")
        }
    }

    if summary.judged > 0 || summary.demoted > 0 {
        info!(
            target: "4da::llm_judgments",
            judged = summary.judged,
            analyses = summary.analyses_stored,
            demoted = summary.demoted,
            "Post-cycle LLM passes complete"
        );
    }
    summary
}

// ============================================================================
// Judge pass
// ============================================================================

struct JudgeOutcome {
    judged: usize,
    analyses_stored: usize,
}

async fn evaluate_with_provider(db: &Database, provider: LLMProvider) -> Result<JudgeOutcome> {
    let mut outcome = JudgeOutcome {
        judged: 0,
        analyses_stored: 0,
    };

    let unjudged = db
        .get_unjudged_item_ids(INGESTION_THRESHOLD, BATCH_SIZE * 4)
        .map_err(|e| {
            crate::error::FourDaError::Internal(format!("Failed to get unjudged items: {e}"))
        })?;
    if unjudged.is_empty() {
        return Ok(outcome);
    }

    let model_name = provider.model.clone();
    let client = LLMClient::new(provider);
    let user_context = crate::adversarial::build_user_context_summary();

    for chunk in unjudged.chunks(BATCH_SIZE) {
        let items = load_items_for_judgment(db, chunk)?;
        if items.is_empty() {
            continue;
        }

        match evaluate_batch(&client, &items, &user_context).await {
            Ok(results) => {
                let (judged, analyses) = store_batch_results(db, &items, results, &model_name);
                outcome.judged += judged;
                outcome.analyses_stored += analyses;
            }
            Err(e) => {
                warn!(target: "4da::llm_judgments", error = %e, "Batch evaluation failed");
                break;
            }
        }
    }

    if outcome.judged > 0 {
        info!(
            target: "4da::llm_judgments",
            judged = outcome.judged,
            analyses = outcome.analyses_stored,
            "Stored LLM judgments for ingested items"
        );
    }

    Ok(outcome)
}

/// Store one parsed batch: upsert the judgment row for every response and —
/// when the model produced the content-analysis fields and the item has real
/// content — upsert the `content_analyses` row the scoring read side keys on.
/// Returns `(judgments_stored, analyses_stored)`.
fn store_batch_results(
    db: &Database,
    items: &[ItemForJudgment],
    results: Vec<(i64, JudgmentResponse)>,
    model_name: &str,
) -> (usize, usize) {
    let mut judged = 0;
    let mut analyses = 0;

    for (item_id, response) in results {
        let relevance = response.relevance.unwrap_or(0.0).clamp(0.0, 1.0);
        let explanation = response.explanation.clone().unwrap_or_default();
        let confidence = response.confidence.unwrap_or(0.5).clamp(0.0, 1.0);
        let actions_json = response
            .actions
            .as_ref()
            .map(|a| serde_json::to_string(a).unwrap_or_default());

        if let Err(e) = db.upsert_llm_judgment(
            item_id,
            relevance,
            &explanation,
            actions_json.as_deref(),
            confidence,
            model_name,
            PROMPT_VERSION,
        ) {
            warn!(target: "4da::llm_judgments", error = %e, item_id, "Failed to store judgment");
            continue;
        }
        judged += 1;

        if let Some(analysis) = analysis_from_response(items, item_id, &response) {
            match crate::content_analysis::store_analysis(db, item_id, &analysis) {
                Ok(()) => analyses += 1,
                Err(e) => {
                    warn!(target: "4da::llm_judgments", error = %e, item_id, "Failed to store content analysis")
                }
            }
        }
    }

    (judged, analyses)
}

/// Build a `ContentAnalysis` from a judgment response, or `None` when the
/// model omitted the analysis fields (it is instructed to omit rather than
/// guess when the text is too thin) or the item has no content to key the
/// cache on. The hash is computed over the item's FULL stored content —
/// exactly what the pipeline read side hashes (`pipeline_v2` hashes
/// `input.content`, which is the same `source_items.content` column).
fn analysis_from_response(
    items: &[ItemForJudgment],
    item_id: i64,
    response: &JudgmentResponse,
) -> Option<crate::content_analysis::ContentAnalysis> {
    let item = items.iter().find(|i| i.id == item_id)?;
    let content = item.content.as_deref()?;
    if content.trim().is_empty() {
        // Every empty body hashes to the same digest; one row would
        // cross-contaminate every content-less item's multiplier.
        return None;
    }
    let technical_depth = to_scale_1_5(response.technical_depth)?;
    let novelty = to_scale_1_5(response.novelty)?;
    // Absent audience → Intermediate: the neutral 1.0 multiplier, never a
    // fabricated boost or penalty.
    let audience_level = response
        .audience_level
        .as_deref()
        .map(crate::content_analysis::AudienceLevel::from_str_lossy)
        .unwrap_or(crate::content_analysis::AudienceLevel::Intermediate);

    Some(crate::content_analysis::ContentAnalysis {
        technical_depth,
        novelty,
        audience_level,
        key_insight: response
            .key_insight
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        content_hash: crate::content_analysis::content_hash(content),
        analyzed_at: String::new(), // stamped by the DB at write time
    })
}

/// Clamp a model-reported 1-5 scale value into range; non-finite → `None`
/// (treated as omitted, never fabricated).
fn to_scale_1_5(v: Option<f64>) -> Option<u8> {
    let v = v?;
    if !v.is_finite() {
        return None;
    }
    Some(v.round().clamp(1.0, 5.0) as u8)
}

// ============================================================================
// Demote-only verdict feedback (Tier 2 → Tier 1)
// ============================================================================

/// Demote curated items (`feed_relevant = 1`) whose fresh judgment under the
/// current prompt version is BOTH clearly irrelevant and confident, via the
/// canonical persist boundary (`persist_feed_verdicts_with_reasons`), stamped
/// [`VerdictReason::LlmReject`].
///
/// This is the precision lever the embedding pipeline cannot provide: a
/// tutorial that name-drops the user's whole stack, or an announcement shaped
/// like a release, rides similarity into the feed — the judge reads it for
/// what it is.
///
/// Bounds, mirroring `Database::demote_sunk_verdicts` doctrine:
/// - demote-only — promotion needs a full run's dedup/diversity context;
/// - serendipity verdicts are immune (excluded in the candidate query):
///   anti-bubble picks are SUPPOSED to look irrelevant to a relevance judge,
///   and demoting them here would silently delete the feature;
/// - judgments older than [`DEMOTION_JUDGMENT_WINDOW_DAYS`] never demote;
/// - capped at `cap` per pass, each demotion logged with title + judged
///   relevance;
/// - convergent: a demoted row fails `feed_relevant = 1` on the next pass.
fn apply_judgment_demotions(db: &Database, cap: usize) -> Result<usize> {
    let candidates = db.get_llm_reject_candidates(
        PROMPT_VERSION,
        DEMOTION_RELEVANCE_BELOW,
        DEMOTION_CONFIDENCE_MIN,
        DEMOTION_JUDGMENT_WINDOW_DAYS,
        cap,
    )?;
    if candidates.is_empty() {
        // A zero here is ambiguous, and an ambiguous zero is how a gate that has
        // never fired stays invisible. Probe one notch looser and say which case
        // this is — same doctrine as DEP_SCOPE_DEGRADED: a mechanism that
        // silently does nothing must be distinguishable from one that correctly
        // found nothing to do.
        let probe = db
            .get_llm_reject_candidates(
                PROMPT_VERSION,
                DEMOTION_PROBE_RELEVANCE_BELOW,
                DEMOTION_PROBE_CONFIDENCE_MIN,
                DEMOTION_JUDGMENT_WINDOW_DAYS,
                cap,
            )
            .unwrap_or_default();
        if !probe.is_empty() {
            warn!(
                target: "4da::llm_judgments",
                gate_relevance_below = DEMOTION_RELEVANCE_BELOW,
                gate_confidence_min = DEMOTION_CONFIDENCE_MIN,
                probe_relevance_below = DEMOTION_PROBE_RELEVANCE_BELOW,
                probe_confidence_min = DEMOTION_PROBE_CONFIDENCE_MIN,
                would_demote_at_probe = probe.len(),
                "LLM demotion gate matched nothing, but {} curated item(s) qualify one notch looser — the gate may be calibrated past the judge's output distribution",
                probe.len()
            );
        } else {
            debug!(
                target: "4da::llm_judgments",
                "LLM demotion gate matched nothing and nothing qualifies at the probe bounds either — the judge agrees with the curated feed"
            );
        }
        return Ok(0);
    }

    let verdicts: Vec<(i64, bool, VerdictSource, Option<VerdictReason>)> = candidates
        .iter()
        .map(|c| {
            (
                c.item_id,
                false,
                VerdictSource::Score,
                Some(VerdictReason::LlmReject),
            )
        })
        .collect();
    let demoted =
        db.persist_feed_verdicts_with_reasons(&verdicts, crate::scoring::PIPELINE_VERSION)?;

    for c in &candidates {
        let title_preview: String = c.title.chars().take(80).collect();
        info!(
            target: "4da::llm_judgments",
            item_id = c.item_id,
            judged_relevance = c.judged_relevance,
            confidence = c.confidence,
            title = %title_preview,
            "LLM judge demoted curated item (llm_reject)"
        );
    }

    Ok(demoted)
}

// ============================================================================
// Internal Types
// ============================================================================

struct ItemForJudgment {
    id: i64,
    title: String,
    content: Option<String>,
    source_type: String,
    relevance_score: f64,
}

// ============================================================================
// Helpers
// ============================================================================

/// Get the LLM provider from settings.
/// Returns `None` if no provider is configured or the API key is missing
/// (for non-Ollama providers).
fn get_llm_settings() -> Option<LLMProvider> {
    let mgr = crate::get_settings_manager();
    let mut guard = mgr.lock();
    guard.ensure_keys_hydrated();
    let provider = guard.get().llm.clone();

    if provider.provider != "ollama" && provider.api_key.is_empty() {
        return None;
    }

    Some(provider)
}

fn load_items_for_judgment(db: &Database, ids: &[i64]) -> Result<Vec<ItemForJudgment>> {
    let conn = db.conn.lock();
    let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT id, title, content, source_type, COALESCE(relevance_score, 0.0)
         FROM source_items WHERE id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| {
        crate::error::FourDaError::Internal(format!("Failed to prepare query: {e}"))
    })?;

    let params: Vec<&dyn rusqlite::types::ToSql> = ids
        .iter()
        .map(|id| id as &dyn rusqlite::types::ToSql)
        .collect();
    let rows = stmt
        .query_map(params.as_slice(), |row| {
            Ok(ItemForJudgment {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                source_type: row.get(3)?,
                relevance_score: row.get(4)?,
            })
        })
        .map_err(|e| crate::error::FourDaError::Internal(format!("Failed to query items: {e}")))?;

    let mut items = Vec::new();
    for row in rows {
        match row {
            Ok(item) => items.push(item),
            Err(e) => warn!(target: "4da::llm_judgments", error = %e, "Failed to read item row"),
        }
    }
    Ok(items)
}

async fn evaluate_batch(
    client: &LLMClient,
    items: &[ItemForJudgment],
    user_context: &str,
) -> Result<Vec<(i64, JudgmentResponse)>> {
    let items_block: String = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let content_preview = item
                .content
                .as_deref()
                .filter(|c| !c.is_empty())
                .map(|c| {
                    let truncated: String = c.chars().take(500).collect();
                    format!("\nContent: {truncated}")
                })
                .unwrap_or_default();
            format!(
                "--- Item {} (id={}) ---\nTitle: {}\nSource: {}\nScore: {:.2}{content_preview}",
                i + 1,
                item.id,
                item.title,
                item.source_type,
                item.relevance_score
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let system_prompt = format!(
        "You are an intelligence relevance evaluator for a developer tool. \
         Evaluate each item's relevance to this specific user.\n\n\
         {user_context}\n\n\
         For each item, respond with a JSON array where each element has:\n\
         - \"id\": the item id\n\
         - \"relevance\": 0.0-1.0 (how relevant to THIS user specifically)\n\
         - \"explanation\": one sentence explaining WHY this matters to this user \
           (must reference a specific fact from the item AND the user's context)\n\
         - \"actions\": array of suggested actions (e.g. [\"review_security\", \"investigate\"])\n\
         - \"confidence\": 0.0-1.0 (how confident you are in your relevance assessment)\n\
         - \"technical_depth\": integer 1-5 (1 = announcement/listicle-level, \
           5 = deep implementation detail)\n\
         - \"novelty\": integer 1-5 (1 = rehash of well-known material, \
           5 = genuinely new technique or result)\n\
         - \"audience_level\": one of \"Beginner\", \"Intermediate\", \"Advanced\", \"Expert\"\n\
         - \"key_insight\": one-sentence key technical insight from the content, or null\n\n\
         Rules:\n\
         - Explanation MUST reference something specific from the item (a package name, CVE, etc.) \
           AND something from the user's context\n\
         - Generic explanations like \"relevant to your interests\" score 0 confidence\n\
         - If the item has no clear connection to the user's stack/topics, relevance should be < 0.3\n\
         - A package only affects the user if it is actually in their stack/dependencies. Do NOT claim cross-ecosystem impact (e.g. a JavaScript/npm package affecting a Rust backend, or vice-versa). If you cannot confirm it is in their stack, relevance < 0.3\n\
         - Judge technical_depth/novelty/audience_level ONLY from the provided text; if the \
           content preview is too thin to judge them, OMIT those fields entirely rather than guessing\n\
         - Return ONLY a valid JSON array, no other text"
    );

    let user_msg = format!("Evaluate these items:\n\n{items_block}");

    let messages = vec![Message {
        role: "user".to_string(),
        content: user_msg,
    }];

    let response = client.complete(&system_prompt, messages).await?;
    parse_batch_response(&response.content)
}

/// Parse the model's batched JSON reply into per-item judgments.
///
/// Tolerates a markdown code fence and elements with missing fields (all
/// fields are `Option`); elements without an `id` are dropped — there is
/// nothing to attach them to. A reply that is not a JSON array at all is an
/// error (the caller logs and stops the batch loop).
fn parse_batch_response(text: &str) -> Result<Vec<(i64, JudgmentResponse)>> {
    let trimmed = text.trim();
    // Extract JSON from a potential markdown code block
    let json_text = if trimmed.starts_with("```") {
        trimmed
            .lines()
            .skip(1)
            .take_while(|l| !l.starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        trimmed.to_string()
    };

    #[derive(Deserialize)]
    struct BatchItem {
        id: Option<i64>,
        #[serde(flatten)]
        judgment: JudgmentResponse,
    }

    let parsed: Vec<BatchItem> = serde_json::from_str(&json_text)?;

    Ok(parsed
        .into_iter()
        .filter_map(|bi| Some((bi.id?, bi.judgment)))
        .collect())
}

#[cfg(test)]
#[path = "llm_judgments_tests.rs"]
mod tests;
