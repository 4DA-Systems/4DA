// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Wave 8a tests: ACE dependency-scan hygiene — the import-scraped
//! builtin-module purge (`purge_builtin_import_dependencies`), the stale
//! decision-window close it performs, and the orphaned-project reconcile
//! (`prune_orphaned_project_dependencies`).

use crate::test_utils::test_db;

/// Insert a user_dependencies row with raw SQL (bypasses write guards —
/// simulates LEGACY rows written before the scanner learned to skip builtins;
/// `detected_from` stays at its 'unknown' default).
fn raw_user_dep(
    db: &crate::db::Database,
    project_path: &str,
    package: &str,
    version: Option<&str>,
    eco: &str,
    is_direct: i32,
) {
    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, datetime('now'), datetime('now'))",
        rusqlite::params![project_path, package, version, eco, is_direct],
    )
    .unwrap();
}

/// Insert a user_dependencies row WITH provenance (post-migration-87 rows).
fn raw_user_dep_prov(
    db: &crate::db::Database,
    project_path: &str,
    package: &str,
    version: Option<&str>,
    eco: &str,
    is_direct: i32,
    detected_from: &str,
) {
    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_from, detected_at, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, datetime('now'), datetime('now'))",
        rusqlite::params![project_path, package, version, eco, is_direct, detected_from],
    )
    .unwrap();
}

/// Tables the purge sweeps that the unit-test Database does not migrate
/// (`project_dependencies` is ACE-side; `decision_windows` uses IF NOT EXISTS
/// so it is a no-op when the main migrations already created it).
fn create_side_tables(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS project_dependencies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_path TEXT NOT NULL,
            manifest_type TEXT NOT NULL DEFAULT 'packagejson',
            package_name TEXT NOT NULL,
            version TEXT, is_dev INTEGER DEFAULT 0, is_direct INTEGER DEFAULT 1,
            language TEXT DEFAULT 'javascript',
            last_scanned TEXT DEFAULT (datetime('now')),
            UNIQUE(project_path, package_name)
        );
        CREATE TABLE IF NOT EXISTS decision_windows (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            window_type TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            urgency REAL NOT NULL DEFAULT 0.5,
            relevance REAL NOT NULL DEFAULT 0.5,
            source_item_ids TEXT NOT NULL DEFAULT '[]',
            signal_chain_id INTEGER,
            dependency TEXT,
            status TEXT NOT NULL DEFAULT 'open',
            opened_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT, acted_at TEXT, closed_at TEXT,
            outcome TEXT, lead_time_hours REAL, streets_engine TEXT
        );",
    )
    .unwrap();
}

fn user_dep_names(db: &crate::db::Database) -> Vec<(String, String)> {
    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare("SELECT package_name, ecosystem FROM user_dependencies ORDER BY package_name")
        .unwrap();
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    rows
}

// ============================================================================
// Builtin-module purge
// ============================================================================

#[test]
fn builtin_purge_deletes_import_scraped_builtins_only() {
    let db = test_db();

    // The live-DB pollution signature: builtin name, version NULL, direct,
    // javascript ecosystem (fs/path/http et al from the import scrape).
    for name in ["fs", "path", "http", "child_process", "url"] {
        raw_user_dep(
            &db,
            "d:/dev/kairos-mvp/backend",
            name,
            None,
            "javascript",
            1,
        );
    }
    // node:-prefixed specifier persisted verbatim.
    raw_user_dep(
        &db,
        "d:/dev/kairos-mvp/backend",
        "node:fs",
        None,
        "javascript",
        1,
    );
    // Python stdlib scraped from imports.
    raw_user_dep(&db, "d:/dev/py-tool", "os", None, "python", 1);
    raw_user_dep(&db, "d:/dev/py-tool", "json", None, "python", 1);

    // MUST SURVIVE — real npm polyfill packages (versioned, from lockfiles).
    raw_user_dep(&db, "d:/dev/web", "buffer", Some("5.7.1"), "javascript", 0);
    raw_user_dep(&db, "d:/dev/web", "events", Some("3.3.0"), "javascript", 0);
    raw_user_dep(
        &db,
        "d:/dev/web",
        "string_decoder",
        Some("1.3.0"),
        "javascript",
        0,
    );
    // MUST SURVIVE — a DIRECT but VERSIONED builtin-named npm dep.
    raw_user_dep(
        &db,
        "d:/dev/web",
        "punycode",
        Some("2.3.1"),
        "javascript",
        1,
    );
    // MUST SURVIVE — the real Rust http/url CRATES (other ecosystem).
    raw_user_dep(&db, "d:/dev/rust-svc", "http", Some("1.4.0"), "rust", 1);
    raw_user_dep(&db, "d:/dev/rust-svc", "url", None, "rust", 1);
    // MUST SURVIVE — an unversioned lockfile transitive sharing a builtin name.
    raw_user_dep(&db, "d:/dev/web", "util", None, "javascript", 0);
    // MUST SURVIVE — a normal package, unversioned + direct.
    raw_user_dep(&db, "d:/dev/web", "react", None, "javascript", 1);

    let counts = {
        let conn = db.conn.lock();
        crate::db::purge_builtin_import_dependencies(&conn).unwrap()
    };
    assert_eq!(
        counts.user_dependencies, 8,
        "exactly the pollution rows purge: fs, path, http, child_process, url, node:fs, os, json"
    );

    let remaining = user_dep_names(&db);
    let names: Vec<&str> = remaining.iter().map(|(n, _)| n.as_str()).collect();
    for kept in [
        "buffer",
        "events",
        "string_decoder",
        "punycode",
        "http", // rust crate
        "url",  // rust crate
        "util", // transitive
        "react",
    ] {
        assert!(names.contains(&kept), "must survive purge: {kept}");
    }
    for gone in ["fs", "path", "child_process", "node:fs", "os", "json"] {
        assert!(!names.contains(&gone), "must be purged: {gone}");
    }
    // http/url survive ONLY as the rust rows.
    assert!(remaining
        .iter()
        .all(|(n, e)| !(n == "http" && e == "javascript") && !(n == "url" && e == "javascript")));
}

#[test]
fn builtin_purge_sweeps_project_dependencies_and_snapshots() {
    let db = test_db();
    let conn = db.conn.lock();
    create_side_tables(&conn);

    // project_dependencies would re-seed user_dependencies on the next sync
    // if left behind — it must be swept with the same signature.
    conn.execute_batch(
        "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_direct, language) VALUES
            ('d:/dev/app', 'packagejson', 'http', NULL, 1, 'javascript'),
            ('d:/dev/app', 'packagejson', 'express', NULL, 1, 'javascript'),
            ('d:/dev/svc', 'cargotoml', 'http', '1.4.0', 1, 'rust');
         INSERT INTO dependency_snapshots (project_path, package_name, ecosystem, version, is_direct) VALUES
            ('d:/dev/app', 'fs', 'javascript', NULL, 1),
            ('d:/dev/app', 'express', 'javascript', '4.19.0', 1);",
    )
    .unwrap();

    let counts = crate::db::purge_builtin_import_dependencies(&conn).unwrap();
    assert_eq!(counts.project_dependencies, 1, "js http row only");
    assert_eq!(counts.dependency_snapshots, 1, "fs snapshot only");

    let pd: i64 = conn
        .query_row("SELECT COUNT(*) FROM project_dependencies", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(pd, 2, "express + rust http survive");
    let ds: i64 = conn
        .query_row("SELECT COUNT(*) FROM dependency_snapshots", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(ds, 1, "versioned express snapshot survives");
}

#[test]
fn builtin_purge_closes_stale_window_without_surviving_versioned_dep() {
    let db = test_db();
    {
        let conn = db.conn.lock();
        create_side_tables(&conn);
        // The live incident: "Security: http" window minted from the
        // import-scraped js builtin, no real versioned dep named http.
        conn.execute(
            "INSERT INTO decision_windows (window_type, title, dependency, status)
             VALUES ('security_patch', 'Security: http', 'http', 'open')",
            [],
        )
        .unwrap();
    }
    raw_user_dep(&db, "d:/dev/app", "http", None, "javascript", 1);

    let counts = {
        let conn = db.conn.lock();
        crate::db::purge_builtin_import_dependencies(&conn).unwrap()
    };
    assert_eq!(counts.user_dependencies, 1);
    assert_eq!(counts.windows_closed, 1, "phantom window closed");

    let conn = db.conn.lock();
    let (status, outcome): (String, Option<String>) = conn
        .query_row(
            "SELECT status, outcome FROM decision_windows WHERE dependency = 'http'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, "closed");
    assert_eq!(outcome.as_deref(), Some("invalidated"));
}

#[test]
fn builtin_purge_keeps_window_backed_by_versioned_dep() {
    let db = test_db();
    {
        let conn = db.conn.lock();
        create_side_tables(&conn);
        conn.execute(
            "INSERT INTO decision_windows (window_type, title, dependency, status)
             VALUES ('security_patch', 'Security: http', 'http', 'open')",
            [],
        )
        .unwrap();
    }
    // The phantom js row AND the real rust http crate (versioned).
    raw_user_dep(&db, "d:/dev/app", "http", None, "javascript", 1);
    raw_user_dep(&db, "d:/dev/svc", "http", Some("1.4.0"), "rust", 1);

    let counts = {
        let conn = db.conn.lock();
        crate::db::purge_builtin_import_dependencies(&conn).unwrap()
    };
    assert_eq!(counts.user_dependencies, 1, "js phantom purged");
    assert_eq!(
        counts.windows_closed, 0,
        "window survives — a versioned http dep exists"
    );

    let conn = db.conn.lock();
    let status: String = conn
        .query_row(
            "SELECT status FROM decision_windows WHERE dependency = 'http'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "open");
}

#[test]
fn builtin_purge_is_idempotent() {
    let db = test_db();
    {
        let conn = db.conn.lock();
        create_side_tables(&conn);
        conn.execute(
            "INSERT INTO decision_windows (window_type, title, dependency, status)
             VALUES ('security_patch', 'Security: fs', 'fs', 'open')",
            [],
        )
        .unwrap();
    }
    raw_user_dep(&db, "d:/dev/app", "fs", None, "javascript", 1);
    raw_user_dep(&db, "d:/dev/app", "react", Some("18.2.0"), "javascript", 1);

    let conn = db.conn.lock();
    let first = crate::db::purge_builtin_import_dependencies(&conn).unwrap();
    assert_eq!(first.user_dependencies, 1);
    assert_eq!(first.windows_closed, 1);

    let second = crate::db::purge_builtin_import_dependencies(&conn).unwrap();
    assert_eq!(second.total(), 0, "second run deletes nothing");
    assert_eq!(second.windows_closed, 0, "no window re-closed");
}

// ============================================================================
// Provenance semantics (migration 87, adversarial-review findings 0/1/3/4)
// ============================================================================

/// Finding [1]: a MANIFEST-DECLARED dep sharing a builtin name (the npm
/// `buffer` polyfill declared in package.json with no lockfile → version
/// NULL, is_direct 1) must SURVIVE the purge — provenance makes it immune.
#[test]
fn builtin_purge_manifest_declared_builtin_shadow_survives() {
    let db = test_db();
    raw_user_dep_prov(
        &db,
        "d:/dev/web",
        "buffer",
        None,
        "javascript",
        1,
        "manifest",
    );
    raw_user_dep_prov(
        &db,
        "d:/dev/web",
        "events",
        None,
        "javascript",
        1,
        "manifest",
    );
    // Same names via import scrape in another project — pollution, purged.
    raw_user_dep_prov(
        &db,
        "d:/dev/api",
        "buffer",
        None,
        "javascript",
        1,
        "import_scrape",
    );
    // Arm A ignores version: an import-scraped builtin is pollution even if
    // some path stamped a version onto the row later.
    raw_user_dep_prov(
        &db,
        "d:/dev/api",
        "fs",
        Some("0.0.1"),
        "javascript",
        1,
        "import_scrape",
    );
    // Import-scraped REAL package: not a builtin, survives.
    raw_user_dep_prov(
        &db,
        "d:/dev/api",
        "react",
        None,
        "javascript",
        1,
        "import_scrape",
    );

    let counts = {
        let conn = db.conn.lock();
        crate::db::purge_builtin_import_dependencies(&conn).unwrap()
    };
    assert_eq!(
        counts.user_dependencies, 2,
        "the two import_scrape builtins"
    );

    let remaining = user_dep_names(&db);
    let names: Vec<&str> = remaining.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        names.contains(&"buffer"),
        "manifest-declared buffer polyfill survives"
    );
    assert!(
        names.contains(&"events"),
        "manifest-declared events polyfill survives"
    );
    assert!(names.contains(&"react"), "import-scraped real pkg survives");
    assert!(!names.contains(&"fs"), "import_scrape builtin purged");
    assert_eq!(
        remaining.iter().filter(|(n, _)| n == "buffer").count(),
        1,
        "only the import_scrape buffer row was deleted"
    );
}

/// Finding [4]: legacy go rows from the pre-fix import scrape ("http" from
/// net/http — stored by last path segment) are healed; full-path go module
/// rows and versioned go rows survive.
#[test]
fn builtin_purge_legacy_go_stdlib_rows() {
    let db = test_db();
    // Legacy pollution (detected_from='unknown' default).
    raw_user_dep(&db, "d:/dev/gosvc", "http", None, "go", 1);
    raw_user_dep(&db, "d:/dev/gosvc", "json", None, "go", 1);
    raw_user_dep(&db, "d:/dev/gosvc", "fmt", None, "go", 1);
    // MUST SURVIVE: full module path (dot in first segment), versioned row,
    // and a bare real-module last segment not on the stdlib list.
    raw_user_dep(
        &db,
        "d:/dev/gosvc",
        "github.com/gin-gonic/gin",
        None,
        "go",
        1,
    );
    // (different project path — UNIQUE(project_path, package_name, ecosystem))
    raw_user_dep(&db, "d:/dev/other-gosvc", "http", Some("1.0.0"), "go", 1);
    raw_user_dep(&db, "d:/dev/gosvc", "gin", None, "go", 1);

    let counts = {
        let conn = db.conn.lock();
        crate::db::purge_builtin_import_dependencies(&conn).unwrap()
    };
    assert_eq!(counts.user_dependencies, 3, "http/json/fmt legacy go rows");

    let remaining = user_dep_names(&db);
    let names: Vec<&str> = remaining.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"github.com/gin-gonic/gin"));
    assert!(names.contains(&"gin"));
    assert!(names.contains(&"http"), "versioned go http row survives");
    assert!(!names.contains(&"json"));
    assert!(!names.contains(&"fmt"));
}

/// Finding [0]: a window backed by a real-but-UNVERSIONED row the purge
/// KEEPS (rust `url` crate from a manifest-only scan) must stay open — the
/// old survivor query required `version IS NOT NULL` and invalidated it.
#[test]
fn builtin_purge_keeps_window_backed_by_unversioned_kept_row() {
    let db = test_db();
    {
        let conn = db.conn.lock();
        create_side_tables(&conn);
        conn.execute(
            "INSERT INTO decision_windows (window_type, title, dependency, status)
             VALUES ('security_patch', 'Security: url', 'url', 'open')",
            [],
        )
        .unwrap();
    }
    // The js phantom (purged) and the real rust url CRATE with NULL version
    // (manifest-only scan — no Cargo.lock processed yet). The rust row is
    // KEPT by the purge, so it must protect the window.
    raw_user_dep(&db, "d:/dev/app", "url", None, "javascript", 1);
    raw_user_dep_prov(&db, "d:/dev/svc", "url", None, "rust", 1, "manifest");

    let counts = {
        let conn = db.conn.lock();
        crate::db::purge_builtin_import_dependencies(&conn).unwrap()
    };
    assert_eq!(counts.user_dependencies, 1, "only the js phantom purged");
    assert_eq!(
        counts.windows_closed, 0,
        "window survives — a kept (unversioned rust) url row exists"
    );

    let conn = db.conn.lock();
    let status: String = conn
        .query_row(
            "SELECT status FROM decision_windows WHERE dependency = 'url'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "open");
}

/// Finding [3]: the manifest-scan sync is authoritative for is_direct in
/// BOTH directions — a row stored direct is downgraded when the manifest now
/// marks it `// indirect`, and vice versa.
#[test]
fn store_manifest_dependency_is_authoritative_for_is_direct() {
    let db = test_db();
    let proj = "d:/dev/gosvc";

    // Pre-fix state: go indirect module wrongly stored direct.
    raw_user_dep(&db, proj, "golang.org/x/text", None, "go", 1);
    // Sync now says: indirect. Must downgrade (old path preserved is_direct).
    db.store_manifest_dependency(
        proj,
        "golang.org/x/text",
        None,
        "go",
        false,
        false,
        "manifest",
    )
    .unwrap();
    // And upgrade works too.
    raw_user_dep(&db, proj, "github.com/spf13/cobra", None, "go", 0);
    db.store_manifest_dependency(
        proj,
        "github.com/spf13/cobra",
        None,
        "go",
        false,
        true,
        "manifest",
    )
    .unwrap();

    let conn = db.conn.lock();
    let direct_of = |name: &str| -> i64 {
        conn.query_row(
            "SELECT is_direct FROM user_dependencies WHERE package_name = ?1",
            rusqlite::params![name],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(direct_of("golang.org/x/text"), 0, "downgraded to indirect");
    assert_eq!(direct_of("github.com/spf13/cobra"), 1, "upgraded to direct");
    let prov: String = conn
        .query_row(
            "SELECT detected_from FROM user_dependencies WHERE package_name = 'golang.org/x/text'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(prov, "manifest", "provenance propagated by the sync");
}

// ============================================================================
// Orphaned-project reconcile
// ============================================================================

#[test]
fn orphan_prune_removes_deleted_project_rows_keeps_existing() {
    let db = test_db();
    let conn = db.conn.lock();
    create_side_tables(&conn);

    conn.execute_batch(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_direct) VALUES
            ('d:/dev/deleted-app', 'react', '18.2.0', 'javascript', 1),
            ('d:/dev/deleted-app', 'axios', '1.6.0', 'javascript', 1),
            ('d:/dev/alive-app', 'serde', '1.0.0', 'rust', 1);
         INSERT INTO project_dependencies (project_path, manifest_type, package_name, language) VALUES
            ('d:/dev/deleted-app', 'packagejson', 'react', 'javascript'),
            ('d:/dev/alive-app', 'cargotoml', 'serde', 'rust');
         INSERT INTO dependency_snapshots (project_path, package_name, ecosystem) VALUES
            ('d:/dev/deleted-app', 'react', 'javascript'),
            ('d:/dev/alive-app', 'serde', 'rust');",
    )
    .unwrap();

    let missing = |p: &str| p.contains("deleted-app");
    let counts = crate::db::prune_orphaned_project_dependencies(&conn, &missing).unwrap();
    assert_eq!(counts.orphaned_paths, 1);
    assert_eq!(counts.user_dependencies, 2);
    assert_eq!(counts.project_dependencies, 1);
    assert_eq!(counts.dependency_snapshots, 1);

    let remaining_paths: Vec<String> = conn
        .prepare("SELECT DISTINCT project_path FROM user_dependencies")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert_eq!(remaining_paths, vec!["d:/dev/alive-app".to_string()]);

    // Idempotent: nothing left to prune.
    let again = crate::db::prune_orphaned_project_dependencies(&conn, &missing).unwrap();
    assert_eq!(again.orphaned_paths, 0);
    assert_eq!(again.total(), 0);
}

#[test]
fn orphan_prune_never_touches_existing_paths() {
    let db = test_db();
    let conn = db.conn.lock();
    create_side_tables(&conn);
    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem) VALUES
            ('d:/dev/alive', 'tokio', '1.38.0', 'rust')",
        [],
    )
    .unwrap();

    // Nothing is missing on disk.
    let counts = crate::db::prune_orphaned_project_dependencies(&conn, &|_| false).unwrap();
    assert_eq!(counts.orphaned_paths, 0);
    assert_eq!(counts.total(), 0);
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM user_dependencies", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
}

#[test]
fn missing_on_disk_probe_is_conservative() {
    use crate::db::project_path_missing_on_disk;

    // Existing path: not missing (the repo root of this test process).
    let cwd = std::env::current_dir().unwrap();
    assert!(!project_path_missing_on_disk(&cwd.to_string_lossy()));

    // Empty / relative / UNC paths: never missing.
    assert!(!project_path_missing_on_disk(""));
    assert!(!project_path_missing_on_disk("relative/proj"));
    assert!(!project_path_missing_on_disk(r"\\server\share\proj"));
    assert!(!project_path_missing_on_disk("//server/share/proj"));

    // A genuinely deleted path under an EXISTING root is missing.
    let ghost = cwd.join("definitely-not-a-real-project-4da-wave8a");
    assert!(project_path_missing_on_disk(&ghost.to_string_lossy()));

    // Unplugged-volume guard (Windows): a path whose drive root is gone is
    // NOT missing. Probe for a drive letter that doesn't exist.
    #[cfg(windows)]
    {
        for letter in ['q', 'w', 'y'] {
            let root = format!("{letter}:/");
            if !std::path::Path::new(&root).exists() {
                assert!(
                    !project_path_missing_on_disk(&format!("{letter}:/dev/project")),
                    "offline volume must not count as deleted"
                );
                break;
            }
        }
    }
}
