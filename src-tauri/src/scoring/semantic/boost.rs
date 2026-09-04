// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Main semantic ACE boost — vector-similarity scoring against ACE context topics.

use std::collections::HashMap;

use super::super::ace_context::ACEContext;
use super::super::corpus_calibration::{self as cc, SemanticCalibration};
use crate::scoring_config;
use fourda_macros::score_component;

use super::super::utils::topic_grounds;

/// Compute the semantic ACE boost using embeddings.
///
/// v29 (2026-09-04): the TOP-3 mean similarity to the user's stack elements
/// (active topics + detected tech), mapped through the corpus-calibrated
/// `SemanticCalibration`. The previous formula averaged the cosine over EVERY
/// topic and tech (~55 entries, many minted from the user's own source) and
/// subtracted a fixed 0.5: the average of one relevant and fifty unrelated
/// topics sits near 0.47 for every item, so the boost was a uniform ≈ −0.03
/// and its 0.18 confirmation threshold was reached by 0 of 20,014 live
/// breakdowns. The `mod.rs` v26 note claimed this fix had landed; it was
/// reverted on the PR branch before the squash.
///
/// Confidence/tech weights are deliberately NOT applied to the top-k mean:
/// the corpus sample the pivot is fitted from uses the same unweighted
/// definition (`corpus_calibration::stack_top_k_mean`), and the two must agree
/// or the percentiles describe a different quantity than the one scored.
pub(crate) fn compute_semantic_ace_boost(
    item_embedding: &[f32],
    ace_ctx: &ACEContext,
    topic_embeddings: &HashMap<String, Vec<f32>>,
    calibration: &SemanticCalibration,
) -> Option<f32> {
    if topic_embeddings.is_empty() {
        return None; // Fall back to keyword matching
    }

    // Pre-compute item embedding norm once (hot loop optimization)
    let item_norm = crate::vector_norm(item_embedding);
    if item_norm < f32::EPSILON {
        return None; // Zero-norm embedding can't produce meaningful similarity
    }

    // Zero-norm topic embeddings are failed-embed placeholders (provider
    // outage) — `stack_top_k_mean` skips them, so they can neither drag the
    // mean down nor fabricate a maximum-negative boost.
    let stack: Vec<&Vec<f32>> = ace_ctx
        .active_topics
        .iter()
        .chain(ace_ctx.detected_tech.iter())
        .filter_map(|t| topic_embeddings.get(t))
        .collect();
    if stack.is_empty() {
        return None;
    }

    let top_k = cc::stack_top_k_mean(item_embedding, item_norm, &stack)?;
    Some(cc::semantic_boost(top_k, calibration))
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

    fn legacy() -> SemanticCalibration {
        SemanticCalibration::default()
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

        let clean_boost = compute_semantic_ace_boost(&item, &ctx, &clean, &legacy());
        let poisoned_boost = compute_semantic_ace_boost(&item, &ctx, &poisoned, &legacy());
        assert_eq!(
            clean_boost, poisoned_boost,
            "a zero-vector topic must contribute nothing to the top-k mean"
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
        assert_eq!(
            compute_semantic_ace_boost(&item, &ctx, &embeddings, &legacy()),
            None
        );
    }

    /// The v26 ablation, now pinned: one relevant topic among many unrelated
    /// ones must NOT be averaged away. Under the old all-topic average the
    /// on-stack item's boost collapsed as unrelated topics were added.
    #[test]
    fn unrelated_topics_do_not_dilute_the_top_k_boost() {
        let item = seed_embedding("tokio async runtime");
        let mut embeddings = HashMap::new();
        embeddings.insert("tokio".to_string(), seed_embedding("tokio async runtime"));
        let focused = ctx_with_topics(&[("tokio", 0.9)]);
        let focused_boost =
            compute_semantic_ace_boost(&item, &focused, &embeddings, &legacy()).expect("boost");

        let mut noisy = focused.clone();
        for (i, name) in [
            "briefing_prompt",
            "grounding",
            "node:fs",
            "azure",
            "kubernetes",
        ]
        .iter()
        .enumerate()
        {
            noisy.active_topics.push(name.to_string());
            noisy.topic_confidence.insert(name.to_string(), 0.7);
            embeddings.insert(
                name.to_string(),
                seed_embedding(&format!("unrelated topic number {i} about {name}")),
            );
        }
        let noisy_boost =
            compute_semantic_ace_boost(&item, &noisy, &embeddings, &legacy()).expect("boost");
        // Top-3 of {1.0, unrelated, unrelated…} still carries the exact match;
        // the drop from a perfect single-topic boost is bounded by the two
        // next-best unrelated cosines, never by the size of the topic list.
        assert!(
            noisy_boost > 0.1,
            "five unrelated topics must not erase an exact stack match (got {noisy_boost})"
        );
        assert!(focused_boost >= noisy_boost);
    }

    /// The corpus calibration is what turns a top-k mean into a boost: the
    /// same item sits below the threshold under a pivot at its own level and
    /// above it under a pivot fitted to a lower corpus median.
    #[test]
    fn calibration_pivot_decides_the_threshold_crossing() {
        let item = seed_embedding("tokio async runtime");
        let ctx = ctx_with_topics(&[("tokio", 0.9)]);
        let mut embeddings = HashMap::new();
        embeddings.insert("tokio".to_string(), seed_embedding("tokio async runtime"));
        // Exact match: top-k mean = 1.0.
        let high_pivot = SemanticCalibration {
            pivot: 1.0,
            gain: 2.0,
            source: cc::CalibrationSource::Corpus,
        };
        let low_pivot = SemanticCalibration {
            pivot: 0.5,
            gain: 2.0,
            source: cc::CalibrationSource::Corpus,
        };
        let under = compute_semantic_ace_boost(&item, &ctx, &embeddings, &high_pivot).unwrap();
        let over = compute_semantic_ace_boost(&item, &ctx, &embeddings, &low_pivot).unwrap();
        assert!(under.abs() < 1e-5);
        assert!(over >= scoring_config::SEMANTIC_THRESHOLD);
    }
}
