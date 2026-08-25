// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for dependency_health — extracted from dependency_health.rs so the
//! production module stays under the Rust file-size ceiling (loaded via #[path]).

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

// ====================================================================
// Patched-alert auto-resolution
// ====================================================================

fn store_active_alert(db: &Database, pkg: &str, eco: &str, sev: &str, affected: &str) {
    db.store_dependency_alert(&crate::db::DependencyAlert {
        id: 0,
        package_name: pkg.to_string(),
        ecosystem: eco.to_string(),
        alert_type: "cve".to_string(),
        severity: sev.to_string(),
        title: format!("CVE in {pkg} (affected {affected})"),
        description: None,
        affected_versions: Some(affected.to_string()),
        source_url: None,
        source_item_id: None,
        detected_at: String::new(),
        resolved_at: None,
    })
    .unwrap();
}

#[test]
fn resolve_patched_alerts_only_when_install_is_out_of_range() {
    let db = crate::test_utils::test_db();

    // PATCHED — installed 10.27.0, affected < 10.26.0 => should resolve.
    store_active_alert(&db, "liquidjs", "npm", "CRITICAL", "< 10.26.0");
    db.store_dependency(
        "/p/app",
        "liquidjs",
        Some("10.27.0"),
        "javascript",
        false,
        None,
    )
    .unwrap();

    // STILL AFFECTED — installed 1.8.3, affected >= 1.1.0, <= 1.8.3 => keep.
    store_active_alert(&db, "shell-quote", "npm", "CRITICAL", ">= 1.1.0, <= 1.8.3");
    db.store_dependency(
        "/p/app",
        "shell-quote",
        Some("1.8.3"),
        "javascript",
        false,
        None,
    )
    .unwrap();

    // MIXED — 3.2.4 (affected) + 4.1.5 (safe), affected < 4.1.0 => keep,
    // because not EVERY installed instance is out of range.
    store_active_alert(&db, "vitest", "npm", "CRITICAL", "< 4.1.0");
    db.store_dependency("/p/app", "vitest", Some("3.2.4"), "javascript", true, None)
        .unwrap();
    db.store_dependency(
        "/p/other",
        "vitest",
        Some("4.1.5"),
        "javascript",
        true,
        None,
    )
    .unwrap();

    // NOT INSTALLED — alert exists, package absent from deps => keep (scan-gap safe).
    store_active_alert(&db, "ghost-pkg", "npm", "HIGH", "< 9.9.9");

    // UNKNOWN VERSION — installed with NULL version => keep (conservative).
    store_active_alert(&db, "mystery", "npm", "HIGH", "< 2.0.0");
    db.store_dependency("/p/app", "mystery", None, "javascript", false, None)
        .unwrap();

    assert_eq!(db.get_active_alerts().unwrap().len(), 5);

    let resolved = resolve_patched_dependency_alerts(&db).unwrap();
    assert_eq!(resolved, 1, "only the patched liquidjs alert resolves");

    let active = db.get_active_alerts().unwrap();
    let pkgs: std::collections::HashSet<&str> =
        active.iter().map(|a| a.package_name.as_str()).collect();
    assert!(!pkgs.contains("liquidjs"), "patched liquidjs resolved");
    assert!(pkgs.contains("shell-quote"), "in-range shell-quote kept");
    assert!(
        pkgs.contains("vitest"),
        "mixed-version vitest kept (3.2.4 affected)"
    );
    assert!(
        pkgs.contains("ghost-pkg"),
        "uninstalled alert kept (scan-gap safe)"
    );
    assert!(
        pkgs.contains("mystery"),
        "unknown-version alert kept (conservative)"
    );
}

#[test]
fn resolve_patched_alerts_is_noop_without_alerts() {
    let db = crate::test_utils::test_db();
    assert_eq!(resolve_patched_dependency_alerts(&db).unwrap(), 0);
}

// ---------------------------------------------------------------------------
// Audit-alert lifecycle: opened by an authority, closed by the same authority.
// ---------------------------------------------------------------------------

fn store_audit_alert(db: &Database, pkg: &str, eco: &str, title: &str, affected: Option<&str>) {
    db.store_dependency_alert(&crate::db::DependencyAlert {
        id: 0,
        package_name: pkg.to_string(),
        ecosystem: eco.to_string(),
        alert_type: "audit".to_string(),
        severity: "MEDIUM".to_string(),
        title: title.to_string(),
        description: None,
        affected_versions: affected.map(String::from),
        source_url: None,
        source_item_id: None,
        detected_at: String::new(),
        resolved_at: None,
    })
    .unwrap();
}

/// `(package, normalized_ecosystem, title)` — the shape reconcile compares on.
fn key(pkg: &str, eco: &str, title: &str) -> (String, String, String) {
    (
        pkg.to_string(),
        crate::sources::cve_matching::normalize_ecosystem(eco).to_string(),
        title.to_string(),
    )
}

fn eco_set(ecos: &[&str]) -> std::collections::HashSet<String> {
    ecos.iter()
        .map(|e| crate::sources::cve_matching::normalize_ecosystem(e).to_string())
        .collect()
}

/// The semver resolver must not touch audit-sourced alerts even when it thinks
/// the install is out of range. `cargo audit` read the real lockfile; this
/// function is re-deriving a verdict from a stored range with strictly less
/// information, and when the two disagreed it closed live advisories.
#[test]
fn resolve_skips_audit_sourced_alerts() {
    let db = crate::test_utils::test_db();
    // Range says "< 1.2.1" and the install is 1.10.1, so the generic resolver
    // would happily clear this. It must not, because the alert is audit-owned.
    store_audit_alert(
        &db,
        "bytes",
        "rust",
        "RUSTSEC-2026-0007: overflow",
        Some("<1.2.1"),
    );
    db.store_dependency("/p/app", "bytes", Some("1.10.1"), "rust", false, None)
        .unwrap();

    assert_eq!(
        resolve_patched_dependency_alerts(&db).unwrap(),
        0,
        "audit alerts are the reconciler's to retire, not this function's"
    );
    assert_eq!(db.get_active_alerts().unwrap().len(), 1);
}

/// A finding that disappears and comes back must land on its ORIGINAL row.
/// Re-inserting instead is what grew 129 rows for 13 advisories.
#[test]
fn returning_finding_reopens_its_original_row() {
    let db = crate::test_utils::test_db();
    let title = "RUSTSEC-2026-0007: overflow";
    store_audit_alert(&db, "bytes", "rust", title, None);

    let first = db.get_active_alerts().unwrap();
    assert_eq!(first.len(), 1);
    let original_id = first[0].id;
    let original_detected = first[0].detected_at.clone();

    // The alert gets resolved (by reconcile, or by the user).
    db.resolve_alert(original_id).unwrap();
    assert!(db.get_active_alerts().unwrap().is_empty());

    // The next scan reports it again.
    store_audit_alert(&db, "bytes", "rust", title, None);

    let reopened = db.get_active_alerts().unwrap();
    assert_eq!(reopened.len(), 1, "must not create a parallel row");
    assert_eq!(
        reopened[0].id, original_id,
        "the same advisory must reopen its original row"
    );
    assert_eq!(
        reopened[0].detected_at, original_detected,
        "first-seen time is history and must survive a reopen"
    );
}

/// Present in the fresh scan => keep. Absent => retire.
#[test]
fn reconcile_retires_only_what_the_audit_stopped_reporting() {
    let db = crate::test_utils::test_db();
    store_audit_alert(&db, "bytes", "rust", "RUSTSEC-2026-0007: overflow", None);
    store_audit_alert(&db, "rkyv", "rust", "RUSTSEC-2026-0233: uaf", None);
    assert_eq!(db.get_active_alerts().unwrap().len(), 2);

    // This cycle cargo-audit reports only rkyv — bytes was upgraded.
    let current = [key("rkyv", "rust", "RUSTSEC-2026-0233: uaf")]
        .into_iter()
        .collect();

    let retired = db
        .reconcile_audit_alerts(&eco_set(&["crates.io"]), &current)
        .unwrap();
    assert_eq!(retired, 1);

    let active = db.get_active_alerts().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].package_name, "rkyv");
}

/// THE safety property. If an ecosystem's audit never ran — tool missing,
/// timed out, unreadable output, or a lockfile format it does not parse — its
/// alerts must survive untouched. An empty finding set from a broken toolchain
/// must never read as "nothing is vulnerable any more".
#[test]
fn reconcile_leaves_unaudited_ecosystems_untouched() {
    let db = crate::test_utils::test_db();
    store_audit_alert(&db, "bytes", "rust", "RUSTSEC-2026-0007: overflow", None);
    store_audit_alert(&db, "lodash", "npm", "GHSA-x: prototype pollution", None);

    // Only the Rust audit completed; npm was skipped (e.g. pnpm-only tree).
    // Nothing was reported by either.
    let empty = std::collections::HashSet::new();
    let retired = db
        .reconcile_audit_alerts(&eco_set(&["crates.io"]), &empty)
        .unwrap();
    assert_eq!(retired, 1, "only the audited ecosystem may be retired");

    let active = db.get_active_alerts().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(
        active[0].package_name, "lodash",
        "the un-audited npm alert must survive"
    );

    // And with NO ecosystem audited at all, nothing moves.
    assert_eq!(
        db.reconcile_audit_alerts(&std::collections::HashSet::new(), &empty)
            .unwrap(),
        0
    );
    assert_eq!(db.get_active_alerts().unwrap().len(), 1);
}

/// Reconcile is scoped to audit-sourced rows; CVE/OSV alerts have their own
/// lifecycle and must be invisible to it.
#[test]
fn reconcile_ignores_non_audit_alerts() {
    let db = crate::test_utils::test_db();
    store_active_alert(&db, "liquidjs", "npm", "CRITICAL", "< 10.26.0");
    let empty = std::collections::HashSet::new();

    assert_eq!(
        db.reconcile_audit_alerts(&eco_set(&["npm"]), &empty)
            .unwrap(),
        0
    );
    assert_eq!(db.get_active_alerts().unwrap().len(), 1);
}

#[test]
fn resolve_keeps_alert_when_affected_range_unparseable() {
    let db = crate::test_utils::test_db();
    // Garbage range can't be parsed -> version_is_affected is conservative
    // (true) -> alert is NOT resolved even though a version is installed.
    store_active_alert(&db, "weird", "npm", "HIGH", "not-a-range");
    db.store_dependency("/p/app", "weird", Some("1.0.0"), "javascript", false, None)
        .unwrap();
    assert_eq!(resolve_patched_dependency_alerts(&db).unwrap(), 0);
    assert_eq!(db.get_active_alerts().unwrap().len(), 1);
}

fn di(pkg: &str, version: &str, is_direct: bool) -> crate::db::DependencyInstanceInput {
    crate::db::DependencyInstanceInput {
        package_name: pkg.to_string(),
        version: version.to_string(),
        is_direct,
        is_dev: false,
        scope: "unknown".to_string(),
    }
}

#[test]
fn resolve_keeps_alert_when_a_collapsed_duplicate_is_still_vulnerable() {
    // THE false-negative fix (Phase 92). The collapsed user_dependencies row
    // keeps only the patched 4.17.21 survivor — on that data alone this
    // auto-resolver would clear the alert. But the project also installs a
    // vulnerable 4.17.20 duplicate, retained only by the multi-version
    // inventory. Folding instances in must KEEP the alert.
    let db = crate::test_utils::test_db();
    store_active_alert(&db, "lodash", "npm", "CRITICAL", "< 4.17.21");
    db.store_dependency(
        "/p/app",
        "lodash",
        Some("4.17.21"),
        "javascript",
        false,
        None,
    )
    .unwrap();
    db.store_dependency_instances(
        "/p/app",
        "javascript",
        &[
            di("lodash", "4.17.21", true),
            di("lodash", "4.17.20", false),
        ],
    )
    .unwrap();

    assert_eq!(
        resolve_patched_dependency_alerts(&db).unwrap(),
        0,
        "the hidden vulnerable 4.17.20 duplicate must keep the alert live"
    );
    assert_eq!(db.get_active_alerts().unwrap().len(), 1);
}

#[test]
fn resolve_still_clears_when_all_instances_are_patched() {
    // Regression guard: instances present, every version out of range -> the
    // alert still auto-resolves (the fold does not over-suppress).
    let db = crate::test_utils::test_db();
    store_active_alert(&db, "safe-pkg", "npm", "HIGH", "< 2.0.0");
    db.store_dependency(
        "/p/app",
        "safe-pkg",
        Some("2.5.0"),
        "javascript",
        false,
        None,
    )
    .unwrap();
    db.store_dependency_instances(
        "/p/app",
        "javascript",
        &[
            di("safe-pkg", "2.5.0", true),
            di("safe-pkg", "2.6.0", false),
        ],
    )
    .unwrap();

    assert_eq!(
        resolve_patched_dependency_alerts(&db).unwrap(),
        1,
        "all installed instances patched -> alert resolves"
    );
}
