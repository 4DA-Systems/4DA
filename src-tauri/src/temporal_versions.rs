// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The installed versions of one package near a project, for the two
//! readers that must not disagree (audit E4 review): the scorer, before it
//! calls a registry advisory "version unknown", and Preemption Tier 2, before
//! it lists one as an alert.
//!
//! Scope, narrowest first, never across ecosystem families:
//! 1. the project's OWN lockfile rows;
//! 2. else the NEAREST ancestor project with rows, bounded to the project's
//!    active repo root (a workspace lockfile; never a stray lockfile in a
//!    home directory or above the repo);
//! 3. else, only when the project has no lockfile of its own and exactly one
//!    nested project pins the package, that nested project.
//!
//! Every distinct version in the chosen scope is returned, so a caller can
//! treat the advisory as affected if ANY copy is (fail toward alerting).

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::params;

use super::{active_repo_roots, ecosystem_family};
use crate::project_inclusion::comparison_form;

fn norm(path: &str) -> String {
    comparison_form(path).trim_end_matches('/').to_string()
}

/// `path` is `root` or inside it.
fn within(path: &str, root: &str) -> bool {
    path == root || path.starts_with(&format!("{root}/"))
}

/// The name a package is matched by within an ecosystem family, as the
/// matcher folds it (`fourda_depengine::matching::package_key`): PyPI per
/// PEP 503 (lowercase, every run of `-`, `_`, `.` one `-`), so "Django" is
/// "django" and "Foo_Bar" is "foo-bar"; Cargo treats `-` and `_` as the same
/// crate; npm names are exact (scopes included); others compare lowercase.
fn folded_name(name: &str, family: &str) -> String {
    match family {
        "python" => fourda_depengine::matching::pep503_name(name),
        "rust" => name.trim().to_lowercase().replace('_', "-"),
        "javascript" => name.trim().to_string(),
        _ => name.trim().to_lowercase(),
    }
}

/// The spellings of `package` the name index is asked for: as given, its
/// lowercase and folded forms, and (where separators fold) the folded form
/// with `_` / `.` in place of `-`.
fn candidate_names(package: &str, family: &str) -> Vec<String> {
    let folded = folded_name(package, family);
    let mut out = vec![package.trim().to_string(), package.trim().to_lowercase()];
    match family {
        "python" => {
            out.push(folded.replace('-', "_"));
            out.push(folded.replace('-', "."));
        }
        "rust" => out.push(folded.replace('-', "_")),
        _ => {}
    }
    out.push(folded);
    out.sort();
    out.dedup();
    out
}

/// Pinned versions of `package` per project path (normalized), same
/// ecosystem family, matched by [`folded_name`]. The spellings in
/// [`candidate_names`] hit the `idx_user_deps_package` index; only when
/// none is stored and the family folds separators (a "Foo_Bar" pin for an
/// advisory on "foo-bar") does a separator-insensitive scan run.
fn pins_by_project(
    conn: &rusqlite::Connection,
    package: &str,
    family: &str,
) -> BTreeMap<String, BTreeSet<String>> {
    let want = folded_name(package, family);
    let candidates = candidate_names(package, family);
    let placeholders = vec!["?"; candidates.len()].join(", ");
    let indexed = format!(
        "SELECT project_path, ecosystem, version, package_name FROM user_dependencies
         WHERE package_name IN ({placeholders})
           AND version IS NOT NULL AND version <> ''"
    );
    let mut out = collect_pins(conn, &indexed, &candidates, family, &want);
    if out.is_empty() && matches!(family, "python" | "rust") {
        // LIKE is ASCII case-insensitive, and `_` matches any one character,
        // so every separator spelling and case is a candidate; the fold
        // below keeps only the true ones.
        let pattern: String = want
            .chars()
            .map(|c| match c {
                '-' | '_' | '.' => '_',
                '%' => '_',
                c => c,
            })
            .collect();
        let scan = "SELECT project_path, ecosystem, version, package_name FROM user_dependencies
             WHERE package_name LIKE ?1
               AND version IS NOT NULL AND version <> ''";
        out = collect_pins(conn, scan, &[pattern], family, &want);
    }
    out
}

/// The rows `sql` returns for `args`, kept when the name folds to `want` in
/// `family`.
fn collect_pins(
    conn: &rusqlite::Connection,
    sql: &str,
    args: &[String],
    family: &str,
    want: &str,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let Ok(mut stmt) = conn.prepare_cached(sql) else {
        return out;
    };
    let Ok(rows) = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    }) else {
        return out;
    };
    for (path, eco, version, name) in rows.flatten() {
        if ecosystem_family(&eco) == family && folded_name(&name, family) == want {
            out.entry(norm(&path)).or_default().insert(version);
        }
    }
    out
}

/// Whether the project has any lockfile row of its own (for rule 3).
fn has_own_lockfile(conn: &rusqlite::Connection, project: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM user_dependencies
         WHERE LOWER(REPLACE(project_path, '\\', '/')) IN (?1, ?2) LIMIT 1",
        params![project, format!("{project}/")],
        |_| Ok(()),
    )
    .is_ok()
}

/// The versions in scope for ONE project (rules 1 to 3).
fn versions_for_project(
    pins: &BTreeMap<String, BTreeSet<String>>,
    project: &str,
    roots: &[String],
    own_lockfile: impl FnOnce() -> bool,
) -> BTreeSet<String> {
    if let Some(own) = pins.get(project) {
        return own.clone();
    }
    let root = roots
        .iter()
        .filter(|r| within(project, r))
        .max_by_key(|r| r.len());
    if let Some(root) = root {
        let nearest = pins
            .keys()
            .filter(|p| within(project, p) && within(p, root))
            .max_by_key(|p| p.len());
        if let Some(path) = nearest {
            return pins[path].clone();
        }
    }
    let nested: Vec<&String> = pins.keys().filter(|p| within(p, project)).collect();
    if nested.len() == 1 && !own_lockfile() {
        return pins[nested[0]].clone();
    }
    BTreeSet::new()
}

/// Every distinct installed version of `package` in scope for any of
/// `project_paths` (see the module doc), sorted. Empty: unknown.
pub(crate) fn installed_versions_nearby(
    conn: &rusqlite::Connection,
    package: &str,
    ecosystem: &str,
    project_paths: &[String],
) -> Vec<String> {
    let pins = pins_by_project(conn, package, &ecosystem_family(ecosystem));
    if pins.is_empty() {
        return Vec::new();
    }
    let roots: Vec<String> = active_repo_roots(conn).iter().map(|r| norm(r)).collect();
    let mut out = BTreeSet::new();
    for project in project_paths.iter().map(|p| norm(p)) {
        out.extend(versions_for_project(&pins, &project, &roots, || {
            has_own_lockfile(conn, &project)
        }));
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pins(entries: &[(&str, &str)]) -> BTreeMap<String, BTreeSet<String>> {
        let mut m: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (p, v) in entries {
            m.entry((*p).to_string())
                .or_default()
                .insert((*v).to_string());
        }
        m
    }

    #[test]
    fn own_lockfile_beats_a_stray_parent_lockfile() {
        let p = pins(&[
            ("c:/users/me", "4.17.21"),
            ("c:/users/me/code/api", "4.17.1"),
        ]);
        let roots = vec!["c:/users/me/code/api".to_string()];
        let v = versions_for_project(&p, "c:/users/me/code/api", &roots, || true);
        assert_eq!(v.into_iter().collect::<Vec<_>>(), vec!["4.17.1"]);
    }

    #[test]
    fn a_parent_above_the_repo_root_never_supplies_a_version() {
        let p = pins(&[("c:/users/me", "4.17.21")]);
        let roots = vec!["c:/users/me/code/api".to_string()];
        assert!(versions_for_project(&p, "c:/users/me/code/api/web", &roots, || true).is_empty());
        // With no active roots nothing widens upward at all.
        assert!(versions_for_project(&p, "c:/users/me/code/api", &[], || true).is_empty());
    }

    #[test]
    fn the_nearest_ancestor_inside_the_repo_root_answers() {
        let p = pins(&[("d:/mono", "1.0.0"), ("d:/mono/pkgs", "2.0.0")]);
        let roots = vec!["d:/mono".to_string()];
        let v = versions_for_project(&p, "d:/mono/pkgs/app", &roots, || true);
        assert_eq!(v.into_iter().collect::<Vec<_>>(), vec!["2.0.0"]);
    }

    #[test]
    fn a_nested_pin_counts_only_without_an_own_lockfile_and_only_if_unique() {
        let p = pins(&[("d:/app/sub", "1.0.0")]);
        assert!(versions_for_project(&p, "d:/app", &[], || true).is_empty());
        assert_eq!(versions_for_project(&p, "d:/app", &[], || false).len(), 1);
        let two = pins(&[("d:/app/a", "1.0.0"), ("d:/app/b", "2.0.0")]);
        assert!(versions_for_project(&two, "d:/app", &[], || false).is_empty());
    }

    /// The pins of `stored` (name, ecosystem as stored) in one project, read
    /// for `asked` the way the matcher names it.
    fn read(stored: &[(&str, &str)], asked: &str, ecosystem: &str) -> Vec<String> {
        let db = crate::test_utils::test_db();
        for (i, (name, eco)) in stored.iter().enumerate() {
            db.store_dependency(
                "d:/work/app",
                name,
                Some(&format!("1.0.{i}")),
                eco,
                false,
                None,
            )
            .expect("pin");
        }
        let conn = db.conn.lock();
        installed_versions_nearby(&conn, asked, ecosystem, &["d:/work/app".to_string()])
    }

    #[test]
    fn a_pypi_pin_is_found_by_its_pep503_name() {
        assert_eq!(read(&[("Django", "python")], "django", "PyPI"), ["1.0.0"]);
        assert_eq!(read(&[("Foo_Bar", "python")], "foo-bar", "PyPI"), ["1.0.0"]);
        assert_eq!(read(&[("foo.bar", "python")], "Foo_Bar", "PyPI"), ["1.0.0"]);
        assert!(read(&[("foobar", "python")], "foo-bar", "PyPI").is_empty());
    }

    #[test]
    fn a_cargo_pin_is_found_across_dash_and_underscore() {
        assert_eq!(
            read(&[("serde_json", "rust")], "serde-json", "crates.io"),
            ["1.0.0"]
        );
        assert_eq!(
            read(&[("serde-json", "rust")], "serde_json", "crates.io"),
            ["1.0.0"]
        );
    }

    #[test]
    fn npm_names_stay_exact_and_families_never_mix() {
        assert_eq!(
            read(&[("@scope/pkg", "javascript")], "@scope/pkg", "npm"),
            ["1.0.0"]
        );
        assert!(read(&[("JSONStream", "javascript")], "jsonstream", "npm").is_empty());
        assert!(read(&[("foo_bar", "javascript")], "foo-bar", "npm").is_empty());
        assert!(read(&[("django", "javascript")], "django", "PyPI").is_empty());
    }

    #[test]
    fn several_copies_in_scope_are_all_returned() {
        let p = pins(&[("d:/app", "1.0.0"), ("d:/app", "2.0.0")]);
        assert_eq!(versions_for_project(&p, "d:/app", &[], || true).len(), 2);
    }
}
