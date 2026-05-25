// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Taste embedding — user preference vector computation and taste-based scoring boost.

use std::collections::HashMap;

/// Compute taste embedding: weighted centroid of topic affinity embeddings.
///
/// The taste embedding captures the user's holistic preference profile as a single
/// unit vector matching EMBEDDING_DIMS. Items with high cosine similarity to this vector are more
/// likely to match the user's tastes — even if they don't match any individual topic.
///
/// # Arguments
/// * `affinities` - (topic, affinity_score, confidence) triples from ACE behavior learning
/// * `topic_embeddings` - topic -> embedding map (EMBEDDING_DIMS-dim, already loaded) (already loaded)
pub(crate) fn compute_taste_embedding(
    affinities: &[(String, f32, f32)],
    topic_embeddings: &HashMap<String, Vec<f32>>,
) -> Option<Vec<f32>> {
    if affinities.is_empty() || topic_embeddings.is_empty() {
        return None;
    }

    let dim = crate::EMBEDDING_DIMS;
    let mut centroid = vec![0.0f32; dim];
    let mut total_weight = 0.0f32;

    for (topic, affinity, confidence) in affinities {
        if let Some(emb) = topic_embeddings.get(topic) {
            if emb.len() != dim {
                continue;
            }
            // Weight = affinity_score * confidence
            // Positive affinities pull toward liked content
            // Negative affinities push away from disliked content
            let weight = affinity * confidence;
            for (c, e) in centroid.iter_mut().zip(emb.iter()) {
                *c += weight * e;
            }
            total_weight += weight.abs();
        }
    }

    if total_weight < f32::EPSILON {
        return None;
    }

    // Normalize to unit vector for cosine similarity
    let norm = crate::vector_norm(&centroid);
    if norm < f32::EPSILON {
        return None;
    }
    for c in &mut centroid {
        *c /= norm;
    }

    Some(centroid)
}

/// Compute taste similarity between an item embedding and the user's taste embedding.
///
/// Returns a small boost/penalty (clamped to +/-0.08) that personalizes scoring
/// without dominating it. High similarity items get a positive nudge.
pub(crate) fn compute_taste_boost(item_embedding: &[f32], taste_embedding: &[f32]) -> f32 {
    let item_norm = crate::vector_norm(item_embedding);
    if item_norm < f32::EPSILON {
        return 0.0;
    }
    let sim = crate::cosine_similarity_with_norm(item_embedding, item_norm, taste_embedding);
    // Center around 0.4 (typical background similarity) and scale
    // sim=0.8 → +0.08, sim=0.4 → 0.0, sim=0.0 → -0.08
    ((sim - 0.4) * 0.2).clamp(-0.08, 0.08)
}
