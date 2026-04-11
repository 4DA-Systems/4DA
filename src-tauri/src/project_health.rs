//! Project Health Radar for 4DA
//!
//! Per-project health dashboard combining dependency freshness,
//! security exposure, ecosystem momentum, and community signals.

use crate::error::Result;
use crate::project_health_dimensions::{
    compute_community, compute_freshness, compute_momentum, compute_security, generate_alerts,
};
use serde::{Deserialize, Serialize};
use tracing::warn;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectHealth {
    pub project_path: String,
    pub project_name: String,
    pub overall_score: f32,
    pub freshness: HealthDimension,
    pub security: HealthDimension,
    pub momentum: HealthDimension,
    pub community: HealthDimension,
    pub alerts: Vec<HealthAlert>,
    pub last_checked: String,
    pub dependency_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthDimension {
    pub score: f32,
    pub label: String,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthAlert {
    pub severity: String,
    pub message: String,
    pub dependency: Option<String>,
}

// ============================================================================
// Implementation
// ============================================================================

/// Compute health for all tracked projects
pub fn compute_all_project_health(conn: &rusqlite::Connection) -> Result<Vec<ProjectHealth>> {
    // Get unique project paths from dependencies
    let mut stmt = conn.prepare("SELECT DISTINCT project_path FROM project_dependencies")?;

    let paths: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in project_health: {e}");
                None
            }
        })
        .collect();

    let mut results = Vec::new();
    for path in paths {
        match compute_project_health(conn, &path) {
            Ok(health) => results.push(health),
            Err(e) => {
                warn!(target: "4da::health", path = %path, error = %e, "Failed to compute health");
            }
        }
    }

    results.sort_by(|a, b| {
        a.overall_score
            .partial_cmp(&b.overall_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(results)
}

/// Compute health for a single project
pub fn compute_project_health(
    conn: &rusqlite::Connection,
    project_path: &str,
) -> Result<ProjectHealth> {
    let deps = crate::temporal::get_project_dependencies(conn, project_path)?;
    let project_name = std::path::Path::new(project_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let freshness = compute_freshness(&deps, conn)?;
    let security = compute_security(&deps, conn)?;
    let momentum = compute_momentum(&deps, conn)?;
    let community = compute_community(&deps, conn)?;
    let alerts = generate_alerts(&deps, &freshness, &security);

    let overall = (freshness.score + security.score + momentum.score + community.score) / 4.0;

    Ok(ProjectHealth {
        project_path: project_path.to_string(),
        project_name,
        overall_score: overall,
        freshness,
        security,
        momentum,
        community,
        alerts,
        last_checked: chrono::Utc::now().to_rfc3339(),
        dependency_count: deps.len(),
    })
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_project_health(project_path: Option<String>) -> Result<Vec<ProjectHealth>> {
    crate::settings::require_signal_feature("get_project_health")?;
    let conn = crate::open_db_connection()?;
    if let Some(path) = project_path {
        let health = compute_project_health(&conn, &path)?;
        Ok(vec![health])
    } else {
        compute_all_project_health(&conn)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temporal::upsert_dependency;
    use rusqlite::Connection;

    /// Create an in-memory database with the tables needed by project_health.
    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_dependencies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                manifest_type TEXT NOT NULL,
                package_name TEXT NOT NULL,
                version TEXT,
                is_dev INTEGER DEFAULT 0,
                is_direct INTEGER DEFAULT 1,
                language TEXT NOT NULL DEFAULT 'unknown',
                last_scanned TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(project_path, package_name)
            );
            CREATE INDEX IF NOT EXISTS idx_deps_package ON project_dependencies(package_name);
            CREATE INDEX IF NOT EXISTS idx_deps_project ON project_dependencies(project_path);

            CREATE TABLE IF NOT EXISTS source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                url TEXT,
                title TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                content_hash TEXT NOT NULL DEFAULT '',
                embedding BLOB NOT NULL DEFAULT X'',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_seen TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(source_type, source_id)
            );
            CREATE INDEX IF NOT EXISTS idx_source_type ON source_items(source_type);

            CREATE TABLE IF NOT EXISTS feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER NOT NULL,
                relevant INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (source_item_id) REFERENCES source_items(id)
            );
            CREATE INDEX IF NOT EXISTS idx_feedback_item ON feedback(source_item_id);",
        )
        .expect("create tables");
        conn
    }

    // ---- compute_project_health (integration) ----

    #[test]
    fn project_health_with_deps() {
        let conn = setup_test_db();
        upsert_dependency(
            &conn,
            "/home/user/myapp",
            "cargo.toml",
            "serde",
            Some("1.0"),
            false,
            true,
            "rust",
        )
        .unwrap();
        upsert_dependency(
            &conn,
            "/home/user/myapp",
            "cargo.toml",
            "tokio",
            Some("1.35"),
            false,
            true,
            "rust",
        )
        .unwrap();

        let health = compute_project_health(&conn, "/home/user/myapp").unwrap();
        assert_eq!(health.project_name, "myapp");
        assert_eq!(health.project_path, "/home/user/myapp");
        assert_eq!(health.dependency_count, 2);
        // Overall is average of 4 dimensions, all should be > 0
        assert!(health.overall_score > 0.0);
        assert!(health.overall_score <= 1.0);
        // Freshness should be good (both deps have versions)
        assert!(health.freshness.score >= 0.8);
    }

    // ---- compute_all_project_health ----

    #[test]
    fn all_project_health_returns_sorted_by_score() {
        let conn = setup_test_db();
        // Two projects
        upsert_dependency(
            &conn,
            "/proj/alpha",
            "cargo.toml",
            "serde",
            Some("1.0"),
            false,
            true,
            "rust",
        )
        .unwrap();
        upsert_dependency(
            &conn,
            "/proj/beta",
            "cargo.toml",
            "tokio",
            None,
            false,
            true,
            "rust",
        )
        .unwrap();

        let results = compute_all_project_health(&conn).unwrap();
        assert_eq!(results.len(), 2);
        // Should be sorted by overall_score ascending
        assert!(results[0].overall_score <= results[1].overall_score);
    }

    // ---- Serialization ----

    #[test]
    fn health_dimension_serializes_to_json() {
        let dim = HealthDimension {
            score: 0.85,
            label: "Good".to_string(),
            details: "All clear".to_string(),
        };
        let json = serde_json::to_value(&dim).unwrap();
        let score = json["score"].as_f64().unwrap();
        assert!((score - 0.85).abs() < 1e-5, "Expected ~0.85, got {}", score);
        assert_eq!(json["label"], "Good");
        assert_eq!(json["details"], "All clear");
    }

    #[test]
    fn health_alert_serializes_with_optional_dependency() {
        let alert_with = HealthAlert {
            severity: "critical".to_string(),
            message: "CVE detected".to_string(),
            dependency: Some("openssl".to_string()),
        };
        let alert_without = HealthAlert {
            severity: "low".to_string(),
            message: "Minor issue".to_string(),
            dependency: None,
        };
        let json_with = serde_json::to_value(&alert_with).unwrap();
        let json_without = serde_json::to_value(&alert_without).unwrap();

        assert_eq!(json_with["dependency"], "openssl");
        assert!(json_without["dependency"].is_null());
    }
}
