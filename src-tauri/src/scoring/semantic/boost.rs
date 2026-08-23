// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Main semantic ACE boost — vector-similarity scoring against ACE context topics.

use std::collections::HashMap;

use super::super::ace_context::ACEContext;
use crate::scoring_config;
use fourda_macros::score_component;

use super::super::utils::topic_grounds;

/// Compute semantic ACE boost using embeddings
/// PASIFA: Uses vector similarity instead of keyword matching when embeddings available
pub(crate) fn compute_semantic_ace_boost(
    item_embedding: &[f32],
    ace_ctx: &ACEContext,
    topic_embeddings: &HashMap<String, Vec<f32>>,
) -> Option<f32> {
    if topic_embeddings.is_empty() {
        return None; // Fall back to keyword matching
    }

    // Pre-compute item embedding norm once (hot loop optimization)
    let item_norm = crate::vector_norm(item_embedding);
    if item_norm < f32::EPSILON {
        return None; // Zero-norm embedding can't produce meaningful similarity
    }

    let mut max_similarity: f32 = 0.0;
    let mut weighted_sum: f32 = 0.0;
    let mut weight_total: f32 = 0.0;

    // Compute similarity with active topics.
    // Zero-norm topic embeddings are failed-embed placeholders (provider
    // outage) — their cosine is always 0, so averaging them drags every item's
    // semantic boost toward the floor. Skip them entirely.
    for topic in &ace_ctx.active_topics {
        if let Some(topic_emb) = topic_embeddings.get(topic) {
            if crate::vector_norm(topic_emb) < f32::EPSILON {
                continue;
            }
            let sim = crate::cosine_similarity_with_norm(item_embedding, item_norm, topic_emb);
            let conf = ace_ctx.topic_confidence.get(topic).copied().unwrap_or(0.5);
            weighted_sum += sim * conf;
            weight_total += conf;
            max_similarity = max_similarity.max(sim);
        }
    }

    // Compute similarity with detected tech (per-project weighted)
    // Primary project tech → 0.85 weight, secondary → 0.40 (from ace_ctx.tech_weights)
    for tech in &ace_ctx.detected_tech {
        if let Some(tech_emb) = topic_embeddings.get(tech) {
            if crate::vector_norm(tech_emb) < f32::EPSILON {
                continue; // failed-embed placeholder — never average
            }
            let sim = crate::cosine_similarity_with_norm(item_embedding, item_norm, tech_emb);
            let tech_weight = ace_ctx.tech_weights.get(tech).copied().unwrap_or(0.35);
            weighted_sum += sim * tech_weight;
            weight_total += tech_weight;
            max_similarity = max_similarity.max(sim);
        }
    }

    if weight_total == 0.0 {
        return None;
    }

    // Compute weighted average similarity
    let avg_similarity = weighted_sum / weight_total;

    // The learned-affinity multiplier that scaled this boost (±50% from
    // `topic_affinities` similarity) was REMOVED in v19 (AD-029) — the
    // semantic ACE boost is now purely stack/context similarity.
    //
    // Convert similarity (0-1) to boost (-0.3 to 0.5) range
    // High similarity (>0.7) = positive boost
    // Low similarity (<0.3) = negative boost
    let base_boost = (avg_similarity - 0.5) * 1.0; // Center around 0.5

    Some(base_boost.clamp(-0.3, 0.5))
}

/// Keyword-based ACE boost fallback when embeddings unavailable
/// Both topics (from extract_topics) and ace_ctx fields are already lowercase.
/// Strict grounding (v12): generic fragments can't earn the boost.
#[score_component(output_range = "0.0..=0.3")]
pub(crate) fn compute_keyword_ace_boost(topics: &[String], ace_ctx: &ACEContext) -> f32 {
    let mut boost: f32 = 0.0;
    for topic in topics {
        for active in &ace_ctx.active_topics {
            if topic_grounds(topic, active) {
                boost += scoring_config::ACE_ACTIVE_TOPIC_BOOST
                    * ace_ctx.topic_confidence.get(active).copied().unwrap_or(0.5);
                break;
            }
        }
        for tech in &ace_ctx.detected_tech {
            if topic_grounds(topic, tech) {
                boost += scoring_config::ACE_DETECTED_TECH_BOOST;
                break;
            }
        }
    }
    boost.clamp(0.0, scoring_config::ACE_MAX_BOOST)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::seed_embedding;

    fn ctx_with_topics(topics: &[(&str, f32)]) -> ACEContext {
        let mut ctx = ACEContext::default();
        for &(topic, conf) in topics {
            ctx.active_topics.push(topic.to_string());
            ctx.topic_confidence.insert(topic.to_string(), conf);
        }
        ctx
    }

    #[test]
    fn zero_topic_embedding_is_skipped_not_averaged() {
        let item = seed_embedding("tokio async runtime");
        let ctx = ctx_with_topics(&[("rust", 0.9), ("failed-topic", 0.9)]);

        let mut clean = HashMap::new();
        clean.insert("rust".to_string(), seed_embedding("tokio async runtime"));
        let mut poisoned = clean.clone();
        poisoned.insert(
            "failed-topic".to_string(),
            vec![0.0_f32; crate::EMBEDDING_DIMS],
        );

        let clean_boost = compute_semantic_ace_boost(&item, &ctx, &clean);
        let poisoned_boost = compute_semantic_ace_boost(&item, &ctx, &poisoned);
        assert_eq!(
            clean_boost, poisoned_boost,
            "a zero-vector topic must contribute nothing to the average"
        );
        // Sanity: the surviving real topic is identical to the item → high boost.
        assert!(clean_boost.expect("real topic present") > 0.4);
    }

    #[test]
    fn all_zero_topic_embeddings_fall_back_to_none() {
        let item = seed_embedding("tokio async runtime");
        let ctx = ctx_with_topics(&[("failed-topic", 0.9)]);
        let mut embeddings = HashMap::new();
        embeddings.insert(
            "failed-topic".to_string(),
            vec![0.0_f32; crate::EMBEDDING_DIMS],
        );

        // Only poison available → no usable signal → None (keyword fallback),
        // NOT a fabricated maximum-negative boost from an all-zero average.
        assert_eq!(compute_semantic_ace_boost(&item, &ctx, &embeddings), None);
    }
}
