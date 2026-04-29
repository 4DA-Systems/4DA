// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Database module for 4DA - Persistence layer for embeddings and sources
//!
//! Uses sqlite-vec for vector similarity search at scale.
//! Designed to handle hundreds of thousands of sources.

mod cache;
mod channels;
#[cfg(test)]
mod concurrency_tests;
mod dependencies;
pub(crate) mod encryption;
mod history;
pub(crate) mod migrations;
mod llm_judgments;
mod osv_advisories;
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
    /// BCP-47 language code detected from title text (e.g. "en", "ja", "de").
    /// Defaults to "en" for items ingested before language detection was added.
    pub detected_lang: String,
    /// Canonical feed URL that produced this item (e.g. RSS feed URL).
    /// Used for per-feed health tracking in custom sources.
    pub feed_origin: Option<String>,
    /// JSON-serialized structured tags from source metadata (SO tags, GitHub topics, etc.).
    /// Parsed at scoring time for source-fair topic extraction.
    pub tags: Option<String>,
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
    /// Pool of read-only connections for parallel query execution.
    /// These bypass the writer lock, allowing concurrent reads during writes.
    read_pool: Vec<Mutex<Connection>>,
}

/// Number of read-only connections in the pool.
/// SQLite WAL mode allows concurrent readers, so this gives us parallelism.
const READ_POOL_SIZE: usize = 3;

impl Database {
    /// Initialize database with sqlite-vec extension
    pub fn new(db_path: &Path) -> SqliteResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        crate::register_sqlite_vec_extension();

        let conn = Connection::open(db_path)?;

        // Apply database encryption key if available.
        // When SQLCipher is enabled, this must be the first statement after open.
        let db_key = encryption::get_or_create_db_key();
        if let Err(e) = encryption::apply_key_to_connection(&conn, db_key.as_deref()) {
            tracing::warn!(target: "4da::db", error = %e, "Failed to apply encryption key — continuing unencrypted");
        }

        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA wal_autocheckpoint = 1000;
            PRAGMA cache_size = -64000;
            PRAGMA mmap_size = 268435456;
            PRAGMA temp_store = MEMORY;
            PRAGMA busy_timeout = 5000;
        ",
        )?;

        // TRUNCATE checkpoint BEFORE opening read connections.
        // PASSIVE can't move pages while readers hold snapshots, so a stale
        // WAL grows unbounded. TRUNCATE resets it while we're the only connection.
        if db_path.to_string_lossy() != ":memory:" {
            let wal_path = db_path.with_extension("db-wal");
            let wal_large = std::fs::metadata(&wal_path)
                .map(|m| m.len() > 50 * 1024 * 1024)
                .unwrap_or(false);
            if wal_large {
                let wal_mb = std::fs::metadata(&wal_path)
                    .map(|m| m.len() / (1024 * 1024))
                    .unwrap_or(0);
                tracing::info!(target: "4da::db", wal_mb, "Large WAL — TRUNCATE checkpoint before read pool");
                if let Err(e) = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);") {
                    tracing::warn!(target: "4da::db", error = %e, "TRUNCATE checkpoint failed");
                }
            }
        }

        // Create read-only connection pool for parallel queries.
        // WAL mode allows multiple concurrent readers alongside one writer.
        // Skip pool for in-memory databases (tests) — they can't share connections.
        let is_file_db = db_path.to_string_lossy() != ":memory:";
        let mut read_pool = Vec::with_capacity(if is_file_db { READ_POOL_SIZE } else { 0 });
        if is_file_db {
            for i in 0..READ_POOL_SIZE {
                match Connection::open_with_flags(
                    db_path,
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                        | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX
                        | rusqlite::OpenFlags::SQLITE_OPEN_URI,
                ) {
                    Ok(reader) => {
                        // Apply encryption key to read connections too
                        if let Err(e) =
                            encryption::apply_key_to_connection(&reader, db_key.as_deref())
                        {
                            tracing::warn!(target: "4da::db", pool = i, error = %e, "Failed to apply encryption key to reader");
                        }
                        reader
                            .execute_batch(
                                "PRAGMA busy_timeout = 5000;
                                 PRAGMA cache_size = -16000;
                                 PRAGMA mmap_size = 134217728;
                                 PRAGMA query_only = ON;",
                            )
                            .ok();
                        read_pool.push(Mutex::new(reader));
                    }
                    Err(e) => {
                        tracing::warn!(target: "4da::db", index = i, error = %e, "Failed to create read pool connection");
                    }
                }
            }
            tracing::info!(target: "4da::db", pool_size = read_pool.len(), "Read connection pool initialized");
        }

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: db_path.to_path_buf(),
            read_pool,
        };

        db.migrate()?;

        // Quick integrity check — detect corruption early before it compounds.
        // Uses quick_check (faster than integrity_check, catches most issues).
        {
            let conn = db.conn.lock();
            match conn.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0)) {
                Ok(ref status) if status == "ok" => {
                    tracing::debug!(target: "4da::db", "Database integrity: ok");
                }
                Ok(status) => {
                    tracing::error!(
                        target: "4da::db",
                        status = %status,
                        "DATABASE CORRUPTION DETECTED — quick_check failed. Consider restoring from backup."
                    );
                }
                Err(e) => {
                    tracing::warn!(target: "4da::db", error = %e, "Could not run integrity check");
                }
            }
        }

        // Restrict database file permissions on Unix (contains user data)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(db_path, std::fs::Permissions::from_mode(0o600));
        }

        Ok(db)
    }

    /// Borrow a read-only connection from the pool for parallel query execution.
    /// Falls back to the writer connection if the pool is exhausted.
    /// Use this for SELECT queries that don't need write access.
    pub fn read_conn(&self) -> parking_lot::MutexGuard<'_, Connection> {
        // Try each pool connection with try_lock (non-blocking)
        for reader in &self.read_pool {
            if let Some(guard) = reader.try_lock() {
                return guard;
            }
        }
        // All readers busy — fall back to writer (contention, but correct)
        self.conn.lock()
    }

    /// Checkpoint the WAL file if it exceeds the size threshold.
    /// Returns the number of WAL pages moved to the main database.
    pub fn checkpoint_wal_if_needed(&self) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        // Check WAL size via PRAGMA wal_checkpoint(PASSIVE) first
        // PASSIVE won't block writers
        let mut pages_moved: i32 = 0;
        conn.query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |row| {
            pages_moved = row.get::<_, i32>(1).unwrap_or(0);
            Ok(())
        })?;
        Ok(pages_moved as usize)
    }

    /// Run lightweight scheduled maintenance (safe to call frequently).
    /// - WAL checkpoint (TRUNCATE if large, else PASSIVE)
    /// - PRAGMA optimize (SQLite auto-tune)
    /// Does NOT VACUUM (too heavy for frequent runs).
    pub fn run_scheduled_maintenance(&self) -> SqliteResult<()> {
        let conn = self.conn.lock();
        let wal_path = self.db_path.with_extension("db-wal");
        let wal_large = std::fs::metadata(&wal_path)
            .map(|m| m.len() > 50 * 1024 * 1024)
            .unwrap_or(false);
        if wal_large {
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        } else {
            conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);")?;
        }
        conn.execute_batch("PRAGMA optimize;")?;
        tracing::info!(target: "4da::db", "Scheduled maintenance: WAL checkpoint + optimize complete");
        Ok(())
    }

    /// Check database file size and warn if approaching limits.
    /// 2GB max prevents unbounded growth from malicious or prolific sources.
    pub fn check_db_size(db_path: &std::path::Path) -> u64 {
        const MAX_DB_SIZE: u64 = 2_147_483_648; // 2 GB
        const WARN_DB_SIZE: u64 = 1_610_612_736; // 1.5 GB

        match std::fs::metadata(db_path) {
            Ok(meta) => {
                let size = meta.len();
                if size > MAX_DB_SIZE {
                    tracing::error!(
                        target: "4da::db",
                        size_mb = size / 1_000_000,
                        "Database exceeds 2GB limit — cleanup recommended"
                    );
                } else if size > WARN_DB_SIZE {
                    tracing::warn!(
                        target: "4da::db",
                        size_mb = size / 1_000_000,
                        "Database approaching 2GB limit"
                    );
                }
                size
            }
            Err(_) => 0,
        }
    }

    /// Log a security-relevant event to the audit table.
    pub fn log_security_event(&self, event_type: &str, details: &str, severity: &str) {
        let conn = self.conn.lock();
        if let Err(e) = conn.execute(
            "INSERT INTO security_audit_log (event_type, details, severity) VALUES (?1, ?2, ?3)",
            rusqlite::params![event_type, details, severity],
        ) {
            tracing::warn!(target: "4da::db", error = %e, event_type, severity, "Failed to write security audit log entry");
        }
    }

    /// Query security audit log entries for compliance review.
    pub fn get_security_audit_log(
        &self,
        limit: i64,
        event_filter: Option<&str>,
    ) -> Vec<(i64, String, String, String, String)> {
        let conn = self.conn.lock();
        let (sql, params): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match event_filter {
            Some(filter) => (
                "SELECT id, timestamp, event_type, COALESCE(details, ''), severity \
                 FROM security_audit_log WHERE event_type = ?1 \
                 ORDER BY timestamp DESC LIMIT ?2",
                vec![Box::new(filter.to_string()), Box::new(limit)],
            ),
            None => (
                "SELECT id, timestamp, event_type, COALESCE(details, ''), severity \
                 FROM security_audit_log ORDER BY timestamp DESC LIMIT ?1",
                vec![Box::new(limit)],
            ),
        };
        conn.prepare(sql)
            .and_then(|mut stmt| {
                let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();
                stmt.query_map(params_refs.as_slice(), |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default()
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
        let conn = self.read_conn();
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
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM context_vec", [])?;
        let count = tx.execute("DELETE FROM context_chunks", [])?;
        tx.commit()?;
        Ok(count)
    }

    /// Get context count
    pub fn context_count(&self) -> SqliteResult<i64> {
        let conn = self.read_conn();
        conn.query_row("SELECT COUNT(*) FROM context_chunks", [], |row| row.get(0))
    }

    /// KNN search for similar contexts using sqlite-vec (O(log n) instead of O(n))
    pub fn find_similar_contexts(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> SqliteResult<Vec<SimilarityResult>> {
        let conn = self.read_conn();
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
        let conn = self.read_conn();
        let embedding_blob = embedding_to_blob(query_embedding);

        let mut stmt = conn.prepare(
            "SELECT s.id, s.source_type, s.source_id, s.url, s.title, s.content,
                    s.content_hash, s.embedding, s.created_at, s.last_seen, v.distance,
                    COALESCE(s.detected_lang, 'en'), s.feed_origin, s.tags
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
                detected_lang: row
                    .get::<_, String>(11)
                    .unwrap_or_else(|_| "en".to_string()),
                feed_origin: row.get(12).ok().flatten(),
                tags: row.get(13).ok().flatten(),
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
        .unwrap_or_else(|_| {
            tracing::warn!("Failed to parse datetime '{}', falling back to now", s);
            Utc::now()
        })
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

/// Expected embedding dimension (MiniLM / text-embedding-3-small)
pub(crate) const EMBEDDING_DIM: usize = 384;

/// Convert f32 embedding to blob for storage.
/// Validates dimension before conversion — rejects wrong-sized vectors.
pub(crate) fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    if !embedding.is_empty() && embedding.len() != EMBEDDING_DIM {
        tracing::error!(
            target: "4da::db",
            "Embedding dimension mismatch: expected {} but got {} — storing anyway",
            EMBEDDING_DIM, embedding.len()
        );
    }
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert blob back to f32 embedding.
/// Returns empty vec on invalid blobs instead of panicking.
pub(crate) fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    if blob.is_empty() {
        return Vec::new();
    }
    if blob.len() % 4 != 0 {
        tracing::warn!(
            target: "4da::db",
            "Invalid embedding blob size: {} bytes (not divisible by 4) — returning empty",
            blob.len()
        );
        return Vec::new();
    }
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
