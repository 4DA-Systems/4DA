// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Wave 6 tests: canonical project-inclusion policy — tier-2 scaffolding
//! write guards, F3 dedup-key fidelity (platform-conditional path case,
//! ecosystem-restricted hyphen fold), and the purge_non_project_intelligence
//! self-heal across every path-keyed intelligence table.

use crate::test_utils::test_db;

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

// ============================================================================
// Wave 6: canonical project-inclusion policy
// (tier-2 scaffolding write guards, F3 dedup-key fidelity, and the
// purge_non_project_intelligence self-heal across every path-keyed table)
// ============================================================================

#[test]
fn store_dependency_rejects_tier2_scaffolding_paths() {
    let db = test_db();

    // Fixture trees and registry-squat placeholders, both slash styles.
    let excluded = [
        "/home/u/repo/fixtures/fake-app",
        r"D:\ledger\test-fixtures\rails-app",
        "/home/u/go/pkg/testdata/mod",
        r"D:\4DA\crates-placeholder",
        "d:/4da/npm-placeholder",
    ];
    for path in excluded {
        db.store_dependency(path, "left-pad", Some("1.3.0"), "npm", false, None)
            .unwrap();
        db.store_transitive_dependency(path, "chalk", Some("5.0.0"), "npm", false)
            .unwrap();
        assert!(
            db.get_project_dependencies(path).unwrap().is_empty(),
            "tier-2 scaffolding path must not be stored: {path}"
        );
    }

    // Segment-based only: name substrings are real projects and must store.
    let kept = ["/home/u/myfixtures-app", r"D:\work\placeholderify"];
    for path in kept {
        db.store_dependency(path, "serde", Some("1.0.0"), "rust", false, None)
            .unwrap();
        assert_eq!(
            db.get_project_dependencies(path).unwrap().len(),
            1,
            "segment-lookalike real project must be stored: {path}"
        );
    }
}

#[test]
fn purge_dedup_does_not_fold_hyphen_underscore_for_npm() {
    let db = test_db();

    // npm treats left-pad and left_pad as DIFFERENT packages — the
    // hyphen->underscore fold must not collapse them (F3).
    raw_insert_user_dep(&db, "/projects/web", "left-pad", "npm");
    raw_insert_user_dep(&db, "/projects/web", "left_pad", "npm");
    // crates.io treats serde-json and serde_json as the SAME package — fold.
    raw_insert_user_dep(&db, "/projects/api", "serde-json", "rust");
    raw_insert_user_dep(&db, "/projects/api", "serde_json", "rust");

    let counts = {
        let conn = db.conn.lock();
        crate::db::purge_agent_infra_dependencies(&conn).unwrap()
    };
    assert_eq!(
        counts.duplicates, 1,
        "only the rust hyphen/underscore pair collapses"
    );

    let remaining = db.get_all_user_dependencies().unwrap();
    let npm: Vec<_> = remaining.iter().filter(|d| d.ecosystem == "npm").collect();
    assert_eq!(npm.len(), 2, "both distinct npm packages survive");
    let rust: Vec<_> = remaining.iter().filter(|d| d.ecosystem == "rust").collect();
    assert_eq!(rust.len(), 1, "rust name variants collapsed to one");
}

#[test]
fn purge_dedup_path_case_folding_matches_platform_canonical_form() {
    let db = test_db();

    // Same package, two path-CASE variants (same slash style). On Windows the
    // canonical form lowercases (case-insensitive fs) so they collapse; on
    // Linux /home/u/App and /home/u/app are genuinely different projects and
    // MUST both survive (F3 — the old unconditional LOWER() collapsed them).
    raw_insert_user_dep(&db, "/home/u/App", "express", "npm");
    raw_insert_user_dep(&db, "/home/u/app", "express", "npm");

    let counts = {
        let conn = db.conn.lock();
        crate::db::purge_agent_infra_dependencies(&conn).unwrap()
    };
    let remaining = db.get_all_user_dependencies().unwrap();
    if cfg!(windows) {
        assert_eq!(counts.duplicates, 1, "case variants collapse on Windows");
        assert_eq!(remaining.len(), 1);
    } else {
        assert_eq!(
            counts.duplicates, 0,
            "distinct case-sensitive projects must not collapse on Linux"
        );
        assert_eq!(remaining.len(), 2);
    }
}

/// Create the ACE-side tables the non-project purge sweeps (they live in the
/// same 4da.db in production but are created by the ACE migration, which the
/// unit-test Database does not run).
fn create_ace_tables(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS detected_projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            languages TEXT, frameworks TEXT, dependencies TEXT,
            last_activity TEXT, detection_confidence REAL DEFAULT 0.5,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS detected_tech (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            category TEXT NOT NULL,
            confidence REAL DEFAULT 0.5,
            source TEXT NOT NULL,
            evidence TEXT
        );
        CREATE TABLE IF NOT EXISTS project_dependencies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_path TEXT NOT NULL,
            manifest_type TEXT NOT NULL DEFAULT 'cargotoml',
            package_name TEXT NOT NULL,
            version TEXT, is_dev INTEGER DEFAULT 0, is_direct INTEGER DEFAULT 1,
            language TEXT DEFAULT 'rust',
            last_scanned TEXT DEFAULT (datetime('now')),
            UNIQUE(project_path, package_name)
        );",
    )
    .unwrap();
}

#[test]
fn purge_non_project_intelligence_clears_fixtures_and_placeholders_keeps_real() {
    let db = test_db();
    let conn = db.conn.lock();
    create_ace_tables(&conn);

    // The five live-DB fixture rows this purge exists to kill (tier 1 via the
    // .claude segment), plus tier-2 scaffolding, plus real projects.
    for (path, name) in [
        (
            r"D:\4DA\.claude\plans\ledger-fixtures\csharp-service",
            "csharp-service",
        ),
        (
            r"D:\4DA\.claude\plans\ledger-fixtures\flutter-app",
            "flutter-app",
        ),
        (
            r"D:\4DA\.claude\plans\ledger-fixtures\java-service",
            "java-service",
        ),
        (
            r"D:\4DA\.claude\plans\ledger-fixtures\php-laravel-app",
            "php-laravel-app",
        ),
        (
            r"D:\4DA\.claude\plans\ledger-fixtures\ruby-rails-app",
            "ruby-rails-app",
        ),
        (r"D:\4DA\crates-placeholder", "crates-placeholder"),
        ("d:/4da/npm-placeholder", "npm-placeholder"),
        (r"D:\4DA", "4DA"),
        ("/home/u/dev/real-app", "real-app"),
    ] {
        conn.execute(
            "INSERT INTO detected_projects (path, name) VALUES (?1, ?2)",
            rusqlite::params![path, name],
        )
        .unwrap();
    }

    // project_dependencies pollution from the placeholders + a real row.
    // (manifest_type/language are NOT NULL in the real migrated schema.)
    conn.execute_batch(
        "INSERT INTO project_dependencies (project_path, manifest_type, package_name, language) VALUES
            ('d:/4da/crates-placeholder', 'cargotoml', 'clap', 'rust'),
            ('d:/4da/npm-placeholder', 'packagejson', 'chalk', 'javascript'),
            ('d:/4da', 'cargotoml', 'tauri', 'rust');",
    )
    .unwrap();

    // detected_tech: one row evidenced ONLY by scaffolding (delete), one with
    // mixed evidence (rewrite, keep the real entry), one clean (untouched).
    conn.execute_batch(
        r"INSERT INTO detected_tech (name, category, source, evidence) VALUES
            ('dart', 'language', 'manifest',
             'Found in D:\4DA\.claude\plans\ledger-fixtures\flutter-app\pubspec.yaml'),
            ('rust', 'language', 'manifest',
             'Found in D:\4DA\src-tauri\Cargo.toml; Found in D:\4DA\crates-placeholder\Cargo.toml'),
            ('typescript', 'language', 'manifest',
             'Found in D:\4DA\package.json');",
    )
    .unwrap();

    let counts = crate::db::purge_non_project_intelligence(&conn).unwrap();
    assert_eq!(
        counts.detected_projects, 7,
        "5 ledger fixtures + 2 placeholders purged from detected_projects"
    );
    assert_eq!(
        counts.project_dependencies, 2,
        "placeholder dep rows purged"
    );
    assert_eq!(counts.detected_tech_deleted, 1, "dart row (fixture-only)");
    assert_eq!(
        counts.detected_tech_rewritten, 1,
        "rust row keeps only real evidence"
    );

    let projects: Vec<String> = conn
        .prepare("SELECT path FROM detected_projects ORDER BY path")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert_eq!(projects.len(), 2, "only the two real projects remain");
    assert!(projects.iter().any(|p| p == r"D:\4DA"));
    assert!(projects.iter().any(|p| p == "/home/u/dev/real-app"));

    let rust_evidence: String = conn
        .query_row(
            "SELECT evidence FROM detected_tech WHERE name = 'rust'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rust_evidence, r"Found in D:\4DA\src-tauri\Cargo.toml");

    // Idempotent: a second pass finds nothing.
    let again = crate::db::purge_non_project_intelligence(&conn).unwrap();
    assert_eq!(again.total(), 0, "second pass is a no-op");
    assert_eq!(again.detected_tech_rewritten, 0);
}

#[test]
fn purge_non_project_intelligence_tolerates_missing_ace_tables() {
    let db = test_db();
    let conn = db.conn.lock();
    // No detected_projects / detected_tech / project_dependencies tables —
    // brand-new install where ACE has not migrated yet. Must not error.
    let counts = crate::db::purge_non_project_intelligence(&conn).unwrap();
    assert_eq!(counts.detected_projects, 0);
    assert_eq!(counts.detected_tech_deleted, 0);
}
