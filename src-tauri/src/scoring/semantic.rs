use once_cell::sync::OnceCell;
use std::collections::HashMap;
use tracing::debug;

use super::ace_context::ACEContext;
use super::utils::topic_overlaps;
use crate::{ace, embed_texts, get_ace_engine, scoring_config};
use fourda_macros::score_component;

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

    // Compute similarity with active topics
    for topic in &ace_ctx.active_topics {
        if let Some(topic_emb) = topic_embeddings.get(topic) {
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
            let sim = crate::cosine_similarity_with_norm(item_embedding, item_norm, tech_emb);
            let tech_weight = ace_ctx.tech_weights.get(tech).copied().unwrap_or(0.6);
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

    // Apply learned affinities as multiplier with confidence weighting
    let mut affinity_mult: f32 = 1.0;
    for (topic, &(affinity, confidence)) in &ace_ctx.topic_affinities {
        if let Some(topic_emb) = topic_embeddings.get(topic) {
            let sim = crate::cosine_similarity_with_norm(item_embedding, item_norm, topic_emb);
            if sim > 0.5 {
                // Item is similar to a topic we have affinity data for
                // Scale by both similarity and confidence
                affinity_mult += affinity * confidence * 0.3 * sim;
            }
        }
    }
    affinity_mult = affinity_mult.clamp(0.5, 1.5);

    // Convert similarity (0-1) to boost (-0.3 to 0.5) range
    // High similarity (>0.7) = positive boost
    // Low similarity (<0.3) = negative boost
    let base_boost = (avg_similarity - 0.5) * 1.0; // Center around 0.5

    Some((base_boost * affinity_mult).clamp(-0.3, 0.5))
}

/// Embed ACE topics for semantic matching
/// Uses database-persisted embeddings with in-memory cache fallback
/// Returns topic -> embedding map
pub(crate) async fn get_topic_embeddings(ace_ctx: &ACEContext) -> HashMap<String, Vec<f32>> {
    // Lazy static cache for topic embeddings
    use parking_lot::Mutex;
    static TOPIC_EMBEDDING_CACHE: OnceCell<Mutex<HashMap<String, Vec<f32>>>> = OnceCell::new();
    static DB_LOADED: OnceCell<Mutex<bool>> = OnceCell::new();

    let cache = TOPIC_EMBEDDING_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let db_loaded = DB_LOADED.get_or_init(|| Mutex::new(false));

    // Phase 1 (sync): Load DB cache + collect topics needing embedding
    // All MutexGuard usage is scoped here so they drop before any .await
    let topics_to_embed: Vec<String> = {
        let mut cache_guard = cache.lock();
        let mut db_loaded_guard = db_loaded.lock();

        // First time: load persisted embeddings from database
        if !*db_loaded_guard {
            if let Ok(ace) = get_ace_engine() {
                if let Ok(db_embeddings) = ace::load_topic_embeddings(ace.get_conn()) {
                    for (topic, embedding) in db_embeddings {
                        cache_guard.insert(topic, embedding);
                    }
                    debug!(
                        target: "4da::embeddings",
                        count = cache_guard.len(),
                        "Loaded topic embeddings from database"
                    );
                }
            }
            *db_loaded_guard = true;
        }

        // Collect topics that need embedding
        let mut needed: Vec<String> = Vec::new();
        for topic in &ace_ctx.active_topics {
            if !cache_guard.contains_key(topic) {
                needed.push(topic.clone());
            }
        }
        for tech in &ace_ctx.detected_tech {
            if !cache_guard.contains_key(tech) {
                needed.push(tech.clone());
            }
        }
        for topic in ace_ctx.topic_affinities.keys() {
            if !cache_guard.contains_key(topic) {
                needed.push(topic.clone());
            }
        }

        needed
    }; // MutexGuards dropped here - safe to .await below

    // Phase 2 (async): Generate embeddings for missing topics
    if !topics_to_embed.is_empty() {
        let batch: Vec<String> = topics_to_embed.into_iter().take(50).collect();
        let batch_len = batch.len();

        if let Ok(embeddings) = embed_texts(&batch).await {
            // Phase 3 (sync): Store results back into cache
            let mut cache_guard = cache.lock();

            let ace_conn = get_ace_engine().ok().map(|ace| ace.get_conn().clone());
            for (topic, embedding) in batch.into_iter().zip(embeddings.into_iter()) {
                if let Some(ref conn) = ace_conn {
                    if let Err(e) = ace::store_topic_embedding(conn, &topic, &embedding) {
                        tracing::warn!("Failed to store topic embedding: {e}");
                    }
                }
                cache_guard.insert(topic, embedding);
            }

            debug!(
                target: "4da::embeddings",
                generated = batch_len,
                "Generated and persisted new topic embeddings"
            );
        }
    }

    // Phase 4 (sync): Build result from cache
    let cache_guard = cache.lock();

    let mut result = HashMap::new();
    for topic in &ace_ctx.active_topics {
        if let Some(emb) = cache_guard.get(topic) {
            result.insert(topic.clone(), emb.clone());
        }
    }
    for tech in &ace_ctx.detected_tech {
        if let Some(emb) = cache_guard.get(tech) {
            result.insert(tech.clone(), emb.clone());
        }
    }
    for topic in ace_ctx.topic_affinities.keys() {
        if let Some(emb) = cache_guard.get(topic) {
            result.insert(topic.clone(), emb.clone());
        }
    }

    result
}

/// Compute taste embedding: weighted centroid of topic affinity embeddings.
///
/// The taste embedding captures the user's holistic preference profile as a single
/// 384-dim unit vector. Items with high cosine similarity to this vector are more
/// likely to match the user's tastes — even if they don't match any individual topic.
///
/// # Arguments
/// * `affinities` - (topic, affinity_score, confidence) triples from ACE behavior learning
/// * `topic_embeddings` - topic -> 384-dim embedding map (already loaded)
pub(crate) fn compute_taste_embedding(
    affinities: &[(String, f32, f32)],
    topic_embeddings: &HashMap<String, Vec<f32>>,
) -> Option<Vec<f32>> {
    if affinities.is_empty() || topic_embeddings.is_empty() {
        return None;
    }

    let dim = 384;
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

/// Keyword-based ACE boost fallback when embeddings unavailable
/// Both topics (from extract_topics) and ace_ctx fields are already lowercase
#[score_component(output_range = "0.0..=0.3")]
pub(crate) fn compute_keyword_ace_boost(topics: &[String], ace_ctx: &ACEContext) -> f32 {
    let mut boost: f32 = 0.0;
    for topic in topics {
        for active in &ace_ctx.active_topics {
            if topic_overlaps(topic, active) {
                boost += scoring_config::ACE_ACTIVE_TOPIC_BOOST
                    * ace_ctx.topic_confidence.get(active).copied().unwrap_or(0.5);
                break;
            }
        }
        for tech in &ace_ctx.detected_tech {
            if topic_overlaps(topic, tech) {
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
    use std::collections::HashMap;

    /// Helper: cosine similarity via the crate's norm-based function
    fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
        let a_norm = crate::vector_norm(a);
        crate::cosine_similarity_with_norm(a, a_norm, b)
    }

    /// Helper: build a minimal ACEContext with active topics and confidence
    fn ace_ctx_with_topics(topics: &[(&str, f32)]) -> ACEContext {
        let mut ctx = ACEContext::default();
        for &(topic, conf) in topics {
            ctx.active_topics.push(topic.to_string());
            ctx.topic_confidence.insert(topic.to_string(), conf);
        }
        ctx
    }

    #[test]
    fn test_empty_topic_embeddings_returns_none() {
        let item_emb = seed_embedding("rust programming");
        let ace_ctx = ace_ctx_with_topics(&[("rust", 0.9)]);
        let topic_embeddings: HashMap<String, Vec<f32>> = HashMap::new();

        let result = compute_semantic_ace_boost(&item_emb, &ace_ctx, &topic_embeddings);
        assert!(
            result.is_none(),
            "Empty topic embeddings should return None, got {:?}",
            result
        );
    }

    #[test]
    fn test_identical_embedding_produces_max_boost() {
        let emb = seed_embedding("rust");
        let ace_ctx = ace_ctx_with_topics(&[("rust", 1.0)]);
        let mut topic_embeddings = HashMap::new();
        topic_embeddings.insert("rust".to_string(), emb.clone());

        let result = compute_semantic_ace_boost(&emb, &ace_ctx, &topic_embeddings);
        assert!(
            result.is_some(),
            "Identical embeddings should produce a result"
        );
        let boost = result.unwrap();
        // Cosine similarity of identical unit vectors = 1.0
        // base_boost = (1.0 - 0.5) * 1.0 = 0.5, clamped to 0.5
        assert!(
            boost > 0.4,
            "Identical embedding should produce near-max boost, got {}",
            boost
        );
        assert!(
            boost <= 0.5,
            "Boost should be clamped to 0.5, got {}",
            boost
        );
    }

    #[test]
    fn test_orthogonal_embeddings_produce_zero_boost() {
        // Construct two orthogonal 384-dim unit vectors manually
        let mut emb_a = vec![0.0f32; 384];
        emb_a[0] = 1.0; // unit vector along dimension 0

        let mut emb_b = vec![0.0f32; 384];
        emb_b[1] = 1.0; // unit vector along dimension 1

        let ace_ctx = ace_ctx_with_topics(&[("topic_b", 1.0)]);
        let mut topic_embeddings = HashMap::new();
        topic_embeddings.insert("topic_b".to_string(), emb_b);

        let result = compute_semantic_ace_boost(&emb_a, &ace_ctx, &topic_embeddings);
        assert!(
            result.is_some(),
            "Should return Some for orthogonal vectors"
        );
        let boost = result.unwrap();
        // Cosine similarity of orthogonal vectors = 0.0
        // base_boost = (0.0 - 0.5) * 1.0 = -0.5, clamped to -0.3
        assert!(
            boost <= 0.0,
            "Orthogonal embeddings should produce non-positive boost, got {}",
            boost
        );
        assert!(
            boost >= -0.3,
            "Boost should be clamped to -0.3, got {}",
            boost
        );
    }

    // ====================================================================
    // Taste Embedding Tests
    // ====================================================================

    #[test]
    fn test_compute_taste_embedding_empty() {
        let affinities: Vec<(String, f32, f32)> = vec![];
        let topic_embs: HashMap<String, Vec<f32>> = HashMap::new();
        assert!(compute_taste_embedding(&affinities, &topic_embs).is_none());
    }

    #[test]
    fn test_compute_taste_embedding_single_topic() {
        let emb = seed_embedding("rust");
        let affinities = vec![("rust".to_string(), 0.8, 0.9)];
        let mut topic_embs = HashMap::new();
        topic_embs.insert("rust".to_string(), emb.clone());

        let taste = compute_taste_embedding(&affinities, &topic_embs);
        assert!(taste.is_some());
        let taste = taste.unwrap();
        assert_eq!(taste.len(), 384);

        // Should be unit normalized
        let norm = crate::vector_norm(&taste);
        assert!(
            (norm - 1.0).abs() < 0.01,
            "Taste embedding should be unit normalized, got {}",
            norm
        );

        // Should be highly similar to the input embedding
        let sim = cosine_sim(&taste, &emb);
        assert!(
            sim > 0.99,
            "Single-topic taste should be nearly identical, got {}",
            sim
        );
    }

    #[test]
    fn test_compute_taste_embedding_blends_topics() {
        let emb_a = seed_embedding("rust");
        let emb_b = seed_embedding("python");
        let affinities = vec![
            ("rust".to_string(), 0.8, 1.0),
            ("python".to_string(), 0.4, 1.0),
        ];
        let mut topic_embs = HashMap::new();
        topic_embs.insert("rust".to_string(), emb_a.clone());
        topic_embs.insert("python".to_string(), emb_b.clone());

        let taste = compute_taste_embedding(&affinities, &topic_embs).unwrap();

        // Should be more similar to rust (higher weight) than python
        let sim_rust = cosine_sim(&taste, &emb_a);
        let sim_python = cosine_sim(&taste, &emb_b);
        assert!(
            sim_rust > sim_python,
            "Taste should be more similar to higher-weighted topic: rust={:.3} python={:.3}",
            sim_rust,
            sim_python
        );
    }

    #[test]
    fn test_compute_taste_embedding_negative_affinities() {
        let emb_a = seed_embedding("rust");
        let emb_b = seed_embedding("career advice");
        let affinities = vec![
            ("rust".to_string(), 0.9, 1.0),
            ("career advice".to_string(), -0.8, 1.0),
        ];
        let mut topic_embs = HashMap::new();
        topic_embs.insert("rust".to_string(), emb_a.clone());
        topic_embs.insert("career advice".to_string(), emb_b.clone());

        let taste = compute_taste_embedding(&affinities, &topic_embs).unwrap();

        // Taste should be more similar to liked topic than disliked
        let sim_rust = cosine_sim(&taste, &emb_a);
        let sim_career = cosine_sim(&taste, &emb_b);
        assert!(
            sim_rust > sim_career,
            "Taste should prefer liked over disliked: rust={:.3} career={:.3}",
            sim_rust,
            sim_career
        );
    }

    #[test]
    fn test_taste_boost_identical() {
        let emb = seed_embedding("rust");
        let boost = compute_taste_boost(&emb, &emb);
        // Cosine similarity of identical = 1.0 → (1.0 - 0.4) * 0.2 = 0.12, clamped to 0.08
        assert!(
            boost > 0.0,
            "Identical embeddings should produce positive boost"
        );
        assert!(
            boost <= 0.08,
            "Boost should be clamped to 0.08, got {}",
            boost
        );
    }

    #[test]
    fn test_taste_boost_orthogonal() {
        let mut emb_a = vec![0.0f32; 384];
        emb_a[0] = 1.0;
        let mut emb_b = vec![0.0f32; 384];
        emb_b[1] = 1.0;

        let boost = compute_taste_boost(&emb_a, &emb_b);
        // Cosine sim = 0.0 → (0.0 - 0.4) * 0.2 = -0.08
        assert!(
            boost < 0.0,
            "Orthogonal embeddings should produce negative boost"
        );
        assert!(
            boost >= -0.08,
            "Boost should be clamped to -0.08, got {}",
            boost
        );
    }

    #[test]
    fn test_taste_boost_zero_embedding() {
        let zero = vec![0.0f32; 384];
        let taste = seed_embedding("rust");
        let boost = compute_taste_boost(&zero, &taste);
        assert!(
            (boost - 0.0).abs() < f32::EPSILON,
            "Zero embedding should produce 0 boost"
        );
    }

    #[test]
    fn test_zero_norm_embedding_handled_gracefully() {
        let zero_emb = vec![0.0f32; 384];
        let ace_ctx = ace_ctx_with_topics(&[("rust", 1.0)]);
        let mut topic_embeddings = HashMap::new();
        topic_embeddings.insert("rust".to_string(), seed_embedding("rust"));

        let result = compute_semantic_ace_boost(&zero_emb, &ace_ctx, &topic_embeddings);
        // Zero-norm item embedding returns None (checked at line 23-25)
        assert!(
            result.is_none(),
            "Zero-norm embedding should return None, got {:?}",
            result
        );
    }
}
