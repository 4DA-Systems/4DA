// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Database module for 4DA - Persistence layer for embeddings and sources
//!
//! Uses sqlite-vec for vector similarity search at scale.
//! Designed to handle hundreds of thousands of sources.

mod brief_rejections;
mod cache;
mod channels;
#[cfg(test)]
mod concurrency_tests;
#[cfg(test)]
mod context_rebuild_tests;
pub(crate) mod dep_snapshots;
mod dependencies;
pub(crate) mod encryption;
#[cfg(test)]
mod fts_sync_tests;
mod helpers;
mod history;
pub(crate) mod hybrid_search;
pub(crate) mod llm_judgments;
pub(crate) mod migrations;
mod osv_advisories;
mod scoring_queries;
pub mod source_item_deps;
mod sources;
#[cfg(test)]
mod stress_tests;
mod verdicts;

pub use cache::*;
pub use dep_snapshots::*;
pub use dependencies::*;
// Flat re-export so every existing `crate::db::parse_datetime` /
// `super::blob_to_embedding` path keeps working after the helpers extraction.
pub(crate) use helpers::*;
pub use history::*;
pub use scoring_queries::*;
pub use sources::*;
pub use verdicts::*;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection, Result as SqliteResult};
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
    /// Publication date from the source adapter (RSS pubDate, npm time, OSV
    /// published, ...). None for items whose adapter carried no parseable date
    /// or that predate the column — readers fall back to created_at
    /// (first-seen). This is the honest freshness axis: last_seen refreshes on
    /// every re-fetch, created_at is when 4DA first saw it, published_at is
    /// when the content actually appeared.
    pub published_at: Option<DateTime<Utc>>,
}

/// Similarity result from vector search
#[derive(Debug, Clone)]
pub struct SimilarityResult {
    pub context_id: i64,
    pub source_file: String,
    pub text: String,
    pub distance: f32,
}

/// Outcome of [`Database::reconcile_context_provenance`] — how many chunks were
/// scanned, re-tagged, and pruned when healing the context corpus.
#[derive(Debug, Default, Clone)]
pub struct ContextReconcileStats {
    pub scanned: usize,
    pub reclassified: usize,
    pub pruned_reject: usize,
    pub pruned_over_cap: usize,
}

impl ContextReconcileStats {
    /// Total chunks removed from the corpus (reject + over-cap).
    pub fn total_pruned(&self) -> usize {
        self.pruned_reject + self.pruned_over_cap
    }
}

/// A fully-prepared chunk (text + embedding already computed) for
/// [`Database::rebuild_contexts`].
#[derive(Debug, Clone)]
pub struct NewContextChunk {
    pub source_file: String,
    pub text: String,
    pub embedding: Vec<f32>,
    pub weight: f32,
}

/// Outcome of [`Database::rebuild_contexts`]. `refused` set means the corpus
/// was left UNTOUCHED (the replacement set was unusable — committing it would
/// have amounted to a wipe).
#[derive(Debug, Default, Clone)]
pub struct ContextRebuildStats {
    pub previous_count: usize,
    pub attempted: usize,
    pub admitted: usize,
    pub skipped_reject: usize,
    pub skipped_doc_cap: usize,
    pub deduped: usize,
    pub refused: Option<&'static str>,
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

/// Read-only pool size (WAL allows concurrent readers) — also the parallel-drain
/// thread ceiling: each scoring thread borrows one reader for its per-item KNN
/// (`analysis_backfill::score_chunk`), so more threads would serialize on the writer.
pub(crate) const READ_POOL_SIZE: usize = 3;

/// WAL size above which a checkpoint is escalated from PASSIVE to TRUNCATE.
///
/// PASSIVE cannot reset the WAL while any reader holds a snapshot — with a read pool of
/// three plus a second process, the file only ever grows to its high-water mark. TRUNCATE
/// does reset it, but takes a brief exclusive lock (bounded by `busy_timeout`), so it is
/// gated on size rather than run every time.
///
/// This gate was 50 MB, which the engine could not reach in practice: `wal_autocheckpoint
/// = 1000` at a 4 KiB page size keeps the file churning around 4 MB, so a WAL that was
/// never getting truncated sat between the two numbers indefinitely — 25.9 MB when the
/// defect was reported, 47.7 MB a day later. 16 MB is 4x the autocheckpoint threshold:
/// high enough that a healthy WAL never trips it, low enough that a real backlog does.
pub(crate) const WAL_TRUNCATE_THRESHOLD_BYTES: u64 = 16 * 1024 * 1024;

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
                .map(|m| m.len() > WAL_TRUNCATE_THRESHOLD_BYTES)
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

    /// Run lightweight scheduled maintenance (safe to call frequently).
    /// - WAL checkpoint (TRUNCATE if large, else PASSIVE)
    /// - PRAGMA optimize (SQLite auto-tune)
    ///
    /// Does NOT VACUUM (too heavy for frequent runs).
    ///
    /// Called from the GUI monitoring loop hourly **and** from the end of every headless
    /// engine cycle. The second caller is the point: for 24 hours `fourda-engine` wrote
    /// continuously while the GUI was closed, so nothing here ever ran and the WAL grew
    /// unbounded. Maintenance must belong to whoever is doing the writing, not to whoever
    /// happens to have a window open.
    pub fn run_scheduled_maintenance(&self) -> SqliteResult<()> {
        let conn = self.conn.lock();
        let wal_path = self.db_path.with_extension("db-wal");
        let wal_bytes = std::fs::metadata(&wal_path).map(|m| m.len()).unwrap_or(0);
        if wal_bytes > WAL_TRUNCATE_THRESHOLD_BYTES {
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        } else {
            conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);")?;
        }
        conn.execute_batch("PRAGMA optimize;")?;
        tracing::info!(
            target: "4da::db",
            wal_mb = wal_bytes / (1024 * 1024),
            truncated = wal_bytes > WAL_TRUNCATE_THRESHOLD_BYTES,
            "Scheduled maintenance: WAL checkpoint + optimize complete"
        );
        Ok(())
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

    /// Upsert context with weight for section-aware indexing.
    ///
    /// This is the single chokepoint every context writer funnels through, so
    /// the content-admission policy ([`crate::context_admission`]) is enforced
    /// HERE: reject non-context provenance, tag `source_type`, apply the
    /// class weight multiplier, and cap how much any one doc source may
    /// contribute. No indexer — present or future — can bypass it. Returns `0`
    /// (never a valid rowid) when the chunk is not admitted.
    pub fn upsert_context_weighted(
        &self,
        source_file: &str,
        text: &str,
        embedding: &[f32],
        weight: f32,
    ) -> SqliteResult<i64> {
        use crate::context_admission::{
            classify_source_with_content, ContextClass, MAX_DOC_CHUNKS_PER_SOURCE,
        };

        // Content-aware: a #[cfg(test)] region of a prod file is TestCode even
        // though its path says Code — test fixtures must never ground the feed.
        let class = classify_source_with_content(source_file, text);
        if !class.is_admitted() {
            crate::context_admission::log_admission_skip(source_file, "rejected-provenance");
            return Ok(0);
        }
        let source_type = class.source_type();
        let effective_weight = weight * class.weight_multiplier();

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

        // Per-source proportionality cap (content-agnostic): a single doc file
        // may contribute at most MAX_DOC_CHUNKS_PER_SOURCE chunks. Only checked
        // for NEW doc chunks — an update re-weights an existing row. Code/config
        // are exempt (many files legitimately share a basename; aggregate
        // dominance is caught by the corpus-health check instead).
        if existing_id.is_none() && class == ContextClass::Doc {
            let existing_for_source: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM context_chunks WHERE source_file = ?1 AND source_type = 'doc'",
                    params![source_file],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            if existing_for_source as usize >= MAX_DOC_CHUNKS_PER_SOURCE {
                crate::context_admission::log_admission_skip(source_file, "doc-source-cap");
                return Ok(0);
            }
        }

        let tx = conn.unchecked_transaction()?;
        if let Some(id) = existing_id {
            tx.execute(
                "UPDATE context_chunks SET source_file = ?1, weight = ?2, source_type = ?3, updated_at = datetime('now') WHERE id = ?4",
                params![source_file, effective_weight, source_type, id],
            )?;
            tx.execute(
                "UPDATE context_vec SET embedding = ?1 WHERE rowid = ?2",
                params![embedding_blob, id],
            )?;
            tx.commit()?;
            Ok(id)
        } else {
            tx.execute(
                "INSERT INTO context_chunks (source_file, content_hash, text, embedding, weight, source_type, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
                params![source_file, content_hash, text, embedding_blob, effective_weight, source_type],
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

    /// Clear all context chunks. Reserved for the EXPLICIT user action
    /// (`clear_context` command). Indexing paths must never call this — use
    /// [`Self::rebuild_contexts`], which replaces the corpus atomically.
    /// (A startup path that cleared first and re-embedded for ~10 minutes left
    /// the 2026-07-15 boot scoring 701 items against an empty corpus.)
    pub fn clear_contexts(&self) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM context_vec", [])?;
        let count = tx.execute("DELETE FROM context_chunks", [])?;
        tx.commit()?;
        Ok(count)
    }

    /// Atomically replace the grounding corpus with a fully-prepared entry set.
    ///
    /// The whole swap — delete old rows, admit new ones — happens in ONE
    /// transaction, so no reader ever observes an empty or partial corpus and
    /// a crash at any point leaves the previous corpus intact. Callers do all
    /// slow work (file IO, chunking, embedding) BEFORE calling this; the swap
    /// itself is sub-second.
    ///
    /// Every entry passes the same admission policy as
    /// [`Self::upsert_context_weighted`] (classify by provenance, reject class
    /// dropped, docs capped per source, content-hash dedupe within the set),
    /// so this path cannot be used to bypass the chokepoint.
    ///
    /// Refuses (corpus untouched, `refused` set) when the entry set is empty
    /// or when nothing in it is admissible — committing either would be a
    /// wipe wearing a rebuild's clothes.
    pub fn rebuild_contexts(
        &self,
        entries: &[NewContextChunk],
    ) -> SqliteResult<ContextRebuildStats> {
        use crate::context_admission::{
            classify_source_with_content, log_admission_skip, ContextClass,
            MAX_DOC_CHUNKS_PER_SOURCE,
        };
        use std::collections::{HashMap, HashSet};

        let conn = self.conn.lock();
        let previous_count = conn.query_row("SELECT COUNT(*) FROM context_chunks", [], |r| {
            r.get::<_, i64>(0)
        })? as usize;
        let mut stats = ContextRebuildStats {
            previous_count,
            ..Default::default()
        };
        if entries.is_empty() {
            stats.refused = Some("empty-entry-set");
            return Ok(stats);
        }

        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM context_vec", [])?;
        tx.execute("DELETE FROM context_chunks", [])?;
        {
            let mut ins_chunk = tx.prepare(
                "INSERT INTO context_chunks (source_file, content_hash, text, embedding, weight, source_type, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            )?;
            let mut ins_vec =
                tx.prepare("INSERT INTO context_vec (rowid, embedding) VALUES (?1, ?2)")?;
            let mut seen_hashes: HashSet<String> = HashSet::new();
            let mut doc_counts: HashMap<&str, usize> = HashMap::new();

            for e in entries {
                stats.attempted += 1;
                let class = classify_source_with_content(&e.source_file, &e.text);
                if !class.is_admitted() {
                    log_admission_skip(&e.source_file, "rejected-provenance");
                    stats.skipped_reject += 1;
                    continue;
                }
                let content_hash = hash_content(&e.text);
                if !seen_hashes.insert(content_hash.clone()) {
                    stats.deduped += 1;
                    continue;
                }
                if class == ContextClass::Doc {
                    let n = doc_counts.entry(e.source_file.as_str()).or_insert(0);
                    *n += 1;
                    if *n > MAX_DOC_CHUNKS_PER_SOURCE {
                        log_admission_skip(&e.source_file, "doc-source-cap");
                        stats.skipped_doc_cap += 1;
                        continue;
                    }
                }
                let blob = embedding_to_blob(&e.embedding);
                ins_chunk.execute(params![
                    e.source_file,
                    content_hash,
                    e.text,
                    blob,
                    e.weight * class.weight_multiplier(),
                    class.source_type(),
                ])?;
                let id = tx.last_insert_rowid();
                ins_vec.execute(params![id, blob])?;
                stats.admitted += 1;
            }
        }

        if stats.admitted == 0 {
            // Dropping the uncommitted transaction rolls the deletes back —
            // the previous corpus survives.
            stats.refused = Some("zero-admitted");
            return Ok(stats);
        }
        tx.commit()?;
        Ok(stats)
    }

    /// Read a value from the generic `kv_store` table, normalized to a string
    /// REGARDLESS of the storage class SQLite kept it in.
    ///
    /// This normalization is load-bearing: the installed-base `kv_store` was
    /// created by the ACE schema with `value REAL NOT NULL`, so a flag written
    /// as the string '2' is coerced to REAL 2.0 by column affinity. A plain
    /// `row.get::<String>` on that REAL fails (InvalidColumnType), and the old
    /// `.ok()`-swallowed read returned None — making every "one-time"
    /// migration flag unreadable and re-running the corpus-wiping hygiene
    /// rebuild on EVERY boot (observed five consecutive times, 2026-07-14/15).
    /// Integral REALs normalize back to their integer string ("2.0" -> "2")
    /// so version comparisons and `parse::<usize>()` round-trip.
    pub fn get_kv(&self, key: &str) -> SqliteResult<Option<String>> {
        use rusqlite::types::Value;
        use rusqlite::OptionalExtension;
        let conn = self.read_conn();
        let v: Option<Value> = conn
            .query_row(
                "SELECT value FROM kv_store WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(v.map(|v| match v {
            Value::Text(s) => s,
            Value::Integer(i) => i.to_string(),
            #[allow(clippy::cast_possible_truncation)]
            Value::Real(f) if f.fract() == 0.0 && f.abs() < i64::MAX as f64 => {
                (f as i64).to_string()
            }
            Value::Real(f) => f.to_string(),
            Value::Blob(b) => String::from_utf8_lossy(&b).into_owned(),
            Value::Null => String::new(),
        }))
    }

    /// Write a value to the generic `kv_store` table. Goes through the
    /// mutex-serialized writer connection, so unlike an ad-hoc connection it
    /// cannot lose a race for the write lock and fail with SQLITE_BUSY — the
    /// failure mode that silently dropped one-time-migration flags four boots
    /// in a row (2026-07-14) and made a corpus-wiping rebuild re-run each boot.
    pub fn set_kv(&self, key: &str, value: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value, updated_at) VALUES (?1, ?2, datetime('now'))",
            params![key, value],
        )?;
        Ok(())
    }

    /// Record the current corpus size as the collapse-detection baseline.
    /// Call ONLY when the corpus is sound (healthy AND grounded) — a collapsed
    /// corpus must keep re-alarming against the last sound size, not ratify
    /// the collapse as the new normal.
    pub fn record_corpus_baseline(
        &self,
        health: &crate::context_admission::CorpusHealth,
    ) -> SqliteResult<()> {
        debug_assert!(health.healthy && health.grounding_chunks > 0);
        self.set_kv(
            crate::context_admission::CORPUS_BASELINE_KV_KEY,
            &health.total.to_string(),
        )
    }

    /// Get context count
    pub fn context_count(&self) -> SqliteResult<i64> {
        let conn = self.read_conn();
        conn.query_row("SELECT COUNT(*) FROM context_chunks", [], |row| row.get(0))
    }

    /// Snapshot the grounding-corpus composition and assess its health (the
    /// immune system). Cheap `GROUP BY`; call at startup and after indexing to
    /// catch a pollution recurrence the moment it forms.
    pub fn context_health(&self) -> SqliteResult<crate::context_admission::CorpusHealth> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT source_file, COALESCE(source_type, 'text') AS st, COUNT(*) AS c
             FROM context_chunks GROUP BY source_file, st",
        )?;
        let tallies = stmt
            .query_map([], |row| {
                Ok(crate::context_admission::SourceTally {
                    source_file: row.get(0)?,
                    source_type: row.get(1)?,
                    count: row.get::<_, i64>(2)? as usize,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;
        drop(stmt);
        drop(conn);
        let baseline = self
            .get_kv(crate::context_admission::CORPUS_BASELINE_KV_KEY)?
            .and_then(|v| v.parse::<usize>().ok());
        Ok(crate::context_admission::assess_corpus(&tallies, baseline))
    }

    /// Reclassify every context chunk's provenance from its source path and
    /// prune what the admission policy would not admit today: `reject`-classed
    /// rows and doc chunks beyond the per-file cap (keeping the earliest-indexed
    /// ones). Removes from both `context_chunks` and the `context_vec` shadow.
    ///
    /// This is the healing routine shared by the one-time migration (installed
    /// base) and the startup immune system (ongoing auto-quarantine). Idempotent:
    /// a second run reclassifies nothing and prunes nothing.
    pub fn reconcile_context_provenance(&self) -> SqliteResult<ContextReconcileStats> {
        use crate::context_admission::{
            classify_source_with_content, ContextClass, MAX_DOC_CHUNKS_PER_SOURCE,
        };
        use std::collections::HashMap;

        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_file, COALESCE(source_type, ''), text FROM context_chunks ORDER BY id",
        )?;
        struct Row {
            id: i64,
            source_file: String,
            source_type: String,
            class: ContextClass,
        }
        let rows: Vec<Row> = stmt
            .query_map([], |r| {
                let source_file: String = r.get(1)?;
                let text: String = r.get(3)?;
                // Classify inside the map so chunk text never accumulates in
                // memory — only the verdict is kept.
                let class = classify_source_with_content(&source_file, &text);
                Ok(Row {
                    id: r.get(0)?,
                    source_file,
                    source_type: r.get(2)?,
                    class,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;
        drop(stmt);

        // File-majority rule: chunking splits a test file into many chunks and
        // only some carry a marker (live: `stack_simulation.rs` fixture chunks
        // held bare fixture strings, no `#[test]` in sight — and its stored
        // source_file is a BARE basename, so the tests/-dir path signal is
        // gone). When a strong majority of a Code file's chunks are test-
        // classified, the FILE is a test file and every chunk of it is demoted.
        // Prod files with an inline test module sit well under the threshold,
        // so their production chunks keep grounding.
        const TEST_FILE_MAJORITY: f64 = 0.6;
        let mut per_file: HashMap<&str, (usize, usize)> = HashMap::new(); // (code+test, test)
        for row in &rows {
            if matches!(row.class, ContextClass::Code | ContextClass::TestCode) {
                let e = per_file.entry(row.source_file.as_str()).or_insert((0, 0));
                e.0 += 1;
                if row.class == ContextClass::TestCode {
                    e.1 += 1;
                }
            }
        }
        let test_majority_files: std::collections::HashSet<&str> = per_file
            .iter()
            .filter(|(_, counts)| {
                counts.0 >= 3 && (counts.1 as f64 / counts.0 as f64) >= TEST_FILE_MAJORITY
            })
            .map(|(f, _)| *f)
            .collect();

        let mut stats = ContextReconcileStats {
            scanned: rows.len(),
            ..Default::default()
        };
        let mut reclass: Vec<(i64, &'static str)> = Vec::new();
        let mut delete_ids: Vec<i64> = Vec::new();
        let mut doc_counts: HashMap<String, usize> = HashMap::new();

        for row in &rows {
            let mut class = row.class;
            if class == ContextClass::Reject {
                delete_ids.push(row.id);
                stats.pruned_reject += 1;
                continue;
            }
            if class == ContextClass::Code && test_majority_files.contains(row.source_file.as_str())
            {
                class = ContextClass::TestCode;
            }
            if class == ContextClass::Doc {
                let n = doc_counts.entry(row.source_file.clone()).or_insert(0);
                *n += 1;
                if *n > MAX_DOC_CHUNKS_PER_SOURCE {
                    delete_ids.push(row.id);
                    stats.pruned_over_cap += 1;
                    continue;
                }
            }
            let st = class.source_type();
            if row.source_type != st {
                reclass.push((row.id, st));
            }
        }

        let tx = conn.unchecked_transaction()?;
        {
            let mut up = tx.prepare("UPDATE context_chunks SET source_type = ?1 WHERE id = ?2")?;
            for (id, st) in &reclass {
                up.execute(params![st, id])?;
            }
        }
        stats.reclassified = reclass.len();
        {
            let mut del_c = tx.prepare("DELETE FROM context_chunks WHERE id = ?1")?;
            let mut del_v = tx.prepare("DELETE FROM context_vec WHERE rowid = ?1")?;
            for id in &delete_ids {
                del_v.execute(params![id])?;
                del_c.execute(params![id])?;
            }
        }
        tx.commit()?;
        Ok(stats)
    }

    /// KNN search for similar contexts using sqlite-vec (O(log n) instead of O(n)).
    ///
    /// Grounding is CODE-ONLY: results are restricted to grounding-eligible
    /// provenance (`code`/`config` — see [`crate::context_admission`]). Prose /
    /// doc embeddings are semantic wildcards that once surfaced a Spanish
    /// business course as "Similar to your code" on a Docker tool; they must
    /// never ground the feed nor move the context score. Both scoring pipelines
    /// read from here, so this one filter fixes evidence AND score at once.
    ///
    /// The filter is applied by OVER-FETCHING from the KNN index and then
    /// keeping only grounding-eligible rows — not as a `WHERE` on the vec
    /// `MATCH`, because sqlite-vec selects `k` rows FIRST and applies join
    /// predicates after, which would silently under-fill the result.
    pub fn find_similar_contexts(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> SqliteResult<Vec<SimilarityResult>> {
        let conn = self.read_conn();
        let embedding_blob = embedding_to_blob(query_embedding);
        let overfetch = limit.saturating_mul(6).max(24) as i64;

        let mut stmt = conn.prepare(
            "SELECT v.rowid, v.distance, c.source_file, c.text, c.source_type
             FROM context_vec v
             JOIN context_chunks c ON c.id = v.rowid
             WHERE v.embedding MATCH ?1 AND k = ?2
             ORDER BY v.distance",
        )?;

        let rows = stmt.query_map(params![embedding_blob, overfetch], |row| {
            Ok((
                SimilarityResult {
                    context_id: row.get(0)?,
                    distance: row.get(1)?,
                    source_file: row.get(2)?,
                    text: row.get(3)?,
                },
                row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            ))
        })?;

        let mut out = Vec::with_capacity(limit);
        for row in rows {
            let (res, source_type) = row?;
            let grounds = crate::context_admission::ContextClass::from_source_type(&source_type)
                .is_some_and(crate::context_admission::ContextClass::grounding_eligible);
            if grounds {
                out.push(res);
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }
}
