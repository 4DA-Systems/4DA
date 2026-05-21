// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Topic embedding storage and retrieval via sqlite-vec.

use parking_lot::Mutex;
use rusqlite::Connection;
use std::sync::Arc;
use tracing::debug;

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

    let expected_dim = crate::EMBEDDING_DIMS;
    for (topic, blob) in rows.flatten() {
        let embedding = blob_to_embedding(&blob);
        if embedding.len() == expected_dim {
            result.insert(topic, embedding);
        }
    }

    debug!(
        target: "ace::embedding",
        count = result.len(),
        "Loaded topic embeddings from database"
    );

    Ok(result)
}

// ============================================================================
// ACE Embedding Methods
// ============================================================================

impl ACE {
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
