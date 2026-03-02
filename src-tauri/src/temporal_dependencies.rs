//! Dependency and relationship management for the temporal store.
//!
//! Extracted from temporal.rs — handles project dependency CRUD and
//! cross-item relationship upserts.

use crate::temporal::ProjectDependency;
use rusqlite::params;

// ============================================================================
// Project Dependencies
// ============================================================================

/// Upsert a project dependency
pub fn upsert_dependency(
    conn: &rusqlite::Connection,
    project_path: &str,
    manifest_type: &str,
    package_name: &str,
    version: Option<&str>,
    is_dev: bool,
    language: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, language, last_scanned)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
         ON CONFLICT(project_path, package_name)
         DO UPDATE SET version = ?4, is_dev = ?5, last_scanned = datetime('now')",
        params![project_path, manifest_type, package_name, version, is_dev as i32, language],
    )
    .map_err(|e| format!("Failed to upsert dependency: {}", e))?;
    Ok(())
}

/// Get all dependencies for a project
pub fn get_project_dependencies(
    conn: &rusqlite::Connection,
    project_path: &str,
) -> Result<Vec<ProjectDependency>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, project_path, manifest_type, package_name, version, is_dev, language, last_scanned
             FROM project_dependencies
             WHERE project_path = ?1
             ORDER BY package_name",
        )
        .map_err(|e| e.to_string())?;

    let results: Vec<ProjectDependency> = stmt
        .query_map(params![project_path], |row| {
            Ok(ProjectDependency {
                id: row.get(0)?,
                project_path: row.get(1)?,
                manifest_type: row.get(2)?,
                package_name: row.get(3)?,
                version: row.get(4)?,
                is_dev: row.get::<_, i32>(5)? != 0,
                language: row.get(6)?,
                last_scanned: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

/// Get all tracked dependencies, scoped to projects with recent git activity.
/// Only includes deps from project trees that have commits in the last 60 days.
/// Falls back to all deps if no git signals exist (first run).
pub fn get_all_dependencies(conn: &rusqlite::Connection) -> Result<Vec<ProjectDependency>, String> {
    // Get active repo roots from git_signals (repos with recent commits)
    let active_roots: Vec<String> = conn
        .prepare(
            "SELECT DISTINCT repo_path FROM git_signals
             WHERE commit_hash IS NOT NULL AND commit_hash != ''
             AND timestamp > datetime('now', '-60 days')",
        )
        .and_then(|mut stmt| {
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    let mut stmt = conn
        .prepare(
            "SELECT id, project_path, manifest_type, package_name, version, is_dev, language, last_scanned
             FROM project_dependencies
             ORDER BY project_path, package_name",
        )
        .map_err(|e| e.to_string())?;

    let all_deps: Vec<ProjectDependency> = stmt
        .query_map([], |row| {
            Ok(ProjectDependency {
                id: row.get(0)?,
                project_path: row.get(1)?,
                manifest_type: row.get(2)?,
                package_name: row.get(3)?,
                version: row.get(4)?,
                is_dev: row.get::<_, i32>(5)? != 0,
                language: row.get(6)?,
                last_scanned: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Filter to deps from active project trees only
    if active_roots.is_empty() {
        // No git signals yet — return all deps (first run fallback)
        return Ok(all_deps);
    }

    let filtered: Vec<ProjectDependency> = all_deps
        .into_iter()
        .filter(|dep| {
            // Normalize for case-insensitive comparison on Windows
            let dep_path = dep.project_path.to_lowercase();
            active_roots.iter().any(|root| {
                let root_lower = root.to_lowercase();
                dep_path.starts_with(&root_lower) || root_lower.starts_with(&dep_path)
            })
        })
        .collect();

    // If filtering eliminated everything, fall back to all deps
    if filtered.is_empty() {
        return Ok(conn
            .prepare(
                "SELECT id, project_path, manifest_type, package_name, version, is_dev, language, last_scanned
                 FROM project_dependencies ORDER BY project_path, package_name",
            )
            .map_err(|e| e.to_string())?
            .query_map([], |row| {
                Ok(ProjectDependency {
                    id: row.get(0)?,
                    project_path: row.get(1)?,
                    manifest_type: row.get(2)?,
                    package_name: row.get(3)?,
                    version: row.get(4)?,
                    is_dev: row.get::<_, i32>(5)? != 0,
                    language: row.get(6)?,
                    last_scanned: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect());
    }

    Ok(filtered)
}

// ============================================================================
// Item Relationships
// ============================================================================

/// Create or update an item relationship
pub fn upsert_relationship(
    conn: &rusqlite::Connection,
    source_item_id: i64,
    related_item_id: i64,
    relationship_type: &str,
    strength: f64,
    metadata: Option<&serde_json::Value>,
) -> Result<(), String> {
    let metadata_str = metadata.map(|m| serde_json::to_string(m).unwrap_or_default());
    conn.execute(
        "INSERT INTO item_relationships (source_item_id, related_item_id, relationship_type, strength, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(source_item_id, related_item_id, relationship_type)
         DO UPDATE SET strength = ?4, metadata = ?5",
        params![source_item_id, related_item_id, relationship_type, strength, metadata_str],
    )
    .map_err(|e| format!("Failed to upsert relationship: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temporal::ProjectDependency;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_dependencies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                manifest_type TEXT NOT NULL,
                package_name TEXT NOT NULL,
                version TEXT,
                is_dev BOOLEAN DEFAULT 0,
                language TEXT NOT NULL DEFAULT 'unknown',
                last_scanned TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(project_path, package_name)
            );
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
            CREATE TABLE IF NOT EXISTS git_signals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                repo_path TEXT NOT NULL,
                commit_hash TEXT,
                timestamp TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .expect("create tables");
        conn
    }

    #[test]
    fn upsert_and_get_project_dependencies() {
        let conn = setup_test_db();
        upsert_dependency(
            &conn,
            "/home/user/proj",
            "cargo.toml",
            "serde",
            Some("1.0.193"),
            false,
            "rust",
        )
        .unwrap();
        upsert_dependency(
            &conn,
            "/home/user/proj",
            "cargo.toml",
            "tokio",
            Some("1.35.0"),
            false,
            "rust",
        )
        .unwrap();
        upsert_dependency(
            &conn,
            "/home/user/proj",
            "cargo.toml",
            "tracing",
            Some("0.1.40"),
            true,
            "rust",
        )
        .unwrap();

        let deps = get_project_dependencies(&conn, "/home/user/proj").unwrap();
        assert_eq!(deps.len(), 3);
        // Ordered by package_name
        assert_eq!(deps[0].package_name, "serde");
        assert_eq!(deps[1].package_name, "tokio");
        assert_eq!(deps[2].package_name, "tracing");
        assert!(!deps[0].is_dev);
        assert!(deps[2].is_dev);
        assert_eq!(deps[0].version, Some("1.0.193".to_string()));
    }

    #[test]
    fn upsert_dependency_updates_existing() {
        let conn = setup_test_db();
        upsert_dependency(
            &conn,
            "/proj",
            "cargo.toml",
            "serde",
            Some("1.0.100"),
            false,
            "rust",
        )
        .unwrap();
        upsert_dependency(
            &conn,
            "/proj",
            "cargo.toml",
            "serde",
            Some("1.0.200"),
            true,
            "rust",
        )
        .unwrap();

        let deps = get_project_dependencies(&conn, "/proj").unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].version, Some("1.0.200".to_string()));
        assert!(deps[0].is_dev);
    }

    #[test]
    fn get_all_dependencies_fallback_no_git_signals() {
        let conn = setup_test_db();
        upsert_dependency(
            &conn,
            "/proj_a",
            "package.json",
            "react",
            Some("18.2.0"),
            false,
            "javascript",
        )
        .unwrap();
        upsert_dependency(
            &conn,
            "/proj_b",
            "cargo.toml",
            "serde",
            Some("1.0.0"),
            false,
            "rust",
        )
        .unwrap();

        // No git signals -> should return all deps
        let all = get_all_dependencies(&conn).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn get_project_dependencies_empty_for_unknown_path() {
        let conn = setup_test_db();
        upsert_dependency(
            &conn,
            "/real/path",
            "cargo.toml",
            "serde",
            Some("1.0"),
            false,
            "rust",
        )
        .unwrap();

        let deps = get_project_dependencies(&conn, "/other/path").unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn upsert_relationship_creates_and_updates() {
        let conn = setup_test_db();
        let meta = serde_json::json!({"reason": "similar topic"});
        upsert_relationship(&conn, 1, 2, "similar", 0.85, Some(&meta)).unwrap();

        // Verify it was created
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM item_relationships", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let strength: f64 = conn
            .query_row(
                "SELECT strength FROM item_relationships WHERE source_item_id = 1 AND related_item_id = 2",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!((strength - 0.85).abs() < 0.001);

        // Update strength via upsert
        upsert_relationship(&conn, 1, 2, "similar", 0.95, None).unwrap();
        let updated: f64 = conn
            .query_row(
                "SELECT strength FROM item_relationships WHERE source_item_id = 1 AND related_item_id = 2",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!((updated - 0.95).abs() < 0.001);

        // Still only one row (upsert, not duplicate)
        let count2: i64 = conn
            .query_row("SELECT COUNT(*) FROM item_relationships", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count2, 1);
    }

    #[test]
    fn upsert_relationship_different_types_creates_separate_rows() {
        let conn = setup_test_db();
        upsert_relationship(&conn, 1, 2, "similar", 0.8, None).unwrap();
        upsert_relationship(&conn, 1, 2, "references", 0.6, None).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM item_relationships", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn project_dependency_serde_roundtrip() {
        let dep = ProjectDependency {
            id: 1,
            project_path: "/home/user/myapp".to_string(),
            manifest_type: "cargo.toml".to_string(),
            package_name: "tokio".to_string(),
            version: Some("1.35.0".to_string()),
            is_dev: false,
            language: "rust".to_string(),
            last_scanned: "2026-02-28T12:00:00".to_string(),
        };
        let json = serde_json::to_string(&dep).unwrap();
        let deserialized: ProjectDependency = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.package_name, "tokio");
        assert!(!deserialized.is_dev);
        assert_eq!(deserialized.language, "rust");
    }
}
