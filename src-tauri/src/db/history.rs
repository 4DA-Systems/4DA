// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

/// One item that relevance-aware forgetting (Phase 4) would delete — surfaced by the
/// dry-run so the policy can be inspected before any deletion is enabled.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PrunableSample {
    pub id: i64,
    pub title: String,
    pub source_type: String,
    pub relevance_score: f64,
    pub created_at: String,
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
        let tx = conn.unchecked_transaction()?;

        let deleted_items: usize = tx.execute(
            "DELETE FROM source_items WHERE last_seen < datetime('now', ?1)",
            params![format!("-{} days", retention_days)],
        )?;

        let deleted_feedback: usize = tx.execute(
            "DELETE FROM feedback WHERE created_at < datetime('now', ?1)",
            params![format!("-{} days", retention_days * 2)],
        )?;

        let deleted_void: usize = tx.execute("DELETE FROM void_positions", []).unwrap_or(0);

        // Clean superseded digested_intelligence older than 7 days (was 90 — 98.7% were dead rows)
        let deleted_intelligence: usize = tx.execute(
            "DELETE FROM digested_intelligence WHERE superseded_by IS NOT NULL AND created_at < datetime('now', '-7 days')",
            [],
        ).unwrap_or(0);

        // Clean expired/closed decision_windows older than 60 days
        let deleted_windows: usize = tx.execute(
            "DELETE FROM decision_windows WHERE status IN ('expired', 'closed') AND created_at < datetime('now', '-60 days')",
            [],
        ).unwrap_or(0);

        // Clean old autophagy_cycles older than 180 days (keep recent history)
        let deleted_cycles: usize = tx
            .execute(
                "DELETE FROM autophagy_cycles WHERE created_at < datetime('now', '-180 days')",
                [],
            )
            .unwrap_or(0);

        // Clean temporal_events older than 30 days
        let deleted_temporal: usize = tx
            .execute(
                "DELETE FROM temporal_events WHERE created_at < datetime('now', '-30 days')",
                [],
            )
            .unwrap_or(0);

        // Clean file_signals older than 7 days
        let deleted_file_signals: usize = tx
            .execute(
                "DELETE FROM file_signals WHERE timestamp < datetime('now', '-7 days')",
                [],
            )
            .unwrap_or(0);

        // Keep only the most recent 500 sun_runs
        let deleted_sun_runs: usize = tx.execute(
            "DELETE FROM sun_runs WHERE id NOT IN (SELECT id FROM sun_runs ORDER BY created_at DESC LIMIT 500)",
            [],
        ).unwrap_or(0);

        // Clean orphaned necessity scores
        let deleted_necessity: usize = tx.execute(
            "DELETE FROM item_necessity WHERE source_item_id NOT IN (SELECT id FROM source_items)",
            [],
        ).unwrap_or(0);

        // Clean orphaned embeddings (belt-and-suspenders with cleanup_old_items)
        if let Err(e) = tx.execute(
            "DELETE FROM source_vec WHERE rowid NOT IN (SELECT id FROM source_items)",
            [],
        ) {
            tracing::warn!(target: "4da::db", error = %e, "Failed to clean orphaned source_vec embeddings during deep clean");
        }

        info!(
            target: "4da::db",
            deleted_intelligence, deleted_windows, deleted_cycles, deleted_necessity,
            deleted_temporal, deleted_file_signals, deleted_sun_runs,
            "Deep clean: pruned unbounded tables"
        );

        tx.commit()?;

        // PRAGMA optimize and VACUUM run outside the transaction (VACUUM cannot be transactional)
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

    // ========================================================================
    // Relevance-aware forgetting (Phase 4 of the scoring relevance funnel)
    // ========================================================================
    //
    // The blunt `run_maintenance` prune deletes by `last_seen` age regardless of
    // value — but firehose noise keeps getting re-listed by its source, so its
    // `last_seen` stays fresh and it never ages out, while a quiet-but-relevant
    // dependency item would. That's backwards. This forgets CONFIRMED noise
    // (scored below `noise_threshold`) by `created_at` age, while structurally
    // protecting anything that could matter: high-stakes items (security/breaking/
    // CVE) and anything that scored at or above the threshold are never touched.
    // Items are re-fetchable from their source, so this is a cache eviction of
    // proven-irrelevant content, not a loss of signal.

    /// The shared WHERE clause for relevance-aware forgetting. Kept in one place so
    /// the dry-run count, the sample, and the actual delete can never diverge.
    fn noise_prune_predicate(noise_threshold: f64, min_age_days: i64) -> String {
        format!(
            "relevance_score IS NOT NULL \
               AND relevance_score < {noise_threshold} \
               AND scored_pipeline_version >= 1 \
               AND created_at < datetime('now', '-{min_age_days} days') \
               AND cve_ids IS NULL \
               AND (content_type IS NULL \
                    OR content_type NOT IN ('security_advisory', 'breaking_change'))"
        )
    }

    /// DRY-RUN: how many items the relevance-aware forget would delete. Read-only.
    pub fn count_prunable_noise(
        &self,
        noise_threshold: f64,
        min_age_days: i64,
    ) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        let sql = format!(
            "SELECT COUNT(*) FROM source_items WHERE {}",
            Self::noise_prune_predicate(noise_threshold, min_age_days)
        );
        conn.query_row(&sql, [], |r| r.get(0))
    }

    /// DRY-RUN: a sample of the worst+oldest items the forget would delete, for
    /// inspection before enabling deletion. Read-only.
    pub fn get_prunable_noise_sample(
        &self,
        noise_threshold: f64,
        min_age_days: i64,
        limit: usize,
    ) -> SqliteResult<Vec<PrunableSample>> {
        let conn = self.conn.lock();
        let sql = format!(
            "SELECT id, title, source_type, relevance_score, created_at
             FROM source_items WHERE {}
             ORDER BY relevance_score ASC, created_at ASC
             LIMIT ?1",
            Self::noise_prune_predicate(noise_threshold, min_age_days)
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(PrunableSample {
                id: row.get(0)?,
                title: row.get(1)?,
                source_type: row.get(2)?,
                relevance_score: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    /// Delete a bounded batch of confirmed-noise items (worst+oldest first).
    /// Bounded by `max_delete` so a single call can never run an unbounded
    /// transaction; call repeatedly to converge. ON DELETE CASCADE handles
    /// dependent rows; orphaned `source_vec` embeddings are swept too. Returns the
    /// number of source_items deleted.
    pub fn prune_noise(
        &self,
        noise_threshold: f64,
        min_age_days: i64,
        max_delete: usize,
    ) -> SqliteResult<usize> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        let deleted = tx.execute(
            &format!(
                "DELETE FROM source_items WHERE id IN (
                     SELECT id FROM source_items WHERE {}
                     ORDER BY relevance_score ASC, created_at ASC
                     LIMIT ?1
                 )",
                Self::noise_prune_predicate(noise_threshold, min_age_days)
            ),
            params![max_delete as i64],
        )?;
        // Sweep embeddings orphaned by the delete (source_vec has no FK cascade).
        let _ = tx.execute(
            "DELETE FROM source_vec WHERE rowid NOT IN (SELECT id FROM source_items)",
            [],
        );
        tx.commit()?;
        Ok(deleted)
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
            .query_row("SELECT COUNT(*) FROM digested_intelligence", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        let decision_windows: i64 = conn
            .query_row("SELECT COUNT(*) FROM decision_windows", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        let autophagy_cycles: i64 = conn
            .query_row("SELECT COUNT(*) FROM autophagy_cycles", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        let necessity_scores: i64 = conn
            .query_row("SELECT COUNT(*) FROM item_necessity", [], |row| row.get(0))
            .unwrap_or(0);

        // DB file size via PRAGMA
        let db_size_bytes: i64 = conn
            .query_row(
                "SELECT page_count * page_size FROM pragma_page_count, pragma_page_size",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let oldest_item_date: Option<String> = conn
            .query_row("SELECT MIN(created_at) FROM source_items", [], |row| {
                row.get(0)
            })
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
        // Sanitize output_preview to prevent secret leakage in stored logs
        let sanitized_output = output_preview.map(|s| {
            let truncated = if s.len() > 500 {
                &s[..s.floor_char_boundary(500)]
            } else {
                s
            };
            crate::llm::sanitize_api_error(truncated)
        });

        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO command_history (command, working_dir, exit_code, success, output_preview)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                command,
                working_dir,
                exit_code,
                success as i32,
                sanitized_output.as_deref()
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
        // Age is measured in whole DAYS, so compare against a DATE (midnight)
        // boundary, not the current instant. With `datetime('now', '-N days')`
        // a 0-day threshold resolves to "right now" and deletes items inserted
        // microseconds ago (last_seen < now) — a time-of-day race. `date('now',
        // '-N days')` means "older than midnight N days ago", so today's items
        // always survive a 0-day cleanup and the boundary is deterministic.
        let deleted = conn.execute(
            "DELETE FROM source_items WHERE last_seen < date('now', ?1)",
            params![format!("-{} days", max_age_days)],
        )?;
        if let Err(e) = conn.execute(
            "DELETE FROM feedback WHERE source_item_id NOT IN (SELECT id FROM source_items)",
            [],
        ) {
            tracing::warn!(target: "4da::db", error = %e, "Failed to clean orphaned feedback rows during cleanup_old_items");
        }
        if let Err(e) = conn.execute(
            "DELETE FROM source_vec WHERE rowid NOT IN (SELECT id FROM source_items)",
            [],
        ) {
            tracing::warn!(target: "4da::db", error = %e, "Failed to clean orphaned source_vec rows during cleanup_old_items");
        }
        Ok(deleted)
    }

    /// Bytes currently on the freelist — the space a VACUUM could return to the OS.
    /// A database packed with live content reports ~0 here regardless of total file
    /// size, so this (not raw size) is the honest signal for "optimization would help".
    pub fn reclaimable_bytes(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        let freelist: i64 = conn.query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
        let page_size: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        Ok(freelist.saturating_mul(page_size))
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

        // Cleanup with a 0-day threshold deletes items older than midnight today
        // (date('now', '-0 days')). Items inserted just now have last_seen = today,
        // so none are older than that boundary — deterministically zero deleted,
        // with no dependence on how much wall-clock elapses during the test.
        let deleted = db.cleanup_old_items(0).unwrap();
        assert_eq!(
            deleted, 0,
            "Items seen today should not be deleted with a 0 day threshold"
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

    /// Relevance-aware forgetting must delete ONLY confirmed old noise, and must
    /// structurally protect: relevant items, high-stakes items, and recent items.
    #[test]
    fn test_relevance_aware_noise_prune() {
        let db = test_db();

        let old_noise = insert_test_item(&db, "hackernews", "noise1", "old junk", "x");
        let relevant = insert_test_item(&db, "crates_io", "rel1", "your stack release", "x");
        let high_stakes = insert_test_item(&db, "cve", "cve1", "old advisory", "x");
        let recent_noise = insert_test_item(&db, "reddit", "noise2", "fresh junk", "x");

        {
            let conn = db.conn.lock();
            // old, near-zero noise → PRUNABLE
            conn.execute(
                "UPDATE source_items SET relevance_score=0.02, scored_pipeline_version=5, created_at=datetime('now','-120 days') WHERE id=?1",
                rusqlite::params![old_noise],
            ).unwrap();
            // old but RELEVANT → protected (score >= threshold)
            conn.execute(
                "UPDATE source_items SET relevance_score=0.62, scored_pipeline_version=5, created_at=datetime('now','-120 days') WHERE id=?1",
                rusqlite::params![relevant],
            ).unwrap();
            // old, low score, but HIGH-STAKES → protected (content_type carve-out)
            conn.execute(
                "UPDATE source_items SET relevance_score=0.02, scored_pipeline_version=5, content_type='security_advisory', created_at=datetime('now','-120 days') WHERE id=?1",
                rusqlite::params![high_stakes],
            ).unwrap();
            // low score but RECENT → protected (inside the age window)
            conn.execute(
                "UPDATE source_items SET relevance_score=0.02, scored_pipeline_version=5, created_at=datetime('now','-10 days') WHERE id=?1",
                rusqlite::params![recent_noise],
            ).unwrap();
        }

        // Dry-run: exactly one item qualifies.
        assert_eq!(db.count_prunable_noise(0.05, 90).unwrap(), 1);
        let sample = db.get_prunable_noise_sample(0.05, 90, 10).unwrap();
        assert_eq!(sample.len(), 1);
        assert_eq!(sample[0].id, old_noise);

        // Actual prune removes only the old noise; the protected three survive.
        let deleted = db.prune_noise(0.05, 90, 100).unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(db.total_item_count().unwrap(), 3);
        assert_eq!(
            db.count_prunable_noise(0.05, 90).unwrap(),
            0,
            "nothing left to prune after the sweep"
        );
    }

    /// The default noise-prune floor was lowered from 90 to 30 days because the corpus
    /// turns over far faster than a quarter (the 90-day floor was effectively inert).
    /// Pin that a 45-day-old confirmed-noise item is forgotten at a 30-day floor yet
    /// stays protected at the old 90-day floor.
    #[test]
    fn test_noise_prune_30day_floor_catches_what_90day_missed() {
        let db = test_db();
        let id = insert_test_item(&db, "hackernews", "n45", "45-day-old junk", "x");
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE source_items SET relevance_score=0.02, scored_pipeline_version=5, created_at=datetime('now','-45 days') WHERE id=?1",
                rusqlite::params![id],
            )
            .unwrap();
        }
        // Old 90-day floor: inert (the item is only 45 days old).
        assert_eq!(db.count_prunable_noise(0.05, 90).unwrap(), 0);
        // New 30-day floor: the item is now forgettable.
        assert_eq!(db.count_prunable_noise(0.05, 30).unwrap(), 1);
        assert_eq!(db.prune_noise(0.05, 30, 100).unwrap(), 1);
        assert_eq!(db.total_item_count().unwrap(), 0);
    }

    /// reclaimable_bytes reports ~0 on a freshly-packed DB (no freelist pages), proving
    /// raw file size is the wrong signal for "optimization would help" — only dead pages
    /// that VACUUM can return to the OS count as reclaimable.
    #[test]
    fn test_reclaimable_bytes_zero_on_packed_db() {
        let db = test_db();
        insert_test_item(&db, "hackernews", "a", "a", "x");
        assert_eq!(db.reclaimable_bytes().unwrap(), 0);
    }
}
