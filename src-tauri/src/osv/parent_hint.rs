// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Which parent package brings a transitive vulnerable copy in, what the
//! user's own files say about its requirement, and what a newer parent did.
//!
//! An upgrade step for a transitive copy used to read "Fixed only upstream —
//! awaits a parent-package update or lockfile refresh" and stop there. Live
//! 2026-10-02, Preemption's rmcp step: rmcp 1.7.0 reaches the bridge project
//! only through victauri-plugin 0.8.4, and another project on the same
//! machine runs victauri-plugin 0.9.0 resolved against rmcp 3.4.1. Both facts
//! were in `dependency_edges` and `dependency_instances`; the step named
//! neither.
//!
//! Everything here is read from the user's own machine (accuracy first —
//! the brief once invented fix versions, 2026-09-10):
//! - a parent is named only when its edge is backed by an installed instance
//!   of that parent in the same project (edges are upserted and never pruned
//!   per rescan, so stale versions linger) and the edge provably leads to the
//!   vulnerable copy;
//! - whether a lockfile refresh is enough is decided by the ONE rule in
//!   `osv::fix_path`: the parent's requirement where it can be read (the
//!   `package-lock.json` edge, else the parent's installed manifest), else
//!   semver compatibility — and the text says which;
//! - a newer parent is cited only where a project actually resolved it, and
//!   only when that resolved child version is outside every advisory 4DA
//!   holds for the package;
//! - whether a newer parent release exists at all is read from the registry
//!   rows 4DA already holds ([`ParentStanding`]), never fetched: no row, no
//!   claim either way.
//!
//! The link rules moved into the engine crate (`fourda_depengine::parent_hint`,
//! single-engine C0.4) and are re-exported here under their old paths. This
//! module keeps the database half: the edges and installed copies it reads,
//! the registry rows behind [`ParentStanding`], the walk up to the declared
//! dependencies (`osv::parent_route`), and the disk reads the engine asks for
//! (`osv::fix_path::answer_need`).

use std::collections::HashMap;
use std::path::Path;

use fourda_depengine::needs::resolve;
use fourda_depengine::parent_hint::{parent_links_lookup, Projects};

use crate::db::Database;

use super::fix_path::{answer_need, host_join, DetectManager, Manager};
use super::fix_target::LineTarget;
use super::matching::{check_version_affected, parse_version};

#[cfg(test)]
pub use fourda_depengine::parent_hint::Resolution;
pub(super) use fourda_depengine::parent_hint::{installed_parent, normalize_path};
pub use fourda_depengine::parent_hint::{
    unlinked_sites, EdgeObs, InstalledObs, ParentLink, ParentRelease, ParentStanding, Through,
    TransitiveRoutes, UnlinkedSite, MAX_LEVERS,
};

/// The registry `source_type`s and title prefix for an OSV-canonical
/// ecosystem (the adapters' own title shapes, `dep_linker::registry_release_identity`).
fn registry_shape(ecosystem_norm: &str) -> Option<(&'static [&'static str], &'static str)> {
    match ecosystem_norm {
        "npm" => Some((&["npm_registry", "npm"], "npm: ")),
        "crates.io" => Some((&["crates_io", "crates"], "crates.io: ")),
        "PyPI" => Some((&["pypi"], "PyPI: ")),
        "Go" => Some((&["go_modules", "go"], "Go: ")),
        _ => None,
    }
}

/// [`ParentStanding`] of `name` at `installed`, from the registry rows held.
pub fn parent_standing(
    db: &Database,
    ecosystem_norm: &str,
    name: &str,
    installed: &str,
) -> ParentStanding {
    let Some((types, prefix)) = registry_shape(ecosystem_norm) else {
        return ParentStanding::Unknown;
    };
    let rows: Vec<(i64, String)> = {
        let conn = db.conn.lock();
        let list = types
            .iter()
            .map(|t| format!("'{t}'"))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, title FROM source_items
             WHERE source_type IN ({list}) AND (title LIKE ?1 OR title = ?2)
             ORDER BY id"
        );
        conn.prepare(&sql)
            .and_then(|mut stmt| {
                stmt.query_map(
                    rusqlite::params![format!("{prefix}{name} %"), format!("[DEPRECATED] {name}")],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map(|rows| rows.filter_map(Result::ok).collect())
            })
            .unwrap_or_default()
    };
    // The newest release named is one a user can take: listed by the
    // registry facts or the feed, not withdrawn, and passing the one
    // predicate every reader shares (`parent_release::Advisories::usable`:
    // never malware, never an advisory with a fix). Review of #981.
    let book = super::release_facts::book(db);
    let registry = book.registry(ecosystem_norm, name);
    let advisories = super::parent_release::Advisories::stored(
        ecosystem_norm,
        db.get_osv_advisories_for_package(name, ecosystem_norm)
            .unwrap_or_default(),
    );
    let releases = book.releases(ecosystem_norm, name).unwrap_or_default();
    let newest = advisories.newest(&releases);
    let usable = |v: &str| {
        !registry.excludes(ecosystem_norm, v)
            && advisories.usable(v, newest.as_deref(), Some(installed))
    };
    let listed = releases.versions.clone();
    let standing = standing_from_with(&rows, name, installed, &listed, &usable);
    // Review of #994, B3: "already the newest release", or nothing known at
    // all, is false when newer releases exist and only advisories the
    // installed version lacks rule them out; then that is what is said
    // (parity decision 2). A deprecated package keeps its deprecated outcome
    // first, as the MCP decides (#1008 review, S3).
    if !matches!(
        standing,
        ParentStanding::Newest { deprecated: false } | ParentStanding::Unknown
    ) {
        return standing;
    }
    let newer = super::parent_release::ascending(&releases.versions, ecosystem_norm)
        .into_iter()
        .filter(|v| !registry.excludes(ecosystem_norm, v))
        .filter(|v| {
            super::parent_release::compare(v, installed, ecosystem_norm)
                == Some(std::cmp::Ordering::Greater)
        })
        .map(String::as_str);
    let ids = advisories.introduced_union(newer, newest.as_deref(), installed);
    if ids.is_empty() {
        standing
    } else {
        ParentStanding::Introduces { advisories: ids }
    }
}

/// The pure core of [`parent_standing`]: `rows` are `(id, title)` registry
/// rows, oldest first. The newest row decides deprecation (a package can be
/// un-deprecated); the highest version recorded decides the rest.
#[cfg(test)]
pub fn standing_from(rows: &[(i64, String)], name: &str, installed: &str) -> ParentStanding {
    standing_from_with(rows, name, installed, &[], &|_| true)
}

/// [`standing_from`] with the registry's own release list (`listed`) joined
/// to the feed rows, and only the releases `usable` accepts counted as the
/// newest (not withdrawn, clear of the package's advisories).
pub fn standing_from_with(
    rows: &[(i64, String)],
    name: &str,
    installed: &str,
    listed: &[String],
    usable: &dyn Fn(&str) -> bool,
) -> ParentStanding {
    let deprecated_title = format!("[DEPRECATED] {name}");
    let mine: Vec<&(i64, String)> = rows
        .iter()
        .filter(|(_, t)| {
            t.eq_ignore_ascii_case(&deprecated_title)
                || crate::dep_linker::registry_release_identity(t)
                    .is_some_and(|(n, _)| crate::dep_linker::registry_names_equal(&n, name))
        })
        .collect();
    let deprecated = mine
        .iter()
        .max_by_key(|(id, _)| *id)
        .is_some_and(|(_, t)| t.eq_ignore_ascii_case(&deprecated_title));
    let newest = mine
        .iter()
        .filter_map(|(_, t)| crate::dep_linker::registry_release_identity(t)?.1)
        .chain(listed.iter().cloned())
        .filter(|v| usable(v))
        .filter_map(|v| {
            parse_version(&v)
                .filter(|p| p.pre.is_empty())
                .map(|p| (p, v))
        })
        .max_by(|a, b| a.0.cmp(&b.0));
    match (newest, parse_version(installed)) {
        (Some((latest, raw)), Some(inst)) if latest > inst => ParentStanding::Newer { latest: raw },
        (Some(_), Some(_)) => ParentStanding::Newest { deprecated },
        _ if deprecated => ParentStanding::Deprecated,
        _ => ParentStanding::Unknown,
    }
}

/// Fill what the registry rows and the graph above the parent say: its
/// standing, the direct dependency it arrives through, the override hint.
fn annotate(
    db: &Database,
    ecosystem_norm: &str,
    links: &mut [ParentLink],
    manager: &dyn Fn(&str) -> Option<Manager>,
) {
    // Only a link whose parent must move names a parent move: a
    // refresh-enough link needs neither the graph above it nor the registry.
    for link in links.iter_mut().filter(|l| !l.refresh.is_enough()) {
        link.standing = parent_standing(db, ecosystem_norm, &link.parent, &link.parent_version);
        link.override_hint = super::fix_path::override_hint(
            manager(&link.project),
            ecosystem_norm,
            link.is_bundled(),
        );
        if !link.parent_is_direct {
            let route = super::parent_route::declared_route(
                db,
                ecosystem_norm,
                &link.project,
                &link.parent,
                &link.parent_version,
            );
            (link.through, link.lever_unknown) = levers(db, ecosystem_norm, route);
        }
    }
}

/// Every declared dependency the parent arrives through (the first carries
/// the others), each with its registry-row standing; or why none can be named.
fn levers(
    db: &Database,
    ecosystem_norm: &str,
    route: super::parent_route::Route,
) -> (Option<Through>, Option<String>) {
    let all = match route {
        super::parent_route::Route::Levers(all) => all,
        super::parent_route::Route::Unknown(why) => return (None, Some(why)),
    };
    let mut named: Vec<Through> = all
        .iter()
        .take(MAX_LEVERS)
        .map(|(name, version)| Through {
            standing: parent_standing(db, ecosystem_norm, name, version),
            ..Through::declared(name, version)
        })
        .collect();
    if named.is_empty() {
        return (None, None);
    }
    let mut first = named.remove(0);
    first.also = named;
    first.more = all.len().saturating_sub(MAX_LEVERS);
    (Some(first), None)
}

/// The parent links for every TRANSITIVE site of `lines`, read from the DB.
/// Empty when no line has a transitive site (no query runs) or on any read
/// failure — a missing hint only falls back to the generic text.
pub fn load_parent_links(
    db: &Database,
    ecosystem_norm: &str,
    package: &str,
    lines: &[LineTarget],
) -> Vec<ParentLink> {
    load_routes(db, ecosystem_norm, package, lines).links
}

/// [`load_parent_links`] plus the transitive sites no parent was read for.
pub fn load_routes(
    db: &Database,
    ecosystem_norm: &str,
    package: &str,
    lines: &[LineTarget],
) -> TransitiveRoutes {
    load_routes_with(db, ecosystem_norm, package, lines, true)
}

/// [`load_routes`]; `releases: false` leaves out the registry-read parent
/// releases (`osv::parent_release_load`), for the walk that decides which
/// parents' requirements to read.
pub(super) fn load_routes_with(
    db: &Database,
    ecosystem_norm: &str,
    package: &str,
    lines: &[LineTarget],
    releases: bool,
) -> TransitiveRoutes {
    let any_transitive = lines.iter().any(|l| l.sites.iter().any(|s| !s.is_direct));
    if !any_transitive {
        return TransitiveRoutes::default();
    }
    let edges = read_edges(db, ecosystem_norm, package).unwrap_or_default();
    let mut names: Vec<String> = edges.iter().map(|e| e.parent.clone()).collect();
    names.push(package.to_string());
    names.sort();
    names.dedup();
    let mut installed = Vec::new();
    // Normalized project path -> the path as stored (the matcher lowercases
    // it; a case-sensitive filesystem needs the original to find a lockfile).
    let mut raw_paths: HashMap<String, String> = HashMap::new();
    for name in &names {
        let Ok(rows) = db.get_package_instances(ecosystem_norm, name) else {
            return TransitiveRoutes::default();
        };
        for r in rows {
            let project = normalize_path(&r.project_path);
            raw_paths
                .entry(project.clone())
                .or_insert_with(|| r.project_path.clone());
            installed.push(InstalledObs {
                project,
                package: r.package_name.to_lowercase(),
                version: r.version,
                is_direct: r.is_direct,
            });
        }
    }
    let Ok(advisories) = db.get_osv_advisories_for_package(package, ecosystem_norm) else {
        return TransitiveRoutes::default();
    };
    let is_clear = |version: &str| {
        advisories
            .iter()
            .all(|a| !check_version_affected(ecosystem_norm, Some(version), &a.affected_ranges).0)
    };
    let raw = |project: &str| {
        raw_paths
            .get(project)
            .cloned()
            .unwrap_or_else(|| project.to_string())
    };
    let manager = |project: &str| Manager::detect(Path::new(&raw(project)), ecosystem_norm);
    // The engine reads the parents' installed manifests and the
    // `package-lock.json` tree through its `needs` round-trip; the disk
    // answers here (`fix_path::answer_need`).
    let projects = Projects {
        manager: &manager,
        dir: &raw,
        join: &host_join,
    };
    let mut links = resolve(
        |provided| {
            parent_links_lookup(
                package, lines, &edges, &installed, &is_clear, &projects, provided,
            )
        },
        answer_need,
    );
    annotate(db, ecosystem_norm, &mut links, &manager);
    if releases {
        super::parent_release_load::annotate(db, ecosystem_norm, package, &mut links, &raw);
    }
    let unlinked = unlinked_sites(package, lines, &links, &manager);
    TransitiveRoutes { links, unlinked }
}

pub(super) fn read_edges(
    db: &Database,
    ecosystem_norm: &str,
    package: &str,
) -> Option<Vec<EdgeObs>> {
    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(
            "SELECT project_path, ecosystem, parent_package, parent_version, child_version
             FROM dependency_edges
             WHERE child_package = ?1
             ORDER BY project_path, parent_package, parent_version",
        )
        .ok()?;
    let rows = stmt
        .query_map([package], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .ok()?;
    Some(
        rows.filter_map(Result::ok)
            .filter(|(_, eco, ..)| {
                crate::ecosystem::Ecosystem::parse(eco)
                    .is_some_and(|e| e.osv_name() == ecosystem_norm)
            })
            .map(|(project, _, parent, parent_version, child_spec)| EdgeObs {
                project: normalize_path(&project),
                parent,
                parent_version,
                child_spec,
            })
            .collect(),
    )
}

#[cfg(test)]
#[path = "parent_hint_tests.rs"]
mod tests;
