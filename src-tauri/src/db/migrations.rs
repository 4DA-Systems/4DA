// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Database migrations — schema versioning, backup, and migration orchestration.

use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use rusqlite::{params, Connection, Result as SqliteResult};
use std::collections::HashMap;
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
/// **Cold-boot contract:** this function is safe to run before
/// `Database::new(db_path)` only because the integrity probe uses a short
/// busy timeout and treats lock contention as transient, not corruption. A
/// running GUI/headless engine must never make startup wait indefinitely or
/// quarantine a merely-locked database.
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
            if let Err(e) = conn.busy_timeout(std::time::Duration::from_millis(250)) {
                tracing::warn!(
                    target: "4da::db::recovery",
                    path = %db_path.display(),
                    error = %e,
                    "Could not set DB recovery busy timeout"
                );
            }
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
                Err(e) if is_lock_contention(&e) => {
                    tracing::warn!(
                        target: "4da::db::recovery",
                        path = %db_path.display(),
                        error = %e,
                        "DB recovery quick_check skipped because database is locked"
                    );
                    return CorruptionRecovery::RecoveryFailed {
                        reason: format!("database locked during recovery quick_check: {e}"),
                    };
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
        Err(e) if is_lock_contention(&e) => {
            tracing::warn!(
                target: "4da::db::recovery",
                path = %db_path.display(),
                error = %e,
                "DB recovery skipped because database is locked"
            );
            return CorruptionRecovery::RecoveryFailed {
                reason: format!("database locked during recovery open: {e}"),
            };
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

fn is_lock_contention(e: &rusqlite::Error) -> bool {
    matches!(
        e.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    )
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
mod backup_prune_tests {
    use super::*;
    use tempfile::tempdir;

    /// The pre-migration backup must SURVIVE its own prune.
    ///
    /// Regression: the prune sorted paths as strings, so once the schema
    /// reached three digits `"…v100" < "…v98" < "…v99"` and the newest backup
    /// sorted first — straight into the delete slice. Observed live on
    /// 2026-07-26: Phase 101 wrote `4da.db.backup.v100` (2.45 GB) and deleted it
    /// 216 ms later, leaving the migration with no rollback point while two
    /// older backups survived.
    #[test]
    fn pre_migration_backup_survives_its_own_prune() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("4da.db");
        let db = Database::new(&db_path).expect("open temp db");

        // Two older backups already on disk — enough that the prune engages.
        for v in [98, 99] {
            std::fs::write(dir.path().join(format!("4da.db.backup.v{v}")), b"old").unwrap();
        }

        db.backup_before_migration(100);

        let fresh = dir.path().join("4da.db.backup.v100");
        assert!(
            fresh.exists(),
            "the backup just created was pruned — the migration has no rollback point"
        );
        // Keep-2 semantics, resolved on the VERSION axis: v99 + v100 survive,
        // v98 (the genuinely oldest) is the one that goes.
        assert!(
            dir.path().join("4da.db.backup.v99").exists(),
            "v99 is one of the two most recent and must be kept"
        );
        assert!(
            !dir.path().join("4da.db.backup.v98").exists(),
            "v98 is the oldest of three and should have been pruned"
        );
    }

    /// A backup file whose suffix does not parse as a version is left alone
    /// rather than sorted to an arbitrary slot where it could be deleted.
    #[test]
    fn unparseable_backup_names_are_never_pruned() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("4da.db");
        let db = Database::new(&db_path).expect("open temp db");

        let odd = dir.path().join("4da.db.backup.vOLD");
        std::fs::write(&odd, b"manual").unwrap();
        for v in [98, 99] {
            std::fs::write(dir.path().join(format!("4da.db.backup.v{v}")), b"old").unwrap();
        }

        db.backup_before_migration(100);

        assert!(odd.exists(), "unparseable backup name must not be deleted");
    }

    fn names(files: &[(&str, i64)]) -> Vec<(String, i64)> {
        files.iter().map(|(n, t)| ((*n).to_string(), *t)).collect()
    }

    /// Hand-made `.bak-pre-*` snapshots were invisible to the pruner, which only ever
    /// understood `.db.backup.vN`. Eight of them accumulated (up to 1.57 GB each) beside
    /// a 203 MB database; D: hit zero bytes free and killed a build.
    #[test]
    fn manual_bak_pre_snapshots_are_pruned_newest_first() {
        let files = names(&[
            ("4da.db.bak-pre-v5-20260801", 100),
            ("4da.db.bak-pre-v6-20260805", 200),
            ("4da.db.bak-pre-v7-20260809", 300),
            ("4da.db.bak-pre-v8-20260812", 400),
        ]);
        let doomed = plan_backup_pruning(&files, "4da.db", BACKUP_RETENTION_COUNT, None);
        assert_eq!(
            doomed,
            vec![
                "4da.db.bak-pre-v5-20260801".to_string(),
                "4da.db.bak-pre-v6-20260805".to_string()
            ],
            "the two oldest manual snapshots go, the two newest stay"
        );
    }

    /// Each family gets its own retention slots — a full `.bak-pre-*` shelf must never
    /// evict the versioned backups a migration rolls back to, or vice versa.
    #[test]
    fn families_are_pruned_independently() {
        let files = names(&[
            ("4da.db.backup.v98", 0),
            ("4da.db.backup.v99", 0),
            ("4da.db.backup.v100", 0),
            ("4da.db.bak-pre-a", 100),
            ("4da.db.bak-pre-b", 200),
            ("4da.db.bak-pre-c", 300),
            ("4da.db-wal.bak-pre-c", 350),
            ("4da.db.corrupt", 10),
            ("4da.db.corrupt-1754000000", 20),
            ("4da.db.corrupt-1755000000", 30),
        ]);
        let doomed = plan_backup_pruning(&files, "4da.db", BACKUP_RETENTION_COUNT, None);
        assert_eq!(
            doomed,
            vec![
                "4da.db.backup.v98".to_string(),
                "4da.db.bak-pre-a".to_string(),
            ],
            "the oldest of each COLLECTABLE family — quarantine copies are never collected"
        );
    }

    /// A hand-made snapshot and its `-wal` sibling are one backup and must be kept or
    /// dropped together — restoring a database without its WAL, or a WAL without its
    /// database, is worse than having neither. Counting the sibling as its own retention
    /// slot (the first cut of this pruner) both split pairs and silently halved how many
    /// real backups survived.
    #[test]
    fn a_snapshot_and_its_wal_sibling_prune_as_one_unit() {
        let files = names(&[
            ("4da.db.bak-pre-a", 100),
            ("4da.db-wal.bak-pre-a", 110),
            ("4da.db.bak-pre-b", 200),
            ("4da.db-wal.bak-pre-b", 210),
            ("4da.db.bak-pre-c", 300),
            ("4da.db-wal.bak-pre-c", 310),
        ]);
        let doomed = plan_backup_pruning(&files, "4da.db", BACKUP_RETENTION_COUNT, None);
        assert_eq!(
            doomed,
            vec![
                "4da.db-wal.bak-pre-a".to_string(),
                "4da.db.bak-pre-a".to_string()
            ],
            "the oldest PAIR goes whole; two complete pairs survive"
        );

        // Same tag from every spelling the operator uses lands in one unit.
        for name in ["4da.db.bak-pre-a", "4da.db-wal.bak-pre-a", "4da.bak-pre-a"] {
            assert_eq!(
                classify_backup(name, "4da.db").map(|(_, _, unit)| unit),
                Some("-pre-a".to_string()),
                "{name} must share the -pre-a retention unit"
            );
        }
    }

    /// Quarantine copies are recognised but NEVER deleted.
    ///
    /// An earlier cut of this pruner collected them to reclaim disk. That is unsafe: a
    /// quarantine copy is a database the app could not open, so it is the user's only
    /// copy of that data — and when the corrupt-database fallback misfires on a
    /// perfectly good database (a schema written by a newer 4DA), the quarantined file
    /// *is* the entire live corpus. Reclaiming 338 MB is not worth a chance of deleting
    /// 15,659 items.
    #[test]
    fn quarantine_copies_are_never_pruned() {
        let files = names(&[
            ("4da.db.corrupt", 0),
            ("4da.db.corrupt-1", 1),
            ("4da.db.corrupt-2", 2),
            ("4da.db.corrupt-3", 3),
            ("4da.db.corrupt-4", 4),
        ]);
        let doomed = plan_backup_pruning(&files, "4da.db", BACKUP_RETENTION_COUNT, None);
        assert!(
            doomed.is_empty(),
            "quarantined databases must survive the pruner; got {doomed:?}"
        );
        // Still classified, so the caller can report the disk they hold.
        assert_eq!(
            classify_backup("4da.db.corrupt-1", "4da.db").map(|(f, _, _)| f),
            Some(BackupFamily::Quarantine)
        );
    }

    /// The pruner only ever considers copies of THIS database. A sibling backup of some
    /// other file in the data directory is not ours to delete.
    #[test]
    fn unrelated_siblings_are_never_candidates() {
        for name in [
            "settings.json.bak-pre-v8",
            "settings.json.corrupt",
            "4da.db",
            "4da.db-wal",
            "4da.db-shm",
            "4da.db.bakery",
            "notes.txt",
        ] {
            assert!(
                classify_backup(name, "4da.db").is_none(),
                "{name} must not be classified as a prunable backup"
            );
        }
    }

    /// Whatever the ordering says, the file the caller just wrote is off limits.
    #[test]
    fn protected_name_is_never_pruned() {
        let files = names(&[
            ("4da.db.bak-pre-a", 100),
            ("4da.db.bak-pre-b", 200),
            ("4da.db.bak-pre-c", 300),
        ]);
        let doomed = plan_backup_pruning(
            &files,
            "4da.db",
            BACKUP_RETENTION_COUNT,
            Some("4da.db.bak-pre-a"),
        );
        assert!(doomed.is_empty(), "protected file must survive: {doomed:?}");
    }

    /// End-to-end through the filesystem: the manual family really is collected now.
    #[test]
    fn manual_snapshots_are_pruned_on_disk() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("4da.db");
        let db = Database::new(&db_path).expect("open temp db");

        for n in ["a", "b", "c"] {
            std::fs::write(dir.path().join(format!("4da.db.bak-pre-{n}")), b"snap").unwrap();
            // Distinct mtimes so "newest" is well-defined. The planner orders on
            // milliseconds, so a short sleep is enough.
            std::thread::sleep(std::time::Duration::from_millis(25));
        }

        db.backup_before_migration(100);

        let survivors: Vec<bool> = ["a", "b", "c"]
            .iter()
            .map(|n| dir.path().join(format!("4da.db.bak-pre-{n}")).exists())
            .collect();
        assert_eq!(
            survivors,
            vec![false, true, true],
            "oldest manual snapshot pruned, two newest kept"
        );
        assert!(
            dir.path().join("4da.db.backup.v100").exists(),
            "the versioned backup is a different family and must be untouched"
        );
    }
}

#[cfg(test)]
mod schema_version_guard_tests {
    use super::*;
    use tempfile::tempdir;

    /// A binary older than the database it opens must be refused, not tolerated.
    ///
    /// This guard has existed since 2026-03-29 and was never tested, which is worth
    /// fixing now because schema 104 raised the cost of it failing. `source_items_fts`
    /// is maintained by triggers from 104 onward; a pre-104 binary still runs its own
    /// `INSERT OR REPLACE INTO source_items_fts`, and that write landing on top of the
    /// trigger's leaves the index failing FTS5 `('integrity-check', 1)` **while search
    /// results still look correct** — measured on a schema-104 fixture, not assumed.
    ///
    /// So this refusal is the only thing standing between a stale build and silent index
    /// divergence. On this fleet that is not hypothetical: the scheduled background
    /// refresh runs `target/debug/fourda.exe`, which is whatever was last compiled there.
    #[test]
    fn a_database_newer_than_the_binary_is_refused() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("4da.db");

        {
            let db = Database::new(&db_path).expect("first open migrates to current schema");
            db.conn
                .lock()
                .execute(
                    "UPDATE schema_version SET version = ?1",
                    [TEST_FUTURE_VERSION],
                )
                .expect("stamp a future schema version");
        }

        // `Database` is not `Debug`, so unwrap the error side by hand rather than
        // `expect_err`.
        let refused = Database::new(&db_path);
        assert!(
            refused.is_err(),
            "a database newer than the binary must be refused, not silently accepted"
        );
        let msg = refused.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(
            msg.contains("newer than this version of 4DA supports"),
            "the refusal must say WHY, so a stale build is diagnosable from one log line; got: {msg}"
        );
    }

    /// The common case must still work: reopening a database this binary wrote is fine.
    /// Without this, the guard above could pass while being far too aggressive.
    #[test]
    fn a_database_at_the_current_schema_reopens_cleanly() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("4da.db");
        drop(Database::new(&db_path).expect("first open"));
        let db = Database::new(&db_path).expect("reopen at the same schema must succeed");
        db.fts_integrity_check()
            .expect("a reopened database must still have a consistent FTS index");
    }

    /// Far enough ahead that no future migration can accidentally reach it.
    const TEST_FUTURE_VERSION: i64 = 9_999;
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
    fn locked_db_returns_recovery_failed_without_quarantine() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("locked.db");
        let locker = Connection::open(&path).unwrap();
        locker
            .execute_batch(
                "
                PRAGMA journal_mode = DELETE;
                CREATE TABLE t (x INTEGER);
                INSERT INTO t VALUES (1);
                BEGIN EXCLUSIVE;
                INSERT INTO t VALUES (2);
                ",
            )
            .unwrap();

        let started = std::time::Instant::now();
        let result = recover_corrupt_db_if_needed(&path);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "locked DB recovery must return quickly, elapsed {:?}",
            started.elapsed()
        );

        match result {
            CorruptionRecovery::RecoveryFailed { reason } => {
                assert!(reason.contains("locked"), "unexpected reason: {reason}");
            }
            other => panic!("expected RecoveryFailed for locked DB, got {other:?}"),
        }
        assert!(path.exists(), "locked DB must not be moved");
        let quarantine_files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains("corrupt-"))
            .collect();
        assert!(
            quarantine_files.is_empty(),
            "locked DB must not be quarantined"
        );

        let _ = locker.execute_batch("ROLLBACK;");
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

/// The one phrase that identifies a "database written by a newer 4DA" refusal.
///
/// Shared by the producer (`migrate`) and the detector
/// (`state.rs::is_schema_newer_than_binary`) so the two cannot drift apart. They must not:
/// if the caller fails to recognise this error it falls into the corrupt-database
/// fallback, which renames the user's entire corpus to `.db.corrupt` and starts empty.
/// Measured on a 296 MB / 15,659-item database: it came back as 0 items.
pub(crate) const SCHEMA_TOO_NEW_PHRASE: &str = "is newer than this version of 4DA supports";

/// How many files of each backup family survive a prune.
pub(crate) const BACKUP_RETENTION_COUNT: usize = 2;

/// The families of database-sized files that accumulate next to `4da.db`, each pruned
/// independently so a full slot in one never evicts the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BackupFamily {
    /// `<db>.backup.v<N>` — written automatically by [`Database::backup_before_migration`].
    /// Ordered by the parsed schema version.
    Versioned,
    /// `<db>.bak-*`, `<db>-wal.bak*`, `<db>-shm.bak*` — snapshots taken by hand before a
    /// risky migration or a dogfood run (the `.gitignore` calls these "operator backups").
    /// No version axis, so ordered by mtime.
    Manual,
    /// `<db>.corrupt`, `<db>.corrupt-<unix>` — corruption quarantine copies.
    ///
    /// **Never auto-pruned.** These are classified so the pruner can *report* the disk
    /// they hold, not reclaim it. A quarantine copy is by definition a database the app
    /// could not open, which makes it the user's only copy of that data — and a
    /// misclassified one can be their entire live corpus (see
    /// [`SCHEMA_TOO_NEW_PHRASE`]). Deleting it is unrecoverable, so it stays until a
    /// human decides. Ordered by mtime for reporting only.
    Quarantine,
}

/// Classify a file name in the data directory, returning its family, the schema version
/// encoded in the name (`Versioned` only, `0` otherwise), and the **retention unit** it
/// belongs to — the set of files that are kept or deleted together.
///
/// Everything is matched against the live database's own file name, so a sibling like
/// `settings.json.bak-2026-08-12` is never a candidate — only copies of *this* database.
/// `4da.db`, `4da.db-wal` and `4da.db-shm` themselves match nothing.
pub(crate) fn classify_backup(
    name: &str,
    db_file_name: &str,
) -> Option<(BackupFamily, i64, String)> {
    if let Some(rest) = name.strip_prefix(&format!("{db_file_name}.backup.v")) {
        let version: i64 = rest.parse().ok()?;
        return Some((BackupFamily::Versioned, version, name.to_string()));
    }

    // A hand-made snapshot and its `-wal`/`-shm` siblings share everything after the
    // `.bak` marker (`4da.db.bak-pre-v8-…` next to `4da.db-wal.bak-pre-v8-…`), so that
    // shared tag is the retention unit: half a backup restores worse than none. The
    // shorter `4da.bak-…` spelling the operator has also used lands in the same unit.
    let stem = db_file_name
        .rsplit_once('.')
        .map_or(db_file_name, |(s, _)| s);
    for prefix in [
        format!("{db_file_name}.bak"),
        format!("{db_file_name}-wal.bak"),
        format!("{db_file_name}-shm.bak"),
        format!("{stem}.bak"),
    ] {
        if let Some(tag) = name.strip_prefix(&prefix) {
            // Require a separator so `4da.db.bakery` is not mistaken for a backup.
            if tag.is_empty() || tag.starts_with(['-', '.', '_']) {
                return Some((BackupFamily::Manual, 0, tag.to_string()));
            }
        }
    }

    if name == format!("{db_file_name}.corrupt")
        || name.starts_with(&format!("{db_file_name}.corrupt-"))
    {
        return Some((BackupFamily::Quarantine, 0, name.to_string()));
    }
    None
}

/// Decide which backup files to remove, keeping the newest `keep` retention *units* of
/// each family. A unit is usually one file; for a hand-made snapshot it is the snapshot
/// plus its `-wal`/`-shm` siblings, which are only useful together.
///
/// Pure so the retention rule can be tested without a filesystem — the previous prune bug
/// (lexicographic sort silently deleting the backup it had just written, from schema 100
/// onward) survived because the rule was only reachable through real `read_dir` output.
///
/// `files` is `(file_name, mtime_millis)`; a unit is ordered by its newest member.
/// `protect` is never returned, whatever the ordering says. Output is sorted for
/// determinism.
pub(crate) fn plan_backup_pruning(
    files: &[(String, i64)],
    db_file_name: &str,
    keep: usize,
    protect: Option<&str>,
) -> Vec<String> {
    // family -> unit key -> (newest order key, member file names)
    let mut families: HashMap<BackupFamily, HashMap<String, (i64, Vec<&str>)>> = HashMap::new();
    for (name, mtime) in files {
        let Some((family, version, unit)) = classify_backup(name, db_file_name) else {
            continue;
        };
        let order = if family == BackupFamily::Versioned {
            version
        } else {
            *mtime
        };
        let entry = families
            .entry(family)
            .or_default()
            .entry(unit)
            .or_insert((i64::MIN, Vec::new()));
        entry.0 = entry.0.max(order);
        entry.1.push(name.as_str());
    }

    // Quarantine copies are classified but never collected — see [`BackupFamily::Quarantine`].
    families.remove(&BackupFamily::Quarantine);

    let mut doomed: Vec<String> = Vec::new();
    for units in families.values() {
        let mut ordered: Vec<(i64, &str, &[&str])> = units
            .iter()
            .map(|(unit, (order, members))| (*order, unit.as_str(), members.as_slice()))
            .collect();
        // Newest last. Tie-break on the unit key so equal mtimes prune deterministically.
        ordered.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
        if ordered.len() <= keep {
            continue;
        }
        let cutoff = ordered.len() - keep;
        for (_, _, members) in &ordered[..cutoff] {
            for name in *members {
                if Some(*name) == protect {
                    continue;
                }
                doomed.push((*name).to_string());
            }
        }
    }
    doomed.sort();
    doomed
}

impl Database {
    /// Create a pre-migration backup of the database file.
    /// Keeps only the last [`BACKUP_RETENTION_COUNT`] backups of each family.
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
        self.prune_old_backups(
            backup_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned()),
        );
    }

    /// Prune stale database-sized files beside `4da.db`, keeping the newest
    /// [`BACKUP_RETENTION_COUNT`] of each [`BackupFamily`].
    ///
    /// Historically this only understood `<db>.backup.v<N>` and sorted PATHS
    /// lexicographically — correct only while every version had the same digit count. As
    /// strings, "…v100" < "…v98" < "…v99", so from schema 100 onward the newest backup
    /// sorted FIRST and the prune deleted the very file it had just written. Live evidence
    /// (2026-07-26, Phase 101 on the 2.4 GB corpus): `Pre-migration backup created … v100
    /// bytes=2451460096`, then `Pruned old backup … v100` 216 ms later — the migration ran
    /// with NO rollback point while v98 (4 days old) and v99 survived.
    ///
    /// It also collected *only* that one family, so the two unbounded ones grew forever:
    /// hand-made `.bak-pre-*` snapshots (eight of them, up to 1.57 GB each) and
    /// `.db.corrupt-<unix>` quarantine copies, which the corruption-recovery path writes
    /// one of per incident. Roughly 10 GB of them sat beside a 203 MB database, and D:
    /// reached zero bytes free and killed a build.
    ///
    /// Deliberately conservative: matched strictly against this database's own file name
    /// (a sibling `settings.json.bak-…` is never a candidate), an unparseable suffix is
    /// left alone rather than sorted to an arbitrary position, a file whose mtime cannot
    /// be read is treated as newest rather than oldest, and the backup this migration just
    /// wrote is protected whatever the ordering says.
    fn prune_old_backups(&self, protect: Option<String>) {
        let Some(parent) = self.db_path.parent() else {
            return;
        };
        let Some(db_file_name) = self.db_path.file_name().map(|n| n.to_string_lossy()) else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(parent) else {
            return;
        };

        let files: Vec<(String, i64)> = entries
            .filter_map(|e| match e {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in db_migrations: {e}");
                    None
                }
            })
            .map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                // An unreadable mtime sorts NEWEST (i64::MAX), so an unreadable file is
                // kept rather than deleted.
                let mtime = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(i64::MAX, |d| d.as_millis() as i64);
                (name, mtime)
            })
            .collect();

        for name in plan_backup_pruning(
            &files,
            &db_file_name,
            BACKUP_RETENTION_COUNT,
            protect.as_deref(),
        ) {
            let path = parent.join(&name);
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    info!(target: "4da::db", path = %path.display(), "Pruned old backup");
                }
                Err(e) => {
                    tracing::warn!(
                        target: "4da::db",
                        path = %path.display(),
                        error = %e,
                        "Failed to prune old backup"
                    );
                }
            }
        }
    }

    /// Install the three triggers that keep `source_items_fts` in lockstep with
    /// `source_items`, then rebuild the index from scratch.
    ///
    /// `source_items_fts` is an **external content** FTS5 table (`content='source_items'`),
    /// which means SQLite stores only the inverted index — never the text — and does
    /// **nothing** automatically. Every insert, update and delete on `source_items` has
    /// to be mirrored into the index by hand, and until now that was done by hand:
    ///
    /// - `batch_upsert_pending_source_items` wrote rows with no FTS statement at all,
    ///   so anything that failed embedding was invisible to search forever.
    /// - Nothing anywhere issued an FTS `'delete'`. Retention (`cleanup_old_items`,
    ///   `run_maintenance`, `prune_noise`) and the cascade trigger all removed rows
    ///   from `source_items` and left their postings behind.
    /// - The paths that *did* write used `INSERT OR REPLACE` **after** updating
    ///   `source_items`. On an external-content table the implicit REPLACE-delete reads
    ///   the old values back **from the content table**, which by then already held the
    ///   NEW text — so it deleted the new postings and stranded the old ones. Measured on
    ///   the founder's live 247 MB corpus: 2,631 divergent terms over 38 rows, and a
    ///   search for a word that had been *edited out* of an item still returned it.
    ///
    /// Hand-maintenance of an external-content index is a standing invitation to forget
    /// one path, and three paths were already forgotten. Triggers are the fix SQLite
    /// itself documents for this table type: they cannot be bypassed by a new call site,
    /// they see the pre-update values the `'delete'` command requires, and they let every
    /// upsert/cleanup function drop its FTS bookkeeping entirely.
    ///
    /// Two deliberate narrowings on the UPDATE trigger:
    /// - `OF title, content` — scoring stamps thousands of `relevance_score` /
    ///   `scored_pipeline_version` updates per drain and must not reindex anything.
    /// - `WHEN old IS NOT new` — a re-fetch rewrites identical text on nearly every
    ///   cycle; skipping those makes this strictly less write amplification than the
    ///   unconditional `INSERT OR REPLACE` it replaces.
    ///
    /// The closing `'rebuild'` discards the diverged index and regenerates it from
    /// `source_items`, so the triggers start from a correct index — an FTS `'delete'`
    /// carrying values that were never indexed would otherwise corrupt it further.
    /// Measured against a copy of the founder's 12,273-row / 247 MB database, the whole
    /// `Database::new` path (pre-migration backup copy included) took 5.9 s.
    pub(crate) fn install_fts_sync_triggers_and_rebuild(c: &Connection) -> SqliteResult<()> {
        // Defensive: Phase 78 creates this, but a trigger body referencing a missing
        // table fails at CREATE time and would wedge the migration in a retry loop.
        c.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS source_items_fts USING fts5(
                 title,
                 content,
                 content='source_items',
                 content_rowid='id',
                 tokenize='porter unicode61'
             );",
        )?;

        c.execute_batch(
            "DROP TRIGGER IF EXISTS trg_source_items_fts_insert;
             DROP TRIGGER IF EXISTS trg_source_items_fts_update;
             DROP TRIGGER IF EXISTS trg_source_items_fts_delete;

             CREATE TRIGGER trg_source_items_fts_insert
             AFTER INSERT ON source_items
             BEGIN
                 INSERT INTO source_items_fts(rowid, title, content)
                 VALUES (NEW.id, COALESCE(NEW.title, ''), COALESCE(NEW.content, ''));
             END;

             CREATE TRIGGER trg_source_items_fts_update
             AFTER UPDATE OF title, content ON source_items
             WHEN OLD.title IS NOT NEW.title OR OLD.content IS NOT NEW.content
             BEGIN
                 INSERT INTO source_items_fts(source_items_fts, rowid, title, content)
                 VALUES ('delete', OLD.id, COALESCE(OLD.title, ''), COALESCE(OLD.content, ''));
                 INSERT INTO source_items_fts(rowid, title, content)
                 VALUES (NEW.id, COALESCE(NEW.title, ''), COALESCE(NEW.content, ''));
             END;

             CREATE TRIGGER trg_source_items_fts_delete
             AFTER DELETE ON source_items
             BEGIN
                 INSERT INTO source_items_fts(source_items_fts, rowid, title, content)
                 VALUES ('delete', OLD.id, COALESCE(OLD.title, ''), COALESCE(OLD.content, ''));
             END;",
        )?;

        let started = std::time::Instant::now();
        c.execute_batch("INSERT INTO source_items_fts(source_items_fts) VALUES('rebuild');")?;
        info!(
            target: "4da::db",
            rebuild_ms = started.elapsed().as_millis() as i64,
            "Phase 104: FTS5 sync triggers installed and search index rebuilt"
        );
        Ok(())
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

            -- Generic key-value store. Historically created only by the ACE
            -- schema init (ace/db.rs) with `value REAL NOT NULL`, which meant
            -- (a) a headless-first or test database had no kv_store at all and
            -- (b) string flags were coerced to REAL by column affinity.
            -- Declared here typeless (BLOB affinity: stores TEXT as TEXT,
            -- numbers as numbers) so whichever init runs first, the table
            -- exists; Database::get_kv normalizes whatever affinity produced.
            CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY NOT NULL,
                value,
                updated_at TEXT DEFAULT (datetime('now'))
            );

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
        conn.execute_batch(&format!(
            "
            -- Vector index for context chunks ({dim}d embeddings)
            CREATE VIRTUAL TABLE IF NOT EXISTS context_vec USING vec0(
                embedding float[{dim}]
            );

            -- Vector index for source items ({dim}d embeddings)
            CREATE VIRTUAL TABLE IF NOT EXISTS source_vec USING vec0(
                embedding float[{dim}]
            );
        ",
            dim = crate::EMBEDDING_DIMS
        ))?;

        // Determine current schema version for backup decision
        let mut current_version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap_or(1);

        const TARGET_VERSION: i64 = 104;

        // Downgrade detection: if DB schema is newer than this binary expects,
        // show a clear error instead of silently corrupting the schema.
        //
        // The caller MUST distinguish this from corruption — see
        // [`SCHEMA_TOO_NEW_PHRASE`] and `state.rs::is_schema_newer_than_binary`.
        if current_version > TARGET_VERSION {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISMATCH),
                Some(format!(
                    "Database schema version {current_version} {SCHEMA_TOO_NEW_PHRASE} \
                     (max {TARGET_VERSION}). Your data has NOT been modified. \
                     You are running an older version of 4DA against a newer database — \
                     update 4DA to open it."
                )),
            ));
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

            // ── Phase 58: Commitment Contracts (Intelligence Reconciliation Phase 11) ──
            // Stores the user's "refutation conditions" — what would convince
            // them a decision was wrong. A background watcher monitors incoming
            // source items against these conditions and flips the contract's
            // status when a match fires.
            if current_version < 58 {
                Self::run_versioned_migration(
                    &conn,
                    57,
                    58,
                    "Phase 58: Commitment Contracts (Intelligence Reconciliation)",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS commitment_contracts (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                decision_statement TEXT NOT NULL,
                                refutation_condition TEXT NOT NULL,
                                subject TEXT NOT NULL DEFAULT '',
                                status TEXT NOT NULL DEFAULT 'active',
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                triggered_at TEXT,
                                trigger_item_id INTEGER,
                                FOREIGN KEY (trigger_item_id) REFERENCES source_items(id)
                             );
                             CREATE INDEX IF NOT EXISTS idx_contracts_status
                               ON commitment_contracts(status);
                             CREATE INDEX IF NOT EXISTS idx_contracts_subject
                               ON commitment_contracts(subject);",
                        )?;
                        info!(
                            target: "4da::db",
                            "Created commitment_contracts table + 2 indices (Intelligence Reconciliation Phase 11)"
                        );
                        Ok(())
                    },
                )?;
            }

            // ── Phase 59: Alert Triage (Trust & Credibility Tier 2) ──
            // Persistent security triage actions replacing localStorage-based
            // acknowledgment. Supports investigating/fixed/not_applicable/
            // accepted_risk/snoozed/acknowledged with audit trail and expiry.
            if current_version < 59 {
                Self::run_versioned_migration(
                    &conn,
                    58,
                    59,
                    "Phase 59: Alert Triage (persistent security triage actions)",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS alert_triage (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                item_id INTEGER NOT NULL,
                                advisory_id TEXT,
                                action TEXT NOT NULL CHECK(action IN ('investigating', 'fixed', 'not_applicable', 'accepted_risk', 'snoozed', 'acknowledged')),
                                reason TEXT,
                                resolved_at TEXT NOT NULL DEFAULT (datetime('now')),
                                expires_at TEXT,
                                UNIQUE(item_id)
                            );
                            CREATE INDEX IF NOT EXISTS idx_alert_triage_item ON alert_triage(item_id);
                            CREATE INDEX IF NOT EXISTS idx_alert_triage_expires ON alert_triage(expires_at) WHERE expires_at IS NOT NULL;",
                        )?;
                        info!(
                            target: "4da::db",
                            "Created alert_triage table + 2 indices (Trust & Credibility Tier 2)"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 60: Feed origin tracking for per-feed health
            if current_version < 60 {
                Self::run_versioned_migration(
                    &conn,
                    59,
                    60,
                    "Phase 60: feed_origin column on source_items",
                    |c| {
                        let has_column: bool = c
                            .query_row(
                                "SELECT COUNT(*) FROM pragma_table_info('source_items') WHERE name='feed_origin'",
                                [],
                                |row| row.get::<_, i64>(0).map(|count| count > 0),
                            )
                            .unwrap_or(false);
                        if !has_column {
                            c.execute_batch(
                                "ALTER TABLE source_items ADD COLUMN feed_origin TEXT;
                                 CREATE INDEX IF NOT EXISTS idx_source_feed_origin ON source_items(feed_origin);",
                            )?;
                            info!(target: "4da::db", "Added feed_origin column + index to source_items");
                        }
                        Ok(())
                    },
                )?;
            }

            // Phase 61: Per-feed health tracking for circuit breaking
            if current_version < 61 {
                Self::run_versioned_migration(
                    &conn,
                    60,
                    61,
                    "Phase 61: feed_health table for per-feed circuit breaking",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS feed_health (
                                feed_origin TEXT NOT NULL,
                                source_type TEXT NOT NULL,
                                consecutive_failures INTEGER NOT NULL DEFAULT 0,
                                total_successes INTEGER NOT NULL DEFAULT 0,
                                total_failures INTEGER NOT NULL DEFAULT 0,
                                last_success_at TEXT,
                                last_failure_at TEXT,
                                last_error TEXT,
                                circuit_open INTEGER NOT NULL DEFAULT 0,
                                circuit_opened_at TEXT,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                                PRIMARY KEY (feed_origin, source_type)
                            );
                            CREATE INDEX IF NOT EXISTS idx_feed_health_source_type ON feed_health(source_type);
                            CREATE INDEX IF NOT EXISTS idx_feed_health_circuit ON feed_health(circuit_open) WHERE circuit_open = 1;",
                        )?;
                        info!(target: "4da::db", "Created feed_health table with circuit breaker support");
                        Ok(())
                    },
                )?;
            }

            // Phase 62: Structured tags for source-fair topic extraction
            if current_version < 62 {
                Self::run_versioned_migration(
                    &conn,
                    61,
                    62,
                    "Phase 62: structured tags column for source-fair scoring",
                    |c| {
                        let has_column = c
                            .query_row(
                                "SELECT COUNT(*) FROM pragma_table_info('source_items') WHERE name = 'tags'",
                                [],
                                |row| row.get::<_, i64>(0).map(|count| count > 0),
                            )
                            .unwrap_or(false);
                        if !has_column {
                            c.execute_batch(
                                "ALTER TABLE source_items ADD COLUMN tags TEXT DEFAULT NULL;",
                            )?;
                            info!(target: "4da::db", "Added tags column to source_items for source-fair scoring");
                        }
                        Ok(())
                    },
                )?;
            }

            // Phase 63: Local OSV advisory mirror for Tier 1 verified intelligence
            if current_version < 63 {
                Self::run_versioned_migration(
                    &conn,
                    62,
                    63,
                    "Phase 63: local OSV advisory mirror",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS osv_advisories (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                advisory_id TEXT NOT NULL,
                                summary TEXT NOT NULL,
                                details TEXT,
                                package_name TEXT NOT NULL,
                                ecosystem TEXT NOT NULL,
                                affected_ranges TEXT,
                                fixed_versions TEXT,
                                severity_type TEXT,
                                cvss_score REAL,
                                source_url TEXT,
                                published_at TEXT,
                                modified_at TEXT,
                                synced_at TEXT NOT NULL DEFAULT (datetime('now')),
                                UNIQUE(advisory_id, package_name, ecosystem)
                            );
                            CREATE INDEX IF NOT EXISTS idx_osv_advisories_package
                                ON osv_advisories(package_name, ecosystem);
                            CREATE INDEX IF NOT EXISTS idx_osv_advisories_advisory
                                ON osv_advisories(advisory_id);
                            CREATE INDEX IF NOT EXISTS idx_osv_advisories_cvss
                                ON osv_advisories(cvss_score DESC);

                            CREATE TABLE IF NOT EXISTS osv_sync_status (
                                ecosystem TEXT PRIMARY KEY,
                                last_synced_at TEXT NOT NULL,
                                advisory_count INTEGER NOT NULL DEFAULT 0,
                                error TEXT
                            );",
                        )?;
                        info!(
                            target: "4da::db",
                            "Created osv_advisories + osv_sync_status tables (Tier 1 intelligence)"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 64: LLM judgment storage for Tier 2 intelligence
            if current_version < 64 {
                Self::run_versioned_migration(
                    &conn,
                    63,
                    64,
                    "Phase 64: LLM judgment storage for Tier 2 intelligence",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS llm_judgments (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                source_item_id INTEGER NOT NULL,
                                relevance_score REAL NOT NULL,
                                explanation TEXT NOT NULL,
                                actions TEXT,
                                confidence REAL NOT NULL,
                                model TEXT NOT NULL,
                                prompt_version TEXT NOT NULL DEFAULT 'v1',
                                judged_at TEXT NOT NULL DEFAULT (datetime('now')),
                                UNIQUE(source_item_id, prompt_version)
                            );
                            CREATE INDEX IF NOT EXISTS idx_llm_judgments_item
                                ON llm_judgments(source_item_id);
                            CREATE INDEX IF NOT EXISTS idx_llm_judgments_relevance
                                ON llm_judgments(relevance_score DESC);",
                        )?;
                        info!(
                            target: "4da::db",
                            "Created llm_judgments table (Tier 2 intelligence)"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 65: structured dismiss feedback for compound intelligence
            if current_version < 65 {
                Self::run_versioned_migration(
                    &conn,
                    64,
                    65,
                    "Phase 65: structured dismiss feedback for compound intelligence",
                    |c| {
                        // The `interactions` table lives in the ACE database, not the
                        // main DB. On a fresh install the ACE DB may not have been
                        // initialized yet, so check the table exists before ALTER.
                        let table_exists = c
                            .query_row(
                                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='interactions'",
                                [],
                                |row| row.get::<_, i64>(0),
                            )
                            .unwrap_or(0)
                            > 0;

                        if table_exists {
                            let has_dismiss_reason = c
                                .query_row(
                                    "SELECT COUNT(*) FROM pragma_table_info('interactions') WHERE name = 'dismiss_reason'",
                                    [],
                                    |row| row.get::<_, i64>(0),
                                )
                                .unwrap_or(0)
                                > 0;

                            if !has_dismiss_reason {
                                c.execute_batch(
                                    "ALTER TABLE interactions ADD COLUMN dismiss_reason TEXT;
                                     ALTER TABLE interactions ADD COLUMN dismiss_category TEXT;",
                                )?;
                            }
                            info!(
                                target: "4da::db",
                                "Added dismiss_reason + dismiss_category to interactions (compound intelligence loop)"
                            );
                        } else {
                            info!(
                                target: "4da::db",
                                "Skipped dismiss columns — interactions table not yet created (ACE DB)"
                            );
                        }
                        Ok(())
                    },
                )?;
            }

            // Phase 66: Scoring event log for debugging + recalibration backtesting
            if current_version < 66 {
                Self::run_versioned_migration(
                    &conn,
                    65,
                    66,
                    "Phase 66: scoring event log for audit trail + recalibration",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS scoring_events (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                cycle_ts TEXT NOT NULL DEFAULT (datetime('now')),
                                total_scored INTEGER NOT NULL,
                                total_relevant INTEGER NOT NULL,
                                avg_score REAL NOT NULL,
                                max_score REAL NOT NULL,
                                gate_rejections INTEGER NOT NULL DEFAULT 0,
                                commodity_caps INTEGER NOT NULL DEFAULT 0,
                                enrichment_promotions INTEGER NOT NULL DEFAULT 0,
                                briefing_items INTEGER NOT NULL DEFAULT 0
                            );
                            CREATE INDEX IF NOT EXISTS idx_scoring_events_ts ON scoring_events(cycle_ts);"
                        )?;
                        info!(
                            target: "4da::db",
                            "Created scoring_events table for audit trail"
                        );
                        Ok(())
                    },
                )?;
            }

            if current_version < 67 {
                Self::run_versioned_migration(
                    &conn,
                    66,
                    67,
                    "Phase 67: canonicalize project_dependencies paths (dedup Windows casing)",
                    |c| {
                        // Delete rows that would collide after normalization BEFORE
                        // updating paths — the UNIQUE(project_path, package_name)
                        // constraint rejects the UPDATE if duplicates exist.
                        c.execute_batch(
                            "DELETE FROM project_dependencies
                             WHERE id NOT IN (
                                 SELECT MIN(id)
                                 FROM project_dependencies
                                 GROUP BY LOWER(REPLACE(project_path, '\\', '/')),
                                          package_name
                             );",
                        )?;
                        c.execute_batch(
                            "UPDATE project_dependencies
                             SET project_path = LOWER(REPLACE(project_path, '\\', '/'));",
                        )?;
                        let remaining: i64 =
                            c.query_row("SELECT COUNT(*) FROM project_dependencies", [], |row| {
                                row.get(0)
                            })?;
                        info!(
                            target: "4da::db",
                            "Canonicalized project_dependencies paths, {} rows remaining",
                            remaining
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 68: Add withdrawn_at column to osv_advisories
            if current_version < 68 {
                Self::run_versioned_migration(
                    &conn,
                    67,
                    68,
                    "Phase 68: track withdrawn OSV advisories",
                    |c| {
                        c.execute(
                            "ALTER TABLE osv_advisories ADD COLUMN withdrawn_at TEXT",
                            [],
                        )?;
                        info!(
                            target: "4da::db",
                            "Added withdrawn_at column to osv_advisories (filters withdrawn from active counts)"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 69: source_item_dependencies — durable links between source items and user deps
            if current_version < 69 {
                Self::run_versioned_migration(
                    &conn,
                    68,
                    69,
                    "Phase 69: source_item_dependencies table",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS source_item_dependencies (
                                id INTEGER PRIMARY KEY,
                                source_item_id INTEGER NOT NULL,
                                package_name TEXT NOT NULL,
                                ecosystem TEXT,
                                match_type TEXT NOT NULL DEFAULT 'title_heuristic',
                                confidence REAL NOT NULL DEFAULT 0.5,
                                evidence_text TEXT,
                                source_url TEXT,
                                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                                FOREIGN KEY (source_item_id) REFERENCES source_items(id) ON DELETE CASCADE
                            );
                            CREATE INDEX IF NOT EXISTS idx_sid_pkg ON source_item_dependencies(source_item_id, package_name);
                            CREATE INDEX IF NOT EXISTS idx_pkg_eco ON source_item_dependencies(package_name, ecosystem);
                            CREATE INDEX IF NOT EXISTS idx_match_type ON source_item_dependencies(match_type);
                            CREATE UNIQUE INDEX IF NOT EXISTS idx_sid_pkg_eco ON source_item_dependencies(source_item_id, package_name, COALESCE(ecosystem, ''));",
                        )?;
                        info!(
                            target: "4da::db",
                            "Created source_item_dependencies table with indexes (replaces title LIKE heuristic)"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 70: blind_spot_dismissals — persist user dismissals across restarts
            if current_version < 70 {
                Self::run_versioned_migration(
                    &conn,
                    69,
                    70,
                    "Phase 70: blind_spot_dismissals table",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS blind_spot_dismissals (
                                id INTEGER PRIMARY KEY,
                                item_id TEXT NOT NULL UNIQUE,
                                reason TEXT NOT NULL,
                                dismissed_at DATETIME DEFAULT CURRENT_TIMESTAMP
                            );
                            CREATE INDEX IF NOT EXISTS idx_bsd_item ON blind_spot_dismissals(item_id);",
                        )?;
                        info!(
                            target: "4da::db",
                            "Created blind_spot_dismissals table for persistent dismissals"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 71: dependency_snapshots — point-in-time dep snapshots + current view
            if current_version < 71 {
                Self::run_versioned_migration(
                    &conn,
                    70,
                    71,
                    "Phase 71: dependency_snapshots table + current_dependencies view",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS dependency_snapshots (
                                id INTEGER PRIMARY KEY,
                                project_path TEXT NOT NULL,
                                package_name TEXT NOT NULL,
                                ecosystem TEXT NOT NULL,
                                version TEXT,
                                is_direct INTEGER NOT NULL DEFAULT 1,
                                is_dev INTEGER NOT NULL DEFAULT 0,
                                source TEXT NOT NULL DEFAULT 'manifest',
                                scanned_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                                UNIQUE(project_path, package_name, ecosystem)
                            );
                            CREATE INDEX IF NOT EXISTS idx_ds_project ON dependency_snapshots(project_path);
                            CREATE INDEX IF NOT EXISTS idx_ds_package ON dependency_snapshots(package_name);
                            CREATE INDEX IF NOT EXISTS idx_ds_scanned ON dependency_snapshots(scanned_at);

                            CREATE VIEW IF NOT EXISTS current_dependencies AS
                            SELECT ds.* FROM dependency_snapshots ds
                            INNER JOIN (
                                SELECT project_path, package_name, ecosystem, MAX(scanned_at) as latest
                                FROM dependency_snapshots
                                GROUP BY project_path, package_name, ecosystem
                            ) latest ON ds.project_path = latest.project_path
                                AND ds.package_name = latest.package_name
                                AND ds.ecosystem = latest.ecosystem
                                AND ds.scanned_at = latest.latest;",
                        )?;
                        info!(
                            target: "4da::db",
                            "Created dependency_snapshots table + current_dependencies view"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 72: Feedback Outbox — durable retry queue for trust feedback
            if current_version < 72 {
                Self::run_versioned_migration(
                    &conn,
                    71,
                    72,
                    "Phase 72: feedback_outbox table for durable retry",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS feedback_outbox (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                event_type TEXT NOT NULL,
                                signal_id TEXT,
                                alert_id TEXT,
                                source_type TEXT,
                                topic TEXT,
                                notes TEXT,
                                dismiss_reason TEXT,
                                dismiss_category TEXT,
                                queued_at INTEGER NOT NULL,
                                attempts INTEGER NOT NULL DEFAULT 0,
                                last_attempt_at INTEGER,
                                status TEXT NOT NULL DEFAULT 'pending'
                            );
                            CREATE INDEX IF NOT EXISTS idx_feedback_outbox_status
                                ON feedback_outbox(status, attempts);",
                        )?;
                        info!(
                            target: "4da::db",
                            "Created feedback_outbox table (durable retry queue)"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 73: Dedup index for feedback outbox
            if current_version < 73 {
                Self::run_versioned_migration(
                    &conn,
                    72,
                    73,
                    "Phase 73: feedback_outbox dedup index",
                    |c| {
                        c.execute_batch(
                            "CREATE UNIQUE INDEX IF NOT EXISTS idx_feedback_outbox_dedup
                                ON feedback_outbox(event_type, COALESCE(signal_id,''), COALESCE(alert_id,''), COALESCE(source_type,''), COALESCE(topic,''), status);",
                        )?;
                        info!(
                            target: "4da::db",
                            "Added dedup index to feedback_outbox"
                        );
                        Ok(())
                    },
                )?;
            }

            if current_version < 74 {
                Self::run_versioned_migration(
                    &conn,
                    73,
                    74,
                    "Phase 74: fix source_item_dependencies dedup key (ecosystem-variant duplicates)",
                    |c| {
                        c.execute_batch(
                            "-- Merge ecosystem-variant duplicates: keep the row with highest confidence
                             DELETE FROM source_item_dependencies
                             WHERE id NOT IN (
                                 SELECT id FROM (
                                     SELECT id, ROW_NUMBER() OVER (
                                         PARTITION BY source_item_id, package_name
                                         ORDER BY confidence DESC, id ASC
                                     ) AS rn
                                     FROM source_item_dependencies
                                 ) WHERE rn = 1
                             );
                             -- Drop the old unique index that included ecosystem
                             DROP INDEX IF EXISTS idx_sid_pkg_eco;
                             -- Create new unique index on (source_item_id, package_name) only
                             CREATE UNIQUE INDEX idx_sid_pkg_eco ON source_item_dependencies(source_item_id, package_name);",
                        )?;
                        info!(
                            target: "4da::db",
                            "Fixed source_item_dependencies dedup key — removed ecosystem from unique constraint"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 75: Stability Detector — learned facet lifecycle with evidence tracking
            if current_version < 75 {
                Self::run_versioned_migration(
                    &conn,
                    74,
                    75,
                    "Phase 75: stability detector (learned facets + evidence)",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS learned_facets (
                                facet_id TEXT PRIMARY KEY,
                                class TEXT NOT NULL,
                                key TEXT NOT NULL,
                                value TEXT NOT NULL,
                                stability REAL NOT NULL DEFAULT 0.0,
                                state TEXT NOT NULL DEFAULT 'candidate',
                                user_state TEXT NOT NULL DEFAULT 'auto',
                                evidence_count INTEGER NOT NULL DEFAULT 0,
                                first_seen_at INTEGER NOT NULL,
                                last_seen_at INTEGER NOT NULL,
                                UNIQUE(class, key)
                            );
                            CREATE INDEX IF NOT EXISTS idx_facets_class_state ON learned_facets(class, state);
                            CREATE INDEX IF NOT EXISTS idx_facets_stability ON learned_facets(stability DESC);

                            CREATE TABLE IF NOT EXISTS facet_evidence (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                facet_id TEXT NOT NULL REFERENCES learned_facets(facet_id) ON DELETE CASCADE,
                                cue_family TEXT NOT NULL,
                                evidence_type TEXT NOT NULL,
                                confidence REAL NOT NULL,
                                observed_at INTEGER NOT NULL
                            );
                            CREATE INDEX IF NOT EXISTS idx_evidence_facet ON facet_evidence(facet_id);
                            CREATE INDEX IF NOT EXISTS idx_evidence_observed ON facet_evidence(observed_at DESC);",
                        )?;
                        info!(target: "4da::db", "Created learned_facets + facet_evidence tables for stability detector");
                        Ok(())
                    },
                )?;
            }

            // Phase 76: Topic Hotness — cross-source signal consolidation
            if current_version < 76 {
                Self::run_versioned_migration(
                    &conn,
                    75,
                    76,
                    "Phase 76: topic hotness (cross-source consolidation)",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS topic_hotness (
                                topic_key TEXT PRIMARY KEY,
                                mention_count INTEGER NOT NULL DEFAULT 0,
                                distinct_sources INTEGER NOT NULL DEFAULT 0,
                                last_seen_at INTEGER NOT NULL,
                                query_hits INTEGER NOT NULL DEFAULT 0,
                                hotness_score REAL NOT NULL DEFAULT 0.0,
                                materialized INTEGER NOT NULL DEFAULT 0,
                                first_seen_at INTEGER NOT NULL
                            );
                            CREATE INDEX IF NOT EXISTS idx_hotness_score ON topic_hotness(hotness_score DESC);
                            CREATE INDEX IF NOT EXISTS idx_hotness_materialized ON topic_hotness(materialized, hotness_score DESC);

                            CREATE TABLE IF NOT EXISTS topic_hotness_sources (
                                day_source_key TEXT PRIMARY KEY,
                                topic_key TEXT NOT NULL,
                                source_type TEXT NOT NULL,
                                seen_at INTEGER NOT NULL
                            );
                            CREATE INDEX IF NOT EXISTS idx_hotness_sources_topic ON topic_hotness_sources(topic_key);",
                        )?;
                        info!(target: "4da::db", "Created topic_hotness + topic_hotness_sources tables");
                        Ok(())
                    },
                )?;
            }

            // Phase 77: Briefing Seals — compound temporal memory
            if current_version < 77 {
                Self::run_versioned_migration(
                    &conn,
                    76,
                    77,
                    "Phase 77: briefing seals (compound temporal memory)",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS briefing_seals (
                                seal_id TEXT PRIMARY KEY,
                                seal_date TEXT NOT NULL,
                                seal_level INTEGER NOT NULL DEFAULT 0,
                                parent_seal_id TEXT,
                                summary_text TEXT NOT NULL,
                                item_count INTEGER NOT NULL,
                                top_topics TEXT NOT NULL DEFAULT '[]',
                                token_count INTEGER NOT NULL DEFAULT 0,
                                created_at INTEGER NOT NULL
                            );
                            CREATE INDEX IF NOT EXISTS idx_seals_level_date ON briefing_seals(seal_level, seal_date DESC);
                            CREATE INDEX IF NOT EXISTS idx_seals_parent ON briefing_seals(parent_seal_id);",
                        )?;
                        info!(target: "4da::db", "Created briefing_seals table for compound temporal memory");
                        Ok(())
                    },
                )?;
            }

            if current_version < 78 {
                Self::run_versioned_migration(
                    &conn,
                    77,
                    78,
                    "Phase 78: FTS5 index for hybrid search (BM25 + vector + RRF)",
                    |c| {
                        c.execute_batch(
                            "CREATE VIRTUAL TABLE IF NOT EXISTS source_items_fts USING fts5(
                                title,
                                content,
                                content='source_items',
                                content_rowid='id',
                                tokenize='porter unicode61'
                            );

                            -- Populate FTS5 from existing data
                            INSERT OR IGNORE INTO source_items_fts(rowid, title, content)
                                SELECT id, COALESCE(title, ''), COALESCE(content, '')
                                FROM source_items;",
                        )?;
                        info!(target: "4da::db", "Created FTS5 index for hybrid search");
                        Ok(())
                    },
                )?;
            }

            if current_version < 79 {
                Self::run_versioned_migration(
                    &conn,
                    78,
                    79,
                    &format!(
                        "Phase 79: embedding dimension upgrade to {}d (vec table recreation)",
                        crate::EMBEDDING_DIMS
                    ),
                    |c| {
                        let dim = crate::EMBEDDING_DIMS;
                        c.execute_batch(&format!(
                            "DROP TABLE IF EXISTS context_vec;
                             DROP TABLE IF EXISTS source_vec;
                             CREATE VIRTUAL TABLE context_vec USING vec0(
                                 embedding float[{dim}]
                             );
                             CREATE VIRTUAL TABLE source_vec USING vec0(
                                 embedding float[{dim}]
                             );"
                        ))?;
                        c.execute_batch(
                            "UPDATE source_items SET embedding = zeroblob(0), embedding_status = 'pending'
                               WHERE length(embedding) > 0;
                             UPDATE context_chunks SET embedding = zeroblob(0)
                               WHERE length(embedding) > 0;",
                        )?;
                        info!(
                            target: "4da::db",
                            dim,
                            "Recreated vec0 tables at {dim}d, marked all embeddings for regeneration"
                        );
                        Ok(())
                    },
                )?;
            }

            if current_version < 80 {
                Self::run_versioned_migration(
                    &conn,
                    79,
                    80,
                    "Phase 80: pipeline version tracking for score staleness prevention",
                    |c| {
                        c.execute_batch(
                            "ALTER TABLE source_items ADD COLUMN scored_pipeline_version INTEGER NOT NULL DEFAULT 0;"
                        )?;
                        Ok(())
                    },
                )?;
            }

            if current_version < 81 {
                Self::run_versioned_migration(
                    &conn,
                    80,
                    81,
                    "Phase 81: error telemetry table",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS error_telemetry (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                category TEXT NOT NULL,
                                message TEXT NOT NULL,
                                context TEXT,
                                count INTEGER NOT NULL DEFAULT 1,
                                first_seen TEXT NOT NULL DEFAULT (datetime('now')),
                                last_seen TEXT NOT NULL DEFAULT (datetime('now')),
                                UNIQUE(category, message)
                            );
                            CREATE INDEX IF NOT EXISTS idx_error_telemetry_category ON error_telemetry(category);
                            CREATE INDEX IF NOT EXISTS idx_error_telemetry_last_seen ON error_telemetry(last_seen);",
                        )?;
                        Ok(())
                    },
                )?;
            }

            // Phase 82: signal_type + signal_priority persistence on source_items
            // for MCP server to read pipeline-computed signals instead of
            // re-classifying with keywords.
            if current_version < 82 {
                Self::run_versioned_migration(
                    &conn,
                    81,
                    82,
                    "Phase 82: signal_type + signal_priority columns on source_items",
                    |c| {
                        c.execute_batch(
                            "ALTER TABLE source_items ADD COLUMN signal_type TEXT;
                             ALTER TABLE source_items ADD COLUMN signal_priority TEXT;",
                        )?;
                        Ok(())
                    },
                )?;
            }

            // Phase 83: backfill -1.0 precision sentinels to NULL. Earlier weekly
            // precision computation stored -1.0 as an "insufficient data" sentinel;
            // an impossible numeric in a REAL column corrupts every reader. Undefined
            // precision is NULL (no-vanity-metric doctrine); the compute path now
            // writes None, this cleans the historical rows.
            if current_version < 83 {
                Self::run_versioned_migration(
                    &conn,
                    82,
                    83,
                    "Phase 83: backfill precision_stats -1.0 sentinels to NULL",
                    |c| {
                        c.execute_batch(
                            "UPDATE precision_stats SET precision = NULL WHERE precision < 0;",
                        )?;
                        Ok(())
                    },
                )?;
            }

            // Phase 84: dependency_edges — capture the parent->child dependency
            // graph from lockfiles so transitive-vulnerability reachability can be
            // computed. Today's parsers flatten lockfiles to (name, version) and
            // discard edges; without edges reachability is unknowable. Ships silent
            // (internal computation only; no surfaced output yet).
            if current_version < 84 {
                Self::run_versioned_migration(
                    &conn,
                    83,
                    84,
                    "Phase 84: dependency_edges table for reachability",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS dependency_edges (
                                 id INTEGER PRIMARY KEY,
                                 project_path TEXT NOT NULL,
                                 ecosystem TEXT NOT NULL,
                                 parent_package TEXT NOT NULL,
                                 parent_version TEXT,
                                 child_package TEXT NOT NULL,
                                 child_version TEXT,
                                 scope TEXT NOT NULL DEFAULT 'unknown',
                                 detected_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );
                             CREATE INDEX IF NOT EXISTS idx_dep_edges_parent
                                 ON dependency_edges (project_path, parent_package);
                             CREATE INDEX IF NOT EXISTS idx_dep_edges_child
                                 ON dependency_edges (project_path, child_package);",
                        )?;
                        Ok(())
                    },
                )?;
            }

            if current_version < 85 {
                Self::run_versioned_migration(
                    &conn,
                    84,
                    85,
                    "Phase 85: platform-aware dependency relevance (target_cfg, platform_active)",
                    |c| {
                        // Platform relevance for dependency advisories. `target_cfg`
                        // is the gating spec (e.g. cfg(windows)) or NULL for
                        // unconditional deps; `platform_active` is 0 when the dep is
                        // not built on the host. Default 1 keeps every existing row
                        // visible until the scanner populates these (ships silent).
                        c.execute_batch(
                            "ALTER TABLE project_dependencies ADD COLUMN target_cfg TEXT;
                             ALTER TABLE project_dependencies ADD COLUMN platform_active INTEGER DEFAULT 1;",
                        )?;
                        Ok(())
                    },
                )?;
            }

            // Phase 86: brief_rejections — structured "Filtered Out" verdicts
            // parsed from the narrated Brief's machine trailer. The scoring
            // analyzer reads the last 7 days of these to demote feed items the
            // Brief already rejected ("yesterday's noise becomes tomorrow's
            // signal"). Internal plumbing, not a UI intelligence type.
            if current_version < 86 {
                Self::run_versioned_migration(
                    &conn,
                    85,
                    86,
                    "Phase 86: brief_rejections table (Brief verdicts feed demotion)",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS brief_rejections (
                                 id INTEGER PRIMARY KEY,
                                 briefing_id INTEGER,
                                 source_item_id INTEGER NOT NULL,
                                 reason TEXT NOT NULL,
                                 created_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );
                             CREATE INDEX IF NOT EXISTS idx_brief_rejections_item
                                 ON brief_rejections (source_item_id);",
                        )?;
                        Ok(())
                    },
                )?;
            }

            // Phase 87: dependency provenance. `detected_from` records HOW a
            // dependency row was discovered — 'manifest' (declared in a
            // manifest file), 'lockfile' (resolved from a lockfile walk), or
            // 'import_scrape' (inferred from source-file import lines).
            // Existing rows default to 'unknown'. The builtin-module self-heal
            // purge keys on this: provenance='import_scrape' rows with builtin
            // names are pollution by definition; 'manifest' rows are immune
            // (a user CAN declare the npm 'buffer' polyfill); 'unknown' legacy
            // rows fall back to the one-shot heuristic (builtin name +
            // version IS NULL + is_direct=1).
            if current_version < 87 {
                Self::run_versioned_migration(
                    &conn,
                    86,
                    87,
                    "Phase 87: detected_from provenance on dependency tables",
                    |c| {
                        for table in ["user_dependencies", "project_dependencies"] {
                            let has_column: bool = c
                                .query_row(
                                    &format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name='detected_from'"),
                                    [],
                                    |row| row.get::<_, i64>(0).map(|count| count > 0),
                                )
                                .unwrap_or(false);
                            if !has_column {
                                c.execute_batch(&format!(
                                    "ALTER TABLE {table} ADD COLUMN detected_from TEXT NOT NULL DEFAULT 'unknown';",
                                ))?;
                                info!(target: "4da::db", "Added detected_from column to {table}");
                            }
                        }
                        Ok(())
                    },
                )?;
            }

            // Phase 88: state_signature on briefing_item_history. Lets the brief
            // detect whether a persistent Critical/High advisory is UNCHANGED
            // (same package/versions/advisories/projects/severity) across days, so
            // it collapses to a compact "still open" line instead of re-screaming
            // the same full card every morning. Nullable — legacy rows keep NULL
            // and simply don't match a signature, which is the desired behavior
            // (a clean baseline on first run after the upgrade).
            if current_version < 88 {
                Self::run_versioned_migration(
                    &conn,
                    87,
                    88,
                    "Phase 88: state_signature on briefing_item_history",
                    |c| {
                        let has_column: bool = c
                            .query_row(
                                "SELECT COUNT(*) FROM pragma_table_info('briefing_item_history') WHERE name='state_signature'",
                                [],
                                |row| row.get::<_, i64>(0).map(|count| count > 0),
                            )
                            .unwrap_or(false);
                        if !has_column {
                            c.execute_batch(
                                "ALTER TABLE briefing_item_history ADD COLUMN state_signature TEXT;
                                 CREATE INDEX IF NOT EXISTS idx_briefing_history_signature
                                     ON briefing_item_history(source_type, state_signature, briefing_date);",
                            )?;
                            info!(target: "4da::db", "Added state_signature column to briefing_item_history");
                        }
                        Ok(())
                    },
                )?;
            }

            if current_version < 89 {
                Self::run_versioned_migration(
                    &conn,
                    88,
                    89,
                    "Phase 89: strength-weighted topic affinities + poisoned-profile recompute",
                    |c| {
                        // topic_affinities/interactions are created by the ACE
                        // bootstrap (ace/db.rs), which shares the production DB
                        // file but is absent on a fresh main-DB (in-memory
                        // tests, first run before ACE init). Fresh databases
                        // get the full column set from the ACE CREATE; this
                        // phase only repairs EXISTING profiles.
                        let has_table = |name: &str| -> bool {
                            c.query_row(
                                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                                [name],
                                |row| row.get::<_, i64>(0).map(|n| n > 0),
                            )
                            .unwrap_or(false)
                        };
                        if !has_table("topic_affinities") || !has_table("interactions") {
                            info!(
                                target: "4da::db",
                                "Phase 89: ACE tables not present (fresh DB) — nothing to repair"
                            );
                            return Ok(());
                        }

                        // Columns for strength-weighted affinity evidence. The
                        // old formula compared bare COUNTS, and its instant
                        // negative arm fired for ANY negatives-only topic — so
                        // 40 passive ignores (−0.1) poisoned a topic exactly
                        // like 40 explicit rejections (−1.0). The 2026-07-13
                        // live profile had the user's own stack (typescript,
                        // tauri, rust, sqlite, tokio…) at hard-negative
                        // affinity with ZERO positive signals.
                        let has_column: bool = c
                            .query_row(
                                "SELECT COUNT(*) FROM pragma_table_info('topic_affinities') WHERE name='weighted_positive'",
                                [],
                                |row| row.get::<_, i64>(0).map(|count| count > 0),
                            )
                            .unwrap_or(false);
                        if !has_column {
                            c.execute_batch(
                                "ALTER TABLE topic_affinities ADD COLUMN weighted_positive REAL NOT NULL DEFAULT 0;
                                 ALTER TABLE topic_affinities ADD COLUMN weighted_negative REAL NOT NULL DEFAULT 0;
                                 ALTER TABLE topic_affinities ADD COLUMN explicit_negative_signals INTEGER NOT NULL DEFAULT 0;",
                            )?;
                        }

                        // One-time deterministic profile repair: rebuild the
                        // weighted evidence from the intact interaction log
                        // (interactions.item_topics JSON + signal_strength),
                        // then recompute every affinity under the corrected
                        // formula. Topics with no logged interactions reset to
                        // neutral evidence — stale poison does not survive.
                        c.execute_batch(
                            "WITH per_topic AS (
                                 SELECT je.value AS topic,
                                        SUM(CASE WHEN i.signal_strength > 0 THEN MIN(i.signal_strength, 1.5) ELSE 0 END) AS wpos,
                                        SUM(CASE WHEN i.signal_strength < 0 THEN MIN(-i.signal_strength, 1.5) ELSE 0 END) AS wneg,
                                        SUM(CASE WHEN i.signal_strength <= -0.8 THEN 1 ELSE 0 END) AS expneg
                                 FROM interactions i, json_each(i.item_topics) je
                                 WHERE i.item_topics IS NOT NULL AND json_valid(i.item_topics)
                                 GROUP BY je.value
                             )
                             UPDATE topic_affinities SET
                                 weighted_positive = COALESCE((SELECT wpos FROM per_topic WHERE per_topic.topic = topic_affinities.topic), 0),
                                 weighted_negative = COALESCE((SELECT wneg FROM per_topic WHERE per_topic.topic = topic_affinities.topic), 0),
                                 explicit_negative_signals = COALESCE((SELECT expneg FROM per_topic WHERE per_topic.topic = topic_affinities.topic), 0);",
                        )?;
                        // Recompute all rows under the corrected formula (the
                        // exact SQL the live per-interaction path uses, minus
                        // the per-topic WHERE).
                        let recompute_all = crate::ace::behavior::tracking::RECOMPUTE_AFFINITY_SQL
                            .replace(" WHERE topic = ?1", "");
                        c.execute_batch(&recompute_all)?;
                        info!(
                            target: "4da::db",
                            "Recomputed topic affinities under strength-weighted formula"
                        );
                        Ok(())
                    },
                )?;
            }

            if current_version < 90 {
                Self::run_versioned_migration(
                    &conn,
                    89,
                    90,
                    "Phase 90: affinity instant-arm keyed on weighted evidence (re-recompute)",
                    |c| {
                        // Phase 89's instant negative arm checked the HISTORICAL
                        // positive_signals count, which pre-dates weighting and
                        // reads 0 for pre-2026-07-13 evidence — so one explicit
                        // dismissal of a junk item left `rust` at -1.0 despite
                        // backfilled positive weighted evidence. The arm (in
                        // RECOMPUTE_AFFINITY_SQL) now keys on weighted_positive;
                        // re-run the recompute so dormant rows heal too.
                        let has_table: bool = c
                            .query_row(
                                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='topic_affinities'",
                                [],
                                |row| row.get::<_, i64>(0).map(|n| n > 0),
                            )
                            .unwrap_or(false);
                        if !has_table {
                            return Ok(());
                        }
                        let recompute_all = crate::ace::behavior::tracking::RECOMPUTE_AFFINITY_SQL
                            .replace(" WHERE topic = ?1", "");
                        c.execute_batch(&recompute_all)?;
                        info!(
                            target: "4da::db",
                            "Re-ran affinity recompute with weighted-evidence instant arm"
                        );
                        Ok(())
                    },
                )?;
            }

            if current_version < 91 {
                Self::run_versioned_migration(
                    &conn,
                    90,
                    91,
                    "Phase 91: published_at on source_items (freshness truth)",
                    |c| {
                        // Publication date from source adapters (RSS pubDate,
                        // npm time, OSV published, ...). Adapters always parsed
                        // these but they were dropped at the DB boundary, so a
                        // 2023 article a feed keeps in its XML re-entered the
                        // analysis window forever via last_seen refreshes.
                        // NULL = unknown; every reader COALESCEs to created_at
                        // (first-seen), so no backfill is needed or honest.
                        let has_column: bool = c
                            .query_row(
                                "SELECT COUNT(*) FROM pragma_table_info('source_items') WHERE name='published_at'",
                                [],
                                |row| row.get::<_, i64>(0).map(|count| count > 0),
                            )
                            .unwrap_or(false);
                        if !has_column {
                            c.execute_batch(
                                "ALTER TABLE source_items ADD COLUMN published_at TEXT DEFAULT NULL;
                                 CREATE INDEX IF NOT EXISTS idx_source_items_effective_published
                                     ON source_items(COALESCE(published_at, created_at));",
                            )?;
                            info!(target: "4da::db", "Added published_at to source_items");
                        }
                        Ok(())
                    },
                )?;
            }

            // Phase 92: dependency_instances — the multi-version installed
            // inventory. Every prior dep table collapses to one row per
            // (project, package, ecosystem): the UNIQUE(...) DO UPDATE SET
            // version = COALESCE(...) upsert discards every extra installed
            // version (diamond deps, monorepo duplicates, a direct+transitive
            // version skew). A lockfile genuinely resolves a package at
            // multiple versions, and matching an advisory against only the
            // surviving row lets a still-vulnerable duplicate pass as
            // "not affected" — a false negative (accuracy-first: worse than a
            // competitor's noise). This table records ONE ROW PER INSTALLED
            // INSTANCE so negative verdicts (not_affected / safe-to-close /
            // quiet-week) can be proven against EVERY installed version.
            // Ships SILENT: populated by the lockfile processors, not yet read
            // by any surface (the version-confirmed matcher rewires onto it
            // next, behind the founder dogfood gate).
            if current_version < 92 {
                Self::run_versioned_migration(
                    &conn,
                    91,
                    92,
                    "Phase 92: dependency_instances multi-version inventory",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS dependency_instances (
                                 id INTEGER PRIMARY KEY,
                                 project_path TEXT NOT NULL,
                                 ecosystem TEXT NOT NULL,
                                 package_name TEXT NOT NULL,
                                 version TEXT NOT NULL,
                                 is_direct INTEGER NOT NULL DEFAULT 0,
                                 is_dev INTEGER NOT NULL DEFAULT 0,
                                 scope TEXT NOT NULL DEFAULT 'unknown',
                                 detected_at TEXT NOT NULL DEFAULT (datetime('now')),
                                 UNIQUE(project_path, ecosystem, package_name, version)
                             );
                             CREATE INDEX IF NOT EXISTS idx_dep_instances_project
                                 ON dependency_instances (project_path, ecosystem);
                             CREATE INDEX IF NOT EXISTS idx_dep_instances_pkg
                                 ON dependency_instances (ecosystem, package_name);",
                        )?;
                        info!(target: "4da::db", "Created dependency_instances table (Phase 92)");
                        Ok(())
                    },
                )?;
            }

            // Phase 93: heal Mastodon titles. derive_title() used to weld text
            // runs at stripped-tag boundaries ("editionshttps://…"), keep bare
            // URL anchor text, and keep hashtag runs — 3,208/20,938 stored
            // mastodon titles carried a URL, 7,818 carried hashtags (live
            // corpus 2026-07-17). Titles are display + classifier input on
            // every surface, so re-derive EVERY stored mastodon title from the
            // raw HTML body with the FIXED function — historical rows heal
            // identically to fresh ingest (welds like "debutASRock" carry no
            // '#' or '://', so a targeted filter would miss them; the dry-run
            // on a corpus copy showed re-derivation is safe across all rows).
            // Rows whose body re-derives to empty (URL-only toots) keep their
            // old title rather than going blank.
            if current_version < 93 {
                Self::run_versioned_migration(
                    &conn,
                    92,
                    93,
                    "Phase 93: re-derive polluted mastodon titles from content",
                    |c| {
                        let mut stmt = c.prepare(
                            "SELECT id, content FROM source_items
                             WHERE source_type = 'mastodon'
                               AND content IS NOT NULL AND content != ''",
                        )?;
                        let rows: Vec<(i64, String)> = stmt
                            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                            .collect::<std::result::Result<_, _>>()?;
                        drop(stmt);

                        let mut healed = 0usize;
                        for (id, content) in rows {
                            let title = crate::sources::mastodon::derive_title(&content);
                            if !title.is_empty() {
                                c.execute(
                                    "UPDATE source_items SET title = ?1 WHERE id = ?2",
                                    rusqlite::params![title, id],
                                )?;
                                healed += 1;
                            }
                        }
                        info!(
                            target: "4da::db",
                            healed,
                            "Phase 93: re-derived mastodon titles"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 94: second mastodon-title heal with derive_title v3. The
            // Phase 93 rules left an ongoing residue class: Mastodon splits a
            // long displayed URL across invisible/ellipsis spans, so the
            // scheme-less visible part ("news.ycombinator.com/item?id=4") and
            // the severed tail ("8895199") survived as title words, plus the
            // now-orphaned label they hung off ("Comments:"). v3 drops all
            // three (dev vocabulary like node.js / TCP/IP survives — see the
            // is_bare_url/is_url_tail unit tests). Same re-derive-all shape
            // as Phase 93; rows whose body re-derives to empty keep their old
            // title.
            if current_version < 94 {
                Self::run_versioned_migration(
                    &conn,
                    93,
                    94,
                    "Phase 94: re-derive mastodon titles (scheme-less URLs + severed tails)",
                    |c| {
                        let mut stmt = c.prepare(
                            "SELECT id, content FROM source_items
                             WHERE source_type = 'mastodon'
                               AND content IS NOT NULL AND content != ''",
                        )?;
                        let rows: Vec<(i64, String)> = stmt
                            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                            .collect::<std::result::Result<_, _>>()?;
                        drop(stmt);

                        let mut healed = 0usize;
                        for (id, content) in rows {
                            let title = crate::sources::mastodon::derive_title(&content);
                            if !title.is_empty() {
                                c.execute(
                                    "UPDATE source_items SET title = ?1 WHERE id = ?2",
                                    rusqlite::params![title, id],
                                )?;
                                healed += 1;
                            }
                        }
                        info!(
                            target: "4da::db",
                            healed,
                            "Phase 94: re-derived mastodon titles (v3 rules)"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 95: persist the per-run feed curation verdict. The
            // analysis run computes a curated corpus every time (dedup,
            // diversity, rerank, brief-rejection demotions, threshold) and
            // then throws the verdict away — only raw scores persist. Any
            // surface re-deriving "the corpus" from raw scores (the content
            // graph did) resurrects items today's brain rejects, because old
            // items are never re-scored and scoring is non-stationary even
            // within one pipeline version (live 2026-07-19: war-news items
            // held stale 0.94 scores under v16 while the current run scored
            // fresh war news 0.07-0.40, relevant=false). NULL = never judged,
            // 1 = in the curated corpus, 0 = judged and rejected.
            if current_version < 95 {
                Self::run_versioned_migration(
                    &conn,
                    94,
                    95,
                    "Phase 95: feed_relevant curation verdict on source_items",
                    |c| {
                        // Idempotent under version-rewind (the migration test
                        // harness re-runs phases): ALTER only when the column
                        // is genuinely absent.
                        let has_column: bool = c
                            .prepare("SELECT COUNT(*) FROM pragma_table_info('source_items') WHERE name = 'feed_relevant'")?
                            .query_row([], |r| r.get::<_, i64>(0))
                            .map(|n| n > 0)?;
                        if !has_column {
                            c.execute_batch(
                                "ALTER TABLE source_items ADD COLUMN feed_relevant INTEGER;
                                 ALTER TABLE source_items ADD COLUMN feed_verdict_at TEXT;",
                            )?;
                        }
                        c.execute_batch(
                            "CREATE INDEX IF NOT EXISTS idx_si_feed_relevant
                                 ON source_items(feed_relevant, created_at)
                                 WHERE feed_relevant IS NOT NULL;",
                        )?;
                        info!(
                            target: "4da::db",
                            "Phase 95: feed_relevant verdict columns added"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 96: snoozed_items becomes schema-owned. The snooze_item
            // command previously created this table lazily on first use, so
            // every read path had to tolerate its absence — and none did: the
            // table was write-only (live audit 2026-07-19: `snooze_until`
            // appeared in exactly two places, the CREATE and the INSERT).
            // Owning it in migrations lets the graph and feed filter on it
            // unconditionally, which is what makes Snooze real.
            if current_version < 96 {
                Self::run_versioned_migration(
                    &conn,
                    95,
                    96,
                    "Phase 96: snoozed_items owned by schema (snooze becomes filterable)",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS snoozed_items (
                                 source_item_id INTEGER PRIMARY KEY,
                                 snooze_until TEXT NOT NULL,
                                 created_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );
                             CREATE INDEX IF NOT EXISTS idx_snoozed_until
                                 ON snoozed_items(snooze_until);",
                        )?;
                        info!(
                            target: "4da::db",
                            "Phase 96: snoozed_items table owned by schema"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 97: content-graph layout anchors — persisted cluster
            // positions so the map stays spatially recognizable day-over-day.
            // Deterministic-per-build layouts still recomputed globally on
            // every corpus change, so one new item could rearrange the whole
            // map (audit P2.11). Clusters that overlap a stored anchor's
            // member set seed at the anchor position instead of a spiral slot.
            if current_version < 97 {
                Self::run_versioned_migration(
                    &conn,
                    96,
                    97,
                    "Phase 97: graph_layout_anchors (temporal layout stability)",
                    |c| {
                        c.execute_batch(
                            "CREATE TABLE IF NOT EXISTS graph_layout_anchors (
                                 window_days INTEGER NOT NULL,
                                 cluster_key TEXT NOT NULL,
                                 x REAL NOT NULL,
                                 y REAL NOT NULL,
                                 member_ids TEXT NOT NULL,
                                 updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                                 PRIMARY KEY (window_days, cluster_key)
                             );",
                        )?;
                        info!(
                            target: "4da::db",
                            "Phase 97: graph_layout_anchors table created"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 98 is a data heal for two live pollution classes
            // (2026-07-21 signal-quality audit):
            //   1. Test-code grounding: ~25% of the grounding corpus was test
            //      fixtures (adversarial strings BUILT to name deps) surfacing
            //      as "Similar to your code" evidence. Clearing the reconcile
            //      flag re-runs the (idempotent, no re-embed) startup
            //      provenance reconcile, which now demotes test chunks to the
            //      non-grounding `test_code` class.
            //   2. Junk decision windows: dep-less adoption/migration windows
            //      minted from bare keyword hits ("released", "better than")
            //      — 85 open "Adoption:" rows incl. an Apple TV trailer — fed
            //      "Relevant to open decision" evidence. The generator no
            //      longer creates dep-less windows; this expires the backlog.
            if current_version < 98 {
                Self::run_versioned_migration(
                    &conn,
                    97,
                    98,
                    "Phase 98: re-arm provenance reconcile (test-code demotion) + expire dep-less decision windows",
                    |c| {
                        // Fresh databases may not have these tables yet (both
                        // are created outside this migration chain) — a fresh
                        // install has nothing to heal, so guard on existence.
                        let table_exists = |c: &Connection, name: &str| -> SqliteResult<bool> {
                            c.prepare(
                                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                            )?
                            .query_row([name], |r| r.get::<_, i64>(0))
                            .map(|n| n > 0)
                        };
                        if table_exists(c, "kv_store")? {
                            c.execute(
                                "DELETE FROM kv_store WHERE key = 'context_provenance_reconcile_version'",
                                [],
                            )?;
                        }
                        let expired = if table_exists(c, "decision_windows")? {
                            c.execute(
                                "UPDATE decision_windows
                                 SET status = 'expired', closed_at = datetime('now')
                                 WHERE status = 'open'
                                   AND dependency IS NULL
                                   AND window_type IN ('adoption', 'migration')",
                                [],
                            )?
                        } else {
                            0
                        };
                        info!(
                            target: "4da::db",
                            expired_windows = expired,
                            "Phase 98: reconcile re-armed, dep-less decision windows expired"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 99: expire the OPEN auto-minted decision-window backlog so
            // it re-mints under ecosystem-aware dep matching. Live 2026-07-21:
            // the user's CARGO crate `tracing` held 7 open "Security: tracing"
            // windows minted from dd-trace-{java,py,go,js,rb,dotnet} advisories
            // — ambiguous names passed on generic "library"/"package"
            // vocabulary regardless of ecosystem. Windows are cheap DERIVED
            // state re-detected every monitoring cycle: expiring the open set
            // is a full heal (genuinely valid windows return within one cycle,
            // wrong-ecosystem ones cannot), while 'acted' rows — the user's
            // own decisions — are untouched.
            if current_version < 99 {
                Self::run_versioned_migration(
                    &conn,
                    98,
                    99,
                    "Phase 99: expire open decision windows for ecosystem-aware re-mint",
                    |c| {
                        let table_exists: bool = c
                            .prepare(
                                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='decision_windows'",
                            )?
                            .query_row([], |r| r.get::<_, i64>(0))
                            .map(|n| n > 0)?;
                        let expired = if table_exists {
                            c.execute(
                                "UPDATE decision_windows
                                 SET status = 'expired', closed_at = datetime('now')
                                 WHERE status = 'open'",
                                [],
                            )?
                        } else {
                            0
                        };
                        info!(
                            target: "4da::db",
                            expired_windows = expired,
                            "Phase 99: open decision windows expired for ecosystem-aware re-mint"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 100: index scored_pipeline_version. Every scheduled analysis
            // probes the stale-version backlog (`merge_stale_drain_batch`) and the
            // never-scored backlog (`scored_pipeline_version = 0`); without an
            // index both walk the relevance index checking the version row-by-row
            // — measured ~700-800ms per probe on a 192k corpus EVEN WHEN ZERO
            // items are stale, paid every 30-min cycle forever. With the index
            // the empty/near-drained probe is ~0ms and shrinks with the backlog,
            // while the full-stale bump-time chunk query is unchanged (planner
            // correctly keeps the relevance index when the version range matches
            // everything). Measured on a live-DB copy 2026-07-25; build ~700ms.
            if current_version < 100 {
                Self::run_versioned_migration(
                    &conn,
                    99,
                    100,
                    "Phase 100: scored_pipeline_version index for drain/backfill probes",
                    |c| {
                        c.execute_batch(
                            "CREATE INDEX IF NOT EXISTS idx_source_items_scored_version
                                 ON source_items(scored_pipeline_version);",
                        )?;
                        info!(target: "4da::db", "Phase 100: scored_pipeline_version index created");
                        Ok(())
                    },
                )?;
            }

            // Phase 101: verdict epochs. `feed_relevant` (Phase 95) is the
            // value that decides what the user SEES, and it shipped with a
            // timestamp but no pipeline version — so every PIPELINE_VERSION
            // bump silently invalidated the whole curated corpus and nothing
            // converged it back. Measured live 2026-07-26, AFTER the v18 drain
            // had brought scores to 100% current: 399 of 426 curated items
            // still held a pre-v18 verdict and 181 of those now score below
            // the relevance threshold (156 of them the exact `crates_io`
            // look-alike class v18 declared categorically never feed-relevant).
            //
            // Both columns are nullable with NO backfill: NULL means "written
            // before provenance was recorded", which is the truth. The partial
            // index covers only the curated set (hundreds of rows), so the
            // per-cycle staleness probe is ~0ms — the Phase-100 lesson, that a
            // probe paid every cycle forever must be indexed, applied up front.
            if current_version < 101 {
                Self::run_versioned_migration(
                    &conn,
                    100,
                    101,
                    "Phase 101: feed verdict epoch + provenance columns",
                    |c| {
                        // Idempotent under version-rewind (the migration test
                        // harness re-runs phases): ALTER only when absent.
                        let has_column: bool = c
                            .prepare("SELECT COUNT(*) FROM pragma_table_info('source_items') WHERE name = 'feed_verdict_version'")?
                            .query_row([], |r| r.get::<_, i64>(0))
                            .map(|n| n > 0)?;
                        if !has_column {
                            c.execute_batch(
                                "ALTER TABLE source_items ADD COLUMN feed_verdict_version INTEGER;
                                 ALTER TABLE source_items ADD COLUMN feed_verdict_source TEXT;",
                            )?;
                        }
                        // COVERING deliberately: both stamp columns are in the
                        // index, so the per-cycle staleness probe is answered
                        // without touching a table row. Measured on a copy of
                        // the live 2.4 GB corpus (426 curated rows): indexing
                        // `feed_verdict_version` ALONE left the planner on
                        // `idx_si_feed_relevant` and 426 random row lookups —
                        // 902 ms cold, every cycle, forever. Adding
                        // `feed_verdict_source` flips it to
                        // "SCAN … USING COVERING INDEX" at 3.7 ms cold. Same
                        // trap Phase 100 was created to close; the partial
                        // `WHERE feed_relevant = 1` keeps the index to the
                        // curated set rather than the whole corpus.
                        c.execute_batch(
                            "CREATE INDEX IF NOT EXISTS idx_si_feed_verdict_version
                                 ON source_items(feed_verdict_version, feed_verdict_source)
                                 WHERE feed_relevant = 1;",
                        )?;
                        info!(
                            target: "4da::db",
                            "Phase 101: feed verdict epoch columns added"
                        );
                        Ok(())
                    },
                )?;
            }

            // Phase 102: third mastodon-title heal. derive_title now skips
            // `<span class="invisible">` content — Mastodon wraps the
            // non-displayed parts of a long URL (scheme prefix, severed tail)
            // in invisible spans, and a purely-alphabetic tail ("ay", the end
            // of "sketch-a-day") passed every Phase-94 heuristic and landed
            // mid-title ("…tip jar are ay Code for", live corpus 2026-07-27).
            // Same shape as Phases 93/94: re-derive every stored mastodon
            // title from the raw body with the fixed function; rows that
            // re-derive to empty (URL-only toots) keep their old title.
            if current_version < 102 {
                Self::run_versioned_migration(
                    &conn,
                    101,
                    102,
                    "Phase 102: re-derive mastodon titles (invisible-span tails)",
                    |c| {
                        let mut stmt = c.prepare(
                            "SELECT id, content FROM source_items
                             WHERE source_type = 'mastodon'
                               AND content IS NOT NULL AND content != ''",
                        )?;
                        let rows: Vec<(i64, String)> = stmt
                            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                            .collect::<std::result::Result<_, _>>()?;
                        drop(stmt);

                        let mut healed = 0usize;
                        for (id, content) in rows {
                            let title = crate::sources::mastodon::derive_title(&content);
                            if !title.is_empty() {
                                c.execute(
                                    "UPDATE source_items SET title = ?1 WHERE id = ?2",
                                    rusqlite::params![title, id],
                                )?;
                                healed += 1;
                            }
                        }
                        info!(
                            target: "4da::db",
                            healed,
                            "Phase 102: re-derived mastodon titles (invisible-span tails)"
                        );
                        Ok(())
                    },
                )?;
            }

            if current_version < 103 {
                Self::run_versioned_migration(
                    &conn,
                    102,
                    103,
                    "Phase 103: purge poisoned calibration samples + orphaned tuner threshold",
                    |c| {
                        // Every pre-v19 calibration sample recorded the
                        // POST-curve confidence under `raw_score`
                        // (analysis_rerank persisted judgment.confidence after
                        // CalibratedCore had already applied the curve). With
                        // the 2026-06-19 degenerate curve live, that meant
                        // 3,028 rows of literal 1.0/1.0 — fitting the next
                        // curve from them would reproduce the poison. v19
                        // persists the true pre-curve raw score; the legacy
                        // rows are unusable for fitting and are removed.
                        let purged = c.execute("DELETE FROM calibration_samples", [])?;
                        // The frozen auto-tuners' persisted threshold could
                        // otherwise linger forever (its reinstall path is
                        // gone, but dead state invites resurrection bugs).
                        let kv = c.execute(
                            "DELETE FROM kv_store WHERE key = 'relevance_threshold'",
                            [],
                        )?;
                        info!(
                            target: "4da::db",
                            purged,
                            kv_removed = kv,
                            "Phase 103: poisoned calibration samples + tuner threshold purged (AD-029)"
                        );
                        Ok(())
                    },
                )?;
            }

            if current_version < 104 {
                Self::run_versioned_migration(
                    &conn,
                    103,
                    104,
                    "Phase 104: FTS5 sync triggers + index rebuild (search index had diverged)",
                    Self::install_fts_sync_triggers_and_rebuild,
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
            // Phase 61: Per-feed health
            "feed_health",
            // Phase 86: Brief rejection verdicts
            "brief_rejections",
            // Phase 96: snooze becomes filterable (schema-owned table)
            "snoozed_items",
            // Phase 97: temporal layout stability for the content graph
            "graph_layout_anchors",
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

    /// Phase 84: dependency_edges table exists with the expected schema and indexes.
    #[test]
    fn test_phase_84_dependency_edges_table_and_indexes() {
        let db = test_db();
        let conn = db.conn.lock();

        // Table exists.
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='dependency_edges'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            table_exists,
            "dependency_edges table should exist after migration"
        );

        // Expected columns present.
        let mut stmt = conn
            .prepare("SELECT name FROM pragma_table_info('dependency_edges')")
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        let expected_cols = [
            "id",
            "project_path",
            "ecosystem",
            "parent_package",
            "parent_version",
            "child_package",
            "child_version",
            "scope",
            "detected_at",
        ];
        for col in expected_cols {
            assert!(
                cols.iter().any(|c| c == col),
                "dependency_edges column '{}' missing; got {:?}",
                col,
                cols
            );
        }

        // Both indexes created.
        let mut idx_stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='dependency_edges'",
            )
            .unwrap();
        let indexes: Vec<String> = idx_stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        for idx in ["idx_dep_edges_parent", "idx_dep_edges_child"] {
            assert!(
                indexes.iter().any(|i| i == idx),
                "dependency_edges index '{}' missing; got {:?}",
                idx,
                indexes
            );
        }
    }

    /// Phase 92: dependency_instances exists with the multi-version UNIQUE key,
    /// expected columns, and indexes; and the key genuinely permits the same
    /// package at two versions in one project (the collapse the table fixes).
    #[test]
    fn test_phase_92_dependency_instances_table_and_multi_version() {
        let db = test_db();
        let conn = db.conn.lock();

        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='dependency_instances'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(table_exists, "dependency_instances table should exist");

        let mut stmt = conn
            .prepare("SELECT name FROM pragma_table_info('dependency_instances')")
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        for col in [
            "id",
            "project_path",
            "ecosystem",
            "package_name",
            "version",
            "is_direct",
            "is_dev",
            "scope",
            "detected_at",
        ] {
            assert!(
                cols.iter().any(|c| c == col),
                "dependency_instances column '{col}' missing; got {cols:?}"
            );
        }

        let mut idx_stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='dependency_instances'")
            .unwrap();
        let indexes: Vec<String> = idx_stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        for idx in ["idx_dep_instances_project", "idx_dep_instances_pkg"] {
            assert!(
                indexes.iter().any(|i| i == idx),
                "dependency_instances index '{idx}' missing; got {indexes:?}"
            );
        }

        // The UNIQUE(project, ecosystem, package, version) key must ADMIT two
        // versions of one package in one project — the entire purpose. A key
        // that collapsed versions would reject the second insert.
        conn.execute_batch(
            "INSERT INTO dependency_instances (project_path, ecosystem, package_name, version)
                 VALUES ('/p', 'npm', 'lodash', '4.17.20');
             INSERT INTO dependency_instances (project_path, ecosystem, package_name, version)
                 VALUES ('/p', 'npm', 'lodash', '4.17.21');",
        )
        .expect("multi-version insert must be permitted by the UNIQUE key");
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM dependency_instances WHERE project_path='/p' AND package_name='lodash'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(n, 2, "both versions retained");
    }

    /// Phase 92 lifts TARGET_VERSION to 92. Verify the test DB reached it.
    #[test]
    fn test_phase_92_schema_version_reached() {
        let db = test_db();
        let conn = db.conn.lock();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert!(
            version >= 92,
            "schema_version should be >= 92 after migration; got {version}"
        );
    }

    /// Phase 93 lifts TARGET_VERSION to 93 and re-derives polluted mastodon
    /// titles (URL-welds, hashtag runs) from stored content with the fixed
    /// derive_title. Verify the version and the healing behavior.
    #[test]
    fn test_phase_93_heals_mastodon_titles() {
        let db = test_db();
        let conn = db.conn.lock();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert!(
            version >= 93,
            "schema_version should be >= 93 after migration; got {version}"
        );

        // Seed a polluted row the way the old derive_title wrote it, wind the
        // version back to 92, and run the (idempotent, versioned) migration
        // pass again so only Phase 93 re-executes.
        conn.execute_batch(
            "INSERT INTO source_items (source_type, source_id, title, content, url, content_hash, embedding)
             VALUES ('mastodon', 'tag:test-93', 'SQLite editionshttps://mort.coffee/x#Tech #Rust',
                     '<p>SQLite editions<br><a href=\"https://mort.coffee/x\">https://mort.coffee/x</a> <a>#Tech</a> <a>#Rust</a></p>',
                     'https://example.com/1', 'hash-test-93', X'00');
             UPDATE schema_version SET version = 92;",
        )
        .unwrap();
        drop(conn);
        db.migrate().expect("re-running migrations from v92");

        let conn = db.conn.lock();
        let healed: String = conn
            .query_row(
                "SELECT title FROM source_items WHERE source_id = 'tag:test-93'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(healed, "SQLite editions");
    }

    /// Phase 102 lifts TARGET_VERSION to 102 and re-heals mastodon titles
    /// with the invisible-span-aware derive_title (a purely-alphabetic
    /// severed URL tail like "ay" passed every earlier heuristic). Wind back
    /// to 101, seed the corrupted row, re-run, assert clean.
    #[test]
    fn test_phase_102_heals_invisible_span_tails() {
        let db = test_db();
        let conn = db.conn.lock();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert!(
            version >= 102,
            "schema_version should be >= 102 after migration; got {version}"
        );

        conn.execute_batch(
            "INSERT INTO source_items (source_type, source_id, title, content, url, content_hash, embedding)
             VALUES ('mastodon', 'tag:test-102', 'The archives and tip jar are ay Code for',
                     '<p>The archives and tip jar are at: <a href=\"https://example.com/sketch-a-day\"><span class=\"invisible\">https://</span><span class=\"ellipsis\">example.com/sketch-a-d</span><span class=\"invisible\">ay</span></a> Code for this</p>',
                     'https://example.com/2', 'hash-test-102', X'00');
             UPDATE schema_version SET version = 101;",
        )
        .unwrap();
        drop(conn);
        db.migrate().expect("re-running migrations from v101");

        let conn = db.conn.lock();
        let healed: String = conn
            .query_row(
                "SELECT title FROM source_items WHERE source_id = 'tag:test-102'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            !healed.contains(" ay ") && !healed.ends_with(" ay"),
            "invisible tail survived the heal: {healed}"
        );
        assert!(
            healed.starts_with("The archives and tip jar"),
            "prose head lost: {healed}"
        );
    }

    /// Phase 94 re-heals with derive_title v3 (scheme-less URLs, severed
    /// tails, orphaned labels). Wind back to 93, seed the residue class
    /// Phase 93 left behind, re-run, assert clean.
    #[test]
    fn test_phase_94_heals_schemeless_url_residue() {
        let db = test_db();
        let conn = db.conn.lock();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert!(
            version >= 94,
            "schema_version should be >= 94 after migration; got {version}"
        );

        conn.execute_batch(
            "INSERT INTO source_items (source_type, source_id, title, content, url, content_hash, embedding)
             VALUES ('mastodon', 'tag:test-94', 'Parsing SGF files for fun video.infosec.exchange/w/8iK3',
                     '<p>Parsing SGF files for fun <a>video.infosec.exchange/w/8iK3NByz1pVVba4kGmycDs</a></p>',
                     'https://example.com/2', 'hash-test-94', X'00');
             UPDATE schema_version SET version = 93;",
        )
        .unwrap();
        drop(conn);
        db.migrate().expect("re-running migrations from v93");

        let conn = db.conn.lock();
        let healed: String = conn
            .query_row(
                "SELECT title FROM source_items WHERE source_id = 'tag:test-94'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(healed, "Parsing SGF files for fun");
    }

    /// Phase 84 lifts TARGET_VERSION to 84. Verify the test DB reached it.
    #[test]
    fn test_phase_84_schema_version_reached() {
        let db = test_db();
        let conn = db.conn.lock();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert!(
            version >= 84,
            "schema_version should be >= 84 after migration; got {}",
            version
        );
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

    /// Phase 95 adds the persisted feed-curation verdict (corpus parity).
    /// Phase 101 lifts TARGET_VERSION to 101 and adds the verdict epoch guard.
    /// Both columns must be nullable with NO backfill — an unstamped verdict is
    /// genuinely of unknown provenance, and inventing a version for it would
    /// make the reconciliation pass skip exactly the rows that need it.
    #[test]
    fn test_phase_101_verdict_epoch_columns() {
        let db = test_db();
        let conn = db.conn.lock();

        let mut stmt = conn
            .prepare("SELECT name, \"notnull\", dflt_value FROM pragma_table_info('source_items')")
            .unwrap();
        let cols: Vec<(String, i64, Option<String>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        for col in ["feed_verdict_version", "feed_verdict_source"] {
            let found = cols.iter().find(|(name, _, _)| name == col);
            let (_, notnull, default) = found
                .unwrap_or_else(|| panic!("source_items column '{col}' missing after Phase 101"));
            assert_eq!(*notnull, 0, "{col} must be nullable (NULL = unstamped)");
            assert!(
                default.is_none(),
                "{col} must have no default — a default would backfill a claim the DB cannot support"
            );
        }

        let idx_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master
                 WHERE type='index' AND name='idx_si_feed_verdict_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            idx_exists,
            "idx_si_feed_verdict_version missing — the staleness probe runs every \
             analysis cycle forever and must not scan"
        );

        // The index must COVER the probe. Measured on the live corpus: with
        // `feed_verdict_source` absent from the index the planner falls back to
        // idx_si_feed_relevant + a row lookup per curated item — 902ms cold per
        // cycle vs 3.7ms covered. Assert on the index columns so a future edit
        // cannot quietly drop the covering property.
        let indexed: Vec<String> = conn
            .prepare("SELECT name FROM pragma_index_info('idx_si_feed_verdict_version')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        for col in ["feed_verdict_version", "feed_verdict_source"] {
            assert!(
                indexed.iter().any(|c| c == col),
                "idx_si_feed_verdict_version must cover '{col}' or the probe stops \
                 being index-only; got {indexed:?}"
            );
        }
    }

    #[test]
    fn test_phase_95_feed_verdict_columns() {
        let db = test_db();
        let conn = db.conn.lock();

        let mut stmt = conn
            .prepare("SELECT name FROM pragma_table_info('source_items')")
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        for col in ["feed_relevant", "feed_verdict_at"] {
            assert!(
                cols.iter().any(|c| c == col),
                "source_items column '{}' missing; got {:?}",
                col,
                cols
            );
        }

        let idx_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master
                 WHERE type='index' AND name='idx_si_feed_relevant'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(idx_exists, "idx_si_feed_relevant missing");

        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert!(
            version >= 95,
            "schema_version should be >= 95; got {version}"
        );
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

    /// Phase 70: blind_spot_dismissals table + index present.
    #[test]
    fn test_phase_70_blind_spot_dismissals() {
        let db = test_db();
        let conn = db.conn.lock();

        // Table exists.
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='blind_spot_dismissals'",
                [],
                |row| row.get::<_, i64>(0).map(|c| c > 0),
            )
            .unwrap_or(false);
        assert!(
            table_exists,
            "blind_spot_dismissals table should exist after Phase 70 migration"
        );

        // Expected columns present.
        let mut stmt = conn
            .prepare("SELECT name FROM pragma_table_info('blind_spot_dismissals')")
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        for col in ["id", "item_id", "reason", "dismissed_at"] {
            assert!(
                cols.iter().any(|c| c == col),
                "blind_spot_dismissals column '{}' missing; got {:?}",
                col,
                cols
            );
        }

        // Index exists.
        let mut idx_stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='blind_spot_dismissals'")
            .unwrap();
        let indexes: Vec<String> = idx_stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(
            indexes.iter().any(|i| i == "idx_bsd_item"),
            "idx_bsd_item index missing; got {:?}",
            indexes
        );
    }

    /// Phase 71: dependency_snapshots table, indexes, and current_dependencies view.
    #[test]
    fn test_phase_71_dependency_snapshots() {
        let db = test_db();
        let conn = db.conn.lock();

        // Table exists.
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='dependency_snapshots'",
                [],
                |row| row.get::<_, i64>(0).map(|c| c > 0),
            )
            .unwrap_or(false);
        assert!(
            table_exists,
            "dependency_snapshots table should exist after Phase 71 migration"
        );

        // Expected columns present.
        let mut stmt = conn
            .prepare("SELECT name FROM pragma_table_info('dependency_snapshots')")
            .unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        for col in [
            "id",
            "project_path",
            "package_name",
            "ecosystem",
            "version",
            "is_direct",
            "is_dev",
            "source",
            "scanned_at",
        ] {
            assert!(
                cols.iter().any(|c| c == col),
                "dependency_snapshots column '{}' missing; got {:?}",
                col,
                cols
            );
        }

        // Indexes exist.
        let mut idx_stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='dependency_snapshots'")
            .unwrap();
        let indexes: Vec<String> = idx_stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        for idx in ["idx_ds_project", "idx_ds_package", "idx_ds_scanned"] {
            assert!(
                indexes.iter().any(|i| i == idx),
                "index '{}' missing on dependency_snapshots; got {:?}",
                idx,
                indexes
            );
        }

        // View exists.
        let view_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='view' AND name='current_dependencies'",
                [],
                |row| row.get::<_, i64>(0).map(|c| c > 0),
            )
            .unwrap_or(false);
        assert!(
            view_exists,
            "current_dependencies view should exist after Phase 71 migration"
        );
    }
}
