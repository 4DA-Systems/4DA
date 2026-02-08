//! Database module for 4DA - Persistence layer for embeddings and sources
//!
//! Uses sqlite-vec for vector similarity search at scale.
//! Designed to handle hundreds of thousands of sources.

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{
    ffi::sqlite3_auto_extension, params, Connection, OptionalExtension, Result as SqliteResult,
};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use tracing::info;

// ============================================================================
// Types
// ============================================================================

/// A stored context chunk with its embedding
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields populated from SQL, returned via API
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
#[allow(dead_code)] // Fields populated from SQL, returned via API
pub struct StoredSourceItem {
    pub id: i64,
    pub source_type: String, // "hackernews", "arxiv", "rss", etc.
    pub source_id: String,   // External ID (HN id, arXiv id, etc.)
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
#[allow(dead_code)] // Fields populated from SQL, returned via API
pub struct SimilarityResult {
    pub context_id: i64,
    pub source_file: String,
    pub text: String,
    pub distance: f32,
}

// ============================================================================
// Database Manager
// ============================================================================

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Initialize database with sqlite-vec extension
    pub fn new(db_path: &Path) -> SqliteResult<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        // Register sqlite-vec extension BEFORE opening connection
        // This must be done once globally and before any connection is opened
        unsafe {
            sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }

        let conn = Connection::open(db_path)?;

        // Enable extension loading and initialize sqlite-vec
        conn.execute_batch(
            "
            -- Enable foreign keys
            PRAGMA foreign_keys = ON;

            -- WAL mode for better concurrency
            PRAGMA journal_mode = WAL;
        ",
        )?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        // Run migrations
        db.migrate()?;

        Ok(db)
    }

    /// Run database migrations
    fn migrate(&self) -> SqliteResult<()> {
        let conn = self.conn.lock();

        conn.execute_batch(
            "
            -- Context chunks table (your local files)
            CREATE TABLE IF NOT EXISTS context_chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_file TEXT NOT NULL,
                content_hash TEXT NOT NULL UNIQUE,
                text TEXT NOT NULL,
                embedding BLOB NOT NULL,
                weight REAL NOT NULL DEFAULT 1.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_context_source ON context_chunks(source_file);
            CREATE INDEX IF NOT EXISTS idx_context_hash ON context_chunks(content_hash);

            -- Source items table (HN, arXiv, RSS, etc.)
            CREATE TABLE IF NOT EXISTS source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                url TEXT,
                title TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                content_hash TEXT NOT NULL,
                embedding BLOB NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_seen TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(source_type, source_id)
            );
            CREATE INDEX IF NOT EXISTS idx_source_type ON source_items(source_type);
            CREATE INDEX IF NOT EXISTS idx_source_hash ON source_items(content_hash);
            CREATE INDEX IF NOT EXISTS idx_source_seen ON source_items(last_seen);
            CREATE INDEX IF NOT EXISTS idx_source_type_created ON source_items(source_type, created_at);

            -- Sources registry (track what sources we monitor)
            CREATE TABLE IF NOT EXISTS sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                config TEXT,  -- JSON config for the source
                last_fetch TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            -- User feedback for learning
            CREATE TABLE IF NOT EXISTS feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER NOT NULL,
                relevant INTEGER NOT NULL,  -- 1 = relevant, 0 = not relevant
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (source_item_id) REFERENCES source_items(id)
            );
            CREATE INDEX IF NOT EXISTS idx_feedback_item ON feedback(source_item_id);

            -- Schema version for future migrations
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            );
        ",
        )?;

        // Insert initial schema version (separate from batch, with explicit check)
        let version_exists: bool = conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |row| {
                row.get::<_, i64>(0).map(|count| count > 0)
            })
            .unwrap_or(false);

        if !version_exists {
            conn.execute("INSERT INTO schema_version (version) VALUES (1)", [])?;
        }

        // Migration: Add weight column if it doesn't exist (for existing databases)
        let has_weight_column: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('context_chunks') WHERE name='weight'",
                [],
                |row| row.get::<_, i64>(0).map(|count| count > 0),
            )
            .unwrap_or(false);

        if !has_weight_column {
            conn.execute(
                "ALTER TABLE context_chunks ADD COLUMN weight REAL NOT NULL DEFAULT 1.0",
                [],
            )?;
            info!("Added weight column to context_chunks table");
        }

        // Create vec0 virtual tables for KNN search (sqlite-vec)
        // These enable O(log n) similarity search instead of O(n) brute force
        conn.execute_batch(
            "
            -- Vector index for context chunks (384-dim MiniLM embeddings)
            CREATE VIRTUAL TABLE IF NOT EXISTS context_vec USING vec0(
                embedding float[384]
            );

            -- Vector index for source items (384-dim MiniLM embeddings)
            CREATE VIRTUAL TABLE IF NOT EXISTS source_vec USING vec0(
                embedding float[384]
            );
        ",
        )?;

        // Phase 1 migration: Multi-format file support
        let current_version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap_or(1);

        if current_version < 2 {
            info!(target: "4da::db", "Running Phase 1 migration (schema version 2)");
            Self::migrate_to_phase_1(&conn)?;
            conn.execute("UPDATE schema_version SET version = 2", [])?;
            info!(target: "4da::db", "Phase 1 migration completed");
        }

        // Phase 2 migration: Natural Language Query System
        let current_version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap_or(2);

        if current_version < 3 {
            info!(target: "4da::db", "Running Phase 2 migration (schema version 3)");
            Self::migrate_to_phase_2(&conn)?;
            conn.execute("UPDATE schema_version SET version = 3", [])?;
            info!(target: "4da::db", "Phase 2 migration completed");
        }

        info!(target: "4da::db", "Database schema initialized with sqlite-vec");
        Ok(())
    }

    /// Phase 1 migration: Multi-format file support
    fn migrate_to_phase_1(conn: &Connection) -> SqliteResult<()> {
        // Add source_type column for tracking file formats
        let has_source_type: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('context_chunks') WHERE name='source_type'",
                [],
                |row| row.get::<_, i64>(0).map(|count| count > 0),
            )
            .unwrap_or(false);

        if !has_source_type {
            conn.execute(
                "ALTER TABLE context_chunks ADD COLUMN source_type TEXT DEFAULT 'text'",
                [],
            )?;
            info!("Added source_type column to context_chunks");
        }

        // Add page_number column for multi-page documents
        let has_page_number: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('context_chunks') WHERE name='page_number'",
                [],
                |row| row.get::<_, i64>(0).map(|count| count > 0),
            )
            .unwrap_or(false);

        if !has_page_number {
            conn.execute(
                "ALTER TABLE context_chunks ADD COLUMN page_number INTEGER",
                [],
            )?;
            info!("Added page_number column to context_chunks");
        }

        // Add confidence column for OCR/transcription quality
        let has_confidence: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('context_chunks') WHERE name='confidence'",
                [],
                |row| row.get::<_, i64>(0).map(|count| count > 0),
            )
            .unwrap_or(false);

        if !has_confidence {
            conn.execute(
                "ALTER TABLE context_chunks ADD COLUMN confidence REAL DEFAULT 1.0",
                [],
            )?;
            info!("Added confidence column to context_chunks");
        }

        // Add extracted_at column for tracking extraction time
        let has_extracted_at: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('context_chunks') WHERE name='extracted_at'",
                [],
                |row| row.get::<_, i64>(0).map(|count| count > 0),
            )
            .unwrap_or(false);

        if !has_extracted_at {
            conn.execute(
                "ALTER TABLE context_chunks ADD COLUMN extracted_at TEXT",
                [],
            )?;
            info!("Added extracted_at column to context_chunks");
        }

        // Create extraction_jobs table for async processing
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS extraction_jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                file_type TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('pending', 'processing', 'completed', 'failed')),
                error TEXT,
                started_at TEXT,
                completed_at TEXT,
                extracted_chunks INTEGER DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_extraction_jobs_status ON extraction_jobs(status);
            CREATE INDEX IF NOT EXISTS idx_extraction_jobs_file_path ON extraction_jobs(file_path);
        ",
        )?;
        info!("Created extraction_jobs table");

        // Create file_metadata_cache table to avoid reprocessing
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS file_metadata_cache (
                file_path TEXT PRIMARY KEY,
                file_hash TEXT NOT NULL,
                file_type TEXT NOT NULL,
                page_count INTEGER,
                word_count INTEGER,
                extracted_at TEXT NOT NULL,
                last_modified TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_file_metadata_hash ON file_metadata_cache(file_hash);
            CREATE INDEX IF NOT EXISTS idx_file_metadata_type ON file_metadata_cache(file_type);
        ",
        )?;
        info!("Created file_metadata_cache table");

        Ok(())
    }

    /// Phase 2 migration: Natural Language Query System
    fn migrate_to_phase_2(conn: &Connection) -> SqliteResult<()> {
        // Create query_cache table for caching parsed queries
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS query_cache (
                query_hash TEXT PRIMARY KEY,
                natural_language TEXT NOT NULL,
                parsed_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_used TEXT NOT NULL DEFAULT (datetime('now')),
                use_count INTEGER DEFAULT 1
            );
            CREATE INDEX IF NOT EXISTS idx_query_cache_created ON query_cache(created_at);
        ",
        )?;
        info!("Created query_cache table");

        // Create query_history table for learning from user queries
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS query_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query TEXT NOT NULL,
                parsed_intent TEXT,
                results_count INTEGER NOT NULL,
                user_clicked BOOLEAN DEFAULT 0,
                clicked_item_id INTEGER,
                execution_ms INTEGER,
                timestamp TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_query_history_timestamp ON query_history(timestamp);
            CREATE INDEX IF NOT EXISTS idx_query_history_intent ON query_history(parsed_intent);
        ",
        )?;
        info!("Created query_history table");

        // Create chunk_sentiment table for sentiment analysis caching
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS chunk_sentiment (
                chunk_id INTEGER PRIMARY KEY,
                sentiment TEXT NOT NULL CHECK(sentiment IN ('positive', 'negative', 'neutral', 'mixed')),
                confidence REAL NOT NULL,
                keywords TEXT,
                analyzed_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (chunk_id) REFERENCES context_chunks(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_chunk_sentiment_sentiment ON chunk_sentiment(sentiment);
        ",
        )?;
        info!("Created chunk_sentiment table");

        // Create void_positions table for caching 3D projected positions
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS void_positions (
                item_id INTEGER NOT NULL,
                item_type TEXT NOT NULL,
                x REAL NOT NULL,
                y REAL NOT NULL,
                z REAL NOT NULL,
                projection_version INTEGER NOT NULL,
                PRIMARY KEY (item_id, item_type)
            );
            CREATE INDEX IF NOT EXISTS idx_void_positions_version
                ON void_positions(projection_version);
        ",
        )?;
        info!("Created void_positions table");

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
        // Default weight of 1.0 for backwards compatibility
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

        // Check if exists to determine if insert or update
        let existing_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM context_chunks WHERE content_hash = ?1",
                params![content_hash],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing_id {
            // Update existing
            conn.execute(
                "UPDATE context_chunks SET source_file = ?1, weight = ?2, updated_at = datetime('now') WHERE id = ?3",
                params![source_file, weight, id],
            )?;
            // Update vec0 index
            conn.execute(
                "UPDATE context_vec SET embedding = ?1 WHERE rowid = ?2",
                params![embedding_blob, id],
            )?;
            Ok(id)
        } else {
            // Insert new
            conn.execute(
                "INSERT INTO context_chunks (source_file, content_hash, text, embedding, weight, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
                params![source_file, content_hash, text, embedding_blob, weight],
            )?;
            let id = conn.last_insert_rowid();
            // Insert into vec0 index with matching rowid
            conn.execute(
                "INSERT INTO context_vec (rowid, embedding) VALUES (?1, ?2)",
                params![id, embedding_blob],
            )?;
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
                created_at: Utc::now(), // Simplified for now
            })
        })?;

        rows.collect()
    }

    /// Clear all context chunks (for re-indexing)
    pub fn clear_contexts(&self) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        // Clear vec0 index first
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
    #[allow(dead_code)] // Future: similarity-based recommendations
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
                created_at: Utc::now(),
                last_seen: Utc::now(),
            })
        })?;

        rows.collect()
    }

    // ========================================================================
    // Source Item Operations
    // ========================================================================

    /// Store or update a source item (also updates vec0 index)
    pub fn upsert_source_item(
        &self,
        source_type: &str,
        source_id: &str,
        url: Option<&str>,
        title: &str,
        content: &str,
        embedding: &[f32],
    ) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        let content_hash = hash_content(&format!("{}{}", title, content));
        let embedding_blob = embedding_to_blob(embedding);

        // Check if exists to determine if insert or update
        let existing_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM source_items WHERE source_type = ?1 AND source_id = ?2",
                params![source_type, source_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing_id {
            // Update existing
            conn.execute(
                "UPDATE source_items SET url = ?1, title = ?2, content = ?3, content_hash = ?4, embedding = ?5, last_seen = datetime('now') WHERE id = ?6",
                params![url, title, content, content_hash, embedding_blob, id],
            )?;
            // Update vec0 index
            conn.execute(
                "UPDATE source_vec SET embedding = ?1 WHERE rowid = ?2",
                params![embedding_blob, id],
            )?;
            Ok(id)
        } else {
            // Insert new
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, embedding, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
                params![source_type, source_id, url, title, content, content_hash, embedding_blob],
            )?;
            let id = conn.last_insert_rowid();
            // Insert into vec0 index with matching rowid
            conn.execute(
                "INSERT INTO source_vec (rowid, embedding) VALUES (?1, ?2)",
                params![id, embedding_blob],
            )?;
            Ok(id)
        }
    }

    /// Check if a source item exists (for incremental updates)
    #[allow(dead_code)] // Future: incremental fetch optimization
    pub fn source_item_exists(&self, source_type: &str, source_id: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM source_items WHERE source_type = ?1 AND source_id = ?2",
            params![source_type, source_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get a single source item by type and id (for cache lookup)
    pub fn get_source_item(
        &self,
        source_type: &str,
        source_id: &str,
    ) -> SqliteResult<Option<StoredSourceItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen
             FROM source_items
             WHERE source_type = ?1 AND source_id = ?2"
        )?;

        let mut rows = stmt.query_map(params![source_type, source_id], |row| {
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
                created_at: Utc::now(),
                last_seen: Utc::now(),
            })
        })?;

        match rows.next() {
            Some(Ok(item)) => Ok(Some(item)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    /// Update last_seen timestamp for an existing item
    pub fn touch_source_item(&self, source_type: &str, source_id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE source_items SET last_seen = datetime('now') WHERE source_type = ?1 AND source_id = ?2",
            params![source_type, source_id],
        )?;
        Ok(())
    }

    /// Get source items by type
    #[allow(dead_code)] // Future: source-specific queries
    pub fn get_source_items(
        &self,
        source_type: &str,
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen
             FROM source_items
             WHERE source_type = ?1
             ORDER BY last_seen DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![source_type, limit as i64], |row| {
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
                created_at: Utc::now(),
                last_seen: Utc::now(),
            })
        })?;

        rows.collect()
    }

    /// Get recent source items within a time window (hours)
    /// Returns items with embeddings for cache-first analysis
    pub fn get_items_since_hours(
        &self,
        hours: i64,
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen
             FROM source_items
             WHERE datetime(last_seen) >= datetime('now', ?1)
             ORDER BY last_seen DESC
             LIMIT ?2"
        )?;

        let hours_param = format!("-{} hours", hours);
        let rows = stmt.query_map(params![hours_param, limit as i64], |row| {
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

    /// Count items by source type
    pub fn source_item_count(&self, source_type: &str) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM source_items WHERE source_type = ?1",
            params![source_type],
            |row| row.get(0),
        )
    }

    /// Count total items
    pub fn total_item_count(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM source_items", [], |row| row.get(0))
    }

    // ========================================================================
    // Feedback Operations
    // ========================================================================

    /// Record user feedback
    #[allow(dead_code)] // Future: learning loop integration
    pub fn record_feedback(&self, source_item_id: i64, relevant: bool) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, ?2)",
            params![source_item_id, relevant as i32],
        )?;
        Ok(())
    }

    // ========================================================================
    // Source Registry
    // ========================================================================

    /// Register a source
    pub fn register_source(&self, source_type: &str, name: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO sources (source_type, name) VALUES (?1, ?2)",
            params![source_type, name],
        )?;
        Ok(())
    }

    /// Update last fetch time for a source
    pub fn update_source_fetch_time(&self, source_type: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE sources SET last_fetch = datetime('now') WHERE source_type = ?1",
            params![source_type],
        )?;
        Ok(())
    }

    /// Get enabled sources
    #[allow(dead_code)] // Future: source management UI
    pub fn get_enabled_sources(&self) -> SqliteResult<Vec<(String, String)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT source_type, name FROM sources WHERE enabled = 1")?;

        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

        rows.collect()
    }
}

// ============================================================================
// Digest Types
// ============================================================================

/// Item for digest purposes
#[derive(Debug, Clone)]
pub struct DigestSourceItem {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub relevance_score: Option<f64>,
    pub topics: Vec<String>,
}

// ============================================================================
// Digest Operations (Database extension)
// ============================================================================

impl Database {
    /// Get recent source items since a given date, for digest generation
    /// Returns items ordered by created_at descending
    pub fn get_relevant_items_since(
        &self,
        since: chrono::DateTime<chrono::Utc>,
        _min_score: f64,
        limit: usize,
    ) -> SqliteResult<Vec<DigestSourceItem>> {
        let conn = self.conn.lock();

        // Get recent items - note: relevance_score is computed separately
        // For now, we return all recent items and let the caller compute scores
        let mut stmt = conn.prepare(
            "SELECT id, title, url, source_type, created_at, content
             FROM source_items
             WHERE datetime(created_at) >= datetime(?1)
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;

        let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();

        let rows = stmt.query_map(params![since_str, limit as i64], |row| {
            let content: String = row.get(5)?;
            // Extract simple topics from content (first few meaningful words)
            let topics: Vec<String> = content
                .split_whitespace()
                .filter(|w| w.len() > 4)
                .take(5)
                .map(|s| s.to_lowercase())
                .collect();

            Ok(DigestSourceItem {
                id: row.get(0)?,
                title: row.get(1)?,
                url: row.get(2)?,
                source_type: row.get(3)?,
                created_at: parse_datetime(row.get::<_, String>(4)?),
                relevance_score: None, // Computed elsewhere
                topics,
            })
        })?;

        rows.collect()
    }

    // ========================================================================
    // Void Position Cache Operations (Phase 2: Universe)
    // ========================================================================

    /// Upsert a projected 3D position for an item
    #[allow(dead_code)]
    pub fn upsert_void_position(
        &self,
        item_id: i64,
        item_type: &str,
        x: f32,
        y: f32,
        z: f32,
        projection_version: i64,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO void_positions (item_id, item_type, x, y, z, projection_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(item_id, item_type) DO UPDATE SET
                x = excluded.x, y = excluded.y, z = excluded.z,
                projection_version = excluded.projection_version",
            params![item_id, item_type, x, y, z, projection_version],
        )?;
        Ok(())
    }

    /// Batch upsert positions (much faster than individual calls)
    #[allow(dead_code)]
    pub fn upsert_void_positions_batch(
        &self,
        positions: &[(i64, &str, f32, f32, f32)],
        projection_version: i64,
    ) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let mut count = 0;
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO void_positions (item_id, item_type, x, y, z, projection_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(item_id, item_type) DO UPDATE SET
                    x = excluded.x, y = excluded.y, z = excluded.z,
                    projection_version = excluded.projection_version",
            )?;
            for (id, item_type, x, y, z) in positions {
                stmt.execute(params![id, item_type, x, y, z, projection_version])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Get cached position for a single item
    #[allow(dead_code)]
    pub fn get_void_position(
        &self,
        item_id: i64,
        item_type: &str,
    ) -> SqliteResult<Option<(f32, f32, f32, i64)>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT x, y, z, projection_version FROM void_positions
             WHERE item_id = ?1 AND item_type = ?2",
            params![item_id, item_type],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
    }

    /// Get all cached positions for a given projection version
    #[allow(dead_code)]
    pub fn get_void_positions(
        &self,
        projection_version: i64,
        limit: usize,
    ) -> SqliteResult<Vec<(i64, String, f32, f32, f32)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT item_id, item_type, x, y, z FROM void_positions
             WHERE projection_version = ?1
             ORDER BY item_id
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![projection_version, limit as i64], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Delete positions for a specific projection version (for cache invalidation)
    #[allow(dead_code)]
    pub fn clear_void_positions(&self, projection_version: Option<i64>) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        if let Some(version) = projection_version {
            conn.execute(
                "DELETE FROM void_positions WHERE projection_version = ?1",
                params![version],
            )
        } else {
            conn.execute("DELETE FROM void_positions", [])
        }
    }

    /// Count cached positions
    #[allow(dead_code)]
    pub fn void_position_count(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM void_positions", [], |row| row.get(0))
    }

    // ========================================================================
    // Void Universe Data Queries (Phase 2)
    // ========================================================================

    /// Get source items with embeddings for projection (lightweight: no content text).
    /// Returns (id, source_type, title, url, embedding, age_hours).
    #[allow(dead_code)]
    pub fn get_source_items_for_projection(
        &self,
        limit: usize,
    ) -> SqliteResult<Vec<(i64, String, String, Option<String>, Vec<f32>, f32)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, title, url, embedding,
                    (julianday('now') - julianday(last_seen)) * 24.0 as age_hours
             FROM source_items
             ORDER BY last_seen DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let embedding_blob: Vec<u8> = row.get(4)?;
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                blob_to_embedding(&embedding_blob),
                row.get::<_, f64>(5).unwrap_or(0.0) as f32,
            ))
        })?;

        rows.collect()
    }

    /// Get context chunks with embeddings for projection (lightweight).
    /// Returns (id, source_file, text_preview, embedding).
    #[allow(dead_code)]
    pub fn get_context_chunks_for_projection(
        &self,
        limit: usize,
    ) -> SqliteResult<Vec<(i64, String, String, Vec<f32>)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_file, substr(text, 1, 100), embedding
             FROM context_chunks
             ORDER BY updated_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let embedding_blob: Vec<u8> = row.get(3)?;
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                blob_to_embedding(&embedding_blob),
            ))
        })?;

        rows.collect()
    }

    /// Get a single source item by ID (full detail for particle selection)
    #[allow(dead_code)]
    pub fn get_source_item_by_id(&self, id: i64) -> SqliteResult<Option<StoredSourceItem>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen
             FROM source_items WHERE id = ?1",
            params![id],
            |row| {
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
            },
        ).optional()
    }
}

/// Parse datetime string to chrono DateTime
fn parse_datetime(s: String) -> chrono::DateTime<chrono::Utc> {
    use chrono::{NaiveDateTime, TimeZone, Utc};
    NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
        .map(|dt| Utc.from_utc_datetime(&dt))
        .unwrap_or_else(|_| Utc::now())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Hash content for deduplication
fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// Convert f32 embedding to blob for storage
fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert blob back to f32 embedding
fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().unwrap();
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
