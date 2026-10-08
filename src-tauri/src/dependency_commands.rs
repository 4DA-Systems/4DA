// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

    // Alert severity summary.
    // Compare case-insensitively: the CVE write-path stores UPPERCASE
    // ("CRITICAL"/"HIGH"/...) while the legacy local-audit path and pre-5d6fb063
    // rows stored lowercase. A case-sensitive `== "critical"` silently counted
    // ZERO criticals/highs while real CRITICAL/HIGH CVEs sat in the table —
    // the dashboard read "0 critical / 0 high" over genuine RCE-grade vulns.
    let critical = active_alerts
        .iter()
        .filter(|a| a.severity.eq_ignore_ascii_case("critical"))
        .count();
    let high = active_alerts
        .iter()
        .filter(|a| a.severity.eq_ignore_ascii_case("high"))
        .count();
    let medium = active_alerts
        .iter()
        .filter(|a| a.severity.eq_ignore_ascii_case("medium"))
        .count();
    let low = active_alerts
        .iter()
        .filter(|a| a.severity.eq_ignore_ascii_case("low"))
        .count();

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
    let project_path = crate::ipc_guard::validate_path_input("project_path", &project_path)?;
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
            let license_status = dep.license.as_deref().map(|l| {
                let (status, reason) = check_license_compatibility(l);
                serde_json::json!({ "status": status, "reason": reason })
            });
            serde_json::json!({
                "name": dep.package_name,
                "version": dep.version,
                "ecosystem": dep.ecosystem,
                "is_dev": dep.is_dev,
                "is_direct": dep.is_direct,
                "license": dep.license,
                "license_status": license_status,
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

/// Check if a license is compatible with proprietary use.
/// Returns (status, reason) where status is one of:
/// "compatible", "caution", "warning", "unknown".
fn check_license_compatibility(license: &str) -> (&'static str, &'static str) {
    let lower = license.to_lowercase();
    match lower.as_str() {
        // Permissive — fully compatible
        "mit" | "apache-2.0" | "bsd-2-clause" | "bsd-3-clause" | "isc" | "0bsd" | "unlicense"
        | "cc0-1.0" | "wtfpl" => ("compatible", "Permissive license"),
        // Weak copyleft — usually fine for dependencies
        "mpl-2.0" | "lgpl-2.1" | "lgpl-3.0" | "lgpl-2.1-only" | "lgpl-3.0-only" => {
            ("caution", "Weak copyleft — check linking requirements")
        }
        // Strong copyleft — may require source disclosure
        "gpl-2.0" | "gpl-3.0" | "gpl-2.0-only" | "gpl-3.0-only" | "agpl-3.0" | "agpl-3.0-only" => {
            ("warning", "Strong copyleft — may require source disclosure")
        }
        // Pattern-based fallbacks
        _ if lower.contains("gpl") => ("warning", "GPL-family license detected"),
        _ if lower.contains("proprietary") || lower.contains("commercial") => {
            ("warning", "Proprietary or commercial license")
        }
        _ => ("unknown", "License not recognized — review manually"),
    }
}

// ============================================================================
// Project allowlist ("Your Stack") — user-controlled grounding scope
// ============================================================================

/// List locally-detected projects (that carry dependencies) with whether each
/// currently counts toward the user's stack grounding. Drives the "Your Stack"
/// settings UI. Reads the same `project_dependencies` table the relevance
/// grounding consumes, so the toggle maps 1:1 to what "Affects You" sees.
#[tauri::command]
pub async fn list_projects_with_stack_status() -> Result<serde_json::Value> {
    let conn = crate::open_db_connection().context("Failed to open database")?;
    let mut stmt = conn
        .prepare(
            "SELECT project_path, COUNT(*) AS dep_count
             FROM project_dependencies
             GROUP BY project_path
             ORDER BY dep_count DESC",
        )
        .context("Failed to prepare project query")?;
    let rows: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .context("Failed to query projects")?
        .filter_map(|r| r.ok())
        .collect();

    drop(stmt);
    let membership = crate::project_inclusion::StackMembership::load(&conn);
    Ok(serde_json::json!(stack_status_rows(rows, &membership)))
}

/// One Settings row per project, carrying the SAME verdict grounding uses
/// (`project_inclusion::counts_toward_stack`) and why: `included` is the
/// user's toggle (not excluded), `counts` is whether the project actually
/// grounds "Affects You" — a dormant or scratch project can be included and
/// still not count until the user force-includes it.
fn stack_status_rows(
    rows: Vec<(String, i64)>,
    membership: &crate::project_inclusion::StackMembership,
) -> Vec<serde_json::Value> {
    rows.into_iter()
        // Hard-excluded rows (agent infra / fixture scaffolding, tiers 1+2)
        // are purged at startup and blocked at write time; filter defensively
        // so they can never appear as toggleable "projects" even on a
        // not-yet-purged DB. Tier-3 projects DO stay listed — that toggle is
        // the product mechanism for ambiguous dirs.
        .filter(|(path, _)| !crate::project_inclusion::is_hard_excluded(path))
        .map(|(path, dep_count)| {
            let status = membership.status(&path);
            let name = path
                .rsplit(['/', '\\'])
                .find(|s| !s.is_empty())
                .unwrap_or(&path)
                .to_string();
            serde_json::json!({
                "path": path,
                "name": name,
                "dependency_count": dep_count,
                "included": !status.excluded,
                "counts": status.counts,
                "forced": status.forced,
                "dormant": status.dormant,
                "dormant_days": status.dormant_days,
                "scratch": status.scratch,
            })
        })
        .collect()
}

/// Include or exclude a project from the user's stack grounding. Excluding a
/// project drops its deps from relevance scoring on the next analysis.
///
/// `force` (optional, default false) is "Force include": with `included` it
/// counts a dormant or scratch project anyway; with `!included` it takes that
/// force back without excluding the project. Stored in the same
/// `excluded_project_paths` setting as a `!path` entry — no new storage.
#[tauri::command]
pub async fn set_project_in_stack(path: String, included: bool, force: Option<bool>) -> Result<()> {
    let path = crate::ipc_guard::validate_path_input("path", &path)?;
    let manager = crate::get_settings_manager();
    let mut guard = manager.lock();
    // Idempotent, and canonical-form aware: a `D:\...` entry and a `d:/...`
    // entry can never coexist for one project.
    let entries = crate::project_inclusion::apply_stack_choice(
        guard.get_excluded_project_paths(),
        &path,
        included,
        force.unwrap_or(false),
    );
    guard
        .set_excluded_project_paths(entries)
        .context("Failed to persist stack selection")?;
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_compatibility_permissive() {
        assert_eq!(check_license_compatibility("MIT").0, "compatible");
        assert_eq!(check_license_compatibility("Apache-2.0").0, "compatible");
        assert_eq!(check_license_compatibility("ISC").0, "compatible");
        assert_eq!(check_license_compatibility("BSD-3-Clause").0, "compatible");
        assert_eq!(check_license_compatibility("0BSD").0, "compatible");
        assert_eq!(check_license_compatibility("Unlicense").0, "compatible");
    }

    #[test]
    fn test_license_compatibility_copyleft() {
        assert_eq!(check_license_compatibility("GPL-3.0").0, "warning");
        assert_eq!(check_license_compatibility("AGPL-3.0").0, "warning");
        assert_eq!(check_license_compatibility("MPL-2.0").0, "caution");
        assert_eq!(check_license_compatibility("LGPL-3.0").0, "caution");
    }

    /// The Settings payload carries the same verdict grounding uses, and why.
    #[test]
    fn stack_status_rows_carry_dormant_scratch_and_counts() {
        let conn = rusqlite::Connection::open_in_memory().expect("db");
        conn.execute_batch(&format!(
            "CREATE TABLE detected_projects (path TEXT, name TEXT, last_activity TEXT, scratch INTEGER);
             INSERT INTO detected_projects VALUES
               ('D:\\4DA\\src-tauri', 'a', '{now}', 0),
               ('D:\\4DA\\victauri-gauntlet', 'b', '2026-04-28T19:28:43Z', 1),
               ('D:\\4DA\\cli', 'c', '{now}', 1);",
            now = chrono::Utc::now().to_rfc3339()
        ))
        .expect("seed");
        let membership = crate::project_inclusion::StackMembership::load(&conn);
        let rows = stack_status_rows(
            vec![
                ("d:/4da/src-tauri".to_string(), 75),
                ("d:/4da/victauri-gauntlet".to_string(), 6),
                ("d:/4da/cli".to_string(), 4),
                ("d:/4da/.claude/worktrees/agent-x".to_string(), 9),
            ],
            &membership,
        );
        assert_eq!(rows.len(), 3, "hard-excluded agent infra is never listed");
        let live = &rows[0];
        assert_eq!(live["counts"], true);
        assert_eq!(live["dormant"], false);
        assert_eq!(live["scratch"], false);
        let gauntlet = &rows[1];
        assert_eq!(gauntlet["included"], true, "not excluded by the user");
        assert_eq!(
            gauntlet["counts"], false,
            "but dormant + scratch: not counted"
        );
        assert_eq!(gauntlet["dormant"], true);
        assert_eq!(gauntlet["scratch"], true);
        assert!(gauntlet["dormant_days"].as_i64().is_some_and(|d| d > 90));
        assert_eq!(gauntlet["forced"], false);
        let cli = &rows[2];
        assert_eq!(cli["scratch"], true);
        assert_eq!(cli["dormant"], false);
        assert_eq!(cli["counts"], false);
    }

    #[test]
    fn test_license_compatibility_unknown() {
        assert_eq!(
            check_license_compatibility("some-custom-license").0,
            "unknown"
        );
        // GPL substring detection
        assert_eq!(check_license_compatibility("GPL-2.0-or-later").0, "warning");
    }
}
