//! Database migrations — schema versioning, backup, and migration orchestration.

use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use rusqlite::{params, Connection, Result as SqliteResult};
use std::path::{Path, PathBuf};
use tracing::info;

use super::Database;

// ============================================================================
// Cold-Boot Recovery Notice — surfaced via startup_health
// ============================================================================
//
// `state.rs::get_database()` calls `recover_corrupt_db_if_needed` *before*
// `Database::new()`. The recovery result is stored in this static so that
// `startup_health::check_database()` can pick it up and emit a `HealthIssue`
// the frontend already knows how to display. This avoids plumbing
// `AppHandle` into the lazy database initializer (which has no async runtime
// and no Tauri context at the moment it runs).

/// Last cold-boot DB recovery outcome. Set once per process by `state.rs`,
/// read by `startup_health.rs`. `None` means recovery hasn't run yet.
static DB_RECOVERY_NOTICE: OnceCell<RwLock<Option<CorruptionRecovery>>> = OnceCell::new();

/// Record a recovery outcome for the startup health check to surface.
/// Called by `state.rs::get_database()` immediately after running
/// `recover_corrupt_db_if_needed`.
pub fn set_db_recovery_notice(result: CorruptionRecovery) {
    let cell = DB_RECOVERY_NOTICE.get_or_init(|| RwLock::new(None));
    *cell.write() = Some(result);
}

/// Read and clear the recovery notice. Returns `None` if recovery never ran
/// or has already been read once. Used by `startup_health::check_database()`.
///
/// We clear after reading so the issue is shown exactly once per cold boot —
/// repeated frontend health-check polls don't keep re-surfacing the banner
/// after the user has already seen and dismissed it.
pub fn take_db_recovery_notice() -> Option<CorruptionRecovery> {
    let cell = DB_RECOVERY_NOTICE.get()?;
    cell.write().take()
}

// ============================================================================
// Cold-Boot Corruption Recovery
// ============================================================================

/// Result of a cold-boot integrity check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorruptionRecovery {
    /// DB file does not exist yet — first run, nothing to recover.
    NoExistingDb,
    /// DB opened cleanly and `PRAGMA quick_check` returned `ok`.
    Healthy,
    /// DB was corrupt and was successfully restored from a backup.
    /// `restored_from` is the path of the backup file used.
    RestoredFromBackup { restored_from: PathBuf },
    /// DB was corrupt and no usable backup existed. The corrupt file was
    /// quarantined to `quarantined_to`. The next call to `Database::new`
    /// will create a fresh DB at the original path.
    QuarantinedNoBackup { quarantined_to: PathBuf },
    /// DB was corrupt and recovery itself failed (filesystem error,
    /// permission issue, etc.). The original file is untouched. The
    /// caller must decide whether to abort startup or proceed degraded.
    RecoveryFailed { reason: String },
}

/// Inspect the main DB at `db_path`. If it's missing the function returns
/// `NoExistingDb`. If it opens cleanly and `quick_check` returns `ok`, the
/// function returns `Healthy`. Otherwise the function attempts to restore
/// from the most recent `*.db.backup.v*` sibling file in the same directory.
///
/// Recovery semantics, in order:
///
/// 1. **Quarantine.** The corrupt file is renamed to
///    `<name>.corrupt-<unix-timestamp>` so it can be examined post-mortem
///    and never accidentally re-opened.
/// 2. **Restore.** The most recent backup (highest `vN` suffix) is copied
///    over the original path. If the copy succeeds the function returns
///    `RestoredFromBackup`.
/// 3. **Fresh start.** If no backup exists, the function returns
///    `QuarantinedNoBackup`. The caller's normal `Database::new` call
///    will then create an empty DB at the original path and run all
///    migrations from scratch.
///
/// **This function is intentionally unwired in the cold-boot path as of
/// 2026-04-11.** Wiring it requires touching the bootstrap site (likely
/// `lib.rs` or `state.rs::open_db_connection`) which is currently
/// claimed by another terminal's read-only sweep. Wire it after the next
/// commit lock release by calling it once at the start of the database
/// initialization path, before `Database::new(db_path)`.
///
/// All file operations are infallible at the API level — failures are
/// captured into `CorruptionRecovery::RecoveryFailed` so the caller can
/// log and decide. The function never panics.
///
/// **Wiring:** called from `state.rs::get_database()` immediately before
/// `Database::new(&db_path)`. The result is stored via
/// `set_db_recovery_notice()` so `startup_health::check_database()` can
/// surface a `HealthIssue` to the frontend on the next health-check poll.
pub fn recover_corrupt_db_if_needed(db_path: &Path) -> CorruptionRecovery {
    // 1. Missing file → first run, nothing to do.
    if !db_path.exists() {
        return CorruptionRecovery::NoExistingDb;
    }

    // 2. Try to open the file and run a structural integrity check.
    //    Use `quick_check` rather than `integrity_check` because the latter
    //    is O(n) on rows and a 500MB DB would block startup for seconds.
    //    `quick_check` catches structural corruption (the kind that causes
    //    crash loops) without scanning every row.
    let healthy = match Connection::open(db_path) {
        Ok(conn) => {
            let pragma_result: rusqlite::Result<String> =
                conn.query_row("PRAGMA quick_check", [], |row| row.get(0));
            match pragma_result {
                Ok(s) if s == "ok" => true,
                Ok(other) => {
                    tracing::error!(
                        target: "4da::db::recovery",
                        path = %db_path.display(),
                        result = %other,
                        "PRAGMA quick_check did not return 'ok' — DB is corrupt"
                    );
                    false
                }
                Err(e) => {
                    tracing::error!(
                        target: "4da::db::recovery",
                        path = %db_path.display(),
                        error = %e,
                        "PRAGMA quick_check failed — DB is corrupt"
                    );
                    false
                }
            }
        }
        Err(e) => {
            tracing::error!(
                target: "4da::db::recovery",
                path = %db_path.display(),
                error = %e,
                "Connection::open failed — DB is unreadable"
            );
            false
        }
    };

    if healthy {
        return CorruptionRecovery::Healthy;
    }

    // 3. Quarantine the corrupt file.
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let quarantine_path = db_path.with_extension(format!("db.corrupt-{timestamp}"));

    if let Err(e) = std::fs::rename(db_path, &quarantine_path) {
        return CorruptionRecovery::RecoveryFailed {
            reason: format!(
                "Could not quarantine corrupt DB to {}: {e}",
                quarantine_path.display()
            ),
        };
    }
    tracing::warn!(
        target: "4da::db::recovery",
        from = %db_path.display(),
        to = %quarantine_path.display(),
        "Quarantined corrupt DB"
    );

    // 4. Find the most recent backup. Backups are named "<stem>.db.backup.vN"
    //    with N = schema version at backup time. We pick the file with the
    //    highest N because higher schema = more migrations applied = closer
    //    to the user's expected state.
    let parent = match db_path.parent() {
        Some(p) => p,
        None => {
            return CorruptionRecovery::QuarantinedNoBackup {
                quarantined_to: quarantine_path,
            };
        }
    };

    let backup = find_most_recent_backup(parent, db_path);

    let restore_from = match backup {
        Some(p) => p,
        None => {
            tracing::warn!(
                target: "4da::db::recovery",
                "No backups available — next launch will start with a fresh DB"
            );
            return CorruptionRecovery::QuarantinedNoBackup {
                quarantined_to: quarantine_path,
            };
        }
    };

    // 5. Restore by copying the backup over the (now-empty) original path.
    if let Err(e) = std::fs::copy(&restore_from, db_path) {
        return CorruptionRecovery::RecoveryFailed {
            reason: format!(
                "Quarantined corrupt DB but failed to restore from {}: {e}",
                restore_from.display()
            ),
        };
    }

    tracing::info!(
        target: "4da::db::recovery",
        from = %restore_from.display(),
        to = %db_path.display(),
        "Restored DB from backup after quarantine"
    );

    CorruptionRecovery::RestoredFromBackup {
        restored_from: restore_from,
    }
}

/// Scan the directory for files matching `<stem>.db.backup.vN` siblings of
/// `db_path` and return the one with the highest numeric suffix.
fn find_most_recent_backup(dir: &Path, db_path: &Path) -> Option<PathBuf> {
    let stem = db_path.file_stem()?.to_string_lossy().to_string();
    let prefix = format!("{stem}.db.backup.v");

    let entries = std::fs::read_dir(dir).ok()?;

    let mut best: Option<(u64, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !name.starts_with(&prefix) {
            continue;
        }
        let suffix = &name[prefix.len()..];
        // Parse the version number from the suffix. We accept any unsigned
        // integer — Database versioning is monotonically increasing.
        let version: u64 = match suffix.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if best.as_ref().map_or(true, |(v, _)| version > *v) {
            best = Some((version, path));
        }
    }

    best.map(|(_, p)| p)
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::tempdir;

    #[test]
    fn no_existing_db_returns_no_existing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.db");
        assert_eq!(
            recover_corrupt_db_if_needed(&path),
            CorruptionRecovery::NoExistingDb
        );
    }

    #[test]
    fn healthy_db_returns_healthy() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("healthy.db");
        // Create a real, valid SQLite file with some content.
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE t (x INTEGER); INSERT INTO t VALUES (1);")
            .unwrap();
        drop(conn);

        assert_eq!(
            recover_corrupt_db_if_needed(&path),
            CorruptionRecovery::Healthy
        );
        // Original file untouched.
        assert!(path.exists());
    }

    #[test]
    fn corrupt_db_with_no_backup_quarantines() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("corrupt.db");
        // Garbage bytes are not a valid SQLite file.
        std::fs::write(&path, b"this is not a sqlite database, just garbage").unwrap();

        let result = recover_corrupt_db_if_needed(&path);
        match result {
            CorruptionRecovery::QuarantinedNoBackup { quarantined_to } => {
                assert!(quarantined_to.exists());
                assert!(!path.exists()); // original moved
                assert!(quarantined_to
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains("corrupt-"));
            }
            other => panic!("expected QuarantinedNoBackup, got {other:?}"),
        }
    }

    #[test]
    fn corrupt_db_with_backup_restores() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("d.db");

        // Create a valid backup file with a known marker table.
        let backup_path = dir.path().join("d.db.backup.v3");
        let backup_conn = Connection::open(&backup_path).unwrap();
        backup_conn
            .execute_batch("CREATE TABLE marker (id INTEGER); INSERT INTO marker VALUES (42);")
            .unwrap();
        drop(backup_conn);

        // Write garbage as the "current" db.
        std::fs::write(&path, b"corrupt").unwrap();

        let result = recover_corrupt_db_if_needed(&path);
        match result {
            CorruptionRecovery::RestoredFromBackup { restored_from } => {
                assert_eq!(restored_from, backup_path);
                // Original path now contains the backup contents.
                assert!(path.exists());
                let conn = Connection::open(&path).unwrap();
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM marker", [], |r| r.get(0))
                    .unwrap();
                assert_eq!(count, 1);
            }
            other => panic!("expected RestoredFromBackup, got {other:?}"),
        }
    }

    #[test]
    fn picks_highest_version_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("d.db");

        // Three backups, v1 / v5 / v3 — recovery should pick v5.
        for (v, marker) in [(1u32, 100i64), (5, 500), (3, 300)] {
            let p = dir.path().join(format!("d.db.backup.v{v}"));
            let c = Connection::open(&p).unwrap();
            c.execute_batch(&format!(
                "CREATE TABLE m (x INTEGER); INSERT INTO m VALUES ({marker});"
            ))
            .unwrap();
        }

        std::fs::write(&path, b"corrupt").unwrap();

        let result = recover_corrupt_db_if_needed(&path);
        if let CorruptionRecovery::RestoredFromBackup { restored_from } = result {
            assert!(restored_from.to_string_lossy().ends_with("v5"));
            // Verify it's the v5 marker (500), not v1 (100) or v3 (300).
            let conn = Connection::open(&path).unwrap();
            let val: i64 = conn
                .query_row("SELECT x FROM m LIMIT 1", [], |r| r.get(0))
                .unwrap();
            assert_eq!(val, 500);
        } else {
            panic!("expected RestoredFromBackup");
        }
    }
}

impl Database {
    /// Create a pre-migration backup of the database file.
    /// Keeps only the last 2 backups to avoid disk bloat.
    pub(crate) fn backup_before_migration(&self, current_version: i64) {
        let backup_path = self
            .db_path
            .with_extension(format!("db.backup.v{current_version}"));
        // Checkpoint WAL so the main db file is consistent for copy
        if let Some(conn) = self.conn.try_lock() {
            if let Err(e) = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)") {
                tracing::warn!("DB execute failed: {e}");
            }
        }
        match std::fs::copy(&self.db_path, &backup_path) {
            Ok(bytes) => {
                info!(target: "4da::db", path = %backup_path.display(), bytes, "Pre-migration backup created");
            }
            Err(e) => {
                tracing::warn!(target: "4da::db", error = %e, "Pre-migration backup failed (continuing anyway)");
            }
        }
        // Prune old backups: keep only the 2 most recent
        if let Some(parent) = self.db_path.parent() {
            if let Ok(entries) = std::fs::read_dir(parent) {
                let mut backups: Vec<PathBuf> = entries
                    .filter_map(|e| match e {
                        Ok(v) => Some(v),
                        Err(e) => {
                            tracing::warn!("Row processing failed in db_migrations: {e}");
                            None
                        }
                    })
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
            let res = migration_fn(&tx).and_then(|()| {
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
        if let Err(e) = conn.execute(
            "INSERT INTO migration_history (from_version, to_version, executed_at, duration_ms, success) VALUES (?1, ?2, datetime('now'), ?3, ?4)",
            params![from_version, to_version, duration_ms, result.is_ok() as i32],
        ) {
            tracing::warn!(target: "4da::db", error = %e, from_version, to_version, "Failed to record migration in migration_history");
        }

        match &result {
            Ok(()) => {
                info!(target: "4da::db", name, to_version, duration_ms, "{} completed in {}ms", name, duration_ms);
            }
            Err(e) => {
                tracing::error!(target: "4da::db", name, to_version, error = %e, "{} FAILED — rolled back", name);
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

        const TARGET_VERSION: i64 = 57;

        // Downgrade detection: if DB schema is newer than this binary expects,
        // show a clear error instead of silently corrupting the schema.
        if current_version > TARGET_VERSION {
            return Err(rusqlite::Error::QueryReturnedNoRows).map_err(|_| {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISMATCH),
                    Some(format!(
                        "Database schema version {} is newer than this version of 4DA supports (max {}). \
                         You may be running an older version of 4DA against a newer database. \
                         Please update 4DA or restore a database backup.",
                        current_version, TARGET_VERSION
                    )),
                )
            });
        }

        if current_version < TARGET_VERSION {
            // Drop the conn lock briefly to allow backup (needs filesystem access)
            drop(conn);
            self.backup_before_migration(current_version);

            // Validate backup was written correctly
            let backup_path = self
                .db_path
                .with_extension(format!("db.backup.v{current_version}"));
            if let Ok(backup_meta) = std::fs::metadata(&backup_path) {
                if backup_meta.len() == 0 {
                    tracing::warn!(target: "4da::db", "Migration backup is empty — skipping backup validation");
                } else {
                    tracing::info!(target: "4da::db",
                        backup_path = ?backup_path,
                        size_bytes = backup_meta.len(),
                        "Migration backup validated"
                    );
                }
            }

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

                            -- DEAD TABLE: coach_messages — deprecated coach system, never used in production
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

                            -- DEAD TABLE: coach_documents — deprecated coach system, never used in production
                            CREATE TABLE IF NOT EXISTS coach_documents (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                doc_type TEXT NOT NULL,
                                content TEXT NOT NULL,
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );

                            -- DEAD TABLE: coach_nudges — deprecated coach system, never used in production
                            CREATE TABLE IF NOT EXISTS coach_nudges (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                nudge_type TEXT NOT NULL,
                                content TEXT NOT NULL,
                                dismissed INTEGER DEFAULT 0,
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );
                            CREATE INDEX IF NOT EXISTS idx_coach_nudges_dismissed
                                ON coach_nudges(dismissed);

                            -- DEAD TABLE: video_curriculum — deprecated coach system, never used in production
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

            // Phase 18 migration: Playbook progress table
            if current_version < 18 {
                Self::run_versioned_migration(&conn, 17, 18, "Phase 18: playbook progress", |c| {
                    c.execute_batch(
                        "CREATE TABLE IF NOT EXISTS playbook_progress (
                                module_id TEXT NOT NULL,
                                lesson_idx INTEGER NOT NULL,
                                completed_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                                PRIMARY KEY (module_id, lesson_idx)
                            );",
                    )
                })?;
            }

            if current_version < 19 {
                Self::run_versioned_migration(&conn, 18, 19, "Phase 19: scoring stats", |c| {
                    c.execute_batch(
                        "CREATE TABLE IF NOT EXISTS scoring_stats (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                run_type TEXT NOT NULL,
                                total_scored INTEGER NOT NULL,
                                relevant_count INTEGER NOT NULL,
                                excluded_count INTEGER NOT NULL,
                                rejection_rate REAL NOT NULL,
                                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
                            );",
                    )
                })?;
            }

            // Phase 20 migration: achievement engine tables
            if current_version < 20 {
                Self::run_versioned_migration(
                    &conn,
                    19,
                    20,
                    "Phase 20: achievement engine",
                    |c| crate::achievement_engine::create_tables(c),
                )?;
            }

            // Phase 21 migration: Content Personalization cache + read state
            if current_version < 21 {
                Self::run_versioned_migration(
                    &conn,
                    20,
                    21,
                    "Phase 21: content personalization",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS content_personalization_cache (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                module_id TEXT NOT NULL,
                                lesson_idx INTEGER NOT NULL,
                                block_type TEXT NOT NULL,
                                block_id TEXT NOT NULL,
                                content_json TEXT NOT NULL,
                                generation_path TEXT NOT NULL,
                                context_hash TEXT NOT NULL,
                                profile_hash TEXT NOT NULL,
                                llm_tokens_used INTEGER DEFAULT 0,
                                llm_cost_cents INTEGER DEFAULT 0,
                                generated_at TEXT DEFAULT (datetime('now')),
                                expires_at TEXT,
                                UNIQUE(module_id, lesson_idx, block_type, block_id, context_hash)
                            );
                            CREATE INDEX IF NOT EXISTS idx_personalization_cache_lookup
                                ON content_personalization_cache(module_id, lesson_idx, context_hash);

                            CREATE TABLE IF NOT EXISTS content_read_state (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                module_id TEXT NOT NULL,
                                lesson_idx INTEGER NOT NULL,
                                context_hash TEXT NOT NULL,
                                profile_snapshot TEXT NOT NULL,
                                read_at TEXT DEFAULT (datetime('now')),
                                UNIQUE(module_id, lesson_idx)
                            );",
                        )
                    },
                )?;
            }

            // Phase 22 migration: Information Channels
            if current_version < 22 {
                Self::run_versioned_migration(
                    &conn,
                    21,
                    22,
                    "Phase 22: information channels",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS channels (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                slug TEXT NOT NULL UNIQUE,
                                title TEXT NOT NULL,
                                description TEXT NOT NULL DEFAULT '',
                                topic_query TEXT NOT NULL DEFAULT '[]',
                                status TEXT NOT NULL DEFAULT 'active',
                                source_count INTEGER NOT NULL DEFAULT 0,
                                render_count INTEGER NOT NULL DEFAULT 0,
                                last_rendered_at TEXT,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );
                            CREATE INDEX IF NOT EXISTS idx_channels_slug ON channels(slug);
                            CREATE INDEX IF NOT EXISTS idx_channels_status ON channels(status);

                            CREATE TABLE IF NOT EXISTS channel_renders (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                channel_id INTEGER NOT NULL,
                                version INTEGER NOT NULL,
                                content_markdown TEXT NOT NULL,
                                content_hash TEXT NOT NULL,
                                source_item_ids TEXT NOT NULL DEFAULT '[]',
                                model TEXT,
                                tokens_used INTEGER,
                                latency_ms INTEGER,
                                rendered_at TEXT NOT NULL DEFAULT (datetime('now')),
                                FOREIGN KEY (channel_id) REFERENCES channels(id),
                                UNIQUE(channel_id, version)
                            );
                            CREATE INDEX IF NOT EXISTS idx_channel_renders_channel
                                ON channel_renders(channel_id);

                            CREATE TABLE IF NOT EXISTS channel_provenance (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                render_id INTEGER NOT NULL,
                                claim_index INTEGER NOT NULL,
                                claim_text TEXT NOT NULL,
                                source_item_ids TEXT NOT NULL DEFAULT '[]',
                                source_titles TEXT NOT NULL DEFAULT '[]',
                                source_urls TEXT NOT NULL DEFAULT '[]',
                                FOREIGN KEY (render_id) REFERENCES channel_renders(id)
                            );
                            CREATE INDEX IF NOT EXISTS idx_channel_provenance_render
                                ON channel_provenance(render_id);

                            CREATE TABLE IF NOT EXISTS channel_source_matches (
                                channel_id INTEGER NOT NULL,
                                source_item_id INTEGER NOT NULL,
                                match_score REAL NOT NULL DEFAULT 0.0,
                                matched_at TEXT NOT NULL DEFAULT (datetime('now')),
                                PRIMARY KEY (channel_id, source_item_id),
                                FOREIGN KEY (channel_id) REFERENCES channels(id),
                                FOREIGN KEY (source_item_id) REFERENCES source_items(id)
                            );
                            CREATE INDEX IF NOT EXISTS idx_channel_source_matches_channel
                                ON channel_source_matches(channel_id);",
                        )?;

                        // Seed default channels
                        let seeds: &[(&str, &str, &str, &str)] = &[
                            (
                                "local-ai-hardware",
                                "Hardware for Local AI",
                                "GPU availability, VRAM benchmarks, quantization advances, and hardware acceleration for local inference.",
                                r#"["gpu","nvidia","amd","apple silicon","vram","quantization","gguf","local inference","hardware acceleration","npu","cuda","rocm","metal"]"#,
                            ),
                            (
                                "local-llm-landscape",
                                "Local LLM Landscape",
                                "Open-weight models, inference engines, fine-tuning techniques, and the local AI ecosystem.",
                                r#"["ollama","llama","llm","gguf","mistral","llama.cpp","vllm","mlx","fine-tuning","lora","open source model","embedding model","whisper","inference engine"]"#,
                            ),
                            (
                                "developer-tools-shifting",
                                "Developer Tools Shifting",
                                "IDE evolution, AI coding assistants, build systems, and the changing developer toolchain.",
                                r#"["developer tools","cli","ide","vscode","neovim","build system","ai coding","copilot","cursor","toolchain","dx","bun","deno","turbopack"]"#,
                            ),
                        ];
                        for (slug, title, desc, topics) in seeds {
                            c.execute(
                                "INSERT OR IGNORE INTO channels
                                    (slug, title, description, topic_query, status,
                                     source_count, render_count, created_at, updated_at)
                                 VALUES (?1, ?2, ?3, ?4, 'active', 0, 0, datetime('now'), datetime('now'))",
                                rusqlite::params![slug, title, desc, topics],
                            )?;
                        }

                        info!(target: "4da::db", "Created channels tables and seeded 3 default channels");
                        Ok(())
                    },
                )?;
            }

            // Phase 23 migration: Performance indexes
            if current_version < 23 {
                Self::run_versioned_migration(
                    &conn,
                    22,
                    23,
                    "Phase 23: performance indexes",
                    |c| {
                        c.execute_batch(
                        "CREATE INDEX IF NOT EXISTS idx_feedback_created ON feedback(created_at);
                         CREATE INDEX IF NOT EXISTS idx_feedback_item_relevant ON feedback(source_item_id, relevant);
                         CREATE INDEX IF NOT EXISTS idx_source_items_created ON source_items(created_at);
                         CREATE INDEX IF NOT EXISTS idx_digest_superseded ON digested_intelligence(superseded_by);
                         CREATE INDEX IF NOT EXISTS idx_channel_renders_channel_version ON channel_renders(channel_id, version);",
                    )
                    },
                )?;
            }

            // Phase 24 migration: Intelligence History (Trajectory Phase 2)
            if current_version < 24 {
                Self::run_versioned_migration(
                    &conn,
                    23,
                    24,
                    "Phase 24: intelligence history",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS intelligence_history (
                                id INTEGER PRIMARY KEY,
                                recorded_at TEXT NOT NULL DEFAULT (datetime('now')),
                                accuracy REAL NOT NULL,
                                topics_learned INTEGER NOT NULL,
                                items_analyzed INTEGER NOT NULL,
                                relevant_found INTEGER NOT NULL
                            );
                            CREATE INDEX IF NOT EXISTS idx_intelligence_history_recorded
                                ON intelligence_history(recorded_at);",
                        )
                    },
                )?;
            }

            // Phase 25 migration: Local Telemetry
            if current_version < 25 {
                Self::run_versioned_migration(&conn, 24, 25, "Phase 25: local telemetry", |c| {
                    c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS user_events (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                event_type TEXT NOT NULL,
                                view_id TEXT,
                                metadata TEXT,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                session_id TEXT
                            );
                            CREATE INDEX IF NOT EXISTS idx_user_events_type ON user_events(event_type);
                            CREATE INDEX IF NOT EXISTS idx_user_events_created ON user_events(created_at);",
                        )
                })?;
            }

            // Phase 26: Drop unused tables
            if current_version < 26 {
                Self::run_versioned_migration(
                    &conn,
                    25,
                    26,
                    "Phase 26: Drop unused tables",
                    |c| {
                        c.execute_batch(
                            "DROP TABLE IF EXISTS git_commit_history;
                         DROP TABLE IF EXISTS chunk_sentiment;
                         DROP TABLE IF EXISTS item_relationships;
                         DROP TABLE IF EXISTS query_cache;
                         DROP TABLE IF EXISTS query_history;
                         DROP TABLE IF EXISTS file_metadata_cache;",
                        )?;
                        Ok(())
                    },
                )?;
            }

            // Phase 27: Team sync infrastructure (AD-023)
            if current_version < 27 {
                Self::run_versioned_migration(
                    &conn,
                    26,
                    27,
                    "Phase 27: team sync infrastructure",
                    Self::migrate_to_phase_27,
                )?;
            }

            // Phase 28: Team intelligence + shared resources
            if current_version < 28 {
                Self::run_versioned_migration(
                    &conn,
                    27,
                    28,
                    "Phase 28: team intelligence + shared resources",
                    Self::migrate_to_phase_28,
                )?;
            }

            // Phase 29: Team monitoring + signals
            if current_version < 29 {
                Self::run_versioned_migration(
                    &conn,
                    28,
                    29,
                    "Phase 29: team monitoring + signals",
                    Self::migrate_to_phase_29,
                )?;
            }

            // Phase 30: Enterprise audit log
            if current_version < 30 {
                Self::run_versioned_migration(
                    &conn,
                    29,
                    30,
                    "Phase 30: enterprise audit log",
                    Self::migrate_to_phase_30,
                )?;
            }

            // Phase 31: Enterprise webhooks
            if current_version < 31 {
                Self::run_versioned_migration(
                    &conn,
                    30,
                    31,
                    "Phase 31: enterprise webhooks",
                    Self::migrate_to_phase_31,
                )?;
            }

            // Phase 32: Enterprise organization + retention
            if current_version < 32 {
                Self::run_versioned_migration(
                    &conn,
                    31,
                    32,
                    "Phase 32: enterprise organization + retention",
                    Self::migrate_to_phase_32,
                )?;
            }

            if current_version < 33 {
                Self::run_versioned_migration(
                    &conn,
                    32,
                    33,
                    "Phase 33: SSO pending auth for OIDC state/nonce",
                    Self::migrate_to_phase_33,
                )?;
            }

            if current_version < 34 {
                Self::run_versioned_migration(
                    &conn,
                    33,
                    34,
                    "Phase 34: Dependency Intelligence tables",
                    Self::migrate_to_phase_34,
                )?;
            }

            if current_version < 35 {
                Self::run_versioned_migration(
                    &conn,
                    34,
                    35,
                    "Phase 35: Developer OS Intelligence tables",
                    Self::migrate_to_phase_35,
                )?;
            }

            if current_version < 36 {
                Self::run_versioned_migration(
                    &conn,
                    35,
                    36,
                    "Phase 36: Waitlist + i18n preferences",
                    Self::migrate_to_phase_36,
                )?;
            }

            if current_version < 37 {
                Self::run_versioned_migration(
                    &conn,
                    36,
                    37,
                    "Phase 37: License compliance column",
                    Self::migrate_to_phase_37,
                )?;
            }

            // Phase 38: Drop abandoned feature tables
            if current_version < 38 {
                Self::run_versioned_migration(
                    &conn,
                    37,
                    38,
                    "Phase 38: Drop abandoned feature tables",
                    Self::migrate_to_phase_38,
                )?;
            }

            // Phase 39: Briefing item history for novelty detection
            if current_version < 39 {
                Self::run_versioned_migration(
                    &conn,
                    38,
                    39,
                    "Phase 39: briefing_item_history",
                    |c| {
                        c.execute_batch(
                        "CREATE TABLE IF NOT EXISTS briefing_item_history (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            item_title TEXT NOT NULL,
                            source_type TEXT NOT NULL,
                            briefing_date TEXT NOT NULL,
                            created_at TEXT NOT NULL DEFAULT (datetime('now'))
                        );
                        CREATE INDEX IF NOT EXISTS idx_briefing_history_date ON briefing_item_history(briefing_date);",
                    )
                    },
                )?;
            }

            // Phase 40: Item necessity scores (persisted for MCP server access)
            if current_version < 40 {
                Self::run_versioned_migration(
                    &conn,
                    39,
                    40,
                    "Phase 40: item_necessity table for MCP access",
                    |c| {
                        c.execute_batch(
                        "CREATE TABLE IF NOT EXISTS item_necessity (
                            source_item_id INTEGER PRIMARY KEY REFERENCES source_items(id),
                            necessity_score REAL NOT NULL DEFAULT 0.0,
                            necessity_reason TEXT,
                            necessity_category TEXT,
                            necessity_urgency TEXT,
                            scored_at TEXT NOT NULL DEFAULT (datetime('now'))
                        );
                        CREATE INDEX IF NOT EXISTS idx_necessity_score ON item_necessity(necessity_score);",
                    )
                    },
                )?;
            }

            // Phase 41: Content analysis cache (deep pre-score content analysis)
            if current_version < 41 {
                Self::run_versioned_migration(
                    &conn,
                    40,
                    41,
                    "Phase 41: content_analyses table",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS content_analyses (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                source_item_id INTEGER NOT NULL,
                                content_hash TEXT NOT NULL,
                                technical_depth INTEGER NOT NULL,
                                novelty INTEGER NOT NULL,
                                audience_level TEXT NOT NULL,
                                key_insight TEXT,
                                analyzed_at TEXT NOT NULL DEFAULT (datetime('now')),
                                UNIQUE(content_hash)
                            );
                            CREATE INDEX IF NOT EXISTS idx_content_analyses_hash ON content_analyses(content_hash);
                            CREATE INDEX IF NOT EXISTS idx_content_analyses_item ON content_analyses(source_item_id);",
                        )
                    },
                )?;
            }

            if current_version < 42 {
                Self::run_versioned_migration(
                    &conn,
                    41,
                    42,
                    "Phase 42: view_count column on source_items for return-visit tracking",
                    |c| {
                        let has_column: bool = c
                            .query_row(
                                "SELECT COUNT(*) FROM pragma_table_info('source_items') WHERE name='view_count'",
                                [],
                                |row| row.get::<_, i64>(0).map(|count| count > 0),
                            )
                            .unwrap_or(false);
                        if !has_column {
                            c.execute_batch(
                                "ALTER TABLE source_items ADD COLUMN view_count INTEGER DEFAULT 0;",
                            )?;
                            info!("Added view_count column to source_items");
                        }
                        Ok(())
                    },
                )?;
            }

            // Phase 43: Content translation cache for multilingual feed items
            if current_version < 43 {
                Self::run_versioned_migration(
                    &conn,
                    42,
                    43,
                    "Phase 43: translation_cache for multilingual content",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS translation_cache (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                content_hash TEXT NOT NULL,
                                source_lang TEXT NOT NULL DEFAULT 'en',
                                target_lang TEXT NOT NULL,
                                source_text TEXT NOT NULL,
                                translated_text TEXT NOT NULL,
                                provider TEXT NOT NULL,
                                model_version TEXT,
                                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                                last_used_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                                use_count INTEGER NOT NULL DEFAULT 1
                            );
                            CREATE UNIQUE INDEX IF NOT EXISTS idx_translation_cache_lookup
                                ON translation_cache(content_hash, target_lang);
                            CREATE INDEX IF NOT EXISTS idx_translation_cache_expiry
                                ON translation_cache(last_used_at);",
                        )?;
                        info!(target: "4da::db", "Created translation_cache table for multilingual content");
                        Ok(())
                    },
                )?;
            }

            // Phase 44: Performance indexes for hot query paths
            if current_version < 44 {
                Self::run_versioned_migration(
                    &conn,
                    43,
                    44,
                    "Phase 44: performance indexes for feedback and source_items",
                    |c| {
                        c.execute_batch(
                            "CREATE INDEX IF NOT EXISTS idx_feedback_created_at ON feedback(created_at);
                            CREATE INDEX IF NOT EXISTS idx_feedback_relevant ON feedback(relevant);
                            CREATE INDEX IF NOT EXISTS idx_source_items_created_at ON source_items(created_at);",
                        )?;
                        info!(target: "4da::db", "Created performance indexes for feedback and source_items");
                        Ok(())
                    },
                )?;
            }

            // Phase 45: Add detected_lang column for multilingual content detection
            if current_version < 45 {
                Self::run_versioned_migration(
                    &conn,
                    44,
                    45,
                    "Phase 45: detected_lang column for multilingual content",
                    |c| {
                        c.execute_batch(
                            "ALTER TABLE source_items ADD COLUMN detected_lang TEXT DEFAULT 'en';",
                        )?;
                        info!(target: "4da::db", "Added detected_lang column to source_items");
                        Ok(())
                    },
                )?;
            }

            // Phase 46: app_meta table for embedding model tracking
            if current_version < 46 {
                Self::run_versioned_migration(
                    &conn,
                    45,
                    46,
                    "Phase 46: app_meta table for embedding model tracking",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS app_meta (
                                key TEXT PRIMARY KEY,
                                value TEXT NOT NULL
                            );",
                        )?;
                        info!(target: "4da::db", "Created app_meta table for embedding model tracking");
                        Ok(())
                    },
                )?;
            }

            // Phase 47: Security audit log for compliance and incident tracking
            if current_version < 47 {
                Self::run_versioned_migration(
                    &conn,
                    46,
                    47,
                    "Phase 47: Security audit log table",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS security_audit_log (
                                id INTEGER PRIMARY KEY,
                                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                                event_type TEXT NOT NULL,
                                details TEXT,
                                severity TEXT NOT NULL DEFAULT 'info'
                            );
                            CREATE INDEX IF NOT EXISTS idx_security_audit_timestamp
                                ON security_audit_log(timestamp);
                            CREATE INDEX IF NOT EXISTS idx_security_audit_event
                                ON security_audit_log(event_type);",
                        )?;
                        info!(target: "4da::db", "Created security_audit_log table");
                        Ok(())
                    },
                )?;
            }

            // Phase 48: Score persistence + language index for briefing fallback
            if current_version < 48 {
                Self::run_versioned_migration(
                    &conn,
                    47,
                    48,
                    "Phase 48: relevance_score column + detected_lang index",
                    |c| {
                        c.execute_batch(
                            "ALTER TABLE source_items ADD COLUMN relevance_score REAL DEFAULT NULL;
                             CREATE INDEX IF NOT EXISTS idx_source_items_detected_lang ON source_items(detected_lang);
                             CREATE INDEX IF NOT EXISTS idx_source_items_relevance_score ON source_items(relevance_score);",
                        )?;
                        info!(target: "4da::db", "Added relevance_score column and language/score indexes");
                        Ok(())
                    },
                )?;
            }

            // Phase 49: Add ON DELETE CASCADE triggers for orphan prevention
            // SQLite doesn't support ALTER CONSTRAINT, so we use triggers instead.
            if current_version < 49 {
                Self::run_versioned_migration(
                    &conn,
                    48,
                    49,
                    "Phase 49: cascade delete triggers for orphan prevention",
                    |c| {
                        c.execute_batch(
                            "-- Cascade deletes from source_items to dependent tables
                             CREATE TRIGGER IF NOT EXISTS trg_source_items_cascade_delete
                             AFTER DELETE ON source_items
                             BEGIN
                                 DELETE FROM feedback WHERE source_item_id = OLD.id;
                                 DELETE FROM item_necessity WHERE source_item_id = OLD.id;
                                 DELETE FROM channel_source_matches WHERE source_item_id = OLD.id;
                                 DELETE FROM content_analyses WHERE source_item_id = OLD.id;
                             END;

                             -- Cascade deletes from channels to dependent tables
                             CREATE TRIGGER IF NOT EXISTS trg_channels_cascade_delete
                             AFTER DELETE ON channels
                             BEGIN
                                 DELETE FROM channel_renders WHERE channel_id = OLD.id;
                                 DELETE FROM channel_source_matches WHERE channel_id = OLD.id;
                             END;

                             -- Cascade deletes from channel_renders to provenance
                             CREATE TRIGGER IF NOT EXISTS trg_channel_renders_cascade_delete
                             AFTER DELETE ON channel_renders
                             BEGIN
                                 DELETE FROM channel_provenance WHERE render_id = OLD.id;
                             END;",
                        )?;
                        info!(target: "4da::db", "Added cascade delete triggers for orphan prevention");
                        Ok(())
                    },
                )?;
            }

            // Phase 50: Additional cascade triggers + FK join indexes (audit gaps)
            if current_version < 50 {
                Self::run_versioned_migration(
                    &conn,
                    49,
                    50,
                    "Phase 50: audit cascade triggers + FK join indexes",
                    |c| {
                        c.execute_batch(
                            "-- Cascade delete triggers for gaps identified in audit
                             CREATE TRIGGER IF NOT EXISTS trg_source_items_delete_dep_alerts
                             AFTER DELETE ON source_items
                             BEGIN
                                 DELETE FROM dependency_alerts WHERE source_item_id = OLD.id;
                             END;

                             CREATE TRIGGER IF NOT EXISTS trg_webhooks_delete_deliveries
                             AFTER DELETE ON webhooks
                             BEGIN
                                 DELETE FROM webhook_deliveries WHERE webhook_id = OLD.id;
                             END;

                             -- Performance indexes on frequently-joined FK columns
                             CREATE INDEX IF NOT EXISTS idx_channel_renders_channel ON channel_renders(channel_id);
                             CREATE INDEX IF NOT EXISTS idx_channel_provenance_render ON channel_provenance(render_id);",
                        )?;
                        info!(target: "4da::db", "Added audit cascade triggers and FK join indexes");
                        Ok(())
                    },
                )?;
            }

            // Phase 51: Sovereign Cold Boot — persisted scheduler state.
            // Stores last-run timestamps for each background job so they
            // survive process restart. Without this table, every cold boot
            // re-fires the entire backlog of "scheduled" jobs because the
            // in-memory atomics default to 0 (the cold-boot stampede).
            if current_version < 51 {
                Self::run_versioned_migration(
                    &conn,
                    50,
                    51,
                    "Phase 51: scheduler_state for cold-boot stampede prevention",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS scheduler_state (
                                job_name TEXT PRIMARY KEY NOT NULL,
                                last_run_unix INTEGER NOT NULL DEFAULT 0,
                                last_duration_ms INTEGER,
                                run_count INTEGER NOT NULL DEFAULT 0,
                                last_outcome TEXT,
                                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );

                             -- Pre-seed all known jobs with last_run = 0 so a fresh
                             -- DB still benefits from the grace period (the scheduler
                             -- will skip jobs whose first run is within the grace).
                             -- Existing rows are left alone (INSERT OR IGNORE).
                             INSERT OR IGNORE INTO scheduler_state (job_name, last_run_unix) VALUES
                                ('health_check', 0),
                                ('db_maintenance', 0),
                                ('vacuum', 0),
                                ('anomaly_detection', 0),
                                ('cve_scan', 0),
                                ('dep_health', 0),
                                ('behavior_decay', 0),
                                ('autophagy', 0),
                                ('accuracy_record', 0),
                                ('temporal_snapshot', 0);",
                        )?;
                        info!(target: "4da::db", "Created scheduler_state table (Sovereign Cold Boot)");
                        Ok(())
                    },
                )?;
            }

            // Phase 52: Trust Ledger — intelligence quality measurement
            if current_version < 52 {
                Self::run_versioned_migration(
                    &conn,
                    51,
                    52,
                    "Phase 52: trust ledger (precision, preemption, action tracking)",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS trust_events (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                event_type TEXT NOT NULL,
                                signal_id TEXT,
                                alert_id TEXT,
                                source_type TEXT,
                                topic TEXT,
                                lead_time_hours REAL,
                                user_action TEXT,
                                outcome TEXT,
                                confidence_at_surface REAL,
                                notes TEXT,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                resolved_at TEXT
                             );

                             CREATE TABLE IF NOT EXISTS precision_stats (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                period TEXT NOT NULL,
                                domain TEXT NOT NULL,
                                total_surfaced INTEGER DEFAULT 0,
                                true_positives INTEGER DEFAULT 0,
                                false_positives INTEGER DEFAULT 0,
                                false_negatives INTEGER DEFAULT 0,
                                acted_on INTEGER DEFAULT 0,
                                dismissed INTEGER DEFAULT 0,
                                precision REAL,
                                action_conversion_rate REAL,
                                avg_lead_time_hours REAL,
                                computed_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );

                             CREATE TABLE IF NOT EXISTS preemption_wins (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                alert_id TEXT NOT NULL,
                                alert_title TEXT NOT NULL,
                                alerted_at TEXT NOT NULL,
                                incident_at TEXT,
                                lead_time_hours REAL,
                                affected_deps TEXT,
                                user_acted INTEGER DEFAULT 0,
                                verified INTEGER DEFAULT 0,
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );",
                        )?;
                        info!(target: "4da::db", "Created trust_events, precision_stats, preemption_wins tables");
                        Ok(())
                    },
                )?;
            }

            // Phase 53: Add is_direct column to project_dependencies for
            // direct vs transitive dependency differentiation in scoring.
            if current_version < 53 {
                Self::run_versioned_migration(
                    &conn,
                    52,
                    53,
                    "Phase 53: is_direct column on project_dependencies",
                    |c| {
                        let has_column: bool = c
                            .query_row(
                                "SELECT COUNT(*) FROM pragma_table_info('project_dependencies') WHERE name='is_direct'",
                                [],
                                |row| row.get::<_, i64>(0).map(|count| count > 0),
                            )
                            .unwrap_or(false);
                        if !has_column {
                            c.execute_batch(
                                "ALTER TABLE project_dependencies ADD COLUMN is_direct INTEGER DEFAULT 1;",
                            )?;
                            info!(target: "4da::db", "Added is_direct column to project_dependencies");
                        }
                        Ok(())
                    },
                )?;
            }

            // Phase 54: Glyph Envelope Protocol audit-only table.
            // Stores shadow envelopes generated by `glyph_integration::mcp_envelope`
            // when the `glyph_audit` feature is enabled. Table is created
            // unconditionally so toggling the feature does not require a
            // schema migration — the feature flag controls whether rows
            // are written, not whether the table exists.
            if current_version < 54 {
                Self::run_versioned_migration(
                    &conn,
                    53,
                    54,
                    "Phase 54: glyph_audit table (GEP Phase 2 audit-only)",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS glyph_audit (
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
                             CREATE INDEX IF NOT EXISTS idx_glyph_audit_envelope  ON glyph_audit(envelope_id);",
                        )?;
                        info!(target: "4da::db", "Created glyph_audit table + 4 indices (GEP Phase 2)");
                        Ok(())
                    },
                )?;
            }

            // ── Phase 55: Intelligence pipeline accuracy overhaul ─────────
            // Adds structured metadata columns to source_items for entity
            // extraction at ingestion time (content type classification and
            // CVE ID extraction) plus project relevance scoring.
            if current_version < 55 {
                Self::run_versioned_migration(
                    &conn,
                    54,
                    55,
                    "Phase 55: content_type + cve_ids columns + project_relevance",
                    |c| {
                        c.execute_batch(
                            "ALTER TABLE source_items ADD COLUMN content_type TEXT DEFAULT NULL;
                             ALTER TABLE source_items ADD COLUMN cve_ids TEXT DEFAULT NULL;
                             CREATE INDEX IF NOT EXISTS idx_source_content_type ON source_items(content_type);

                             -- Project relevance scoring: filter out example/demo/test projects
                             -- from intelligence surfaces (preemption, blind spots)
                             ALTER TABLE project_dependencies ADD COLUMN project_relevance REAL DEFAULT 1.0;
                             CREATE INDEX IF NOT EXISTS idx_deps_relevance ON project_dependencies(project_relevance);

                             -- Retroactively set low relevance for known noise directory patterns.
                             -- New ACE scans will compute proper relevance; this covers existing data.
                             UPDATE project_dependencies SET project_relevance = 0.05
                               WHERE project_path LIKE '%/example%'
                                  OR project_path LIKE '%/demo%'
                                  OR project_path LIKE '%/test/%'
                                  OR project_path LIKE '%/tests/%'
                                  OR project_path LIKE '%/tutorial%'
                                  OR project_path LIKE '%/template%'
                                  OR project_path LIKE '%/sample%'
                                  OR project_path LIKE '%/fixture%'
                                  OR project_path LIKE '%/benchmark%'
                                  OR project_path LIKE '%\\example%'
                                  OR project_path LIKE '%\\demo%'
                                  OR project_path LIKE '%\\test\\%'
                                  OR project_path LIKE '%\\tests\\%'
                                  OR project_path LIKE '%\\tutorial%'
                                  OR project_path LIKE '%\\template%'
                                  OR project_path LIKE '%\\sample%'
                                  OR project_path LIKE '%\\fixture%'
                                  OR project_path LIKE '%\\benchmark%'
                                  OR project_path LIKE '%workbench%'
                                  OR project_path LIKE '%worktree%';",
                        )?;
                        info!(target: "4da::db", "Added content_type, cve_ids, project_relevance columns + indices");
                        Ok(())
                    },
                )?;
            }

            // ── Phase 56: Intelligence Mesh provenance table ─────────────
            // Pre-launch architectural pivot (see docs/strategy/INTELLIGENCE-
            // MESH.md §5.3). Every AI-influenced artifact — relevance score,
            // LLM rerank adjustment, summary, briefing, translation, embed —
            // gets a provenance row recording which model/prompt/calibration
            // produced it. This unlocks:
            //   • Receipts ("Why this score?" UI panel)
            //   • Drift detection when a model's behavior changes
            //   • Safe migration across model swaps (compound-learning
            //     respects provenance cohorts)
            //   • Shadow-arena peer comparisons (shadow_peer_id)
            //
            // The table is additive. No existing data changes. Artifacts
            // produced before this migration are simply un-stamped; when a
            // later pass re-scores them, new provenance is recorded. We
            // intentionally do NOT backfill fake provenance rows — absence
            // of a row means "unknown / pre-mesh", which is honest.
            if current_version < 56 {
                Self::run_versioned_migration(
                    &conn,
                    55,
                    56,
                    "Phase 56: Intelligence Mesh provenance table",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS provenance (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                artifact_kind TEXT NOT NULL,
                                artifact_id TEXT NOT NULL,
                                model_identity_hash TEXT NOT NULL,
                                provider TEXT NOT NULL,
                                model TEXT NOT NULL,
                                prompt_version TEXT,
                                calibration_id TEXT,
                                task TEXT NOT NULL,
                                temperature REAL,
                                raw_response_hash TEXT,
                                shadow_peer_id INTEGER,
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );
                             CREATE INDEX IF NOT EXISTS idx_provenance_artifact
                               ON provenance(artifact_kind, artifact_id);
                             CREATE INDEX IF NOT EXISTS idx_provenance_model
                               ON provenance(model_identity_hash);
                             CREATE INDEX IF NOT EXISTS idx_provenance_created_at
                               ON provenance(created_at);
                             CREATE INDEX IF NOT EXISTS idx_provenance_task
                               ON provenance(task);",
                        )?;
                        info!(
                            target: "4da::db",
                            "Created provenance table + 4 indices (Intelligence Mesh Phase 3)"
                        );
                        Ok(())
                    },
                )?;
            }

            // ── Phase 57: Intelligence Mesh calibration samples table ──────
            // Phase 5b.2 (The Filter) needs per-signal persistence so the
            // fitter can pair advisor judgments with downstream user
            // interactions and derive binary labels. Provenance captures
            // MODEL identity per judged item but NOT the score the advisor
            // gave — without the score we can't fit a curve. This table
            // is the fitter's input; one row per AdvisorSignal emitted by
            // the rerank loop.
            //
            // The table is append-only at stamp time. `processed_at` is
            // NULL until a fit run consumes the row; once set, the sample
            // has contributed to at least one curve and won't be refit.
            //
            // Sample age: the fitter waits a minimum window (e.g. 24h)
            // from created_at before considering a row paired, because
            // InteractionPattern classification needs dwell + scroll
            // telemetry that only arrives on item close.
            if current_version < 57 {
                Self::run_versioned_migration(
                    &conn,
                    56,
                    57,
                    "Phase 57: Intelligence Mesh calibration samples table",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS calibration_samples (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                source_item_id INTEGER NOT NULL,
                                model_identity_hash TEXT NOT NULL,
                                task TEXT NOT NULL,
                                prompt_version TEXT NOT NULL,
                                raw_score REAL NOT NULL,
                                confidence REAL NOT NULL,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                processed_at TEXT
                             );
                             -- Pairing join: sample → interactions on (source_item_id, time window).
                             CREATE INDEX IF NOT EXISTS idx_cal_samples_item
                               ON calibration_samples(source_item_id, created_at);
                             -- Fitter's candidate-set scan: unfit rows per (model, task).
                             CREATE INDEX IF NOT EXISTS idx_cal_samples_unfit
                               ON calibration_samples(model_identity_hash, task, processed_at);
                             CREATE INDEX IF NOT EXISTS idx_cal_samples_created
                               ON calibration_samples(created_at);",
                        )?;
                        info!(
                            target: "4da::db",
                            "Created calibration_samples table + 3 indices (Intelligence Mesh Phase 5b.2)"
                        );
                        Ok(())
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

        // DEAD TABLE: file_metadata_cache — never used, dropped in Phase 26
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
        // DEAD TABLE: query_cache — never used, dropped in Phase 26
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

        // DEAD TABLE: query_history — never used, dropped in Phase 26
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

        // DEAD TABLE: chunk_sentiment — never populated, dropped in Phase 26
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
                CREATE INDEX IF NOT EXISTS idx_source_items_embedding_status ON source_items(embedding_status);
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

        // DEAD TABLE: item_relationships — never used, dropped in Phase 26
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

    /// Phase 27 migration: Team sync infrastructure (AD-023)
    fn migrate_to_phase_27(conn: &Connection) -> SqliteResult<()> {
        conn.execute_batch(
            "-- Team sync queue (outbound entries not yet acknowledged by relay)
            CREATE TABLE IF NOT EXISTS team_sync_queue (
                entry_id    TEXT PRIMARY KEY,
                team_id     TEXT NOT NULL,
                client_id   TEXT NOT NULL,
                operation   TEXT NOT NULL,
                hlc_ts      INTEGER NOT NULL,
                encrypted   BLOB,
                relay_seq   INTEGER,
                created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                acked_at    INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_tsq_pending
                ON team_sync_queue(acked_at) WHERE acked_at IS NULL;

            -- Team sync log (inbound entries received from relay)
            CREATE TABLE IF NOT EXISTS team_sync_log (
                relay_seq   INTEGER NOT NULL,
                team_id     TEXT NOT NULL,
                client_id   TEXT NOT NULL,
                encrypted   BLOB NOT NULL,
                received_at INTEGER NOT NULL DEFAULT (unixepoch()),
                applied     INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (relay_seq, team_id)
            );
            CREATE INDEX IF NOT EXISTS idx_tsl_unapplied
                ON team_sync_log(applied) WHERE applied = 0;

            -- Team sync state (track highest processed sequence per team)
            CREATE TABLE IF NOT EXISTS team_sync_state (
                team_id         TEXT PRIMARY KEY,
                last_relay_seq  INTEGER NOT NULL DEFAULT 0,
                last_sync_at    INTEGER
            );

            -- Team crypto keys (keypair + team symmetric key)
            CREATE TABLE IF NOT EXISTS team_crypto (
                team_id             TEXT PRIMARY KEY,
                our_public_key      BLOB NOT NULL,
                our_private_key_enc BLOB NOT NULL,
                team_symmetric_key_enc BLOB,
                created_at          INTEGER NOT NULL DEFAULT (unixepoch())
            );

            -- Team members cache (synced from relay)
            CREATE TABLE IF NOT EXISTS team_members_cache (
                team_id      TEXT NOT NULL,
                client_id    TEXT NOT NULL,
                display_name TEXT NOT NULL,
                role         TEXT NOT NULL DEFAULT 'member',
                public_key   BLOB,
                last_seen    TEXT,
                PRIMARY KEY (team_id, client_id)
            );",
        )?;

        info!(target: "4da::db", "Created team sync tables (queue, log, state, crypto, members)");
        Ok(())
    }

    /// Phase 28: Team intelligence + shared resources
    fn migrate_to_phase_28(conn: &Connection) -> SqliteResult<()> {
        conn.execute_batch(
            "-- Shared resources (DNA, decisions, signals shared between team members)
            CREATE TABLE IF NOT EXISTS shared_resources (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                resource_data TEXT NOT NULL,
                shared_by TEXT NOT NULL,
                visibility TEXT DEFAULT 'team',
                visible_to TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                expires_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_shared_team_type
                ON shared_resources(team_id, resource_type);
            CREATE INDEX IF NOT EXISTS idx_shared_expires
                ON shared_resources(expires_at) WHERE expires_at IS NOT NULL;

            -- Team decisions (proposals + votes)
            CREATE TABLE IF NOT EXISTS team_decisions (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                title TEXT NOT NULL,
                decision_type TEXT NOT NULL,
                rationale TEXT NOT NULL,
                proposed_by TEXT NOT NULL,
                status TEXT DEFAULT 'proposed',
                created_at TEXT DEFAULT (datetime('now')),
                resolved_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_team_decisions_team
                ON team_decisions(team_id, status);

            -- Decision votes
            CREATE TABLE IF NOT EXISTS decision_votes (
                decision_id TEXT NOT NULL,
                voter_id TEXT NOT NULL,
                stance TEXT NOT NULL,
                rationale TEXT,
                voted_at TEXT DEFAULT (datetime('now')),
                PRIMARY KEY (decision_id, voter_id)
            );",
        )?;
        info!(target: "4da::db", "Created shared resources + team decisions tables");
        Ok(())
    }

    /// Phase 29: Team monitoring + signals
    fn migrate_to_phase_29(conn: &Connection) -> SqliteResult<()> {
        conn.execute_batch(
            "-- Team signals (aggregated across seats)
            CREATE TABLE IF NOT EXISTS team_signals (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                signal_type TEXT NOT NULL,
                title TEXT NOT NULL,
                severity TEXT NOT NULL,
                tech_topics TEXT,
                detected_by_count INTEGER DEFAULT 1,
                first_detected TEXT DEFAULT (datetime('now')),
                last_detected TEXT DEFAULT (datetime('now')),
                resolved INTEGER DEFAULT 0,
                resolved_by TEXT,
                resolved_at TEXT,
                resolution_notes TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_team_signals_team
                ON team_signals(team_id, resolved);

            -- Team alert policies
            CREATE TABLE IF NOT EXISTS team_alert_policies (
                team_id TEXT PRIMARY KEY,
                min_seats_to_alert INTEGER DEFAULT 2,
                aggregation_window_minutes INTEGER DEFAULT 60,
                notification_channels TEXT DEFAULT '[\"in_app\"]',
                updated_at TEXT DEFAULT (datetime('now'))
            );",
        )?;
        info!(target: "4da::db", "Created team signals + alert policies tables");
        Ok(())
    }

    /// Phase 30: Enterprise audit log
    fn migrate_to_phase_30(conn: &Connection) -> SqliteResult<()> {
        conn.execute_batch(
            "DROP TABLE IF EXISTS audit_log;
            CREATE TABLE audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                team_id TEXT NOT NULL,
                actor_id TEXT NOT NULL,
                actor_display_name TEXT NOT NULL,
                action TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                resource_id TEXT,
                details TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_audit_team_time
                ON audit_log(team_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_audit_actor
                ON audit_log(actor_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_audit_action
                ON audit_log(action);",
        )?;
        info!(target: "4da::db", "Created enterprise audit log table");
        Ok(())
    }

    /// Phase 31: Enterprise webhooks
    fn migrate_to_phase_31(conn: &Connection) -> SqliteResult<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS webhooks (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                events TEXT NOT NULL,
                secret TEXT NOT NULL,
                active INTEGER DEFAULT 1,
                failure_count INTEGER DEFAULT 0,
                last_fired_at TEXT,
                last_status_code INTEGER,
                created_at TEXT DEFAULT (datetime('now')),
                created_by TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_webhooks_team
                ON webhooks(team_id, active);

            CREATE TABLE IF NOT EXISTS webhook_deliveries (
                id TEXT PRIMARY KEY,
                webhook_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                status TEXT DEFAULT 'pending',
                http_status INTEGER,
                attempt_count INTEGER DEFAULT 0,
                next_retry_at TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                delivered_at TEXT,
                FOREIGN KEY (webhook_id) REFERENCES webhooks(id)
            );
            CREATE INDEX IF NOT EXISTS idx_deliveries_pending
                ON webhook_deliveries(status, next_retry_at)
                WHERE status IN ('pending', 'failed');",
        )?;
        info!(target: "4da::db", "Created enterprise webhook tables");
        Ok(())
    }

    /// Phase 32: Enterprise organization + retention policies
    fn migrate_to_phase_32(conn: &Connection) -> SqliteResult<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS organizations (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                license_key_hash TEXT,
                settings TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS org_teams (
                org_id TEXT NOT NULL,
                team_id TEXT NOT NULL,
                PRIMARY KEY (org_id, team_id)
            );

            CREATE TABLE IF NOT EXISTS org_admins (
                org_id TEXT NOT NULL,
                member_id TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'org_admin',
                PRIMARY KEY (org_id, member_id)
            );

            CREATE TABLE IF NOT EXISTS retention_policies (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                retention_days INTEGER NOT NULL,
                updated_at TEXT DEFAULT (datetime('now')),
                UNIQUE(team_id, resource_type)
            );",
        )?;
        info!(target: "4da::db", "Created enterprise organization + retention tables");
        Ok(())
    }

    /// Phase 33: SSO pending auth table for OIDC state/nonce validation.
    fn migrate_to_phase_33(conn: &Connection) -> SqliteResult<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sso_pending_auth (
                id TEXT PRIMARY KEY,
                state TEXT NOT NULL UNIQUE,
                nonce TEXT NOT NULL,
                provider_type TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_sso_pending_state ON sso_pending_auth(state);
            CREATE INDEX IF NOT EXISTS idx_sso_pending_expires ON sso_pending_auth(expires_at);",
        )?;
        info!(target: "4da::db", "Created SSO pending auth table");
        Ok(())
    }

    fn migrate_to_phase_35(conn: &Connection) -> SqliteResult<()> {
        conn.execute_batch(
            "-- Accuracy tracking (Phase 4.1)
            CREATE TABLE IF NOT EXISTS accuracy_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                period TEXT NOT NULL UNIQUE,
                total_scored INTEGER NOT NULL DEFAULT 0,
                total_relevant INTEGER NOT NULL DEFAULT 0,
                user_confirmed INTEGER DEFAULT 0,
                user_rejected INTEGER DEFAULT 0,
                accuracy_pct REAL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            -- Developer temporal graph (Phase 4.5)
            CREATE TABLE IF NOT EXISTS developer_timeline (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                period TEXT NOT NULL UNIQUE,
                tech_snapshot TEXT NOT NULL,
                interest_snapshot TEXT NOT NULL,
                decision_count INTEGER DEFAULT 0,
                feedback_count INTEGER DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_timeline_period ON developer_timeline(period);

            -- AI usage tracking (Phase 8.2)
            CREATE TABLE IF NOT EXISTS ai_usage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                task_type TEXT NOT NULL,
                tokens_in INTEGER DEFAULT 0,
                tokens_out INTEGER DEFAULT 0,
                estimated_cost_usd REAL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_ai_usage_provider ON ai_usage(provider, model);
            CREATE INDEX IF NOT EXISTS idx_ai_usage_task ON ai_usage(task_type);
            CREATE INDEX IF NOT EXISTS idx_ai_usage_date ON ai_usage(created_at);",
        )?;
        info!(target: "4da::db", "Created Developer OS Intelligence tables (accuracy, timeline, AI usage)");
        Ok(())
    }

    fn migrate_to_phase_36(conn: &Connection) -> SqliteResult<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS waitlist_signups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tier TEXT NOT NULL,
                email TEXT NOT NULL,
                name TEXT,
                team_size TEXT,
                company TEXT,
                role TEXT,
                source TEXT DEFAULT 'in-app',
                signed_up_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(email, tier)
            );
            CREATE INDEX IF NOT EXISTS idx_waitlist_tier ON waitlist_signups(tier);",
        )?;
        info!(target: "4da::db", "Created waitlist signups table");
        Ok(())
    }

    fn migrate_to_phase_34(conn: &Connection) -> SqliteResult<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS user_dependencies (
                id INTEGER PRIMARY KEY,
                project_path TEXT NOT NULL,
                package_name TEXT NOT NULL,
                version TEXT,
                ecosystem TEXT NOT NULL,
                is_dev INTEGER DEFAULT 0,
                is_direct INTEGER DEFAULT 1,
                detected_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(project_path, package_name, ecosystem)
            );
            CREATE INDEX IF NOT EXISTS idx_user_deps_package ON user_dependencies(package_name);
            CREATE INDEX IF NOT EXISTS idx_user_deps_ecosystem ON user_dependencies(ecosystem);

            CREATE TABLE IF NOT EXISTS dependency_alerts (
                id INTEGER PRIMARY KEY,
                package_name TEXT NOT NULL,
                ecosystem TEXT NOT NULL,
                alert_type TEXT NOT NULL,
                severity TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT,
                affected_versions TEXT,
                source_url TEXT,
                source_item_id INTEGER,
                detected_at TEXT NOT NULL DEFAULT (datetime('now')),
                resolved_at TEXT,
                FOREIGN KEY (source_item_id) REFERENCES source_items(id)
            );
            CREATE INDEX IF NOT EXISTS idx_dep_alerts_package ON dependency_alerts(package_name, ecosystem);
            CREATE INDEX IF NOT EXISTS idx_dep_alerts_severity ON dependency_alerts(severity);",
        )?;
        info!(target: "4da::db", "Created Dependency Intelligence tables");
        Ok(())
    }

    fn migrate_to_phase_37(conn: &Connection) -> SqliteResult<()> {
        let has_license: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('user_dependencies') WHERE name='license'",
                [],
                |row| row.get::<_, i64>(0).map(|count| count > 0),
            )
            .unwrap_or(false);
        if !has_license {
            conn.execute("ALTER TABLE user_dependencies ADD COLUMN license TEXT", [])?;
        }
        info!(target: "4da::db", "Added license column to user_dependencies");
        Ok(())
    }

    fn migrate_to_phase_38(conn: &Connection) -> SqliteResult<()> {
        // Clean up abandoned feature tables (coach system + video curriculum)
        // These were created in earlier migrations but never used in production.
        // The other dead tables (chunk_sentiment, query_cache, query_history,
        // file_metadata_cache, item_relationships, git_commit_history) were
        // already dropped in Phase 26.
        conn.execute_batch(
            "DROP TABLE IF EXISTS coach_messages;
             DROP TABLE IF EXISTS coach_documents;
             DROP TABLE IF EXISTS coach_nudges;
             DROP TABLE IF EXISTS video_curriculum;",
        )?;
        info!(target: "4da::db", "Cleaned up 4 abandoned feature tables");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::test_db;

    #[test]
    fn test_fresh_db_has_all_expected_tables() {
        let db = test_db();
        let conn = db.conn.lock();
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        let expected = [
            "channels",
            "channel_renders",
            "channel_provenance",
            "channel_source_matches",
            "context_chunks",
            "source_items",
            "temporal_events",
            "feedback",
            "sources",
            "schema_version",
            "migration_history",
            "source_health",
            "briefings",
            "void_positions",
            "team_sync_queue",
            "team_sync_log",
            "team_sync_state",
            "team_crypto",
            "team_members_cache",
            // Phase 28: Team intelligence
            "shared_resources",
            "team_decisions",
            "decision_votes",
            // Phase 29: Team monitoring
            "team_signals",
            "team_alert_policies",
            // Phase 30: Enterprise audit
            "audit_log",
            // Phase 31: Enterprise webhooks
            "webhooks",
            "webhook_deliveries",
            // Phase 32: Enterprise organization
            "organizations",
            "org_teams",
            "org_admins",
            "retention_policies",
            // Phase 33: SSO pending auth
            "sso_pending_auth",
            // Phase 34: Dependency Intelligence
            "user_dependencies",
            "dependency_alerts",
            // Phase 39: Briefing history
            "briefing_item_history",
            // Phase 40: Necessity scoring persistence
            "item_necessity",
            // Phase 41: Content analysis cache
            "content_analyses",
            // Phase 43: Multilingual content translation cache
            "translation_cache",
            // Phase 52: Trust Ledger
            "trust_events",
            "precision_stats",
            "preemption_wins",
        ];
        for table in &expected {
            assert!(
                tables.iter().any(|t| t == table),
                "Expected table '{}' not found in {:?}",
                table,
                tables
            );
        }
    }

    #[test]
    fn test_migrations_are_idempotent() {
        let db = test_db();
        // Running migrate() again should not error
        let result = db.migrate();
        assert!(
            result.is_ok(),
            "Second migrate() call failed: {:?}",
            result.err()
        );
    }

    /// Phase 56: Intelligence Mesh provenance table exists after migration,
    /// has the expected schema, and has the expected indexes.
    #[test]
    fn test_phase_56_provenance_table_and_indexes() {
        let db = test_db();
        let conn = db.conn.lock();

        // Table exists.
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='provenance'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            table_exists,
            "provenance table should exist after migration"
        );

        // Expected columns present.
        let mut stmt = conn
            .prepare("SELECT name FROM pragma_table_info('provenance')")
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        let expected_cols = [
            "id",
            "artifact_kind",
            "artifact_id",
            "model_identity_hash",
            "provider",
            "model",
            "prompt_version",
            "calibration_id",
            "task",
            "temperature",
            "raw_response_hash",
            "shadow_peer_id",
            "created_at",
        ];
        for col in expected_cols {
            assert!(
                cols.iter().any(|c| c == col),
                "provenance column '{}' missing; got {:?}",
                col,
                cols
            );
        }

        // All four indexes created.
        let mut idx_stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='provenance'")
            .unwrap();
        let indexes: Vec<String> = idx_stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        for idx in [
            "idx_provenance_artifact",
            "idx_provenance_model",
            "idx_provenance_created_at",
            "idx_provenance_task",
        ] {
            assert!(
                indexes.iter().any(|i| i == idx),
                "provenance index '{}' missing; got {:?}",
                idx,
                indexes
            );
        }
    }

    /// Phase 57 lifts TARGET_VERSION to 57. Verify the test DB reached it.
    #[test]
    fn test_phase_57_schema_version_reached() {
        let db = test_db();
        let conn = db.conn.lock();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert!(
            version >= 57,
            "schema_version should be >= 57 after migration; got {}",
            version
        );
    }

    /// Phase 57: calibration_samples table + indices present.
    #[test]
    fn test_phase_57_calibration_samples_table_and_indexes() {
        let db = test_db();
        let conn = db.conn.lock();

        // Table exists.
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='calibration_samples'",
                [],
                |row| row.get::<_, i64>(0).map(|c| c > 0),
            )
            .unwrap_or(false);
        assert!(
            table_exists,
            "calibration_samples table should exist after migration"
        );

        // Expected columns all present.
        let mut stmt = conn
            .prepare("SELECT name FROM pragma_table_info('calibration_samples')")
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        for col in [
            "id",
            "source_item_id",
            "model_identity_hash",
            "task",
            "prompt_version",
            "raw_score",
            "confidence",
            "created_at",
            "processed_at",
        ] {
            assert!(
                cols.iter().any(|c| c == col),
                "calibration_samples column '{}' missing; got {:?}",
                col,
                cols
            );
        }

        // All 3 indices present.
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='calibration_samples'",
            )
            .unwrap();
        let indexes: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        for idx in [
            "idx_cal_samples_item",
            "idx_cal_samples_unfit",
            "idx_cal_samples_created",
        ] {
            assert!(
                indexes.iter().any(|i| i == idx),
                "calibration_samples index '{}' missing; got {:?}",
                idx,
                indexes
            );
        }
    }

    #[test]
    fn test_migration_version_tracked() {
        let db = test_db();
        let conn = db.conn.lock();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert!(version > 0, "Schema version should be > 0, got {}", version);
    }

    #[test]
    fn test_vec0_virtual_table_exists() {
        let db = test_db();
        let conn = db.conn.lock();
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name IN ('context_vec', 'source_vec') ORDER BY name")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(
            tables.contains(&"context_vec".to_string()),
            "context_vec virtual table not found"
        );
        assert!(
            tables.contains(&"source_vec".to_string()),
            "source_vec virtual table not found"
        );
    }

    #[test]
    fn test_all_expected_indexes_exist() {
        let db = test_db();
        let conn = db.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='source_items' ORDER BY name",
            )
            .unwrap();
        let indexes: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        let expected_indexes = [
            "idx_source_type",
            "idx_source_hash",
            "idx_source_seen",
            "idx_source_type_created",
        ];
        for idx in &expected_indexes {
            assert!(
                indexes.iter().any(|i| i == idx),
                "Expected index '{}' not found on source_items. Found: {:?}",
                idx,
                indexes
            );
        }
    }
}
