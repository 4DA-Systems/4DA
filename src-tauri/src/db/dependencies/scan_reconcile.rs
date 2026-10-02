// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Post-scan reconcile of `user_dependencies` rows that only the MANIFEST
//! scan wrote.
//!
//! `store_direct_dependencies` copies every `project_dependencies` row into
//! `user_dependencies` (provenance `manifest`, or `import_scrape` from older
//! builds). The manifest scan prunes its own table when a declaration goes
//! away, and the lockfile walk prunes rows its lockfile no longer resolves —
//! but a project WITHOUT its own lockfile (a workspace member crate, a package
//! in an npm workspace) is never walked, so its copied rows were immortal.
//! Live 2026-10-02 they included a dotted-key phantom fixed on 09-25
//! (`<crate>.workspace`), the underscore-spelled import-scrape duplicates
//! (`<crate>_core`), and workspace members stored as installs of themselves.

use rusqlite::{params, Result as SqliteResult};

/// Delete `user_dependencies` rows whose provenance is the manifest scan, that
/// no `project_dependencies` row of the same project still declares, and that
/// nothing touched during the scan that started at `scan_started` (an SQLite
/// `datetime('now')` string).
///
/// The last condition keeps every row a lockfile walk confirmed in this scan:
/// a dependency dropped from the manifest but still resolved by the lockfile
/// is a transitive now, not gone, and Go's synthetic `stdlib` / `toolchain`
/// rows (written from go.mod directives by the walk, never declared in
/// `project_dependencies`) are refreshed every scan. Returns rows deleted.
pub fn prune_undeclared_manifest_rows(
    conn: &rusqlite::Connection,
    scan_started: &str,
) -> SqliteResult<usize> {
    conn.execute(
        "DELETE FROM user_dependencies
         WHERE detected_from IN ('manifest', 'import_scrape')
           AND last_seen_at < ?1
           AND NOT EXISTS (
               SELECT 1 FROM project_dependencies pd
                WHERE pd.project_path = user_dependencies.project_path
                  AND LOWER(pd.package_name) = LOWER(user_dependencies.package_name))",
        params![scan_started],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE user_dependencies (
                 project_path TEXT, package_name TEXT, ecosystem TEXT,
                 detected_from TEXT, last_seen_at TEXT);
             CREATE TABLE project_dependencies (project_path TEXT, package_name TEXT);",
        )
        .unwrap();
        conn
    }

    fn ud(conn: &rusqlite::Connection, path: &str, name: &str, from: &str, seen: &str) {
        conn.execute(
            "INSERT INTO user_dependencies VALUES (?1, ?2, 'rust', ?3, ?4)",
            params![path, name, from, seen],
        )
        .unwrap();
    }

    fn names(conn: &rusqlite::Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT package_name FROM user_dependencies ORDER BY package_name")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn undeclared_manifest_rows_of_an_unwalked_member_are_pruned() {
        let conn = db();
        let member = "d:/ws/crates/cli";
        let before = "2026-10-01 00:00:00";
        let scan = "2026-10-02 00:00:00";
        conn.execute(
            "INSERT INTO project_dependencies VALUES (?1, 'serde')",
            params![member],
        )
        .unwrap();
        ud(&conn, member, "serde", "manifest", scan);
        ud(&conn, member, "acme-core.workspace", "manifest", before);
        ud(&conn, member, "acme_core", "import_scrape", before);
        ud(&conn, member, "acme-core", "manifest", before);
        // A lockfile row and a row touched in this scan always survive.
        ud(&conn, member, "fastrand", "lockfile", before);
        ud(&conn, member, "stdlib", "manifest", scan);
        // Declared under different case: still declared.
        conn.execute(
            "INSERT INTO project_dependencies VALUES (?1, 'Tokio')",
            params![member],
        )
        .unwrap();
        ud(&conn, member, "tokio", "manifest", before);

        let removed = prune_undeclared_manifest_rows(&conn, scan).unwrap();
        assert_eq!(removed, 3);
        assert_eq!(names(&conn), vec!["fastrand", "serde", "stdlib", "tokio"]);
    }
}
