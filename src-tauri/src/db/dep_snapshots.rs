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
    pub fn snapshot_project_deps(&self, project_path: &str, deps: &[DepEntry]) -> Result<usize> {
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

        tx.commit().context("commit snapshot transaction")?;
        Ok(count)
    }

    /// Read the current (latest) dependencies for a project from the
    /// `current_dependencies` view.
    pub fn get_current_deps(&self, project_path: &str) -> Result<Vec<DepSnapshot>> {
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
    pub fn expire_stale_snapshots(&self, older_than_days: i64) -> Result<usize> {
        let conn = self.conn.lock();
        let deleted = conn
            .execute(
                "DELETE FROM dependency_snapshots
                 WHERE scanned_at < datetime('now', ?1)",
                params![format!("-{older_than_days} days")],
            )
            .context("expire stale snapshots")?;
        Ok(deleted)
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

    /// Test 5: Empty deps list is a no-op, returning 0.
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
}
