//! Health dimension calculators for project health radar.
//!
//! Extracted from `project_health.rs` — computes freshness, security,
//! momentum, community scores and generates alerts for a project's
//! dependency health.

use crate::error::Result;
use crate::project_health::{HealthAlert, HealthDimension};
use crate::temporal::ProjectDependency;
use rusqlite::params;

/// Score dependency freshness based on version-info coverage.
pub(crate) fn compute_freshness(
    deps: &[ProjectDependency],
    _conn: &rusqlite::Connection,
) -> Result<HealthDimension> {
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

/// Score security exposure by searching source items for CVE / vulnerability mentions.
pub(crate) fn compute_security(
    deps: &[ProjectDependency],
    conn: &rusqlite::Connection,
) -> Result<HealthDimension> {
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

    // Factor in signal chain predictions — escalating security chains lower the score
    let chain_penalty: i64 = if let Ok(chains) = crate::signal_chains::detect_chains(conn) {
        chains
            .iter()
            .filter(|c| {
                let has_security = c.links.iter().any(|l| l.signal_type == "security_alert");
                if !has_security {
                    return false;
                }
                let prediction = crate::signal_chains::predict_chain_lifecycle(c);
                matches!(
                    prediction.phase,
                    crate::signal_chains::ChainPhase::Escalating
                        | crate::signal_chains::ChainPhase::Peak
                )
            })
            .count() as i64
    } else {
        0
    };

    // Combine direct hits with chain-detected threats
    let total_threats = security_hits + chain_penalty * 2; // chains are weighted 2x since they represent escalating patterns

    let score = if total_threats == 0 {
        0.95
    } else if total_threats <= 2 {
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
        details: if chain_penalty > 0 {
            format!(
                "{security_hits} security items + {chain_penalty} escalating signal chains for your dependencies"
            )
        } else {
            format!("{security_hits} security-related items found for your dependencies")
        },
    })
}

/// Score ecosystem momentum by counting recent source-item mentions of dependencies.
pub(crate) fn compute_momentum(
    deps: &[ProjectDependency],
    conn: &rusqlite::Connection,
) -> Result<HealthDimension> {
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
        details: format!("{total_mentions} mentions of your dependencies in recent sources"),
    })
}

/// Score community sentiment via positive-feedback mentions of dependencies.
pub(crate) fn compute_community(
    deps: &[ProjectDependency],
    conn: &rusqlite::Connection,
) -> Result<HealthDimension> {
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
        details: format!("{positive_mentions} positive community signals about your tech"),
    })
}

/// Generate alerts based on dimension scores and unversioned dependencies.
pub(crate) fn generate_alerts(
    deps: &[ProjectDependency],
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
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temporal::ProjectDependency;
    use rusqlite::Connection;

    /// Create an in-memory database with the tables needed by dimension calculators.
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

    /// Helper: build a ProjectDependency without touching the DB.
    fn make_dep(name: &str, version: Option<&str>, is_dev: bool) -> ProjectDependency {
        ProjectDependency {
            id: 0,
            project_path: "/test/project".to_string(),
            manifest_type: "cargo.toml".to_string(),
            package_name: name.to_string(),
            version: version.map(|v| v.to_string()),
            is_dev,
            is_direct: true,
            language: "rust".to_string(),
            last_scanned: "2026-02-28T00:00:00".to_string(),
        }
    }

    // ---- generate_alerts (pure function) ----

    #[test]
    fn generate_alerts_empty_when_scores_healthy() {
        let deps = vec![make_dep("serde", Some("1.0"), false)];
        let freshness = HealthDimension {
            score: 0.9,
            label: "Good".to_string(),
            details: "ok".to_string(),
        };
        let security = HealthDimension {
            score: 0.95,
            label: "Clean".to_string(),
            details: "ok".to_string(),
        };
        let alerts = generate_alerts(&deps, &freshness, &security);
        assert!(alerts.is_empty(), "Expected no alerts for healthy scores");
    }

    #[test]
    fn generate_alerts_critical_when_security_low() {
        let deps = vec![make_dep("serde", Some("1.0"), false)];
        let freshness = HealthDimension {
            score: 0.9,
            label: "Good".to_string(),
            details: "ok".to_string(),
        };
        let security = HealthDimension {
            score: 0.3,
            label: "Action needed".to_string(),
            details: "bad".to_string(),
        };
        let alerts = generate_alerts(&deps, &freshness, &security);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, "critical");
        assert!(alerts[0].message.contains("Security"));
    }

    #[test]
    fn generate_alerts_medium_when_freshness_low() {
        let deps = vec![make_dep("serde", Some("1.0"), false)];
        let freshness = HealthDimension {
            score: 0.4,
            label: "Needs attention".to_string(),
            details: "bad".to_string(),
        };
        let security = HealthDimension {
            score: 0.95,
            label: "Clean".to_string(),
            details: "ok".to_string(),
        };
        let alerts = generate_alerts(&deps, &freshness, &security);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, "medium");
        assert!(alerts[0].message.contains("version"));
    }

    #[test]
    fn generate_alerts_includes_unversioned_deps() {
        let deps = vec![
            make_dep("serde", None, false),
            make_dep("tokio", None, false),
            make_dep("anyhow", None, false),
            make_dep("thiserror", None, false), // 4th — should be excluded (take(3))
        ];
        let freshness = HealthDimension {
            score: 0.9,
            label: "Good".to_string(),
            details: "ok".to_string(),
        };
        let security = HealthDimension {
            score: 0.95,
            label: "Clean".to_string(),
            details: "ok".to_string(),
        };
        let alerts = generate_alerts(&deps, &freshness, &security);
        // Should have exactly 3 low-severity alerts (capped by take(3))
        assert_eq!(alerts.len(), 3);
        for alert in &alerts {
            assert_eq!(alert.severity, "low");
            assert!(alert.dependency.is_some());
        }
        // The 4th dep should NOT generate an alert
        let dep_names: Vec<_> = alerts
            .iter()
            .filter_map(|a| a.dependency.as_ref())
            .collect();
        assert!(!dep_names.contains(&&"thiserror".to_string()));
    }

    #[test]
    fn generate_alerts_combines_security_freshness_and_deps() {
        let deps = vec![make_dep("serde", None, false)];
        let freshness = HealthDimension {
            score: 0.3,
            label: "Needs attention".to_string(),
            details: "bad".to_string(),
        };
        let security = HealthDimension {
            score: 0.3,
            label: "Action needed".to_string(),
            details: "bad".to_string(),
        };
        let alerts = generate_alerts(&deps, &freshness, &security);
        // 1 critical (security) + 1 medium (freshness) + 1 low (unversioned dep)
        assert_eq!(alerts.len(), 3);
        let severities: Vec<&str> = alerts.iter().map(|a| a.severity.as_str()).collect();
        assert!(severities.contains(&"critical"));
        assert!(severities.contains(&"medium"));
        assert!(severities.contains(&"low"));
    }

    // ---- compute_freshness ----

    #[test]
    fn freshness_returns_perfect_when_no_deps() {
        let conn = setup_test_db();
        let result = compute_freshness(&[], &conn).unwrap();
        assert!((result.score - 1.0).abs() < f32::EPSILON);
        assert_eq!(result.label, "No dependencies");
    }

    #[test]
    fn freshness_scores_based_on_version_ratio() {
        let conn = setup_test_db();

        // All deps have versions => high score
        let deps_all_versioned = vec![
            make_dep("serde", Some("1.0"), false),
            make_dep("tokio", Some("1.35"), false),
        ];
        let result = compute_freshness(&deps_all_versioned, &conn).unwrap();
        assert!(result.score >= 0.8, "All versioned should score >= 0.8");
        assert_eq!(result.label, "Good");

        // No deps have versions => clamped to 0.3
        let deps_no_version = vec![
            make_dep("serde", None, false),
            make_dep("tokio", None, false),
        ];
        let result = compute_freshness(&deps_no_version, &conn).unwrap();
        assert!(
            (result.score - 0.3).abs() < f32::EPSILON,
            "No versions should clamp to 0.3"
        );
        assert_eq!(result.label, "Needs attention");

        // Half versioned => score ~0.5
        let deps_half = vec![
            make_dep("serde", Some("1.0"), false),
            make_dep("tokio", None, false),
        ];
        let result = compute_freshness(&deps_half, &conn).unwrap();
        assert!(
            (result.score - 0.5).abs() < f32::EPSILON,
            "Half versioned should be 0.5"
        );
        assert_eq!(result.label, "Fair");
    }

    // ---- compute_security ----

    #[test]
    fn security_clean_when_no_source_items() {
        let conn = setup_test_db();
        let deps = vec![make_dep("serde", Some("1.0"), false)];
        let result = compute_security(&deps, &conn).unwrap();
        assert!((result.score - 0.95).abs() < f32::EPSILON);
        assert_eq!(result.label, "Clean");
    }

    #[test]
    fn security_degrades_with_cve_mentions() {
        let conn = setup_test_db();
        let deps = vec![make_dep("openssl", Some("3.0"), false)];

        // Insert 3 security-related source items mentioning "openssl"
        for i in 0..3 {
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, content, created_at)
                 VALUES ('hackernews', ?1, ?2, 'details', datetime('now'))",
                params![format!("sec_{}", i), format!("openssl cve found {}", i),],
            )
            .unwrap();
        }

        let result = compute_security(&deps, &conn).unwrap();
        assert!(
            (result.score - 0.3).abs() < f32::EPSILON,
            "3+ security hits should score 0.3, got {}",
            result.score
        );
        assert_eq!(result.label, "Action needed");
    }

    // ---- compute_momentum ----

    #[test]
    fn momentum_low_when_no_mentions() {
        let conn = setup_test_db();
        let deps = vec![make_dep("serde", Some("1.0"), false)];
        let result = compute_momentum(&deps, &conn).unwrap();
        assert!((result.score - 0.3).abs() < f32::EPSILON);
        assert_eq!(result.label, "Low activity");
    }

    #[test]
    fn momentum_rises_with_mentions() {
        let conn = setup_test_db();
        let deps = vec![make_dep("react", Some("18.2"), false)];

        // Insert 10 source items mentioning "react" in title
        for i in 0..10 {
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, content, created_at)
                 VALUES ('hackernews', ?1, ?2, 'content', datetime('now'))",
                params![format!("react_{}", i), format!("New react feature {}", i),],
            )
            .unwrap();
        }

        let result = compute_momentum(&deps, &conn).unwrap();
        assert!(
            (result.score - 0.9).abs() < f32::EPSILON,
            "10+ mentions should score 0.9, got {}",
            result.score
        );
        assert_eq!(result.label, "Active");
    }

    #[test]
    fn momentum_skips_dev_dependencies() {
        let conn = setup_test_db();
        // Only a dev dependency — should be filtered out by the `!d.is_dev` filter
        let deps = vec![make_dep("prettier", Some("3.0"), true)];

        // Insert mentions for prettier
        for i in 0..10 {
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, content, created_at)
                 VALUES ('hackernews', ?1, ?2, 'content', datetime('now'))",
                params![format!("prettier_{}", i), format!("prettier update {}", i),],
            )
            .unwrap();
        }

        let result = compute_momentum(&deps, &conn).unwrap();
        // Dev deps skipped, so 0 mentions => score 0.3
        assert!(
            (result.score - 0.3).abs() < f32::EPSILON,
            "Dev deps should be skipped, got score {}",
            result.score
        );
    }

    // ---- compute_community ----

    #[test]
    fn community_neutral_with_no_feedback() {
        let conn = setup_test_db();
        let deps = vec![make_dep("serde", Some("1.0"), false)];
        let result = compute_community(&deps, &conn).unwrap();
        assert!((result.score - 0.5).abs() < f32::EPSILON);
        assert_eq!(result.label, "Neutral");
    }
}
