//! PASIFA V2 Scoring Pipeline — 8-phase structured architecture
//!
//! Restructures V1 into clean, testable phases while reusing all existing module functions.
//!
//! Key improvements over V1:
//! - **Separate KNN calibration** (`calibrate_knn`) with center=0.49, scale=12 to suppress KNN noise
//! - **Gate count on clean signals** (Phase 3): count signals before any combination
//! - **Multiplicative semantic**: `base * (1.0 + semantic_boost)` not additive
//! - **Single quality composite** (Phase 5): all multipliers dampened and multiplied in one pass,
//!   with domain_quality_mult restored as a multiplicative factor (NOT dampened)
//! - **Single boost cap** (Phase 6): all boosts summed, capped at \[-0.15, 0.35\], then dampened
//! - **Gate table** matches V1 for 0-1 signals: \[(0.25,0.20), (0.45,0.28), ...\]
//! - **Score ceiling applied LAST** in gate phase — after domain gate mult

use std::collections::HashMap;

use crate::db::Database;
use crate::scoring_config;
use crate::signals;
use crate::{
    check_exclusions, extract_topics, get_relevance_threshold, RelevanceMatch, ScoreBreakdown,
    SourceRelevance,
};

use super::pipeline::{ScoringInput, ScoringOptions};
use super::*;

// ============================================================================
// V2 Constants (self-contained)
// ============================================================================

const V2_GATE: [(f32, f32); 6] = [
    (0.25, 0.20), // 0 signals — heavy penalty
    (0.45, 0.28), // 1 signal — safely below 0.35 threshold
    (1.00, 0.65), // 2 signals — tightened ceiling (was 0.80)
    (1.10, 0.85), // 3 signals — tightened ceiling (was 0.92)
    (1.20, 1.00), // 4 signals — strong confirmation
    (1.25, 1.00), // 5 signals — full confidence
];

const BOOST_CAP_MIN: f32 = -0.15;
const BOOST_CAP_MAX: f32 = 0.35;
const KNN_CENTER: f32 = 0.49;

/// Maximum gate ceiling bonus for strong signals (creates mid-band spread).
/// Strong 2-signal items can reach 0.65 + 0.08 = 0.73 vs weak at 0.65.
/// Only applies at 2+ signals — 0-1 signal ceilings are intentionally hard.
const STRENGTH_BONUS_MAX: f32 = 0.08;
const KNN_SCALE: f32 = 12.0;

// ============================================================================
// KNN-specific calibration
// ============================================================================

/// Calibrate a raw KNN distance-derived score using a sigmoid stretch.
/// Same shape as `calibrate_score` but with KNN-tuned parameters:
/// center=0.49 (slightly higher than cosine's 0.48) and scale=12 (conservative).
/// This suppresses KNN noise from high-distance matches that V1 over-counted.
fn calibrate_knn(raw: f32) -> f32 {
    if raw <= 0.0 {
        return 0.0;
    }
    if raw >= 1.0 {
        return 1.0;
    }
    1.0 / (1.0 + ((KNN_CENTER - raw) * KNN_SCALE).exp())
}

// ============================================================================
// Signal strength bonus — mid-band spread
// ============================================================================

/// Compute how strong the confirmed signals are, normalized to [0.0, 1.0].
/// Returns a bonus to add to the gate ceiling — strong 2-signal items get
/// a higher ceiling than weak 2-signal items, creating sub-ranking.
///
/// Each confirmed axis contributes its "excess" above threshold (how far above
/// the minimum confirmation level). The average excess drives the bonus.
fn compute_signal_strength_bonus(
    signal_count: u8,
    context_score: f32,
    interest_score: f32,
    keyword_score: f32,
    semantic_boost: f32,
    dep_match_score: f32,
    feedback_boost: f32,
    affinity_mult: f32,
    stack_pain_match: bool,
) -> f32 {
    // Only applies at 2+ signals — 0-1 ceilings are intentionally hard
    if signal_count < 2 {
        return 0.0;
    }

    let mut strengths: Vec<f32> = Vec::with_capacity(5);

    // Context axis: excess above 0.45 threshold, normalized to [0, 1]
    if context_score >= scoring_config::CONTEXT_THRESHOLD {
        let excess = (context_score - scoring_config::CONTEXT_THRESHOLD)
            / (1.0 - scoring_config::CONTEXT_THRESHOLD);
        strengths.push(excess.clamp(0.0, 1.0));
    }

    // Interest axis: best of interest_score and keyword_score
    let interest_confirmed = interest_score >= scoring_config::INTEREST_THRESHOLD
        || keyword_score >= scoring_config::KEYWORD_THRESHOLD;
    if interest_confirmed {
        let best = interest_score.max(keyword_score);
        let threshold = scoring_config::INTEREST_THRESHOLD.min(scoring_config::KEYWORD_THRESHOLD);
        let excess = (best - threshold) / (1.0 - threshold).max(0.01);
        strengths.push(excess.clamp(0.0, 1.0));
    }

    // ACE axis: semantic boost or active topic match
    let ace_confirmed = semantic_boost >= scoring_config::SEMANTIC_THRESHOLD || stack_pain_match;
    if ace_confirmed {
        if semantic_boost >= scoring_config::SEMANTIC_THRESHOLD {
            // Normalize semantic excess (practical range 0.18-0.50)
            let excess = (semantic_boost - scoring_config::SEMANTIC_THRESHOLD) / 0.32;
            strengths.push(excess.clamp(0.0, 1.0));
        } else {
            // stack_pain_match is binary — use flat 0.4
            strengths.push(0.4);
        }
    }

    // Learned axis: feedback or affinity
    if feedback_boost > scoring_config::FEEDBACK_THRESHOLD
        || affinity_mult >= scoring_config::AFFINITY_THRESHOLD
    {
        // Affinity-driven strength (affinity range 1.15-1.70)
        if affinity_mult >= scoring_config::AFFINITY_THRESHOLD {
            let excess = (affinity_mult - scoring_config::AFFINITY_THRESHOLD) / 0.55;
            strengths.push(excess.clamp(0.0, 1.0));
        } else {
            strengths.push(0.4); // Feedback is less granular
        }
    }

    // Dependency axis
    if dep_match_score >= scoring_config::DEPENDENCY_THRESHOLD {
        let excess = (dep_match_score - scoring_config::DEPENDENCY_THRESHOLD)
            / (1.0 - scoring_config::DEPENDENCY_THRESHOLD);
        strengths.push(excess.clamp(0.0, 1.0));
    }

    if strengths.is_empty() {
        return 0.0;
    }

    let avg_strength = strengths.iter().sum::<f32>() / strengths.len() as f32;
    avg_strength * STRENGTH_BONUS_MAX
}

// ============================================================================
// Signal data structures
// ============================================================================

/// All raw signal values extracted from the input, before calibration.
struct RawSignals {
    context: f32,
    interest: f32,
    keyword_score: f32,
    semantic_boost: f32,
    dep_match_score: f32,
    matched_deps: Vec<dependencies::DepMatch>,
    feedback_boost: f32,
    affinity_mult: f32,
    anti_penalty: f32,
    domain_relevance: f32,
    taste_boost: f32,
    stack_boost: f32,
    stack_pain_match: bool,
    topics: Vec<String>,
    specificity_weight: f32,
}

/// Calibrated signal values ready for combination.
struct CalibratedSignals {
    context_score: f32,
    interest_score: f32,
    keyword_score: f32,
    semantic_boost: f32,
}

// ============================================================================
// Phase 1: Extract all raw signals independently
// ============================================================================

fn extract_signals(
    input: &ScoringInput,
    ctx: &ScoringContext,
    matches: &[RelevanceMatch],
) -> RawSignals {
    let topics = extract_topics(input.title, input.content);

    // Raw context: best KNN score from embedding similarity
    let raw_context = matches.first().map_or(0.0, |m| m.similarity);

    // Raw interest: embedding similarity against declared interests
    let raw_interest = compute_interest_score(input.embedding, &ctx.interests);

    // Keyword interest matching: boosts items containing declared interest terms
    let raw_keyword_score =
        keywords::compute_keyword_interest_score(input.title, input.content, &ctx.interests);
    let specificity_weight =
        keywords::best_interest_specificity_weight(input.title, input.content, &ctx.interests);
    let keyword_score = raw_keyword_score * specificity_weight;

    // Semantic boost with keyword fallback
    let semantic_boost =
        compute_semantic_ace_boost(input.embedding, &ctx.ace_ctx, &ctx.topic_embeddings)
            .unwrap_or_else(|| semantic::compute_keyword_ace_boost(&topics, &ctx.ace_ctx));

    // Dependency intelligence
    let (matched_deps, dep_match_score) =
        match_dependencies(input.title, input.content, &topics, &ctx.ace_ctx);

    // Feedback learning boost
    let feedback_boost = if ctx.feedback_boosts.is_empty() {
        0.0
    } else {
        let mut boost_sum: f64 = 0.0;
        let mut match_count = 0;
        for topic in &topics {
            if let Some(&net_score) = ctx.feedback_boosts.get(topic.as_str()) {
                boost_sum += net_score;
                match_count += 1;
            }
        }
        if match_count > 0 {
            ((boost_sum / match_count as f64) * scoring_config::FEEDBACK_SCALE as f64).clamp(
                scoring_config::FEEDBACK_CAP_RANGE.0 as f64,
                scoring_config::FEEDBACK_CAP_RANGE.1 as f64,
            ) as f32
        } else {
            0.0
        }
    };

    // Affinity and anti-penalty from learned topic preferences
    let affinity_mult = compute_affinity_multiplier(&topics, &ctx.ace_ctx);
    let anti_penalty = compute_anti_penalty(&topics, &ctx.ace_ctx);

    // Domain relevance: graduated penalty based on technology identity
    let domain_relevance =
        crate::domain_profile::compute_domain_relevance(&topics, &ctx.domain_profile);

    // Taste embedding boost
    let taste_boost = match ctx.taste_embedding {
        Some(ref taste_emb) if !input.embedding.is_empty() => {
            semantic::compute_taste_boost(input.embedding, taste_emb)
        }
        _ => 0.0,
    };

    // Stack intelligence
    let stack_boost = crate::stacks::scoring::compute_stack_boost(
        input.title,
        input.content,
        &ctx.composed_stack,
    );

    let stack_pain_match = crate::stacks::scoring::has_pain_point_match(
        input.title,
        input.content,
        &ctx.composed_stack,
    );

    RawSignals {
        context: raw_context,
        interest: raw_interest,
        keyword_score,
        semantic_boost,
        dep_match_score,
        matched_deps,
        feedback_boost,
        affinity_mult,
        anti_penalty,
        domain_relevance,
        taste_boost,
        stack_boost,
        stack_pain_match,
        topics,
        specificity_weight,
    }
}

// ============================================================================
// Phase 2: Calibrate raw signals
// ============================================================================

fn calibrate_signals(raw: &RawSignals) -> CalibratedSignals {
    CalibratedSignals {
        context_score: calibrate_knn(raw.context),
        interest_score: calibrate_score(raw.interest),
        keyword_score: raw.keyword_score,   // passthrough
        semantic_boost: raw.semantic_boost, // passthrough
    }
}

// ============================================================================
// Phase 4: Compute base relevance score — FOUR branches
// ============================================================================

fn compute_relevance(
    cal: &CalibratedSignals,
    ctx: &ScoringContext,
    has_real_embedding: bool,
) -> f32 {
    if ctx.cached_context_count > 0 && ctx.interest_count > 0 {
        // Both context and interest available
        let ctx_w = (scoring_config::BASE_BOTH_CONTEXT_BASE
            + cal.context_score * scoring_config::BASE_BOTH_CONTEXT_SCALE)
            .clamp(
                scoring_config::BASE_BOTH_CONTEXT_BASE,
                scoring_config::BASE_BOTH_CONTEXT_MAX,
            );
        let remaining = 1.0 - ctx_w;
        let int_w = remaining * scoring_config::BASE_BOTH_INTEREST_SHARE;
        let kw_w = remaining * scoring_config::BASE_BOTH_KEYWORD_SHARE;
        let base =
            cal.context_score * ctx_w + cal.interest_score * int_w + cal.keyword_score * kw_w;
        // MULTIPLICATIVE semantic
        (base * (1.0 + cal.semantic_boost)).clamp(0.0, 1.0)
    } else if ctx.interest_count > 0 {
        // Interest only
        let semantic_mult = if has_real_embedding
            && ctx.interest_count < 3
            && ctx.feedback_interaction_count < 10
            && ctx.ace_ctx.dependency_names.len() < 5
        {
            scoring_config::INTEREST_ONLY_SEMANTIC_MULT * 0.4
        } else {
            scoring_config::INTEREST_ONLY_SEMANTIC_MULT
        };
        let base = cal.interest_score * scoring_config::INTEREST_ONLY_INTEREST_W
            + cal.keyword_score * scoring_config::INTEREST_ONLY_KEYWORD_W;
        // MULTIPLICATIVE semantic
        (base * (1.0 + cal.semantic_boost * semantic_mult)).clamp(0.0, 1.0)
    } else if ctx.cached_context_count > 0 {
        // Context only
        (cal.context_score * (1.0 + cal.semantic_boost)).clamp(0.0, 1.0)
    } else {
        // Neither
        (cal.semantic_boost * 1.5).clamp(0.0, 1.0)
    }
}

// ============================================================================
// Phase 5: Compute quality composite — ALL multipliers in one pass
// ============================================================================

/// Returns (quality_score, freshness, source_quality_boost, competing_mult, content_quality_mult,
///          content_dna_mult, content_type, novelty_mult, ecosystem_shift_mult, stack_competing_mult,
///          sophistication_mult, content_analysis_mult)
#[allow(clippy::type_complexity)]
fn compute_quality_composite(
    relevance_score: f32,
    input: &ScoringInput,
    ctx: &ScoringContext,
    raw: &RawSignals,
    options: &ScoringOptions,
    db: &Database,
) -> (
    f32,
    f32,
    f32,
    f32,
    f32,
    f32,
    crate::content_dna::ContentType,
    f32,
    f32,
    f32,
    f32,
    f32,
) {
    // Freshness: topic-aware when autophagy half-lives are available
    let freshness = if options.apply_freshness {
        if let Some(created_at) = input.created_at {
            let base_freshness = compute_temporal_freshness(created_at);
            let topic_adjusted = if !ctx.topic_half_lives.is_empty() && !raw.topics.is_empty() {
                let matching_half_lives: Vec<f32> = raw
                    .topics
                    .iter()
                    .filter_map(|t| ctx.topic_half_lives.get(t.as_str()).copied())
                    .collect();
                if matching_half_lives.is_empty() {
                    base_freshness
                } else {
                    let avg_half_life =
                        matching_half_lives.iter().sum::<f32>() / matching_half_lives.len() as f32;
                    let age_hours =
                        ((chrono::Utc::now() - *created_at).num_minutes() as f32 / 60.0).max(0.0);
                    let calibrated = (-0.693 * age_hours / avg_half_life.max(1.0)).exp();
                    (base_freshness * 0.5 + calibrated * 0.5).clamp(0.3, 1.0)
                }
            } else {
                base_freshness
            };
            // Peak hours boost: slight freshness bonus for content published during
            // the user's active coding hours (from git commit frequency analysis)
            if ctx.ace_ctx.peak_hours.is_empty() {
                topic_adjusted
            } else {
                let publish_hour = chrono::Timelike::hour(created_at) as u8;
                if ctx.ace_ctx.peak_hours.contains(&publish_hour) {
                    (topic_adjusted + 0.03).min(1.0)
                } else {
                    topic_adjusted
                }
            }
        } else {
            1.0
        }
    } else {
        1.0
    };

    // Source quality boost from learned preferences
    let source_quality_boost =
        ctx.source_quality
            .get(input.source_type)
            .copied()
            .map_or(0.0, |score| {
                (score * scoring_config::SOURCE_QUALITY_MULT).clamp(
                    scoring_config::SOURCE_QUALITY_CAP_RANGE.0,
                    scoring_config::SOURCE_QUALITY_CAP_RANGE.1,
                )
            });
    let source_quality_mult = 1.0 + source_quality_boost;

    // Anti-topic multiplier
    let anti_mult = 1.0 - raw.anti_penalty;

    // Domain quality penalty (NOT dampened — preserves full penalty strength)
    let domain_quality_mult = if raw.domain_relevance >= 0.85 {
        1.0
    } else if raw.domain_relevance >= 0.50 {
        1.0 - scoring_config::OFF_DOMAIN_PENALTY * (1.0 - raw.domain_relevance) * 0.5
    } else {
        1.0 - scoring_config::OFF_DOMAIN_PENALTY * (1.0 - raw.domain_relevance)
    };

    // Competing tech penalty
    let competing_mult = crate::competing_tech::compute_competing_penalty(
        &raw.topics,
        input.title,
        &ctx.domain_profile.primary_stack,
    );

    // Content quality
    let content_quality =
        crate::content_quality::compute_content_quality(input.title, input.content, input.url);

    // Content DNA (source-type-aware)
    let (content_type, content_dna_mult) = crate::content_dna::classify_content_for_source(
        input.title,
        input.content,
        input.source_type,
    );

    // Novelty
    let novelty = crate::novelty::compute_novelty(
        input.title,
        input.content,
        &raw.topics,
        &ctx.domain_profile.primary_stack,
    );

    // Ecosystem shift from stack profiles
    let ecosystem_shift_mult = crate::stacks::scoring::detect_ecosystem_shift(
        &raw.topics,
        input.title,
        &ctx.composed_stack,
    );

    // Stack-aware competing tech penalty
    let stack_competing_mult = crate::stacks::scoring::compute_competing_penalty(
        input.title,
        input.content,
        &ctx.composed_stack,
    );

    // Content sophistication (audience-aware depth scoring)
    let sophistication = crate::content_sophistication::compute_sophistication(
        input.title,
        input.content,
        ctx.ace_ctx.detected_tech.len(),
        &ctx.domain_profile,
    );
    let sophistication_mult = sophistication.multiplier;

    // Content analysis multiplier (from cached LLM pre-analysis, if available)
    let content_analysis_mult = {
        let hash = crate::content_analysis::content_hash(input.content);
        crate::content_analysis::get_cached_analysis(db, &hash)
            .ok()
            .flatten()
            .map(|a| {
                let is_senior = ctx.ace_ctx.detected_tech.len() > 15
                    && ctx.domain_profile.dependency_names.len() > 50;
                crate::content_analysis::analysis_to_multiplier(&a, is_senior)
            })
            .unwrap_or(1.0)
    };

    // Asymmetric dampening function
    let dampen = |m: f32| {
        if m < 1.0 {
            1.0 + (m - 1.0) * scoring_config::DAMPENING_PENALTY_STRENGTH
        } else {
            1.0 + (m - 1.0) * scoring_config::DAMPENING_BOOST_STRENGTH
        }
    };

    // Domain-aware content_dna dampening
    let content_dna_dampened = if content_dna_mult < 1.0
        && raw.domain_relevance >= 1.0
        && !ctx.domain_profile.is_empty()
    {
        1.0 + (content_dna_mult - 1.0) * scoring_config::DAMPENING_DOMAIN_AWARE_STRENGTH
    } else {
        dampen(content_dna_mult)
    };

    // Single composite of ALL quality multipliers
    let composite = dampen(competing_mult)
        * dampen(content_quality.multiplier)
        * content_dna_dampened
        * dampen(novelty.multiplier)
        * dampen(ecosystem_shift_mult)
        * dampen(stack_competing_mult)
        * dampen(sophistication_mult)
        * dampen(content_analysis_mult)
        * dampen(freshness)
        * dampen(source_quality_mult)
        * dampen(raw.affinity_mult)
        * anti_mult
        * domain_quality_mult;

    let quality_score = (relevance_score * composite).clamp(0.0, 1.0);

    // ── CVE/Security dependency validation (ported from V1) ─────────────
    // Security items about packages NOT in the user's actual dependencies
    // get strongly penalized. Without this, every CVE source item rides the
    // SecurityAdvisory (1.30) + novelty (1.10) multipliers and surfaces at
    // 70-80% regardless of whether the user uses the affected software.
    //
    // Tiers (aligned with V1 pipeline.rs exactly so both pipelines agree):
    //   * no matched deps at all          → 0.35 (hard suppression)
    //   * matched but confidence < 0.10   → 0.60 (mild penalty)
    //   * strong match                    → unchanged (full strength)
    //
    // Applies to both explicit CVE source items and any other source whose
    // title/content matches the security classifier — so future security
    // sources are governed by the same gate automatically.
    let quality_score = if novelty.is_security
        && raw.dep_match_score < 0.10
        && !raw.matched_deps.is_empty()
    {
        quality_score * 0.60
    } else if novelty.is_security && raw.matched_deps.is_empty() {
        quality_score * 0.35
    } else {
        quality_score
    };

    (
        quality_score,
        freshness,
        source_quality_boost,
        competing_mult,
        content_quality.multiplier,
        content_dna_mult,
        content_type,
        novelty.multiplier,
        ecosystem_shift_mult,
        stack_competing_mult,
        sophistication_mult,
        content_analysis_mult,
    )
}

// ============================================================================
// Phase 6: Compute boosts — sum, cap, dampen, add
// ============================================================================

/// Returns (boosted_score, dep_boost, intent_boost, window_boost, skill_gap_boost,
///          calibration_correction, matched_window_id, matched_skill_gaps)
#[allow(clippy::type_complexity)]
fn compute_boosts(
    quality_score: f32,
    input: &ScoringInput,
    ctx: &ScoringContext,
    raw: &RawSignals,
) -> (f32, f32, f32, f32, f32, f32, Option<i64>, Vec<String>) {
    // Dependency boost (in bootstrap mode, 2x weight)
    let dep_weight = if ctx.feedback_interaction_count < 10 {
        scoring_config::DEPENDENCY_BOOST_WEIGHT * 2.0
    } else {
        scoring_config::DEPENDENCY_BOOST_WEIGHT
    };
    let dep_boost = raw.dep_match_score * dep_weight;

    // Intent boost: amplify items matching recent work topics
    let intent_boost: f32 = if ctx.work_topics.is_empty() {
        0.0
    } else {
        let matching_work_topics = raw
            .topics
            .iter()
            .filter(|t| ctx.work_topics.iter().any(|wt| topic_overlaps(t, wt)))
            .count();
        match matching_work_topics {
            0 => 0.0,
            1 => scoring_config::INTENT_BOOST_SINGLE_MATCH,
            _ => scoring_config::INTENT_BOOST_MULTI_MATCH,
        }
    };

    // Decision window boost
    let (window_boost, matched_window_id) = if ctx.open_windows.is_empty() {
        (0.0, None)
    } else {
        crate::decision_advantage::compute_decision_window_boost(
            &ctx.open_windows,
            input.title,
            input.content,
            &raw.topics,
            &raw.matched_deps
                .iter()
                .map(|d| d.package_name.clone())
                .collect::<Vec<_>>(),
        )
    };

    // Skill-gap boost
    let mut matched_skill_gaps: Vec<String> = Vec::new();
    let skill_gap_boost: f32 = if let Some(ref profile) = ctx.sovereign_profile {
        if profile.intelligence.skill_gaps.is_empty() {
            0.0
        } else {
            for t in &raw.topics {
                if let Some(g) = profile
                    .intelligence
                    .skill_gaps
                    .iter()
                    .find(|g| topic_overlaps(t, &g.dependency))
                {
                    if !matched_skill_gaps.contains(&g.dependency) {
                        matched_skill_gaps.push(g.dependency.clone());
                    }
                }
            }
            match matched_skill_gaps.len() {
                0 => 0.0,
                1 => 0.15,
                _ => 0.20,
            }
        }
    } else {
        0.0
    };

    // Autophagy calibration correction
    let calibration_correction: f32 =
        if !ctx.calibration_deltas.is_empty() && !raw.topics.is_empty() {
            let matching: Vec<f32> = raw
                .topics
                .iter()
                .filter_map(|t| ctx.calibration_deltas.get(t.as_str()).copied())
                .collect();
            if matching.is_empty() {
                0.0
            } else {
                let avg_delta = matching.iter().sum::<f32>() / matching.len() as f32;
                avg_delta.clamp(-0.10, 0.10)
            }
        } else {
            0.0
        };

    // Sum all boosts -> cap -> dampen -> add
    let total_raw = dep_boost
        + raw.stack_boost
        + intent_boost
        + raw.feedback_boost
        + window_boost
        + skill_gap_boost
        + calibration_correction
        + raw.taste_boost;

    let total_capped = total_raw.clamp(BOOST_CAP_MIN, BOOST_CAP_MAX);

    let total_dampened = if total_capped < 0.0 {
        total_capped * scoring_config::DAMPENING_PENALTY_STRENGTH
    } else {
        total_capped * scoring_config::DAMPENING_BOOST_STRENGTH
    };

    let boosted = (quality_score + total_dampened).clamp(0.0, 1.0);

    (
        boosted,
        dep_boost,
        intent_boost,
        window_boost,
        skill_gap_boost,
        calibration_correction,
        matched_window_id,
        matched_skill_gaps,
    )
}

// ============================================================================
// Phase 7: Apply gate effect — confidence multiplier + domain gate + ceiling LAST
// ============================================================================

fn apply_gate_effect(
    score: f32,
    signal_count: u8,
    domain_relevance: f32,
    ctx: &ScoringContext,
    strength_bonus: f32,
) -> f32 {
    let idx = (signal_count as usize).min(5);
    let (conf_mult, base_ceiling) = V2_GATE[idx];
    // Adjust ceiling based on signal strength — strong signals get higher ceiling.
    // This creates sub-ranking within gate tiers: strong 2-signal items at ~0.73
    // are clearly differentiated from weak 2-signal items capped at 0.65.
    let score_ceiling = (base_ceiling + strength_bonus).min(1.0);

    let gated = score * conf_mult;

    // Domain gate: same ramp as V1
    let domain_gate_mult = if domain_relevance >= 1.0 && !ctx.domain_profile.is_empty() {
        scoring_config::DOMAIN_GATE_PRIMARY_BOOST
    } else if domain_relevance >= 0.85 {
        1.0
    } else if domain_relevance >= 0.50 {
        let gap = 1.0 - scoring_config::DOMAIN_GATE_RAMP_BASE;
        scoring_config::DOMAIN_GATE_RAMP_BASE + (domain_relevance - 0.50) * (gap / 0.35)
    } else {
        scoring_config::DOMAIN_GATE_OFF_DOMAIN_MULT
    };

    // Score ceiling applied LAST — domain boost cannot push above gate ceiling
    (gated * domain_gate_mult)
        .min(score_ceiling)
        .clamp(0.0, 1.0)
}

// ============================================================================
// Phase 8: Apply final adjustments — short title cap only
// ============================================================================

fn apply_final_adjustments(score: f32, title: &str) -> f32 {
    let meaningful_words = title.split_whitespace().filter(|w| w.len() >= 2).count();
    if meaningful_words < 3 {
        score.min(scoring_config::QUALITY_FLOOR_SHORT_TITLE_CAP)
    } else {
        score
    }
}

// ============================================================================
// Signal classification (mirrors V1 logic)
// ============================================================================

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn classify_signals(
    relevant: bool,
    combined_score: f32,
    domain_relevance: f32,
    content_type: &crate::content_dna::ContentType,
    options: &ScoringOptions,
    classifier: Option<&signals::SignalClassifier>,
    input: &ScoringInput,
    ctx: &ScoringContext,
    matched_deps: &[dependencies::DepMatch],
    db: &Database,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<Vec<String>>,
    Option<String>,
) {
    let show_and_tell_blocked =
        *content_type == crate::content_dna::ContentType::ShowAndTell && domain_relevance < 1.0;

    // Security advisories and breaking changes with STRONG dependency matches
    // bypass the domain_relevance gate — a CVE in your deps is urgent regardless
    // of how "on-domain" the advisory text appears.
    //
    // Requires a non-dev dep match with confidence >= 0.15 so a single weak
    // word-boundary hit cannot escalate an unrelated CVE into a Critical signal.
    let has_strong_non_dev_match = matched_deps
        .iter()
        .any(|d| !d.is_dev && d.confidence >= 0.15);
    let is_critical_content = (*content_type == crate::content_dna::ContentType::SecurityAdvisory
        || *content_type == crate::content_dna::ContentType::BreakingChange)
        && has_strong_non_dev_match;

    if !is_critical_content
        && !(options.apply_signals
            && relevant
            && combined_score >= 0.30
            && domain_relevance >= 0.70
            && !show_and_tell_blocked)
    {
        return (None, None, None, None, None);
    }

    let Some(clf) = classifier else {
        return (None, None, None, None, None);
    };

    let topics = crate::extract_topics(input.title, input.content);
    let corroboration = super::pipeline::build_corroboration(db, &topics, matched_deps);
    match clf.classify(
        input.title,
        input.content,
        combined_score,
        &ctx.declared_tech,
        &ctx.ace_ctx.detected_tech,
        &corroboration,
    ) {
        Some(mut c) => {
            // Dependency-aware priority escalation
            if !matched_deps.is_empty() {
                let has_non_dev_dep = matched_deps.iter().any(|d| !d.is_dev);
                if c.signal_type == signals::SignalType::SecurityAlert && has_non_dev_dep {
                    c.priority = signals::SignalPriority::Critical;
                    c.action = format!(
                        "Critical: Security issue affects your dependency {}",
                        matched_deps[0].package_name
                    );
                } else if c.signal_type == signals::SignalType::BreakingChange
                    && matched_deps
                        .iter()
                        .any(|d| d.version_delta == dependencies::VersionDelta::NewerMajor)
                    && c.priority < signals::SignalPriority::Alert
                {
                    c.priority = signals::SignalPriority::Alert;
                }
                for dep in matched_deps.iter().take(2) {
                    c.triggers.push(format!("dep:{}", dep.package_name));
                }
            }

            // Score-aware priority cap
            if combined_score < scoring_config::LOW_SCORE_CAP
                && c.priority > signals::SignalPriority::Watch
            {
                c.priority = signals::SignalPriority::Watch;
            } else if (combined_score < scoring_config::MEDIUM_SCORE_CAP
                && c.priority > signals::SignalPriority::Advisory)
                || (combined_score > scoring_config::HIGH_SCORE_FLOOR
                    && c.priority < signals::SignalPriority::Advisory)
            {
                c.priority = signals::SignalPriority::Advisory;
            }

            (
                Some(c.signal_type.slug().to_string()),
                Some(c.priority.label().to_string()),
                Some(c.action),
                Some(c.triggers),
                Some(c.horizon.label().to_string()),
            )
        }
        None => (None, None, None, None, None),
    }
}

// ============================================================================
// Main entry point — identical public signature to V1
// ============================================================================

/// Score a single item through the PASIFA V2 pipeline (8-phase architecture).
/// Returns SourceRelevance with all fields populated — drop-in replacement for V1.
pub(crate) fn score_item(
    input: &ScoringInput,
    ctx: &ScoringContext,
    db: &Database,
    options: &ScoringOptions,
    classifier: Option<&signals::SignalClassifier>,
) -> SourceRelevance {
    let topics = extract_topics(input.title, input.content);

    // ── Exclusion check (before any scoring work) ──────────────────────
    let excluded_by = check_exclusions(&topics, &ctx.exclusions)
        .or_else(|| check_ace_exclusions(&topics, &ctx.ace_ctx));

    if let Some(exclusion) = excluded_by {
        return SourceRelevance {
            id: input.id,
            title: input.title.to_string(),
            url: input.url.map(std::string::ToString::to_string),
            top_score: 0.0,
            matches: vec![],
            relevant: false,
            context_score: 0.0,
            interest_score: 0.0,
            excluded: true,
            excluded_by: Some(exclusion),
            source_type: input.source_type.to_string(),
            explanation: None,
            confidence: Some(0.0),
            score_breakdown: None,
            signal_type: None,
            signal_priority: None,
            signal_action: None,
            signal_triggers: None,
            signal_horizon: None,
            similar_count: 0,
            similar_titles: vec![],
            serendipity: false,
            streets_engine: None,
            decision_window_match: None,
            decision_boost_applied: 0.0,
            created_at: None,
            detected_lang: input.detected_lang.to_string(),
        };
    }

    // ── KNN context search (needed for Phase 1 and final output) ──────
    let matches: Vec<RelevanceMatch> =
        if ctx.cached_context_count > 0 && !input.embedding.is_empty() {
            db.find_similar_contexts(input.embedding, 3)
                .unwrap_or_default()
                .into_iter()
                .map(|result| {
                    let similarity = 1.0 / (1.0 + result.distance);
                    let matched_text = if result.text.len() > 100 {
                        let truncated: String = result.text.chars().take(100).collect();
                        format!("{truncated}...")
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

    // ── Phase 1: Extract all raw signals ──────────────────────────────
    let raw = extract_signals(input, ctx, &matches);

    // ── Phase 2: Calibrate ────────────────────────────────────────────
    let cal = calibrate_signals(&raw);

    // ── Phase 3: Gate count on clean signals ──────────────────────────
    let confirmation = gate::count_confirmed_signals(
        cal.context_score,
        cal.interest_score,
        cal.keyword_score,
        cal.semantic_boost,
        &ctx.ace_ctx,
        &raw.topics,
        raw.feedback_boost,
        raw.affinity_mult,
        raw.dep_match_score,
        raw.stack_pain_match,
        raw.specificity_weight,
    );
    let signal_count = confirmation.count;
    let confirmed_signals = confirmation.confirmed_names();

    // ── Phase 4: Compute base relevance ───────────────────────────────
    let has_real_embedding = input.embedding.iter().any(|&v| v != 0.0);
    let relevance_score = compute_relevance(&cal, ctx, has_real_embedding);

    // ── Phase 5: Quality composite ────────────────────────────────────
    let (
        quality_score,
        freshness,
        source_quality_boost,
        competing_mult,
        content_quality_mult,
        content_dna_mult,
        content_type,
        novelty_mult,
        ecosystem_shift_mult,
        stack_competing_mult,
        _sophistication_mult,
        content_analysis_mult,
    ) = compute_quality_composite(relevance_score, input, ctx, &raw, options, db);

    // ── Phase 6: Boosts ───────────────────────────────────────────────
    let (
        boosted_score,
        _dep_boost,
        intent_boost,
        window_boost,
        skill_gap_boost,
        _calibration_correction,
        matched_window_id,
        matched_skill_gaps,
    ) = compute_boosts(quality_score, input, ctx, &raw);

    // Trend topic boost (temporal clustering signal)
    let trend_boost = if !options.trend_topics.is_empty()
        && raw.topics.iter().any(|t| options.trend_topics.contains(t))
    {
        0.08
    } else {
        0.0
    };
    let boosted_score = (boosted_score + trend_boost).clamp(0.0, 1.0);

    // ── Signal strength bonus (pre-gate) ─────────────────────────────
    let strength_bonus = compute_signal_strength_bonus(
        signal_count,
        cal.context_score,
        cal.interest_score,
        cal.keyword_score,
        cal.semantic_boost,
        raw.dep_match_score,
        raw.feedback_boost,
        raw.affinity_mult,
        raw.stack_pain_match,
    );

    // ── Phase 7: Gate effect ──────────────────────────────────────────
    let conf_idx = (signal_count as usize).min(5);
    let confirmation_mult = V2_GATE[conf_idx].0;
    let gated_score = apply_gate_effect(
        boosted_score,
        signal_count,
        raw.domain_relevance,
        ctx,
        strength_bonus,
    );

    // ── Phase 8: Final adjustments ────────────────────────────────────
    let combined_score = apply_final_adjustments(gated_score, input.title);

    // ── Critical content fast-path ─────────────────────────────────────
    // Security advisories and breaking changes affecting user's actual
    // dependencies ALWAYS surface, regardless of relevance score.
    // This prevents the gate from silently dropping critical alerts.
    //
    // IMPORTANT: the dep match must be strong AND touch a non-dev dep.
    // `dep_match_score > 0.0` is too loose — a single stray word-boundary
    // hit (e.g. the word "hostname" in an unrelated CVE) would trigger the
    // floor and surface CVEs that have nothing to do with the user's stack.
    let is_security = content_type == crate::content_dna::ContentType::SecurityAdvisory;
    let is_breaking = content_type == crate::content_dna::ContentType::BreakingChange;
    let has_strong_dep_match = raw.dep_match_score >= 0.15
        && raw.matched_deps.iter().any(|d| !d.is_dev);
    let critical_fast_path = (is_security || is_breaking) && has_strong_dep_match;

    // If critical fast-path, boost score to ensure it passes the gate
    let combined_score = if critical_fast_path && combined_score < 0.50 {
        combined_score.max(0.50) // Floor at 0.50 for security items matching deps
    } else {
        combined_score
    };

    // ── Relevance determination ───────────────────────────────────────
    let bootstrap_mode = ctx.feedback_interaction_count < 10;
    let min_signals = if bootstrap_mode {
        1u8
    } else {
        scoring_config::QUALITY_FLOOR_MIN_SIGNALS as u8
    };
    let relevant = critical_fast_path  // Critical items always relevant
        || (combined_score >= get_relevance_threshold()
            && (signal_count >= min_signals
                || combined_score >= scoring_config::QUALITY_FLOOR_MIN_SCORE));

    // ── Explanation ───────────────────────────────────────────────────
    let explanation = if relevant || combined_score >= 0.3 {
        Some(generate_relevance_explanation(
            input.title,
            cal.context_score,
            cal.interest_score,
            &matches,
            &ctx.ace_ctx,
            &raw.topics,
            &ctx.interests,
            &ctx.declared_tech,
            &matched_skill_gaps,
        ))
    } else {
        None
    };

    // ── Confidence ────────────────────────────────────────────────────
    let confidence = calculate_confidence(
        cal.context_score,
        cal.interest_score,
        cal.semantic_boost,
        &ctx.ace_ctx,
        &raw.topics,
        ctx.cached_context_count,
        ctx.interest_count as i64,
        signal_count,
    );

    // ── Confidence by signal map ──────────────────────────────────────
    let mut confidence_by_signal = HashMap::new();
    if ctx.cached_context_count > 0 {
        confidence_by_signal.insert("context".to_string(), cal.context_score);
    }
    if ctx.interest_count > 0 {
        confidence_by_signal.insert("interest".to_string(), cal.interest_score);
    }
    if cal.semantic_boost > 0.0 {
        confidence_by_signal.insert("ace_boost".to_string(), cal.semantic_boost);
    }
    if raw.dep_match_score > 0.0 {
        confidence_by_signal.insert("dependency".to_string(), raw.dep_match_score);
    }

    // ── Matched dependency names ──────────────────────────────────────
    let matched_dep_names: Vec<String> = raw
        .matched_deps
        .iter()
        .map(|d| d.package_name.clone())
        .collect();

    // ── Signal classification ─────────────────────────────────────────
    let (sig_type, sig_priority, sig_action, sig_triggers, sig_horizon) = classify_signals(
        relevant,
        combined_score,
        raw.domain_relevance,
        &content_type,
        options,
        classifier,
        input,
        ctx,
        &raw.matched_deps,
        db,
    );

    // ── Necessity scoring ─────────────────────────────────────────────
    let age_hours = input.created_at.map_or(0.0, |ts| {
        (chrono::Utc::now() - *ts).num_minutes().max(0) as f64 / 60.0
    });

    // Contradiction boost: check if item topics overlap with contradicted topics
    let contradiction_boost = if ctx.contradicted_topics.is_empty() {
        0.0
    } else {
        let overlap_count = raw
            .topics
            .iter()
            .filter(|t| ctx.contradicted_topics.contains(t.as_str()))
            .count();
        // Normalize: 1 match = 0.5, 2+ = 1.0
        match overlap_count {
            0 => 0.0,
            1 => 0.5,
            _ => 1.0,
        }
    };

    let necessity_inputs = necessity::NecessityInputs {
        dep_match_score: raw.dep_match_score,
        matched_deps: matched_dep_names.clone(),
        signal_type: sig_type.clone(),
        signal_priority: sig_priority.clone(),
        cve_severity: None, // CVE severity is folded into signal_priority by the classifier
        cvss_score: None,   // Not directly available at this pipeline stage
        affected_project_count: count_affected_projects(db, &matched_dep_names),
        skill_gap_boost,
        window_boost,
        age_hours,
        content_type: Some(content_type.slug().to_string()),
        contradiction_boost,
    };
    let mut necessity_result = necessity::compute_necessity(&necessity_inputs);

    // ── Source authority weighting for necessity ───────────────────────
    // Security items are NOT penalized — a CVE is critical regardless of source.
    // All other necessity categories are modulated by source authority.
    if necessity_result.category != necessity::NecessityCategory::SecurityVulnerability
        && necessity_result.score > 0.0
    {
        let authority = authority::source_authority(input.source_type);
        necessity_result.score = (necessity_result.score * authority).clamp(0.0, 1.0);
    }

    // ── Score breakdown ───────────────────────────────────────────────
    let score_breakdown = ScoreBreakdown {
        context_score: cal.context_score,
        interest_score: cal.interest_score,
        keyword_score: cal.keyword_score,
        ace_boost: cal.semantic_boost,
        affinity_mult: raw.affinity_mult,
        anti_penalty: raw.anti_penalty,
        freshness_mult: freshness,
        feedback_boost: raw.feedback_boost,
        source_quality_boost,
        confidence_by_signal,
        signal_count,
        confirmed_signals: confirmed_signals.clone(),
        confirmation_mult,
        dep_match_score: raw.dep_match_score,
        matched_deps: matched_dep_names,
        domain_relevance: raw.domain_relevance,
        content_quality_mult,
        novelty_mult,
        intent_boost,
        content_type: Some(content_type.slug().to_string()),
        content_dna_mult,
        competing_mult,
        stack_boost: raw.stack_boost,
        ecosystem_shift_mult,
        stack_competing_mult,
        llm_score: None,
        llm_reason: None,
        window_boost,
        matched_window_id,
        skill_gap_boost,
        necessity_score: necessity_result.score,
        necessity_reason: if necessity_result.score > 0.0 {
            Some(necessity_result.reason)
        } else {
            None
        },
        necessity_category: if necessity_result.score > 0.0 {
            Some(necessity_result.category.slug().to_string())
        } else {
            None
        },
        necessity_urgency: if necessity_result.score > 0.0 {
            Some(necessity_result.urgency.label().to_string())
        } else {
            None
        },
        signal_strength_bonus: strength_bonus,
        content_analysis_mult,
    };

    // ── STREETS revenue engine mapping ────────────────────────────────
    let streets_engine = if relevant {
        crate::streets_engine::map_to_streets_engine(
            input.title,
            input.content,
            Some(content_type.slug()),
            sig_type.as_deref(),
        )
    } else {
        None
    };

    // ── Build final result ────────────────────────────────────────────
    SourceRelevance {
        id: input.id,
        title: crate::decode_html_entities(input.title),
        url: input.url.map(std::string::ToString::to_string),
        top_score: combined_score,
        matches,
        relevant,
        context_score: cal.context_score,
        interest_score: cal.interest_score,
        excluded: false,
        excluded_by: None,
        source_type: input.source_type.to_string(),
        explanation,
        confidence: Some(confidence),
        score_breakdown: Some(score_breakdown),
        signal_type: sig_type,
        signal_priority: sig_priority,
        signal_action: sig_action,
        signal_triggers: sig_triggers,
        signal_horizon: sig_horizon,
        similar_count: 0,
        similar_titles: vec![],
        serendipity: false,
        streets_engine,
        decision_window_match: matched_window_id.and_then(|wid| {
            ctx.open_windows
                .iter()
                .find(|w| w.id == wid)
                .map(|w| w.title.clone())
        }),
        decision_boost_applied: window_boost,
        created_at: None,
        detected_lang: input.detected_lang.to_string(),
    }
}

/// Count how many distinct projects use any of the matched dependencies.
/// Returns 0 if no deps matched or the DB query fails (graceful degradation).
fn count_affected_projects(db: &Database, matched_deps: &[String]) -> usize {
    if matched_deps.is_empty() {
        return 0;
    }
    let conn = db.conn.lock();
    // Count distinct projects that have ANY of the matched deps
    let placeholders: String = matched_deps
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT COUNT(DISTINCT project_path) FROM project_dependencies WHERE LOWER(package_name) IN ({})",
        placeholders
    );
    let params: Vec<String> = matched_deps.iter().map(|d| d.to_lowercase()).collect();
    conn.query_row(
        &sql,
        rusqlite::params_from_iter(params.iter()),
        |row: &rusqlite::Row<'_>| row.get(0),
    )
    .unwrap_or(0)
}
