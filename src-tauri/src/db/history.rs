//! Command history, HTTP history, maintenance, and diagnostics.

use rusqlite::{params, Result as SqliteResult};
use tracing::info;
use ts_rs::TS;

use super::Database;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct MaintenanceResult {
    pub deleted_items: usize,
    pub deleted_feedback: usize,
    pub deleted_void: usize,
    pub deleted_intelligence: usize,
    pub deleted_windows: usize,
    pub deleted_cycles: usize,
    pub deleted_necessity: usize,
    pub vacuumed: bool,
}

#[derive(Debug, Clone, Default, serde::Serialize, TS)]
#[ts(export, export_to = "bindings/")]
#[cfg_attr(test, derive(PartialEq))]
pub struct DbStats {
    pub source_items: i64,
    pub context_chunks: i64,
    pub feedback_count: i64,
    pub sources_count: i64,
    pub embeddings_count: i64,
    pub digested_intelligence: i64,
    pub decision_windows: i64,
    pub autophagy_cycles: i64,
    pub necessity_scores: i64,
    pub db_size_bytes: i64,
    pub oldest_item_date: Option<String>,
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
// Maintenance & Stats
// ============================================================================

impl Database {
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

        let deleted_void: usize = conn.execute("DELETE FROM void_positions", []).unwrap_or(0);

        // Clean superseded digested_intelligence older than 90 days
        let deleted_intelligence: usize = conn.execute(
            "DELETE FROM digested_intelligence WHERE superseded_by IS NOT NULL AND created_at < datetime('now', '-90 days')",
            [],
        ).unwrap_or(0);

        // Clean expired/closed decision_windows older than 60 days
        let deleted_windows: usize = conn.execute(
            "DELETE FROM decision_windows WHERE status IN ('expired', 'closed') AND created_at < datetime('now', '-60 days')",
            [],
        ).unwrap_or(0);

        // Clean old autophagy_cycles older than 180 days (keep recent history)
        let deleted_cycles: usize = conn.execute(
            "DELETE FROM autophagy_cycles WHERE created_at < datetime('now', '-180 days')",
            [],
        ).unwrap_or(0);

        // Clean orphaned necessity scores
        let deleted_necessity: usize = conn.execute(
            "DELETE FROM item_necessity WHERE source_item_id NOT IN (SELECT id FROM source_items)",
            [],
        ).unwrap_or(0);

        // Clean orphaned embeddings (belt-and-suspenders with cleanup_old_items)
        let _ = conn.execute(
            "DELETE FROM source_vec WHERE rowid NOT IN (SELECT id FROM source_items)",
            [],
        );

        info!(
            target: "4da::db",
            deleted_intelligence, deleted_windows, deleted_cycles, deleted_necessity,
            "Deep clean: pruned unbounded tables"
        );

        conn.execute_batch("PRAGMA optimize;")?;
        conn.execute_batch("VACUUM;")?;

        Ok(MaintenanceResult {
            deleted_items,
            deleted_feedback,
            deleted_void,
            deleted_intelligence,
            deleted_windows,
            deleted_cycles,
            deleted_necessity,
            vacuumed: true,
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

        let embeddings_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM source_vec", [], |row| row.get(0))
            .unwrap_or(0);

        let digested_intelligence: i64 = conn
            .query_row("SELECT COUNT(*) FROM digested_intelligence", [], |row| row.get(0))
            .unwrap_or(0);

        let decision_windows: i64 = conn
            .query_row("SELECT COUNT(*) FROM decision_windows", [], |row| row.get(0))
            .unwrap_or(0);

        let autophagy_cycles: i64 = conn
            .query_row("SELECT COUNT(*) FROM autophagy_cycles", [], |row| row.get(0))
            .unwrap_or(0);

        let necessity_scores: i64 = conn
            .query_row("SELECT COUNT(*) FROM item_necessity", [], |row| row.get(0))
            .unwrap_or(0);

        // DB file size via PRAGMA
        let db_size_bytes: i64 = conn
            .query_row("SELECT page_count * page_size FROM pragma_page_count, pragma_page_size", [], |row| row.get(0))
            .unwrap_or(0);

        let oldest_item_date: Option<String> = conn
            .query_row(
                "SELECT MIN(created_at) FROM source_items",
                [],
                |row| row.get(0),
            )
            .unwrap_or(None);

        Ok(DbStats {
            source_items,
            context_chunks,
            feedback_count,
            sources_count,
            embeddings_count,
            digested_intelligence,
            decision_windows,
            autophagy_cycles,
            necessity_scores,
            db_size_bytes,
            oldest_item_date,
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

    // ========================================================================
    // Command History
    // ========================================================================

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

    // ========================================================================
    // HTTP History
    // ========================================================================

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

    // ========================================================================
    // Cleanup & Diagnostics
    // ========================================================================

    /// Delete source_items older than the given number of days.
    pub fn cleanup_old_items(&self, max_age_days: u32) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM source_items WHERE last_seen < datetime('now', ?1)",
            params![format!("-{} days", max_age_days)],
        )?;
        let _ = conn.execute(
            "DELETE FROM feedback WHERE source_item_id NOT IN (SELECT id FROM source_items)",
            [],
        );
        let _ = conn.execute(
            "DELETE FROM source_vec WHERE rowid NOT IN (SELECT id FROM source_items)",
            [],
        );
        Ok(deleted)
    }

    /// Run VACUUM if more than threshold rows were deleted.
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
    use crate::test_utils::{insert_test_item, test_db};

    #[test]
    fn test_record_and_get_analysis_history() {
        let db = test_db();

        // DB stats on empty DB
        let stats = db.get_db_stats().unwrap();
        assert_eq!(stats.source_items, 0);
        assert_eq!(stats.context_chunks, 0);
        assert_eq!(stats.feedback_count, 0);
        assert_eq!(stats.sources_count, 0);

        // Insert some items
        insert_test_item(&db, "hackernews", "h1", "HN Item", "hn content");
        insert_test_item(&db, "reddit", "r1", "Reddit Item", "reddit content");

        let stats = db.get_db_stats().unwrap();
        assert_eq!(stats.source_items, 2);

        // Save command history
        db.save_command_history(
            "cargo test",
            "/home/user",
            0,
            true,
            Some("All tests passed"),
        )
        .unwrap();
        db.save_command_history("cargo build", "/home/user", 1, false, Some("error[E0308]"))
            .unwrap();

        let history = db.get_command_history(10).unwrap();
        assert_eq!(history.len(), 2);
        // Most recent first
        assert_eq!(history[0].command, "cargo build");
        assert!(!history[0].success);
        assert_eq!(history[1].command, "cargo test");
        assert!(history[1].success);

        // Save HTTP history
        db.save_http_history("GET", "https://api.example.com/data", 200, 150)
            .unwrap();

        let http_history = db.get_http_history(10).unwrap();
        assert_eq!(http_history.len(), 1);
        assert_eq!(http_history[0].method, "GET");
        assert_eq!(http_history[0].status, 200);
    }

    #[test]
    fn test_prune_old_items_respects_threshold() {
        let db = test_db();

        // Insert items — they will have last_seen = now
        insert_test_item(&db, "hackernews", "old_1", "Old Item 1", "old content 1");
        insert_test_item(&db, "hackernews", "old_2", "Old Item 2", "old content 2");
        insert_test_item(&db, "reddit", "new_1", "New Item", "new content");

        assert_eq!(db.total_item_count().unwrap(), 3);

        // Cleanup with 0 days should delete everything (all items have last_seen = now,
        // but datetime('now', '-0 days') = now, so items with last_seen < now won't match
        // unless they are strictly older). Items inserted just now should survive.
        let deleted = db.cleanup_old_items(0).unwrap();
        // Items were just created so last_seen = now; they should NOT be older than now
        assert_eq!(
            deleted, 0,
            "Items created just now should not be deleted with 0 day threshold"
        );
        assert_eq!(db.total_item_count().unwrap(), 3);

        // Manually age one item by setting last_seen in the past
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE source_items SET last_seen = datetime('now', '-10 days') WHERE source_id = 'old_1'",
                [],
            )
            .unwrap();
        }

        // Now cleanup with 5 day retention should delete the aged item
        let deleted = db.cleanup_old_items(5).unwrap();
        assert_eq!(deleted, 1, "Should delete 1 item older than 5 days");
        assert_eq!(db.total_item_count().unwrap(), 2);

        // vacuum_if_needed should not error
        db.vacuum_if_needed(deleted, 100).unwrap(); // threshold not met, no vacuum
        db.vacuum_if_needed(deleted, 1).unwrap(); // threshold met, runs vacuum
    }
}
