use std::collections::HashMap;

use crate::db::Database;
use crate::scoring_config;
use crate::signals;
use crate::{
    check_exclusions, extract_topics, get_relevance_threshold, RelevanceMatch, ScoreBreakdown,
    SourceRelevance,
};

use super::*;

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

/// Options controlling which scoring stages are applied
pub(crate) struct ScoringOptions {
    pub apply_freshness: bool,
    pub apply_signals: bool,
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
            signal_horizon: None,
            similar_count: 0,
            similar_titles: vec![],
            serendipity: false,
            streets_engine: None,
            decision_window_match: None,
            decision_boost_applied: 0.0,
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
        // Dampen semantic influence for thin-interest personas (bootstrap mode)
        // to prevent embedding-driven false positives before user has provided feedback.
        // Only applies when content has real embeddings — keyword-based ACE boost
        // (the fallback when embeddings are zero) is already well-calibrated.
        let has_real_embedding = input.embedding.iter().any(|&v| v != 0.0);
        let semantic_mult = if has_real_embedding
            && ctx.interest_count < 3
            && ctx.feedback_interaction_count < 10
        {
            scoring_config::INTEREST_ONLY_SEMANTIC_MULT * 0.4
        } else {
            scoring_config::INTEREST_ONLY_SEMANTIC_MULT
        };
        (interest_score * scoring_config::INTEREST_ONLY_INTEREST_W
            + keyword_score * scoring_config::INTEREST_ONLY_KEYWORD_W
            + semantic_boost * semantic_mult)
            .min(1.0)
    } else if ctx.cached_context_count > 0 {
        (context_score + semantic_boost).min(1.0)
    } else {
        (semantic_boost * 2.0).min(1.0)
    };

    // Dependency contribution: dep_match_score weighted into base score
    // This gives a meaningful boost without dominating the other signals.
    // In bootstrap mode (< 10 interactions), dependency matches get 2x weight
    // so first results disproportionately feature the user's actual packages.
    let dep_weight = if ctx.feedback_interaction_count < 10 {
        scoring_config::DEPENDENCY_BOOST_WEIGHT * 2.0
    } else {
        scoring_config::DEPENDENCY_BOOST_WEIGHT
    };
    let base_score = (base_score + dep_match_score * dep_weight).min(1.0);

    // Stack intelligence: pain point and keyword boost from selected profiles
    let stack_boost = crate::stacks::scoring::compute_stack_boost(
        input.title,
        input.content,
        &ctx.composed_stack,
    );
    let base_score = (base_score + stack_boost).min(1.0);

    // Optional freshness — topic-aware when autophagy half-lives are available.
    // Standard freshness uses a global decay curve; calibrated freshness adjusts
    // per-topic based on learned engagement half-lives from autophagy cycles.
    let freshness = if options.apply_freshness {
        if let Some(created_at) = input.created_at {
            let base_freshness = compute_temporal_freshness(created_at);
            // Topic-aware modulation: if autophagy learned that "rust" items stay
            // relevant for 120h but "javascript" decays in 24h, apply that knowledge
            if !ctx.topic_half_lives.is_empty() && !topics.is_empty() {
                let matching_half_lives: Vec<f32> = topics
                    .iter()
                    .filter_map(|t| ctx.topic_half_lives.get(t.as_str()).copied())
                    .collect();
                if !matching_half_lives.is_empty() {
                    let avg_half_life =
                        matching_half_lives.iter().sum::<f32>() / matching_half_lives.len() as f32;
                    let age_hours =
                        ((chrono::Utc::now() - *created_at).num_minutes() as f32 / 60.0).max(0.0);
                    // Calibrated freshness: slower decay for long-lived topics,
                    // faster decay for fast-decaying topics. Blend 50/50 with base
                    // to avoid wild swings from limited autophagy data.
                    let calibrated = (-0.693 * age_hours / avg_half_life.max(1.0)).exp();
                    (base_freshness * 0.5 + calibrated * 0.5).clamp(0.3, 1.0)
                } else {
                    base_freshness
                }
            } else {
                base_freshness
            }
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

    // Taste embedding boost: cosine similarity between item and user's holistic preference vector
    let taste_boost = match ctx.taste_embedding {
        Some(ref taste_emb) if !input.embedding.is_empty() => {
            semantic::compute_taste_boost(input.embedding, taste_emb)
        }
        _ => 0.0,
    };
    let base_score = (base_score + taste_boost).clamp(0.0, 1.0);

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

    // Ecosystem shift detection from stack profiles
    let ecosystem_shift_mult =
        crate::stacks::scoring::detect_ecosystem_shift(&topics, input.title, &ctx.composed_stack);

    // Stack-aware competing tech penalty: suppresses content about alternatives
    // when the user doesn't also mention their own tech (e.g., pure Go article for Rust user)
    let stack_competing_mult = crate::stacks::scoring::compute_competing_penalty(
        input.title,
        input.content,
        &ctx.composed_stack,
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
    let content_dna_dampened =
        if content_dna_mult < 1.0 && domain_relevance >= 1.0 && !ctx.domain_profile.is_empty() {
            1.0 + (content_dna_mult - 1.0) * scoring_config::DAMPENING_DOMAIN_AWARE_STRENGTH
        } else {
            dampen(content_dna_mult)
        };
    let composite_mult = dampen(competing_mult)
        * dampen(content_quality.multiplier)
        * content_dna_dampened
        * dampen(novelty.multiplier)
        * dampen(ecosystem_shift_mult)
        * dampen(stack_competing_mult);
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

    // Decision window boost: items matching open decision windows get a scoring boost.
    // Security patches get up to +0.20, migrations +0.15, adoption/knowledge +0.10.
    let (window_boost, matched_window_id) = if !ctx.open_windows.is_empty() {
        crate::decision_advantage::compute_decision_window_boost(
            &ctx.open_windows,
            input.title,
            input.content,
            &topics,
            &matched_deps
                .iter()
                .map(|d| d.package_name.clone())
                .collect::<Vec<_>>(),
        )
    } else {
        (0.0, None)
    };
    let base_score = (base_score + window_boost).clamp(0.0, 1.0);

    // Skill-gap boost: amplify content about dependencies the user has but hasn't engaged with.
    // Closes the intelligence loop: ACE discovers deps → profile detects gaps → scoring prioritizes.
    let mut matched_skill_gaps: Vec<String> = Vec::new();
    let skill_gap_boost: f32 = if let Some(ref profile) = ctx.sovereign_profile {
        if !profile.intelligence.skill_gaps.is_empty() {
            for t in &topics {
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
                1 => 0.15, // Single gap match (raised from 0.08)
                _ => 0.20, // Multi gap match (raised from 0.12)
            }
        } else {
            0.0
        }
    } else {
        0.0
    };
    let base_score = (base_score + skill_gap_boost).clamp(0.0, 1.0);

    // Autophagy calibration correction: if autophagy detected systematic under/over-scoring
    // for topics in this item, apply a correction. Positive delta = under-scored = boost.
    let calibration_correction: f32 = if !ctx.calibration_deltas.is_empty() && !topics.is_empty() {
        let matching: Vec<f32> = topics
            .iter()
            .filter_map(|t| ctx.calibration_deltas.get(t.as_str()).copied())
            .collect();
        if !matching.is_empty() {
            let avg_delta = matching.iter().sum::<f32>() / matching.len() as f32;
            // Clamp correction to +/-10% to prevent runaway calibration
            avg_delta.clamp(-0.10, 0.10)
        } else {
            0.0
        }
    } else {
        0.0
    };
    let base_score = (base_score + calibration_correction).clamp(0.0, 1.0);

    // Stack pain point match for ACE axis confirmation
    let stack_pain_match = crate::stacks::scoring::has_pain_point_match(
        input.title,
        input.content,
        &ctx.composed_stack,
    );

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
        stack_pain_match,
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
    let domain_gate_mult = if domain_relevance >= 1.0 && !ctx.domain_profile.is_empty() {
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

    // Quality floor: must pass threshold AND either have N+ confirmed signals or strong score.
    // Bootstrap mode: relax signal requirement for new users (< 10 feedback interactions).
    // New users often have only 1 signal axis firing (interest OR dependency), and the
    // 2-signal confirmation gate would show them nothing. After 10+ interactions,
    // the behavioral learning loop provides enough data for the full gate.
    let bootstrap_mode = ctx.feedback_interaction_count < 10;
    let min_signals = if bootstrap_mode {
        1u8
    } else {
        scoring_config::QUALITY_FLOOR_MIN_SIGNALS as u8
    };
    let relevant = combined_score >= get_relevance_threshold()
        && (signal_count >= min_signals
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
            &matched_skill_gaps,
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
        stack_boost,
        ecosystem_shift_mult,
        stack_competing_mult,
        llm_score: None,
        llm_reason: None,
        window_boost,
        matched_window_id,
        skill_gap_boost,
    };

    // Optional signal classification — four gates (all general, tech-stack-agnostic):
    // 1. Item must be relevant (passed confirmation gate + quality floor)
    // 2. combined_score >= 0.30 — no noise signals at 6% or 9% match
    // 3. domain_relevance >= 0.70 — interest-level (0.50) items aren't signal-worthy
    // 4. ShowAndTell ("I built X") requires primary-stack match (1.0) —
    //    "I built [random thing]" shouldn't be a signal unless it's about YOUR tech
    let show_and_tell_blocked =
        content_type == crate::content_dna::ContentType::ShowAndTell && domain_relevance < 1.0;
    let (sig_type, sig_priority, sig_action, sig_triggers, sig_horizon) = if options.apply_signals
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
                            && c.priority < signals::SignalPriority::High
                        {
                            c.priority = signals::SignalPriority::High;
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
                    } else if (combined_score < scoring_config::MEDIUM_SCORE_CAP
                        && c.priority > signals::SignalPriority::Medium)
                        || (combined_score > scoring_config::HIGH_SCORE_FLOOR
                            && c.priority < signals::SignalPriority::Medium)
                    {
                        c.priority = signals::SignalPriority::Medium;
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
        } else {
            (None, None, None, None, None)
        }
    } else {
        (None, None, None, None, None)
    };

    // STREETS revenue engine mapping (only for relevant items)
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
        signal_horizon: sig_horizon,
        similar_count: 0,
        similar_titles: vec![],
        serendipity: false,
        streets_engine,
        decision_window_match: matched_window_id.and_then(|wid| {
            ctx.open_windows.iter().find(|w| w.id == wid).map(|w| w.title.clone())
        }),
        decision_boost_applied: window_boost,
    }
}

// ============================================================================
// Tests (pipeline-level tests that don't belong in individual modules)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{empty_scoring_context, test_db};

    /// Helper: build a ScoringInput with a dummy 384-dim embedding
    fn test_input<'a>(title: &'a str, content: &'a str, embedding: &'a [f32]) -> ScoringInput<'a> {
        ScoringInput {
            id: 1,
            title,
            url: Some("https://example.com"),
            content,
            source_type: "hackernews",
            embedding,
            created_at: None,
        }
    }

    #[test]
    fn test_score_item_zero_context_returns_low_score() {
        let db = test_db();
        let ctx = empty_scoring_context();
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Some random article about gardening",
            "Plants and soil",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        let result = score_item(&input, &ctx, &db, &options, None);

        assert!(
            result.top_score < 0.35,
            "Empty context should produce low score, got {}",
            result.top_score
        );
        assert!(
            !result.relevant,
            "Should not be relevant with empty context"
        );
        assert!(!result.excluded, "Should not be excluded");
    }

    #[test]
    fn test_score_item_excluded_item_returns_zero() {
        let db = test_db();
        let ctx = ScoringContext::builder()
            .exclusions(vec!["blockchain".to_string()])
            .build();
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Blockchain scaling solutions for enterprise",
            "blockchain distributed ledger technology",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        let result = score_item(&input, &ctx, &db, &options, None);

        assert_eq!(result.top_score, 0.0, "Excluded item should have score 0");
        assert!(result.excluded, "Should be marked as excluded");
        assert!(
            result.excluded_by.is_some(),
            "Should report what excluded it"
        );
        assert!(
            result.excluded_by.as_ref().unwrap().contains("blockchain"),
            "Should be excluded by 'blockchain', got {:?}",
            result.excluded_by
        );
    }

    #[test]
    fn test_score_item_two_signals_can_pass() {
        let db = test_db();
        let mut ace_ctx = ace_context::ACEContext::default();
        ace_ctx.active_topics.push("rust".to_string());
        ace_ctx.topic_confidence.insert("rust".to_string(), 0.9);

        // Interest embedding matching "rust" — use a distinctive embedding
        let interest_embedding = vec![0.5_f32; 384];
        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "rust".to_string(),
            weight: 1.0,
            embedding: Some(interest_embedding.clone()),
            source: context_engine::InterestSource::Explicit,
        }];

        let ctx = ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace_ctx)
            .build();

        // Use same embedding as interest so interest_score is high
        let input = test_input(
            "Rust async runtime performance improvements",
            "rust tokio async await performance benchmarks",
            &interest_embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let breakdown = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        // Interest should be confirmed (high interest_score via same embedding)
        // ACE should be confirmed (active_topics contains "rust", title has "rust")
        assert!(
            breakdown.signal_count >= 2,
            "Expected 2+ confirmed signals, got {} ({:?})",
            breakdown.signal_count,
            breakdown.confirmed_signals
        );
    }

    #[test]
    fn test_score_item_single_signal_cannot_pass() {
        // NOTE: This test uses the default ScoringContext (feedback_interaction_count=0),
        // which is bootstrap mode (< 10). However, the confirmation gate itself caps
        // single-signal scores below the relevance threshold, so single-signal items
        // still fail even in bootstrap mode — bootstrap only relaxes the quality floor
        // signal_count check, not the gate's score ceiling.
        let db = test_db();
        // Only set up interests, no ACE context, no context chunks
        let interest_embedding = vec![0.5_f32; 384];
        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "machine learning".to_string(),
            weight: 1.0,
            embedding: Some(interest_embedding.clone()),
            source: context_engine::InterestSource::Explicit,
        }];

        let ctx = ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .build();

        // Same embedding as interest so interest_score is high,
        // but no ACE topics, no context, no dependencies
        let input = test_input(
            "Machine learning model training tips",
            "machine learning neural networks training optimization",
            &interest_embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let breakdown = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        // Only interest axis should confirm (via keyword or embedding match)
        // With just 1 signal the confirmation gate caps below the threshold
        assert!(
            breakdown.signal_count <= 1,
            "Expected at most 1 confirmed signal, got {} ({:?})",
            breakdown.signal_count,
            breakdown.confirmed_signals
        );
        assert!(
            !result.relevant,
            "Single-signal item should not pass relevance gate (score={}, signals={})",
            result.top_score, breakdown.signal_count
        );
    }

    // ========================================================================
    // Stack Intelligence pipeline integration tests
    // ========================================================================

    #[test]
    fn test_score_item_stack_boost_in_breakdown() {
        // When a Rust stack is active and content matches Rust pain points,
        // the score_breakdown should contain a positive stack_boost.
        let db = test_db();
        let rust_stack = crate::stacks::compose_profiles(&["rust_systems".to_string()]);
        let ctx = ScoringContext::builder().composed_stack(rust_stack).build();
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Understanding Pin and Send in Async Rust Lifetimes",
            "async pin send lifetime future tokio complexity borrow checker annotations",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let breakdown = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        assert!(
            breakdown.stack_boost > 0.0,
            "Rust pain point content should produce stack_boost > 0, got {}",
            breakdown.stack_boost
        );
        // The boost should be capped at 0.20 (max from scoring function)
        assert!(
            breakdown.stack_boost <= 0.20,
            "stack_boost should be capped at 0.20, got {}",
            breakdown.stack_boost
        );
    }

    #[test]
    fn test_score_item_no_stack_zero_boost() {
        // When no stack profiles are selected, stack_boost must be exactly 0.0
        // and ecosystem_shift_mult must be exactly 1.0.
        let db = test_db();
        let ctx = empty_scoring_context(); // no composed_stack → inactive
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Understanding Pin and Send in Async Rust Lifetimes",
            "async pin send lifetime future tokio complexity",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let breakdown = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        assert_eq!(
            breakdown.stack_boost, 0.0,
            "No stack selected → stack_boost must be 0.0, got {}",
            breakdown.stack_boost
        );
        assert_eq!(
            breakdown.ecosystem_shift_mult, 1.0,
            "No stack selected → ecosystem_shift_mult must be 1.0, got {}",
            breakdown.ecosystem_shift_mult
        );
        assert_eq!(
            breakdown.stack_competing_mult, 1.0,
            "No stack selected → stack_competing_mult must be 1.0, got {}",
            breakdown.stack_competing_mult
        );
    }

    #[test]
    fn test_score_item_stack_pain_match_confirms_ace_axis() {
        // When stack is active with pain point match, the pain point match should
        // contribute to ACE axis confirmation in the gate. We verify:
        // 1. ACE axis is confirmed (via stack_pain_match)
        // 2. stack_boost > 0 (pain points detected by pipeline)
        // 3. Score is higher than without stack (the boost matters)
        let db = test_db();
        let rust_stack = crate::stacks::compose_profiles(&["rust_systems".to_string()]);
        let ctx = ScoringContext::builder().composed_stack(rust_stack).build();
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Rust Borrow Checker: Ownership and Move Semantics Deep Dive",
            "borrow checker ownership move semantics lifetime annotation rust patterns",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let breakdown = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        // ACE should be confirmed (via stack_pain_match or topic overlap)
        assert!(
            breakdown.confirmed_signals.contains(&"ace".to_string()),
            "ACE axis should be confirmed with stack pain point match, got {:?}",
            breakdown.confirmed_signals
        );

        // Pain point content should produce a positive stack_boost
        assert!(
            breakdown.stack_boost > 0.0,
            "Borrow checker content should trigger Rust pain point, got stack_boost={}",
            breakdown.stack_boost
        );

        // Compare: same content WITHOUT stack should NOT have ACE confirmed
        let ctx_no_stack = empty_scoring_context();
        let result_no_stack = score_item(&input, &ctx_no_stack, &db, &options, None);
        let breakdown_ns = result_no_stack
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        assert!(
            !breakdown_ns.confirmed_signals.contains(&"ace".to_string()),
            "Without stack, ACE should NOT be confirmed (no topics, no semantic), got {:?}",
            breakdown_ns.confirmed_signals
        );
    }

    // ========================================================================
    // Existing unit tests
    // ========================================================================

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

    // ========================================================================
    // Phase 2: Pipeline Integration Tests
    // ========================================================================

    #[test]
    fn test_pipeline_stack_boost_survives_dampening() {
        // Verify stack_boost actually changes the final top_score (not dampened to nothing).
        let db = test_db();
        let rust_stack = crate::stacks::compose_profiles(&["rust_systems".to_string()]);
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Understanding Pin and Send in Async Rust Lifetimes",
            "async pin send lifetime future tokio complexity borrow checker annotations",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        // With stack
        let ctx_with_stack = ScoringContext::builder().composed_stack(rust_stack).build();
        let result_with = score_item(&input, &ctx_with_stack, &db, &options, None);

        // Without stack
        let ctx_no_stack = empty_scoring_context();
        let result_without = score_item(&input, &ctx_no_stack, &db, &options, None);

        let bd = result_with.score_breakdown.as_ref().unwrap();
        assert!(
            bd.stack_boost > 0.0,
            "Stack boost should be positive, got {}",
            bd.stack_boost
        );
        assert!(
            result_with.top_score > result_without.top_score,
            "Stack boost must survive dampening: with={} > without={}",
            result_with.top_score,
            result_without.top_score
        );
    }

    #[test]
    fn test_pipeline_ecosystem_shift_in_composite() {
        // Rust stack active, content with ecosystem shift keywords → mult > 1.0
        let db = test_db();
        let rust_stack = crate::stacks::compose_profiles(&["rust_systems".to_string()]);
        let ctx = ScoringContext::builder().composed_stack(rust_stack).build();
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Async Fn in Trait Is Finally Stable in Rust",
            "native async trait async fn in trait return position impl trait stabilization rust",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let bd = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        assert!(
            bd.ecosystem_shift_mult > 1.0,
            "Rust shift keywords should trigger ecosystem_shift_mult > 1.0, got {}",
            bd.ecosystem_shift_mult
        );
    }

    #[test]
    fn test_pipeline_competing_penalty_suppresses() {
        // Rust stack active, pure Go content → competing penalty < 1.0
        let db = test_db();
        let rust_stack = crate::stacks::compose_profiles(&["rust_systems".to_string()]);
        let ctx = ScoringContext::builder().composed_stack(rust_stack).build();
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Go 1.23 Performance Improvements for Backend Services",
            "go golang backend services performance goroutine scheduling concurrency",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let bd = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        assert!(
            bd.stack_competing_mult < 1.0,
            "Pure Go content with Rust stack should get competing penalty < 1.0, got {}",
            bd.stack_competing_mult
        );
    }

    #[test]
    fn test_pipeline_bootstrap_mode_relaxes_gate() {
        // Bootstrap mode (feedback_interaction_count < 10) relaxes the quality floor
        // to min_signals=1 instead of 2. However, the confirmation gate's SCORE ceiling
        // for 1-signal items (0.32) is already below the relevance threshold (0.35),
        // so single-signal items still can't pass even in bootstrap mode.
        //
        // This test verifies bootstrap mode doesn't BLOCK 2-signal items that would
        // otherwise pass, and that the feedback_interaction_count path is exercised.
        let db = test_db();
        let mut ace_ctx = ace_context::ACEContext::default();
        ace_ctx.active_topics.push("rust".to_string());

        let interest_embedding = vec![0.5_f32; 384];
        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "rust".to_string(),
            weight: 1.0,
            embedding: Some(interest_embedding.clone()),
            source: context_engine::InterestSource::Explicit,
        }];

        // Bootstrap mode: feedback_interaction_count = 0
        let ctx = ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace_ctx)
            .feedback_interaction_count(0) // bootstrap
            .build();

        let input = test_input(
            "Rust async runtime performance improvements",
            "rust tokio async await performance benchmarks",
            &interest_embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let bd = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        // With 2+ signals (interest + ace), the gate opens and score passes
        assert!(
            bd.signal_count >= 2,
            "Expected 2+ signals in bootstrap mode, got {}",
            bd.signal_count
        );
        assert!(
            result.relevant,
            "2-signal item should pass in bootstrap mode (score={}, signals={})",
            result.top_score, bd.signal_count
        );
    }

    #[test]
    fn test_pipeline_normal_mode_requires_two_signals() {
        // Normal mode (feedback_interaction_count >= 10) requires min_signals=2.
        // A single-signal item is blocked by BOTH the gate ceiling (0.32 < 0.35 threshold)
        // and the quality floor (signal_count < 2).
        let db = test_db();
        let interest_embedding = vec![0.5_f32; 384];
        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "machine learning".to_string(),
            weight: 1.0,
            embedding: Some(interest_embedding.clone()),
            source: context_engine::InterestSource::Explicit,
        }];

        let ctx = ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .feedback_interaction_count(50) // normal mode
            .build();

        let input = test_input(
            "Machine Learning Model Training Tips",
            "machine learning neural networks training optimization",
            &interest_embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let bd = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        assert!(
            bd.signal_count <= 1,
            "Expected at most 1 signal, got {} ({:?})",
            bd.signal_count,
            bd.confirmed_signals
        );
        assert!(
            !result.relevant,
            "Single signal must NOT pass in normal mode (score={}, signals={})",
            result.top_score, bd.signal_count
        );
    }

    #[test]
    fn test_pipeline_base_score_interest_only_path() {
        // When cached_context_count = 0 but interests exist, the interest-only
        // scoring path must produce a positive score.
        let db = test_db();
        let interest_embedding = vec![0.5_f32; 384];
        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "rust".to_string(),
            weight: 1.0,
            embedding: Some(interest_embedding.clone()),
            source: context_engine::InterestSource::Explicit,
        }];

        let ctx = ScoringContext::builder()
            .cached_context_count(0) // no context chunks
            .interest_count(1)
            .interests(interests)
            .build();

        let input = test_input(
            "Rust Async Performance Guide",
            "rust tokio async performance optimization",
            &interest_embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        let result = score_item(&input, &ctx, &db, &options, None);

        assert!(
            result.top_score > 0.0,
            "Interest-only path must produce score > 0, got {}",
            result.top_score
        );
    }

    #[test]
    fn test_pipeline_base_score_context_only_path() {
        // When cached_context_count > 0 but no interests, the context-only
        // scoring path must produce a score (even if low due to no embeddings).
        let db = test_db();

        let ctx = ScoringContext::builder()
            .cached_context_count(10) // has context chunks
            .interest_count(0) // no interests
            .build();

        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Building REST APIs with Rust and Axum",
            "axum rust web api server tokio serde",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
        };

        let result = score_item(&input, &ctx, &db, &options, None);

        // Score should be >= 0 (might be 0 if no matching context in DB, but shouldn't panic)
        assert!(
            result.top_score >= 0.0,
            "Context-only path must not panic, got score {}",
            result.top_score
        );
        assert!(!result.excluded, "Should not be excluded");
    }

    // ========================================================================
    // Existing unit tests
    // ========================================================================

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
