// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Topic embedding storage and retrieval via sqlite-vec.

use parking_lot::Mutex;
use rusqlite::Connection;
use std::sync::Arc;
use tracing::{debug, info};

use crate::error::{Result, ResultExt};

use super::embedding::EmbeddingService;
use super::ACE;

// ============================================================================
// Embedding Helpers
// ============================================================================

/// Convert embedding vector to blob for SQLite storage
pub fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert blob from SQLite to embedding vector.
/// Returns empty vec on invalid blobs instead of panicking.
pub fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    if blob.is_empty() {
        return Vec::new();
    }
    if blob.len() % 4 != 0 {
        tracing::warn!(
            target: "4da::ace",
            "blob_to_embedding: blob length {} is not divisible by 4, returning empty",
            blob.len()
        );
        return Vec::new();
    }
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

// ============================================================================
// Module-level Functions
// ============================================================================

/// Store a topic embedding in the database and vec0 index
pub fn store_topic_embedding(
    conn: &Arc<Mutex<Connection>>,
    topic: &str,
    embedding: &[f32],
) -> Result<()> {
    let conn = conn.lock();
    let embedding_blob = embedding_to_blob(embedding);

    // Get the topic's rowid
    let topic_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM active_topics WHERE topic = ?1",
            rusqlite::params![topic],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = topic_id {
        // Update the embedding in active_topics
        conn.execute(
            "UPDATE active_topics SET embedding = ?1 WHERE id = ?2",
            rusqlite::params![embedding_blob, id],
        )
        .context("Failed to update topic embedding")?;

        // Update or insert into vec0 index
        // First try to update existing, then insert if not found
        let updated = conn
            .execute(
                "UPDATE topic_vec SET embedding = ?1 WHERE rowid = ?2",
                rusqlite::params![embedding_blob, id],
            )
            .unwrap_or(0);

        if updated == 0 {
            // Insert with explicit rowid matching the topic id
            conn.execute(
                "INSERT OR REPLACE INTO topic_vec (rowid, embedding) VALUES (?1, ?2)",
                rusqlite::params![id, embedding_blob],
            )
            .context("Failed to insert topic into vec0")?;
        }
    }

    Ok(())
}

/// Load all topic embeddings from the database
pub fn load_topic_embeddings(
    conn: &Arc<Mutex<Connection>>,
) -> Result<std::collections::HashMap<String, Vec<f32>>> {
    let conn = conn.lock();
    let mut result = std::collections::HashMap::new();

    let mut stmt = conn.prepare(
        "SELECT topic, embedding FROM active_topics
             WHERE embedding IS NOT NULL
             AND julianday('now') - julianday(last_seen) <= 7",
    )?;

    let rows = stmt.query_map([], |row| {
        let topic: String = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;
        Ok((topic, blob))
    })?;

    for (topic, blob) in rows.flatten() {
        let embedding = blob_to_embedding(&blob);
        result.insert(topic, embedding);
    }

    debug!(
        target: "ace::embedding",
        count = result.len(),
        "Loaded topic embeddings from database"
    );

    Ok(result)
}

/// Generate embeddings for topics that don't have them
/// Returns count of topics updated
#[allow(dead_code)] // Reason: batch embedding generation not yet wired into startup pipeline
pub async fn generate_missing_topic_embeddings(conn: &Arc<Mutex<Connection>>) -> Result<usize> {
    // Find topics without embeddings
    let topics_without_embeddings: Vec<(i64, String)> = {
        let conn_guard = conn.lock();
        let mut stmt = conn_guard.prepare(
            "SELECT id, topic FROM active_topics
                 WHERE embedding IS NULL
                 AND julianday('now') - julianday(last_seen) <= 7
                 LIMIT 50",
        )?;

        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };

    if topics_without_embeddings.is_empty() {
        return Ok(0);
    }

    info!(
        target: "ace::embedding",
        count = topics_without_embeddings.len(),
        "Generating embeddings for topics without embeddings"
    );

    // Generate embeddings using the main embed_texts function
    let topic_texts: Vec<String> = topics_without_embeddings
        .iter()
        .map(|(_, t)| t.clone())
        .collect();

    let embeddings = crate::embed_texts(&topic_texts).await?;

    // Store embeddings
    let mut updated = 0;
    for ((id, topic), embedding) in topics_without_embeddings.iter().zip(embeddings.iter()) {
        let embedding_blob = embedding_to_blob(embedding);

        let conn_guard = conn.lock();

        // Update active_topics
        if conn_guard
            .execute(
                "UPDATE active_topics SET embedding = ?1 WHERE id = ?2",
                rusqlite::params![embedding_blob, id],
            )
            .is_ok()
        {
            // Insert into vec0 index
            if let Err(e) = conn_guard.execute(
                "INSERT OR REPLACE INTO topic_vec (rowid, embedding) VALUES (?1, ?2)",
                rusqlite::params![id, embedding_blob],
            ) {
                tracing::warn!(target: "4da::ace", error = %e, topic = %topic, topic_id = id, "Failed to insert topic embedding into vec0 index");
            }
            updated += 1;
            debug!(target: "ace::embedding", topic = %topic, "Generated embedding for topic");
        }
    }

    info!(target: "ace::embedding", updated = updated, "Generated topic embeddings");
    Ok(updated)
}

/// KNN search for topics similar to a given embedding
/// Returns (topic, similarity_score) pairs sorted by similarity
#[allow(dead_code)] // Reason: semantic topic KNN search not yet wired into scoring pipeline
pub fn find_similar_topics_knn(
    conn: &Arc<Mutex<Connection>>,
    query_embedding: &[f32],
    limit: usize,
) -> Result<Vec<(String, f32)>> {
    let conn = conn.lock();
    let embedding_blob = embedding_to_blob(query_embedding);

    let mut stmt = conn.prepare(
        "SELECT at.topic, tv.distance
             FROM topic_vec tv
             JOIN active_topics at ON at.id = tv.rowid
             WHERE tv.embedding MATCH ?1
             AND k = ?2
             ORDER BY tv.distance",
    )?;

    let rows = stmt.query_map(rusqlite::params![embedding_blob, limit as i32], |row| {
        let topic: String = row.get(0)?;
        let distance: f32 = row.get(1)?;
        // Convert L2 distance to similarity (1 / (1 + distance))
        let similarity = 1.0 / (1.0 + distance);
        Ok((topic, similarity))
    })?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ============================================================================
// ACE Embedding Methods
// ============================================================================

impl ACE {
    /// Generate embedding for a topic
    #[allow(dead_code)] // Reason: ACE embedding API, not yet called from scoring pipeline
    pub fn embed_topic(&self, topic: &str) -> Result<Vec<f32>> {
        match &self.embedding_service {
            Some(service) => service.lock().embed(topic),
            None => Err("Embedding service not initialized".into()),
        }
    }

    /// Find similar topics
    pub fn find_similar_topics(&self, query: &str, top_k: usize) -> Result<Vec<(String, f32)>> {
        let topics = self.get_active_topics()?;
        let topic_strings: Vec<String> = topics.iter().map(|t| t.topic.clone()).collect();

        match &self.embedding_service {
            Some(service) => service.lock().find_similar(query, &topic_strings, top_k),
            None => Err("Embedding service not initialized".into()),
        }
    }

    /// Access the embedding service (for maintenance operations like cache pruning).
    pub fn embedding_service(&self) -> Option<&Mutex<EmbeddingService>> {
        self.embedding_service.as_ref()
    }

    /// Check if embedding service is operational
    pub fn is_embedding_operational(&self) -> bool {
        self.embedding_service
            .as_ref()
            .is_some_and(|s| s.lock().is_operational())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_blob_roundtrip() {
        let original = vec![1.0_f32, 2.5, -3.7, 0.0, 42.0];
        let blob = embedding_to_blob(&original);
        let recovered = blob_to_embedding(&blob);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_empty_embedding_roundtrip() {
        let original: Vec<f32> = vec![];
        let blob = embedding_to_blob(&original);
        let recovered = blob_to_embedding(&blob);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_single_value_roundtrip() {
        let original = vec![42.0_f32];
        let blob = embedding_to_blob(&original);
        assert_eq!(blob.len(), 4);
        let recovered = blob_to_embedding(&blob);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_blob_preserves_precision() {
        let original = vec![
            std::f32::consts::PI,
            std::f32::consts::E,
            std::f32::consts::SQRT_2,
            std::f32::consts::LN_2,
            f32::MIN_POSITIVE,
            f32::MAX,
            f32::MIN,
        ];
        let blob = embedding_to_blob(&original);
        let recovered = blob_to_embedding(&blob);
        assert_eq!(original.len(), recovered.len());
        for (a, b) in original.iter().zip(recovered.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "Precision lost for value {a}");
        }
    }
}
