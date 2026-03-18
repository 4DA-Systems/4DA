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
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, datetime('now'), datetime('now'))
             ON CONFLICT(project_path, package_name, ecosystem)
             DO UPDATE SET
                version = COALESCE(?3, user_dependencies.version),
                is_dev = ?5,
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
            "SELECT id, project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at
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
            "SELECT id, project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at
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
            let projects: Vec<String> = projects_str.split(',').map(|s| s.to_string()).collect();
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

    /// Store a new dependency alert.
    pub fn store_dependency_alert(&self, alert: &DependencyAlert) -> SqliteResult<i64> {
        let conn = self.conn.lock();
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
        db.store_dependency("/projects/myapp", "tokio", Some("1.35.0"), "rust", false)
            .unwrap();
        db.store_dependency("/projects/myapp", "serde", None, "rust", false)
            .unwrap();
        db.store_dependency("/projects/myapp", "pretty_assertions", None, "rust", true)
            .unwrap();

        let deps = db.get_project_dependencies("/projects/myapp").unwrap();
        assert_eq!(deps.len(), 3);

        let tokio = deps.iter().find(|d| d.package_name == "tokio").unwrap();
        assert_eq!(tokio.version.as_deref(), Some("1.35.0"));
        assert_eq!(tokio.ecosystem, "rust");
        assert!(!tokio.is_dev);

        let pa = deps
            .iter()
            .find(|d| d.package_name == "pretty_assertions")
            .unwrap();
        assert!(pa.is_dev);
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
        )
        .unwrap();
        // Upsert with new version
        db.store_dependency(
            "/projects/myapp",
            "react",
            Some("19.0.0"),
            "javascript",
            false,
        )
        .unwrap();

        let deps = db.get_project_dependencies("/projects/myapp").unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].version.as_deref(), Some("19.0.0"));
    }

    #[test]
    fn test_cross_project_packages() {
        let db = test_db();
        db.store_dependency("/projects/app1", "serde", None, "rust", false)
            .unwrap();
        db.store_dependency("/projects/app2", "serde", None, "rust", false)
            .unwrap();
        db.store_dependency("/projects/app1", "tokio", None, "rust", false)
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
        db.store_dependency("/projects/app1", "tokio", None, "rust", false)
            .unwrap();
        db.store_dependency("/projects/app2", "react", None, "javascript", false)
            .unwrap();

        let all = db.get_all_user_dependencies().unwrap();
        assert_eq!(all.len(), 2);
    }
}
