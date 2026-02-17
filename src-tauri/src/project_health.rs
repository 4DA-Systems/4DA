//! Project Health Radar for 4DA
//!
//! Per-project health dashboard combining dependency freshness,
//! security exposure, ecosystem momentum, and community signals.

use rusqlite::params;
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
pub fn compute_all_project_health(
    conn: &rusqlite::Connection,
) -> Result<Vec<ProjectHealth>, String> {
    // Get unique project paths from dependencies
    let mut stmt = conn
        .prepare("SELECT DISTINCT project_path FROM project_dependencies")
        .map_err(|e| e.to_string())?;

    let paths: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut results = Vec::new();
    for path in paths {
        match compute_project_health(conn, &path) {
            Ok(health) => results.push(health),
            Err(e) => {
                warn!(target: "4da::health", path = %path, error = %e, "Failed to compute health")
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
) -> Result<ProjectHealth, String> {
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

fn compute_freshness(
    deps: &[crate::temporal::ProjectDependency],
    _conn: &rusqlite::Connection,
) -> Result<HealthDimension, String> {
    if deps.is_empty() {
        return Ok(HealthDimension {
            score: 1.0,
            label: "No dependencies".to_string(),
            details: "No dependencies tracked".to_string(),
        });
    }

    // Score based on how recently deps were scanned and if versions are present
    let with_version = deps.iter().filter(|d| d.version.is_some()).count();
    let version_ratio = with_version as f32 / deps.len() as f32;

    let score = version_ratio.clamp(0.3, 1.0);
    let label = if score >= 0.8 {
        "Good"
    } else if score >= 0.5 {
        "Fair"
    } else {
        "Needs attention"
    };

    Ok(HealthDimension {
        score,
        label: label.to_string(),
        details: format!(
            "{}/{} dependencies have version info",
            with_version,
            deps.len()
        ),
    })
}

fn compute_security(
    deps: &[crate::temporal::ProjectDependency],
    conn: &rusqlite::Connection,
) -> Result<HealthDimension, String> {
    // Check source items for security mentions related to our deps
    let mut security_hits = 0;

    for dep in deps.iter().take(20) {
        let pattern = format!("%{}%", dep.package_name);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items
                 WHERE (title LIKE ?1 OR content LIKE ?1)
                   AND (title LIKE '%cve%' OR title LIKE '%vulnerability%' OR title LIKE '%security%'
                        OR content LIKE '%cve%' OR content LIKE '%vulnerability%')
                   AND created_at >= datetime('now', '-30 days')",
                params![pattern],
                |row| row.get(0),
            )
            .unwrap_or(0);
        security_hits += count;
    }

    let score = if security_hits == 0 {
        0.95
    } else if security_hits <= 2 {
        0.6
    } else {
        0.3
    };

    let label = if score >= 0.8 {
        "Clean"
    } else if score >= 0.5 {
        "Monitor"
    } else {
        "Action needed"
    };

    Ok(HealthDimension {
        score,
        label: label.to_string(),
        details: format!(
            "{} security-related items found for your dependencies",
            security_hits
        ),
    })
}

fn compute_momentum(
    deps: &[crate::temporal::ProjectDependency],
    conn: &rusqlite::Connection,
) -> Result<HealthDimension, String> {
    // Check how many source items mention key dependencies (activity = momentum)
    let mut total_mentions = 0i64;

    for dep in deps.iter().filter(|d| !d.is_dev).take(15) {
        let pattern = format!("%{}%", dep.package_name);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items
                 WHERE (title LIKE ?1)
                   AND created_at >= datetime('now', '-14 days')",
                params![pattern],
                |row| row.get(0),
            )
            .unwrap_or(0);
        total_mentions += count;
    }

    let score = if total_mentions >= 10 {
        0.9
    } else if total_mentions >= 3 {
        0.7
    } else if total_mentions >= 1 {
        0.5
    } else {
        0.3
    };

    Ok(HealthDimension {
        score,
        label: if score >= 0.7 {
            "Active"
        } else {
            "Low activity"
        }
        .to_string(),
        details: format!(
            "{} mentions of your dependencies in recent sources",
            total_mentions
        ),
    })
}

fn compute_community(
    deps: &[crate::temporal::ProjectDependency],
    conn: &rusqlite::Connection,
) -> Result<HealthDimension, String> {
    // Simple proxy: count of positive-sentiment source items mentioning deps
    let mut positive_mentions = 0i64;

    for dep in deps.iter().filter(|d| !d.is_dev).take(10) {
        let pattern = format!("%{}%", dep.package_name);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items si
                 JOIN feedback f ON f.source_item_id = si.id
                 WHERE si.title LIKE ?1 AND f.relevant = 1
                   AND f.created_at >= datetime('now', '-30 days')",
                params![pattern],
                |row| row.get(0),
            )
            .unwrap_or(0);
        positive_mentions += count;
    }

    let score = if positive_mentions >= 5 {
        0.9
    } else if positive_mentions >= 2 {
        0.7
    } else {
        0.5
    };

    Ok(HealthDimension {
        score,
        label: if score >= 0.7 { "Positive" } else { "Neutral" }.to_string(),
        details: format!(
            "{} positive community signals about your tech",
            positive_mentions
        ),
    })
}

fn generate_alerts(
    deps: &[crate::temporal::ProjectDependency],
    freshness: &HealthDimension,
    security: &HealthDimension,
) -> Vec<HealthAlert> {
    let mut alerts = Vec::new();

    if security.score < 0.5 {
        alerts.push(HealthAlert {
            severity: "critical".to_string(),
            message: "Security issues detected in your dependencies".to_string(),
            dependency: None,
        });
    }

    if freshness.score < 0.5 {
        alerts.push(HealthAlert {
            severity: "medium".to_string(),
            message: "Many dependencies lack version information".to_string(),
            dependency: None,
        });
    }

    // Check for deps without versions
    for dep in deps.iter().filter(|d| d.version.is_none()).take(3) {
        alerts.push(HealthAlert {
            severity: "low".to_string(),
            message: format!("No version tracked for {}", dep.package_name),
            dependency: Some(dep.package_name.clone()),
        });
    }

    alerts
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_project_health(project_path: Option<String>) -> Result<Vec<ProjectHealth>, String> {
    crate::settings::require_pro_feature("get_project_health")?;
    let conn = crate::open_db_connection()?;
    if let Some(path) = project_path {
        let health = compute_project_health(&conn, &path)?;
        Ok(vec![health])
    } else {
        compute_all_project_health(&conn)
    }
}
