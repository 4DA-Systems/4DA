// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use crate::context_engine;
use crate::embedding_calibration;
use crate::scoring_config;
use fourda_macros::score_component;

/// Calibrate a raw similarity score (typically compressed in [0.3-0.6]) into
/// a spread distribution using a sigmoid stretch. Uses adaptive parameters
/// from `embedding_calibration` (auto-computed from observed distribution,
/// known-model lookup, or DSL defaults). This fixes the "everything scores
/// 45-50%" problem regardless of which embedding model the user runs.
#[score_component(output_range = "0.0..=1.0")]
pub(crate) fn calibrate_score(raw: f32) -> f32 {
    if raw <= 0.0 {
        return 0.0;
    }
    if raw >= 1.0 {
        return 1.0;
    }
    let center = embedding_calibration::get_sigmoid_center();
    let scale = embedding_calibration::get_sigmoid_scale();
    1.0 / (1.0 + ((center - raw) * scale).exp())
}

/// Compute interest score by comparing item embedding against interest embeddings
#[score_component(output_range = "0.0..=1.0")]
pub(crate) fn compute_interest_score(
    item_embedding: &[f32],
    interests: &[context_engine::Interest],
) -> f32 {
    if interests.is_empty() {
        return 0.0;
    }

    // Pre-compute item embedding norm once (hot loop optimization)
    let item_norm = crate::vector_norm(item_embedding);
    if item_norm < f32::EPSILON {
        return 0.0; // Zero-norm embedding can't produce meaningful similarity
    }
    let mut max_score: f32 = 0.0;

    for interest in interests {
        if let Some(ref interest_embedding) = interest.embedding {
            let similarity =
                crate::cosine_similarity_with_norm(item_embedding, item_norm, interest_embedding);
            let specificity = embedding_specificity_weight(&interest.topic);
            let weighted = similarity * interest.weight * specificity;
            max_score = max_score.max(weighted);
        }
    }

    max_score
}

/// Known broad/generic interest terms that match too many items.
/// These get reduced keyword weight to prevent flooding.
pub(crate) const BROAD_INTEREST_TERMS: &[&str] = &[
    "open source",
    "ai",
    "ml",
    "cloud",
    "web",
    "programming",
    "software",
    "technology",
    "development",
    "coding",
    "data",
    "security",
    "devops",
    "backend",
    "frontend",
    "fullstack",
    "machine learning",
    "artificial intelligence",
    "deep learning",
    "tech",
    "engineering",
    "developer",
    "startup",
    "saas",
    "testing",
    "framework",
    "performance",
    "api",
    "database",
    "automation",
    "monitoring",
    "infrastructure",
    "containers",
    "microservices",
    "serverless",
    "tutorial",
    "best practices",
    "tooling",
];

/// Specificity weight for embedding-based interest matching.
/// Broad terms get 0.40x to prevent "Open Source" from dominating via embeddings.
pub(crate) fn embedding_specificity_weight(interest_topic: &str) -> f32 {
    let topic_lower = interest_topic.to_lowercase();
    let is_broad = BROAD_INTEREST_TERMS
        .iter()
        .any(|b| topic_lower == *b || topic_lower.contains(b));
    if is_broad {
        scoring_config::SPECIFICITY_EMBEDDING_BROAD
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_specificity_broad_attenuated() {
        assert_eq!(embedding_specificity_weight("Open Source"), 0.40);
        assert_eq!(embedding_specificity_weight("AI"), 0.40);
        assert_eq!(embedding_specificity_weight("machine learning"), 0.40);
    }

    #[test]
    fn test_embedding_specificity_specific_full() {
        assert_eq!(embedding_specificity_weight("Tauri"), 1.0);
        assert_eq!(embedding_specificity_weight("rust"), 1.0);
        assert_eq!(embedding_specificity_weight("sqlite-vss"), 1.0);
    }

    #[test]
    fn test_broad_terms_include_expanded_set() {
        for term in &[
            "testing",
            "framework",
            "performance",
            "api",
            "database",
            "automation",
            "monitoring",
            "infrastructure",
            "containers",
            "microservices",
            "serverless",
            "tutorial",
            "best practices",
            "tooling",
        ] {
            assert_eq!(
                embedding_specificity_weight(term),
                0.40,
                "'{term}' should be classified as broad"
            );
        }
    }

    // ====================================================================
    // calibrate_score tests
    // ====================================================================

    #[test]
    fn test_calibrate_score_zero() {
        assert_eq!(calibrate_score(0.0), 0.0);
    }

    #[test]
    fn test_calibrate_score_one() {
        assert_eq!(calibrate_score(1.0), 1.0);
    }

    #[test]
    fn test_calibrate_score_negative() {
        assert_eq!(calibrate_score(-0.5), 0.0);
    }

    #[test]
    fn test_calibrate_score_above_one() {
        assert_eq!(calibrate_score(1.5), 1.0);
    }

    #[test]
    fn test_calibrate_score_midpoint() {
        // At the sigmoid center, output should be close to 0.5
        let center = crate::embedding_calibration::get_sigmoid_center();
        let cal = calibrate_score(center);
        assert!(
            (cal - 0.5).abs() < 0.05,
            "At sigmoid center ({}), calibrated should be ~0.5, got {}",
            center,
            cal
        );
    }

    #[test]
    fn test_calibrate_score_monotonic() {
        // Calibration should be monotonically increasing
        let values: Vec<f32> = (0..=10).map(|i| i as f32 / 10.0).collect();
        let calibrated: Vec<f32> = values.iter().map(|&v| calibrate_score(v)).collect();
        for i in 0..calibrated.len() - 1 {
            assert!(
                calibrated[i] <= calibrated[i + 1],
                "calibrate_score should be monotonic: {} > {} at inputs ({}, {})",
                calibrated[i],
                calibrated[i + 1],
                values[i],
                values[i + 1]
            );
        }
    }

    #[test]
    fn test_calibrate_score_spreads_midrange() {
        // The typical [0.40-0.56] band should spread to a wider range
        let low_mid = calibrate_score(0.40);
        let high_mid = calibrate_score(0.56);
        let spread = high_mid - low_mid;
        assert!(
            spread > 0.3,
            "Midrange [0.40-0.56] should spread to >0.3 range, got {}",
            spread
        );
    }

    // ====================================================================
    // embedding_specificity_weight edge cases
    // ====================================================================

    #[test]
    fn test_embedding_specificity_case_insensitive() {
        assert_eq!(embedding_specificity_weight("OPEN SOURCE"), 0.40);
        assert_eq!(embedding_specificity_weight("Machine Learning"), 0.40);
    }

    #[test]
    fn test_embedding_specificity_contains_broad() {
        // "artificial intelligence" contains "ai"
        assert_eq!(
            embedding_specificity_weight("artificial intelligence"),
            0.40
        );
    }

    #[test]
    fn test_embedding_specificity_empty_string() {
        // Empty string doesn't match any broad term
        assert_eq!(embedding_specificity_weight(""), 1.0);
    }

    // ====================================================================
    // BROAD_INTEREST_TERMS coverage
    // ====================================================================

    #[test]
    fn test_broad_interest_terms_complete() {
        // Verify key terms are in the list
        let key_terms = [
            "open source",
            "ai",
            "ml",
            "cloud",
            "web",
            "programming",
            "software",
            "technology",
            "development",
            "security",
        ];
        for term in &key_terms {
            assert!(
                BROAD_INTEREST_TERMS.contains(term),
                "'{}' should be in BROAD_INTEREST_TERMS",
                term
            );
        }
    }
}
