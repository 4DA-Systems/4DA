//! Migration and settings hardening tests
//!
//! Tests that database migrations are transactional, backups are created,
//! and settings validation clamps invalid values.

use std::path::PathBuf;

/// Helper: create a temp directory for test databases
fn temp_db_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = dir.path().join("test.db");
    (dir, db_path)
}

/// Test: fresh database initializes cleanly with all tables
#[test]
fn test_fresh_database_init() {
    let (_dir, db_path) = temp_db_dir();
    let db = fourda_lib::db::Database::new(&db_path).expect("Failed to create database");

    // Verify core tables exist
    let stats = db.get_db_stats().expect("Failed to get stats");
    assert_eq!(stats.source_items, 0);
    assert_eq!(stats.context_chunks, 0);
    assert_eq!(stats.feedback_count, 0);

    // Verify schema version is at latest
    let version = db
        .get_schema_version()
        .expect("Failed to get schema version");
    assert!(
        version >= 10,
        "Schema version should be >= 10, got {}",
        version
    );
}

/// Test: migration_history table is populated on fresh init
#[test]
fn test_migration_history_populated() {
    let (_dir, db_path) = temp_db_dir();
    let db = fourda_lib::db::Database::new(&db_path).expect("Failed to create database");

    let history = db
        .get_migration_history()
        .expect("Failed to get migration history");
    // Fresh database runs all migrations from v1 to v10
    assert!(
        history.len() >= 8,
        "Expected at least 8 migration records, got {}",
        history.len()
    );

    // All migrations should be successful
    for entry in &history {
        assert_eq!(
            entry.success, 1,
            "Migration {} -> {} failed",
            entry.from_version, entry.to_version
        );
        assert!(entry.duration_ms >= 0, "Duration should be non-negative");
    }
}

/// Test: database can be opened twice (migrations are idempotent)
#[test]
fn test_reopen_database_no_migration() {
    let (_dir, db_path) = temp_db_dir();

    // First open: runs all migrations
    {
        let _db = fourda_lib::db::Database::new(&db_path).expect("Failed to create database");
    }

    // Second open: should not run any new migrations
    let db = fourda_lib::db::Database::new(&db_path).expect("Failed to reopen database");
    let version = db
        .get_schema_version()
        .expect("Failed to get schema version");
    assert!(version >= 10);
}

/// Test: basic CRUD operations work after migration
#[test]
fn test_crud_after_migration() {
    let (_dir, db_path) = temp_db_dir();
    let db = fourda_lib::db::Database::new(&db_path).expect("Failed to create database");

    // Insert a source item
    let embedding = vec![0.0f32; 384];
    let id = db
        .upsert_source_item(
            "hackernews",
            "test-1",
            Some("https://example.com"),
            "Test Title",
            "Test content",
            &embedding,
        )
        .expect("Failed to insert source item");
    assert!(id > 0);

    // Verify count
    let count = db.total_item_count().expect("Failed to count items");
    assert_eq!(count, 1);

    // Register a source
    db.register_source("hackernews", "Hacker News")
        .expect("Failed to register source");

    // Verify source registry
    let sources = db.get_all_sources().expect("Failed to get sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].0, "hackernews");
}

/// Test: backup file is created during migration
#[test]
fn test_backup_created_during_migration() {
    let (dir, db_path) = temp_db_dir();

    // Create a database at an older version by manually initializing
    {
        fourda_lib::state::register_sqlite_vec_extension();
        let conn = rusqlite::Connection::open(&db_path).expect("Failed to open connection");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY);
             INSERT OR REPLACE INTO schema_version (version) VALUES (1);
             CREATE TABLE IF NOT EXISTS migration_history (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 from_version INTEGER NOT NULL,
                 to_version INTEGER NOT NULL,
                 executed_at TEXT NOT NULL DEFAULT (datetime('now')),
                 duration_ms INTEGER NOT NULL DEFAULT 0,
                 success INTEGER NOT NULL DEFAULT 0
             );
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
             CREATE TABLE IF NOT EXISTS sources (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 source_type TEXT NOT NULL UNIQUE,
                 name TEXT NOT NULL,
                 enabled INTEGER NOT NULL DEFAULT 1,
                 config TEXT,
                 last_fetch TEXT,
                 created_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             CREATE TABLE IF NOT EXISTS feedback (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 source_item_id INTEGER NOT NULL,
                 relevant INTEGER NOT NULL,
                 created_at TEXT NOT NULL DEFAULT (datetime('now')),
                 FOREIGN KEY (source_item_id) REFERENCES source_items(id)
             );
             CREATE VIRTUAL TABLE IF NOT EXISTS context_vec USING vec0(embedding float[384]);
             CREATE VIRTUAL TABLE IF NOT EXISTS source_vec USING vec0(embedding float[384]);",
        )
        .expect("Failed to set up old schema");
    }

    // Now open with Database::new which should trigger migrations + backup
    let _db =
        fourda_lib::db::Database::new(&db_path).expect("Failed to open database for migration");

    // Check that a backup file was created
    let backup_exists = std::fs::read_dir(dir.path())
        .expect("Failed to read temp dir")
        .filter_map(|e| e.ok())
        .any(|e| e.path().to_string_lossy().contains(".db.backup.v"));

    assert!(
        backup_exists,
        "Backup file should be created during migration"
    );
}

// ============================================================================
// Settings Validation Tests
// ============================================================================

/// Test: settings with default values pass validation
#[test]
fn test_default_settings_valid() {
    let mut settings = fourda_lib::settings::Settings::default();
    settings.validate(); // Should not panic or change anything

    assert_eq!(settings.rerank.max_items_per_batch, 48);
    assert_eq!(settings.embedding_threshold, 0.50);
    assert_eq!(settings.monitoring.interval_minutes, 10);
}

/// Test: max_items_per_batch = 0 is clamped to 1
#[test]
fn test_settings_clamp_zero_batch_size() {
    let mut settings = fourda_lib::settings::Settings::default();
    settings.rerank.max_items_per_batch = 0;
    settings.validate();
    assert_eq!(settings.rerank.max_items_per_batch, 1);
}

/// Test: embedding_threshold out of range is clamped
#[test]
fn test_settings_clamp_embedding_threshold() {
    let mut settings = fourda_lib::settings::Settings::default();

    settings.embedding_threshold = 1.5;
    settings.validate();
    assert_eq!(settings.embedding_threshold, 1.0);

    settings.embedding_threshold = -0.5;
    settings.validate();
    assert_eq!(settings.embedding_threshold, 0.0);
}

/// Test: min_embedding_score out of range is clamped
#[test]
fn test_settings_clamp_min_embedding_score() {
    let mut settings = fourda_lib::settings::Settings::default();

    settings.rerank.min_embedding_score = 2.0;
    settings.validate();
    assert_eq!(settings.rerank.min_embedding_score, 1.0);

    settings.rerank.min_embedding_score = -1.0;
    settings.validate();
    assert_eq!(settings.rerank.min_embedding_score, 0.0);
}

/// Test: monitoring interval_minutes = 0 is clamped to 1
#[test]
fn test_settings_clamp_zero_interval() {
    let mut settings = fourda_lib::settings::Settings::default();
    settings.monitoring.interval_minutes = 0;
    settings.validate();
    assert_eq!(settings.monitoring.interval_minutes, 1);
}

/// Test: empty context_dirs entries are removed
#[test]
fn test_settings_remove_empty_context_dirs() {
    let mut settings = fourda_lib::settings::Settings::default();
    settings.context_dirs = vec![
        "valid/path".to_string(),
        "".to_string(),
        "  ".to_string(),
        "another/valid".to_string(),
    ];
    settings.validate();
    assert_eq!(settings.context_dirs.len(), 2);
    assert_eq!(settings.context_dirs[0], "valid/path");
    assert_eq!(settings.context_dirs[1], "another/valid");
}

/// Test: settings deserialization with missing fields uses defaults
#[test]
fn test_settings_missing_fields_defaults() {
    let json = r#"{
        "llm": { "provider": "none", "api_key": "", "model": "" },
        "rerank": { "enabled": true, "max_items_per_batch": 48, "min_embedding_score": 0.2, "daily_token_limit": 500000, "daily_cost_limit_cents": 100 },
        "context_dirs": [],
        "embedding_threshold": 0.5
    }"#;

    let settings: fourda_lib::settings::Settings =
        serde_json::from_str(json).expect("Failed to deserialize settings");

    // Fields not in JSON should have defaults
    assert_eq!(settings.monitoring.interval_minutes, 10);
    assert_eq!(settings.monitoring.enabled, true);
    assert_eq!(settings.rss_feeds.len(), 0);
}
