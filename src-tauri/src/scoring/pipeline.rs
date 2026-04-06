use std::collections::HashMap;

use crate::db::Database;
use crate::scoring_config;
use crate::signals;
use crate::{
    check_exclusions, extract_topics, get_relevance_threshold, RelevanceMatch, ScoreBreakdown,
    SourceRelevance,
};

use super::*;

/// Build a real CorroborationContext from the database for a given item.
///
/// Queries:
/// 1. How many distinct source types have items about similar topics in the last 72 hours
/// 2. Whether any matched dependency confirms the signal
/// 3. Whether any open signal chain covers this topic and its current phase
pub(super) fn build_corroboration(
    db: &Database,
    topics: &[String],
    matched_deps: &[dependencies::DepMatch],
) -> signals::CorroborationContext {
    if topics.is_empty() {
        return signals::CorroborationContext::default();
    }

    // 1. Count distinct source types with items about the same topics in last 72 hours
    let source_count = {
        let conn = db.conn.lock();
        let topic_like_clauses: Vec<String> = topics
            .iter()
            .take(5) // Limit to top 5 topics for query performance
            .map(|t| {
                format!(
                    "LOWER(title) LIKE '%{}%'",
                    t.to_lowercase().replace('\'', "''")
                )
            })
            .collect();

        if topic_like_clauses.is_empty() {
            1_usize
        } else {
            let where_clause = topic_like_clauses.join(" OR ");
            let query = format!(
                "SELECT COUNT(DISTINCT source_type) FROM source_items \
                 WHERE created_at >= datetime('now', '-3 days') AND ({where_clause})"
            );
            conn.query_row(&query, [], |row| row.get::<_, i64>(0))
                .unwrap_or(1) as usize
        }
    };

    // 2. Dependency match — already computed by the pipeline
    let dependency_match = !matched_deps.is_empty() && matched_deps.iter().any(|d| !d.is_dev);

    // 3. Signal chain phase — detect if topics appear across multiple days
    //    (lightweight chain detection without the full detect_chains() machinery)
    let chain_phase = {
        let conn = db.conn.lock();
        let mut phase: Option<String> = None;
        for topic in topics.iter().take(3) {
            let topic_lower = topic.to_lowercase();
            // Count distinct days this topic has appeared in source items (last 7 days)
            let day_count: i64 = conn
                .query_row(
                    "SELECT COUNT(DISTINCT DATE(created_at)) FROM source_items \
                     WHERE created_at >= datetime('now', '-7 days') AND LOWER(title) LIKE ?1",
                    rusqlite::params![format!("%{}%", topic_lower)],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            if day_count >= 4 {
                phase = Some("peak".to_string());
                break;
            } else if day_count >= 3 {
                phase = Some("escalating".to_string());
                break;
            } else if day_count >= 2 && phase.is_none() {
                phase = Some("active".to_string());
            }
        }
        phase
    };

    signals::CorroborationContext {
        source_count,
        dependency_match,
        chain_phase,
    }
}

/// Input data for scoring a single item
pub(crate) struct ScoringInput<'a> {
    pub id: u64,
    pub title: &'a str,
    pub url: Option<&'a str>,
    pub content: &'a str,
    pub source_type: &'a str,
    pub embedding: &'a [f32],
    pub created_at: Option<&'a chrono::DateTime<chrono::Utc>>,
    pub detected_lang: &'a str,
}

/// Options controlling which scoring stages are applied
pub(crate) struct ScoringOptions {
    pub apply_freshness: bool,
    pub apply_signals: bool,
    pub trend_topics: Vec<String>,
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
            created_at: input.created_at.map(chrono::DateTime::to_rfc3339),
            detected_lang: input.detected_lang.to_string(),
        };
    }

    // Language gate: detect mismatch between content and user language
    let user_lang = crate::i18n::get_user_language();
    let lang_mismatch = !input.detected_lang.is_empty() && input.detected_lang != user_lang;

    // KNN context search — must check for real (non-zero) embeddings, not just non-empty.
    // Zero-vector fallback (when Ollama is unavailable) produces identical KNN distances
    // for all items, collapsing context_score to a uniform value.
    let has_real_embedding = input.embedding.iter().any(|&v| v != 0.0);
    let matches: Vec<RelevanceMatch> = if ctx.cached_context_count > 0 && has_real_embedding {
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

    // Raw scores from embedding similarity (compressed in ~0.40-0.56 range)
    let raw_context = matches.first().map_or(0.0, |m| m.similarity);
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
        (semantic_boost * 1.5).min(1.0)
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
                if matching_half_lives.is_empty() {
                    base_freshness
                } else {
                    let avg_half_life =
                        matching_half_lives.iter().sum::<f32>() / matching_half_lives.len() as f32;
                    let age_hours =
                        ((chrono::Utc::now() - *created_at).num_minutes() as f32 / 60.0).max(0.0);
                    // Calibrated freshness: slower decay for long-lived topics,
                    // faster decay for fast-decaying topics. Blend 50/50 with base
                    // to avoid wild swings from limited autophagy data.
                    let calibrated = (-0.693 * age_hours / avg_half_life.max(1.0)).exp();
                    (base_freshness * 0.5 + calibrated * 0.5).clamp(0.3, 1.0)
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
    let base_score = (base_score + source_quality_boost).clamp(0.0, 1.0);

    // Taste embedding boost: cosine similarity between item and user's holistic preference vector
    let taste_boost = match ctx.taste_embedding {
        Some(ref taste_emb) if has_real_embedding => {
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

    // Content DNA: source-type-aware utility multiplier
    let (content_type, content_dna_mult) = crate::content_dna::classify_content_for_source(
        input.title,
        input.content,
        input.source_type,
    );

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

    // ── CVE/Security dependency validation ──
    // Security items about packages NOT in user's actual dependencies get strongly
    // penalized. Prevents Budibase/pyLoad CVEs scoring 90%+ when user doesn't
    // use them. The dep_match_score check validates against Cargo.toml/package.json.
    let base_score = if novelty.is_security && dep_match_score < 0.10 && !matched_deps.is_empty() {
        // Some deps matched but very weakly — mild penalty
        base_score * 0.60
    } else if novelty.is_security && matched_deps.is_empty() {
        // No dependency match at all — this CVE is about software the user doesn't use
        base_score * 0.35
    } else {
        base_score
    };

    // ── Primary tech release boost ──
    // Releases of the user's primary languages/frameworks deserve a strong additive
    // boost. "Announcing Rust 1.94.0" should score 70%+, not 52%.
    let base_score = if novelty.is_release {
        let is_primary_release = topics
            .iter()
            .any(|t| ctx.domain_profile.primary_stack.contains(&t.to_lowercase()));
        if is_primary_release {
            (base_score + 0.15).min(1.0)
        } else {
            base_score
        }
    } else {
        base_score
    };

    // Intent boost: amplify items matching recent work topics (what you're coding RIGHT NOW)
    // If you committed code about "scoring" in the last 2h, articles about scoring get boosted
    let intent_boost: f32 = if ctx.work_topics.is_empty() {
        0.0
    } else {
        let matching_work_topics = topics
            .iter()
            .filter(|t| ctx.work_topics.iter().any(|wt| topic_overlaps(t, wt)))
            .count();
        match matching_work_topics {
            0 => 0.0,
            1 => scoring_config::INTENT_BOOST_SINGLE_MATCH,
            _ => scoring_config::INTENT_BOOST_MULTI_MATCH,
        }
    };
    let base_score = (base_score + intent_boost).clamp(0.0, 1.0);

    // Feedback learning boost (Phase 9): apply feedback-derived topic multiplier
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
            // Scale: net_score ranges from -1.0 to 1.0
            // Apply as +-15% boost per matching topic, capped at +-20%
            ((boost_sum / match_count as f64) * scoring_config::FEEDBACK_SCALE as f64).clamp(
                scoring_config::FEEDBACK_CAP_RANGE.0 as f64,
                scoring_config::FEEDBACK_CAP_RANGE.1 as f64,
            ) as f32
        } else {
            0.0
        }
    };
    let base_score = (base_score + feedback_boost).clamp(0.0, 1.0);

    // Decision window boost: items matching open decision windows get a scoring boost.
    // Security patches get up to +0.20, migrations +0.15, adoption/knowledge +0.10.
    let (window_boost, matched_window_id) = if ctx.open_windows.is_empty() {
        (0.0, None)
    } else {
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
    };
    let base_score = (base_score + window_boost).clamp(0.0, 1.0);

    // Skill-gap boost: amplify content about dependencies the user has but hasn't engaged with.
    // Closes the intelligence loop: ACE discovers deps → profile detects gaps → scoring prioritizes.
    let mut matched_skill_gaps: Vec<String> = Vec::new();
    let skill_gap_boost: f32 = if let Some(ref profile) = ctx.sovereign_profile {
        if profile.intelligence.skill_gaps.is_empty() {
            0.0
        } else {
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
        if matching.is_empty() {
            0.0
        } else {
            let avg_delta = matching.iter().sum::<f32>() / matching.len() as f32;
            // Clamp correction to +/-10% to prevent runaway calibration
            avg_delta.clamp(-0.10, 0.10)
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
        specificity_weight,
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

    // Language mismatch cap: foreign content cannot exceed 0.05 (well below 0.35 threshold)
    let combined_score = if lang_mismatch {
        combined_score.min(0.05)
    } else {
        combined_score
    };

    // Quality floor: must pass threshold AND either have N+ confirmed signals or strong score.
    // Always require at least 2 signals — single-signal items are false positives too often.
    // The confirmation gate already caps single-signal scores at 0.28 (below 0.35 threshold),
    // but this explicit floor prevents any bypass path.
    let min_signals = scoring_config::QUALITY_FLOOR_MIN_SIGNALS.max(2.0) as u8;
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
        necessity_score: 0.0,
        necessity_reason: None,
        necessity_category: None,
        necessity_urgency: None,
        signal_strength_bonus: 0.0, // V1 pipeline: no strength bonus
        content_analysis_mult: 1.0, // V1 pipeline: no content analysis
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
            let corroboration = build_corroboration(db, &topics, &matched_deps);
            match clf.classify(
                input.title,
                input.content,
                combined_score,
                &ctx.declared_tech,
                &ctx.ace_ctx.detected_tech,
                &corroboration,
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
                                "Critical: Security issue affects your dependency {}",
                                matched_deps[0].package_name
                            );
                        } else if c.signal_type == signals::SignalType::BreakingChange
                            && matched_deps
                                .iter()
                                .any(|d| d.version_delta == VersionDelta::NewerMajor)
                            && c.priority < signals::SignalPriority::Alert
                        {
                            c.priority = signals::SignalPriority::Alert;
                        }
                        // Add dep:package_name triggers
                        for dep in matched_deps.iter().take(2) {
                            c.triggers.push(format!("dep:{}", dep.package_name));
                        }
                    }

                    // Score-aware priority cap — low scores cannot produce HIGH priority
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
        url: input.url.map(std::string::ToString::to_string),
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
            ctx.open_windows
                .iter()
                .find(|w| w.id == wid)
                .map(|w| w.title.clone())
        }),
        decision_boost_applied: window_boost,
        created_at: input.created_at.map(chrono::DateTime::to_rfc3339),
        detected_lang: input.detected_lang.to_string(),
    }
}

// --- Sibling modules ---

#[path = "pipeline_tests.rs"]
mod pipeline_tests;
