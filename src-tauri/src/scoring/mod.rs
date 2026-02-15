mod ace_context;
mod affinity;
mod calibration;
mod dedup;
mod dependencies;
mod explanation;
mod gate;
mod keywords;
mod semantic;
mod utils;

// Public API — external callers use crate::scoring::function_name unchanged
pub(crate) use ace_context::{check_ace_exclusions, get_ace_context, ACEContext};
pub(crate) use affinity::{
    compute_affinity_multiplier, compute_anti_penalty, compute_unified_relevance,
};
pub(crate) use calibration::{calibrate_score, compute_interest_score};
pub(crate) use dedup::{
    compute_serendipity_candidates, dedup_results, sort_results, topic_dedup_results,
};
pub(crate) use dependencies::{match_dependencies, VersionDelta};
pub(crate) use explanation::{
    calculate_confidence, compute_temporal_freshness, generate_relevance_explanation,
};
pub(crate) use gate::apply_confirmation_gate;
pub(crate) use semantic::{compute_semantic_ace_boost, get_topic_embeddings};
pub(crate) use utils::topic_overlaps;

use std::collections::HashMap;
use tracing::info;

use crate::context_engine;
use crate::db::Database;
use crate::scoring_config;
use crate::signals;
use crate::{
    check_exclusions, extract_topics, get_ace_engine, get_context_engine, get_relevance_threshold,
    RelevanceMatch, ScoreBreakdown, SourceRelevance,
};
use fourda_macros::ScoringBuilder;

// ============================================================================
// Unified Scoring Pipeline
// ============================================================================

/// Input data for scoring a single item
pub(crate) struct ScoringInput<'a> {
    pub id: u64,
    pub title: &'a str,
    pub url: Option<&'a str>,
    pub content: &'a str,
    pub source_type: &'a str,
    pub embedding: &'a [f32],
    pub created_at: Option<&'a chrono::DateTime<chrono::Utc>>,
}

/// Pre-loaded context for scoring (computed once per analysis run)
#[derive(ScoringBuilder)]
pub(crate) struct ScoringContext {
    pub cached_context_count: i64,
    pub interest_count: usize,
    pub interests: Vec<context_engine::Interest>,
    pub exclusions: Vec<String>,
    pub ace_ctx: ACEContext,
    pub topic_embeddings: HashMap<String, Vec<f32>>,
    /// Feedback-derived topic boosts: topic -> net_score (-1.0 to 1.0)
    pub feedback_boosts: HashMap<String, f64>,
    /// Source quality scores from learned preferences: source_type -> score (-1.0 to 1.0)
    pub source_quality: HashMap<String, f32>,
    /// User's explicitly declared tech stack (3-5 items from onboarding).
    /// Used for signal action text and priority escalation — much smaller than detected_tech.
    pub declared_tech: Vec<String>,
    /// Domain profile: graduated technology identity for domain relevance scoring
    pub domain_profile: crate::domain_profile::DomainProfile,
    /// Recent work topics from git activity (last 2h) for intent-aware scoring
    pub work_topics: Vec<String>,
}

/// Options controlling which scoring stages are applied
pub(crate) struct ScoringOptions {
    pub apply_freshness: bool,
    pub apply_signals: bool,
}

/// Build a ScoringContext by loading all needed state. Call once per analysis run.
pub(crate) async fn build_scoring_context(db: &Database) -> Result<ScoringContext, String> {
    let cached_context_count = db.context_count().map_err(|e| e.to_string())?;

    let context_engine = get_context_engine()?;
    let static_identity = context_engine
        .get_static_identity()
        .map_err(|e| format!("Failed to load context: {}", e))?;

    // User's explicit tech stack from onboarding (small, curated list)
    let declared_tech: Vec<String> = static_identity
        .tech_stack
        .iter()
        .map(|t| t.to_lowercase())
        .collect();

    let ace_ctx = get_ace_context();

    // Load recent work topics for intent-aware scoring (last 2h of git/file activity)
    let work_topics: Vec<String> = match get_ace_engine() {
        Ok(ace) => ace
            .get_recent_work_topics(2)
            .unwrap_or_default()
            .into_iter()
            .map(|(topic, _weight)| topic)
            .collect(),
        Err(_) => vec![],
    };
    let has_active_work = !work_topics.is_empty();

    let topic_embeddings = get_topic_embeddings(&ace_ctx).await;

    // Load feedback-derived topic boosts (Phase 9: feedback learning loop)
    let feedback_boosts: HashMap<String, f64> = db
        .get_feedback_topic_summary()
        .unwrap_or_default()
        .into_iter()
        .map(|f| (f.topic, f.net_score))
        .collect();

    // Load source quality preferences from ACE behavior learning
    let source_quality: HashMap<String, f32> = match get_ace_engine() {
        Ok(ace) => ace
            .get_source_preferences()
            .unwrap_or_default()
            .into_iter()
            .collect(),
        Err(_) => HashMap::new(),
    };

    // Build domain profile for graduated domain relevance scoring
    let domain_profile = {
        let conn = crate::open_db_connection()?;
        crate::domain_profile::build_domain_profile(&conn)
    };

    info!(target: "4da::ace",
        topics = ace_ctx.active_topics.len(),
        tech = ace_ctx.detected_tech.len(),
        embeddings = topic_embeddings.len(),
        feedback_topics = feedback_boosts.len(),
        source_prefs = source_quality.len(),
        domain_primary = domain_profile.primary_stack.len(),
        domain_all = domain_profile.all_tech.len(),
        has_active_work,
        "ACE context loaded for scoring"
    );

    Ok(ScoringContext {
        cached_context_count,
        interest_count: static_identity.interests.len(),
        interests: static_identity.interests,
        exclusions: static_identity.exclusions,
        ace_ctx,
        topic_embeddings,
        feedback_boosts,
        source_quality,
        declared_tech,
        domain_profile,
        work_topics,
    })
}

/// Score a single item through the full PASIFA pipeline.
/// Returns SourceRelevance with all fields populated.
pub(crate) fn score_item(
    input: &ScoringInput,
    ctx: &ScoringContext,
    db: &Database,
    options: &ScoringOptions,
    classifier: Option<&signals::SignalClassifier>,
) -> SourceRelevance {
    let topics = extract_topics(input.title, input.content);

    // Check exclusions
    let excluded_by = check_exclusions(&topics, &ctx.exclusions)
        .or_else(|| check_ace_exclusions(&topics, &ctx.ace_ctx));

    if let Some(exclusion) = excluded_by {
        return SourceRelevance {
            id: input.id,
            title: input.title.to_string(),
            url: input.url.map(|s| s.to_string()),
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
            similar_count: 0,
            similar_titles: vec![],
            serendipity: false,
        };
    }

    // KNN context search
    let matches: Vec<RelevanceMatch> =
        if ctx.cached_context_count > 0 && !input.embedding.is_empty() {
            db.find_similar_contexts(input.embedding, 3)
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

    // Raw scores from embedding similarity (compressed in ~0.40-0.56 range)
    let raw_context = matches.first().map(|m| m.similarity).unwrap_or(0.0);
    let raw_interest = compute_interest_score(input.embedding, &ctx.interests);

    // Calibrate: stretch compressed similarity scores to use full [0.05-0.95] range
    let context_score = calibrate_score(raw_context);
    let interest_score = calibrate_score(raw_interest);

    // Keyword interest matching: boosts items containing declared interest terms
    let raw_keyword_score =
        keywords::compute_keyword_interest_score(input.title, input.content, &ctx.interests);
    // Apply specificity weighting — broad interests ("Open Source", "AI") contribute less
    let specificity_weight =
        keywords::best_interest_specificity_weight(input.title, input.content, &ctx.interests);
    let keyword_score = raw_keyword_score * specificity_weight;

    // Semantic boost with keyword fallback
    let semantic_boost =
        compute_semantic_ace_boost(input.embedding, &ctx.ace_ctx, &ctx.topic_embeddings)
            .unwrap_or_else(|| semantic::compute_keyword_ace_boost(&topics, &ctx.ace_ctx));

    // Dependency intelligence: match content against user's installed packages
    let (matched_deps, dep_match_score) =
        match_dependencies(input.title, input.content, &topics, &ctx.ace_ctx);

    // Base score weighted by available data — smooth interpolation avoids cliff effects
    let base_score = if ctx.cached_context_count > 0 && ctx.interest_count > 0 {
        // Smoothly shift weight toward context as context_score increases
        // context_score=0.0 → ctx_w=0.15, context_score=1.0 → ctx_w=0.55
        let ctx_w = (scoring_config::BASE_BOTH_CONTEXT_BASE
            + context_score * scoring_config::BASE_BOTH_CONTEXT_SCALE)
            .clamp(
                scoring_config::BASE_BOTH_CONTEXT_BASE,
                scoring_config::BASE_BOTH_CONTEXT_MAX,
            );
        let remaining = 1.0 - ctx_w;
        let int_w = remaining * scoring_config::BASE_BOTH_INTEREST_SHARE; // interests get ~55% of remainder
        let kw_w = remaining * scoring_config::BASE_BOTH_KEYWORD_SHARE; // keywords get ~45% of remainder
        (context_score * ctx_w + interest_score * int_w + keyword_score * kw_w + semantic_boost)
            .min(1.0)
    } else if ctx.interest_count > 0 {
        (interest_score * scoring_config::INTEREST_ONLY_INTEREST_W
            + keyword_score * scoring_config::INTEREST_ONLY_KEYWORD_W
            + semantic_boost * scoring_config::INTEREST_ONLY_SEMANTIC_MULT)
            .min(1.0)
    } else if ctx.cached_context_count > 0 {
        (context_score + semantic_boost).min(1.0)
    } else {
        (semantic_boost * 2.0).min(1.0)
    };

    // Dependency contribution: dep_match_score weighted into base score
    // This gives a meaningful boost without dominating the other signals
    let base_score =
        (base_score + dep_match_score * scoring_config::DEPENDENCY_BOOST_WEIGHT).min(1.0);

    // Optional freshness
    let freshness = if options.apply_freshness {
        if let Some(created_at) = input.created_at {
            compute_temporal_freshness(created_at)
        } else {
            1.0
        }
    } else {
        1.0
    };
    let base_score = (base_score * freshness).clamp(0.0, 1.0);

    // Source quality boost from learned preferences (capped +/-10%)
    let source_quality_boost = ctx
        .source_quality
        .get(input.source_type)
        .copied()
        .map(|score| {
            (score * scoring_config::SOURCE_QUALITY_MULT).clamp(
                scoring_config::SOURCE_QUALITY_CAP_RANGE.0,
                scoring_config::SOURCE_QUALITY_CAP_RANGE.1,
            )
        })
        .unwrap_or(0.0);
    let base_score = (base_score + source_quality_boost).clamp(0.0, 1.0);

    // Domain relevance: graduated penalty based on technology identity
    // Replaces binary off_domain_penalty with tiered relevance (1.0 primary → 0.15 off-domain)
    let domain_relevance =
        crate::domain_profile::compute_domain_relevance(&topics, &ctx.domain_profile);
    let off_domain_penalty = if domain_relevance >= 0.85 {
        0.0 // Primary stack or dependency match — no penalty
    } else if domain_relevance >= 0.50 {
        // Interest/adjacent match — mild penalty scaling from 0 to half the max
        scoring_config::OFF_DOMAIN_PENALTY * (1.0 - domain_relevance) * 0.5
    } else {
        // Off-domain — full penalty
        scoring_config::OFF_DOMAIN_PENALTY * (1.0 - domain_relevance)
    };
    let base_score = (base_score - off_domain_penalty).clamp(0.0, 1.0);

    // Competing tech penalty: content primarily about alternatives gets demoted
    let competing_mult = crate::competing_tech::compute_competing_penalty(
        &topics,
        input.title,
        &ctx.domain_profile.primary_stack,
    );

    // Content quality: penalize clickbait, boost authoritative technical content
    let content_quality =
        crate::content_quality::compute_content_quality(input.title, input.content, input.url);

    // Content DNA: utility multiplier by content type
    let (content_type, content_dna_mult) =
        crate::content_dna::classify_content(input.title, input.content);

    // Novelty: penalize introductory content for known tech, boost releases
    let novelty = crate::novelty::compute_novelty(
        input.title,
        input.content,
        &topics,
        &ctx.domain_profile.primary_stack,
    );

    // Combine all quality multipliers as a SINGLE dampened composite.
    // Asymmetric dampening: penalties keep more teeth than boosts.
    let dampen = |m: f32| {
        if m < 1.0 {
            1.0 + (m - 1.0) * scoring_config::DAMPENING_PENALTY_STRENGTH
        } else {
            1.0 + (m - 1.0) * scoring_config::DAMPENING_BOOST_STRENGTH
        }
    };
    // Domain-aware content_dna dampening: "I built [YOUR TECH]" is valuable,
    // "I built [random thing]" is not. When domain_relevance == 1.0 (primary stack),
    // reduce content_dna penalty strength for primary stack items.
    let content_dna_dampened = if content_dna_mult < 1.0 && domain_relevance >= 1.0 {
        1.0 + (content_dna_mult - 1.0) * scoring_config::DAMPENING_DOMAIN_AWARE_STRENGTH
    } else {
        dampen(content_dna_mult)
    };
    let composite_mult = dampen(competing_mult)
        * dampen(content_quality.multiplier)
        * content_dna_dampened
        * dampen(novelty.multiplier);
    let base_score = (base_score * composite_mult).clamp(0.0, 1.0);

    // Intent boost: amplify items matching recent work topics (what you're coding RIGHT NOW)
    // If you committed code about "scoring" in the last 2h, articles about scoring get boosted
    let intent_boost: f32 = if !ctx.work_topics.is_empty() {
        let matching_work_topics = topics
            .iter()
            .filter(|t| ctx.work_topics.iter().any(|wt| topic_overlaps(t, wt)))
            .count();
        match matching_work_topics {
            0 => 0.0,
            1 => scoring_config::INTENT_BOOST_SINGLE_MATCH,
            _ => scoring_config::INTENT_BOOST_MULTI_MATCH,
        }
    } else {
        0.0
    };
    let base_score = (base_score + intent_boost).clamp(0.0, 1.0);

    // Feedback learning boost (Phase 9): apply feedback-derived topic multiplier
    let feedback_boost = if !ctx.feedback_boosts.is_empty() {
        let mut boost_sum: f64 = 0.0;
        let mut match_count = 0;
        for topic in &topics {
            if let Some(&net_score) = ctx.feedback_boosts.get(topic.as_str()) {
                boost_sum += net_score;
                match_count += 1;
            }
        }
        if match_count > 0 {
            // Scale: net_score ranges from -1.0 to 1.0
            // Apply as +-15% boost per matching topic, capped at +-20%
            ((boost_sum / match_count as f64) * scoring_config::FEEDBACK_SCALE as f64).clamp(
                scoring_config::FEEDBACK_CAP_RANGE.0 as f64,
                scoring_config::FEEDBACK_CAP_RANGE.1 as f64,
            ) as f32
        } else {
            0.0
        }
    } else {
        0.0
    };
    let base_score = (base_score + feedback_boost).clamp(0.0, 1.0);

    // Multi-signal confirmation gate: require 2+ independent axes to pass
    let affinity_mult = compute_affinity_multiplier(&topics, &ctx.ace_ctx);
    let (gated_score, signal_count, confirmation_mult, confirmed_signals) = apply_confirmation_gate(
        base_score,
        context_score,
        interest_score,
        keyword_score,
        semantic_boost,
        &ctx.ace_ctx,
        &topics,
        feedback_boost,
        affinity_mult,
        dep_match_score,
    );

    // Unified scoring (applies affinity + anti-penalty on gated score)
    let combined_score = compute_unified_relevance(gated_score, &topics, &ctx.ace_ctx);

    // Domain relevance gate: multiplicative adjustment for domain alignment.
    // Primary stack gets a BOOST (not just penalty avoidance) so it definitively
    // outranks equivalent generic content. Interest-level items get a mild discount
    // (not the harsh 0.70 from before which over-filtered). Off-domain gets crushed.
    //   1.0  primary   → 1.10x (boost — YOUR tech definitively outranks adjacent)
    //   0.85 dependency → 1.00x (neutral)
    //   0.70 adjacent   → 0.92x (mild discount)
    //   0.50 interest   → 0.82x (moderate discount)
    //   0.15 off-domain → 0.40x (crush)
    let domain_gate_mult = if domain_relevance >= 1.0 {
        scoring_config::DOMAIN_GATE_PRIMARY_BOOST
    } else if domain_relevance >= 0.85 {
        1.0 // Dependency match — neutral
    } else if domain_relevance >= 0.50 {
        // Linear ramp: ramp_base at relevance=0.50 → 1.0 at relevance=0.85
        let gap = 1.0 - scoring_config::DOMAIN_GATE_RAMP_BASE;
        scoring_config::DOMAIN_GATE_RAMP_BASE + (domain_relevance - 0.50) * (gap / 0.35)
    } else {
        scoring_config::DOMAIN_GATE_OFF_DOMAIN_MULT
    };
    let combined_score = (combined_score * domain_gate_mult).clamp(0.0, 1.0);

    // Title information floor: ultra-short titles are fundamentally low-information.
    // "where to start", "Event listeners", "a question" — regardless of keyword matches,
    // these can't be top-quality results for ANY user. Cap score so they never dominate.
    let meaningful_words = input
        .title
        .split_whitespace()
        .filter(|w| w.len() >= 2)
        .count();
    let combined_score = if meaningful_words < 3 {
        combined_score.min(scoring_config::QUALITY_FLOOR_SHORT_TITLE_CAP)
    } else {
        combined_score
    };

    // Quality floor: must pass threshold AND either have 2+ confirmed signals or strong score
    let relevant = combined_score >= get_relevance_threshold()
        && (signal_count >= scoring_config::QUALITY_FLOOR_MIN_SIGNALS as u8
            || combined_score >= scoring_config::QUALITY_FLOOR_MIN_SCORE);

    let anti_penalty = compute_anti_penalty(&topics, &ctx.ace_ctx);

    // Explanation
    let explanation = if relevant || combined_score >= 0.3 {
        Some(generate_relevance_explanation(
            input.title,
            context_score,
            interest_score,
            &matches,
            &ctx.ace_ctx,
            &topics,
            &ctx.interests,
            &ctx.declared_tech,
        ))
    } else {
        None
    };

    // Confidence (scales with confirmation count)
    let confidence = calculate_confidence(
        context_score,
        interest_score,
        semantic_boost,
        &ctx.ace_ctx,
        &topics,
        ctx.cached_context_count,
        ctx.interest_count as i64,
        signal_count,
    );

    let mut confidence_by_signal = HashMap::new();
    if ctx.cached_context_count > 0 {
        confidence_by_signal.insert("context".to_string(), context_score);
    }
    if ctx.interest_count > 0 {
        confidence_by_signal.insert("interest".to_string(), interest_score);
    }
    if semantic_boost > 0.0 {
        confidence_by_signal.insert("ace_boost".to_string(), semantic_boost);
    }
    if dep_match_score > 0.0 {
        confidence_by_signal.insert("dependency".to_string(), dep_match_score);
    }

    let matched_dep_names: Vec<String> = matched_deps
        .iter()
        .map(|d| d.package_name.clone())
        .collect();

    let score_breakdown = ScoreBreakdown {
        context_score,
        interest_score,
        keyword_score,
        ace_boost: semantic_boost,
        affinity_mult,
        anti_penalty,
        freshness_mult: freshness,
        feedback_boost,
        source_quality_boost,
        confidence_by_signal,
        signal_count,
        confirmed_signals: confirmed_signals.clone(),
        confirmation_mult,
        dep_match_score,
        matched_deps: matched_dep_names,
        domain_relevance,
        content_quality_mult: content_quality.multiplier,
        novelty_mult: novelty.multiplier,
        intent_boost,
        content_type: Some(content_type.slug().to_string()),
        content_dna_mult,
        competing_mult,
        llm_score: None,
        llm_reason: None,
    };

    // Optional signal classification — four gates (all general, tech-stack-agnostic):
    // 1. Item must be relevant (passed confirmation gate + quality floor)
    // 2. combined_score >= 0.30 — no noise signals at 6% or 9% match
    // 3. domain_relevance >= 0.70 — interest-level (0.50) items aren't signal-worthy
    // 4. ShowAndTell ("I built X") requires primary-stack match (1.0) —
    //    "I built [random thing]" shouldn't be a signal unless it's about YOUR tech
    let show_and_tell_blocked =
        content_type == crate::content_dna::ContentType::ShowAndTell && domain_relevance < 1.0;
    let (sig_type, sig_priority, sig_action, sig_triggers) = if options.apply_signals
        && relevant
        && combined_score >= 0.30
        && domain_relevance >= 0.70
        && !show_and_tell_blocked
    {
        if let Some(clf) = classifier {
            match clf.classify(
                input.title,
                input.content,
                combined_score,
                &ctx.declared_tech,
                &ctx.ace_ctx.detected_tech,
            ) {
                Some(mut c) => {
                    // Dependency-aware priority escalation:
                    // Security + non-dev dependency match → Critical
                    // Breaking change + newer version → High
                    if !matched_deps.is_empty() {
                        let has_non_dev_dep = matched_deps.iter().any(|d| !d.is_dev);
                        if c.signal_type == signals::SignalType::SecurityAlert && has_non_dev_dep {
                            c.priority = signals::SignalPriority::Critical;
                            c.action = format!(
                                "URGENT: Security issue affects your dependency {}",
                                matched_deps[0].package_name
                            );
                        } else if c.signal_type == signals::SignalType::BreakingChange
                            && matched_deps
                                .iter()
                                .any(|d| d.version_delta == VersionDelta::NewerMajor)
                        {
                            if c.priority < signals::SignalPriority::High {
                                c.priority = signals::SignalPriority::High;
                            }
                        }
                        // Add dep:package_name triggers
                        for dep in matched_deps.iter().take(2) {
                            c.triggers.push(format!("dep:{}", dep.package_name));
                        }
                    }

                    // Score-aware priority cap — low scores cannot produce HIGH priority
                    if combined_score < scoring_config::LOW_SCORE_CAP
                        && c.priority > signals::SignalPriority::Low
                    {
                        c.priority = signals::SignalPriority::Low;
                    } else if combined_score < scoring_config::MEDIUM_SCORE_CAP
                        && c.priority > signals::SignalPriority::Medium
                    {
                        c.priority = signals::SignalPriority::Medium;
                    } else if combined_score > scoring_config::HIGH_SCORE_FLOOR
                        && c.priority < signals::SignalPriority::Medium
                    {
                        c.priority = signals::SignalPriority::Medium;
                    }
                    (
                        Some(c.signal_type.slug().to_string()),
                        Some(c.priority.label().to_string()),
                        Some(c.action),
                        Some(c.triggers),
                    )
                }
                None => (None, None, None, None),
            }
        } else {
            (None, None, None, None)
        }
    } else {
        (None, None, None, None)
    };

    SourceRelevance {
        id: input.id,
        title: crate::decode_html_entities(input.title),
        url: input.url.map(|s| s.to_string()),
        top_score: combined_score,
        matches,
        relevant,
        context_score,
        interest_score,
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
        similar_count: 0,
        similar_titles: vec![],
        serendipity: false,
    }
}

// ============================================================================
// Tests (pipeline-level tests that don't belong in individual modules)
// ============================================================================

#[cfg(test)]
mod tests {
    // Test source quality boost: positive score
    #[test]
    fn test_source_quality_positive_boost() {
        let score = 0.5_f32;
        let source_score = 0.8_f32;
        let boost = (source_score * 0.10).clamp(-0.10, 0.10);
        let result = (score + boost).clamp(0.0, 1.0);
        assert!(
            (result - 0.58).abs() < 0.01,
            "Positive source should boost by up to 10%: got {}",
            result
        );
    }

    // Test source quality boost: negative reduction
    #[test]
    fn test_source_quality_negative_reduction() {
        let score = 0.5_f32;
        let source_score = -0.6_f32;
        let boost = (source_score * 0.10).clamp(-0.10, 0.10);
        let result = (score + boost).clamp(0.0, 1.0);
        assert!(
            (result - 0.44).abs() < 0.01,
            "Negative source should reduce by up to 10%: got {}",
            result
        );
    }

    // Test source quality boost: unknown source returns 0
    #[test]
    fn test_source_quality_unknown_neutral() {
        let source_quality: std::collections::HashMap<String, f32> =
            std::collections::HashMap::new();
        let boost = source_quality
            .get("unknown_source")
            .copied()
            .map(|score| (score * 0.10).clamp(-0.10, 0.10))
            .unwrap_or(0.0);
        assert_eq!(boost, 0.0, "Unknown source should be neutral");
    }

    // Test source quality boost: cap enforcement
    #[test]
    fn test_source_quality_cap_enforcement() {
        // Even with extreme source score, boost capped at +/-10%
        let extreme_positive = (2.0_f32 * 0.10).clamp(-0.10, 0.10);
        assert_eq!(extreme_positive, 0.10, "Positive boost should cap at 0.10");

        let extreme_negative = (-2.0_f32 * 0.10).clamp(-0.10, 0.10);
        assert_eq!(
            extreme_negative, -0.10,
            "Negative boost should cap at -0.10"
        );
    }

    // Phase 2: Dependency prefix filter test
    #[test]
    fn test_dependency_prefix_filtered_from_seeding() {
        let topics = vec![
            "@radix-ui/react-select",
            "@types/node",
            "react",
            "typescript",
        ];
        let filtered: Vec<_> = topics
            .into_iter()
            .filter(|t| !t.starts_with('@') && !t.contains('/') && t.len() > 2)
            .collect();
        assert_eq!(filtered, vec!["react", "typescript"]);
    }
}
