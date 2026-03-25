//! MUSE Database Schema and Migrations
//!
//! Extends the ACE database with MUSE-specific tables for context packs,
//! source files, generation history, drift snapshots, and pack blends.
//!
//! Called from `ace::db::migrate()` to keep all schema in one migration flow.

use parking_lot::Mutex;
use rusqlite::Connection;
use std::sync::Arc;
use tracing::info;

use crate::error::{Result, ResultExt};

/// Run MUSE database migrations.
///
/// This is called after ACE migrations complete, in the same database.
/// All tables use `CREATE TABLE IF NOT EXISTS` for idempotency.
pub fn migrate(conn: &Arc<Mutex<Connection>>) -> Result<()> {
    let conn = conn.lock();

    conn.execute_batch(
        r#"
        -- ═══════════════════════════════════════════════════════════════
        -- MUSE: CONTEXT PACKS
        -- ═══════════════════════════════════════════════════════════════

        -- Context Packs — the atomic unit of creative context
        CREATE TABLE IF NOT EXISTS muse_packs (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            pack_type TEXT NOT NULL DEFAULT 'custom',
            is_active INTEGER NOT NULL DEFAULT 0,
            source_count INTEGER NOT NULL DEFAULT 0,
            confidence REAL NOT NULL DEFAULT 0.0,
            color_profile TEXT,                      -- JSON: dominant colors, temperature, contrast
            composition_profile TEXT,                -- JSON: symmetry, negative_space, focal_point
            style_embedding BLOB,                   -- 384-dim centroid of all source embeddings
            cluster_centers TEXT,                    -- JSON array of cluster center embeddings (base64)
            thematic_topics TEXT,                    -- JSON array of {label, weight}
            anti_patterns TEXT,                      -- JSON array of {label, weight}
            sonic_profile TEXT,                      -- JSON: timbre, rhythm, harmony, production
            motion_profile TEXT,                     -- JSON: pacing, transitions, camera
            metadata TEXT,                           -- JSON: arbitrary pack metadata
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_muse_packs_active ON muse_packs(is_active);
        CREATE INDEX IF NOT EXISTS idx_muse_packs_type ON muse_packs(pack_type);

        -- ═══════════════════════════════════════════════════════════════
        -- MUSE: PACK SOURCE FILES
        -- ═══════════════════════════════════════════════════════════════

        -- Source files that contribute to a pack
        CREATE TABLE IF NOT EXISTS muse_pack_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pack_id TEXT NOT NULL REFERENCES muse_packs(id) ON DELETE CASCADE,
            file_path TEXT NOT NULL,
            file_type TEXT NOT NULL,                 -- 'image', 'video', 'audio', 'document', 'project_file'
            media_type TEXT,                         -- MIME type
            extraction_status TEXT NOT NULL DEFAULT 'pending',
            extracted_at TEXT,
            embedding BLOB,                         -- 384-dim individual file embedding
            color_data TEXT,                         -- JSON: per-file color analysis
            composition_data TEXT,                   -- JSON: per-file composition analysis
            spectral_data TEXT,                      -- JSON: per-file audio analysis
            confidence REAL NOT NULL DEFAULT 0.0,
            file_hash TEXT,                          -- SHA-256 for change detection
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_muse_sources_pack ON muse_pack_sources(pack_id);
        CREATE INDEX IF NOT EXISTS idx_muse_sources_type ON muse_pack_sources(file_type);
        CREATE INDEX IF NOT EXISTS idx_muse_sources_status ON muse_pack_sources(extraction_status);

        -- ═══════════════════════════════════════════════════════════════
        -- MUSE: GENERATION HISTORY (feedback learning)
        -- ═══════════════════════════════════════════════════════════════

        -- Records of AI generation through MUSE influence
        CREATE TABLE IF NOT EXISTS muse_generations (
            id TEXT PRIMARY KEY,
            pack_id TEXT REFERENCES muse_packs(id),
            provider TEXT NOT NULL,                  -- 'runway', 'midjourney', 'dalle', 'sora', etc.
            prompt TEXT NOT NULL,
            enriched_prompt TEXT,
            influence_payload TEXT,                  -- JSON: full context sent to provider
            outcome TEXT,                            -- 'kept', 'rejected', 'modified', 'unknown'
            outcome_signal REAL,                     -- -1.0 to 1.0
            outcome_notes TEXT,
            generation_params TEXT,                  -- JSON: provider-specific params
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            outcome_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_muse_gen_pack ON muse_generations(pack_id);
        CREATE INDEX IF NOT EXISTS idx_muse_gen_outcome ON muse_generations(outcome);
        CREATE INDEX IF NOT EXISTS idx_muse_gen_provider ON muse_generations(provider);

        -- ═══════════════════════════════════════════════════════════════
        -- MUSE: CREATIVE DRIFT TRACKING
        -- ═══════════════════════════════════════════════════════════════

        -- Periodic snapshots tracking creative evolution over time
        CREATE TABLE IF NOT EXISTS muse_drift_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pack_id TEXT REFERENCES muse_packs(id),
            snapshot_type TEXT NOT NULL,              -- 'color', 'composition', 'style', 'sonic', 'overall'
            metric_name TEXT NOT NULL,
            metric_value REAL NOT NULL,
            embedding_snapshot BLOB,                 -- 384-dim style centroid at snapshot time
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_muse_drift_pack ON muse_drift_snapshots(pack_id, snapshot_type);
        CREATE INDEX IF NOT EXISTS idx_muse_drift_time ON muse_drift_snapshots(created_at);

        -- ═══════════════════════════════════════════════════════════════
        -- MUSE: PACK BLENDS (composite packs)
        -- ═══════════════════════════════════════════════════════════════

        -- Records which packs are blended into a composite pack
        CREATE TABLE IF NOT EXISTS muse_pack_blends (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            blend_pack_id TEXT NOT NULL REFERENCES muse_packs(id) ON DELETE CASCADE,
            source_pack_id TEXT NOT NULL REFERENCES muse_packs(id),
            weight REAL NOT NULL DEFAULT 0.5,
            UNIQUE(blend_pack_id, source_pack_id)
        );

        -- ═══════════════════════════════════════════════════════════════
        -- MUSE: STYLE VECTOR INDEX (sqlite-vec KNN)
        -- ═══════════════════════════════════════════════════════════════

        -- Vector index for pack style embeddings (enables similarity search)
        CREATE VIRTUAL TABLE IF NOT EXISTS muse_style_vec USING vec0(
            embedding float[384]
        );
    "#,
    )
    .context("MUSE migration failed")?;

    info!(target: "muse::db", "MUSE database schema initialized");
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Arc<Mutex<Connection>> {
        crate::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().expect("Failed to open in-memory DB");
        Arc::new(Mutex::new(conn))
    }

    #[test]
    fn test_muse_migration_succeeds() {
        let conn = setup_test_db();
        let result = migrate(&conn);
        assert!(result.is_ok(), "MUSE migration failed: {result:?}");
    }

    #[test]
    fn test_muse_migration_idempotent() {
        let conn = setup_test_db();
        migrate(&conn).expect("First migration failed");
        migrate(&conn).expect("Second migration should be idempotent");
    }

    #[test]
    fn test_muse_tables_exist() {
        let conn = setup_test_db();
        migrate(&conn).expect("Migration failed");
        let conn = conn.lock();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'muse_%'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"muse_packs".to_string()));
        assert!(tables.contains(&"muse_pack_sources".to_string()));
        assert!(tables.contains(&"muse_generations".to_string()));
        assert!(tables.contains(&"muse_drift_snapshots".to_string()));
        assert!(tables.contains(&"muse_pack_blends".to_string()));
    }

    #[test]
    fn test_insert_and_query_pack() {
        let conn = setup_test_db();
        migrate(&conn).expect("Migration failed");
        let conn = conn.lock();

        conn.execute(
            "INSERT INTO muse_packs (id, name, pack_type, confidence) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["pack-001", "Test Pack", "custom", 0.85],
        )
        .expect("Insert failed");

        let name: String = conn
            .query_row("SELECT name FROM muse_packs WHERE id = ?1", ["pack-001"], |row| {
                row.get(0)
            })
            .expect("Query failed");

        assert_eq!(name, "Test Pack");
    }

    #[test]
    fn test_pack_source_foreign_key() {
        let conn = setup_test_db();
        migrate(&conn).expect("Migration failed");
        let conn = conn.lock();

        // Enable foreign keys
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

        conn.execute(
            "INSERT INTO muse_packs (id, name, pack_type) VALUES ('p1', 'Test', 'custom')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO muse_pack_sources (pack_id, file_path, file_type) VALUES ('p1', '/test/image.png', 'image')",
            [],
        )
        .unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM muse_pack_sources WHERE pack_id = 'p1'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(count, 1);
    }

    #[test]
    fn test_muse_style_vec_table() {
        let conn = setup_test_db();
        migrate(&conn).expect("Migration failed");
        let conn = conn.lock();

        // Verify we can insert into the vec0 table
        let zero_vec = vec![0.0f32; 384];
        let blob: Vec<u8> = zero_vec.iter().flat_map(|f| f.to_le_bytes()).collect();

        conn.execute(
            "INSERT INTO muse_style_vec (rowid, embedding) VALUES (1, ?1)",
            [&blob],
        )
        .expect("Vec insert failed");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM muse_style_vec", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
