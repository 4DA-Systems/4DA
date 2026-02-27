use once_cell::sync::OnceCell;
use std::collections::HashMap;
use tracing::{debug, warn};

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

    // Compute similarity with detected tech
    for tech in &ace_ctx.detected_tech {
        if let Some(tech_emb) = topic_embeddings.get(tech) {
            let sim = crate::cosine_similarity_with_norm(item_embedding, item_norm, tech_emb);
            weighted_sum += sim * 0.6; // Detected tech is auto-inferred, weaker than declared (1.0)
            weight_total += 0.6;
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
    use std::sync::Mutex;
    static TOPIC_EMBEDDING_CACHE: OnceCell<Mutex<HashMap<String, Vec<f32>>>> = OnceCell::new();
    static DB_LOADED: OnceCell<Mutex<bool>> = OnceCell::new();

    let cache = TOPIC_EMBEDDING_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let db_loaded = DB_LOADED.get_or_init(|| Mutex::new(false));

    // Phase 1 (sync): Load DB cache + collect topics needing embedding
    // All MutexGuard usage is scoped here so they drop before any .await
    let topics_to_embed: Vec<String> = {
        let Ok(mut cache_guard) = cache.lock() else {
            warn!(target: "4da::embeddings", "Topic cache lock poisoned, returning empty");
            return HashMap::new();
        };
        let Ok(mut db_loaded_guard) = db_loaded.lock() else {
            warn!(target: "4da::embeddings", "DB loaded lock poisoned, returning empty");
            return HashMap::new();
        };

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
            let Ok(mut cache_guard) = cache.lock() else {
                warn!(target: "4da::embeddings", "Topic cache lock poisoned after embed, returning empty");
                return HashMap::new();
            };

            let ace_conn = get_ace_engine().ok().map(|ace| ace.get_conn().clone());
            for (topic, embedding) in batch.into_iter().zip(embeddings.into_iter()) {
                if let Some(ref conn) = ace_conn {
                    let _ = ace::store_topic_embedding(conn, &topic, &embedding);
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
    let Ok(cache_guard) = cache.lock() else {
        warn!(target: "4da::embeddings", "Topic cache lock poisoned building result, returning empty");
        return HashMap::new();
    };

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
