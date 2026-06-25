// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Topic embedding cache — 4-phase lazy-load with DB persistence and in-memory fallback.

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use tracing::debug;

use super::super::ace_context::ACEContext;
use super::enrichment::enrich_topic_for_embedding;
use crate::{ace, embed_texts, get_ace_engine};

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
        for dep_name in ace_ctx.dependency_info.keys() {
            if !cache_guard.contains_key(dep_name) {
                needed.push(dep_name.clone());
            }
        }

        needed
    }; // MutexGuards dropped here - safe to .await below

    // Phase 2 (async): Generate embeddings for missing topics
    // Enrich bare names with descriptive text for higher-quality embeddings
    if !topics_to_embed.is_empty() {
        let batch: Vec<String> = topics_to_embed.into_iter().take(50).collect();
        let batch_len = batch.len();
        let enriched: Vec<String> = batch
            .iter()
            .map(|t| enrich_topic_for_embedding(t))
            .collect();

        if let Ok(embeddings) = embed_texts(&enriched).await {
            // Phase 3 (sync): Store results back into cache
            let mut cache_guard = cache.lock();

            let ace_conn = get_ace_engine().ok().map(|ace| ace.get_conn().clone());
            for (topic, embedding) in batch.into_iter().zip(embeddings) {
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
    for dep_name in ace_ctx.dependency_info.keys() {
        if let Some(emb) = cache_guard.get(dep_name) {
            result.insert(dep_name.clone(), emb.clone());
        }
    }

    result
}
