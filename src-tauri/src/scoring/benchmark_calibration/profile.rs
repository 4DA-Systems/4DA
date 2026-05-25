// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Profile construction with real embeddings for benchmark calibration.

use std::collections::HashMap;

use super::{profile_ctx, ScoringContext};

/// Build a scoring context for the named profile, replacing dummy 0.5 vectors
/// with real embeddings from the topic_embeddings map.
pub(super) fn build_profile_with_embeddings(
    name: &str,
    topic_embeddings: &HashMap<String, Vec<f32>>,
) -> ScoringContext {
    let mut ctx = profile_ctx(name);

    // Replace interest embeddings with real ones
    for interest in &mut ctx.interests {
        if let Some(real_emb) = topic_embeddings.get(&interest.topic) {
            interest.embedding = Some(real_emb.clone());
        }
    }

    // Populate topic_embeddings map for ALL topics the pipeline will look up:
    // interest topics, ACE active_topics, and detected_tech (all lowercase keys).
    for interest in &ctx.interests {
        let lower = interest.topic.to_lowercase();
        if let Some(real_emb) = topic_embeddings
            .get(&interest.topic)
            .or_else(|| topic_embeddings.get(&lower))
        {
            ctx.topic_embeddings.insert(lower, real_emb.clone());
        }
    }
    for topic in &ctx.ace_ctx.active_topics {
        if !ctx.topic_embeddings.contains_key(topic) {
            if let Some(real_emb) = topic_embeddings.get(topic) {
                ctx.topic_embeddings.insert(topic.clone(), real_emb.clone());
            }
        }
    }
    for tech in &ctx.ace_ctx.detected_tech {
        if !ctx.topic_embeddings.contains_key(tech) {
            if let Some(real_emb) = topic_embeddings.get(tech) {
                ctx.topic_embeddings.insert(tech.clone(), real_emb.clone());
            }
        }
    }

    ctx
}
