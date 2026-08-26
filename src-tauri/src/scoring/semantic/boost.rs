// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Main semantic ACE boost — vector-similarity scoring against ACE context topics.

use std::collections::HashMap;

use super::super::ace_context::ACEContext;
use crate::scoring_config;
use fourda_macros::score_component;

use super::super::utils::topic_grounds;

/// How many of the closest stack elements the semantic boost averages over.
///
/// Three, not all of them. See the note at the top-K selection below for why
/// averaging over the whole context set is the wrong shape.
const SEMANTIC_TOP_K: usize = 3;

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

    // (similarity, weight) for every stack element that produced a usable
    // comparison. Collected rather than accumulated so the TOP-K can be taken
    // below — see [`SEMANTIC_TOP_K`].
    let mut scored: Vec<(f32, f32)> = Vec::new();

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
            scored.push((sim, conf));
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
            scored.push((sim, tech_weight));
        }
    }

    // Top-K by similarity, then a weighted mean of those.
    //
    // This was a weighted average over EVERY active topic and detected tech,
    // which made the boost a taste centroid: moderately similar to everything
    // and strongly similar to nothing. Averaging punished twice over — adding
    // a junk topic RAISED the score for junk-adjacent items and LOWERED it for
    // genuinely on-stack ones, because every extra term pulled the mean toward
    // the middle. The project had already measured that shape once (Phase-0
    // triage: "relevant items smear across the similarity range") and it was
    // live again with kubernetes, grpc, fourda_macros and content_dna_classifiers
    // in the average (2026-08-26 audit, A9).
    //
    // An item that is a dead-on match for ONE real stack element should score
    // high on that basis alone, and an unrelated element should be able to sit
    // in the context without dragging it down. Top-K delivers both: a junk
    // topic only affects the result if it is genuinely among the item's
    // closest matches, which is exactly when it SHOULD.
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(SEMANTIC_TOP_K);

    let weight_total: f32 = scored.iter().map(|(_, w)| w).sum();
    if weight_total == 0.0 {
        return None;
    }
    let weighted_sum: f32 = scored.iter().map(|(sim, w)| sim * w).sum();
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

#[cfg(test)]
mod top_k_tests {
    use super::*;

    /// Unit basis vector: 1.0 at `axis`, 0 elsewhere. Cosine between two
    /// distinct axes is 0; with itself, 1.
    fn axis(a: usize) -> Vec<f32> {
        let mut v = vec![0.0_f32; crate::EMBEDDING_DIMS];
        v[a % crate::EMBEDDING_DIMS] = 1.0;
        v
    }

    /// Half-way between two axes — similarity ~0.707 to each.
    fn blend(a: usize, b: usize) -> Vec<f32> {
        let mut v = vec![0.0_f32; crate::EMBEDDING_DIMS];
        v[a % crate::EMBEDDING_DIMS] = 1.0;
        v[b % crate::EMBEDDING_DIMS] = 1.0;
        v
    }

    fn ctx_with(topics: &[&str]) -> ACEContext {
        let mut c = ACEContext::default();
        for t in topics {
            c.active_topics.push((*t).to_string());
            c.topic_confidence.insert((*t).to_string(), 0.7);
        }
        c
    }

    /// THE A9 invariant. An item that matches the user's real stack must not
    /// score lower because the context also holds something unrelated to it.
    /// Under the old whole-set average, every junk topic pulled the mean toward
    /// the middle: adding one LOWERED an on-stack item and RAISED a junk-adjacent
    /// one. Needs more than SEMANTIC_TOP_K on-stack topics to be meaningful.
    #[test]
    fn an_unrelated_topic_does_not_drag_down_an_on_stack_item() {
        let item = axis(0);
        let mut embs: HashMap<String, Vec<f32>> = HashMap::new();
        for (i, t) in ["rust", "tokio", "serde", "tauri"].iter().enumerate() {
            // all close to the item, slightly apart from each other
            embs.insert(
                (*t).to_string(),
                if i == 0 { axis(0) } else { blend(0, 10 + i) },
            );
        }
        let before = compute_semantic_ace_boost(
            &item,
            &ctx_with(&["rust", "tokio", "serde", "tauri"]),
            &embs,
        )
        .expect("baseline boost");

        // Now the context also holds a topic minted from a test fixture that
        // has nothing to do with this item — the live `kubernetes` case.
        embs.insert("kubernetes".to_string(), axis(500));
        let after = compute_semantic_ace_boost(
            &item,
            &ctx_with(&["rust", "tokio", "serde", "tauri", "kubernetes"]),
            &embs,
        )
        .expect("boost with junk topic");

        assert!(
            after >= before - 1e-6,
            "an unrelated topic must not lower an on-stack item's boost: {before} -> {after}"
        );
    }

    /// The other half: matching a real stack element must be worth strictly
    /// more than matching nothing, even when the context is mostly junk.
    ///
    /// Asserted as a COMPARISON, not against an absolute floor. The absolute
    /// value depends on the similarity scale, and orthogonal unit vectors
    /// (cosine exactly 0) are not a shape real embeddings produce — an early
    /// version of this test asserted `boost > 0` on four exactly-orthogonal
    /// junk topics and failed for that reason rather than a real one.
    #[test]
    fn matching_the_stack_beats_matching_nothing() {
        let mut embs: HashMap<String, Vec<f32>> = HashMap::new();
        embs.insert("rust".to_string(), axis(0));
        for (i, t) in ["kubernetes", "grpc", "azure", "redis"].iter().enumerate() {
            embs.insert((*t).to_string(), axis(300 + i));
        }
        let topics = ["rust", "kubernetes", "grpc", "azure", "redis"];

        // An item sitting exactly on the user's real stack element…
        let on_stack = compute_semantic_ace_boost(&axis(0), &ctx_with(&topics), &embs)
            .expect("on-stack boost");
        // …versus one that matches nothing in the context at all.
        let off_stack = compute_semantic_ace_boost(&axis(900), &ctx_with(&topics), &embs)
            .expect("off-stack boost");

        assert!(
            on_stack > off_stack,
            "matching a real stack element must beat matching nothing: {on_stack} vs {off_stack}"
        );
    }

    #[test]
    fn zero_norm_topic_embeddings_are_skipped() {
        let item = axis(0);
        let mut embs: HashMap<String, Vec<f32>> = HashMap::new();
        embs.insert("rust".to_string(), axis(0));
        embs.insert("broken".to_string(), vec![0.0_f32; crate::EMBEDDING_DIMS]);
        let boost = compute_semantic_ace_boost(&item, &ctx_with(&["rust", "broken"]), &embs)
            .expect("boost");
        let solo =
            compute_semantic_ace_boost(&item, &ctx_with(&["rust"]), &embs).expect("solo boost");
        assert!(
            (boost - solo).abs() < 1e-6,
            "a failed-embed placeholder must not move the boost: {solo} vs {boost}"
        );
    }

    #[test]
    fn no_usable_embeddings_returns_none() {
        let item = axis(0);
        assert!(compute_semantic_ace_boost(&item, &ctx_with(&["rust"]), &HashMap::new()).is_none());
    }

    #[test]
    fn zero_norm_item_returns_none() {
        let mut embs: HashMap<String, Vec<f32>> = HashMap::new();
        embs.insert("rust".to_string(), axis(0));
        let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
        assert!(compute_semantic_ace_boost(&zero, &ctx_with(&["rust"]), &embs).is_none());
    }
}
