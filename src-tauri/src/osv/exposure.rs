// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Is a stored advisory ABOUT this install? (AD-045)
//!
//! A package name is not an identity. `jsonwebtoken` is a Rust crate and an
//! unrelated npm package, with unrelated advisories, and the founder's machine
//! carries both. Measured 2026-09-10: Blind Spots told the user "you're on
//! 9.0.3 – 10.4.0" for the crates.io row naming `relay` and `src-tauri`, where
//! 9.0.3 is the npm copy in the VS Code extension; and the knowledge-gap row
//! for `hono` 4.13.5 counted "3 unread security advisories" that were all
//! fixed IN 4.13.5.
//!
//! Every per-package lookup that feeds a claim is keyed by (ecosystem,
//! package), and a stored advisory row counts against an install only when the
//! advisory is for that install's ecosystem AND the install's version is
//! inside its range. This module holds the two primitives that make that
//! cheap: canonical ecosystem identity, and a live verdict for an advisory
//! source row (osv/cve) against the installs a claim names.

use rusqlite::params;

/// The canonical ecosystem id (the OSV name) for any alias the tables store:
/// `rust` / `cargo` / `crates.io` → `crates.io`, `javascript` / `npm` → `npm`.
/// `None` for anything unrecognised (including the linker's placeholder
/// `advisory`).
pub(crate) fn canonical(ecosystem: &str) -> Option<&'static str> {
    crate::ecosystem::Ecosystem::parse(ecosystem).map(crate::ecosystem::Ecosystem::osv_name)
}

/// The ecosystem a package-registry source's rows belong to — the inverse of
/// `Ecosystem::source_types` for the registry adapters.
pub(crate) fn registry_source_ecosystem(source_type: &str) -> Option<&'static str> {
    match source_type {
        "npm_registry" | "npm" => Some("npm"),
        "crates_io" | "crates" => Some("crates.io"),
        "pypi" => Some("PyPI"),
        "go_modules" | "go" => Some("Go"),
        "maven" => Some("Maven"),
        "nuget" => Some("NuGet"),
        "packagist" => Some("Packagist"),
        "rubygems" => Some("RubyGems"),
        _ => None,
    }
}

/// One installed copy a claim names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Install {
    /// Canonical ecosystem ([`canonical`]); `None` when the source did not say.
    pub ecosystem: Option<&'static str>,
    pub version: String,
}

impl Install {
    pub(crate) fn new(ecosystem: Option<&str>, version: impl Into<String>) -> Self {
        Self {
            ecosystem: ecosystem.and_then(canonical),
            version: version.into(),
        }
    }
}

/// `(ecosystem, affected_ranges)` of the stored advisories an osv/cve source
/// row stands for, for `package`. osv rows carry the advisory id as their
/// `source_id`; cve rows carry the CVE id, which the mirror stores as an alias
/// (`aliases` is a JSON array). Names compare the way crates.io does
/// (case-insensitive, `-` == `_`).
pub(crate) fn advisory_records(
    conn: &rusqlite::Connection,
    source_id: &str,
    package: &str,
) -> Vec<(String, Option<String>)> {
    let id = source_id.trim();
    if id.is_empty() || package.trim().is_empty() {
        return Vec::new();
    }
    let alias_pattern = format!("%\"{id}\"%");
    let Ok(mut stmt) = conn.prepare(
        "SELECT ecosystem, affected_ranges FROM osv_advisories
         WHERE REPLACE(lower(package_name), '-', '_') = REPLACE(lower(?1), '-', '_')
           AND withdrawn_at IS NULL
           AND (advisory_id = ?2 OR aliases LIKE ?3)",
    ) else {
        return Vec::new();
    };
    stmt.query_map(params![package, id, alias_pattern], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    })
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

/// Live verdict for an advisory source row against the installs a claim names.
///
/// - `Some(true)`: some install in the advisory's own ecosystem is inside its
///   affected range.
/// - `Some(false)`: the row resolves to stored advisories and none reaches any
///   install — every install is past the fix, or the advisory is for another
///   ecosystem's package of the same name.
/// - `None`: the row resolves to no stored advisory, or there is no install to
///   judge. The caller keeps its conservative fallback.
///
/// An install whose ecosystem is unknown is judged against every record — the
/// pre-AD-045 behaviour, kept only where the data cannot say more.
pub(crate) fn advisory_row_reaches(
    conn: &rusqlite::Connection,
    source_id: &str,
    package: &str,
    installs: &[Install],
) -> Option<bool> {
    if installs.is_empty() {
        return None;
    }
    let records = advisory_records(conn, source_id, package);
    if records.is_empty() {
        return None;
    }
    Some(records.iter().any(|(ecosystem, ranges)| {
        let advisory_ecosystem = canonical(ecosystem);
        installs.iter().any(|install| {
            let same_ecosystem = match (install.ecosystem, advisory_ecosystem) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            };
            same_ecosystem
                && crate::osv::matching::check_version_affected(
                    Some(install.version.as_str()),
                    ranges,
                )
                .0
        })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real ranges shape the mirror stores (OSV `affected[].ranges`).
    fn ranges_fixed_at(fixed: &str) -> String {
        format!(r#"[{{"type":"SEMVER","events":[{{"introduced":"0"}},{{"fixed":"{fixed}"}}]}}]"#)
    }

    fn conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE osv_advisories (
                 id INTEGER PRIMARY KEY, advisory_id TEXT NOT NULL, summary TEXT NOT NULL DEFAULT '',
                 package_name TEXT NOT NULL, ecosystem TEXT NOT NULL, affected_ranges TEXT,
                 fixed_versions TEXT, aliases TEXT, withdrawn_at TEXT
             );",
        )
        .expect("schema");
        let add = |id: &str, pkg: &str, eco: &str, fixed: &str, aliases: &str| {
            conn.execute(
                "INSERT INTO osv_advisories (advisory_id, package_name, ecosystem, affected_ranges, aliases)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, pkg, eco, ranges_fixed_at(fixed), aliases],
            )
            .expect("insert");
        };
        // The founder's two same-named packages, as stored live 2026-09-10.
        add(
            "GHSA-h395-gr6q-cpjc",
            "jsonwebtoken",
            "crates.io",
            "10.3.0",
            r#"["CVE-2026-25537"]"#,
        );
        add(
            "GHSA-27h2-hvpr-p74q",
            "jsonwebtoken",
            "npm",
            "9.0.0",
            r#"["CVE-2022-23529"]"#,
        );
        add(
            "GHSA-crvj-82cr-hjcx",
            "hono",
            "npm",
            "4.13.5",
            r#"["CVE-2026-84363"]"#,
        );
        conn
    }

    #[test]
    fn canonical_folds_every_alias_the_tables_store() {
        assert_eq!(canonical("rust"), Some("crates.io"));
        assert_eq!(canonical("cargo"), Some("crates.io"));
        assert_eq!(canonical("crates.io"), Some("crates.io"));
        assert_eq!(canonical("javascript"), Some("npm"));
        assert_eq!(canonical("npm"), Some("npm"));
        assert_eq!(
            canonical("advisory"),
            None,
            "the linker's placeholder is not an ecosystem"
        );
    }

    #[test]
    fn a_crates_advisory_never_reaches_the_npm_package_of_the_same_name() {
        let conn = conn();
        let npm_copy = [Install::new(Some("javascript"), "9.0.3")];
        let rust_copy = [Install::new(Some("rust"), "9.3.1")];
        assert_eq!(
            advisory_row_reaches(&conn, "GHSA-h395-gr6q-cpjc", "jsonwebtoken", &npm_copy),
            Some(false),
            "crates.io advisory vs the npm install"
        );
        assert_eq!(
            advisory_row_reaches(&conn, "GHSA-h395-gr6q-cpjc", "jsonwebtoken", &rust_copy),
            Some(true),
            "crates.io advisory vs the exposed crates install"
        );
    }

    #[test]
    fn a_cve_row_resolves_through_the_mirrors_aliases() {
        let conn = conn();
        let fixed = [Install::new(Some("npm"), "4.13.5")];
        let exposed = [Install::new(Some("npm"), "4.13.3")];
        assert_eq!(
            advisory_row_reaches(&conn, "CVE-2026-84363", "hono", &fixed),
            Some(false),
            "4.13.5 IS the fix — the advisory is resolved for this install"
        );
        assert_eq!(
            advisory_row_reaches(&conn, "CVE-2026-84363", "hono", &exposed),
            Some(true)
        );
    }

    #[test]
    fn unresolvable_rows_and_missing_installs_cannot_be_judged() {
        let conn = conn();
        let any = [Install::new(Some("npm"), "1.0.0")];
        assert_eq!(
            advisory_row_reaches(&conn, "CVE-0000-0000", "hono", &any),
            None
        );
        assert_eq!(
            advisory_row_reaches(&conn, "GHSA-crvj-82cr-hjcx", "hono", &[]),
            None
        );
    }

    #[test]
    fn registry_sources_name_their_ecosystem() {
        assert_eq!(registry_source_ecosystem("crates_io"), Some("crates.io"));
        assert_eq!(registry_source_ecosystem("npm_registry"), Some("npm"));
        assert_eq!(registry_source_ecosystem("hackernews"), None);
    }
}
