// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Cheap relevance triage — the first stage of the scoring funnel.
//!
//! The goal of 4DA is NOT to fully score every item. It is to **never miss what's
//! relevant while spending expensive compute only where it can change the answer.**
//! This module is the cheap gate that makes that tractable: for an item we already
//! have an embedding for, deciding "is this even in the user's universe?" is a couple
//! of dot products and a dependency-name check — microseconds, no LLM, no DB writes.
//!
//! ## Design contract — HIGH RECALL, deferral not deletion
//! A gate that drops a *relevant* item is a silent false negative: invisible in
//! production and the worst possible failure for an intelligence product. So:
//!
//! 1. The gate is tuned for **recall, not precision** — when in doubt, KEEP. Letting
//!    noise through is fine; stage 2 (full PASIFA scoring) filters it. Dropping
//!    something relevant is not fine.
//! 2. Dropped items are **DEFERRED, never deleted.** Relevance is a function of
//!    (item × user-state × time): a release of a crate you don't use yet is noise
//!    today and critical the day you adopt it. The product promise — "yesterday's
//!    noise becomes tomorrow's signal" — *requires* that deferred items stay
//!    re-examinable when the profile changes or a high-signal event fires.
//! 3. **High-stakes categories are never gated out** (error-cost asymmetry): missing a
//!    security advisory or breaking change for your stack is catastrophic; showing one
//!    extra is trivial. Those bypass the similarity test entirely.
//!
//! Phase 0 validates this gate against the live corpus (measuring the false-negative
//! rate on the currently-relevant set) BEFORE it is ever wired into the pipeline.
//!
//! ## MEASURED FINDING (2026-06-05, live 36k corpus) — use as a PRIORITIZER, not a filter
//! Sweeping thresholds against the real corpus showed the semantic similarity signal does
//! NOT cleanly separate relevant from noise:
//!   taste/topic 0.45/0.55 → keep 84%, false-neg 1.5%   (barely filters)
//!   taste/topic 0.55/0.65 → keep 33%, false-neg 15%    (shreds recall)
//! Relevant items are smeared across the similarity range (the taste centroid is the
//! average of a broad developer profile — moderately similar to almost everything,
//! strongly similar to almost nothing). There is no threshold that meaningfully filters
//! without dropping relevant content.
//!
//! Conclusion: do NOT use `keep == false` as a hard DROP. The precise signals
//! (`DepMatch`, `HighStakes`) are the only safe "definitely score now" tier; the
//! semantic signal is best used to PRIORITIZE the backfill (score most-similar
//! unscored items first), with everything else DEFERRED (scored eventually, never
//! discarded). Real selectivity comes from upstream source filtering + forgetting,
//! not from this gate. See `.claude/plans/scoring-relevance-funnel.md` Phase 2.

use crate::utils::cosine_similarity;

use super::dependencies::match_dependencies;
use super::ScoringContext;

/// Tunable thresholds for the cheap gate. Defaults are deliberately LOW (high recall);
/// Phase 0's recall measurement against the live corpus confirms or tightens them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TriageThresholds {
    /// Minimum raw cosine to the user's taste centroid to keep an item.
    /// `compute_taste_boost` treats ~0.4 as "typical background similarity", so a
    /// keep threshold a touch above background errs toward recall.
    pub taste_min: f32,
    /// Minimum raw cosine to ANY single tracked-topic embedding to keep an item.
    pub topic_min: f32,
}

impl Default for TriageThresholds {
    fn default() -> Self {
        Self {
            taste_min: 0.45,
            topic_min: 0.55,
        }
    }
}

/// Why the gate reached its verdict — recorded for audit and (later) for the
/// re-examination triggers in Phase 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TriageReason {
    /// Security/breaking/CVE — kept unconditionally (error-cost asymmetry).
    HighStakes,
    /// Matched the user's dependency graph (their actual stack).
    DepMatch,
    /// Close to the user's holistic taste centroid.
    TasteSimilar,
    /// Close to a specific tracked topic.
    TopicSimilar,
    /// No usable (non-zero) embedding — kept (fail-open: never drop what we couldn't judge).
    NoEmbedding,
    /// Far from the user's entire profile — deferred (re-examinable, NOT deleted).
    Deferred,
}

/// Verdict from the cheap gate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TriageVerdict {
    /// `true` = worth full scoring now; `false` = defer (keep for re-examination).
    pub keep: bool,
    pub reason: TriageReason,
    /// The similarity that drove the decision (taste or best-topic cosine, or dep
    /// confidence) — surfaced for the recall audit and threshold tuning.
    pub similarity: f32,
}

impl TriageVerdict {
    fn keep(reason: TriageReason, similarity: f32) -> Self {
        Self {
            keep: true,
            reason,
            similarity,
        }
    }
}

/// Decide whether an item is worth full scoring. Cheap and side-effect free.
///
/// Evaluated in priority order; first match wins:
/// 1. High-stakes carve-out (never gated)
/// 2. Dependency-graph match (your stack)
/// 3. Semantic similarity to taste centroid, then to any tracked topic
/// 4. Otherwise defer
pub(crate) fn triage_item(
    embedding: &[f32],
    title: &str,
    content: &str,
    content_type: Option<&str>,
    cve_ids: Option<&str>,
    ctx: &ScoringContext,
    thresholds: &TriageThresholds,
) -> TriageVerdict {
    // (1) High-stakes carve-out — a missed CVE/breaking-change for the user's stack is
    //     catastrophic, so these are never gated out regardless of similarity. The
    //     dep-match in stage 2 (and full scoring) decides *how* urgent; the gate just
    //     guarantees they are never silently dropped here.
    let has_cve = cve_ids.is_some_and(|c| !c.trim().is_empty());
    let high_stakes_ct = matches!(
        content_type,
        Some("security_advisory") | Some("breaking_change")
    );
    if has_cve || high_stakes_ct {
        return TriageVerdict::keep(TriageReason::HighStakes, 1.0);
    }

    // (2) Dependency match — reuse the canonical scorer dep-matcher so the gate's notion
    //     of "your stack" is identical to scoring's (word boundaries, ambiguous-name
    //     handling). For a gate, over-matching is acceptable (recall > precision).
    let (dep_matches, dep_score) = match_dependencies(title, content, &[], &ctx.ace_ctx);
    if !dep_matches.is_empty() {
        return TriageVerdict::keep(TriageReason::DepMatch, dep_score.max(0.01));
    }

    // (3) Semantic relevance. A zero/fallback embedding can't be judged semantically —
    //     fail OPEN (keep), because silently dropping something we simply couldn't
    //     evaluate would be a false negative we can't see.
    let item_norm = crate::vector_norm(embedding);
    if item_norm < f32::EPSILON {
        return TriageVerdict::keep(TriageReason::NoEmbedding, 0.0);
    }

    if let Some(taste) = ctx.taste_embedding.as_deref() {
        let sim = cosine_similarity(embedding, taste);
        if sim >= thresholds.taste_min {
            return TriageVerdict::keep(TriageReason::TasteSimilar, sim);
        }
    }

    let mut best_topic = 0.0f32;
    for emb in ctx.topic_embeddings.values() {
        if emb.len() == embedding.len() {
            let s = cosine_similarity(embedding, emb);
            if s > best_topic {
                best_topic = s;
            }
        }
    }
    if best_topic >= thresholds.topic_min {
        return TriageVerdict::keep(TriageReason::TopicSimilar, best_topic);
    }

    // (4) Far from the user's entire profile → defer. Kept re-examinable; NOT deleted.
    TriageVerdict {
        keep: false,
        reason: TriageReason::Deferred,
        similarity: best_topic.max(0.0),
    }
}

#[cfg(test)]
#[path = "triage_tests.rs"]
mod tests;
