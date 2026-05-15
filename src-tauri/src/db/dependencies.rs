// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Dependency Intelligence — CRUD operations for user_dependencies and dependency_alerts.
//!
//! Stores dependencies discovered by ACE scanner and alerts detected from content analysis.

use rusqlite::{params, Result as SqliteResult};
use serde::{Deserialize, Serialize};

use super::Database;

// ============================================================================
// Types
// ============================================================================

/// A dependency stored in user_dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredDependency {
    pub id: i64,
    pub project_path: String,
    pub package_name: String,
    pub version: Option<String>,
    pub ecosystem: String,
    pub is_dev: bool,
    pub is_direct: bool,
    pub detected_at: String,
    pub last_seen_at: String,
    pub license: Option<String>,
}

/// A package used across multiple projects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossProjectPackage {
    pub package_name: String,
    pub ecosystem: String,
    pub project_count: i64,
    pub projects: Vec<String>,
}

/// An alert associated with a dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyAlert {
    pub id: i64,
    pub package_name: String,
    pub ecosystem: String,
    pub alert_type: String,
    pub severity: String,
    pub title: String,
    pub description: Option<String>,
    pub affected_versions: Option<String>,
    pub source_url: Option<String>,
    pub source_item_id: Option<i64>,
    pub detected_at: String,
    pub resolved_at: Option<String>,
}

// ============================================================================
// Database Operations
// ============================================================================

impl Database {
    /// Store (upsert) a dependency discovered by ACE scanner.
    pub fn store_dependency(
        &self,
        project_path: &str,
        package_name: &str,
        version: Option<&str>,
        ecosystem: &str,
        is_dev: bool,
        license: Option<&str>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, license, detected_at, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, datetime('now'), datetime('now'))
             ON CONFLICT(project_path, package_name, ecosystem)
             DO UPDATE SET
                version = COALESCE(?3, user_dependencies.version),
                is_dev = ?5,
                license = COALESCE(?6, user_dependencies.license),
                last_seen_at = datetime('now')",
            params![project_path, package_name, version, ecosystem, is_dev as i32, license],
        )?;
        Ok(())
    }

    /// Store (upsert) a transitive dependency discovered from a lockfile.
    /// Sets `is_direct = 0` for new entries. On conflict, preserves existing
    /// `is_direct` value (so direct deps from manifests are not downgraded).
    /// Lockfile version is preferred (it's the actual resolved/installed version).
    pub fn store_transitive_dependency(
        &self,
        project_path: &str,
        package_name: &str,
        version: Option<&str>,
        ecosystem: &str,
        is_dev: bool,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, datetime('now'), datetime('now'))
             ON CONFLICT(project_path, package_name, ecosystem)
             DO UPDATE SET
                version = COALESCE(?3, user_dependencies.version),
                is_dev = MIN(user_dependencies.is_dev, ?5),
                last_seen_at = datetime('now')",
            params![project_path, package_name, version, ecosystem, is_dev as i32],
        )?;
        Ok(())
    }

    /// Get all dependencies for a specific project.
    pub fn get_project_dependencies(
        &self,
        project_path: &str,
    ) -> SqliteResult<Vec<StoredDependency>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at, license
             FROM user_dependencies
             WHERE project_path = ?1
             ORDER BY package_name",
        )?;

        let rows = stmt.query_map(params![project_path], map_dependency_row)?;
        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in dependencies: {e}");
                    None
                }
            })
            .collect())
    }

    /// Get all tracked dependencies across all projects.
    pub fn get_all_user_dependencies(&self) -> SqliteResult<Vec<StoredDependency>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at, license
             FROM user_dependencies
             ORDER BY ecosystem, package_name",
        )?;

        let rows = stmt.query_map([], map_dependency_row)?;
        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in dependencies: {e}");
                    None
                }
            })
            .collect())
    }

    /// Get user dependencies filtered to relevant runtime deps only.
    ///
    /// Excludes dev deps, transitive deps, and worktree paths to prevent
    /// inflated advisory matches from agent-generated worktree copies.
    pub fn get_relevant_user_dependencies(&self) -> SqliteResult<Vec<StoredDependency>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at, license
             FROM user_dependencies
             WHERE is_dev = 0 AND is_direct = 1
               AND project_path NOT LIKE '%/.claude/worktrees/%'
               AND project_path NOT LIKE '%\\.claude\\worktrees\\%'
             ORDER BY ecosystem, package_name",
        )?;

        let rows = stmt.query_map([], map_dependency_row)?;
        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in dependencies: {e}");
                    None
                }
            })
            .collect())
    }

    /// Get all ACE-scanned dependencies from `project_dependencies`.
    /// Returns them as `StoredDependency` for compatibility with OSV matching.
    /// Maps `language` → `ecosystem` and `last_scanned` → `detected_at`/`last_seen_at`.
    pub fn get_all_scanned_dependencies(&self) -> SqliteResult<Vec<StoredDependency>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, project_path, package_name, version, language, is_dev, is_direct, last_scanned
             FROM project_dependencies
             ORDER BY language, package_name",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(StoredDependency {
                id: row.get(0)?,
                project_path: row.get(1)?,
                package_name: row.get(2)?,
                version: row.get(3)?,
                ecosystem: row.get::<_, String>(4)?,
                is_dev: row.get::<_, bool>(5)?,
                is_direct: row.get::<_, bool>(6)?,
                detected_at: row.get::<_, String>(7)?,
                last_seen_at: row.get::<_, String>(7)?,
                license: None,
            })
        })?;

        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in scanned dependencies: {e}");
                    None
                }
            })
            .collect())
    }

    /// Get ACE-scanned dependencies filtered to relevant runtime deps only.
    ///
    /// Excludes dev deps, transitive deps, and low-relevance projects
    /// (example/demo/test directories). Falls back gracefully if `is_direct`
    /// or `project_relevance` columns don't exist in older databases.
    pub fn get_relevant_scanned_dependencies(&self) -> SqliteResult<Vec<StoredDependency>> {
        let conn = self.conn.lock();

        // Check which filter columns exist (Phase 53 added is_direct, Phase 55 added project_relevance)
        let has_direct = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('project_dependencies') WHERE name = 'is_direct'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;

        let has_relevance = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('project_dependencies') WHERE name = 'project_relevance'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;

        let direct_clause = if has_direct { "AND is_direct = 1" } else { "" };
        let relevance_clause = if has_relevance {
            "AND project_relevance >= 0.15"
        } else {
            ""
        };
        let direct_col = if has_direct {
            "is_direct"
        } else {
            "1 as is_direct"
        };

        let sql = format!(
            "SELECT id, project_path, package_name, version, language, is_dev, {direct_col}, last_scanned
             FROM project_dependencies
             WHERE is_dev = 0
               {direct_clause}
               {relevance_clause}
             ORDER BY language, package_name"
        );

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(StoredDependency {
                id: row.get(0)?,
                project_path: row.get(1)?,
                package_name: row.get(2)?,
                version: row.get(3)?,
                ecosystem: row.get::<_, String>(4)?,
                is_dev: false,
                is_direct: row.get::<_, bool>(6)?,
                detected_at: row.get::<_, String>(7)?,
                last_seen_at: row.get::<_, String>(7)?,
                license: None,
            })
        })?;

        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in relevant scanned dependencies: {e}");
                    None
                }
            })
            .collect())
    }

    /// Get packages that appear in multiple projects (cross-project insight).
    pub fn get_cross_project_packages(&self) -> SqliteResult<Vec<CrossProjectPackage>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT package_name, ecosystem, COUNT(DISTINCT project_path) as project_count,
                    GROUP_CONCAT(DISTINCT project_path) as projects
             FROM user_dependencies
             GROUP BY package_name, ecosystem
             HAVING project_count > 1
             ORDER BY project_count DESC, package_name",
        )?;

        let rows = stmt.query_map([], |row| {
            let projects_str: String = row.get(3)?;
            let projects: Vec<String> = projects_str
                .split(',')
                .map(std::string::ToString::to_string)
                .collect();
            Ok(CrossProjectPackage {
                package_name: row.get(0)?,
                ecosystem: row.get(1)?,
                project_count: row.get(2)?,
                projects,
            })
        })?;

        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in dependencies: {e}");
                    None
                }
            })
            .collect())
    }

    /// Check if an alert already exists for this package/ecosystem/title combination.
    pub fn alert_exists(
        &self,
        package_name: &str,
        ecosystem: &str,
        title: &str,
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM dependency_alerts WHERE package_name = ?1 AND ecosystem = ?2 AND title = ?3 AND resolved_at IS NULL",
            params![package_name, ecosystem, title],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Store a new dependency alert, skipping duplicates.
    /// Returns the row ID if inserted, or 0 if the alert already exists.
    pub fn store_dependency_alert(&self, alert: &DependencyAlert) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        // Check for existing unresolved alert with same package/ecosystem/title
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM dependency_alerts WHERE package_name = ?1 AND ecosystem = ?2 AND title = ?3 AND resolved_at IS NULL",
                params![alert.package_name, alert.ecosystem, alert.title],
                |row| row.get::<_, i64>(0).map(|c| c > 0),
            )
            .unwrap_or(false);

        if exists {
            return Ok(0); // Duplicate — skip insertion
        }

        conn.execute(
            "INSERT INTO dependency_alerts (package_name, ecosystem, alert_type, severity, title, description, affected_versions, source_url, source_item_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                alert.package_name,
                alert.ecosystem,
                alert.alert_type,
                alert.severity,
                alert.title,
                alert.description,
                alert.affected_versions,
                alert.source_url,
                alert.source_item_id,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get all active (unresolved) alerts.
    pub fn get_active_alerts(&self) -> SqliteResult<Vec<DependencyAlert>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, package_name, ecosystem, alert_type, severity, title, description,
                    affected_versions, source_url, source_item_id, detected_at, resolved_at
             FROM dependency_alerts
             WHERE resolved_at IS NULL
             ORDER BY
                CASE severity
                    WHEN 'critical' THEN 0
                    WHEN 'high' THEN 1
                    WHEN 'medium' THEN 2
                    WHEN 'low' THEN 3
                    ELSE 4
                END,
                detected_at DESC",
        )?;

        let rows = stmt.query_map([], map_alert_row)?;
        Ok(rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Row processing failed in dependency_alerts: {e}");
                    None
                }
            })
            .collect())
    }

    /// Resolve (dismiss) an alert by ID.
    pub fn resolve_alert(&self, alert_id: i64) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE dependency_alerts SET resolved_at = datetime('now') WHERE id = ?1",
            params![alert_id],
        )?;
        Ok(())
    }
}

// ============================================================================
// Row Mappers
// ============================================================================

fn map_dependency_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredDependency> {
    Ok(StoredDependency {
        id: row.get(0)?,
        project_path: row.get(1)?,
        package_name: row.get(2)?,
        version: row.get(3)?,
        ecosystem: row.get(4)?,
        is_dev: row.get::<_, i32>(5)? != 0,
        is_direct: row.get::<_, i32>(6)? != 0,
        detected_at: row.get(7)?,
        last_seen_at: row.get(8)?,
        license: row.get(9)?,
    })
}

fn map_alert_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DependencyAlert> {
    Ok(DependencyAlert {
        id: row.get(0)?,
        package_name: row.get(1)?,
        ecosystem: row.get(2)?,
        alert_type: row.get(3)?,
        severity: row.get(4)?,
        title: row.get(5)?,
        description: row.get(6)?,
        affected_versions: row.get(7)?,
        source_url: row.get(8)?,
        source_item_id: row.get(9)?,
        detected_at: row.get(10)?,
        resolved_at: row.get(11)?,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::test_utils::test_db;

    #[test]
    fn test_store_and_retrieve_dependency() {
        let db = test_db();
        db.store_dependency(
            "/projects/myapp",
            "tokio",
            Some("1.35.0"),
            "rust",
            false,
            Some("MIT"),
        )
        .unwrap();
        db.store_dependency(
            "/projects/myapp",
            "serde",
            None,
            "rust",
            false,
            Some("MIT OR Apache-2.0"),
        )
        .unwrap();
        db.store_dependency(
            "/projects/myapp",
            "pretty_assertions",
            None,
            "rust",
            true,
            None,
        )
        .unwrap();

        let deps = db.get_project_dependencies("/projects/myapp").unwrap();
        assert_eq!(deps.len(), 3);

        let tokio = deps.iter().find(|d| d.package_name == "tokio").unwrap();
        assert_eq!(tokio.version.as_deref(), Some("1.35.0"));
        assert_eq!(tokio.ecosystem, "rust");
        assert!(!tokio.is_dev);
        assert_eq!(tokio.license.as_deref(), Some("MIT"));

        let pa = deps
            .iter()
            .find(|d| d.package_name == "pretty_assertions")
            .unwrap();
        assert!(pa.is_dev);
        assert_eq!(pa.license, None);
    }

    #[test]
    fn test_upsert_updates_last_seen() {
        let db = test_db();
        db.store_dependency(
            "/projects/myapp",
            "react",
            Some("18.0.0"),
            "javascript",
            false,
            Some("MIT"),
        )
        .unwrap();
        // Upsert with new version
        db.store_dependency(
            "/projects/myapp",
            "react",
            Some("19.0.0"),
            "javascript",
            false,
            None,
        )
        .unwrap();

        let deps = db.get_project_dependencies("/projects/myapp").unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].version.as_deref(), Some("19.0.0"));
        // License should be preserved from the first insert (COALESCE keeps existing)
        assert_eq!(deps[0].license.as_deref(), Some("MIT"));
    }

    #[test]
    fn test_cross_project_packages() {
        let db = test_db();
        db.store_dependency("/projects/app1", "serde", None, "rust", false, None)
            .unwrap();
        db.store_dependency("/projects/app2", "serde", None, "rust", false, None)
            .unwrap();
        db.store_dependency("/projects/app1", "tokio", None, "rust", false, None)
            .unwrap();

        let cross = db.get_cross_project_packages().unwrap();
        assert_eq!(cross.len(), 1);
        assert_eq!(cross[0].package_name, "serde");
        assert_eq!(cross[0].project_count, 2);
    }

    #[test]
    fn test_store_and_resolve_alert() {
        use super::DependencyAlert;

        let db = test_db();
        let alert = DependencyAlert {
            id: 0,
            package_name: "lodash".to_string(),
            ecosystem: "javascript".to_string(),
            alert_type: "vulnerability".to_string(),
            severity: "critical".to_string(),
            title: "Prototype pollution in lodash < 4.17.21".to_string(),
            description: Some("CVE-2021-23337".to_string()),
            affected_versions: Some("< 4.17.21".to_string()),
            source_url: None,
            source_item_id: None,
            detected_at: String::new(),
            resolved_at: None,
        };

        let alert_id = db.store_dependency_alert(&alert).unwrap();
        assert!(alert_id > 0);

        let active = db.get_active_alerts().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].package_name, "lodash");

        db.resolve_alert(alert_id).unwrap();
        let active_after = db.get_active_alerts().unwrap();
        assert_eq!(active_after.len(), 0);
    }

    #[test]
    fn test_get_all_user_dependencies() {
        let db = test_db();
        db.store_dependency("/projects/app1", "tokio", None, "rust", false, None)
            .unwrap();
        db.store_dependency("/projects/app2", "react", None, "javascript", false, None)
            .unwrap();

        let all = db.get_all_user_dependencies().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_alert_deduplication() {
        use super::DependencyAlert;

        let db = test_db();
        let alert = DependencyAlert {
            id: 0,
            package_name: "lodash".to_string(),
            ecosystem: "javascript".to_string(),
            alert_type: "vulnerability".to_string(),
            severity: "critical".to_string(),
            title: "Prototype pollution".to_string(),
            description: None,
            affected_versions: None,
            source_url: None,
            source_item_id: None,
            detected_at: String::new(),
            resolved_at: None,
        };

        // First insert should succeed
        let id1 = db.store_dependency_alert(&alert).unwrap();
        assert!(id1 > 0);

        // Second insert of same alert should be skipped (returns 0)
        let id2 = db.store_dependency_alert(&alert).unwrap();
        assert_eq!(id2, 0, "Duplicate alert should return 0");

        // Only one alert should exist
        let active = db.get_active_alerts().unwrap();
        assert_eq!(active.len(), 1);

        // alert_exists should return true
        assert!(db
            .alert_exists("lodash", "javascript", "Prototype pollution")
            .unwrap());
        assert!(!db
            .alert_exists("lodash", "javascript", "Different title")
            .unwrap());
    }

    #[test]
    fn test_transitive_dependency_storage() {
        let db = test_db();

        // Store a direct dependency first
        db.store_dependency(
            "/projects/myapp",
            "serde",
            Some("1.0.204"),
            "rust",
            false,
            None,
        )
        .unwrap();

        // Store a transitive dependency
        db.store_transitive_dependency(
            "/projects/myapp",
            "serde_derive",
            Some("1.0.204"),
            "rust",
            false,
        )
        .unwrap();

        let deps = db.get_project_dependencies("/projects/myapp").unwrap();
        assert_eq!(deps.len(), 2);

        let serde = deps.iter().find(|d| d.package_name == "serde").unwrap();
        assert!(serde.is_direct, "Manifest dep should be direct");
        assert_eq!(serde.version.as_deref(), Some("1.0.204"));

        let serde_derive = deps
            .iter()
            .find(|d| d.package_name == "serde_derive")
            .unwrap();
        assert!(
            !serde_derive.is_direct,
            "Lockfile-only dep should be transitive"
        );
        assert_eq!(serde_derive.version.as_deref(), Some("1.0.204"));
    }

    #[test]
    fn test_get_relevant_user_dependencies_filters() {
        let db = test_db();
        // Direct, non-dev — should be included
        db.store_dependency(
            "/projects/myapp",
            "tokio",
            Some("1.35.0"),
            "rust",
            false,
            None,
        )
        .unwrap();
        // Dev dep — should be excluded
        db.store_dependency(
            "/projects/myapp",
            "pretty_assertions",
            None,
            "rust",
            true,
            None,
        )
        .unwrap();
        // Transitive — should be excluded
        db.store_transitive_dependency(
            "/projects/myapp",
            "serde_derive",
            Some("1.0.204"),
            "rust",
            false,
        )
        .unwrap();
        // Worktree path — should be excluded
        db.store_dependency(
            "/projects/.claude/worktrees/agent-abc123/myapp",
            "react",
            Some("18.0.0"),
            "javascript",
            false,
            None,
        )
        .unwrap();

        let relevant = db.get_relevant_user_dependencies().unwrap();
        assert_eq!(
            relevant.len(),
            1,
            "Only direct non-dev non-worktree deps should be returned"
        );
        assert_eq!(relevant[0].package_name, "tokio");
    }

    #[test]
    fn test_transitive_does_not_downgrade_direct() {
        let db = test_db();

        // Store as direct first (from manifest)
        db.store_dependency(
            "/projects/myapp",
            "tokio",
            Some("1.35.0"),
            "rust",
            false,
            None,
        )
        .unwrap();

        // Then store same package as transitive (from lockfile) — should NOT downgrade is_direct
        db.store_transitive_dependency("/projects/myapp", "tokio", Some("1.35.1"), "rust", false)
            .unwrap();

        let deps = db.get_project_dependencies("/projects/myapp").unwrap();
        assert_eq!(deps.len(), 1);

        let tokio = deps.iter().find(|d| d.package_name == "tokio").unwrap();
        assert!(
            tokio.is_direct,
            "Direct dep should stay direct even after transitive upsert"
        );
        // Version should be updated to lockfile version (COALESCE keeps non-null)
        assert_eq!(tokio.version.as_deref(), Some("1.35.1"));
    }

    /// Validates the startup cleanup SQL queries that purge worktree rows,
    /// deduplicate by normalized name+path, and remove ephemeral temp paths
    /// from user_dependencies (app_setup.rs startup cleanup block).
    #[test]
    fn test_startup_user_dependency_cleanup() {
        let db = test_db();
        let conn = db.conn.lock();

        // --- Seed test data ---

        // 3 worktree rows (should be purged by query 1)
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
             VALUES ('D:\\4DA\\.claude\\worktrees\\agent-abc123\\src', 'tokio', '1.35.0', 'rust', 0, 1, datetime('now'), datetime('now'))",
            [],
        ).expect("insert worktree row 1");
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
             VALUES ('D:\\4DA\\.claude\\worktrees\\agent-def456\\src', 'serde', '1.0.0', 'rust', 0, 1, datetime('now'), datetime('now'))",
            [],
        ).expect("insert worktree row 2");
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
             VALUES ('/home/user/.claude/worktrees/agent-789/proj', 'react', '18.0.0', 'javascript', 0, 1, datetime('now'), datetime('now'))",
            [],
        ).expect("insert worktree row 3");

        // 2 casing duplicates of the same logical dep (query 2 keeps the latest rowid)
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
             VALUES ('D:\\Documents\\myapp', 'my-pkg', '1.0.0', 'rust', 0, 1, datetime('now'), datetime('now'))",
            [],
        ).expect("insert casing dup 1");
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
             VALUES ('D:\\documents\\myapp', 'my_pkg', '2.0.0', 'rust', 0, 1, datetime('now'), datetime('now'))",
            [],
        ).expect("insert casing dup 2");

        // 1 temp-path row (should be purged by query 3)
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
             VALUES ('C:\\Users\\Admin\\AppData\\Local\\Temp\\clone\\proj', 'axios', '1.0.0', 'javascript', 0, 1, datetime('now'), datetime('now'))",
            [],
        ).expect("insert temp row");

        // 2 clean rows (should survive all queries)
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
             VALUES ('D:\\4DA', 'tauri', '2.0.0', 'rust', 0, 1, datetime('now'), datetime('now'))",
            [],
        ).expect("insert clean row 1");
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
             VALUES ('D:\\projects\\web', 'vite', '5.0.0', 'javascript', 0, 1, datetime('now'), datetime('now'))",
            [],
        ).expect("insert clean row 2");

        // Verify starting count: 3 worktree + 2 dups + 1 temp + 2 clean = 8
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM user_dependencies", [], |r| r.get(0))
            .expect("count before");
        assert_eq!(before, 8, "Expected 8 rows before cleanup");

        // --- Query 1: purge worktree rows ---
        let deleted_worktree = conn
            .execute(
                "DELETE FROM user_dependencies WHERE project_path LIKE '%worktrees%agent-%'",
                [],
            )
            .expect("worktree purge");
        assert_eq!(deleted_worktree, 3, "Should purge 3 worktree rows");

        // --- Query 2: deduplicate by normalized name + path + ecosystem ---
        let deleted_dedup = conn
            .execute(
                "DELETE FROM user_dependencies WHERE rowid NOT IN (
                    SELECT MAX(rowid) FROM user_dependencies
                    GROUP BY LOWER(REPLACE(package_name, '-', '_')), LOWER(project_path), LOWER(ecosystem)
                )",
                [],
            )
            .expect("dedup");
        assert_eq!(
            deleted_dedup, 1,
            "Should deduplicate 1 casing/hyphen variant"
        );

        // --- Query 3: purge temp paths ---
        let deleted_temp = conn
            .execute(
                "DELETE FROM user_dependencies WHERE project_path LIKE '%/tmp/%' OR project_path LIKE '%\\tmp\\%' OR project_path LIKE '%AppData%Local%Temp%'",
                [],
            )
            .expect("temp purge");
        assert_eq!(deleted_temp, 1, "Should purge 1 temp-path row");

        // --- Verify final state ---
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM user_dependencies", [], |r| r.get(0))
            .expect("count after");
        assert_eq!(
            remaining, 3,
            "Should have 3 rows remaining: 1 surviving dup + 2 clean"
        );

        // Verify the surviving rows are the expected ones
        let mut stmt = conn
            .prepare("SELECT package_name FROM user_dependencies ORDER BY package_name")
            .expect("prepare final query");
        let names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .expect("query names")
            .filter_map(|r| r.ok())
            .collect();
        // my_pkg survived (higher rowid than my-pkg), plus tauri and vite
        assert_eq!(names, vec!["my_pkg", "tauri", "vite"]);
    }
}
