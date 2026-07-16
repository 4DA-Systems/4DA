// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for dependency intelligence CRUD operations.

use crate::db::dependencies::types::{DependencyAlert, DependencyInstanceInput};
use crate::test_utils::test_db;

fn inst(name: &str, version: &str, is_direct: bool) -> DependencyInstanceInput {
    DependencyInstanceInput {
        package_name: name.to_string(),
        version: version.to_string(),
        is_direct,
        is_dev: false,
        scope: "unknown".to_string(),
    }
}

#[test]
fn dependency_instances_keep_multiple_versions_of_one_package() {
    // THE launch-blocking correctness gate (Phase 92): user_dependencies /
    // project_dependencies collapse to one row per (project, package,
    // ecosystem), so a package installed at two versions in ONE project loses a
    // version — and a negative verdict computed against only the survivor can
    // pass a still-vulnerable duplicate as "not affected". dependency_instances
    // must retain BOTH.
    let db = test_db();
    let project = "/projects/monorepo";
    db.store_dependency_instances(
        project,
        "javascript",
        &[
            inst("lodash", "4.17.20", false),
            inst("lodash", "4.17.21", true),
            inst("react", "18.3.1", true),
        ],
    )
    .unwrap();

    let all = db.get_dependency_instances(project).unwrap();
    assert_eq!(
        all.len(),
        3,
        "both lodash versions + react must be retained"
    );

    let lodash: Vec<_> = all.iter().filter(|i| i.package_name == "lodash").collect();
    assert_eq!(
        lodash.len(),
        2,
        "both lodash instances survive (no collapse)"
    );
    let mut versions: Vec<&str> = lodash.iter().map(|i| i.version.as_str()).collect();
    versions.sort_unstable();
    assert_eq!(versions, vec!["4.17.20", "4.17.21"]);
    // is_direct is tracked per instance, not collapsed away.
    assert!(lodash.iter().any(|i| i.version == "4.17.21" && i.is_direct));
    assert!(lodash
        .iter()
        .any(|i| i.version == "4.17.20" && !i.is_direct));
}

#[test]
fn dependency_instances_refresh_not_accumulate_on_rescan() {
    // DELETE-then-insert per (project, ecosystem): a rescan after an upgrade
    // must REPLACE the set, so a version upgraded away does not linger as a
    // phantom vulnerable instance (the opposite failure to the collapse).
    let db = test_db();
    let project = "/projects/app";
    db.store_dependency_instances(project, "rust", &[inst("tokio", "1.35.0", true)])
        .unwrap();
    db.store_dependency_instances(project, "rust", &[inst("tokio", "1.40.0", true)])
        .unwrap();

    let all = db.get_dependency_instances(project).unwrap();
    assert_eq!(all.len(), 1, "rescan replaces, not accumulates");
    assert_eq!(all[0].version, "1.40.0", "the fresh version wins");
}

#[test]
fn dependency_instances_isolate_per_ecosystem_on_refresh() {
    // A rust rescan must not wipe the javascript instances of the same project.
    let db = test_db();
    let project = "/projects/tauri-app";
    db.store_dependency_instances(project, "rust", &[inst("serde", "1.0.200", true)])
        .unwrap();
    db.store_dependency_instances(project, "javascript", &[inst("react", "18.3.1", true)])
        .unwrap();
    // Re-scan rust only.
    db.store_dependency_instances(project, "rust", &[inst("serde", "1.0.210", true)])
        .unwrap();

    let all = db.get_dependency_instances(project).unwrap();
    assert_eq!(all.len(), 2, "rust refresh leaves the js instance intact");
    assert!(all
        .iter()
        .any(|i| i.package_name == "react" && i.version == "18.3.1"));
    assert!(all
        .iter()
        .any(|i| i.package_name == "serde" && i.version == "1.0.210"));
}

#[test]
fn get_package_instances_is_cross_project_and_ecosystem_normalized() {
    // The version-confirmed matcher's read: every installed version of a
    // package across all projects, reachable by ACE language string OR OSV name.
    let db = test_db();
    db.store_dependency_instances("/projects/a", "rust", &[inst("openssl", "0.10.55", true)])
        .unwrap();
    db.store_dependency_instances("/projects/b", "rust", &[inst("openssl", "0.10.70", true)])
        .unwrap();

    // Queried by the OSV ecosystem name ("crates.io"), not the stored language ("rust").
    let instances = db.get_package_instances("crates.io", "openssl").unwrap();
    assert_eq!(instances.len(), 2, "both projects' versions returned");
    let mut versions: Vec<&str> = instances.iter().map(|i| i.version.as_str()).collect();
    versions.sort_unstable();
    assert_eq!(versions, vec!["0.10.55", "0.10.70"]);
    // And by the stored language string too.
    assert_eq!(
        db.get_package_instances("rust", "openssl").unwrap().len(),
        2
    );
}

#[test]
fn dependency_instances_coverage_gate_and_exclusion() {
    let db = test_db();
    // Excluded paths (agent worktrees / scratch) never populate — the write is a
    // silent no-op, mirroring store_dependency.
    let written = db
        .store_dependency_instances(
            "/repo/.claude/worktrees/x",
            "rust",
            &[inst("tokio", "1.0.0", true)],
        )
        .unwrap();
    assert_eq!(written, 0, "excluded path stores nothing");
    assert!(!db
        .project_has_dependency_instances("/repo/.claude/worktrees/x")
        .unwrap());

    // A real project: coverage gate flips true once populated.
    assert!(!db
        .project_has_dependency_instances("/projects/real")
        .unwrap());
    db.store_dependency_instances(
        "/projects/real",
        "go",
        &[inst("golang.org/x/net", "0.17.0", true)],
    )
    .unwrap();
    assert!(db
        .project_has_dependency_instances("/projects/real")
        .unwrap());
}

#[test]
fn test_store_and_retrieve_dependency() {
    let db = test_db();
    db.store_dependency(
        "/projects/myapp",
        "tokio",
        Some("1.35.0"),
        "rust",
        false,
        Some("MIT"),
    )
    .unwrap();
    db.store_dependency(
        "/projects/myapp",
        "serde",
        None,
        "rust",
        false,
        Some("MIT OR Apache-2.0"),
    )
    .unwrap();
    db.store_dependency(
        "/projects/myapp",
        "pretty_assertions",
        None,
        "rust",
        true,
        None,
    )
    .unwrap();

    let deps = db.get_project_dependencies("/projects/myapp").unwrap();
    assert_eq!(deps.len(), 3);

    let tokio = deps.iter().find(|d| d.package_name == "tokio").unwrap();
    assert_eq!(tokio.version.as_deref(), Some("1.35.0"));
    assert_eq!(tokio.ecosystem, "rust");
    assert!(!tokio.is_dev);
    assert_eq!(tokio.license.as_deref(), Some("MIT"));

    let pa = deps
        .iter()
        .find(|d| d.package_name == "pretty_assertions")
        .unwrap();
    assert!(pa.is_dev);
    assert_eq!(pa.license, None);
}

#[test]
fn store_dependency_dedups_raw_and_canonical_paths() {
    // A lockfile processor passes the RAW scan path (OS backslashes); the manifest
    // scan stores the CANONICAL path. Before canonicalization these produced TWO rows
    // (a null-version + a versioned dup) for one dependency. They must now collapse to
    // ONE row, findable via either path form.
    let db = test_db();
    db.store_dependency("proj\\app", "serde", None, "rust", false, None)
        .unwrap();
    db.store_dependency("proj/app", "serde", Some("1.2.3"), "rust", false, None)
        .unwrap();

    let via_raw = db.get_project_dependencies("proj\\app").unwrap();
    let via_canon = db.get_project_dependencies("proj/app").unwrap();
    assert_eq!(
        via_raw.len(),
        1,
        "raw + canonical writes must collapse to one row"
    );
    assert_eq!(via_canon.len(), 1, "found via the canonical path too");
    assert_eq!(
        via_raw[0].version.as_deref(),
        Some("1.2.3"),
        "version from the lockfile write is preserved on the single row"
    );
}

#[test]
fn test_upsert_updates_last_seen() {
    let db = test_db();
    db.store_dependency(
        "/projects/myapp",
        "react",
        Some("18.0.0"),
        "javascript",
        false,
        Some("MIT"),
    )
    .unwrap();
    // Upsert with new version
    db.store_dependency(
        "/projects/myapp",
        "react",
        Some("19.0.0"),
        "javascript",
        false,
        None,
    )
    .unwrap();

    let deps = db.get_project_dependencies("/projects/myapp").unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].version.as_deref(), Some("19.0.0"));
    // License should be preserved from the first insert (COALESCE keeps existing)
    assert_eq!(deps[0].license.as_deref(), Some("MIT"));
}

#[test]
fn test_cross_project_packages() {
    let db = test_db();
    db.store_dependency("/projects/app1", "serde", None, "rust", false, None)
        .unwrap();
    db.store_dependency("/projects/app2", "serde", None, "rust", false, None)
        .unwrap();
    db.store_dependency("/projects/app1", "tokio", None, "rust", false, None)
        .unwrap();

    let cross = db.get_cross_project_packages().unwrap();
    assert_eq!(cross.len(), 1);
    assert_eq!(cross[0].package_name, "serde");
    assert_eq!(cross[0].project_count, 2);
}

#[test]
fn test_store_and_resolve_alert() {
    let db = test_db();
    let alert = DependencyAlert {
        id: 0,
        package_name: "lodash".to_string(),
        ecosystem: "javascript".to_string(),
        alert_type: "vulnerability".to_string(),
        severity: "critical".to_string(),
        title: "Prototype pollution in lodash < 4.17.21".to_string(),
        description: Some("CVE-2021-23337".to_string()),
        affected_versions: Some("< 4.17.21".to_string()),
        source_url: None,
        source_item_id: None,
        detected_at: String::new(),
        resolved_at: None,
    };

    let alert_id = db.store_dependency_alert(&alert).unwrap();
    assert!(alert_id > 0);

    let active = db.get_active_alerts().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].package_name, "lodash");

    db.resolve_alert(alert_id).unwrap();
    let active_after = db.get_active_alerts().unwrap();
    assert_eq!(active_after.len(), 0);
}

#[test]
fn test_get_all_user_dependencies() {
    let db = test_db();
    db.store_dependency("/projects/app1", "tokio", None, "rust", false, None)
        .unwrap();
    db.store_dependency("/projects/app2", "react", None, "javascript", false, None)
        .unwrap();

    let all = db.get_all_user_dependencies().unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn test_alert_deduplication() {
    let db = test_db();
    let alert = DependencyAlert {
        id: 0,
        package_name: "lodash".to_string(),
        ecosystem: "javascript".to_string(),
        alert_type: "vulnerability".to_string(),
        severity: "critical".to_string(),
        title: "Prototype pollution".to_string(),
        description: None,
        affected_versions: None,
        source_url: None,
        source_item_id: None,
        detected_at: String::new(),
        resolved_at: None,
    };

    // First insert should succeed
    let id1 = db.store_dependency_alert(&alert).unwrap();
    assert!(id1 > 0);

    // Second insert of same alert should be skipped (returns 0)
    let id2 = db.store_dependency_alert(&alert).unwrap();
    assert_eq!(id2, 0, "Duplicate alert should return 0");

    // Only one alert should exist
    let active = db.get_active_alerts().unwrap();
    assert_eq!(active.len(), 1);

    // alert_exists should return true
    assert!(db
        .alert_exists("lodash", "javascript", "Prototype pollution")
        .unwrap());
    assert!(!db
        .alert_exists("lodash", "javascript", "Different title")
        .unwrap());
}

#[test]
fn test_transitive_dependency_storage() {
    let db = test_db();

    // Store a direct dependency first
    db.store_dependency(
        "/projects/myapp",
        "serde",
        Some("1.0.204"),
        "rust",
        false,
        None,
    )
    .unwrap();

    // Store a transitive dependency
    db.store_transitive_dependency(
        "/projects/myapp",
        "serde_derive",
        Some("1.0.204"),
        "rust",
        false,
    )
    .unwrap();

    let deps = db.get_project_dependencies("/projects/myapp").unwrap();
    assert_eq!(deps.len(), 2);

    let serde = deps.iter().find(|d| d.package_name == "serde").unwrap();
    assert!(serde.is_direct, "Manifest dep should be direct");
    assert_eq!(serde.version.as_deref(), Some("1.0.204"));

    let serde_derive = deps
        .iter()
        .find(|d| d.package_name == "serde_derive")
        .unwrap();
    assert!(
        !serde_derive.is_direct,
        "Lockfile-only dep should be transitive"
    );
    assert_eq!(serde_derive.version.as_deref(), Some("1.0.204"));
}

#[test]
fn test_get_relevant_user_dependencies_filters() {
    let db = test_db();
    // Direct, non-dev — should be included
    db.store_dependency(
        "/projects/myapp",
        "tokio",
        Some("1.35.0"),
        "rust",
        false,
        None,
    )
    .unwrap();
    // Dev dep — should be excluded
    db.store_dependency(
        "/projects/myapp",
        "pretty_assertions",
        None,
        "rust",
        true,
        None,
    )
    .unwrap();
    // Transitive — should be excluded
    db.store_transitive_dependency(
        "/projects/myapp",
        "serde_derive",
        Some("1.0.204"),
        "rust",
        false,
    )
    .unwrap();
    // Worktree path — should be excluded
    db.store_dependency(
        "/projects/.claude/worktrees/agent-abc123/myapp",
        "react",
        Some("18.0.0"),
        "javascript",
        false,
        None,
    )
    .unwrap();

    let relevant = db.get_relevant_user_dependencies().unwrap();
    assert_eq!(
        relevant.len(),
        1,
        "Only direct non-dev non-worktree deps should be returned"
    );
    assert_eq!(relevant[0].package_name, "tokio");
}

#[test]
fn test_get_auditable_user_dependencies_keeps_scope_but_filters_ephemeral_paths() {
    let db = test_db();
    db.store_dependency(
        "/projects/myapp",
        "tokio",
        Some("1.35.0"),
        "rust",
        false,
        None,
    )
    .unwrap();
    db.store_dependency(
        "/projects/myapp",
        "pretty_assertions",
        Some("1.4.0"),
        "rust",
        true,
        None,
    )
    .unwrap();
    db.store_transitive_dependency(
        "/projects/myapp",
        "serde_derive",
        Some("1.0.204"),
        "rust",
        false,
    )
    .unwrap();
    db.store_dependency(
        "/projects/.claude/worktrees/named-branch/myapp",
        "react",
        Some("19.0.0"),
        "javascript",
        false,
        None,
    )
    .unwrap();
    db.store_dependency(
        r"C:\Users\Admin\AppData\Local\Temp\clone",
        "axios",
        Some("1.8.0"),
        "javascript",
        false,
        None,
    )
    .unwrap();

    let auditable = db.get_auditable_user_dependencies().unwrap();
    let names: Vec<&str> = auditable
        .iter()
        .map(|dep| dep.package_name.as_str())
        .collect();

    assert_eq!(auditable.len(), 3);
    assert!(names.contains(&"tokio"));
    assert!(names.contains(&"pretty_assertions"));
    assert!(names.contains(&"serde_derive"));
    assert!(!names.contains(&"react"));
    assert!(!names.contains(&"axios"));
}

#[test]
fn test_transitive_does_not_downgrade_direct() {
    let db = test_db();

    // Store as direct first (from manifest)
    db.store_dependency(
        "/projects/myapp",
        "tokio",
        Some("1.35.0"),
        "rust",
        false,
        None,
    )
    .unwrap();

    // Then store same package as transitive (from lockfile) — should NOT downgrade is_direct
    db.store_transitive_dependency("/projects/myapp", "tokio", Some("1.35.1"), "rust", false)
        .unwrap();

    let deps = db.get_project_dependencies("/projects/myapp").unwrap();
    assert_eq!(deps.len(), 1);

    let tokio = deps.iter().find(|d| d.package_name == "tokio").unwrap();
    assert!(
        tokio.is_direct,
        "Direct dep should stay direct even after transitive upsert"
    );
    // Version should be updated to lockfile version (COALESCE keeps non-null)
    assert_eq!(tokio.version.as_deref(), Some("1.35.1"));
}

/// Validates the startup cleanup pipeline: the agent-infra purge + dedup
/// (db::purge_agent_infra_dependencies) followed by the ephemeral temp-path
/// purge SQL (app_setup.rs startup cleanup block).
#[test]
fn test_startup_user_dependency_cleanup() {
    let db = test_db();
    let conn = db.conn.lock();

    // --- Seed test data ---

    // 3 worktree rows (should be purged by query 1)
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
         VALUES ('D:\\4DA\\.claude\\worktrees\\agent-abc123\\src', 'tokio', '1.35.0', 'rust', 0, 1, datetime('now'), datetime('now'))",
        [],
    ).expect("insert worktree row 1");
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
         VALUES ('D:\\4DA\\.claude\\worktrees\\agent-def456\\src', 'serde', '1.0.0', 'rust', 0, 1, datetime('now'), datetime('now'))",
        [],
    ).expect("insert worktree row 2");
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
         VALUES ('/home/user/.claude/worktrees/agent-789/proj', 'react', '18.0.0', 'javascript', 0, 1, datetime('now'), datetime('now'))",
        [],
    ).expect("insert worktree row 3");

    // 2 duplicates of the same logical dep: slash-style path variant plus a
    // hyphen/underscore name variant in a fold-eligible ecosystem (rust).
    // Same case on both rows so the collapse is platform-independent — case
    // folding of the path is Windows-only (Linux paths are case-sensitive).
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
         VALUES ('D:\\Documents\\myapp', 'my-pkg', '1.0.0', 'rust', 0, 1, datetime('now'), datetime('now'))",
        [],
    ).expect("insert slash dup 1");
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
         VALUES ('D:/Documents/myapp', 'my_pkg', '2.0.0', 'rust', 0, 1, datetime('now'), datetime('now'))",
        [],
    ).expect("insert slash dup 2");

    // 1 temp-path row (should be purged by query 3)
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
         VALUES ('C:\\Users\\Admin\\AppData\\Local\\Temp\\clone\\proj', 'axios', '1.0.0', 'javascript', 0, 1, datetime('now'), datetime('now'))",
        [],
    ).expect("insert temp row");

    // 2 clean rows (should survive all queries)
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
         VALUES ('D:\\4DA', 'tauri', '2.0.0', 'rust', 0, 1, datetime('now'), datetime('now'))",
        [],
    ).expect("insert clean row 1");
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
         VALUES ('D:\\projects\\web', 'vite', '5.0.0', 'javascript', 0, 1, datetime('now'), datetime('now'))",
        [],
    ).expect("insert clean row 2");

    // Verify starting count: 3 worktree + 2 dups + 1 temp + 2 clean = 8
    let before: i64 = conn
        .query_row("SELECT COUNT(*) FROM user_dependencies", [], |r| r.get(0))
        .expect("count before");
    assert_eq!(before, 8, "Expected 8 rows before cleanup");

    // --- Step 1+2: agent-infra purge + dedup (the app_setup self-heal call) ---
    let counts = crate::db::purge_agent_infra_dependencies(&conn).expect("agent-infra purge");
    assert_eq!(counts.user_dependencies, 3, "Should purge 3 worktree rows");
    assert_eq!(
        counts.duplicates, 1,
        "Should deduplicate 1 casing/hyphen variant"
    );

    // --- Step 3: purge temp paths (separate app_setup block) ---
    let deleted_temp = conn
        .execute(
            "DELETE FROM user_dependencies WHERE project_path LIKE '%/tmp/%' OR project_path LIKE '%\\tmp\\%' OR project_path LIKE '%AppData%Local%Temp%'",
            [],
        )
        .expect("temp purge");
    assert_eq!(deleted_temp, 1, "Should purge 1 temp-path row");

    // --- Verify final state ---
    let remaining: i64 = conn
        .query_row("SELECT COUNT(*) FROM user_dependencies", [], |r| r.get(0))
        .expect("count after");
    assert_eq!(
        remaining, 3,
        "Should have 3 rows remaining: 1 surviving dup + 2 clean"
    );

    // Verify the surviving rows are the expected ones
    let mut stmt = conn
        .prepare("SELECT package_name FROM user_dependencies ORDER BY package_name")
        .expect("prepare final query");
    let names: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .expect("query names")
        .filter_map(|r| r.ok())
        .collect();
    // my_pkg survived (higher rowid than my-pkg), plus tauri and vite
    assert_eq!(names, vec!["my_pkg", "tauri", "vite"]);
}

// ---------------------------------------------------------------------------
// Dependency edges (Step 1: reachability foundation)
// ---------------------------------------------------------------------------

use crate::ace::scanner::{DependencyEdge, EdgeScope};

#[test]
fn test_store_and_retrieve_dependency_edges() {
    let db = test_db();
    let edges = vec![
        DependencyEdge {
            parent: "app".to_string(),
            parent_version: Some("0.1.0".to_string()),
            child: "serde".to_string(),
            child_version: Some("1.0.190".to_string()),
            scope: EdgeScope::Runtime,
        },
        DependencyEdge {
            parent: "app".to_string(),
            parent_version: Some("0.1.0".to_string()),
            child: "jest".to_string(),
            child_version: None,
            scope: EdgeScope::Dev,
        },
    ];

    let n = db
        .store_dependency_edges("/projects/myapp", "rust", &edges)
        .expect("store edges");
    assert_eq!(n, 2);

    let rows = db
        .get_dependency_edges("/projects/myapp")
        .expect("get edges");
    assert_eq!(rows.len(), 2);
    assert!(rows
        .iter()
        .any(|r| r.child_package == "serde" && r.scope == "runtime"));
    assert!(rows
        .iter()
        .any(|r| r.child_package == "jest" && r.scope == "dev"));
}

#[test]
fn test_store_dependency_edges_skips_worktree_and_empty() {
    let db = test_db();
    let edges = vec![DependencyEdge {
        parent: "app".to_string(),
        parent_version: None,
        child: "x".to_string(),
        child_version: None,
        scope: EdgeScope::Runtime,
    }];

    // Worktree path is excluded.
    let n = db
        .store_dependency_edges("/home/u/repo/.claude/worktrees/agent-abc", "rust", &edges)
        .expect("store excluded");
    assert_eq!(n, 0);

    // Empty input stores nothing.
    let n = db
        .store_dependency_edges("/projects/clean", "rust", &[])
        .expect("store empty");
    assert_eq!(n, 0);
    assert!(db
        .get_dependency_edges("/projects/clean")
        .unwrap()
        .is_empty());
}

// ============================================================================
// Agent-infrastructure exclusion + self-heal purge
// (regression tests for the .claude/ fixture pollution that put phantom
// Ruby/PHP CVEs on the Preemption Radar — nokogiri / symfony from
// .claude/plans/ledger-fixtures/*)
// ============================================================================

/// Insert a user_dependencies row with raw SQL, bypassing the write-time guard
/// (simulates rows written before the guard existed).
fn raw_insert_user_dep(db: &crate::db::Database, project_path: &str, package: &str, eco: &str) {
    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
         VALUES (?1, ?2, '1.0.0', ?3, 0, 1, datetime('now'), datetime('now'))",
        rusqlite::params![project_path, package, eco],
    )
    .unwrap();
}

#[test]
fn store_dependency_rejects_agent_infra_paths() {
    let db = test_db();

    // Fixture + worktree paths, both slash styles and case variants, all rejected.
    let excluded = [
        r"D:\4DA\.claude\plans\ledger-fixtures\ruby-rails-app",
        "d:/4da/.claude/plans/ledger-fixtures/php-laravel-app",
        r"D:\4DA\.CLAUDE\worktrees\ci-hardening\src-tauri",
        "/home/u/repo/.claude/worktrees/agent-abc",
        "/home/u/repo/.codex/worktrees/agent-xyz",
        r"C:\repo\.codex\scratch",
    ];
    for path in excluded {
        db.store_dependency(path, "nokogiri", Some("1.13.0"), "ruby", false, None)
            .unwrap();
        db.store_transitive_dependency(path, "rack", Some("2.2.0"), "ruby", false)
            .unwrap();
        assert!(
            db.get_project_dependencies(path).unwrap().is_empty(),
            "agent-infra path must not be stored: {path}"
        );
    }

    // Real project paths are kept (including ones merely containing 'claude').
    let kept = [r"D:\projects\my-real-app", "/home/u/projects/claude-client"];
    for path in kept {
        db.store_dependency(path, "serde", Some("1.0.0"), "rust", false, None)
            .unwrap();
        assert_eq!(
            db.get_project_dependencies(path).unwrap().len(),
            1,
            "real project path must be stored: {path}"
        );
    }

    // The auditable pool sees only the real projects.
    let auditable = db.get_auditable_user_dependencies().unwrap();
    assert_eq!(auditable.len(), 2);
    assert!(auditable.iter().all(|d| d.package_name == "serde"));
}

#[test]
fn snapshot_project_deps_rejects_agent_infra_paths() {
    let db = test_db();
    let deps = vec![crate::db::dep_snapshots::DepEntry {
        name: "nokogiri".into(),
        ecosystem: "ruby".into(),
        version: None,
        is_direct: true,
        is_dev: false,
        source: "manifest".into(),
    }];

    let n = db
        .snapshot_project_deps(
            r"D:\4DA\.claude\plans\ledger-fixtures\ruby-rails-app",
            &deps,
        )
        .unwrap();
    assert_eq!(n, 0, "fixture snapshot must be a no-op");

    let n = db
        .snapshot_project_deps("/projects/real-app", &deps)
        .unwrap();
    assert_eq!(n, 1, "real project snapshot must be stored");
}

#[test]
fn purge_agent_infra_deletes_fixture_and_worktree_rows() {
    let db = test_db();

    // Legacy pollution written before the write-time guard, in both path styles.
    raw_insert_user_dep(
        &db,
        r"D:\4DA\.claude\plans\ledger-fixtures\ruby-rails-app",
        "nokogiri",
        "ruby",
    );
    raw_insert_user_dep(
        &db,
        "d:/4da/.claude/plans/ledger-fixtures/php-laravel-app",
        "symfony/http-foundation",
        "php",
    );
    raw_insert_user_dep(
        &db,
        "d:/4da/.claude/worktrees/ci-hardening",
        "serde",
        "rust",
    );
    raw_insert_user_dep(&db, "/repo/.codex/worktrees/agent-x", "left-pad", "npm");
    // F1 regression: legitimate projects that the OLD substring pattern
    // ('%worktrees%agent-%') deleted every startup — flip-flop churn, since
    // the write guard did not mirror it and scans kept re-adding them. The
    // pattern is gone (agent worktrees always live under .claude/.codex,
    // already covered); these rows MUST survive.
    raw_insert_user_dep(&db, "/home/u/worktrees/reagent-app", "chalk", "npm");
    raw_insert_user_dep(&db, r"D:\worktrees\agent-ui", "vue", "npm");
    // Real project survives.
    raw_insert_user_dep(&db, "/projects/real-app", "tokio", "rust");

    // Legacy snapshot pollution.
    {
        let conn = db.conn.lock();
        conn.execute(
            r"INSERT INTO dependency_snapshots (project_path, package_name, ecosystem, version, is_direct, is_dev, source, scanned_at)
             VALUES ('D:\4DA\.claude\worktrees\agent-a1\src-tauri', 'serde', 'rust', '1.0.0', 1, 0, 'manifest', CURRENT_TIMESTAMP),
                    ('/projects/real-app', 'tokio', 'rust', '1.35.0', 1, 0, 'manifest', CURRENT_TIMESTAMP)",
            [],
        )
        .unwrap();
    }

    let counts = {
        let conn = db.conn.lock();
        crate::db::purge_agent_infra_dependencies(&conn).unwrap()
    };
    assert_eq!(
        counts.user_dependencies, 4,
        "all .claude/.codex user_dependencies purged — and ONLY those"
    );
    assert_eq!(
        counts.dependency_snapshots, 1,
        "agent-infra snapshot purged"
    );

    let remaining = db.get_all_user_dependencies().unwrap();
    let names: Vec<&str> = remaining.iter().map(|d| d.package_name.as_str()).collect();
    assert_eq!(remaining.len(), 3);
    assert!(names.contains(&"tokio"), "real project survives");
    assert!(
        names.contains(&"chalk"),
        "legit /worktrees/reagent-app project survives (F1 regression)"
    );
    assert!(
        names.contains(&"vue"),
        "legit D:\\worktrees\\agent-ui project survives (F1 regression)"
    );

    let snaps = db.get_current_deps("/projects/real-app").unwrap();
    assert_eq!(snaps.len(), 1, "real-project snapshot survives");
}

#[test]
fn purge_agent_infra_collapses_slash_variant_duplicates() {
    let db = test_db();

    // The duplicate-identity bug: the same dependency stored under the raw
    // Windows path (pre-canonicalization, 2026-06-20) AND the canonical
    // forward-slash path (post-canonicalization, 2026-07-03).
    // LOWER(project_path) alone does not collapse these — slash direction differs.
    // The purge's dedup key is case-insensitive ONLY on Windows hosts (case is
    // significant on Linux filesystems), so the case-variant twin is a
    // Windows-only fixture — on other hosts it would be a distinct project.
    raw_insert_user_dep(&db, r"D:\4DA\cli", "commander", "npm");
    if cfg!(windows) {
        raw_insert_user_dep(&db, "d:/4da/cli", "commander", "npm");
    } else {
        raw_insert_user_dep(&db, "D:/4DA/cli", "commander", "npm");
    }
    // Distinct projects must NOT be collapsed.
    raw_insert_user_dep(&db, "/projects/app-a", "serde", "rust");
    raw_insert_user_dep(&db, "/projects/app-b", "serde", "rust");

    let counts = {
        let conn = db.conn.lock();
        crate::db::purge_agent_infra_dependencies(&conn).unwrap()
    };
    assert_eq!(counts.duplicates, 1, "slash-variant duplicate collapsed");

    let remaining = db.get_all_user_dependencies().unwrap();
    assert_eq!(remaining.len(), 3);
    let commander: Vec<_> = remaining
        .iter()
        .filter(|d| d.package_name == "commander")
        .collect();
    assert_eq!(commander.len(), 1, "one commander row survives");
    let expected_key = if cfg!(windows) {
        "d:/4da/cli"
    } else {
        "D:/4DA/cli"
    };
    assert_eq!(
        commander[0].project_path, expected_key,
        "survivor sits on the canonical key"
    );
}

#[test]
fn purge_agent_infra_canonicalizes_residual_paths() {
    let db = test_db();

    // A backslash-only legacy row (no canonical twin): invisible to
    // path-scoped readers that canonicalize their query path. The purge must
    // rewrite it onto the canonical key.
    raw_insert_user_dep(&db, r"D:\Projects\Legacy-App", "express", "npm");

    let counts = {
        let conn = db.conn.lock();
        crate::db::purge_agent_infra_dependencies(&conn).unwrap()
    };
    assert_eq!(counts.canonicalized, 1, "residual path rewritten");

    // Now findable via any path form.
    let deps = db
        .get_project_dependencies(r"D:\Projects\Legacy-App")
        .unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].package_name, "express");
}
