// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! ACE Database Schema and Migrations
//!
//! Implements the full ACE database schema as specified in the stone tablet.

use parking_lot::Mutex;
use rusqlite::Connection;
use std::sync::Arc;
use tracing::info;

use crate::error::{Result, ResultExt};

/// Run all ACE database migrations
pub fn migrate(arc_conn: &Arc<Mutex<Connection>>) -> Result<()> {
    let conn = arc_conn.lock();

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

        -- User interactions (behavior signals)
        -- NOTE: This is the canonical schema — superset of ACE + ContextEngine columns.
        -- ACE uses: item_id, action_type, action_data, item_topics, item_source, signal_strength
        -- ContextEngine uses: source_item_id, action
        -- ACE initializes first, so this schema MUST include all columns both systems need.
        CREATE TABLE IF NOT EXISTS interactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_item_id INTEGER,             -- used by ContextEngine
            item_id INTEGER,                    -- used by ACE (nullable for ContextEngine compat)
            action TEXT,                        -- used by ContextEngine
            action_type TEXT,                   -- 'click', 'save', 'share', 'dismiss', etc.
            action_data TEXT,                   -- JSON with action-specific data (dwell_time, etc.)
            item_topics TEXT,                   -- JSON array
            item_source TEXT,                   -- 'hackernews', 'arxiv', etc.
            signal_strength REAL DEFAULT 0.5,
            timestamp TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_interactions_timestamp ON interactions(timestamp);
        CREATE INDEX IF NOT EXISTS idx_interactions_item ON interactions(source_item_id);
        CREATE INDEX IF NOT EXISTS idx_interactions_item_id ON interactions(item_id);
        CREATE INDEX IF NOT EXISTS idx_interactions_action ON interactions(action);
        CREATE INDEX IF NOT EXISTS idx_interactions_source ON interactions(item_source);
        CREATE INDEX IF NOT EXISTS idx_interactions_item_action ON interactions(item_id, action_type);

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

        -- REMOVED: ace_audit_log — dead table, never INSERT/SELECT/UPDATE/DELETE in production

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

        -- REMOVED: system_health — dead table, never used in production

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
            embedding BLOB,                    -- embedding for semantic search
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (document_id) REFERENCES indexed_documents(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_document_chunks_doc ON document_chunks(document_id);

    "#,
    )
    .context("ACE migration failed")?;

    // detected_tech needs the same re-baseline column so its half-life decay is
    // incremental, not compounding (the sibling topic decay already has it). Without
    // it, apply_detected_tech_decay had no place to record when it last ran.
    conn.execute_batch("ALTER TABLE detected_tech ADD COLUMN last_decay_at TEXT DEFAULT NULL;")
        .ok(); // ok() because column may already exist on subsequent runs

    // Phase 1D migration: Ensure interactions table has ContextEngine columns
    // If the interactions table was created before the schema unification,
    // it may be missing source_item_id and action columns.
    conn.execute_batch("ALTER TABLE interactions ADD COLUMN source_item_id INTEGER;")
        .ok(); // ok() because column may already exist
    conn.execute_batch("ALTER TABLE interactions ADD COLUMN action TEXT;")
        .ok(); // ok() because column may already exist
               // Also relax item_id NOT NULL → nullable (for ContextEngine rows that only use source_item_id)
               // Note: SQLite doesn't support ALTER COLUMN, but new inserts without item_id will work
               // because the CREATE TABLE IF NOT EXISTS won't fire on existing tables.
               // Ensure indexes exist for ContextEngine query patterns
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_interactions_action ON interactions(action);",
    )
    .ok();

    // Phase 1C migration: Anomalies table
    conn.execute_batch(
        r"
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
    ",
    )
    .ok();

    // Create vec0 virtual table for KNN search on topic embeddings (sqlite-vec)
    // This enables O(log n) semantic similarity search for topics
    conn.execute_batch(&format!(
        "
        -- Vector index for active topic embeddings ({dim}d embeddings)
        CREATE VIRTUAL TABLE IF NOT EXISTS topic_vec USING vec0(
            embedding float[{dim}]
        );

        -- REMOVED: affinity_vec — dead virtual table, never queried in production
        -- REMOVED: document_vec — dead virtual table, never queried in production
    ",
        dim = crate::EMBEDDING_DIMS
    ))
    .context("Failed to create topic vec0 tables")?;

    // Dimension migration: if topic_vec exists with old dimensions, rebuild it.
    // Check both active_topics blobs AND the vec0 table itself (probe insert).
    {
        let dim = crate::EMBEDDING_DIMS;
        let expected_bytes = dim * 4;

        let stale_blobs: bool = conn
            .query_row(
                "SELECT length(embedding) FROM active_topics WHERE embedding IS NOT NULL LIMIT 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|len| len as usize != expected_bytes)
            .unwrap_or(false);

        let stale_vec = if !stale_blobs {
            let probe = vec![0u8; expected_bytes];
            let ok = conn
                .execute(
                    "INSERT INTO topic_vec (rowid, embedding) VALUES (-1, ?1)",
                    rusqlite::params![probe],
                )
                .is_ok();
            if ok {
                conn.execute("DELETE FROM topic_vec WHERE rowid = -1", [])
                    .ok();
                false
            } else {
                true
            }
        } else {
            true
        };

        if stale_blobs || stale_vec {
            conn.execute_batch(&format!(
                "DROP TABLE IF EXISTS topic_vec;
                 CREATE VIRTUAL TABLE topic_vec USING vec0(
                     embedding float[{dim}]
                 );
                 UPDATE active_topics SET embedding = NULL
                   WHERE embedding IS NOT NULL;"
            ))
            .ok();
            info!(
                target: "ace::db",
                dim,
                stale_blobs,
                stale_vec,
                "Rebuilt topic_vec at {dim}d — dimension mismatch detected"
            );
        }
    }

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

/// Append one row to the identity ledger: what changed about the user's
/// modelled identity, why, and on what evidence.
///
/// Best-effort by design — a ledger write must never fail the operation it is
/// recording, and a missing table (a DB older than migration 112) must not
/// break topic minting. Errors are logged at debug and swallowed.
///
/// `kind`: "topic" | "tech" | "dependency".
/// `change`: "mint" | "reinforce" | "purge".
pub fn record_identity_change(
    conn: &Connection,
    kind: &str,
    key: &str,
    change: &str,
    reason: &str,
    evidence: Option<&str>,
) {
    if let Err(e) = conn.execute(
        "INSERT INTO identity_ledger (entity_kind, entity_key, change, reason, evidence)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![kind, key, change, reason, evidence],
    ) {
        tracing::debug!(target: "ace::db", error = %e, "identity ledger write skipped");
    }
}

/// One-shot startup self-heal (mirrors the Wave-5 agent-infra dependency
/// purge): delete active_topics rows that can never ground scoring — generic
/// tokens per the canonical predicate (`scoring::is_generic_topic_token`) —
/// plus legacy `commit-*` rows whose emitter was removed but which the
/// upsert-only persistence kept alive. Returns rows deleted. Heals existing
/// installs; the Wave-7 extraction fixes stop new pollution at the source.
pub fn purge_generic_active_topics(conn: &Connection) -> Result<usize> {
    let mut deleted = conn
        .execute("DELETE FROM active_topics WHERE topic LIKE 'commit-%'", [])
        .context("purge legacy commit-* active_topics")?;

    // The genericness predicate lives in Rust (COMMON_ENGLISH_WORDS + topic
    // additions), not SQL — select, filter, then delete by id.
    let generic_named: Vec<(i64, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, topic FROM active_topics")
            .context("select active_topics for generic prune")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .context("read active_topics")?;
        rows.flatten()
            .filter(|(_, topic)| crate::scoring::is_generic_topic_token(&topic.to_lowercase()))
            .collect::<Vec<(i64, String)>>()
    };
    let generic_ids: Vec<i64> = generic_named.iter().map(|(id, _)| *id).collect();

    for (id, topic) in &generic_named {
        record_identity_change(
            conn,
            "topic",
            topic,
            "purge",
            "generic_topic_token",
            Some(&format!("active_topics.id={id}")),
        );
    }

    for chunk in generic_ids.chunks(500) {
        let placeholders = vec!["?"; chunk.len()].join(",");
        let sql = format!("DELETE FROM active_topics WHERE id IN ({placeholders})");
        deleted += conn
            .execute(&sql, rusqlite::params_from_iter(chunk.iter()))
            .context("delete generic active_topics")?;
    }

    Ok(deleted)
}

// ═══════════════════════════════════════════════════════════════
// REMOVED UNUSED FUNCTIONS (cleanup 2026-01-21):
// - mark_path_scanned
// - record_accuracy_metrics
// - get_active_topics_by_weight
// - get_tech_stack_summary
// - get_recent_activity_context
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
    }

    #[test]
    fn test_purge_generic_active_topics() {
        crate::register_sqlite_vec_extension();
        let conn = Arc::new(Mutex::new(Connection::open_in_memory().unwrap()));
        migrate(&conn).unwrap();
        let conn = conn.lock();

        // Deleted: legacy commit-* rows + generic tokens per the canonical
        // predicate. Kept: specific tech (including 3-char tokens like "aws"
        // — the dep-side len<=3 blanket does not apply to topics), compound
        // pattern topics, short language names.
        for topic in [
            "commit-feat",
            "http",
            "rest",
            "api",
            "ui", // 1-2 char noise stays generic
            // Class-name fragments aren't on the generic lists — they stop
            // being minted (Wave 7) but existing rows survive the prune.
            "validationerror",
            "tauri",
            "react-native",
            "error_handling",
            "go",
            "ts",
            "aws",
        ] {
            conn.execute(
                "INSERT INTO active_topics (topic, source) VALUES (?1, 'file_content')",
                [topic],
            )
            .unwrap();
        }

        let deleted = purge_generic_active_topics(&conn).unwrap();
        assert_eq!(deleted, 5, "commit-feat + http + rest + api + ui");

        let remaining: Vec<String> = conn
            .prepare("SELECT topic FROM active_topics ORDER BY topic")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(
            remaining,
            vec![
                "aws",
                "error_handling",
                "go",
                "react-native",
                "tauri",
                "ts",
                "validationerror"
            ]
        );
    }
}
