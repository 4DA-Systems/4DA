// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Where an inventory row's INSTALLED version comes from: the lockfile layer
//! (`dependency_instances`), never a row 4DA wrote back itself.
//!
//! The manifest parsers carry names only, so [`super::included_dependencies_resolved`]
//! resolves each version at read time, and `store_direct_dependencies` writes
//! that resolved version back into `user_dependencies`. Both tiers used to
//! read `user_dependencies` too, which closed a loop for every workspace
//! member: a member has no lockfile of its own, so its first version came
//! from the workspace lockfile (tier ii) and was written back to the
//! member's own row; from then on tier (i) found that row first, read the
//! value 4DA had written, and wrote it back again. Nothing re-read the
//! lockfile. Live 2026-10-27..31: a CLI crate moved `notify` 7 -> 8 on
//! 10-19 (its workspace lockfile resolved 8.2.0), but its row kept 7.0.0
//! and three mornings showed "upgrade notify, installed 7.0.0". Six more
//! member rows were frozen the same way (tokio 1.52.x, getrandom).
//!
//! `dependency_instances` is rewritten from the lockfile on every walk, so
//! reading it breaks the loop, and the write-back then heals the stored row
//! on the next scan. The one fallback to `user_dependencies` is Go: the
//! `go`/`toolchain` directives are versioned facts of `go.mod` itself,
//! re-stored on every walk (`store_go_directive_dependencies`), with no
//! lockfile row to stand in for them.

use rusqlite::params;

use super::{ecosystem_family, ecosystem_family_sql, shares_active_root, ProjectDependency};
use crate::project_inclusion::comparison_form;

/// Does this database carry the lockfile layer? Every migrated database
/// does; a bare test fixture may not, and then the older single-table
/// resolution is all there is to read.
pub(super) fn has_instance_layer(conn: &rusqlite::Connection) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'dependency_instances'",
        [],
        |_| Ok(()),
    )
    .is_ok()
}

/// Tier (i) as a SQL expression over `pd` (a `project_dependencies` row):
/// the manifest's own version, else the SAME project's lockfile, same
/// ecosystem family, direct copy first.
pub(super) fn same_project_version_sql(instances: bool) -> String {
    let pd_family = ecosystem_family_sql("pd.language");
    let ud_family = ecosystem_family_sql("ud.ecosystem");
    let stored = format!(
        "(SELECT ud.version FROM user_dependencies ud
          WHERE ud.project_path = pd.project_path
            AND LOWER(ud.package_name) = LOWER(pd.package_name)
            AND {ud_family} = {pd_family}
            AND ud.version IS NOT NULL AND ud.version <> ''
            {go_only}
          ORDER BY ud.is_direct DESC, ud.last_seen_at DESC
          LIMIT 1)",
        go_only = if instances {
            format!("AND {pd_family} = 'go'")
        } else {
            String::new()
        },
    );
    if !instances {
        return format!("COALESCE(pd.version, {stored})");
    }
    let di_family = ecosystem_family_sql("di.ecosystem");
    format!(
        "COALESCE(
            pd.version,
            (SELECT di.version FROM dependency_instances di
              WHERE di.project_path = pd.project_path
                AND LOWER(di.package_name) = LOWER(pd.package_name)
                AND {di_family} = {pd_family}
                AND di.version IS NOT NULL AND di.version <> ''
              ORDER BY di.is_direct DESC, di.version DESC
              LIMIT 1),
            {stored})"
    )
}

/// Is `ancestor` a strict parent directory of `path` (both normalized)?
fn is_ancestor(ancestor: &str, path: &str) -> bool {
    let a = comparison_form(ancestor);
    let p = comparison_form(path);
    let (a, p) = (a.trim_end_matches('/'), p.trim_end_matches('/'));
    !a.is_empty() && p.starts_with(&format!("{a}/"))
}

/// Tier (ii): a dep still version-less after tier (i) resolves from a
/// lockfile elsewhere under the SAME active repo root, same ecosystem family:
/// the NEAREST enclosing lockfile first (the workspace that builds the
/// member), then any other under that root. Never across roots or
/// ecosystems; with no active roots (first run) nothing widens.
pub(super) fn resolve_versions_within_roots(
    conn: &rusqlite::Connection,
    deps: &mut [ProjectDependency],
    active_roots: &[String],
) {
    if active_roots.is_empty() || deps.iter().all(|d| d.version.is_some()) {
        return;
    }
    let sql = if has_instance_layer(conn) {
        "SELECT project_path, ecosystem, version, is_direct FROM dependency_instances
         WHERE LOWER(package_name) = LOWER(?1)
           AND version IS NOT NULL AND version <> ''"
    } else {
        "SELECT project_path, ecosystem, version, is_direct FROM user_dependencies
         WHERE LOWER(package_name) = LOWER(?1)
           AND version IS NOT NULL AND version <> ''"
    };
    let Ok(mut stmt) = conn.prepare(sql) else {
        return;
    };
    for dep in deps.iter_mut().filter(|d| d.version.is_none()) {
        let family = ecosystem_family(&dep.language);
        let Ok(rows) = stmt.query_map(params![dep.package_name], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)? != 0,
            ))
        }) else {
            continue;
        };
        let mut candidates: Vec<(String, String, bool)> = rows
            .flatten()
            .filter(|(path, ecosystem, ..)| {
                ecosystem_family(ecosystem) == family
                    && shares_active_root(&dep.project_path, path, active_roots)
            })
            .map(|(path, _, version, direct)| (path, version, direct))
            .collect();
        // Nearest enclosing lockfile, then direct copies, then the newest
        // version (a deterministic pick among equals).
        candidates.sort_by(|a, b| {
            let rank = |c: &(String, String, bool)| {
                if is_ancestor(&c.0, &dep.project_path) {
                    comparison_form(&c.0).len()
                } else {
                    0
                }
            };
            rank(b)
                .cmp(&rank(a))
                .then(b.2.cmp(&a.2))
                .then(b.1.cmp(&a.1))
        });
        dep.version = candidates.into_iter().next().map(|(_, version, _)| version);
    }
}
