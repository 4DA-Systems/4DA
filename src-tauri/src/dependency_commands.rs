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

/// Get a license compliance overview for all tracked dependencies.
///
/// Returns counts by compatibility status and a list of packages with
/// caution/warning status that need review.
#[tauri::command]
pub async fn get_license_overview() -> Result<serde_json::Value> {
    let db = crate::get_database()?;
    let all_deps = db
        .get_all_user_dependencies()
        .context("Failed to get dependencies")?;

    let mut compatible = 0u32;
    let mut caution = 0u32;
    let mut warning = 0u32;
    let mut unknown = 0u32;
    let mut issues = Vec::new();

    for dep in &all_deps {
        if let Some(ref license) = dep.license {
            let (status, reason) = check_license_compatibility(license);
            match status {
                "compatible" => compatible += 1,
                "caution" => {
                    caution += 1;
                    issues.push(serde_json::json!({
                        "package": dep.package_name,
                        "ecosystem": dep.ecosystem,
                        "license": license,
                        "status": status,
                        "reason": reason,
                    }));
                }
                "warning" => {
                    warning += 1;
                    issues.push(serde_json::json!({
                        "package": dep.package_name,
                        "ecosystem": dep.ecosystem,
                        "license": license,
                        "status": status,
                        "reason": reason,
                    }));
                }
                _ => unknown += 1,
            }
        } else {
            unknown += 1;
        }
    }

    Ok(serde_json::json!({
        "total": all_deps.len(),
        "compatible": compatible,
        "caution": caution,
        "warning": warning,
        "unknown": unknown,
        "issues": issues,
    }))
}

/// Check npm registry and crates.io for available upgrades.
/// Returns packages where installed version < latest version.
#[tauri::command]
pub async fn check_dependency_upgrades() -> Result<serde_json::Value> {
    let db = crate::get_database()?;
    let all_deps = db
        .get_all_user_dependencies()
        .context("Failed to get dependencies")?;

    let mut upgrades = Vec::new();
    let client = crate::sources::shared_client();

    // Only check direct dependencies with known versions (skip dev deps and transitive)
    let checkable: Vec<_> = all_deps
        .iter()
        .filter(|d| d.is_direct && d.version.is_some() && !d.is_dev)
        .collect();

    // Cap at 50 to avoid rate limits
    for dep in checkable.iter().take(50) {
        let version = dep.version.as_deref().unwrap_or("0.0.0");
        let latest = match dep.ecosystem.as_str() {
            "javascript" | "npm" | "typescript" => {
                fetch_npm_latest(&client, &dep.package_name).await
            }
            "rust" | "crates.io" => fetch_crates_io_latest(&client, &dep.package_name).await,
            _ => None,
        };

        if let Some(latest_version) = latest {
            if is_newer(&latest_version, version) {
                let is_major = is_major_bump(&latest_version, version);
                upgrades.push(serde_json::json!({
                    "package": dep.package_name,
                    "ecosystem": dep.ecosystem,
                    "current": version,
                    "latest": latest_version,
                    "is_major_upgrade": is_major,
                    "project": dep.project_path,
                }));
            }
        }
    }

    Ok(serde_json::json!({
        "checked": checkable.len().min(50),
        "upgrades_available": upgrades.len(),
        "upgrades": upgrades,
    }))
}

// ============================================================================
// Registry query helpers
// ============================================================================

/// Query npm registry for latest version of a package.
async fn fetch_npm_latest(client: &reqwest::Client, package: &str) -> Option<String> {
    // Handle scoped packages: @scope/pkg -> @scope%2Fpkg
    let encoded = package.replace('/', "%2F");
    let url = format!("https://registry.npmjs.org/{}/latest", encoded);

    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Query crates.io for latest stable version of a crate.
async fn fetch_crates_io_latest(client: &reqwest::Client, crate_name: &str) -> Option<String> {
    let url = format!("https://crates.io/api/v1/crates/{}", crate_name);

    let resp = client
        .get(&url)
        .header("User-Agent", "4DA/1.0 (https://4da.ai)")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("crate")
        .and_then(|c| c.get("max_stable_version"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

// ============================================================================
// Version comparison helpers
// ============================================================================

/// Check if `latest` is newer than `current` using semver.
fn is_newer(latest: &str, current: &str) -> bool {
    let Ok(latest_v) = semver::Version::parse(latest.trim_start_matches('v')) else {
        return false;
    };
    let current_clean = current
        .trim_start_matches('v')
        .trim_start_matches('^')
        .trim_start_matches('~');
    let Ok(current_v) = semver::Version::parse(current_clean) else {
        return false;
    };
    latest_v > current_v
}

/// Check if upgrade crosses a major version boundary.
fn is_major_bump(latest: &str, current: &str) -> bool {
    let Ok(latest_v) = semver::Version::parse(latest.trim_start_matches('v')) else {
        return false;
    };
    let current_clean = current
        .trim_start_matches('v')
        .trim_start_matches('^')
        .trim_start_matches('~');
    let Ok(current_v) = semver::Version::parse(current_clean) else {
        return false;
    };
    latest_v.major > current_v.major
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_basic() {
        assert!(is_newer("2.0.0", "1.0.0"));
        assert!(is_newer("1.1.0", "1.0.0"));
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "2.0.0"));
    }

    #[test]
    fn test_is_newer_with_prefixes() {
        assert!(is_newer("2.0.0", "^1.5.0"));
        assert!(is_newer("2.0.0", "~1.5.0"));
        assert!(is_newer("2.0.0", "v1.5.0"));
        assert!(is_newer("v2.0.0", "1.5.0"));
    }

    #[test]
    fn test_is_newer_invalid_versions() {
        assert!(!is_newer("not-a-version", "1.0.0"));
        assert!(!is_newer("1.0.0", "not-a-version"));
        assert!(!is_newer("", ""));
    }

    #[test]
    fn test_is_major_bump() {
        assert!(is_major_bump("2.0.0", "1.9.9"));
        assert!(is_major_bump("3.0.0", "2.5.0"));
        assert!(!is_major_bump("1.5.0", "1.0.0"));
        assert!(!is_major_bump("1.0.1", "1.0.0"));
    }

    #[test]
    fn test_is_major_bump_with_prefixes() {
        assert!(is_major_bump("2.0.0", "^1.0.0"));
        assert!(is_major_bump("2.0.0", "~1.0.0"));
        assert!(!is_major_bump("1.5.0", "^1.0.0"));
    }

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
