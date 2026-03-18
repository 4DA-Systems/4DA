//! Dependency Intelligence — Tauri commands for the Dependency Dashboard.
//!
//! Provides overview, per-project detail, alert management, and cross-project
//! insights by querying the user_dependencies and dependency_alerts tables.

use crate::error::{Result, ResultExt};

/// Get a high-level overview of all dependencies across projects.
///
/// Returns ecosystem breakdown, project list, total counts, and summary stats.
#[tauri::command]
pub async fn get_dependency_overview() -> Result<serde_json::Value> {
    let db = crate::get_database()?;

    let all_deps = db
        .get_all_user_dependencies()
        .context("Failed to get dependencies")?;
    let active_alerts = db.get_active_alerts().context("Failed to get alerts")?;
    let cross_project = db
        .get_cross_project_packages()
        .context("Failed to get cross-project packages")?;

    // Aggregate by ecosystem
    let mut ecosystems: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut projects: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut dev_count = 0u32;
    let mut direct_count = 0u32;

    for dep in &all_deps {
        *ecosystems.entry(dep.ecosystem.clone()).or_insert(0) += 1;
        projects.insert(dep.project_path.clone());
        if dep.is_dev {
            dev_count += 1;
        }
        if dep.is_direct {
            direct_count += 1;
        }
    }

    // Build ecosystem breakdown
    let ecosystem_breakdown: Vec<serde_json::Value> = ecosystems
        .iter()
        .map(|(eco, count)| {
            serde_json::json!({
                "ecosystem": eco,
                "count": count,
            })
        })
        .collect();

    // Build project list with dep counts
    let mut project_list: Vec<serde_json::Value> = Vec::new();
    for project_path in &projects {
        let project_deps: Vec<_> = all_deps
            .iter()
            .filter(|d| &d.project_path == project_path)
            .collect();
        let alert_count = active_alerts
            .iter()
            .filter(|a| {
                project_deps
                    .iter()
                    .any(|d| d.package_name == a.package_name && d.ecosystem == a.ecosystem)
            })
            .count();
        // Extract project name from path (last segment)
        let name = project_path
            .rsplit(['/', '\\'])
            .find(|s| !s.is_empty())
            .unwrap_or(project_path);
        project_list.push(serde_json::json!({
            "name": name,
            "path": project_path,
            "dependency_count": project_deps.len(),
            "alert_count": alert_count,
        }));
    }
    project_list.sort_by(|a, b| {
        let ca = a["dependency_count"].as_u64().unwrap_or(0);
        let cb = b["dependency_count"].as_u64().unwrap_or(0);
        cb.cmp(&ca)
    });

    // Alert severity summary
    let critical = active_alerts
        .iter()
        .filter(|a| a.severity == "critical")
        .count();
    let high = active_alerts
        .iter()
        .filter(|a| a.severity == "high")
        .count();
    let medium = active_alerts
        .iter()
        .filter(|a| a.severity == "medium")
        .count();
    let low = active_alerts.iter().filter(|a| a.severity == "low").count();

    Ok(serde_json::json!({
        "total_dependencies": all_deps.len(),
        "total_projects": projects.len(),
        "direct_dependencies": direct_count,
        "dev_dependencies": dev_count,
        "ecosystems": ecosystem_breakdown,
        "projects": project_list,
        "alerts": {
            "total": active_alerts.len(),
            "critical": critical,
            "high": high,
            "medium": medium,
            "low": low,
        },
        "cross_project_packages": cross_project.len(),
        "cross_project_top": cross_project.iter().take(10).map(|cp| {
            serde_json::json!({
                "package_name": cp.package_name,
                "ecosystem": cp.ecosystem,
                "project_count": cp.project_count,
            })
        }).collect::<Vec<_>>(),
    }))
}

/// Get dependencies for a specific project.
#[tauri::command]
pub async fn get_project_deps(project_path: String) -> Result<serde_json::Value> {
    let db = crate::get_database()?;

    let deps = db
        .get_project_dependencies(&project_path)
        .context("Failed to get project dependencies")?;
    let active_alerts = db.get_active_alerts().context("Failed to get alerts")?;

    // Enrich deps with alert info
    let enriched: Vec<serde_json::Value> = deps
        .iter()
        .map(|dep| {
            let dep_alerts: Vec<&crate::db::DependencyAlert> = active_alerts
                .iter()
                .filter(|a| a.package_name == dep.package_name && a.ecosystem == dep.ecosystem)
                .collect();
            serde_json::json!({
                "name": dep.package_name,
                "version": dep.version,
                "ecosystem": dep.ecosystem,
                "is_dev": dep.is_dev,
                "is_direct": dep.is_direct,
                "detected_at": dep.detected_at,
                "last_seen_at": dep.last_seen_at,
                "alerts": dep_alerts.iter().map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "severity": a.severity,
                        "title": a.title,
                        "alert_type": a.alert_type,
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect();

    // Extract project name from path
    let name = project_path
        .rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(&project_path);

    Ok(serde_json::json!({
        "project_name": name,
        "project_path": project_path,
        "dependencies": enriched,
        "total": deps.len(),
    }))
}

/// Get all active dependency alerts.
#[tauri::command]
pub async fn get_dependency_alerts() -> Result<serde_json::Value> {
    let db = crate::get_database()?;
    let alerts = db
        .get_active_alerts()
        .context("Failed to get dependency alerts")?;

    let serialized: Vec<serde_json::Value> = alerts
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "package_name": a.package_name,
                "ecosystem": a.ecosystem,
                "alert_type": a.alert_type,
                "severity": a.severity,
                "title": a.title,
                "description": a.description,
                "affected_versions": a.affected_versions,
                "source_url": a.source_url,
                "detected_at": a.detected_at,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "alerts": serialized,
        "total": alerts.len(),
    }))
}

/// Resolve (dismiss) a dependency alert.
#[tauri::command]
pub async fn resolve_dependency_alert(alert_id: i64) -> Result<()> {
    let db = crate::get_database()?;
    db.resolve_alert(alert_id)
        .context("Failed to resolve dependency alert")?;
    Ok(())
}
