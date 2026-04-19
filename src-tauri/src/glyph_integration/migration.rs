// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! SQL migration for the `glyph_audit` table.
//!
//! Phase 2 of the GEP rollout adds a single table to 4DA's SQLite database.
//! The schema is identical to the glyph-integration-harness reference
//! implementation in the glyph repo.
//!
//! This file provides the **migration body** as a standalone function so
//! the Phase 54 migration in `db/migrations.rs` can call it without
//! duplicating SQL. Tests in `sqlite_audit_sink::tests` also use it to
//! bootstrap in-memory sinks.

use rusqlite::{Connection, Result as SqliteResult};

/// Create the `glyph_audit` table and its indices.
///
/// Idempotent via `IF NOT EXISTS` — safe to run against databases at any
/// migration state.
pub fn create_glyph_audit_table(conn: &Connection) -> SqliteResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS glyph_audit (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            envelope_id TEXT NOT NULL,
            agent TEXT NOT NULL,
            logged_at TEXT NOT NULL,
            summary TEXT NOT NULL,
            compiled_nl TEXT NOT NULL,
            header_glyphs TEXT NOT NULL,
            verdict TEXT NOT NULL,
            level TEXT NOT NULL,
            payload_bytes INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_glyph_audit_agent     ON glyph_audit(agent);
        CREATE INDEX IF NOT EXISTS idx_glyph_audit_level     ON glyph_audit(level);
        CREATE INDEX IF NOT EXISTS idx_glyph_audit_logged_at ON glyph_audit(logged_at);
        CREATE INDEX IF NOT EXISTS idx_glyph_audit_envelope  ON glyph_audit(envelope_id);
        "#,
    )
}
