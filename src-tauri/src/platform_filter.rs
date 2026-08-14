// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Host-platform dependency relevance filter.
//!
//! Single source of truth for "which of the user's packages are inactive on the
//! host platform". Both the Preemption Radar and Blind Spots use this to
//! DE-PRIORITISE (never exclude) advisories and coverage gaps for deps that the
//! user does not build on this machine — e.g. a `cfg(not(windows))` crate on a
//! Windows box.
//!
//! This lived as a verbatim copy in `preemption.rs` and `blind_spots.rs`. It is
//! security-relevant filtering, so divergence between the two copies would be
//! dangerous: hoisted here so there is exactly one implementation.

use std::collections::HashSet;

/// Lowercased names of packages whose EVERY tracked instance is inactive on the
/// host platform. A package active in even one project/target is NOT included —
/// relevance is "active in any target you build", so we never de-prioritise a
/// dep the user actually ships somewhere.
///
/// De-prioritise, NEVER exclude: callers only use this set to cap urgency to
/// `Watch`; the dep is still surfaced. Returns empty on any query failure
/// (e.g. a pre-Phase-85 DB with no `platform_active` column) so the gate is a
/// graceful no-op rather than a silent drop.
pub(crate) fn load_platform_inactive_packages(conn: &rusqlite::Connection) -> HashSet<String> {
    let mut stmt = match conn.prepare(
        "SELECT LOWER(package_name) FROM project_dependencies
         GROUP BY LOWER(package_name) HAVING MAX(platform_active) = 0",
    ) {
        Ok(s) => s,
        Err(_) => return HashSet::new(),
    };
    stmt.query_map([], |row| row.get::<_, String>(0))
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn platform_inactive_packages_collected_only_when_inactive_everywhere() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE project_dependencies (
                 project_path TEXT, package_name TEXT, is_dev INTEGER DEFAULT 0,
                 is_direct INTEGER DEFAULT 1, platform_active INTEGER DEFAULT 1
             );
             INSERT INTO project_dependencies (project_path, package_name, platform_active) VALUES ('/p', 'libc', 0);
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
        // No platform_active column -> the prepare fails -> graceful empty
        // (nothing de-prioritised).
        assert!(load_platform_inactive_packages(&conn).is_empty());
    }
}
