// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Taste-based scoring boost from a precomputed taste embedding.

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
