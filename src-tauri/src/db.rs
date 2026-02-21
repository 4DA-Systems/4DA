#![allow(dead_code)]
//! Database module for 4DA - Persistence layer for embeddings and sources
//!
//! Uses sqlite-vec for vector similarity search at scale.
//! Designed to handle hundreds of thousands of sources.

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::info;

// ============================================================================
// Types
// ============================================================================

/// Source info tuple: (source_type, name, enabled, last_fetch)
pub type SourceInfo = (String, String, bool, Option<String>);

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
    db_path: PathBuf,
}

impl Database {
    /// Initialize database with sqlite-vec extension
    pub fn new(db_path: &Path) -> SqliteResult<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        // Register sqlite-vec extension BEFORE opening connection
        crate::register_sqlite_vec_extension();

        let conn = Connection::open(db_path)?;

        // Enable extension loading and initialize sqlite-vec
        conn.execute_batch(
            "
            -- Enable foreign keys
            PRAGMA foreign_keys = ON;

            -- WAL mode for better concurrency
            PRAGMA journal_mode = WAL;

            -- Performance: fsync less often (safe with WAL)
            PRAGMA synchronous = NORMAL;

            -- Performance: 64MB page cache
            PRAGMA cache_size = -64000;

            -- Performance: 256MB memory-mapped I/O
            PRAGMA mmap_size = 268435456;

            -- Performance: temp tables in RAM
            PRAGMA temp_store = MEMORY;

            -- Concurrency: wait up to 5s when DB is locked
            PRAGMA busy_timeout = 5000;
        ",
        )?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: db_path.to_path_buf(),
        };

        // Run migrations
        db.migrate()?;

        Ok(db)
    }

    /// Create a pre-migration backup of the database file.
    /// Keeps only the last 2 backups to avoid disk bloat.
    fn backup_before_migration(&self, current_version: i64) {
        let backup_path = self
            .db_path
            .with_extension(format!("db.backup.v{}", current_version));
        // Checkpoint WAL so the main db file is consistent for copy
        if let Some(conn) = self.conn.try_lock() {
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");
        }
        match std::fs::copy(&self.db_path, &backup_path) {
            Ok(bytes) => {
                info!(target: "4da::db", path = %backup_path.display(), bytes, "Pre-migration backup created")
            }
            Err(e) => {
                tracing::warn!(target: "4da::db", error = %e, "Pre-migration backup failed (continuing anyway)")
            }
        }
        // Prune old backups: keep only the 2 most recent
        if let Some(parent) = self.db_path.parent() {
            if let Ok(entries) = std::fs::read_dir(parent) {
                let mut backups: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.to_string_lossy().contains(".db.backup.v"))
                    .collect();
                backups.sort();
                if backups.len() > 2 {
                    for old in &backups[..backups.len() - 2] {
                        let _ = std::fs::remove_file(old);
                        info!(target: "4da::db", path = %old.display(), "Pruned old backup");
                    }
                }
            }
        }
    }

    /// Run a migration step inside a transaction with history recording.
    /// If the migration function fails, the transaction rolls back and schema_version is unchanged.
    fn run_versioned_migration(
        conn: &Connection,
        from_version: i64,
        to_version: i64,
        name: &str,
        migration_fn: impl FnOnce(&Connection) -> SqliteResult<()>,
    ) -> SqliteResult<()> {
        let start = std::time::Instant::now();
        info!(target: "4da::db", "Running {} (schema version {} -> {})", name, from_version, to_version);

        // Execute migration inside a transaction
        let result = {
            let tx = conn.unchecked_transaction()?;
            let res = migration_fn(&tx).and_then(|_| {
                tx.execute(
                    "UPDATE schema_version SET version = ?1",
                    params![to_version],
                )?;
                Ok(())
            });
            match res {
                Ok(()) => tx.commit(),
                Err(e) => Err(e), // tx dropped -> auto-rollback
            }
        };

        let duration_ms = start.elapsed().as_millis() as i64;

        // Record in migration_history (non-fatal if this fails)
        let _ = conn.execute(
            "INSERT INTO migration_history (from_version, to_version, executed_at, duration_ms, success) VALUES (?1, ?2, datetime('now'), ?3, ?4)",
            params![from_version, to_version, duration_ms, result.is_ok() as i32],
        );

        match &result {
            Ok(()) => {
                info!(target: "4da::db", name, to_version, duration_ms, "{} completed in {}ms", name, duration_ms)
            }
            Err(e) => {
                tracing::error!(target: "4da::db", name, to_version, error = %e, "{} FAILED — rolled back", name)
            }
        }

        result
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

            -- Migration history for debugging
            CREATE TABLE IF NOT EXISTS migration_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_version INTEGER NOT NULL,
                to_version INTEGER NOT NULL,
                executed_at TEXT NOT NULL DEFAULT (datetime('now')),
                duration_ms INTEGER NOT NULL DEFAULT 0,
                success INTEGER NOT NULL DEFAULT 0
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

        // Determine current schema version for backup decision
        let mut current_version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap_or(1);

        const TARGET_VERSION: i64 = 15;
        if current_version < TARGET_VERSION {
            // Drop the conn lock briefly to allow backup (needs filesystem access)
            drop(conn);
            self.backup_before_migration(current_version);
            // Re-acquire the lock
            let conn = self.conn.lock();

            // Re-read version after re-acquiring lock
            current_version = conn
                .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
                .unwrap_or(1);

            // Phase 1 migration: Multi-format file support
            if current_version < 2 {
                Self::run_versioned_migration(&conn, 1, 2, "Phase 1: multi-format files", |c| {
                    Self::migrate_to_phase_1(c)
                })?;
                current_version = 2;
            }

            // Phase 2 migration: Natural Language Query System
            if current_version < 3 {
                Self::run_versioned_migration(&conn, 2, 3, "Phase 2: NL query system", |c| {
                    Self::migrate_to_phase_2(c)
                })?;
                current_version = 3;
            }

            // Phase 3 migration: Embedding status tracking for retry
            if current_version < 4 {
                Self::run_versioned_migration(&conn, 3, 4, "Phase 3: embedding retry", |c| {
                    Self::migrate_to_phase_3(c)
                })?;
                current_version = 4;
            }

            // Phase 5 migration: Innovation features infrastructure
            if current_version < 5 {
                Self::run_versioned_migration(&conn, 4, 5, "Phase 5: innovation infra", |c| {
                    Self::migrate_to_phase_5(c)
                })?;
                current_version = 5;
            }

            // Phase 6 migration: Source health table
            if current_version < 6 {
                Self::run_versioned_migration(&conn, 5, 6, "Phase 6: source health", |c| {
                    c.execute_batch(
                        "CREATE TABLE IF NOT EXISTS source_health (
                            source_type TEXT PRIMARY KEY,
                            status TEXT NOT NULL DEFAULT 'unknown',
                            last_success TEXT,
                            last_error TEXT,
                            error_count INTEGER NOT NULL DEFAULT 0,
                            consecutive_failures INTEGER NOT NULL DEFAULT 0,
                            items_fetched INTEGER NOT NULL DEFAULT 0,
                            response_time_ms INTEGER NOT NULL DEFAULT 0,
                            checked_at TEXT NOT NULL DEFAULT (datetime('now'))
                        )",
                    )
                })?;
                current_version = 6;
            }

            // Phase 7 migration: AI summary column on source_items
            if current_version < 7 {
                Self::run_versioned_migration(&conn, 6, 7, "Phase 7: summary column", |c| {
                    let has_summary: bool = c
                        .query_row(
                            "SELECT COUNT(*) FROM pragma_table_info('source_items') WHERE name='summary'",
                            [],
                            |row| row.get::<_, i64>(0).map(|count| count > 0),
                        )
                        .unwrap_or(false);
                    if !has_summary {
                        c.execute(
                            "ALTER TABLE source_items ADD COLUMN summary TEXT DEFAULT NULL",
                            [],
                        )?;
                    }
                    Ok(())
                })?;
                current_version = 7;
            }

            // Phase 8 migration: Persistent briefings table
            if current_version < 8 {
                Self::run_versioned_migration(&conn, 7, 8, "Phase 8: briefings table", |c| {
                    c.execute_batch(
                        "CREATE TABLE IF NOT EXISTS briefings (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            content TEXT NOT NULL,
                            model TEXT,
                            item_count INTEGER NOT NULL DEFAULT 0,
                            tokens_used INTEGER,
                            latency_ms INTEGER,
                            created_at TEXT NOT NULL DEFAULT (datetime('now'))
                        )",
                    )
                })?;
                current_version = 8;
            }

            // Phase 9 migration: Decision Intelligence Layer
            if current_version < 9 {
                Self::run_versioned_migration(&conn, 8, 9, "Phase 9: decisions", |c| {
                    c.execute_batch(
                        "CREATE TABLE IF NOT EXISTS developer_decisions (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            decision_type TEXT NOT NULL,
                            subject TEXT NOT NULL,
                            decision TEXT NOT NULL,
                            rationale TEXT,
                            alternatives_rejected TEXT DEFAULT '[]',
                            context_tags TEXT DEFAULT '[]',
                            confidence REAL NOT NULL DEFAULT 0.8,
                            status TEXT NOT NULL DEFAULT 'active',
                            superseded_by INTEGER,
                            created_at TEXT NOT NULL DEFAULT (datetime('now')),
                            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                            FOREIGN KEY (superseded_by) REFERENCES developer_decisions(id)
                        );
                        CREATE INDEX IF NOT EXISTS idx_decisions_type ON developer_decisions(decision_type);
                        CREATE INDEX IF NOT EXISTS idx_decisions_subject ON developer_decisions(subject);
                        CREATE INDEX IF NOT EXISTS idx_decisions_status ON developer_decisions(status);",
                    )
                })?;

                // Auto-seed decisions from tech_stack (outside transaction, non-fatal)
                if let Err(e) = crate::decisions::seed_decisions_from_profile(&conn) {
                    tracing::warn!(target: "4da::db", error = %e, "Auto-seed decisions failed (non-fatal)");
                }
                current_version = 9;
            }

            // Phase 10 migration: Agent Context Provider
            if current_version < 10 {
                Self::run_versioned_migration(&conn, 9, 10, "Phase 10: agent memory", |c| {
                    c.execute_batch(
                        "CREATE TABLE IF NOT EXISTS agent_memory (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            session_id TEXT NOT NULL,
                            agent_type TEXT NOT NULL,
                            memory_type TEXT NOT NULL,
                            subject TEXT NOT NULL,
                            content TEXT NOT NULL,
                            context_tags TEXT DEFAULT '[]',
                            created_at TEXT NOT NULL DEFAULT (datetime('now')),
                            expires_at TEXT,
                            promoted_to_decision_id INTEGER,
                            FOREIGN KEY (promoted_to_decision_id) REFERENCES developer_decisions(id)
                        );
                        CREATE INDEX IF NOT EXISTS idx_agent_memory_type ON agent_memory(memory_type);
                        CREATE INDEX IF NOT EXISTS idx_agent_memory_subject ON agent_memory(subject);
                        CREATE INDEX IF NOT EXISTS idx_agent_memory_session ON agent_memory(session_id);
                        CREATE INDEX IF NOT EXISTS idx_agent_memory_expires ON agent_memory(expires_at);",
                    )
                })?;
            }

            // Phase 11 migration: Command Deck tables
            if current_version < 11 {
                Self::run_versioned_migration(&conn, 10, 11, "Phase 11: command deck", |c| {
                    c.execute_batch(
                        "CREATE TABLE IF NOT EXISTS command_history (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            command TEXT NOT NULL,
                            working_dir TEXT NOT NULL,
                            exit_code INTEGER,
                            success INTEGER NOT NULL DEFAULT 0,
                            output_preview TEXT,
                            created_at TEXT NOT NULL DEFAULT (datetime('now'))
                        );
                        CREATE INDEX IF NOT EXISTS idx_cmd_history_created ON command_history(created_at);

                        CREATE TABLE IF NOT EXISTS git_commit_history (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            repo_path TEXT NOT NULL,
                            commit_hash TEXT NOT NULL,
                            message TEXT NOT NULL,
                            branch TEXT NOT NULL,
                            files_changed INTEGER NOT NULL DEFAULT 0,
                            created_at TEXT NOT NULL DEFAULT (datetime('now'))
                        );
                        CREATE INDEX IF NOT EXISTS idx_git_commits_repo ON git_commit_history(repo_path);",
                    )
                })?;
                current_version = 11;
            }

            // Phase 12 migration: Toolkit HTTP history
            if current_version < 12 {
                Self::run_versioned_migration(
                    &conn,
                    11,
                    12,
                    "Phase 12: toolkit http history",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS toolkit_http_history (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                method TEXT NOT NULL,
                                url TEXT NOT NULL,
                                status INTEGER NOT NULL,
                                duration_ms INTEGER NOT NULL DEFAULT 0,
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );
                            CREATE INDEX IF NOT EXISTS idx_http_history_created
                                ON toolkit_http_history(created_at);",
                        )
                    },
                )?;
                current_version = 12;
            }

            // Phase 13 migration: Stack Intelligence System
            if current_version < 13 {
                Self::run_versioned_migration(
                    &conn,
                    12,
                    13,
                    "Phase 13: stack intelligence",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS selected_stacks (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                profile_id TEXT NOT NULL UNIQUE,
                                auto_detected INTEGER DEFAULT 0,
                                confidence REAL DEFAULT 1.0,
                                created_at TEXT DEFAULT (datetime('now'))
                            );
                            CREATE INDEX IF NOT EXISTS idx_selected_stacks_profile
                                ON selected_stacks(profile_id);",
                        )
                    },
                )?;
            }

            // Phase 14 migration: Sovereign Profile
            if current_version < 14 {
                Self::run_versioned_migration(&conn, 13, 14, "Phase 14: sovereign profile", |c| {
                    c.execute_batch(
                        "CREATE TABLE IF NOT EXISTS sovereign_profile (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                category TEXT NOT NULL,
                                key TEXT NOT NULL,
                                value TEXT NOT NULL,
                                raw_output TEXT,
                                source_command TEXT,
                                source_lesson TEXT,
                                confidence REAL DEFAULT 1.0,
                                created_at TEXT DEFAULT (datetime('now')),
                                updated_at TEXT DEFAULT (datetime('now')),
                                UNIQUE(category, key)
                            );
                            CREATE INDEX IF NOT EXISTS idx_sovereign_category
                                ON sovereign_profile(category);

                            CREATE TABLE IF NOT EXISTS command_execution_log (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                module_id TEXT NOT NULL,
                                lesson_idx INTEGER NOT NULL,
                                command_id TEXT NOT NULL,
                                command_text TEXT NOT NULL,
                                success INTEGER NOT NULL,
                                exit_code INTEGER,
                                stdout TEXT,
                                stderr TEXT,
                                duration_ms INTEGER,
                                executed_at TEXT DEFAULT (datetime('now'))
                            );
                            CREATE INDEX IF NOT EXISTS idx_cmd_log_module
                                ON command_execution_log(module_id);",
                    )
                })?;
            }

            // Phase 15 migration: Suns Infrastructure
            if current_version < 15 {
                Self::run_versioned_migration(
                    &conn,
                    14,
                    15,
                    "Phase 15: suns infrastructure",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS sun_runs (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                sun_id TEXT NOT NULL,
                                module_id TEXT NOT NULL,
                                success INTEGER NOT NULL,
                                result_message TEXT,
                                data_json TEXT,
                                duration_ms INTEGER,
                                created_at TEXT DEFAULT (datetime('now'))
                            );
                            CREATE INDEX IF NOT EXISTS idx_sun_runs_id
                                ON sun_runs(sun_id);
                            CREATE INDEX IF NOT EXISTS idx_sun_runs_created
                                ON sun_runs(created_at);

                            CREATE TABLE IF NOT EXISTS sun_alerts (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                sun_id TEXT NOT NULL,
                                alert_type TEXT NOT NULL,
                                message TEXT NOT NULL,
                                acknowledged INTEGER NOT NULL DEFAULT 0,
                                created_at TEXT DEFAULT (datetime('now'))
                            );
                            CREATE INDEX IF NOT EXISTS idx_sun_alerts_ack
                                ON sun_alerts(acknowledged);",
                        )
                    },
                )?;
            }

            info!(target: "4da::db", "Database schema initialized with sqlite-vec");
            return Ok(());
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

    /// Phase 3 migration: Embedding status tracking for retry
    fn migrate_to_phase_3(conn: &Connection) -> SqliteResult<()> {
        // Add embedding_status column to track pending/complete/failed embeddings
        let has_status: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('source_items') WHERE name='embedding_status'",
                [],
                |row| row.get::<_, i64>(0).map(|count| count > 0),
            )
            .unwrap_or(false);

        if !has_status {
            conn.execute_batch(
                "
                ALTER TABLE source_items ADD COLUMN embedding_status TEXT DEFAULT 'complete';
                ALTER TABLE source_items ADD COLUMN embed_text TEXT DEFAULT NULL;
                CREATE INDEX IF NOT EXISTS idx_source_embedding_status ON source_items(embedding_status);
                ",
            )?;
            info!("Added embedding_status and embed_text columns to source_items");
        }

        Ok(())
    }

    /// Phase 5 migration: Innovation features infrastructure
    fn migrate_to_phase_5(conn: &Connection) -> SqliteResult<()> {
        // Temporal event store for tracking events across features
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS temporal_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                data JSON NOT NULL,
                embedding BLOB,
                source_item_id INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_temporal_type_time ON temporal_events(event_type, created_at);
            CREATE INDEX IF NOT EXISTS idx_temporal_subject ON temporal_events(subject);
            CREATE INDEX IF NOT EXISTS idx_temporal_expires ON temporal_events(expires_at);
        ",
        )?;
        info!(target: "4da::db", "Created temporal_events table");

        // Project dependencies for enhanced tracking
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS project_dependencies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                manifest_type TEXT NOT NULL,
                package_name TEXT NOT NULL,
                version TEXT,
                is_dev INTEGER DEFAULT 0,
                language TEXT NOT NULL,
                last_scanned TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(project_path, package_name)
            );
            CREATE INDEX IF NOT EXISTS idx_deps_package ON project_dependencies(package_name);
            CREATE INDEX IF NOT EXISTS idx_deps_project ON project_dependencies(project_path);
        ",
        )?;
        info!(target: "4da::db", "Created project_dependencies table");

        // Cross-item relationships for signal chains, diffs, mentions
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS item_relationships (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER NOT NULL,
                related_item_id INTEGER NOT NULL,
                relationship_type TEXT NOT NULL,
                strength REAL DEFAULT 1.0,
                metadata JSON,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(source_item_id, related_item_id, relationship_type)
            );
            CREATE INDEX IF NOT EXISTS idx_rel_source ON item_relationships(source_item_id);
            CREATE INDEX IF NOT EXISTS idx_rel_related ON item_relationships(related_item_id);
            CREATE INDEX IF NOT EXISTS idx_rel_type ON item_relationships(relationship_type);
        ",
        )?;
        info!(target: "4da::db", "Created item_relationships table");

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
    #[allow(dead_code)]
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
                created_at: parse_datetime(row.get::<_, String>(8)?),
                last_seen: parse_datetime(row.get::<_, String>(9)?),
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

    /// Batch upsert source items in a transaction (much faster than individual calls)
    #[allow(clippy::type_complexity)]
    pub fn batch_upsert_source_items(
        &self,
        items: &[(String, String, Option<String>, String, String, Vec<f32>)],
    ) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let mut count = 0;
        let tx = conn.unchecked_transaction()?;
        {
            // Prepare statements once for reuse
            let mut check_stmt = tx.prepare_cached(
                "SELECT id FROM source_items WHERE source_type = ?1 AND source_id = ?2",
            )?;
            let mut update_stmt = tx.prepare_cached(
                "UPDATE source_items SET url = ?1, title = ?2, content = ?3, content_hash = ?4, embedding = ?5, last_seen = datetime('now') WHERE id = ?6",
            )?;
            let mut update_vec_stmt =
                tx.prepare_cached("UPDATE source_vec SET embedding = ?1 WHERE rowid = ?2")?;
            let mut insert_stmt = tx.prepare_cached(
                "INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, embedding, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
            )?;
            let mut insert_vec_stmt =
                tx.prepare_cached("INSERT INTO source_vec (rowid, embedding) VALUES (?1, ?2)")?;

            for (source_type, source_id, url, title, content, embedding) in items {
                let content_hash = hash_content(&format!("{}{}", title, content));
                let embedding_blob = embedding_to_blob(embedding);

                // Check if exists
                let existing_id: Option<i64> = check_stmt
                    .query_row(params![source_type, source_id], |row| row.get(0))
                    .ok();

                if let Some(id) = existing_id {
                    // Update existing
                    update_stmt.execute(params![
                        url.as_deref(),
                        title,
                        content,
                        content_hash,
                        embedding_blob,
                        id
                    ])?;
                    update_vec_stmt.execute(params![embedding_blob, id])?;
                } else {
                    // Insert new
                    insert_stmt.execute(params![
                        source_type,
                        source_id,
                        url.as_deref(),
                        title,
                        content,
                        content_hash,
                        embedding_blob
                    ])?;
                    let id = tx.last_insert_rowid();
                    insert_vec_stmt.execute(params![id, embedding_blob])?;
                }
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Batch upsert source items that failed embedding (stored as pending for retry)
    #[allow(clippy::type_complexity)]
    pub fn batch_upsert_pending_source_items(
        &self,
        items: &[(String, String, Option<String>, String, String, String)], // (source_type, source_id, url, title, content, embed_text)
    ) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let mut count = 0;
        let tx = conn.unchecked_transaction()?;
        {
            let mut check_stmt = tx.prepare_cached(
                "SELECT id FROM source_items WHERE source_type = ?1 AND source_id = ?2",
            )?;
            let mut update_stmt = tx.prepare_cached(
                "UPDATE source_items SET url = ?1, title = ?2, content = ?3, content_hash = ?4, embed_text = ?5, embedding_status = 'pending', last_seen = datetime('now') WHERE id = ?6",
            )?;
            let mut insert_stmt = tx.prepare_cached(
                "INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, embedding, embedding_status, embed_text, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, X'', 'pending', ?7, datetime('now'))",
            )?;

            for (source_type, source_id, url, title, content, embed_text) in items {
                let content_hash = hash_content(&format!("{}{}", title, content));

                let existing_id: Option<i64> = check_stmt
                    .query_row(params![source_type, source_id], |row| row.get(0))
                    .ok();

                if let Some(id) = existing_id {
                    update_stmt.execute(params![
                        url.as_deref(),
                        title,
                        content,
                        content_hash,
                        embed_text,
                        id
                    ])?;
                } else {
                    insert_stmt.execute(params![
                        source_type,
                        source_id,
                        url.as_deref(),
                        title,
                        content,
                        content_hash,
                        embed_text
                    ])?;
                    // Do NOT insert into source_vec - pending items have no valid embedding
                }
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    /// Get items with pending embeddings for retry
    pub fn get_pending_embedding_items(
        &self,
        limit: usize,
    ) -> SqliteResult<Vec<(i64, String, String, String)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, COALESCE(embed_text, title || ' ' || content)
             FROM source_items
             WHERE embedding_status = 'pending'
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        rows.collect()
    }

    /// Upgrade a pending item to complete after successful re-embedding
    pub fn upgrade_pending_to_complete(&self, id: i64, embedding: &[f32]) -> SqliteResult<()> {
        let conn = self.conn.lock();
        let embedding_blob = embedding_to_blob(embedding);

        conn.execute(
            "UPDATE source_items SET embedding = ?1, embedding_status = 'complete', embed_text = NULL WHERE id = ?2",
            params![embedding_blob, id],
        )?;

        // Also insert into source_vec for vector search
        conn.execute(
            "INSERT OR REPLACE INTO source_vec (rowid, embedding) VALUES (?1, ?2)",
            params![id, embedding_blob],
        )?;

        Ok(())
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
             WHERE source_type = ?1 AND source_id = ?2
             AND (embedding_status IS NULL OR embedding_status = 'complete')"
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
                created_at: parse_datetime(row.get::<_, String>(8)?),
                last_seen: parse_datetime(row.get::<_, String>(9)?),
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
                created_at: parse_datetime(row.get::<_, String>(8)?),
                last_seen: parse_datetime(row.get::<_, String>(9)?),
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
        // Use strftime to compute cutoff as ISO string, then compare directly
        // This allows the index on last_seen to be used (no function wrapping the column)
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(hours);
        let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S").to_string();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen
             FROM source_items
             WHERE last_seen >= ?1
             ORDER BY last_seen DESC
             LIMIT ?2"
        )?;

        let hours_param = cutoff_str;
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

    /// Get items added since a specific ISO timestamp (for differential analysis)
    pub fn get_items_since_timestamp(
        &self,
        since: &str,
        limit: usize,
    ) -> SqliteResult<Vec<StoredSourceItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, url, title, content, content_hash, embedding, created_at, last_seen
             FROM source_items
             WHERE last_seen > ?1
             ORDER BY last_seen DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![since, limit as i64], |row| {
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

    /// Check if a specific source is enabled (defaults to true if not in DB)
    pub fn is_source_enabled(&self, source_type: &str) -> bool {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT enabled FROM sources WHERE source_type = ?1",
            params![source_type],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v != 0)
        .unwrap_or(true) // Default to enabled if not in DB
    }

    /// Toggle source enabled/disabled
    pub fn toggle_source_enabled(&self, source_type: &str, enabled: bool) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE sources SET enabled = ?1 WHERE source_type = ?2",
            params![enabled as i64, source_type],
        )?;
        Ok(())
    }

    /// Get all sources with their enabled status
    pub fn get_all_sources(&self) -> SqliteResult<Vec<SourceInfo>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT source_type, name, enabled, last_fetch FROM sources ORDER BY source_type",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        rows.collect()
    }

    /// Get last fetch time for a source
    pub fn get_source_last_fetch(&self, source_type: &str) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT last_fetch FROM sources WHERE source_type = ?1",
            params![source_type],
            |row| row.get(0),
        )
        .optional()
        .map(|v| v.flatten())
    }

    // DB maintenance operations

    /// Run database maintenance: cleanup old items, optimize, vacuum
    pub fn run_maintenance(&self, retention_days: i64) -> SqliteResult<MaintenanceResult> {
        let conn = self.conn.lock();

        let deleted_items: usize = conn.execute(
            "DELETE FROM source_items WHERE last_seen < datetime('now', ?1)",
            params![format!("-{} days", retention_days)],
        )?;

        let deleted_feedback: usize = conn.execute(
            "DELETE FROM feedback WHERE created_at < datetime('now', ?1)",
            params![format!("-{} days", retention_days * 2)],
        )?;

        // Clean void_positions (rebuild on next use)
        let deleted_void: usize = conn.execute("DELETE FROM void_positions", []).unwrap_or(0);

        conn.execute_batch("PRAGMA optimize;")?;
        conn.execute_batch("VACUUM;")?;

        Ok(MaintenanceResult {
            deleted_items,
            deleted_feedback,
            deleted_void,
        })
    }

    /// Get database statistics
    pub fn get_db_stats(&self) -> SqliteResult<DbStats> {
        let conn = self.conn.lock();

        let source_items: i64 = conn
            .query_row("SELECT COUNT(*) FROM source_items", [], |row| row.get(0))
            .unwrap_or(0);

        let context_chunks: i64 = conn
            .query_row("SELECT COUNT(*) FROM context_chunks", [], |row| row.get(0))
            .unwrap_or(0);

        let feedback_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM feedback", [], |row| row.get(0))
            .unwrap_or(0);

        let sources_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0))
            .unwrap_or(0);

        Ok(DbStats {
            source_items,
            context_chunks,
            feedback_count,
            sources_count,
        })
    }

    /// Get the current schema version
    pub fn get_schema_version(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT version FROM schema_version", [], |row| row.get(0))
    }

    /// Get migration history records
    pub fn get_migration_history(&self) -> SqliteResult<Vec<MigrationHistoryEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, from_version, to_version, executed_at, duration_ms, success FROM migration_history ORDER BY id"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(MigrationHistoryEntry {
                id: row.get(0)?,
                from_version: row.get(1)?,
                to_version: row.get(2)?,
                executed_at: row.get(3)?,
                duration_ms: row.get(4)?,
                success: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    /// Record source health after a fetch
    pub fn record_source_health(
        &self,
        source_type: &str,
        success: bool,
        items_fetched: i64,
        response_time_ms: i64,
        error_msg: Option<&str>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();

        if success {
            conn.execute(
                "INSERT INTO source_health (source_type, status, last_success, items_fetched, response_time_ms, consecutive_failures, checked_at)
                 VALUES (?1, 'healthy', datetime('now'), ?2, ?3, 0, datetime('now'))
                 ON CONFLICT(source_type) DO UPDATE SET
                   status = 'healthy', last_success = datetime('now'),
                   items_fetched = ?2, response_time_ms = ?3,
                   consecutive_failures = 0, checked_at = datetime('now')",
                params![source_type, items_fetched, response_time_ms],
            )?;
        } else {
            conn.execute(
                "INSERT INTO source_health (source_type, status, last_error, error_count, consecutive_failures, checked_at)
                 VALUES (?1, 'error', ?2, 1, 1, datetime('now'))
                 ON CONFLICT(source_type) DO UPDATE SET
                   status = CASE WHEN consecutive_failures + 1 >= 5 THEN 'circuit_open' ELSE 'error' END,
                   last_error = ?2,
                   error_count = error_count + 1,
                   consecutive_failures = consecutive_failures + 1,
                   checked_at = datetime('now')",
                params![source_type, error_msg.unwrap_or("Unknown error")],
            )?;
        }

        Ok(())
    }

    /// Get source health for all sources
    pub fn get_source_health(&self) -> SqliteResult<Vec<SourceHealthRecord>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT source_type, status, last_success, last_error, error_count,
                    consecutive_failures, items_fetched, response_time_ms, checked_at
             FROM source_health ORDER BY source_type",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(SourceHealthRecord {
                source_type: row.get(0)?,
                status: row.get(1)?,
                last_success: row.get(2)?,
                last_error: row.get(3)?,
                error_count: row.get(4)?,
                consecutive_failures: row.get(5)?,
                items_fetched: row.get(6)?,
                response_time_ms: row.get(7)?,
                checked_at: row.get(8)?,
            })
        })?;

        rows.collect()
    }

    /// Check if circuit breaker is open for a source (5+ consecutive failures).
    /// Auto-resets after 10 minutes cooldown to allow retry after transient outages.
    pub fn is_circuit_open(&self, source_type: &str) -> bool {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT consecutive_failures, checked_at FROM source_health WHERE source_type = ?1",
            params![source_type],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        );
        match result {
            Ok((failures, checked_at)) if failures >= 5 => {
                // Auto-reset after 10-minute cooldown
                let stale = conn
                    .query_row(
                        "SELECT datetime(?1, '+10 minutes') <= datetime('now')",
                        params![checked_at],
                        |row| row.get::<_, bool>(0),
                    )
                    .unwrap_or(false);
                if stale {
                    let _ = conn.execute(
                        "UPDATE source_health SET consecutive_failures = 0, status = 'error' WHERE source_type = ?1",
                        params![source_type],
                    );
                    tracing::info!(target: "4da::health", source = source_type, "Circuit breaker auto-reset after cooldown");
                    false
                } else {
                    true
                }
            }
            _ => false,
        }
    }

    /// Get feedback summary aggregated by topic for scoring boost
    pub fn get_feedback_topic_summary(&self) -> SqliteResult<Vec<FeedbackTopicSummary>> {
        let conn = self.conn.lock();

        // Join feedback with source_items to get titles for topic extraction
        let mut stmt = conn.prepare(
            "SELECT si.title, f.relevant
             FROM feedback f
             JOIN source_items si ON f.source_item_id = si.id
             WHERE f.created_at > datetime('now', '-30 days')
             ORDER BY f.created_at DESC
             LIMIT 500",
        )?;

        let rows: Vec<(String, bool)> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Aggregate by extracted topics (simple word-based)
        let mut topic_map: std::collections::HashMap<String, (i64, i64)> =
            std::collections::HashMap::new();
        for (title, relevant) in &rows {
            let words: Vec<String> = title
                .to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .filter(|w| {
                    ![
                        "the", "and", "for", "with", "that", "this", "from", "have", "been",
                        "will", "what", "when", "where", "which", "about", "into", "your", "more",
                        "some", "show",
                    ]
                    .contains(w)
                })
                .map(|s| s.to_string())
                .collect();

            for word in words.into_iter().take(5) {
                let entry = topic_map.entry(word).or_insert((0, 0));
                if *relevant {
                    entry.0 += 1; // saves
                } else {
                    entry.1 += 1; // dismissals
                }
            }
        }

        let mut summaries: Vec<FeedbackTopicSummary> = topic_map
            .into_iter()
            .filter(|(_, (saves, dismissals))| saves + dismissals >= 2)
            .map(|(topic, (saves, dismissals))| FeedbackTopicSummary {
                topic,
                saves,
                dismissals,
                net_score: (saves as f64 - dismissals as f64) / (saves + dismissals) as f64,
            })
            .collect();

        summaries.sort_by(|a, b| {
            b.net_score
                .partial_cmp(&a.net_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(summaries)
    }

    /// Count total feedback interactions (used for bootstrap mode detection)
    pub fn query_feedback_count(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM feedback", [], |row| row.get(0))
    }
}

// ============================================================================
// Maintenance & Health Types
// ============================================================================

#[derive(Debug, Clone, serde::Serialize)]
pub struct MaintenanceResult {
    pub deleted_items: usize,
    pub deleted_feedback: usize,
    pub deleted_void: usize,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct DbStats {
    pub source_items: i64,
    pub context_chunks: i64,
    pub feedback_count: i64,
    pub sources_count: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MigrationHistoryEntry {
    pub id: i64,
    pub from_version: i64,
    pub to_version: i64,
    pub executed_at: String,
    pub duration_ms: i64,
    pub success: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceHealthRecord {
    pub source_type: String,
    pub status: String,
    pub last_success: Option<String>,
    pub last_error: Option<String>,
    pub error_count: i64,
    pub consecutive_failures: i64,
    pub items_fetched: i64,
    pub response_time_ms: i64,
    pub checked_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FeedbackTopicSummary {
    pub topic: String,
    pub saves: i64,
    pub dismissals: i64,
    pub net_score: f64,
}

// ============================================================================
// LLM Content Retrieval
// ============================================================================

impl Database {
    /// Get first N chars of content for an item (for LLM judging)
    pub fn get_item_content_snippet(
        &self,
        item_id: i64,
        max_chars: usize,
    ) -> Result<String, String> {
        let conn = self.conn.lock();
        let content: String = conn
            .query_row(
                "SELECT COALESCE(content, '') FROM source_items WHERE id = ?1",
                params![item_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("Failed to get content for item {}: {}", item_id, e))?;

        if content.len() <= max_chars {
            Ok(content)
        } else {
            // Truncate at char boundary
            let truncated: String = content.chars().take(max_chars).collect();
            Ok(truncated)
        }
    }

    /// Get full content + source_type for an item. Returns (content, source_type, char_count).
    pub fn get_item_content(
        &self,
        item_id: i64,
    ) -> Result<Option<(String, String, usize)>, String> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COALESCE(content, ''), source_type FROM source_items WHERE id = ?1",
            params![item_id],
            |row| {
                let content: String = row.get(0)?;
                let source_type: String = row.get(1)?;
                let char_count = content.len();
                Ok((content, source_type, char_count))
            },
        )
        .optional()
        .map_err(|e| format!("Failed to get item content: {}", e))
    }

    /// Get cached AI summary for an item.
    pub fn get_item_summary(&self, item_id: i64) -> Result<Option<String>, String> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT summary FROM source_items WHERE id = ?1",
            params![item_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map(|opt| opt.flatten())
        .map_err(|e| format!("Failed to get item summary: {}", e))
    }

    /// Cache an AI summary for an item.
    pub fn set_item_summary(&self, item_id: i64, summary: &str) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE source_items SET summary = ?1 WHERE id = ?2",
            params![summary, item_id],
        )
        .map_err(|e| format!("Failed to set item summary: {}", e))?;
        Ok(())
    }

    /// Get title for a source item.
    pub fn get_item_title(&self, item_id: i64) -> Result<Option<String>, String> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT title FROM source_items WHERE id = ?1",
            params![item_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Failed to get item title: {}", e))
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
             WHERE created_at >= ?1
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
    #[allow(clippy::type_complexity)]
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
    #[allow(dead_code, clippy::type_complexity)]
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
    #[allow(dead_code, clippy::type_complexity)]
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

// ============================================================================
// Briefing Persistence
// ============================================================================

impl Database {
    /// Save a briefing to the database, pruning to keep only the last 10.
    pub fn save_briefing(
        &self,
        content: &str,
        model: Option<&str>,
        item_count: usize,
        tokens_used: Option<u64>,
        latency_ms: Option<u64>,
    ) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO briefings (content, model, item_count, tokens_used, latency_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                content,
                model,
                item_count as i64,
                tokens_used.map(|v| v as i64),
                latency_ms.map(|v| v as i64),
            ],
        )?;
        let id = conn.last_insert_rowid();

        // Prune to keep only the 10 most recent
        conn.execute(
            "DELETE FROM briefings WHERE id NOT IN (
                SELECT id FROM briefings ORDER BY created_at DESC LIMIT 10
            )",
            [],
        )?;

        Ok(id)
    }

    /// Get the most recent briefing.
    /// Returns (content, model, item_count, created_at).
    #[allow(clippy::type_complexity)]
    pub fn get_latest_briefing(
        &self,
    ) -> SqliteResult<Option<(String, Option<String>, i64, String)>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT content, model, item_count, created_at
             FROM briefings ORDER BY created_at DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
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
            let arr: [u8; 4] = chunk.try_into().unwrap_or([0u8; 4]);
            f32::from_le_bytes(arr)
        })
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

// ============================================================================
// Command History
// ============================================================================

impl Database {
    /// Save a command to history and auto-prune to max entries.
    pub fn save_command_history(
        &self,
        command: &str,
        working_dir: &str,
        exit_code: i32,
        success: bool,
        output_preview: Option<&str>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO command_history (command, working_dir, exit_code, success, output_preview)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                command,
                working_dir,
                exit_code,
                success as i32,
                output_preview
            ],
        )?;
        // Auto-prune to 200 entries
        conn.execute(
            "DELETE FROM command_history WHERE id NOT IN (
                SELECT id FROM command_history ORDER BY created_at DESC LIMIT 200
            )",
            [],
        )?;
        Ok(())
    }

    /// Get recent command history entries.
    pub fn get_command_history(&self, limit: u32) -> SqliteResult<Vec<CommandHistoryRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, command, working_dir, exit_code, success, output_preview, created_at
             FROM command_history
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let entries = stmt
            .query_map([limit], |row: &rusqlite::Row| {
                Ok(CommandHistoryRow {
                    id: row.get(0)?,
                    command: row.get(1)?,
                    working_dir: row.get(2)?,
                    exit_code: row.get(3)?,
                    success: row.get::<_, i64>(4).map(|v| v != 0)?,
                    output_preview: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;
        Ok(entries)
    }
}

/// Row from command_history table
#[derive(Debug, Clone)]
pub struct CommandHistoryRow {
    pub id: i64,
    pub command: String,
    pub working_dir: String,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub output_preview: Option<String>,
    pub created_at: String,
}

// ============================================================================
// Toolkit HTTP History
// ============================================================================

impl Database {
    /// Save an HTTP request to history.
    pub fn save_http_history(
        &self,
        method: &str,
        url: &str,
        status: u16,
        duration_ms: u64,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO toolkit_http_history (method, url, status, duration_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![method, url, status as u32, duration_ms],
        )?;
        conn.execute(
            "DELETE FROM toolkit_http_history WHERE id NOT IN (
                SELECT id FROM toolkit_http_history ORDER BY created_at DESC LIMIT 200
            )",
            [],
        )?;
        Ok(())
    }

    /// Get recent HTTP history entries.
    pub fn get_http_history(&self, limit: u32) -> SqliteResult<Vec<HttpHistoryRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, method, url, status, duration_ms, created_at
             FROM toolkit_http_history
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let entries = stmt
            .query_map([limit], |row: &rusqlite::Row| {
                Ok(HttpHistoryRow {
                    id: row.get(0)?,
                    method: row.get(1)?,
                    url: row.get(2)?,
                    status: row.get::<_, u32>(3).map(|v| v as u16)?,
                    duration_ms: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;
        Ok(entries)
    }
}

/// Row from toolkit_http_history table
#[derive(Debug, Clone)]
pub struct HttpHistoryRow {
    pub id: i64,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub duration_ms: u64,
    pub created_at: String,
}

// ============================================================================
// Maintenance & Diagnostics
// ============================================================================

impl Database {
    /// Delete source_items older than the given number of days.
    /// Returns the number of rows deleted.
    pub fn cleanup_old_items(&self, max_age_days: u32) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM source_items WHERE last_seen < datetime('now', ?1)",
            params![format!("-{} days", max_age_days)],
        )?;
        // Also clean up orphaned feedback records
        let _ = conn.execute(
            "DELETE FROM feedback WHERE source_item_id NOT IN (SELECT id FROM source_items)",
            [],
        );
        // Clean up orphaned vec entries
        let _ = conn.execute(
            "DELETE FROM source_vec WHERE rowid NOT IN (SELECT id FROM source_items)",
            [],
        );
        Ok(deleted)
    }

    /// Run VACUUM if more than threshold rows were deleted.
    /// VACUUM reclaims disk space and defragments the database.
    pub fn vacuum_if_needed(&self, deleted_count: usize, threshold: usize) -> SqliteResult<()> {
        if deleted_count >= threshold {
            let conn = self.conn.lock();
            info!(target: "4da::db", deleted_count, "Running VACUUM after large cleanup");
            conn.execute_batch("VACUUM")?;
        }
        Ok(())
    }

    /// Get source health summary: (source_type, status, consecutive_failures)
    pub fn get_source_health_summary(&self) -> SqliteResult<Vec<(String, String, i64)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT source_type, status, consecutive_failures FROM source_health ORDER BY source_type",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        rows.collect()
    }

    /// Get the database file path
    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }
}

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
