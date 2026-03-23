// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

// ============================================================================
// Vector Math
// ============================================================================

/// Compute L2 norm of a vector
#[inline]
pub(crate) fn vector_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Cosine similarity with precomputed norm for vector `a`
/// Use this in hot loops where you compare the same vector `a` against many `b` vectors
#[inline]
pub(crate) fn cosine_similarity_with_norm(a: &[f32], a_norm: f32, b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_b: f32 = vector_norm(b);
    if a_norm == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (a_norm * norm_b)
}

/// Cosine similarity between two vectors (used by tests; hot path uses cosine_similarity_with_norm)
#[allow(dead_code)] // Reason: only used by test modules; production code uses cosine_similarity_with_norm
pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = vector_norm(a);
    let norm_b: f32 = vector_norm(b);

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim - 1.0).abs() < 0.001,
            "Identical vectors should have similarity 1.0"
        );

        let c = vec![0.0, 1.0, 0.0];
        let sim_orth = cosine_similarity(&a, &c);
        assert!(
            sim_orth.abs() < 0.001,
            "Orthogonal vectors should have similarity 0.0"
        );
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_vector_norm_unit_vector() {
        let v = vec![1.0, 0.0, 0.0];
        assert!((vector_norm(&v) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_vector_norm_zero_vector() {
        let v = vec![0.0, 0.0, 0.0];
        assert!((vector_norm(&v) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_vector_norm_3_4_5() {
        let v = vec![3.0, 4.0];
        assert!((vector_norm(&v) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_with_norm_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let a_norm = vector_norm(&a);
        let sim = cosine_similarity_with_norm(&a, a_norm, &a);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_with_norm_zero_a() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity_with_norm(&a, 0.0, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_with_norm_zero_b() {
        let a = vec![1.0, 2.0, 3.0];
        let a_norm = vector_norm(&a);
        let b = vec![0.0, 0.0, 0.0];
        let sim = cosine_similarity_with_norm(&a, a_norm, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_with_norm_mismatched_length() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity_with_norm(&a, vector_norm(&a), &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_with_norm_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        let sim = cosine_similarity_with_norm(&a, 0.0, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_empty_vectors() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_mismatched_length() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }
}
