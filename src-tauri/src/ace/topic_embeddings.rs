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

/// Backfill the `topic_vec` vec0 KNN index from embeddings already persisted
/// on `active_topics`.
///
/// `store_topic_embedding` only writes a vec0 row for NEWLY generated
/// embeddings - topics whose embedding was loaded from the DB cache never get
/// a topic_vec row, so the index stays permanently behind the source table
/// and semantic topic search/dedup misses most topics. This sync closes that
/// gap: it finds active_topics rows with a persisted embedding but no
/// topic_vec row and inserts them, bounded by `limit` per call.
///
/// Zero-vector and wrong-dimension blobs are skipped - a zero vector in a
/// KNN index pollutes every query.
pub fn sync_topic_vec(conn: &Arc<Mutex<Connection>>, limit: usize) -> Result<usize> {
    let conn = conn.lock();

    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.embedding FROM active_topics t
             WHERE t.embedding IS NOT NULL
               AND NOT EXISTS (SELECT 1 FROM topic_vec v WHERE v.rowid = t.id)
             LIMIT ?1",
        )
        .context("Failed to prepare topic_vec backfill query")?;

    let rows: Vec<(i64, Vec<u8>)> = stmt
        .query_map(rusqlite::params![limit as i64], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .context("Failed to query topics missing from topic_vec")?
        .flatten()
        .collect();

    let expected_dim = crate::EMBEDDING_DIMS;
    let mut inserted = 0usize;
    for (id, blob) in rows {
        let embedding = blob_to_embedding(&blob);
        if embedding.len() != expected_dim || embedding.iter().all(|&v| v == 0.0) {
            continue; // wrong-dimension or zero-vector blob - never index these
        }
        if conn
            .execute(
                "INSERT OR REPLACE INTO topic_vec (rowid, embedding) VALUES (?1, ?2)",
                rusqlite::params![id, blob],
            )
            .is_ok()
        {
            inserted += 1;
        }
    }

    if inserted > 0 {
        debug!(
            target: "ace::embedding",
            inserted,
            "Backfilled topic_vec index from persisted topic embeddings"
        );
    }
    Ok(inserted)
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

    /// Populate the `topic_vec` KNN index from already-embedded active topics.
    ///
    /// Skips entirely when no embedding service is configured - cold machines
    /// without an embedder pay nothing (their topics carry no real embeddings
    /// to index anyway). Bounded by `limit` per call and non-fatal: callers
    /// log errors and move on.
    pub fn populate_topic_vec(&self, limit: usize) -> Result<usize> {
        if self.embedding_service.is_none() {
            return Ok(0);
        }
        sync_topic_vec(&self.conn, limit)
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

    // ========================================================================
    // topic_vec population
    // ========================================================================

    fn insert_topic(ace: &super::super::ACE, topic: &str, embedding: Option<&[f32]>) -> i64 {
        let conn = ace.get_conn().lock();
        let blob = embedding.map(embedding_to_blob);
        conn.execute(
            "INSERT INTO active_topics (topic, weight, confidence, embedding, source)
             VALUES (?1, 0.8, 0.9, ?2, 'manifest')",
            rusqlite::params![topic, blob],
        )
        .expect("insert topic");
        conn.last_insert_rowid()
    }

    fn topic_vec_has_row(ace: &super::super::ACE, rowid: i64) -> bool {
        let conn = ace.get_conn().lock();
        conn.query_row(
            "SELECT COUNT(*) FROM topic_vec WHERE rowid = ?1",
            rusqlite::params![rowid],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
            > 0
    }

    #[test]
    fn test_populate_topic_vec_skips_when_no_embedder() {
        // create_test_ace has embedding_service: None - the skip path.
        let ace = super::super::create_test_ace();
        let real: Vec<f32> = (0..crate::EMBEDDING_DIMS)
            .map(|i| (i as f32) * 0.001)
            .collect();
        let id = insert_topic(&ace, "rust", Some(&real));

        let n = ace
            .populate_topic_vec(100)
            .expect("populate must not error on the skip path");
        assert_eq!(
            n, 0,
            "no embedder -> skip entirely, cold machines pay nothing"
        );
        assert!(
            !topic_vec_has_row(&ace, id),
            "skip path must not touch topic_vec"
        );
    }

    #[test]
    fn test_sync_topic_vec_backfills_valid_embeddings_only() {
        let ace = super::super::create_test_ace();
        let real: Vec<f32> = (0..crate::EMBEDDING_DIMS)
            .map(|i| (i as f32) * 0.001 + 0.1)
            .collect();
        let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];
        let wrong_dim = vec![0.5_f32; 8];

        let id_real = insert_topic(&ace, "tauri", Some(&real));
        let id_zero = insert_topic(&ace, "zerotopic", Some(&zero));
        let id_wrong = insert_topic(&ace, "wrongdim", Some(&wrong_dim));
        let id_none = insert_topic(&ace, "noembedding", None);

        let n = sync_topic_vec(ace.get_conn(), 100).expect("sync must succeed");
        assert_eq!(n, 1, "only the real embedding is indexable");
        assert!(topic_vec_has_row(&ace, id_real), "real embedding indexed");
        assert!(!topic_vec_has_row(&ace, id_zero), "zero vector skipped");
        assert!(
            !topic_vec_has_row(&ace, id_wrong),
            "wrong dimension skipped"
        );
        assert!(!topic_vec_has_row(&ace, id_none), "NULL embedding skipped");
    }

    #[test]
    fn test_sync_topic_vec_idempotent_and_bounded() {
        let ace = super::super::create_test_ace();
        let mut ids = Vec::new();
        for i in 0..5 {
            let emb: Vec<f32> = (0..crate::EMBEDDING_DIMS)
                .map(|j| ((i * 31 + j) as f32) * 0.001 + 0.05)
                .collect();
            ids.push(insert_topic(&ace, &format!("topic-{i}"), Some(&emb)));
        }

        // Bounded: limit 2 -> only 2 synced per call.
        let first = sync_topic_vec(ace.get_conn(), 2).expect("bounded sync");
        assert_eq!(first, 2, "limit must bound the batch");

        // Next call picks up the remainder; a further call is a no-op.
        let second = sync_topic_vec(ace.get_conn(), 100).expect("remainder sync");
        assert_eq!(second, 3, "remaining topics synced");
        let third = sync_topic_vec(ace.get_conn(), 100).expect("idempotent sync");
        assert_eq!(third, 0, "already-synced topics are not re-inserted");

        for id in ids {
            assert!(topic_vec_has_row(&ace, id));
        }
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
