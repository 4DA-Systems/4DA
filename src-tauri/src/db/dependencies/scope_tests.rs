// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the active-root scope on the `user_dependencies` readers, the
//! stale-row prune, and `dependency_alerts.project_path` (2026-09-04 ACE
//! input-hygiene fix).

use crate::db::dependencies::types::DependencyAlert;
use crate::test_utils::test_db;

/// `git_signals` is an ACE table (`ace::db::migrate`), not part of the main
/// schema `test_db()` runs, so each test creates it.
fn with_git_signals(db: &crate::db::Database) {
    let conn = db.conn.lock();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS git_signals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            repo_path TEXT NOT NULL,
            commit_hash TEXT,
            timestamp TEXT DEFAULT (datetime('now'))
        );",
    )
    .unwrap();
}

fn active_root(db: &crate::db::Database, repo: &str) {
    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO git_signals (repo_path, commit_hash) VALUES (?1, 'deadbeef')",
        [repo],
    )
    .unwrap();
}

fn names(deps: &[crate::db::StoredDependency]) -> Vec<&str> {
    let mut v: Vec<&str> = deps.iter().map(|d| d.package_name.as_str()).collect();
    v.sort_unstable();
    v
}

#[test]
fn auditable_and_relevant_readers_are_scoped_to_active_repo_roots() {
    let db = test_db();
    with_git_signals(&db);
    active_root(&db, "/projects/myapp");
    // Active project: direct + transitive.
    db.store_dependency(
        "/projects/myapp",
        "tokio",
        Some("1.35.0"),
        "rust",
        false,
        None,
    )
    .unwrap();
    db.store_transitive_dependency("/projects/myapp", "mio", Some("1.0.2"), "rust", false)
        .unwrap();
    // A project with NO commit in 60 days (no git_signals root at all): the
    // live case was a nested third-party clone contributing 1,811 rows and
    // every rkyv advisory.
    db.store_dependency(
        "/projects/dormant",
        "rkyv",
        Some("0.7.42"),
        "rust",
        false,
        None,
    )
    .unwrap();
    db.store_transitive_dependency(
        "/projects/dormant",
        "bytecheck",
        Some("0.6.11"),
        "rust",
        false,
    )
    .unwrap();

    let auditable = db.get_auditable_user_dependencies().unwrap();
    assert_eq!(names(&auditable), vec!["mio", "tokio"]);
    let relevant = db.get_relevant_user_dependencies().unwrap();
    assert_eq!(names(&relevant), vec!["tokio"]);
}

#[test]
fn readers_fall_back_to_every_row_when_no_active_root_exists() {
    let db = test_db();
    with_git_signals(&db);
    // Table present but empty (first run: git analysis has not written yet).
    db.store_dependency("/projects/a", "tokio", Some("1.35.0"), "rust", false, None)
        .unwrap();
    db.store_dependency(
        "/projects/b",
        "react",
        Some("19.0.0"),
        "javascript",
        false,
        None,
    )
    .unwrap();
    assert_eq!(
        names(&db.get_auditable_user_dependencies().unwrap()),
        vec!["react", "tokio"],
        "first run: unscoped (and logged as dep_scope_degraded), never empty"
    );
    assert_eq!(
        names(&db.get_relevant_user_dependencies().unwrap()),
        vec!["react", "tokio"]
    );
}

#[test]
fn readers_tolerate_a_missing_git_signals_table() {
    // Pre-ACE-migration database: the scope query cannot run; fall back.
    let db = test_db();
    db.store_dependency("/projects/a", "tokio", Some("1.35.0"), "rust", false, None)
        .unwrap();
    assert_eq!(
        names(&db.get_auditable_user_dependencies().unwrap()),
        vec!["tokio"]
    );
}

#[test]
fn stale_rows_are_pruned_when_absent_from_lockfile_and_manifest() {
    let db = test_db();
    // Lowercase, forward-slash: the canonical storage form on every platform
    // (Windows lowercases; Linux preserves case), so the manual
    // project_dependencies insert below keys identically.
    let project = "/projects/app";
    // Lockfile rows from a previous scan.
    db.store_transitive_dependency(project, "old_transitive", Some("0.1.0"), "rust", false)
        .unwrap();
    db.store_transitive_dependency(project, "mio", Some("1.0.2"), "rust", false)
        .unwrap();
    db.store_dependency(project, "tokio", Some("1.35.0"), "rust", false, None)
        .unwrap();
    // A manifest-declared dep the lockfile does not list (e.g. an
    // import-scraped or version-less manifest row synced from
    // project_dependencies) — declared there, so it must survive.
    db.store_manifest_dependency(
        project,
        "declared_only",
        None,
        "rust",
        false,
        true,
        "manifest",
    )
    .unwrap();
    {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, language)
             VALUES ('/projects/app', 'cargotoml', 'declared_only', 'rust')",
            [],
        )
        .unwrap();
    }
    // Another ecosystem in the same project must be untouched.
    db.store_dependency(project, "react", Some("19.0.0"), "javascript", false, None)
        .unwrap();

    let current = vec!["Tokio".to_string(), "mio".to_string()];
    let removed = db
        .prune_stale_user_dependencies(project, "rust", &current)
        .unwrap();
    assert_eq!(
        removed, 1,
        "only the package the lockfile no longer resolves"
    );

    let remaining = db.get_project_dependencies(project).unwrap();
    assert_eq!(
        names(&remaining),
        vec!["declared_only", "mio", "react", "tokio"]
    );
}

#[test]
fn prune_is_a_noop_on_an_empty_lockfile_list() {
    let db = test_db();
    db.store_dependency("/p", "tokio", Some("1.0.0"), "rust", false, None)
        .unwrap();
    assert_eq!(
        db.prune_stale_user_dependencies("/p", "rust", &[]).unwrap(),
        0
    );
    assert_eq!(db.get_project_dependencies("/p").unwrap().len(), 1);
}

fn alert(pkg: &str, project_path: Option<&str>) -> DependencyAlert {
    DependencyAlert {
        id: 0,
        package_name: pkg.to_string(),
        ecosystem: "rust".to_string(),
        alert_type: "audit".to_string(),
        severity: "high".to_string(),
        title: format!("RUSTSEC-2026-0001: {pkg}"),
        description: None,
        affected_versions: None,
        source_url: None,
        source_item_id: None,
        detected_at: String::new(),
        resolved_at: None,
        project_path: project_path.map(String::from),
    }
}

#[test]
fn alerts_record_the_audited_project_path_and_backfill_it() {
    let db = test_db();
    let id = db.store_dependency_alert(&alert("rkyv", None)).unwrap();
    assert!(id > 0);
    assert_eq!(db.get_active_alerts().unwrap()[0].project_path, None);

    // The same advisory reported again WITH a path: the existing row learns
    // it (no duplicate row).
    assert_eq!(
        db.store_dependency_alert(&alert("rkyv", Some("d:/proj/app")))
            .unwrap(),
        0
    );
    let active = db.get_active_alerts().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].project_path.as_deref(), Some("d:/proj/app"));

    // A fresh alert stores its path on insert.
    db.store_dependency_alert(&alert("bytes", Some("d:/other")))
        .unwrap();
    let bytes = db
        .get_active_alerts()
        .unwrap()
        .into_iter()
        .find(|a| a.package_name == "bytes")
        .unwrap();
    assert_eq!(bytes.project_path.as_deref(), Some("d:/other"));
}
