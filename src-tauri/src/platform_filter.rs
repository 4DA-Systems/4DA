// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Host-platform dependency relevance filter.
//!
//! Single source of truth for "which of the user's packages are inactive on the
//! host platform". Both the Preemption Radar and Blind Spots use this to
//! DE-PRIORITISE (never exclude) advisories and coverage gaps for deps that the
//! user does not build on this machine.
//!
//! This lived as a verbatim copy in `preemption.rs` and `blind_spots.rs`. It is
//! security-relevant filtering, so divergence between the two copies would be
//! dangerous: hoisted here so there is exactly one implementation.
//!
//! # The shared predicate
//!
//! **A package is platform-inactive when it is not built on this host.** Two
//! independent facts can make that true, and they are recorded separately
//! because the user-facing explanation differs:
//!
//! 1. **Other build target** — the manifest gates it behind
//!    `[target.'cfg(...)'.dependencies]` for a target this machine is not.
//!    Written by the ACE manifest scan into `project_dependencies.target_cfg`
//!    / `platform_active` (Phase 85). Covers DIRECT deps only.
//! 2. **Lockfile-only** — cargo resolves it for no target/feature combination
//!    this host builds, so it never reaches `rustc` here. Written by the
//!    lockfile walk into `user_dependencies.target_cfg` /
//!    `platform_active` (Phase 122) from [`crate::ace::cargo_resolve`].
//!    Covers TRANSITIVES, which fact 1 structurally cannot see.
//!
//! Both tables are consulted, because neither holds the whole picture:
//! transitives exist only in `user_dependencies`, and the cfg verdict exists
//! only in `project_dependencies`. Before Phase 122 only the second table was
//! read, so no transitive could ever be de-prioritised — the live cause of
//! Preemption's HIGH `quinn-proto` finding on 2026-09-07.
//!
//! The MCP server resolves the same predicate for `vulnerability_scan` in
//! `mcp-4da-server/src/live/version-resolver.ts` (`platformActive =
//! targetActiveOnHost(declaredTarget) && builtOnHost`) using
//! `mcp-4da-server/src/live/cargo-platform.ts`. Keep the two definitions in
//! step: a crate not resolved for the host is platform-inactive on both.

use std::collections::HashSet;

/// Packages whose EVERY tracked instance is inactive on the host platform,
/// with the reason.
///
/// A package active in even one project/target is NOT included — relevance is
/// "active in any target you build", so a dep the user actually ships
/// somewhere is never de-prioritised.
#[derive(Debug, Default)]
pub(crate) struct PlatformInactivePackages {
    /// Lowercased names, inactive everywhere.
    inactive: HashSet<String>,
    /// The subset whose verdict came from cargo's host resolution rather than
    /// a manifest `cfg(...)` spec — "in the lockfile, never compiled here".
    lockfile_only: HashSet<String>,
}

impl PlatformInactivePackages {
    /// Is `name_lower` (already lowercased) inactive on this host?
    pub(crate) fn contains(&self, name_lower: &str) -> bool {
        self.inactive.contains(name_lower)
    }

    /// Is the reason "cargo does not build it here" rather than "it belongs to
    /// another build target"? Only meaningful for a package [`Self::contains`]
    /// returns true for.
    pub(crate) fn is_lockfile_only(&self, name_lower: &str) -> bool {
        self.lockfile_only.contains(name_lower)
    }

    /// The inactive set on its own, for callers that only need membership.
    pub(crate) fn into_names(self) -> HashSet<String> {
        self.inactive
    }
}

/// Load the host-inactive package set.
///
/// De-prioritise, NEVER exclude: callers only use this to cap urgency to
/// `Watch`; the dep is still surfaced. Fails open — an unreadable or
/// pre-migration database yields an empty set, so the gate becomes a graceful
/// no-op rather than a silent drop.
pub(crate) fn load_platform_inactive_packages(
    conn: &rusqlite::Connection,
) -> PlatformInactivePackages {
    PlatformInactivePackages {
        inactive: query_names(conn, INACTIVE_BOTH_TABLES)
            // A database from before Phase 122 has no `platform_active` on
            // `user_dependencies`, so the union cannot prepare. Falling back
            // to the manifest-only verdict keeps the pre-122 behaviour
            // instead of silently losing the filter altogether.
            .or_else(|| query_names(conn, INACTIVE_MANIFEST_ONLY))
            .unwrap_or_default(),
        lockfile_only: query_names(conn, LOCKFILE_ONLY).unwrap_or_default(),
    }
}

/// Inactive in EVERY row of BOTH tables. `UNION ALL` (not `UNION`) so a
/// package active in one table and inactive in the other still reports
/// `MAX(platform_active) = 1` and stays fully urgent.
const INACTIVE_BOTH_TABLES: &str = "SELECT LOWER(package_name) FROM (
         SELECT package_name, platform_active FROM project_dependencies
         UNION ALL
         SELECT package_name, platform_active FROM user_dependencies
     ) GROUP BY LOWER(package_name) HAVING MAX(platform_active) = 0";

/// Pre-Phase-122 fallback: the manifest verdict alone.
const INACTIVE_MANIFEST_ONLY: &str = "SELECT LOWER(package_name) FROM project_dependencies
     GROUP BY LOWER(package_name) HAVING MAX(platform_active) = 0";

/// Packages carrying the lockfile-only marker. Intersected with the inactive
/// set by construction: the marker is only ever written alongside
/// `platform_active = 0`, and a package active elsewhere never reaches a
/// caller because [`PlatformInactivePackages::contains`] is checked first.
const LOCKFILE_ONLY: &str = "SELECT DISTINCT LOWER(package_name) FROM user_dependencies
     WHERE target_cfg = 'lockfile-only'";

/// `None` when the statement could not run at all (missing table or column),
/// which the caller distinguishes from "ran and found nothing".
fn query_names(conn: &rusqlite::Connection, sql: &str) -> Option<HashSet<String>> {
    let mut stmt = conn.prepare(sql).ok()?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0)).ok()?;
    Some(rows.flatten().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Both tables at their Phase 122 shape.
    fn schema(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE project_dependencies (
                 project_path TEXT, package_name TEXT, is_dev INTEGER DEFAULT 0,
                 is_direct INTEGER DEFAULT 1, target_cfg TEXT,
                 platform_active INTEGER DEFAULT 1
             );
             CREATE TABLE user_dependencies (
                 project_path TEXT, package_name TEXT, ecosystem TEXT,
                 is_dev INTEGER DEFAULT 0, is_direct INTEGER DEFAULT 1,
                 target_cfg TEXT, platform_active INTEGER NOT NULL DEFAULT 1
             );",
        )
        .unwrap();
    }

    #[test]
    fn platform_inactive_packages_collected_only_when_inactive_everywhere() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        conn.execute_batch(
            "INSERT INTO project_dependencies (project_path, package_name, platform_active) VALUES ('/p', 'libc', 0);
             INSERT INTO project_dependencies (project_path, package_name, platform_active) VALUES ('/p', 'serde', 1);
             INSERT INTO project_dependencies (project_path, package_name, platform_active) VALUES ('/a', 'shared', 0);
             INSERT INTO project_dependencies (project_path, package_name, platform_active) VALUES ('/b', 'shared', 1);",
        )
        .unwrap();

        let inactive = load_platform_inactive_packages(&conn);
        assert!(
            inactive.contains("libc"),
            "inactive-everywhere dep is collected"
        );
        assert!(
            !inactive.contains("serde"),
            "active dep is not de-prioritised"
        );
        assert!(
            !inactive.contains("shared"),
            "a dep active in any project/target stays prioritised"
        );
    }

    #[test]
    fn platform_inactive_empty_on_pre_phase85_db() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE project_dependencies (project_path TEXT, package_name TEXT);
             INSERT INTO project_dependencies VALUES ('/p', 'libc');",
        )
        .unwrap();
        // No platform_active column on either table -> both statements fail ->
        // graceful empty (nothing de-prioritised).
        assert!(load_platform_inactive_packages(&conn)
            .into_names()
            .is_empty());
    }

    #[test]
    fn a_transitive_marked_lockfile_only_is_inactive_and_labelled() {
        // The 2026-09-07 defect: `quinn-proto` is a TRANSITIVE, so it exists
        // only in user_dependencies and the manifest-only query could never
        // see it.
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        conn.execute_batch(
            "INSERT INTO user_dependencies (project_path, package_name, ecosystem, target_cfg, platform_active)
                 VALUES ('/p', 'quinn-proto', 'rust', 'lockfile-only', 0);",
        )
        .unwrap();

        let inactive = load_platform_inactive_packages(&conn);
        assert!(inactive.contains("quinn-proto"));
        assert!(
            inactive.is_lockfile_only("quinn-proto"),
            "the reason is cargo's host resolution, not a cfg spec"
        );
    }

    #[test]
    fn a_cfg_gated_dep_is_inactive_but_not_labelled_lockfile_only() {
        // The two reasons must stay distinguishable — they get different copy.
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        conn.execute_batch(
            "INSERT INTO project_dependencies (project_path, package_name, target_cfg, platform_active)
                 VALUES ('/p', 'gtk', 'cfg(unix)', 0);",
        )
        .unwrap();

        let inactive = load_platform_inactive_packages(&conn);
        assert!(inactive.contains("gtk"));
        assert!(
            !inactive.is_lockfile_only("gtk"),
            "a cfg-gated dep belongs to another build target, not to nowhere"
        );
    }

    // ---- negative tests: the union must never over-suppress ----------------

    #[test]
    fn a_crate_the_host_builds_stays_active_even_if_inactive_in_the_other_table() {
        // A dep the manifest gates for another target but that cargo DOES
        // resolve here (a second project builds it) must stay fully urgent.
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        conn.execute_batch(
            "INSERT INTO project_dependencies (project_path, package_name, target_cfg, platform_active)
                 VALUES ('/p', 'libc', 'cfg(unix)', 0);
             INSERT INTO user_dependencies (project_path, package_name, ecosystem, platform_active)
                 VALUES ('/q', 'libc', 'rust', 1);",
        )
        .unwrap();

        assert!(
            !load_platform_inactive_packages(&conn).contains("libc"),
            "active in ANY tracked instance means never de-prioritised"
        );
    }

    #[test]
    fn a_host_resolved_crate_is_never_marked_inactive() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        conn.execute_batch(
            "INSERT INTO user_dependencies (project_path, package_name, ecosystem, platform_active)
                 VALUES ('/p', 'serde', 'rust', 1);",
        )
        .unwrap();
        let inactive = load_platform_inactive_packages(&conn);
        assert!(!inactive.contains("serde"));
        assert!(!inactive.is_lockfile_only("serde"));
    }

    #[test]
    fn pre_phase122_db_keeps_the_manifest_verdict_instead_of_losing_the_filter() {
        // user_dependencies without platform_active: the union cannot prepare.
        // The fallback must still de-prioritise the cfg-gated dep — dropping
        // the filter entirely would REGRESS every pre-migration database.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE project_dependencies (
                 project_path TEXT, package_name TEXT, platform_active INTEGER DEFAULT 1);
             CREATE TABLE user_dependencies (project_path TEXT, package_name TEXT);
             INSERT INTO project_dependencies VALUES ('/p', 'gtk', 0);",
        )
        .unwrap();
        let inactive = load_platform_inactive_packages(&conn);
        assert!(inactive.contains("gtk"), "manifest verdict survives");
        assert!(!inactive.is_lockfile_only("gtk"));
    }
}
