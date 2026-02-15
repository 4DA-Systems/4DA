use crate::context_engine;
use crate::scoring_config;
use fourda_macros::score_component;

/// Calibrate a raw similarity score (typically compressed in [0.3-0.6]) into
/// a spread distribution using a sigmoid stretch. Centers at 0.48 (empirical
/// midpoint for text-embedding-3-small L2 distances) and scales to use the
/// full [0.05-0.95] range. This fixes the "everything scores 45-50%" problem.
#[score_component(output_range = "0.0..=1.0")]
pub(crate) fn calibrate_score(raw: f32) -> f32 {
    if raw <= 0.0 {
        return 0.0;
    }
    if raw >= 1.0 {
        return 1.0;
    }
    // Sigmoid stretch: 1 / (1 + exp((center - raw) * scale))
    // center=0.48, scale=12 maps the typical [0.40-0.56] band to [0.15-0.85]
    // (scale=20 was too aggressive, compressing near edges)
    1.0 / (1.0 + ((scoring_config::SIGMOID_CENTER - raw) * scoring_config::SIGMOID_SCALE).exp())
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
}
