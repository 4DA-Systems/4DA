// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The temp-directory rule for the dependency inventory.
//!
//! A project inside the operating system's temp directory is a temporary
//! copy (a test fixture, a throwaway clone), so it must not feed the
//! vulnerability inventory. Until 2026-10-13 this rule was three substring
//! patterns applied with SQL `LIKE` (`%/tmp/%`, `%\tmp\%`,
//! `%AppData%Local%Temp%`, case-insensitive): any project whose path merely
//! contained a `tmp` segment (`D:\work\tmp\api`, `~/tmp/site`) was dropped from
//! matching, and Preemption then told the user "No advisory database covers
//! these dependencies" — the wrong reason (fresh-profile audit E6). The rule is
//! now the OS temp directory itself, and the lockfile walk reports what it
//! skipped under it.

use std::path::Path;
use std::sync::OnceLock;

/// `path` for prefix comparison: forward slashes, no Windows extended-length
/// prefix, no trailing slash, and lowercased on Windows (case-insensitive
/// file system).
fn comparison_form(path: &str) -> String {
    let p = path
        .strip_prefix(r"\\?\")
        .unwrap_or(path)
        .replace('\\', "/");
    let p = p.trim_end_matches('/').to_string();
    if cfg!(windows) {
        p.to_lowercase()
    } else {
        p
    }
}

/// Every spelling of `temp`: as given and canonicalised. On Windows the
/// reported temp dir can be an 8.3 short name (`ADMINI~1`) while scan paths
/// use the long one; on macOS `/var/folders/..` canonicalises to
/// `/private/var/folders/..`. Empty roots are dropped (never "everything").
pub(crate) fn roots_for(temp: &Path) -> Vec<String> {
    let mut roots = vec![comparison_form(&temp.to_string_lossy())];
    if let Ok(canonical) = std::fs::canonicalize(temp) {
        roots.push(comparison_form(&canonical.to_string_lossy()));
    }
    roots.retain(|r| !r.is_empty());
    roots.sort();
    roots.dedup();
    roots
}

/// The OS temp directory's spellings, resolved once per process.
pub(crate) fn os_temp_roots() -> &'static [String] {
    static ROOTS: OnceLock<Vec<String>> = OnceLock::new();
    ROOTS.get_or_init(|| roots_for(&std::env::temp_dir()))
}

/// Whether `path` is one of `roots` or lies below one. Segment-exact: a
/// sibling that only shares a prefix (`/tmpfiles` for root `/tmp`) is not
/// inside it.
pub(crate) fn is_under_any(path: &str, roots: &[String]) -> bool {
    let p = comparison_form(path);
    roots.iter().any(|root| {
        p == *root
            || p.strip_prefix(root.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

/// Whether `path` is inside the operating system's temp directory.
pub(crate) fn is_os_temp_path(path: &str) -> bool {
    is_under_any(path, os_temp_roots())
}

/// Delete `user_dependencies` rows of projects inside `roots` (the startup
/// self-heal for rows an older build stored). Returns rows deleted.
pub(crate) fn purge_temp_dependency_rows(
    conn: &rusqlite::Connection,
    roots: &[String],
) -> rusqlite::Result<usize> {
    let paths: Vec<String> = {
        let mut stmt = conn.prepare("SELECT DISTINCT project_path FROM user_dependencies")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.flatten().filter(|p| is_under_any(p, roots)).collect()
    };
    let mut deleted = 0;
    for path in paths {
        deleted += conn.execute(
            "DELETE FROM user_dependencies WHERE project_path = ?1",
            [&path],
        )?;
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(r: &str) -> Vec<String> {
        vec![comparison_form(r)]
    }

    /// E6: only the OS temp directory is temporary. A project folder that
    /// merely has a `tmp` segment, or sits under some other `Temp`, is a real
    /// project and stays in the inventory.
    #[test]
    fn only_paths_inside_the_temp_root_count_as_temporary() {
        let unix = roots("/tmp");
        assert!(is_under_any("/tmp/clone/app", &unix));
        assert!(is_under_any("/tmp", &unix));
        assert!(!is_under_any("/home/u/tmp/site", &unix));
        assert!(!is_under_any("/tmpfiles/app", &unix));
        assert!(!is_under_any("/home/u/code/api", &unix));

        let win = roots(r"C:\Users\dev\AppData\Local\Temp");
        assert!(is_under_any(r"C:\Users\dev\AppData\Local\Temp\x\app", &win));
        assert!(is_under_any(r"\\?\C:\Users\dev\AppData\Local\Temp\x", &win));
        assert!(is_under_any("c:/users/dev/appdata/local/temp/x", &win) == cfg!(windows));
        assert!(!is_under_any(r"D:\work\tmp\api", &win));
        assert!(!is_under_any(r"C:\Users\dev\Projects\Temperature", &win));
        assert!(!is_under_any(r"C:\Users\other\AppData\Local\Temp\x", &win));
    }

    #[test]
    fn the_real_temp_dir_and_its_canonical_spelling_both_match() {
        let temp = std::env::temp_dir();
        let inside = temp.join("4da-temp-rule").display().to_string();
        assert!(
            is_os_temp_path(&inside),
            "{inside} vs {:?}",
            os_temp_roots()
        );
        if let Ok(canonical) = std::fs::canonicalize(&temp) {
            let inside = canonical.join("x").display().to_string();
            assert!(
                is_os_temp_path(&inside),
                "{inside} vs {:?}",
                os_temp_roots()
            );
        }
        assert!(!os_temp_roots().iter().any(String::is_empty));
    }

    /// The read side of E6: the audit inventory keeps a project whose path has
    /// a `tmp` segment and drops one inside the OS temp directory.
    #[test]
    fn auditable_inventory_drops_the_os_temp_dir_not_tmp_segments() {
        let db = crate::test_utils::test_db();
        let tmp_segment = if cfg!(windows) {
            r"D:\work\tmp\api"
        } else {
            "/home/u/tmp/api"
        };
        let os_temp = std::env::temp_dir()
            .join("4da-clone")
            .join("app")
            .display()
            .to_string();
        {
            let conn = db.conn.lock();
            for (path, pkg) in [(tmp_segment, "axios"), (os_temp.as_str(), "lodash")] {
                conn.execute(
                    "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem,
                         is_dev, is_direct, detected_at, last_seen_at)
                     VALUES (?1, ?2, '1.0.0', 'javascript', 0, 1, datetime('now'), datetime('now'))",
                    rusqlite::params![path, pkg],
                )
                .unwrap();
            }
        }
        let names: Vec<String> = db
            .get_auditable_user_dependencies()
            .unwrap()
            .into_iter()
            .map(|d| d.package_name)
            .collect();
        assert!(names.contains(&"axios".to_string()), "{names:?}");
        assert!(!names.contains(&"lodash".to_string()), "{names:?}");
    }

    #[test]
    fn purge_removes_only_rows_under_the_temp_root() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE user_dependencies (project_path TEXT, package_name TEXT);
             INSERT INTO user_dependencies VALUES ('/tmp/clone/app', 'axios');
             INSERT INTO user_dependencies VALUES ('/tmp/clone/app', 'lodash');
             INSERT INTO user_dependencies VALUES ('/home/u/tmp/site', 'react');
             INSERT INTO user_dependencies VALUES ('/home/u/code/api', 'express');",
        )
        .unwrap();
        assert_eq!(
            purge_temp_dependency_rows(&conn, &roots("/tmp")).unwrap(),
            2
        );
        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM user_dependencies", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 2);
    }
}
