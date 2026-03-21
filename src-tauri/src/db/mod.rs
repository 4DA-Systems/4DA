//! Database module for 4DA - Persistence layer for embeddings and sources
//!
//! Uses sqlite-vec for vector similarity search at scale.
//! Designed to handle hundreds of thousands of sources.

mod cache;
mod channels;
#[cfg(test)]
mod concurrency_tests;
mod dependencies;
mod history;
mod migrations;
mod sources;
#[cfg(test)]
mod stress_tests;

pub use cache::*;
pub use dependencies::*;
pub use history::*;
pub use sources::*;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection, Result as SqliteResult};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

// ============================================================================
// Types
// ============================================================================

/// A stored context chunk with its embedding
#[derive(Debug, Clone)]
pub struct StoredContext {
    pub id: i64,
    pub source_file: String,
    pub content_hash: String,
    pub text: String,
    pub embedding: Vec<f32>,
    pub created_at: DateTime<Utc>,
}

/// A stored source item (HN story, arXiv paper, RSS item, etc.)
#[derive(Debug, Clone)]
pub struct StoredSourceItem {
    pub id: i64,
    pub source_type: String,
    pub source_id: String,
    pub url: Option<String>,
    pub title: String,
    pub content: String,
    pub content_hash: String,
    pub embedding: Vec<f32>,
    pub created_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

/// Similarity result from vector search
#[derive(Debug, Clone)]
pub struct SimilarityResult {
    pub context_id: i64,
    pub source_file: String,
    pub text: String,
    pub distance: f32,
}

/// Aggregate scoring statistics (rejection rate measurement)
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScoringStatsAggregate {
    pub total_runs: i64,
    pub total_scored: i64,
    pub total_relevant: i64,
    pub lifetime_rejection_rate: f64,
    pub last_run_rejection_rate: Option<f64>,
}

// ============================================================================
// Database Manager
// ============================================================================

pub struct Database {
    pub(crate) conn: Arc<Mutex<Connection>>,
    pub(crate) db_path: PathBuf,
}

impl Database {
    /// Initialize database with sqlite-vec extension
    pub fn new(db_path: &Path) -> SqliteResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        crate::register_sqlite_vec_extension();

        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA cache_size = -64000;
            PRAGMA mmap_size = 268435456;
            PRAGMA temp_store = MEMORY;
            PRAGMA busy_timeout = 5000;
        ",
        )?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: db_path.to_path_buf(),
        };

        db.migrate()?;

        Ok(db)
    }

    /// Checkpoint the WAL file if it exceeds the size threshold.
    /// Returns the number of WAL pages moved to the main database.
    pub fn checkpoint_wal_if_needed(&self) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        // Check WAL size via PRAGMA wal_checkpoint(PASSIVE) first
        // PASSIVE won't block writers
        let mut pages_moved: i32 = 0;
        conn.query_row(
            "PRAGMA wal_checkpoint(PASSIVE)",
            [],
            |row| {
                pages_moved = row.get::<_, i32>(1).unwrap_or(0);
                Ok(())
            },
        )?;
        Ok(pages_moved as usize)
    }

    /// Run lightweight scheduled maintenance (safe to call frequently).
    /// - WAL checkpoint (PASSIVE — non-blocking)
    /// - PRAGMA optimize (SQLite auto-tune)
    /// Does NOT VACUUM (too heavy for frequent runs).
    pub fn run_scheduled_maintenance(&self) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);")?;
        conn.execute_batch("PRAGMA optimize;")?;
        tracing::info!(target: "4da::db", "Scheduled maintenance: WAL checkpoint + optimize complete");
        Ok(())
    }

    // ========================================================================
    // Context Operations
    // ========================================================================

    /// Store a context chunk with its embedding (also updates vec0 index)
    pub fn upsert_context(
        &self,
        source_file: &str,
        text: &str,
        embedding: &[f32],
    ) -> SqliteResult<i64> {
        self.upsert_context_weighted(source_file, text, embedding, 1.0)
    }

    /// Upsert context with weight for section-aware indexing
    pub fn upsert_context_weighted(
        &self,
        source_file: &str,
        text: &str,
        embedding: &[f32],
        weight: f32,
    ) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        let content_hash = hash_content(text);
        let embedding_blob = embedding_to_blob(embedding);

        let existing_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM context_chunks WHERE content_hash = ?1",
                params![content_hash],
                |row| row.get(0),
            )
            .ok();

        let tx = conn.unchecked_transaction()?;
        if let Some(id) = existing_id {
            tx.execute(
                "UPDATE context_chunks SET source_file = ?1, weight = ?2, updated_at = datetime('now') WHERE id = ?3",
                params![source_file, weight, id],
            )?;
            tx.execute(
                "UPDATE context_vec SET embedding = ?1 WHERE rowid = ?2",
                params![embedding_blob, id],
            )?;
            tx.commit()?;
            Ok(id)
        } else {
            tx.execute(
                "INSERT INTO context_chunks (source_file, content_hash, text, embedding, weight, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
                params![source_file, content_hash, text, embedding_blob, weight],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO context_vec (rowid, embedding) VALUES (?1, ?2)",
                params![id, embedding_blob],
            )?;
            tx.commit()?;
            Ok(id)
        }
    }

    /// Get all context embeddings
    pub fn get_all_contexts(&self) -> SqliteResult<Vec<StoredContext>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_file, content_hash, text, embedding, created_at
             FROM context_chunks",
        )?;

        let rows = stmt.query_map([], |row| {
            let embedding_blob: Vec<u8> = row.get(4)?;
            Ok(StoredContext {
                id: row.get(0)?,
                source_file: row.get(1)?,
                content_hash: row.get(2)?,
                text: row.get(3)?,
                embedding: blob_to_embedding(&embedding_blob),
                created_at: parse_datetime(row.get::<_, String>(5)?),
            })
        })?;

        rows.collect()
    }

    /// Clear all context chunks (for re-indexing)
    pub fn clear_contexts(&self) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM context_vec", [])?;
        conn.execute("DELETE FROM context_chunks", [])
    }

    /// Get context count
    pub fn context_count(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM context_chunks", [], |row| row.get(0))
    }

    /// KNN search for similar contexts using sqlite-vec (O(log n) instead of O(n))
    pub fn find_similar_contexts(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> SqliteResult<Vec<SimilarityResult>> {
        let conn = self.conn.lock();
        let embedding_blob = embedding_to_blob(query_embedding);

        let mut stmt = conn.prepare(
            "SELECT v.rowid, v.distance, c.source_file, c.text
             FROM context_vec v
             JOIN context_chunks c ON c.id = v.rowid
             WHERE v.embedding MATCH ?1 AND k = ?2
             ORDER BY v.distance",
        )?;

        let rows = stmt.query_map(params![embedding_blob, limit as i64], |row| {
            Ok(SimilarityResult {
                context_id: row.get(0)?,
                distance: row.get(1)?,
                source_file: row.get(2)?,
                text: row.get(3)?,
            })
        })?;

        rows.collect()
    }

    /// KNN search for similar source items using sqlite-vec (O(log n) instead of O(n))
    pub fn find_similar_source_items(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.conn.lock();
        let embedding_blob = embedding_to_blob(query_embedding);

        let mut stmt = conn.prepare(
            "SELECT s.id, s.source_type, s.source_id, s.url, s.title, s.content,
                    s.content_hash, s.embedding, s.created_at, s.last_seen, v.distance
             FROM source_vec v
             JOIN source_items s ON s.id = v.rowid
             WHERE v.embedding MATCH ?1 AND k = ?2
             ORDER BY v.distance",
        )?;

        let rows = stmt.query_map(params![embedding_blob, limit as i64], |row| {
            let embedding_blob: Vec<u8> = row.get(7)?;
            Ok(StoredSourceItem {
                id: row.get(0)?,
                source_type: row.get(1)?,
                source_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get(4)?,
                content: row.get(5)?,
                content_hash: row.get(6)?,
                embedding: blob_to_embedding(&embedding_blob),
                created_at: parse_datetime(row.get::<_, String>(8)?),
                last_seen: parse_datetime(row.get::<_, String>(9)?),
            })
        })?;

        rows.collect()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse datetime string to chrono DateTime
pub(crate) fn parse_datetime(s: String) -> chrono::DateTime<chrono::Utc> {
    use chrono::{NaiveDateTime, TimeZone, Utc};
    NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
        .map(|dt| Utc.from_utc_datetime(&dt))
        .unwrap_or_else(|_| Utc::now())
}

/// Hash content for deduplication
pub(crate) fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// Hash multiple content parts for deduplication without intermediate allocation.
pub(crate) fn hash_content_parts(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Convert f32 embedding to blob for storage
pub(crate) fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert blob back to f32 embedding
pub(crate) fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().unwrap_or([0u8; 4]);
            f32::from_le_bytes(arr)
        })
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_content() {
        let hash1 = hash_content("hello world");
        let hash2 = hash_content("hello world");
        let hash3 = hash_content("different content");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_embedding_roundtrip() {
        let embedding = vec![0.1, 0.2, 0.3, -0.5, 1.0];
        let blob = embedding_to_blob(&embedding);
        let recovered = blob_to_embedding(&blob);

        assert_eq!(embedding.len(), recovered.len());
        for (a, b) in embedding.iter().zip(recovered.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }
}
