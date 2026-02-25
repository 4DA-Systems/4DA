#![allow(dead_code)]
//! Database migrations — schema versioning, backup, and migration orchestration.

use rusqlite::{params, Connection, Result as SqliteResult};
use std::path::PathBuf;
use tracing::info;

use super::Database;

impl Database {
    /// Create a pre-migration backup of the database file.
    /// Keeps only the last 2 backups to avoid disk bloat.
    pub(crate) fn backup_before_migration(&self, current_version: i64) {
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
    pub(crate) fn run_versioned_migration(
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
    pub(crate) fn migrate(&self) -> SqliteResult<()> {
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

        const TARGET_VERSION: i64 = 17;
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

            // Phase 16 migration: STREETS Coach
            if current_version < 16 {
                Self::run_versioned_migration(&conn, 15, 16, "Phase 16: STREETS Coach", |c| {
                    c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS coach_sessions (
                                id TEXT PRIMARY KEY,
                                session_type TEXT NOT NULL,
                                title TEXT NOT NULL DEFAULT 'New Session',
                                context_snapshot TEXT,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );
                            CREATE INDEX IF NOT EXISTS idx_coach_sessions_type
                                ON coach_sessions(session_type);
                            CREATE INDEX IF NOT EXISTS idx_coach_sessions_updated
                                ON coach_sessions(updated_at);

                            CREATE TABLE IF NOT EXISTS coach_messages (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                session_id TEXT NOT NULL REFERENCES coach_sessions(id) ON DELETE CASCADE,
                                role TEXT NOT NULL,
                                content TEXT NOT NULL,
                                token_count INTEGER DEFAULT 0,
                                cost_cents INTEGER DEFAULT 0,
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );
                            CREATE INDEX IF NOT EXISTS idx_coach_messages_session
                                ON coach_messages(session_id);

                            CREATE TABLE IF NOT EXISTS coach_documents (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                document_type TEXT NOT NULL,
                                title TEXT NOT NULL,
                                content TEXT NOT NULL,
                                metadata TEXT DEFAULT '{}',
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );

                            CREATE TABLE IF NOT EXISTS coach_nudges (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                nudge_type TEXT NOT NULL,
                                content TEXT NOT NULL,
                                dismissed INTEGER DEFAULT 0,
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );
                            CREATE INDEX IF NOT EXISTS idx_coach_nudges_dismissed
                                ON coach_nudges(dismissed);

                            CREATE TABLE IF NOT EXISTS video_curriculum (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                video_id TEXT NOT NULL UNIQUE,
                                title TEXT NOT NULL,
                                duration_seconds INTEGER DEFAULT 0,
                                drip_day INTEGER NOT NULL,
                                watched INTEGER DEFAULT 0,
                                watch_progress_seconds INTEGER DEFAULT 0,
                                unlocked_at TEXT,
                                watched_at TEXT
                            );
                            CREATE INDEX IF NOT EXISTS idx_video_curriculum_video
                                ON video_curriculum(video_id);",
                        )
                })?;
            }

            // Phase 17 migration: Intelligence Metabolism (Autophagy + Decision Advantage)
            if current_version < 17 {
                Self::run_versioned_migration(
                    &conn,
                    16,
                    17,
                    "Phase 17: intelligence metabolism",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS digested_intelligence (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                digest_type TEXT NOT NULL,
                                subject TEXT NOT NULL,
                                data TEXT NOT NULL,
                                confidence REAL NOT NULL DEFAULT 0.5,
                                sample_size INTEGER NOT NULL DEFAULT 0,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                expires_at TEXT,
                                superseded_by INTEGER,
                                FOREIGN KEY (superseded_by) REFERENCES digested_intelligence(id)
                            );
                            CREATE INDEX IF NOT EXISTS idx_digest_type_subject
                                ON digested_intelligence(digest_type, subject);
                            CREATE INDEX IF NOT EXISTS idx_digest_created
                                ON digested_intelligence(created_at);

                            CREATE TABLE IF NOT EXISTS autophagy_cycles (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                items_analyzed INTEGER NOT NULL DEFAULT 0,
                                items_pruned INTEGER NOT NULL DEFAULT 0,
                                calibrations_produced INTEGER NOT NULL DEFAULT 0,
                                topic_decay_rates_updated INTEGER NOT NULL DEFAULT 0,
                                source_autopsies_produced INTEGER NOT NULL DEFAULT 0,
                                anti_patterns_detected INTEGER NOT NULL DEFAULT 0,
                                db_size_before_bytes INTEGER NOT NULL DEFAULT 0,
                                db_size_after_bytes INTEGER NOT NULL DEFAULT 0,
                                duration_ms INTEGER NOT NULL DEFAULT 0,
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );

                            CREATE TABLE IF NOT EXISTS decision_windows (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                window_type TEXT NOT NULL,
                                title TEXT NOT NULL,
                                description TEXT NOT NULL DEFAULT '',
                                urgency REAL NOT NULL DEFAULT 0.5,
                                relevance REAL NOT NULL DEFAULT 0.5,
                                source_item_ids TEXT NOT NULL DEFAULT '[]',
                                signal_chain_id INTEGER,
                                dependency TEXT,
                                status TEXT NOT NULL DEFAULT 'open',
                                opened_at TEXT NOT NULL DEFAULT (datetime('now')),
                                expires_at TEXT,
                                acted_at TEXT,
                                closed_at TEXT,
                                outcome TEXT,
                                lead_time_hours REAL,
                                streets_engine TEXT
                            );
                            CREATE INDEX IF NOT EXISTS idx_dw_status ON decision_windows(status);
                            CREATE INDEX IF NOT EXISTS idx_dw_type ON decision_windows(window_type);
                            CREATE INDEX IF NOT EXISTS idx_dw_dependency ON decision_windows(dependency);

                            CREATE TABLE IF NOT EXISTS advantage_score (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                period TEXT NOT NULL,
                                score REAL NOT NULL DEFAULT 0.0,
                                items_surfaced INTEGER NOT NULL DEFAULT 0,
                                avg_lead_time_hours REAL NOT NULL DEFAULT 0.0,
                                windows_opened INTEGER NOT NULL DEFAULT 0,
                                windows_acted INTEGER NOT NULL DEFAULT 0,
                                windows_expired INTEGER NOT NULL DEFAULT 0,
                                knowledge_gaps_closed INTEGER NOT NULL DEFAULT 0,
                                calibration_accuracy REAL NOT NULL DEFAULT 0.0,
                                computed_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );
                            CREATE INDEX IF NOT EXISTS idx_advantage_period
                                ON advantage_score(period, computed_at);",
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
}
