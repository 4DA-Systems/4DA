// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Dependency Snapshots — point-in-time snapshots of project dependencies.
//!
//! Provides a snapshot-based view of project dependencies (as opposed to the
//! append-heavy `user_dependencies` table). The `dependency_snapshots` table
//! records each scan result, and the `current_dependencies` SQL view surfaces
//! only the latest snapshot per (project, package, ecosystem) triple.

use anyhow::{Context, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Database;

// ============================================================================
// Types
// ============================================================================

/// A single dependency captured during a project scan snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DepSnapshot {
    pub id: i64,
    pub project_path: String,
    pub package_name: String,
    pub ecosystem: String,
    pub version: Option<String>,
    pub is_direct: bool,
    pub is_dev: bool,
    pub source: String,
    pub scanned_at: String,
}

/// Input tuple for batch-upserting dependencies into a snapshot.
#[derive(Debug, Clone)]
pub struct DepEntry {
    pub name: String,
    pub ecosystem: String,
    pub version: Option<String>,
    pub is_direct: bool,
    pub is_dev: bool,
    pub source: String,
}

/// Retention window for `dependency_snapshots`, in days.
///
/// A snapshot row's `scanned_at` is bumped by `snapshot_project_deps` on every
/// re-scan of its project, so a row older than this belongs to a project ACE
/// has not seen in three months — a deleted checkout, a renamed directory, or
/// a path the user dropped from their context dirs. Matches the 90-day window
/// the module's own expiry test has always asserted.
pub(crate) const SNAPSHOT_RETENTION_DAYS: i64 = 90;

/// Delete `dependency_snapshots` rows older than `older_than_days`, using a
/// caller-supplied connection.
///
/// Exists separately from [`Database::expire_stale_snapshots`] so the DB
/// maintenance sweep can run this INSIDE its open transaction: `self.conn` is a
/// non-reentrant `parking_lot::Mutex`, so a method that re-locks it would
/// deadlock against the guard `run_maintenance` already holds.
pub(crate) fn expire_stale_snapshots_on(
    conn: &rusqlite::Connection,
    older_than_days: i64,
) -> Result<usize> {
    let deleted = conn
        .execute(
            "DELETE FROM dependency_snapshots
             WHERE scanned_at < datetime('now', ?1)",
            params![format!("-{older_than_days} days")],
        )
        .context("expire stale snapshots")?;
    Ok(deleted)
}

/// Delete this project's snapshot rows that the scan no longer lists, per
/// ecosystem present in `deps`. An ecosystem the scan did not cover is left
/// alone, and an empty `deps` deletes nothing (an empty parse must not wipe
/// a project).
fn prune_unlisted(
    conn: &rusqlite::Connection,
    project_path: &str,
    deps: &[DepEntry],
) -> Result<usize> {
    let mut by_ecosystem: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
    for dep in deps {
        by_ecosystem
            .entry(dep.ecosystem.as_str())
            .or_default()
            .push(dep.name.as_str());
    }
    let mut removed = 0usize;
    for (ecosystem, names) in by_ecosystem {
        let placeholders = vec!["?"; names.len()].join(", ");
        let sql = format!(
            "DELETE FROM dependency_snapshots
             WHERE project_path = ? AND ecosystem = ?
               AND package_name NOT IN ({placeholders})"
        );
        let mut values: Vec<&str> = Vec::with_capacity(names.len() + 2);
        values.push(project_path);
        values.push(ecosystem);
        values.extend(names);
        removed += conn
            .execute(&sql, rusqlite::params_from_iter(values))
            .with_context(|| format!("prune unlisted {ecosystem} snapshots"))?;
    }
    Ok(removed)
}

// ============================================================================
// Database Operations
// ============================================================================

impl Database {
    /// Batch-upsert a set of dependencies for a project into `dependency_snapshots`.
    ///
    /// Each entry is inserted with `ON CONFLICT ... DO UPDATE` on the
    /// `(project_path, package_name, ecosystem)` unique constraint, so
    /// re-scanning the same project replaces the previous snapshot row
    /// for each dependency.
    ///
    /// The scan is also the truth about what is GONE: per ecosystem in `deps`,
    /// rows of this project that the scan no longer lists are deleted, in the
    /// same transaction. Until 2026-11-01 only the 90-day expiry removed
    /// them, so a dropped direct dependency kept counting as the user's stack
    /// in the dep_linker for up to 90 days (48 such rows live). Same scope and
    /// empty-list guard as `temporal::prune_removed_dependencies`, which runs
    /// on `project_dependencies` right before this call.
    pub fn snapshot_project_deps(&self, project_path: &str, deps: &[DepEntry]) -> Result<usize> {
        // Agent worktrees / scratch fixtures / temp clones are not user
        // projects — same write-time guard as user_dependencies (this table
        // held 822 stale agent-worktree rows before the guard existed).
        if super::dependencies::is_excluded_project_path(project_path) {
            return Ok(0);
        }
        // Store on the canonical key (forward slashes; lowercase on Windows)
        // so snapshot rows land on the SAME identity as user_dependencies /
        // project_dependencies rows for the project, regardless of whether
        // the caller passed a raw `D:\...` or canonical `d:/...` path.
        let project_path = crate::project_inclusion::canonical_storage_path(project_path);
        let project_path = project_path.as_str();
        let conn = self.conn.lock();
        let tx = conn
            .unchecked_transaction()
            .context("begin snapshot transaction")?;

        let mut count = 0usize;
        for dep in deps {
            tx.execute(
                "INSERT INTO dependency_snapshots
                    (project_path, package_name, ecosystem, version, is_direct, is_dev, source, scanned_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP)
                 ON CONFLICT(project_path, package_name, ecosystem)
                 DO UPDATE SET
                    version = excluded.version,
                    is_direct = excluded.is_direct,
                    is_dev = excluded.is_dev,
                    source = excluded.source,
                    scanned_at = CURRENT_TIMESTAMP",
                params![
                    project_path,
                    dep.name,
                    dep.ecosystem,
                    dep.version,
                    dep.is_direct as i32,
                    dep.is_dev as i32,
                    dep.source,
                ],
            )
            .with_context(|| format!("upsert snapshot for {}/{}", dep.ecosystem, dep.name))?;
            count += 1;
        }
        prune_unlisted(&tx, project_path, deps)?;

        tx.commit().context("commit snapshot transaction")?;
        Ok(count)
    }

    /// Read the current (latest) dependencies for a project from the
    /// `current_dependencies` view.
    pub fn get_current_deps(&self, project_path: &str) -> Result<Vec<DepSnapshot>> {
        // Canonicalize the query path to match the canonical storage key
        // written by snapshot_project_deps.
        let project_path = crate::project_inclusion::canonical_storage_path(project_path);
        let conn = self.read_conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, project_path, package_name, ecosystem, version,
                        is_direct, is_dev, source, scanned_at
                 FROM current_dependencies
                 WHERE project_path = ?1
                 ORDER BY package_name",
            )
            .context("prepare current_dependencies query")?;

        let rows = stmt
            .query_map(params![project_path], |row| {
                Ok(DepSnapshot {
                    id: row.get(0)?,
                    project_path: row.get(1)?,
                    package_name: row.get(2)?,
                    ecosystem: row.get(3)?,
                    version: row.get(4)?,
                    is_direct: row.get::<_, i32>(5)? != 0,
                    is_dev: row.get::<_, i32>(6)? != 0,
                    source: row.get(7)?,
                    scanned_at: row.get(8)?,
                })
            })
            .context("query current_dependencies")?;

        let mut results = Vec::new();
        for row in rows {
            match row {
                Ok(dep) => results.push(dep),
                Err(e) => {
                    tracing::warn!(target: "4da::db", error = %e, "Row processing failed in dep_snapshots");
                }
            }
        }
        Ok(results)
    }

    /// Delete dependency snapshots older than `older_than_days` days.
    /// Returns the number of rows deleted.
    ///
    /// Production expiry runs from `run_maintenance` via
    /// [`expire_stale_snapshots_on`] (inside that sweep's transaction); this is
    /// the standalone entry point for callers that hold no connection.
    pub fn expire_stale_snapshots(&self, older_than_days: i64) -> Result<usize> {
        let conn = self.conn.lock();
        expire_stale_snapshots_on(&conn, older_than_days)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_db;

    fn sample_deps() -> Vec<DepEntry> {
        vec![
            DepEntry {
                name: "serde".into(),
                ecosystem: "cargo".into(),
                version: Some("1.0.200".into()),
                is_direct: true,
                is_dev: false,
                source: "manifest".into(),
            },
            DepEntry {
                name: "tokio".into(),
                ecosystem: "cargo".into(),
                version: Some("1.37.0".into()),
                is_direct: true,
                is_dev: false,
                source: "manifest".into(),
            },
        ]
    }

    /// Test 1: Snapshot upsert round-trip — insert deps and read them back
    /// via the current_dependencies view.
    #[test]
    fn test_snapshot_roundtrip() {
        let db = test_db();
        let deps = sample_deps();

        let count = db
            .snapshot_project_deps("/home/user/project", &deps)
            .expect("snapshot should succeed");
        assert_eq!(count, 2, "should report 2 upserted rows");

        let current = db
            .get_current_deps("/home/user/project")
            .expect("get_current_deps should succeed");
        assert_eq!(current.len(), 2, "should have 2 current deps");
        assert!(
            current.iter().any(|d| d.package_name == "serde"),
            "serde should be present"
        );
        assert!(
            current.iter().any(|d| d.package_name == "tokio"),
            "tokio should be present"
        );
    }

    /// Test 2: Re-scanning the same project updates existing rows (upsert),
    /// so current_dependencies still returns only one row per package.
    #[test]
    fn test_upsert_overwrites_previous_snapshot() {
        let db = test_db();

        // First scan
        let deps_v1 = vec![DepEntry {
            name: "serde".into(),
            ecosystem: "cargo".into(),
            version: Some("1.0.200".into()),
            is_direct: true,
            is_dev: false,
            source: "manifest".into(),
        }];
        db.snapshot_project_deps("/proj", &deps_v1).unwrap();

        // Second scan with updated version
        let deps_v2 = vec![DepEntry {
            name: "serde".into(),
            ecosystem: "cargo".into(),
            version: Some("1.0.210".into()),
            is_direct: true,
            is_dev: false,
            source: "manifest".into(),
        }];
        db.snapshot_project_deps("/proj", &deps_v2).unwrap();

        let current = db.get_current_deps("/proj").unwrap();
        assert_eq!(
            current.len(),
            1,
            "upsert should not duplicate — still 1 row"
        );
        assert_eq!(
            current[0].version.as_deref(),
            Some("1.0.210"),
            "version should be updated to latest"
        );
    }

    /// Test 3: Different projects maintain separate snapshots.
    #[test]
    fn test_separate_projects() {
        let db = test_db();

        let dep = vec![DepEntry {
            name: "anyhow".into(),
            ecosystem: "cargo".into(),
            version: Some("1.0.0".into()),
            is_direct: true,
            is_dev: false,
            source: "manifest".into(),
        }];

        db.snapshot_project_deps("/proj-a", &dep).unwrap();
        db.snapshot_project_deps("/proj-b", &dep).unwrap();

        let a = db.get_current_deps("/proj-a").unwrap();
        let b = db.get_current_deps("/proj-b").unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(b.len(), 1);
        assert_eq!(a[0].project_path, "/proj-a");
        assert_eq!(b[0].project_path, "/proj-b");
    }

    /// Test 4: expire_stale_snapshots removes old entries.
    #[test]
    fn test_expire_stale_snapshots() {
        let db = test_db();
        let deps = sample_deps();
        db.snapshot_project_deps("/proj", &deps).unwrap();

        // Artificially backdate the scanned_at to 100 days ago
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE dependency_snapshots SET scanned_at = datetime('now', '-100 days')",
                [],
            )
            .unwrap();
        }

        let deleted = db
            .expire_stale_snapshots(90)
            .expect("expire should succeed");
        assert_eq!(deleted, 2, "both deps should be expired (>90 days old)");

        let current = db.get_current_deps("/proj").unwrap();
        assert!(current.is_empty(), "no deps should remain after expiry");
    }

    /// Test 5: the expiry the DB maintenance sweep performs — run exactly the
    /// way `run_maintenance` runs it, from inside a transaction on an
    /// already-held connection.
    ///
    /// This is the regression guard for that wiring. Before it, nothing in
    /// production ever deleted from `dependency_snapshots`, so the table grew
    /// without bound. It pins three things the wiring depends on: the retention
    /// cutoff is applied per row (fresh projects survive), the delete works
    /// against a caller-supplied connection WITHOUT re-locking `self.conn`
    /// (which would deadlock — `parking_lot::Mutex` is not reentrant), and a
    /// second sweep is a no-op.
    #[test]
    fn test_maintenance_style_expiry_is_scoped_and_idempotent() {
        let db = test_db();
        db.snapshot_project_deps("/proj", &sample_deps()).unwrap();
        db.snapshot_project_deps("/fresh", &sample_deps()).unwrap();

        // Backdate only /proj past the retention window.
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE dependency_snapshots
                 SET scanned_at = datetime('now', ?1)
                 WHERE project_path = '/proj'",
                params![format!("-{} days", SNAPSHOT_RETENTION_DAYS + 10)],
            )
            .unwrap();
        }

        // Mirror run_maintenance: hold the writer lock, open a transaction, and
        // expire through the connection-taking helper.
        let sweep = || {
            let conn = db.conn.lock();
            let tx = conn.unchecked_transaction().unwrap();
            let deleted = expire_stale_snapshots_on(&tx, SNAPSHOT_RETENTION_DAYS).unwrap();
            tx.commit().unwrap();
            deleted
        };

        assert_eq!(sweep(), 2, "both stale /proj rows should be expired");
        assert!(
            db.get_current_deps("/proj").unwrap().is_empty(),
            "snapshots past the retention window must be gone"
        );
        assert_eq!(
            db.get_current_deps("/fresh").unwrap().len(),
            2,
            "snapshots inside the window must be untouched"
        );

        assert_eq!(sweep(), 0, "a second sweep must be a no-op");
        assert_eq!(db.get_current_deps("/fresh").unwrap().len(), 2);
    }

    /// Test 6: Empty deps list is a no-op, returning 0.
    #[test]
    fn test_empty_snapshot() {
        let db = test_db();
        let count = db
            .snapshot_project_deps("/proj", &[])
            .expect("empty snapshot should succeed");
        assert_eq!(count, 0);

        let current = db.get_current_deps("/proj").unwrap();
        assert!(current.is_empty());
    }

    fn entry(name: &str, ecosystem: &str) -> DepEntry {
        DepEntry {
            name: name.into(),
            ecosystem: ecosystem.into(),
            version: None,
            is_direct: true,
            is_dev: false,
            source: "manifest".into(),
        }
    }

    fn names(db: &crate::db::Database, project: &str) -> Vec<String> {
        let mut n: Vec<String> = db
            .get_current_deps(project)
            .unwrap()
            .into_iter()
            .map(|d| format!("{}:{}", d.ecosystem, d.package_name))
            .collect();
        n.sort();
        n
    }

    /// A dependency dropped from the manifest is gone after the rescan, not
    /// 90 days later; another ecosystem of the same project keeps its rows.
    #[test]
    fn a_rescan_drops_what_the_manifest_no_longer_lists() {
        let db = test_db();
        db.snapshot_project_deps("/proj", &[entry("serde", "rust"), entry("zip", "rust")])
            .unwrap();
        db.snapshot_project_deps("/proj", &[entry("react", "javascript")])
            .unwrap();

        db.snapshot_project_deps("/proj", &[entry("serde", "rust")])
            .unwrap();

        assert_eq!(names(&db, "/proj"), ["javascript:react", "rust:serde"]);
    }

    /// An empty scan (a manifest that parsed to nothing) wipes nothing.
    #[test]
    fn an_empty_rescan_keeps_the_existing_rows() {
        let db = test_db();
        db.snapshot_project_deps("/proj", &sample_deps()).unwrap();
        db.snapshot_project_deps("/proj", &[]).unwrap();
        assert_eq!(names(&db, "/proj").len(), 2);
    }
}
