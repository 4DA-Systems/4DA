//! ACE Database Schema and Migrations
//!
//! Implements the full ACE database schema as specified in the stone tablet.

use parking_lot::Mutex;
use rusqlite::Connection;
use std::sync::Arc;
use tracing::info;

use crate::error::{Result, ResultExt};

/// Run all ACE database migrations
pub fn migrate(conn: &Arc<Mutex<Connection>>) -> Result<()> {
    let conn = conn.lock();

    conn.execute_batch(
        r#"
        -- Enable WAL mode for better concurrency (prevents "database is locked" errors)
        PRAGMA journal_mode = WAL;
        PRAGMA busy_timeout = 5000;
        PRAGMA synchronous = NORMAL;
        PRAGMA cache_size = -4000;
        PRAGMA mmap_size = 268435456;
        PRAGMA temp_store = MEMORY;

        -- ═══════════════════════════════════════════════════════════════
        -- SIGNAL ACQUISITION TABLES
        -- ═══════════════════════════════════════════════════════════════

        -- Detected projects from manifest scanning
        CREATE TABLE IF NOT EXISTS detected_projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            languages TEXT,                -- JSON array
            frameworks TEXT,               -- JSON array
            dependencies TEXT,             -- JSON array
            last_activity TEXT,
            detection_confidence REAL DEFAULT 0.5,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );

        -- Detected technologies (merged from all sources)
        CREATE TABLE IF NOT EXISTS detected_tech (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            category TEXT NOT NULL,        -- 'language', 'framework', 'library', etc.
            confidence REAL DEFAULT 0.5,
            source TEXT NOT NULL,          -- 'manifest', 'file_extension', etc.
            evidence TEXT,                 -- Semicolon-separated evidence strings
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_detected_tech_name ON detected_tech(name);
        CREATE INDEX IF NOT EXISTS idx_detected_tech_confidence ON detected_tech(confidence);

        -- File change signals
        CREATE TABLE IF NOT EXISTS file_signals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            change_type TEXT NOT NULL,     -- 'created', 'modified', 'deleted'
            extracted_topics TEXT,         -- JSON array
            content_hash TEXT,
            timestamp TEXT DEFAULT (datetime('now')),
            processed INTEGER DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_file_signals_timestamp ON file_signals(timestamp);
        CREATE INDEX IF NOT EXISTS idx_file_signals_processed ON file_signals(processed);

        -- Git signals
        CREATE TABLE IF NOT EXISTS git_signals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            repo_path TEXT NOT NULL,
            commit_hash TEXT,
            commit_message TEXT,
            extracted_topics TEXT,         -- JSON array
            files_changed TEXT,            -- JSON array
            timestamp TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_git_signals_timestamp ON git_signals(timestamp);
        CREATE INDEX IF NOT EXISTS idx_git_signals_repo ON git_signals(repo_path);

        -- ═══════════════════════════════════════════════════════════════
        -- ACTIVE CONTEXT TABLES
        -- ═══════════════════════════════════════════════════════════════

        -- Active topics (derived from current work)
        CREATE TABLE IF NOT EXISTS active_topics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            topic TEXT NOT NULL UNIQUE,
            weight REAL DEFAULT 0.5,
            confidence REAL DEFAULT 0.5,
            embedding BLOB,
            source TEXT NOT NULL,          -- 'file_content', 'git_commit', etc.
            last_seen TEXT DEFAULT (datetime('now')),
            decay_applied INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_active_topics_topic ON active_topics(topic);
        CREATE INDEX IF NOT EXISTS idx_active_topics_last_seen ON active_topics(last_seen);

        -- ═══════════════════════════════════════════════════════════════
        -- LEARNED BEHAVIOR TABLES (extensions to existing)
        -- ═══════════════════════════════════════════════════════════════

        -- Anti-topics (learned exclusions from behavior)
        CREATE TABLE IF NOT EXISTS anti_topics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            topic TEXT NOT NULL UNIQUE,
            rejection_count INTEGER DEFAULT 0,
            confidence REAL DEFAULT 0.0,
            auto_detected INTEGER DEFAULT 1,
            user_confirmed INTEGER DEFAULT 0,
            first_rejection TEXT DEFAULT (datetime('now')),
            last_rejection TEXT DEFAULT (datetime('now')),
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_anti_topics_topic ON anti_topics(topic);

        -- User interactions (behavior signals)
        CREATE TABLE IF NOT EXISTS interactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            item_id INTEGER NOT NULL,
            action_type TEXT NOT NULL,          -- 'click', 'save', 'share', 'dismiss', etc.
            action_data TEXT,                   -- JSON with action-specific data (dwell_time, etc.)
            item_topics TEXT,                   -- JSON array
            item_source TEXT,                   -- 'hackernews', 'arxiv', etc.
            signal_strength REAL NOT NULL,
            timestamp TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_interactions_timestamp ON interactions(timestamp);
        CREATE INDEX IF NOT EXISTS idx_interactions_item ON interactions(item_id);
        CREATE INDEX IF NOT EXISTS idx_interactions_source ON interactions(item_source);
        CREATE INDEX IF NOT EXISTS idx_interactions_item_action ON interactions(item_id, action_type);

        -- Topic affinities (learned preferences)
        CREATE TABLE IF NOT EXISTS topic_affinities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            topic TEXT NOT NULL UNIQUE,
            embedding BLOB,
            positive_signals INTEGER DEFAULT 0,
            negative_signals INTEGER DEFAULT 0,
            total_exposures INTEGER DEFAULT 0,
            affinity_score REAL DEFAULT 0.0,
            confidence REAL DEFAULT 0.0,
            last_interaction TEXT DEFAULT (datetime('now')),
            decay_applied INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_topic_affinities_topic ON topic_affinities(topic);
        CREATE INDEX IF NOT EXISTS idx_topic_affinities_score ON topic_affinities(affinity_score);
        CREATE INDEX IF NOT EXISTS idx_topic_affinities_last_interaction ON topic_affinities(last_interaction);

        -- Source preferences (learned from behavior)
        CREATE TABLE IF NOT EXISTS source_preferences (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL UNIQUE,
            score REAL DEFAULT 0.0,
            interactions INTEGER DEFAULT 0,
            last_interaction TEXT DEFAULT (datetime('now')),
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_source_preferences_source ON source_preferences(source);

        -- Activity patterns (time-based engagement, keyed by type + slot)
        CREATE TABLE IF NOT EXISTS activity_patterns (
            pattern_type TEXT NOT NULL,
            pattern_key TEXT NOT NULL,
            interaction_count INTEGER DEFAULT 0,
            last_updated TEXT,
            PRIMARY KEY (pattern_type, pattern_key)
        );

        -- ═══════════════════════════════════════════════════════════════
        -- VALIDATION & MONITORING TABLES
        -- ═══════════════════════════════════════════════════════════════

        -- Signal validation records
        CREATE TABLE IF NOT EXISTS validated_signals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            signal_type TEXT NOT NULL,
            signal_data TEXT NOT NULL,     -- JSON
            confidence REAL NOT NULL,
            evidence_sources TEXT,         -- JSON array
            contradictions TEXT,           -- JSON array
            freshness REAL,
            timestamp TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_validated_signals_type ON validated_signals(signal_type);
        CREATE INDEX IF NOT EXISTS idx_validated_signals_timestamp ON validated_signals(timestamp);

        -- Audit trail
        CREATE TABLE IF NOT EXISTS audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entry_type TEXT NOT NULL,
            action TEXT NOT NULL,
            reason TEXT,
            contributing_factors TEXT,     -- JSON array
            before_state TEXT,
            after_state TEXT,
            confidence REAL,
            timestamp TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(timestamp);
        CREATE INDEX IF NOT EXISTS idx_audit_log_type ON audit_log(entry_type);

        -- Accuracy metrics (daily snapshots)
        CREATE TABLE IF NOT EXISTS accuracy_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            metric_date TEXT NOT NULL UNIQUE,
            precision_score REAL,
            recall_score REAL,
            engagement_rate REAL,
            items_shown INTEGER DEFAULT 0,
            items_clicked INTEGER DEFAULT 0,
            positive_feedback INTEGER DEFAULT 0,
            negative_feedback INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_accuracy_metrics_date ON accuracy_metrics(metric_date);

        -- System health records
        CREATE TABLE IF NOT EXISTS system_health (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            component TEXT NOT NULL,
            status TEXT NOT NULL,          -- 'healthy', 'degraded', 'failed', 'disabled'
            last_success TEXT,
            error_count INTEGER DEFAULT 0,
            last_error TEXT,
            checked_at TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_system_health_component ON system_health(component);

        -- ═══════════════════════════════════════════════════════════════
        -- COLD START BOOTSTRAP TABLE
        -- ═══════════════════════════════════════════════════════════════

        -- Common project paths to scan on first run
        CREATE TABLE IF NOT EXISTS bootstrap_paths (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            priority INTEGER DEFAULT 0,
            scanned INTEGER DEFAULT 0,
            last_scanned TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        );

        -- Insert default bootstrap paths if not exists
        INSERT OR IGNORE INTO bootstrap_paths (path, priority) VALUES
            ('~/projects', 10),
            ('~/code', 10),
            ('~/dev', 10),
            ('~/src', 10),
            ('~/Documents/GitHub', 8),
            ('~/repos', 8),
            ('~/workspace', 8),
            ('~/work', 7),
            ('~/.config', 3);

        -- ═══════════════════════════════════════════════════════════════
        -- DOCUMENT EXTRACTION TABLES
        -- ═══════════════════════════════════════════════════════════════

        -- Indexed documents (files that have been extracted)
        CREATE TABLE IF NOT EXISTS indexed_documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL UNIQUE,
            file_name TEXT NOT NULL,
            file_type TEXT NOT NULL,           -- 'pdf', 'docx', 'xlsx', 'zip', etc.
            file_size INTEGER,
            content_hash TEXT,
            word_count INTEGER DEFAULT 0,
            page_count INTEGER DEFAULT 0,
            extraction_confidence REAL DEFAULT 0.0,
            extracted_topics TEXT,             -- JSON array
            last_modified TEXT,
            indexed_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_indexed_documents_path ON indexed_documents(file_path);
        CREATE INDEX IF NOT EXISTS idx_indexed_documents_type ON indexed_documents(file_type);
        CREATE INDEX IF NOT EXISTS idx_indexed_documents_indexed ON indexed_documents(indexed_at);

        -- Document chunks (extracted text segments for search)
        CREATE TABLE IF NOT EXISTS document_chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            document_id INTEGER NOT NULL,
            chunk_index INTEGER NOT NULL,
            content TEXT NOT NULL,
            word_count INTEGER DEFAULT 0,
            embedding BLOB,                    -- 384-dim embedding for semantic search
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (document_id) REFERENCES indexed_documents(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_document_chunks_doc ON document_chunks(document_id);

    "#,
    )
    .context("ACE migration failed")?;

    // Phase 1B migration: Add last_decay_at column for continuous decay tracking
    // This replaces the boolean decay_applied flag with a timestamp
    conn.execute_batch("ALTER TABLE topic_affinities ADD COLUMN last_decay_at TEXT DEFAULT NULL;")
        .ok(); // ok() because column may already exist on subsequent runs

    // Phase 1C migration: Anomalies table
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS anomalies (
            id INTEGER PRIMARY KEY,
            anomaly_type TEXT NOT NULL,
            topic TEXT,
            description TEXT NOT NULL,
            confidence REAL DEFAULT 0.5,
            severity TEXT DEFAULT 'medium',
            evidence TEXT DEFAULT '[]',
            detected_at TEXT DEFAULT (datetime('now')),
            resolved INTEGER DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_anomalies_resolved ON anomalies(resolved);
        CREATE INDEX IF NOT EXISTS idx_anomalies_type ON anomalies(anomaly_type);
    "#,
    )
    .ok();

    // Migrate activity_patterns from singleton (JSON arrays) to keyed rows schema.
    // The old schema had columns: id, hourly_engagement, daily_engagement, total_tracked.
    // The new schema uses (pattern_type, pattern_key) as primary key with interaction_count.
    // Safe to drop because the old singleton table had no real user data (initialized to zeros).
    // Use PRAGMA table_info to detect old schema (avoids SQL checker flagging column names).
    {
        let has_old_schema: bool = conn
            .prepare("PRAGMA table_info(activity_patterns)")
            .map(|mut stmt| {
                let cols: Vec<String> = stmt
                    .query_map([], |row| row.get::<_, String>(1))
                    .map(|rows| {
                        rows.filter_map(|r| match r {
                            Ok(v) => Some(v),
                            Err(e) => {
                                tracing::warn!("Row processing failed in ace_db: {e}");
                                None
                            }
                        })
                        .collect()
                    })
                    .unwrap_or_default();
                cols.iter().any(|c| c == "hourly_engagement")
            })
            .unwrap_or(false);
        if has_old_schema {
            conn.execute_batch(
                "DROP TABLE IF EXISTS activity_patterns;
                 CREATE TABLE IF NOT EXISTS activity_patterns (
                     pattern_type TEXT NOT NULL,
                     pattern_key TEXT NOT NULL,
                     interaction_count INTEGER DEFAULT 0,
                     last_updated TEXT,
                     PRIMARY KEY (pattern_type, pattern_key)
                 );",
            )
            .ok();
        }
    }

    // Create vec0 virtual table for KNN search on topic embeddings (sqlite-vec)
    // This enables O(log n) semantic similarity search for topics
    conn.execute_batch(
        "
        -- Vector index for active topic embeddings (384-dim MiniLM embeddings)
        CREATE VIRTUAL TABLE IF NOT EXISTS topic_vec USING vec0(
            embedding float[384]
        );

        -- Vector index for topic affinity embeddings (384-dim MiniLM embeddings)
        CREATE VIRTUAL TABLE IF NOT EXISTS affinity_vec USING vec0(
            embedding float[384]
        );

        -- Vector index for document chunk embeddings (384-dim MiniLM embeddings)
        CREATE VIRTUAL TABLE IF NOT EXISTS document_vec USING vec0(
            embedding float[384]
        );
    ",
    )
    .context("Failed to create topic vec0 tables")?;

    // Key-value store for persisting runtime settings (e.g., auto-tuned relevance threshold)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS kv_store (
            key TEXT PRIMARY KEY NOT NULL,
            value REAL NOT NULL,
            updated_at TEXT DEFAULT (datetime('now'))
        );",
    )
    .ok(); // ok() because table may already exist

    info!(target: "ace::db", "ACE database schema initialized with sqlite-vec");
    Ok(())
}

/// Get bootstrap paths for initial scan
#[allow(dead_code)] // Future: ACE autonomous scanning
pub fn get_bootstrap_paths(conn: &Arc<Mutex<Connection>>) -> Result<Vec<String>> {
    let conn = conn.lock();
    let mut stmt =
        conn.prepare("SELECT path FROM bootstrap_paths WHERE scanned = 0 ORDER BY priority DESC")?;

    let rows = stmt.query_map([], |row| row.get(0))?;

    let paths: std::result::Result<Vec<String>, _> = rows.collect();
    Ok(paths?)
}

// ═══════════════════════════════════════════════════════════════
// REMOVED UNUSED FUNCTIONS (cleanup 2026-01-21):
// - mark_path_scanned
// - record_accuracy_metrics
// - get_active_topics_by_weight
// - get_tech_stack_summary
// - get_recent_activity_context
// - get_topic_affinities (ACE uses BehaviorLearner methods)
// - get_anti_topics (ACE uses BehaviorLearner methods)
// - ActivityContext struct
// - update_component_health
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_migration() {
        // Load sqlite-vec extension for vec0 virtual tables
        crate::register_sqlite_vec_extension();

        let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        assert!(migrate(&conn).is_ok());

        // Verify tables exist
        let conn_guard = conn.lock();
        let tables: Vec<String> = conn_guard
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();

        assert!(tables.contains(&"detected_projects".to_string()));
        assert!(tables.contains(&"detected_tech".to_string()));
        assert!(tables.contains(&"active_topics".to_string()));
        assert!(tables.contains(&"audit_log".to_string()));
    }
}
