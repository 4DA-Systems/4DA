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
use tracing::info;

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
    let mut stmt = match conn.prepare(
        "SELECT DISTINCT LOWER(package_name), LOWER(ecosystem)
         FROM dependency_alerts
         WHERE resolved_at IS NULL AND severity IN ('critical', 'high')",
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
pub fn run_dependency_health_check() -> Result<Vec<DependencyHealth>> {
    let conn = crate::open_db_connection()?;
    let health = check_dependency_health(&conn)?;
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
mod tests {
    use super::*;

    const TEST_SCHEMA: &str = "
        CREATE TABLE source_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_type TEXT DEFAULT 'test',
            source_id TEXT DEFAULT '',
            url TEXT,
            title TEXT DEFAULT '',
            content TEXT DEFAULT '',
            content_hash TEXT DEFAULT '',
            created_at TEXT DEFAULT (datetime('now')),
            last_seen TEXT DEFAULT (datetime('now'))
        );
        CREATE TABLE user_dependencies (
            id INTEGER PRIMARY KEY,
            project_path TEXT NOT NULL,
            package_name TEXT NOT NULL,
            version TEXT,
            ecosystem TEXT NOT NULL,
            is_dev INTEGER DEFAULT 0,
            is_direct INTEGER DEFAULT 1,
            detected_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
            license TEXT,
            UNIQUE(project_path, package_name, ecosystem)
        );
        CREATE TABLE dependency_alerts (
            id INTEGER PRIMARY KEY,
            package_name TEXT NOT NULL,
            ecosystem TEXT NOT NULL,
            alert_type TEXT NOT NULL,
            severity TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT,
            affected_versions TEXT,
            source_url TEXT,
            source_item_id INTEGER,
            detected_at TEXT NOT NULL DEFAULT (datetime('now')),
            resolved_at TEXT
        );
        CREATE TABLE decision_windows (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            window_type TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT DEFAULT '',
            urgency REAL DEFAULT 0.5,
            relevance REAL DEFAULT 0.5,
            source_item_ids TEXT DEFAULT '[]',
            signal_chain_id INTEGER,
            dependency TEXT,
            status TEXT DEFAULT 'open',
            opened_at TEXT DEFAULT (datetime('now')),
            expires_at TEXT,
            acted_at TEXT,
            closed_at TEXT,
            outcome TEXT,
            lead_time_hours REAL,
            streets_engine TEXT
        );
    ";

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(TEST_SCHEMA).unwrap();
        conn
    }

    #[test]
    fn test_healthy_dep_with_recent_mention() {
        let conn = test_conn();
        // Insert a direct, non-dev dependency
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_direct, is_dev)
             VALUES ('/app', 'tokio', '1.35.0', 'rust', 1, 0)",
            [],
        ).unwrap();
        // Insert a recent source item mentioning tokio
        conn.execute(
            "INSERT INTO source_items (title, content, created_at)
             VALUES ('Tokio 1.36 released with new features', 'performance improvements', datetime('now', '-2 days'))",
            [],
        ).unwrap();

        let health = check_dependency_health(&conn).unwrap();
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].package_name, "tokio");
        assert_eq!(health[0].health_status, HealthStatus::Healthy);
    }

    #[test]
    fn test_stale_dep_old_mention() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_direct, is_dev)
             VALUES ('/app', 'oldcrate', '0.1.0', 'rust', 1, 0)",
            [],
        ).unwrap();
        // Only mention is 200 days ago
        conn.execute(
            "INSERT INTO source_items (title, content, created_at)
             VALUES ('oldcrate initial release', 'new crate', datetime('now', '-200 days'))",
            [],
        )
        .unwrap();

        let health = check_dependency_health(&conn).unwrap();
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].health_status, HealthStatus::Stale);
    }

    #[test]
    fn test_security_alert_overrides_stale() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_direct, is_dev)
             VALUES ('/app', 'lodash', '4.17.0', 'javascript', 1, 0)",
            [],
        ).unwrap();
        // Even with old mention...
        conn.execute(
            "INSERT INTO source_items (title, content, created_at)
             VALUES ('lodash old news', 'old', datetime('now', '-200 days'))",
            [],
        )
        .unwrap();
        // ...a critical alert should take priority
        conn.execute(
            "INSERT INTO dependency_alerts (package_name, ecosystem, alert_type, severity, title)
             VALUES ('lodash', 'javascript', 'vulnerability', 'critical', 'Prototype pollution')",
            [],
        )
        .unwrap();

        let health = check_dependency_health(&conn).unwrap();
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].health_status, HealthStatus::SecurityAlert);
    }

    #[test]
    fn test_unknown_when_no_mentions() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_direct, is_dev)
             VALUES ('/app', 'obscure-lib', '1.0.0', 'rust', 1, 0)",
            [],
        ).unwrap();
        // No source items at all

        let health = check_dependency_health(&conn).unwrap();
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].health_status, HealthStatus::Unknown);
    }

    #[test]
    fn test_dev_deps_excluded() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_direct, is_dev)
             VALUES ('/app', 'devtool', '1.0.0', 'rust', 1, 1)",
            [],
        ).unwrap();

        let health = check_dependency_health(&conn).unwrap();
        assert!(health.is_empty(), "dev deps should be excluded");
    }

    #[test]
    fn test_transitive_deps_excluded() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_direct, is_dev)
             VALUES ('/app', 'transitive-lib', '1.0.0', 'rust', 0, 0)",
            [],
        ).unwrap();

        let health = check_dependency_health(&conn).unwrap();
        assert!(health.is_empty(), "transitive deps should be excluded");
    }

    #[test]
    fn test_resolved_alerts_ignored() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_direct, is_dev)
             VALUES ('/app', 'express', '4.18.0', 'javascript', 1, 0)",
            [],
        ).unwrap();
        // Alert exists but is resolved
        conn.execute(
            "INSERT INTO dependency_alerts (package_name, ecosystem, alert_type, severity, title, resolved_at)
             VALUES ('express', 'javascript', 'vulnerability', 'critical', 'Old CVE', datetime('now'))",
            [],
        ).unwrap();
        // Recent mention
        conn.execute(
            "INSERT INTO source_items (title, content, created_at)
             VALUES ('Express 5 beta available', 'new features', datetime('now', '-1 day'))",
            [],
        )
        .unwrap();

        let health = check_dependency_health(&conn).unwrap();
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].health_status, HealthStatus::Healthy);
    }

    #[test]
    fn test_create_proactive_windows_stale() {
        let conn = test_conn();
        let health = vec![DependencyHealth {
            package_name: "stale-crate".to_string(),
            ecosystem: "rust".to_string(),
            installed_version: Some("0.1.0".to_string()),
            latest_known_version: None,
            days_since_last_release: Some(200),
            health_status: HealthStatus::Stale,
            checked_at: "2026-03-27T00:00:00Z".to_string(),
        }];

        create_proactive_windows(&conn, &health).unwrap();

        let windows = get_open_windows(&conn);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].window_type, "knowledge");
        assert!(windows[0].title.contains("stale-crate"));
        assert!(windows[0].title.contains("still maintained"));
        assert_eq!(windows[0].streets_engine.as_deref(), Some("Education"));
    }

    #[test]
    fn test_create_proactive_windows_security() {
        let conn = test_conn();
        let health = vec![DependencyHealth {
            package_name: "vuln-pkg".to_string(),
            ecosystem: "javascript".to_string(),
            installed_version: Some("1.0.0".to_string()),
            latest_known_version: None,
            days_since_last_release: None,
            health_status: HealthStatus::SecurityAlert,
            checked_at: "2026-03-27T00:00:00Z".to_string(),
        }];

        create_proactive_windows(&conn, &health).unwrap();

        let windows = get_open_windows(&conn);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].window_type, "security_patch");
        assert!(windows[0].title.contains("vuln-pkg"));
        assert!(windows[0].title.contains("vulnerability"));
        assert_eq!(windows[0].streets_engine.as_deref(), Some("Automation"));
        assert!(windows[0].urgency >= 0.85);
    }

    #[test]
    fn test_proactive_windows_deduplication() {
        let conn = test_conn();
        let health = vec![DependencyHealth {
            package_name: "dedupe-pkg".to_string(),
            ecosystem: "rust".to_string(),
            installed_version: None,
            latest_known_version: None,
            days_since_last_release: Some(250),
            health_status: HealthStatus::Stale,
            checked_at: "2026-03-27T00:00:00Z".to_string(),
        }];

        // First call creates the window
        create_proactive_windows(&conn, &health).unwrap();
        assert_eq!(get_open_windows(&conn).len(), 1);

        // Second call should not duplicate
        create_proactive_windows(&conn, &health).unwrap();
        assert_eq!(get_open_windows(&conn).len(), 1);
    }

    #[test]
    fn test_healthy_deps_no_windows() {
        let conn = test_conn();
        let health = vec![
            DependencyHealth {
                package_name: "healthy-pkg".to_string(),
                ecosystem: "rust".to_string(),
                installed_version: Some("1.0.0".to_string()),
                latest_known_version: None,
                days_since_last_release: Some(10),
                health_status: HealthStatus::Healthy,
                checked_at: "2026-03-27T00:00:00Z".to_string(),
            },
            DependencyHealth {
                package_name: "unknown-pkg".to_string(),
                ecosystem: "rust".to_string(),
                installed_version: None,
                latest_known_version: None,
                days_since_last_release: None,
                health_status: HealthStatus::Unknown,
                checked_at: "2026-03-27T00:00:00Z".to_string(),
            },
        ];

        create_proactive_windows(&conn, &health).unwrap();
        assert!(
            get_open_windows(&conn).is_empty(),
            "healthy/unknown should not create windows"
        );
    }

    #[test]
    fn test_health_status_serialization() {
        let dep = DependencyHealth {
            package_name: "test".to_string(),
            ecosystem: "rust".to_string(),
            installed_version: Some("1.0.0".to_string()),
            latest_known_version: None,
            days_since_last_release: Some(30),
            health_status: HealthStatus::SecurityAlert,
            checked_at: "2026-03-27T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&dep).unwrap();
        assert!(json.contains("\"security_alert\""));

        let parsed: DependencyHealth = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.health_status, HealthStatus::SecurityAlert);
        assert_eq!(parsed.package_name, "test");
    }

    #[test]
    fn test_medium_severity_not_security_alert() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_direct, is_dev)
             VALUES ('/app', 'mild-risk', '1.0.0', 'rust', 1, 0)",
            [],
        ).unwrap();
        // Medium severity alert — should NOT trigger SecurityAlert
        conn.execute(
            "INSERT INTO dependency_alerts (package_name, ecosystem, alert_type, severity, title)
             VALUES ('mild-risk', 'rust', 'deprecation', 'medium', 'Will be removed in v3')",
            [],
        )
        .unwrap();
        // Recent mention
        conn.execute(
            "INSERT INTO source_items (title, content, created_at)
             VALUES ('mild-risk update news', 'update', datetime('now', '-5 days'))",
            [],
        )
        .unwrap();

        let health = check_dependency_health(&conn).unwrap();
        assert_eq!(health.len(), 1);
        assert_eq!(
            health[0].health_status,
            HealthStatus::Healthy,
            "medium severity should not trigger SecurityAlert"
        );
    }
}
