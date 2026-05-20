// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.

//! ACE-weighted query manipulation — biases search embeddings toward the user's
//! tech context. A Python developer searching "async" gets different results than
//! a Rust developer searching "async" because the query embedding is nudged toward
//! the user's active technology context.

use tracing::debug;

/// Apply ACE context weighting to a query embedding.
///
/// Blends the raw query embedding with the user's tech context centroid.
/// `context_weight` controls the blend strength (0.0 = no bias, 0.3 = moderate).
pub(crate) fn apply_ace_weighting(
    query_embedding: &mut Vec<f32>,
    tech_embeddings: &[Vec<f32>],
    context_weight: f32,
) {
    if tech_embeddings.is_empty() || context_weight <= 0.0 {
        return;
    }

    let dim = query_embedding.len();
    if dim == 0 {
        return;
    }

    // Compute centroid of tech context embeddings
    let mut centroid = vec![0.0f32; dim];
    let mut count = 0usize;
    for emb in tech_embeddings {
        if emb.len() == dim && emb.iter().any(|&v| v != 0.0) {
            for (c, e) in centroid.iter_mut().zip(emb.iter()) {
                *c += e;
            }
            count += 1;
        }
    }

    if count == 0 {
        return;
    }

    for c in &mut centroid {
        *c /= count as f32;
    }

    // Blend: query = (1 - weight) * query + weight * centroid
    let clamped = context_weight.clamp(0.0, 0.5);
    let keep = 1.0 - clamped;
    for (q, c) in query_embedding.iter_mut().zip(centroid.iter()) {
        *q = keep * *q + clamped * c;
    }

    // Re-normalize to unit length
    let norm: f32 = query_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in query_embedding.iter_mut() {
            *v /= norm;
        }
    }

    debug!(
        target: "4da::query_weighting",
        tech_count = count,
        weight = context_weight,
        "Applied ACE context weighting to query embedding"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_tech_embeddings_is_noop() {
        let mut query = vec![1.0, 0.0, 0.0];
        apply_ace_weighting(&mut query, &[], 0.3);
        assert_eq!(query, vec![1.0, 0.0, 0.0]);
    }

    #[test]
    fn zero_weight_is_noop() {
        let mut query = vec![1.0, 0.0, 0.0];
        let tech = vec![vec![0.0, 1.0, 0.0]];
        apply_ace_weighting(&mut query, &tech, 0.0);
        assert_eq!(query, vec![1.0, 0.0, 0.0]);
    }

    #[test]
    fn blending_shifts_toward_centroid() {
        let mut query = vec![1.0, 0.0, 0.0];
        let tech = vec![vec![0.0, 1.0, 0.0]];
        apply_ace_weighting(&mut query, &tech, 0.3);
        // After blending: [0.7, 0.3, 0.0], normalized
        assert!(query[0] > query[1]); // Query direction preserved
        assert!(query[1] > 0.0); // But shifted toward tech context
        let norm: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001); // Unit normalized
    }

    #[test]
    fn weight_clamped_at_half() {
        let mut query = vec![1.0, 0.0];
        let tech = vec![vec![0.0, 1.0]];
        apply_ace_weighting(&mut query, &tech, 0.9); // Should clamp to 0.5
                                                     // At 0.5 blend: [0.5, 0.5], normalized = [0.707, 0.707]
        assert!((query[0] - query[1]).abs() < 0.01);
    }
}
