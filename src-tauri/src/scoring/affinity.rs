use super::ace_context::ACEContext;
use super::utils::topic_overlaps;
use crate::scoring_config;
use fourda_macros::score_component;

/// Compute affinity multiplier from learned topic preferences
/// PASIFA: Applies learned affinities as multiplicative factors with confidence scaling
#[score_component(output_range = "0.3..=1.7")]
pub(crate) fn compute_affinity_multiplier(topics: &[String], ace_ctx: &ACEContext) -> f32 {
    if ace_ctx.topic_affinities.is_empty() {
        return 1.0; // No learned preferences, neutral
    }

    let mut effect_sum: f32 = 0.0;
    let mut match_count: usize = 0;

    // Both topics (from extract_topics) and affinity keys are already lowercase
    for topic in topics {
        // Check direct match
        if let Some(&(affinity, confidence)) = ace_ctx.topic_affinities.get(topic.as_str()) {
            effect_sum += affinity * confidence;
            match_count += 1;
            continue;
        }

        // Check partial matches
        for (aff_topic, &(affinity, confidence)) in &ace_ctx.topic_affinities {
            if topic.contains(aff_topic.as_str()) || aff_topic.contains(topic.as_str()) {
                effect_sum += affinity * confidence * 0.7;
                match_count += 1;
                break;
            }
        }
    }

    if match_count == 0 {
        return 1.0; // No matches, neutral
    }

    // Average effect across matched topics, then convert to multiplier [0.3, 1.7]
    // This ensures confidence directly scales the effect:
    // High confidence (1.0) + high affinity (0.8) -> effect = 0.8 -> mult = 1.56
    // Low confidence (0.3) + high affinity (0.8) -> effect = 0.24 -> mult = 1.17
    let avg_effect = effect_sum / match_count as f32;
    (1.0 + avg_effect * scoring_config::AFFINITY_EFFECT).clamp(
        scoring_config::AFFINITY_MULT_RANGE.0,
        scoring_config::AFFINITY_MULT_RANGE.1,
    )
}

/// Compute anti-topic penalty as a multiplicative factor
/// PASIFA: Items matching anti-topics get reduced score based on confidence
#[score_component(output_range = "0.0..=0.7")]
pub(crate) fn compute_anti_penalty(topics: &[String], ace_ctx: &ACEContext) -> f32 {
    if ace_ctx.anti_topics.is_empty() {
        return 0.0; // No anti-topics, no penalty
    }

    let mut total_penalty: f32 = 0.0;

    // Both topics and anti_topics are already lowercase
    for topic in topics {
        for anti_topic in &ace_ctx.anti_topics {
            if topic.contains(anti_topic.as_str()) || anti_topic.contains(topic.as_str()) {
                let confidence = ace_ctx
                    .anti_topic_confidence
                    .get(anti_topic)
                    .copied()
                    .unwrap_or(0.5);
                total_penalty += 0.3 * confidence;
                break;
            }
        }
    }

    // Cap total penalty at configured max (never fully zero out)
    total_penalty.min(scoring_config::ANTI_PENALTY_MAX)
}

/// Domain penalty for items with zero tech/topic overlap.
/// If none of the item's extracted topics match ANY of: declared_tech, detected_tech, or active_topics,
/// apply a strong penalty. No domain overlap = almost certainly noise.
#[score_component(output_range = "0.0..=0.50")]
pub(crate) fn compute_off_domain_penalty(
    topics: &[String],
    ace_ctx: &ACEContext,
    declared_tech: &[String],
) -> f32 {
    if topics.is_empty()
        || (declared_tech.is_empty()
            && ace_ctx.detected_tech.is_empty()
            && ace_ctx.active_topics.is_empty())
    {
        return 0.0;
    }

    let has_overlap = topics.iter().any(|topic| {
        declared_tech.iter().any(|tech| topic_overlaps(topic, tech))
            || ace_ctx
                .detected_tech
                .iter()
                .any(|tech| topic_overlaps(topic, tech))
            || ace_ctx
                .active_topics
                .iter()
                .any(|at| topic_overlaps(topic, at))
    });

    if has_overlap {
        0.0
    } else {
        scoring_config::OFF_DOMAIN_PENALTY
    }
}

/// Unified relevance scoring using multiplicative formula
/// PASIFA: semantic_sim * affinity_multiplier * (1.0 - anti_penalty)
pub(crate) fn compute_unified_relevance(
    base_score: f32,
    topics: &[String],
    ace_ctx: &ACEContext,
) -> f32 {
    let affinity_mult = compute_affinity_multiplier(topics, ace_ctx);
    let anti_penalty = compute_anti_penalty(topics, ace_ctx);

    // Apply multiplicative formula
    let unified_score = base_score * affinity_mult * (1.0 - anti_penalty);

    // Clamp to valid range
    unified_score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test affinity multiplier with empty context
    #[test]
    fn test_affinity_multiplier_empty_context() {
        let ctx = ACEContext::default();
        let topics = vec!["rust".to_string(), "tauri".to_string()];
        let mult = compute_affinity_multiplier(&topics, &ctx);
        assert_eq!(mult, 1.0, "Empty context should return neutral multiplier");
    }

    // Test affinity multiplier with positive affinity
    #[test]
    fn test_affinity_multiplier_positive() {
        let mut ctx = ACEContext::default();
        ctx.topic_affinities.insert("rust".to_string(), (0.8, 0.9)); // High affinity, high confidence

        let topics = vec!["rust".to_string()];
        let mult = compute_affinity_multiplier(&topics, &ctx);

        // 0.8 * 0.9 = 0.72 weighted affinity
        // 1.0 + 0.72 * 0.7 = 1.504
        assert!(mult > 1.0, "Positive affinity should boost multiplier");
        assert!(mult <= 1.7, "Multiplier should be capped at 1.7");
    }

    // Test affinity multiplier with negative affinity
    #[test]
    fn test_affinity_multiplier_negative() {
        let mut ctx = ACEContext::default();
        ctx.topic_affinities
            .insert("crypto".to_string(), (-0.9, 0.8)); // Strong dislike, high confidence

        let topics = vec!["crypto".to_string()];
        let mult = compute_affinity_multiplier(&topics, &ctx);

        assert!(mult < 1.0, "Negative affinity should reduce multiplier");
        assert!(mult >= 0.3, "Multiplier should be capped at 0.3");
    }

    // Test anti-penalty computation
    #[test]
    fn test_anti_penalty_empty_context() {
        let ctx = ACEContext::default();
        let topics = vec!["test".to_string()];
        let penalty = compute_anti_penalty(&topics, &ctx);
        assert_eq!(penalty, 0.0, "Empty context should return zero penalty");
    }

    // Test anti-penalty with matching anti-topic
    #[test]
    fn test_anti_penalty_with_match() {
        let mut ctx = ACEContext::default();
        ctx.anti_topics.push("spam".to_string());
        ctx.anti_topic_confidence.insert("spam".to_string(), 0.8);

        let topics = vec!["spam".to_string()];
        let penalty = compute_anti_penalty(&topics, &ctx);

        // 0.3 * 0.8 = 0.24
        assert!(penalty > 0.0, "Matching anti-topic should produce penalty");
        assert!(penalty <= 0.7, "Penalty should be capped at 0.7");
    }

    // Test unified relevance scoring
    #[test]
    fn test_unified_relevance_neutral() {
        let ctx = ACEContext::default();
        let topics = vec!["test".to_string()];
        let score = compute_unified_relevance(0.5, &topics, &ctx);

        // With neutral context: 0.5 * 1.0 * (1.0 - 0.0) = 0.5
        assert_eq!(score, 0.5, "Neutral context should preserve base score");
    }

    // Test unified relevance with positive affinity
    #[test]
    fn test_unified_relevance_positive_affinity() {
        let mut ctx = ACEContext::default();
        ctx.topic_affinities.insert("rust".to_string(), (0.8, 1.0));

        let topics = vec!["rust".to_string()];
        let score = compute_unified_relevance(0.5, &topics, &ctx);

        // Base 0.5 * multiplier > 1.0 * (1.0 - 0.0)
        assert!(score > 0.5, "Positive affinity should boost score");
    }

    // Test unified relevance with anti-topic
    #[test]
    fn test_unified_relevance_anti_topic() {
        let mut ctx = ACEContext::default();
        ctx.anti_topics.push("spam".to_string());
        ctx.anti_topic_confidence.insert("spam".to_string(), 1.0);

        let topics = vec!["spam".to_string()];
        let score = compute_unified_relevance(0.5, &topics, &ctx);

        // Base 0.5 * 1.0 * (1.0 - penalty)
        assert!(score < 0.5, "Anti-topic should reduce score");
    }

    // Test confidence weighting effect
    #[test]
    fn test_confidence_weighting() {
        let mut ctx_high_conf = ACEContext::default();
        ctx_high_conf
            .topic_affinities
            .insert("rust".to_string(), (0.8, 1.0));

        let mut ctx_low_conf = ACEContext::default();
        ctx_low_conf
            .topic_affinities
            .insert("rust".to_string(), (0.8, 0.3));

        let topics = vec!["rust".to_string()];

        let score_high = compute_unified_relevance(0.5, &topics, &ctx_high_conf);
        let score_low = compute_unified_relevance(0.5, &topics, &ctx_low_conf);

        assert!(
            score_high > score_low,
            "Higher confidence should produce stronger effect"
        );
    }

    // Test score clamping
    #[test]
    fn test_score_clamping() {
        let mut ctx = ACEContext::default();
        // Extreme positive affinity
        ctx.topic_affinities.insert("rust".to_string(), (1.0, 1.0));

        let topics = vec!["rust".to_string()];
        let score = compute_unified_relevance(1.0, &topics, &ctx);

        assert!(score <= 1.0, "Score should be clamped to 1.0");

        // Extreme negative
        let mut ctx_neg = ACEContext::default();
        ctx_neg
            .topic_affinities
            .insert("spam".to_string(), (-1.0, 1.0));
        ctx_neg.anti_topics.push("spam".to_string());
        ctx_neg
            .anti_topic_confidence
            .insert("spam".to_string(), 1.0);

        let score_neg = compute_unified_relevance(0.5, &["spam".to_string()], &ctx_neg);
        assert!(score_neg >= 0.0, "Score should be clamped to 0.0");
    }

    // Test partial topic matching
    #[test]
    fn test_partial_topic_match() {
        let mut ctx = ACEContext::default();
        ctx.topic_affinities.insert("rust".to_string(), (0.8, 0.9));

        // "rustlang" should partially match "rust"
        let topics = vec!["rustlang".to_string()];
        let mult = compute_affinity_multiplier(&topics, &ctx);

        assert!(mult > 1.0, "Partial match should still produce boost");
    }

    #[test]
    fn test_off_domain_penalty_with_overlap() {
        let mut ace_ctx = ACEContext::default();
        ace_ctx.detected_tech = vec!["rust".to_string()];
        let declared = vec!["rust".to_string()];
        let topics = vec!["rust".to_string(), "performance".to_string()];
        assert_eq!(
            compute_off_domain_penalty(&topics, &ace_ctx, &declared),
            0.0
        );
    }

    #[test]
    fn test_off_domain_penalty_no_overlap() {
        let ace_ctx = ACEContext::default();
        let declared = vec!["rust".to_string(), "react".to_string()];
        let topics = vec!["windows".to_string(), "automation".to_string()];
        assert_eq!(
            compute_off_domain_penalty(&topics, &ace_ctx, &declared),
            scoring_config::OFF_DOMAIN_PENALTY
        );
    }

    #[test]
    fn test_off_domain_penalty_empty_context() {
        let ace_ctx = ACEContext::default();
        let declared: Vec<String> = vec![];
        let topics = vec!["anything".to_string()];
        assert_eq!(
            compute_off_domain_penalty(&topics, &ace_ctx, &declared),
            0.0
        );
    }

    #[test]
    fn test_off_domain_penalty_active_topic_overlap() {
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics = vec!["tauri".to_string()];
        let declared: Vec<String> = vec![];
        let topics = vec!["tauri".to_string(), "desktop".to_string()];
        assert_eq!(
            compute_off_domain_penalty(&topics, &ace_ctx, &declared),
            0.0
        );
    }

    #[test]
    fn test_off_domain_penalty_false_substring_blocked() {
        // "frustrating" should NOT bypass off-domain penalty via "rust" substring
        let ace_ctx = ACEContext::default();
        let declared = vec!["rust".to_string()];
        let topics = vec!["frustrating".to_string()];
        assert_eq!(
            compute_off_domain_penalty(&topics, &ace_ctx, &declared),
            scoring_config::OFF_DOMAIN_PENALTY, // No overlap — "frustrating" != "rust"
        );
    }

    #[test]
    fn test_off_domain_penalty_legitimate_overlap() {
        // "rust-async" SHOULD match "rust" via word boundary
        let ace_ctx = ACEContext::default();
        let declared = vec!["rust".to_string()];
        let topics = vec!["rust-async".to_string()];
        assert_eq!(
            compute_off_domain_penalty(&topics, &ace_ctx, &declared),
            0.0, // Has overlap via word part
        );
    }
}
