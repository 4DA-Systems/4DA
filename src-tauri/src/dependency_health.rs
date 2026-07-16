// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Dependency Health Monitor — classifies dependency health from local DB data
//! and creates proactive decision windows for stale or vulnerable packages.
//!
//! Uses ONLY local data (user_dependencies, dependency_alerts, source_items).
//! No HTTP requests to crates.io, npm, or any external service.

use std::collections::HashSet;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::db::Database;
use crate::decision_advantage::get_open_windows;
use crate::error::Result;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyHealth {
    pub package_name: String,
    pub ecosystem: String,
    pub installed_version: Option<String>,
    pub latest_known_version: Option<String>,
    pub days_since_last_release: Option<i64>,
    pub health_status: HealthStatus,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// Recent release, no known issues
    Healthy,
    /// 6+ months without appearing in source_items
    Stale,
    /// Major version available (reserved for future use)
    MajorBehind,
    /// Known CVE or high-severity alert in dependency_alerts
    SecurityAlert,
    /// Couldn't determine status
    Unknown,
}

// ============================================================================
// Health Check
// ============================================================================

/// Check the health of all direct, non-dev dependencies using local DB data only.
///
/// Classification rules (applied in priority order):
/// 1. If `dependency_alerts` has an unresolved alert with severity "critical" or "high"
///    for the package → `SecurityAlert`
/// 2. If the package hasn't appeared in `source_items` titles for 180+ days → `Stale`
/// 3. Otherwise → `Healthy`
pub fn check_dependency_health(conn: &Connection) -> Result<Vec<DependencyHealth>> {
    let now = chrono::Utc::now().to_rfc3339();

    // Load direct, non-dev dependencies (deduplicated by package_name + ecosystem)
    let mut stmt = conn.prepare(
        "SELECT DISTINCT package_name, ecosystem, version
         FROM user_dependencies
         WHERE is_direct = 1 AND is_dev = 0
         ORDER BY package_name",
    )?;

    let deps: Vec<(String, String, Option<String>)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    if deps.is_empty() {
        return Ok(vec![]);
    }

    // Pre-load all unresolved high/critical alerts into a set for fast lookup
    let alert_packages = load_security_alert_packages(conn);

    let mut results = Vec::with_capacity(deps.len());

    for (package_name, ecosystem, version) in &deps {
        let status = classify_health(conn, package_name, ecosystem, &alert_packages);
        let days_since = compute_days_since_last_mention(conn, package_name);

        results.push(DependencyHealth {
            package_name: package_name.clone(),
            ecosystem: ecosystem.clone(),
            installed_version: version.clone(),
            latest_known_version: None, // No HTTP — local data only
            days_since_last_release: days_since,
            health_status: status,
            checked_at: now.clone(),
        });
    }

    info!(
        target: "4da::dependency_health",
        total = results.len(),
        healthy = results.iter().filter(|r| r.health_status == HealthStatus::Healthy).count(),
        stale = results.iter().filter(|r| r.health_status == HealthStatus::Stale).count(),
        security = results.iter().filter(|r| r.health_status == HealthStatus::SecurityAlert).count(),
        "Dependency health check complete"
    );

    Ok(results)
}

/// Load package names that have unresolved critical/high alerts.
fn load_security_alert_packages(conn: &Connection) -> HashSet<(String, String)> {
    // Compare severity case-insensitively: the CVE write-path stores UPPERCASE
    // ("CRITICAL"/"HIGH") while older/local-audit rows are lowercase. A bare
    // `severity IN ('critical','high')` matched ZERO uppercase CVE alerts, so no
    // SecurityAlert health status (and no security_patch window) ever fired for
    // real CVEs — the same case bug fixed in get_dependency_overview.
    let mut stmt = match conn.prepare(
        "SELECT DISTINCT LOWER(package_name), LOWER(ecosystem)
         FROM dependency_alerts
         WHERE resolved_at IS NULL AND LOWER(severity) IN ('critical', 'high')",
    ) {
        Ok(s) => s,
        Err(_) => return HashSet::new(),
    };

    stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })
    .ok()
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// Classify a single dependency's health status.
fn classify_health(
    conn: &Connection,
    package_name: &str,
    ecosystem: &str,
    alert_packages: &HashSet<(String, String)>,
) -> HealthStatus {
    let key = (package_name.to_lowercase(), ecosystem.to_lowercase());

    // Priority 1: Security alerts
    if alert_packages.contains(&key) {
        return HealthStatus::SecurityAlert;
    }

    // Priority 2: Staleness — check if package hasn't appeared in source_items for 180+ days
    let last_mention = conn
        .query_row(
            "SELECT MAX(created_at) FROM source_items
             WHERE LOWER(title) LIKE ?1
             AND created_at >= datetime('now', '-365 days')",
            params![format!("%{}%", package_name.to_lowercase())],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten();

    match last_mention {
        Some(ref ts) => {
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S") {
                let days_ago = (chrono::Utc::now().naive_utc() - dt).num_days();
                if days_ago >= 180 {
                    return HealthStatus::Stale;
                }
            }
            HealthStatus::Healthy
        }
        // No mentions at all — could be stale or just not in the news; mark as Unknown
        None => HealthStatus::Unknown,
    }
}

/// Compute days since the package was last mentioned in source_items.
fn compute_days_since_last_mention(conn: &Connection, package_name: &str) -> Option<i64> {
    let last_mention: Option<String> = conn
        .query_row(
            "SELECT MAX(created_at) FROM source_items WHERE LOWER(title) LIKE ?1",
            params![format!("%{}%", package_name.to_lowercase())],
            |row| row.get(0),
        )
        .ok()?;

    let ts = last_mention?;
    let dt = chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%d %H:%M:%S").ok()?;
    Some((chrono::Utc::now().naive_utc() - dt).num_days())
}

// ============================================================================
// Proactive Decision Windows
// ============================================================================

/// Create proactive decision windows from dependency health assessments.
///
/// - Stale deps → "knowledge" window: "Review: is {dep} still maintained?"
/// - SecurityAlert deps → "security_patch" window: "Security: {dep} has known vulnerability"
///
/// Deduplicates against existing open windows to avoid flooding.
pub fn create_proactive_windows(conn: &Connection, health: &[DependencyHealth]) -> Result<()> {
    let existing_windows = get_open_windows(conn);
    let existing_deps: HashSet<(String, Option<String>)> = existing_windows
        .iter()
        .map(|w| (w.window_type.clone(), w.dependency.clone()))
        .collect();

    let mut created = 0u32;

    for dep in health {
        match dep.health_status {
            HealthStatus::Stale => {
                let key = ("knowledge".to_string(), Some(dep.package_name.clone()));
                if existing_deps.contains(&key) {
                    continue;
                }
                insert_window(
                    conn,
                    "knowledge",
                    &format!("Review: is {} still maintained?", dep.package_name),
                    &dep.package_name,
                    0.45,
                    0.50,
                    None, // No expiry — knowledge windows persist
                )?;
                created += 1;
            }
            HealthStatus::SecurityAlert => {
                let key = ("security_patch".to_string(), Some(dep.package_name.clone()));
                if existing_deps.contains(&key) {
                    continue;
                }
                insert_window(
                    conn,
                    "security_patch",
                    &format!("Security: {} has known vulnerability", dep.package_name),
                    &dep.package_name,
                    0.85,
                    0.90,
                    Some("+7 days"),
                )?;
                created += 1;
            }
            _ => {}
        }
    }

    if created > 0 {
        info!(
            target: "4da::dependency_health",
            created,
            "Proactive decision windows created from dependency health"
        );
    }

    Ok(())
}

/// Insert a single decision window into the database.
fn insert_window(
    conn: &Connection,
    window_type: &str,
    title: &str,
    dependency: &str,
    urgency: f32,
    relevance: f32,
    expires_offset: Option<&str>,
) -> Result<()> {
    let streets_engine = match window_type {
        "security_patch" => Some("Automation"),
        "knowledge" => Some("Education"),
        _ => None,
    };

    conn.execute(
        "INSERT INTO decision_windows (window_type, title, description, urgency, relevance, dependency, status, expires_at, streets_engine)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', CASE WHEN ?7 IS NOT NULL THEN datetime('now', ?7) ELSE NULL END, ?8)",
        params![
            window_type,
            title,
            title, // description = title for these auto-generated windows
            urgency,
            relevance,
            dependency,
            expires_offset,
            streets_engine,
        ],
    )?;

    Ok(())
}

// ============================================================================
// Background Job Entry Point
// ============================================================================

/// Run a full dependency health check as a background job.
///
/// Opens its own DB connection, checks all direct non-dev dependencies,
/// and creates proactive decision windows for any actionable findings
/// (stale, security alert, or major-version-behind).
///
/// Called by the monitoring scheduler on a 6-hour interval.
/// Re-validate active `dependency_alerts` against the CURRENT installed versions
/// and auto-resolve any whose install has moved out of the advisory's affected
/// range — i.e. the package was upgraded to a patched release. Without this, a
/// fixed vulnerability lingers as an unresolved CRITICAL/HIGH forever and
/// inflates the dependency-dashboard counts (a stale alert reads as a live risk).
///
/// Conservative by construction: an alert is resolved ONLY when the package is
/// installed AND *every* installed instance is definitively outside the affected
/// range (`version_is_affected` returns `false`). Any of the following keeps the
/// alert untouched:
/// - the affected range is missing/empty or unparseable,
/// - any installed instance has an unknown or unparseable version,
/// - any installed instance is still within the affected range,
/// - the package is no longer present in the auditable dependency set (a scan
///   gap must never silently clear a real advisory).
///
/// Returns the number of alerts auto-resolved.
pub fn resolve_patched_dependency_alerts(db: &Database) -> Result<usize> {
    use crate::sources::cve_matching::{normalize_ecosystem, version_is_affected};
    use std::collections::HashMap;

    let alerts = db.get_active_alerts()?;
    if alerts.is_empty() {
        return Ok(0);
    }
    let deps = db.get_auditable_user_dependencies()?;

    // Path normalizer matching the storage/OSV-matcher convention so the
    // instance rows below join to the auditable dep set consistently.
    let norm_path = |p: &str| {
        p.replace('\\', "/")
            .to_lowercase()
            .trim_end_matches('/')
            .to_string()
    };

    // (normalized ecosystem, lowercase package) -> all installed versions.
    let mut installed: HashMap<(String, String), Vec<Option<String>>> = HashMap::new();
    // The exact (project, package, ecosystem) triples the auditable set blessed
    // (tier 1/2/3 inclusion already applied). Instance rows are folded in only
    // for these triples, so a tier-3 "not my stack" project cannot pool a
    // version into my alerts.
    let mut auditable: std::collections::HashSet<(String, String, String)> =
        std::collections::HashSet::new();
    for dep in &deps {
        let key = (
            normalize_ecosystem(&dep.ecosystem).to_string(),
            dep.package_name.to_lowercase(),
        );
        installed.entry(key).or_default().push(dep.version.clone());
        auditable.insert((
            norm_path(&dep.project_path),
            dep.package_name.to_lowercase(),
            normalize_ecosystem(&dep.ecosystem).to_string(),
        ));
    }

    // Deepen with the multi-version inventory (Phase 92). The collapsed
    // user_dependencies row keeps one version per (project, package), so a
    // vulnerable duplicate hidden by the collapse would let this auto-resolver
    // clear a still-live alert — a false negative (the worst failure for a
    // security surface). Folding in every installed instance version can only
    // make `all_unaffected` HARDER to satisfy, so this strictly KEEPS more
    // alerts and never resolves more (accuracy-first). Restricted to auditable
    // (project, package, ecosystem) triples — never introduces a new package.
    match db.get_all_dependency_instances() {
        Ok(rows) => {
            for r in rows {
                let eco = normalize_ecosystem(&r.ecosystem).to_string();
                let pkg = r.package_name.to_lowercase();
                if !auditable.contains(&(norm_path(&r.project_path), pkg.clone(), eco.clone())) {
                    continue;
                }
                if let Some(versions) = installed.get_mut(&(eco, pkg)) {
                    versions.push(Some(r.version));
                }
            }
        }
        Err(e) => warn!(
            target: "4da::health",
            error = %e,
            "dependency_instances read failed — alert auto-resolve uses collapsed versions only"
        ),
    }

    let mut resolved = 0usize;
    for alert in &alerts {
        // Need a concrete affected range to test against; without one we cannot
        // prove the install is safe, so keep the alert.
        let range = match alert.affected_versions.as_deref() {
            Some(r) if !r.trim().is_empty() => r,
            _ => continue,
        };
        let key = (
            normalize_ecosystem(&alert.ecosystem).to_string(),
            alert.package_name.to_lowercase(),
        );
        let Some(versions) = installed.get(&key) else {
            // Package not in the current auditable set — leave it (scan-gap safe).
            continue;
        };

        // Resolve only when EVERY installed instance is definitively unaffected.
        // `version_is_affected` is conservative (returns true on unknown/unparseable
        // version or range), so this never resolves on uncertainty.
        let all_unaffected = versions
            .iter()
            .all(|v| !version_is_affected(v.as_deref(), range));
        if !all_unaffected {
            continue;
        }

        match db.resolve_alert(alert.id) {
            Ok(()) => {
                resolved += 1;
                info!(
                    target: "4da::health",
                    package = alert.package_name.as_str(),
                    ecosystem = alert.ecosystem.as_str(),
                    alert_id = alert.id,
                    severity = alert.severity.as_str(),
                    "Auto-resolved dependency alert — installed version no longer in affected range"
                );
            }
            Err(e) => warn!(
                target: "4da::health",
                alert_id = alert.id,
                error = %e,
                "Failed to auto-resolve patched dependency alert"
            ),
        }
    }

    if resolved > 0 {
        info!(target: "4da::health", resolved, "Auto-resolved patched dependency alerts");
    }
    Ok(resolved)
}

pub fn run_dependency_health_check() -> Result<Vec<DependencyHealth>> {
    let conn = crate::open_db_connection()?;
    let health = check_dependency_health(&conn)?;

    // Retire any alerts whose package has since been patched out of range, so the
    // health classification and dashboard counts reflect only live risks.
    if let Ok(db) = crate::get_database() {
        if let Err(e) = resolve_patched_dependency_alerts(&db) {
            warn!(target: "4da::health", error = %e, "Patched-alert resolution failed");
        }
    }
    let actionable: Vec<_> = health
        .iter()
        .filter(|h| {
            matches!(
                h.health_status,
                HealthStatus::Stale | HealthStatus::SecurityAlert | HealthStatus::MajorBehind
            )
        })
        .collect();
    if !actionable.is_empty() {
        create_proactive_windows(&conn, &health)?;
        info!(
            target: "4da::health",
            alerts = actionable.len(),
            "Dependency health: created proactive windows"
        );
    }
    Ok(health)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "dependency_health_tests.rs"]
mod tests;
