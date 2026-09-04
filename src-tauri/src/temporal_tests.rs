// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `temporal` — extracted from temporal.rs so the production module
//! stays under the Rust file-size ceiling (loaded via #[path], the
//! dependency_health_tests.rs precedent).

use super::*;
use rusqlite::Connection;

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS temporal_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL,
            subject TEXT NOT NULL,
            data JSON NOT NULL,
            embedding BLOB,
            source_item_id INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT
        );",
    )
    .expect("create tables");
    conn
}

#[test]
fn record_and_query_event_roundtrip() {
    let conn = setup_test_db();
    let data = serde_json::json!({"key": "value", "count": 42});
    let id = record_event(&conn, "test_type", "test_subject", &data, Some(100), None).unwrap();
    assert!(id > 0);

    let events = query_events(&conn, "test_type", None, 10).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, id);
    assert_eq!(events[0].event_type, "test_type");
    assert_eq!(events[0].subject, "test_subject");
    assert_eq!(events[0].source_item_id, Some(100));
    assert_eq!(events[0].data["key"], "value");
    assert_eq!(events[0].data["count"], 42);
}

#[test]
fn query_events_respects_limit() {
    let conn = setup_test_db();
    let data = serde_json::json!({});
    for i in 0..5 {
        record_event(&conn, "bulk", &format!("subj_{}", i), &data, None, None).unwrap();
    }
    let events = query_events(&conn, "bulk", None, 3).unwrap();
    assert_eq!(events.len(), 3);
}

#[test]
fn query_events_filters_by_type() {
    let conn = setup_test_db();
    let data = serde_json::json!({});
    record_event(&conn, "type_a", "s1", &data, None, None).unwrap();
    record_event(&conn, "type_b", "s2", &data, None, None).unwrap();
    record_event(&conn, "type_a", "s3", &data, None, None).unwrap();

    let a_events = query_events(&conn, "type_a", None, 10).unwrap();
    assert_eq!(a_events.len(), 2);
    let b_events = query_events(&conn, "type_b", None, 10).unwrap();
    assert_eq!(b_events.len(), 1);
}

#[test]
fn temporal_event_serde_roundtrip() {
    let event = TemporalEvent {
        id: 1,
        event_type: "version_release".to_string(),
        subject: "react".to_string(),
        data: serde_json::json!({"version": "19.0.0", "breaking": true}),
        source_item_id: Some(42),
        created_at: "2026-02-28T10:00:00".to_string(),
        expires_at: Some("2026-03-28T10:00:00".to_string()),
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: TemporalEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.event_type, "version_release");
    assert_eq!(deserialized.data["version"], "19.0.0");
    assert_eq!(deserialized.source_item_id, Some(42));
    assert!(deserialized.expires_at.is_some());
}

fn setup_deps_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE project_dependencies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_path TEXT NOT NULL,
            manifest_type TEXT NOT NULL DEFAULT 'cargotoml',
            package_name TEXT NOT NULL,
            version TEXT,
            is_dev INTEGER DEFAULT 0,
            is_direct INTEGER DEFAULT 1,
            language TEXT NOT NULL DEFAULT 'rust',
            last_scanned TEXT NOT NULL DEFAULT (datetime('now')),
            project_relevance REAL DEFAULT 1.0,
            target_cfg TEXT,
            platform_active INTEGER DEFAULT 1,
            detected_from TEXT NOT NULL DEFAULT 'unknown',
            UNIQUE(project_path, package_name)
        );",
    )
    .unwrap();
    conn
}

#[test]
fn prune_removes_dropped_deps_direct_and_indirect() {
    let conn = setup_deps_db();
    let proj = "D:/proj/app";
    // Two current direct deps + one stale (removed) direct dep.
    for (name, keep) in [("serde", true), ("tokio", true), ("removed_crate", false)] {
        let _ = keep;
        upsert_dependency(
            &conn,
            proj,
            "cargotoml",
            name,
            None,
            false,
            true,
            "rust",
            1.0,
            "manifest",
        )
        .unwrap();
    }
    // A manifest-indirect dep still IN the manifest (kept via
    // current_names) and one REMOVED from the manifest. Every
    // project_dependencies row is manifest-sourced, so indirect rows
    // absent from the latest scan are pruned too — the old
    // `is_direct = 1` scope left them immortal.
    upsert_manifest_indirect_dependency(&conn, proj, "gomod", "kept_indirect", "rust", 1.0)
        .unwrap();
    upsert_manifest_indirect_dependency(&conn, proj, "gomod", "removed_indirect", "rust", 1.0)
        .unwrap();
    // A different-language direct dep must NOT be pruned by a rust scan.
    conn.execute(
        "INSERT INTO project_dependencies (project_path, manifest_type, package_name, language, is_direct)
         VALUES (?1, 'packagejson', 'react', 'javascript', 1)",
        params![canonicalize_project_path(proj)],
    )
    .unwrap();

    let current = vec![
        "serde".to_string(),
        "tokio".to_string(),
        "kept_indirect".to_string(),
    ];
    let removed = prune_removed_dependencies(&conn, proj, "rust", &current).unwrap();
    assert_eq!(
        removed, 2,
        "dropped direct dep AND dropped indirect dep are removed"
    );

    let names: Vec<String> = conn
        .prepare("SELECT package_name FROM project_dependencies ORDER BY package_name")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(names.contains(&"serde".to_string()));
    assert!(names.contains(&"tokio".to_string()));
    assert!(!names.contains(&"removed_crate".to_string()));
    assert!(
        names.contains(&"kept_indirect".to_string()),
        "indirect dep still in the manifest must survive"
    );
    assert!(
        !names.contains(&"removed_indirect".to_string()),
        "indirect dep removed from the manifest must be pruned"
    );
    assert!(
        names.contains(&"react".to_string()),
        "other-language dep must survive"
    );
}

#[test]
fn prune_is_noop_on_empty_keep_list() {
    let conn = setup_deps_db();
    let proj = "D:/proj/app";
    upsert_dependency(
        &conn,
        proj,
        "cargotoml",
        "serde",
        None,
        false,
        true,
        "rust",
        1.0,
        "manifest",
    )
    .unwrap();
    // An empty keep-list must NOT wipe deps (guards against a parse failure
    // deleting a whole project's dependency set).
    let removed = prune_removed_dependencies(&conn, proj, "rust", &[]).unwrap();
    assert_eq!(removed, 0);
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM project_dependencies", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn record_event_with_null_source_item_id() {
    let conn = setup_test_db();
    let data = serde_json::json!({"note": "no source"});
    let id = record_event(&conn, "manual", "user_action", &data, None, None).unwrap();
    let events = query_events(&conn, "manual", None, 10).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, id);
    assert_eq!(events[0].source_item_id, None);
}

#[test]
fn test_canonicalize_windows_paths() {
    let result = canonicalize_project_path(r"D:\Users\Admin\Documents\my-project");
    if cfg!(windows) {
        assert_eq!(result, "d:/users/admin/documents/my-project");
    } else {
        assert_eq!(result, "D:/Users/Admin/Documents/my-project");
    }
}

#[test]
fn test_canonicalize_merges_case_variants() {
    let a = canonicalize_project_path(r"C:\Users\Dev\Documents\kairos-mvp");
    let b = canonicalize_project_path(r"C:\Users\Dev\documents\kairos-mvp");
    if cfg!(windows) {
        assert_eq!(a, b, "Case variants should canonicalize to the same key");
    }
}

#[test]
fn test_canonicalize_forward_slashes() {
    let result = canonicalize_project_path("C:/Users/Dev/project");
    if cfg!(windows) {
        assert_eq!(result, "c:/users/dev/project");
    } else {
        assert_eq!(result, "C:/Users/Dev/project");
    }
}

// ── Dependency scope filter (2026-08-26 audit, R2) ──────────────────
//
// The bug these pin: `git_signals.repo_path` is stored RAW (`D:\\4DA`)
// while `project_dependencies.project_path` is canonicalized
// (`d:/4da/src-tauri`). The old filter lowercased both but compared them
// with raw `starts_with`, so on Windows it matched 0 of 245 rows on every
// run — and `filtered.is_empty()` then re-admitted everything, making a
// dead filter indistinguishable from a correctly-empty one.

#[test]
fn backslash_root_matches_forward_slash_dep_path() {
    // THE regression. Fails against the pre-audit implementation.
    let roots = vec!["D:\\4DA".to_string()];
    assert!(dep_within_active_root("d:/4da/src-tauri", &roots));
    assert!(dep_within_active_root("d:/4da", &roots));
    assert!(dep_within_active_root("D:\\4DA\\relay", &roots));
}

#[test]
fn foreign_project_is_excluded() {
    let roots = vec!["D:\\4DA".to_string()];
    assert!(!dep_within_active_root(
        "c:/users/administrator/documents/kairos-mvp/backend",
        &roots
    ));
}

#[test]
fn sibling_prefix_is_not_a_match() {
    // Path-BOUNDARY matching: `d:/4da` must never swallow `d:/4da-experiments`.
    let roots = vec!["D:\\4DA".to_string()];
    assert!(!dep_within_active_root("d:/4da-experiments", &roots));
    assert!(!dep_within_active_root("d:/4dafoo/src", &roots));
}

#[test]
fn root_recorded_deeper_than_the_manifest_still_matches() {
    // Reverse containment: git root deeper than the dep's project path.
    let roots = vec!["D:\\4DA\\src-tauri".to_string()];
    assert!(dep_within_active_root("d:/4da", &roots));
}

#[test]
fn case_and_trailing_slash_are_normalized() {
    let roots = vec!["d:/4DA/".to_string()];
    assert!(dep_within_active_root("D:/4da/site", &roots));
}

#[test]
fn empty_inputs_never_match() {
    assert!(!dep_within_active_root("", &["D:\\4DA".to_string()]));
    assert!(!dep_within_active_root("d:/4da", &[String::new()]));
    assert!(!dep_within_active_root("d:/4da", &[]));
}

// ── Version resolution scope (2026-09-04 audit) ─────────────────────
//
// The read-time version join matched on package NAME alone: 40 of 184
// d:/4da deps took their version from a foreign lockfile (vite 7.2.2 from
// navcal over 4DA's own 8.1.3; the Rust `jsonwebtoken` crate took kairos's
// npm 9.0.2). Resolution is now same-project + same-ecosystem first, then
// same active repo root, never wider.

fn setup_resolution_db() -> Connection {
    let conn = setup_deps_db();
    conn.execute_batch(
        "CREATE TABLE user_dependencies (
            id INTEGER PRIMARY KEY,
            project_path TEXT NOT NULL,
            package_name TEXT NOT NULL,
            version TEXT,
            ecosystem TEXT NOT NULL,
            is_dev INTEGER DEFAULT 0,
            is_direct INTEGER DEFAULT 1,
            detected_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(project_path, package_name, ecosystem)
        );
        CREATE TABLE git_signals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            repo_path TEXT NOT NULL,
            commit_hash TEXT,
            timestamp TEXT DEFAULT (datetime('now'))
        );",
    )
    .unwrap();
    conn
}

fn manifest_dep(conn: &Connection, project: &str, name: &str, language: &str) {
    conn.execute(
        "INSERT INTO project_dependencies (project_path, manifest_type, package_name, language)
         VALUES (?1, 'x', ?2, ?3)",
        params![project, name, language],
    )
    .unwrap();
}

fn lockfile_dep(conn: &Connection, project: &str, name: &str, version: &str, eco: &str) {
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem)
         VALUES (?1, ?2, ?3, ?4)",
        params![project, name, version, eco],
    )
    .unwrap();
}

fn active_root(conn: &Connection, repo: &str) {
    conn.execute(
        "INSERT INTO git_signals (repo_path, commit_hash) VALUES (?1, 'abc123')",
        params![repo],
    )
    .unwrap();
}

fn version_of(deps: &[ProjectDependency], project: &str, name: &str) -> Option<String> {
    deps.iter()
        .find(|d| d.project_path == project && d.package_name == name)
        .and_then(|d| d.version.clone())
}

#[test]
fn version_resolves_from_the_same_project_not_a_foreign_lockfile() {
    let conn = setup_resolution_db();
    active_root(&conn, "D:\\4DA");
    active_root(&conn, "D:\\navcal");
    manifest_dep(&conn, "d:/4da", "vite", "javascript");
    manifest_dep(&conn, "d:/navcal", "vite", "javascript");
    // navcal's row is DIRECT and newer — the old name-only join picked it
    // for BOTH projects.
    lockfile_dep(&conn, "d:/4da", "vite", "8.1.3", "javascript");
    lockfile_dep(&conn, "d:/navcal", "vite", "7.2.2", "javascript");

    let deps = get_all_dependencies(&conn).unwrap();
    assert_eq!(
        version_of(&deps, "d:/4da", "vite").as_deref(),
        Some("8.1.3")
    );
    assert_eq!(
        version_of(&deps, "d:/navcal", "vite").as_deref(),
        Some("7.2.2")
    );
}

#[test]
fn version_never_crosses_ecosystems_or_roots() {
    let conn = setup_resolution_db();
    active_root(&conn, "D:\\4DA");
    active_root(&conn, "C:\\Users\\dev\\kairos");
    // The Rust jsonwebtoken CRATE in src-tauri; the only versioned row is
    // the same-named npm PACKAGE in another project.
    manifest_dep(&conn, "d:/4da/src-tauri", "jsonwebtoken", "rust");
    lockfile_dep(
        &conn,
        "c:/users/dev/kairos",
        "jsonwebtoken",
        "9.0.2",
        "javascript",
    );
    // Same ecosystem, but a different active root: still not ours.
    manifest_dep(&conn, "d:/4da/relay", "axum", "rust");
    lockfile_dep(&conn, "c:/users/dev/kairos", "axum", "0.7.9", "rust");

    let deps = get_all_dependencies(&conn).unwrap();
    assert_eq!(
        version_of(&deps, "d:/4da/src-tauri", "jsonwebtoken"),
        None,
        "an npm package must never version a Rust crate"
    );
    assert_eq!(
        version_of(&deps, "d:/4da/relay", "axum"),
        None,
        "a lockfile under another repo root must never version this one"
    );
}

#[test]
fn version_resolves_from_a_workspace_lockfile_under_the_same_root() {
    let conn = setup_resolution_db();
    active_root(&conn, "D:\\4DA");
    // Workspace member manifest; the resolved version lives in the
    // workspace-root Cargo.lock (a different project_path, same root).
    manifest_dep(&conn, "d:/4da/crates/member", "serde", "rust");
    lockfile_dep(&conn, "d:/4da", "serde", "1.0.200", "rust");
    // Same root, WRONG ecosystem family: an npm `serde` must not win.
    lockfile_dep(&conn, "d:/4da/site", "serde", "0.0.1", "npm");

    let deps = get_all_dependencies(&conn).unwrap();
    assert_eq!(
        version_of(&deps, "d:/4da/crates/member", "serde").as_deref(),
        Some("1.0.200")
    );
}

#[test]
fn ecosystem_alias_labels_resolve_to_their_family() {
    // `npm` (older user_dependencies rows) and `javascript` (the manifest
    // language) are one family; the SQL rendering must agree.
    let conn = setup_resolution_db();
    active_root(&conn, "D:\\4DA");
    manifest_dep(&conn, "d:/4da", "react", "javascript");
    lockfile_dep(&conn, "d:/4da", "react", "19.1.0", "npm");
    let deps = get_all_dependencies(&conn).unwrap();
    assert_eq!(
        version_of(&deps, "d:/4da", "react").as_deref(),
        Some("19.1.0")
    );
}

/// The alias table must never drift from the canonical recognizer.
#[test]
fn ecosystem_family_agrees_with_the_canonical_ecosystem_parser() {
    use crate::ecosystem::Ecosystem;
    for (alias, family) in ECOSYSTEM_FAMILY_ALIASES {
        assert_eq!(
            Ecosystem::parse(alias),
            Ecosystem::parse(family),
            "{alias} -> {family}"
        );
        assert!(
            Ecosystem::parse(alias).is_some(),
            "{alias} is a known alias"
        );
        assert_eq!(ecosystem_family(alias), *family);
    }
    assert_eq!(ecosystem_family("Rust"), "rust");
    assert_eq!(
        ecosystem_family("elixir"),
        "elixir",
        "unknown labels are their own family"
    );
}
