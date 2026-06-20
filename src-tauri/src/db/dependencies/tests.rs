// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for dependency intelligence CRUD operations.

use crate::db::dependencies::types::DependencyAlert;
use crate::test_utils::test_db;

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

/// Validates the startup cleanup SQL queries that purge worktree rows,
/// deduplicate by normalized name+path, and remove ephemeral temp paths
/// from user_dependencies (app_setup.rs startup cleanup block).
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

    // 2 casing duplicates of the same logical dep (query 2 keeps the latest rowid)
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
         VALUES ('D:\\Documents\\myapp', 'my-pkg', '1.0.0', 'rust', 0, 1, datetime('now'), datetime('now'))",
        [],
    ).expect("insert casing dup 1");
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
         VALUES ('D:\\documents\\myapp', 'my_pkg', '2.0.0', 'rust', 0, 1, datetime('now'), datetime('now'))",
        [],
    ).expect("insert casing dup 2");

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

    // --- Query 1: purge worktree rows ---
    let deleted_worktree = conn
        .execute(
            "DELETE FROM user_dependencies WHERE project_path LIKE '%worktrees%agent-%'",
            [],
        )
        .expect("worktree purge");
    assert_eq!(deleted_worktree, 3, "Should purge 3 worktree rows");

    // --- Query 2: deduplicate by normalized name + path + ecosystem ---
    let deleted_dedup = conn
        .execute(
            "DELETE FROM user_dependencies WHERE rowid NOT IN (
                SELECT MAX(rowid) FROM user_dependencies
                GROUP BY LOWER(REPLACE(package_name, '-', '_')), LOWER(project_path), LOWER(ecosystem)
            )",
            [],
        )
        .expect("dedup");
    assert_eq!(
        deleted_dedup, 1,
        "Should deduplicate 1 casing/hyphen variant"
    );

    // --- Query 3: purge temp paths ---
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
