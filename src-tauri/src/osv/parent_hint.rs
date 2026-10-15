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

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use semver::Version;

use crate::db::Database;

use super::fix_path::{
    installed_parent_requirements, is_bundled, npm_lock_resolutions, npm_lock_tree,
    refresh_verdict, requirement_admits, strip_spec, Dialect, Manager, Refresh, Requirement,
};
use super::fix_target::LineTarget;
use super::matching::{check_version_affected, parse_version};

/// The lockfile root sentinel (`ace::scanner::ROOT_PARENT`): an edge from it
/// makes the child direct, which is not a parent to name.
const ROOT_PARENT: &str = "__root__";

/// At most this many newer-parent observations per link.
const MAX_RESOLUTIONS: usize = 3;

/// One stored edge into the package, path-normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeObs {
    pub project: String,
    pub parent: String,
    pub parent_version: Option<String>,
    /// The edge's raw child spec: a resolved version (Cargo.lock, pnpm), a
    /// requirement (package-lock.json), or `None` (Cargo.lock, unique copy).
    pub child_spec: Option<String>,
}

/// One installed instance (of the package or of one of its parents).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledObs {
    pub project: String,
    /// Lowercased.
    pub package: String,
    pub version: String,
    pub is_direct: bool,
}

/// A newer parent release a project here resolved, with the child it pulled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub parent_version: String,
    pub child_version: String,
    pub project: String,
}

/// One vulnerable transitive copy and the installed parent that brings it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentLink {
    pub project: String,
    pub child_version: String,
    pub target: Option<String>,
    pub parent: String,
    pub parent_version: String,
    pub parent_is_direct: bool,
    /// The parent's requirement on the child as read (lockfile edge or the
    /// parent's installed manifest), for display. `None` when none was read.
    pub requirement: Option<String>,
    /// Whether a lockfile refresh reaches `target` (`osv::fix_path`).
    pub refresh: Refresh,
    /// The refresh command for this project's tool, when `refresh` says a
    /// refresh is enough and the tool has one.
    pub command: Option<String>,
    /// Newer releases of `parent` resolved elsewhere on this machine to a
    /// child version no advisory 4DA holds affects, lowest parent first.
    pub resolutions: Vec<Resolution>,
    /// The DIRECT dependency `parent` arrives through, when `parent` is not
    /// itself declared: the package the project can actually move.
    pub through: Option<Through>,
    /// What the registry rows 4DA holds say about newer `parent` releases.
    pub standing: ParentStanding,
    /// How this project's tool forces the copy past the parent's
    /// requirement (`fix_path::override_hint`), worded to follow "with".
    pub override_hint: Option<String>,
}

/// What the registry rows 4DA already holds say about releases of a parent
/// newer than the installed one. Read offline from `source_items` (the npm,
/// crates.io, PyPI and Go registry adapters): never a network call, and a
/// package 4DA does not watch is `Unknown`, which claims nothing.
///
/// Live 2026-10-12: Preemption said "request itself must be updated" for
/// uuid 3.4.0, but request 2.88.2 is its last release and is deprecated; no
/// update exists to take.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParentStanding {
    /// No registry row for this package.
    #[default]
    Unknown,
    /// The registry showed a release above the installed one. Whether it
    /// admits the fix is not on disk.
    Newer { latest: String },
    /// The installed version is the newest release the registry showed.
    Newest { deprecated: bool },
    /// The registry marks the package deprecated; no version was recorded.
    Deprecated,
}

/// A direct dependency a transitive parent arrives through.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Through {
    pub name: String,
    pub version: String,
    pub standing: ParentStanding,
}

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
    standing_from(&rows, name, installed)
}

/// The pure core of [`parent_standing`]: `rows` are `(id, title)` registry
/// rows, oldest first. The newest row decides deprecation (a package can be
/// un-deprecated); the highest version recorded decides the rest.
pub fn standing_from(rows: &[(i64, String)], name: &str, installed: &str) -> ParentStanding {
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

/// How far up the lockfile graph [`direct_through`] climbs.
const MAX_THROUGH_DEPTH: usize = 6;

/// The direct dependency `parent` `parent_version` reaches `project`
/// through: walk `dependency_edges` up, keeping only edges backed by an
/// installed copy, lowest name first at each level (deterministic). `None`
/// when no direct ancestor can be read.
fn direct_through(
    db: &Database,
    ecosystem_norm: &str,
    project: &str,
    parent: &str,
    parent_version: &str,
) -> Option<(String, String)> {
    let mut frontier: Vec<(String, String)> =
        vec![(parent.to_string(), parent_version.to_string())];
    let mut seen: Vec<String> = vec![parent.to_lowercase()];
    for _ in 0..MAX_THROUGH_DEPTH {
        let mut next: Vec<(String, String)> = Vec::new();
        for (child, child_version) in &frontier {
            let mut edges: Vec<EdgeObs> = read_edges(db, ecosystem_norm, child)
                .unwrap_or_default()
                .into_iter()
                .filter(|e| e.project == project && e.parent != ROOT_PARENT)
                .filter(|e| {
                    e.child_spec
                        .as_deref()
                        .is_none_or(|s| requirement_admits(s, child_version) != Some(false))
                })
                .collect();
            edges.sort_by(|a, b| a.parent.cmp(&b.parent));
            for edge in edges {
                let name = edge.parent.to_lowercase();
                if seen.contains(&name) {
                    continue;
                }
                let installed: Vec<InstalledObs> = db
                    .get_package_instances(ecosystem_norm, &edge.parent)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|r| InstalledObs {
                        project: normalize_path(&r.project_path),
                        package: r.package_name.to_lowercase(),
                        version: r.version,
                        is_direct: r.is_direct,
                    })
                    .collect();
                let Some(copy) = installed_parent(&installed, &edge) else {
                    continue;
                };
                seen.push(name);
                if copy.is_direct {
                    return Some((edge.parent.clone(), copy.version.clone()));
                }
                next.push((edge.parent.clone(), copy.version.clone()));
            }
        }
        if next.is_empty() {
            return None;
        }
        frontier = next;
    }
    None
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
            link.through = direct_through(
                db,
                ecosystem_norm,
                &link.project,
                &link.parent,
                &link.parent_version,
            )
            .map(|(name, version)| Through {
                standing: parent_standing(db, ecosystem_norm, &name, &version),
                name,
                version,
            });
        }
    }
}

impl ParentLink {
    /// The copy ships inside `parent`'s tarball (a bundled npm dependency):
    /// no refresh and no override moves it, only a new `parent` release.
    pub fn is_bundled(&self) -> bool {
        self.requirement.as_deref().is_some_and(is_bundled)
    }
}

/// A transitive copy for which no installed parent could be read (yarn
/// lockfiles carry no edges; a parent may not be installed). Its route rests
/// on semver compatibility alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlinkedSite {
    pub project: String,
    pub installed: String,
    pub target: String,
    pub refresh: Refresh,
    pub command: Option<String>,
}

/// Every transitive copy of one plan row: the ones with a readable parent,
/// and the ones without.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransitiveRoutes {
    pub links: Vec<ParentLink>,
    pub unlinked: Vec<UnlinkedSite>,
}

/// What the pure core may read from disk: the tool owning a project's
/// lockfile, and an installed parent's own requirements on a child.
pub struct Readers<'a> {
    pub manager: &'a dyn Fn(&str) -> Option<Manager>,
    /// `(project, manager, parent, parent_version, child)`.
    pub requirements: &'a dyn Fn(&str, Manager, &str, &str, &str) -> Vec<Requirement>,
    /// `(project, parent, parent_version, child)`: the child versions that
    /// installed parent resolves to, read from the lockfile tree where it
    /// records one (npm v2/v3); `None` keeps the range-based attribution.
    pub resolved_children: &'a dyn Fn(&str, &str, &str, &str) -> Option<Vec<String>>,
}

#[cfg(test)]
impl Readers<'_> {
    /// Nothing on disk: only what the edges themselves record.
    pub const NONE: Readers<'static> = Readers {
        manager: &|_| None,
        requirements: &|_, _, _, _, _| Vec::new(),
        resolved_children: &|_, _, _, _| None,
    };
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
    let requirements = |project: &str, m: Manager, parent: &str, version: &str, child: &str| {
        installed_parent_requirements(m, Path::new(&raw(project)), parent, version, child)
    };
    let resolved_children = |project: &str, parent: &str, version: &str, child: &str| {
        let tree = npm_lock_tree(Path::new(&raw(project)))?;
        npm_lock_resolutions(&tree, parent, version, child)
    };
    let readers = Readers {
        manager: &manager,
        requirements: &requirements,
        resolved_children: &resolved_children,
    };
    let mut links = parent_links_with(package, lines, &edges, &installed, &is_clear, &readers);
    annotate(db, ecosystem_norm, &mut links, &manager);
    let unlinked = unlinked_sites(package, lines, &links, &manager);
    TransitiveRoutes { links, unlinked }
}

fn read_edges(db: &Database, ecosystem_norm: &str, package: &str) -> Option<Vec<EdgeObs>> {
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

/// The pure core of [`load_parent_links`], with nothing read from disk.
#[cfg(test)]
pub fn parent_links(
    package: &str,
    lines: &[LineTarget],
    edges: &[EdgeObs],
    installed: &[InstalledObs],
    is_clear: &dyn Fn(&str) -> bool,
) -> Vec<ParentLink> {
    parent_links_with(package, lines, edges, installed, is_clear, &Readers::NONE)
}

/// The pure core of [`load_parent_links`].
pub fn parent_links_with(
    package: &str,
    lines: &[LineTarget],
    edges: &[EdgeObs],
    installed: &[InstalledObs],
    is_clear: &dyn Fn(&str) -> bool,
    readers: &Readers<'_>,
) -> Vec<ParentLink> {
    let child = package.to_lowercase();
    let mut out: Vec<ParentLink> = Vec::new();
    for line in lines {
        for site in line.sites.iter().filter(|s| !s.is_direct) {
            let child_copies = versions_in(installed, &site.project_path, &child);
            let manager = (readers.manager)(&site.project_path);
            for edge in edges.iter().filter(|e| e.project == site.project_path) {
                if edge.parent == ROOT_PARENT || edge.parent.eq_ignore_ascii_case(package) {
                    continue;
                }
                // The lockfile tree, where it records one, says which copy this
                // parent holds; a range only says which copies it could.
                let resolved = match (edge.child_spec.as_deref(), edge.parent_version.as_deref()) {
                    (Some(spec), _) if is_bundled(spec) => None,
                    (_, Some(pv)) => {
                        (readers.resolved_children)(&site.project_path, &edge.parent, pv, &child)
                    }
                    _ => None,
                };
                let leads_here = match (resolved, edge.child_spec.as_deref()) {
                    (Some(versions), _) => versions.contains(&line.installed_version),
                    (None, Some(spec)) => {
                        requirement_admits(spec, &line.installed_version) == Some(true)
                    }
                    (None, None) => child_copies.len() == 1,
                };
                if !leads_here {
                    continue;
                }
                let Some(parent) = installed_parent(installed, edge) else {
                    continue;
                };
                let requirements =
                    edge_requirements(edge, &parent.version, &child, manager, readers);
                let target = line.target_version.as_deref();
                let refresh = target.map_or(Refresh::Unknown, |t| {
                    refresh_verdict(&line.installed_version, t, &requirements)
                });
                let link = ParentLink {
                    project: site.project_path.clone(),
                    child_version: line.installed_version.clone(),
                    target: line.target_version.clone(),
                    parent: edge.parent.clone(),
                    parent_version: parent.version.clone(),
                    parent_is_direct: parent.is_direct,
                    requirement: display_requirements(&requirements),
                    refresh,
                    command: refresh_command(
                        manager,
                        refresh,
                        package,
                        &line.installed_version,
                        target,
                    ),
                    resolutions: resolutions(
                        &child,
                        edge,
                        &parent.version,
                        edges,
                        installed,
                        is_clear,
                    ),
                    through: None,
                    standing: ParentStanding::Unknown,
                    override_hint: None,
                };
                if !out.iter().any(|l| same_link(l, &link)) {
                    out.push(link);
                }
            }
        }
    }
    out
}

/// The requirements an edge's parent places on the child: the lockfile's own
/// record when the tool writes requirements (npm `package-lock.json`, or any
/// range-shaped spec when the tool is unknown), else the parent's installed
/// manifest. A resolution (Cargo.lock, pnpm) is never read as a pin.
fn edge_requirements(
    edge: &EdgeObs,
    parent_version: &str,
    child: &str,
    manager: Option<Manager>,
    readers: &Readers<'_>,
) -> Vec<Requirement> {
    let spec = edge.child_spec.as_deref().filter(|s| !s.trim().is_empty());
    match (spec, manager) {
        // A bundled copy's pin is the owner's tarball itself, whatever the tool.
        (Some(spec), _) if is_bundled(spec) => vec![Requirement::npm(spec)],
        (Some(spec), Some(m)) if m.records_requirements() => vec![Requirement::npm(spec)],
        (Some(spec), None) if is_range(spec) => vec![Requirement::npm(spec)],
        (_, Some(m)) => {
            (readers.requirements)(&edge.project, m, &edge.parent, parent_version, child)
        }
        _ => Vec::new(),
    }
}

/// "^1.2.5", or Cargo's `0.8.1` shown as the caret it means (`^0.8.1`).
fn display_requirements(requirements: &[Requirement]) -> Option<String> {
    let shown: Vec<String> = requirements
        .iter()
        .map(|r| match r.dialect {
            Dialect::Cargo if r.text.starts_with(|c: char| c.is_ascii_digit()) => {
                format!("^{}", r.text)
            }
            _ => r.text.clone(),
        })
        .collect();
    (!shown.is_empty()).then(|| shown.join(" and "))
}

fn refresh_command(
    manager: Option<Manager>,
    refresh: Refresh,
    package: &str,
    installed: &str,
    target: Option<&str>,
) -> Option<String> {
    if !refresh.is_enough() {
        return None;
    }
    manager?.refresh_command(package, installed, target?)
}

/// The transitive sites with a target that no link covers, judged by semver
/// compatibility alone (no requirement could be read).
pub fn unlinked_sites(
    package: &str,
    lines: &[LineTarget],
    links: &[ParentLink],
    manager: &dyn Fn(&str) -> Option<Manager>,
) -> Vec<UnlinkedSite> {
    let mut out = Vec::new();
    for line in lines {
        let Some(target) = line.target_version.as_deref() else {
            continue;
        };
        for site in line.sites.iter().filter(|s| !s.is_direct) {
            let covered = links.iter().any(|l| {
                l.project == site.project_path && l.child_version == line.installed_version
            });
            if covered {
                continue;
            }
            let refresh = refresh_verdict(&line.installed_version, target, &[]);
            out.push(UnlinkedSite {
                project: site.project_path.clone(),
                installed: line.installed_version.clone(),
                target: target.to_string(),
                refresh,
                command: refresh_command(
                    manager(&site.project_path),
                    refresh,
                    package,
                    &line.installed_version,
                    Some(target),
                ),
            });
        }
    }
    out
}

fn same_link(a: &ParentLink, b: &ParentLink) -> bool {
    a.project == b.project
        && a.child_version == b.child_version
        && a.parent == b.parent
        && a.parent_version == b.parent_version
}

/// The installed instance backing an edge's parent in the edge's project:
/// its recorded version, or the only installed version when none is recorded.
fn installed_parent<'a>(installed: &'a [InstalledObs], edge: &EdgeObs) -> Option<&'a InstalledObs> {
    let parent = edge.parent.to_lowercase();
    let mut copies = installed
        .iter()
        .filter(|i| i.project == edge.project && i.package == parent);
    match edge.parent_version.as_deref() {
        Some(pv) => copies.find(|i| i.version == pv),
        None => {
            let first = copies.next()?;
            copies.next().is_none().then_some(first)
        }
    }
}

fn versions_in<'a>(installed: &'a [InstalledObs], project: &str, package: &str) -> Vec<&'a str> {
    let mut v: Vec<&str> = installed
        .iter()
        .filter(|i| i.project == project && i.package == package)
        .map(|i| i.version.as_str())
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Newer releases of `edge.parent` that a project here resolved to a child
/// version no advisory affects. The child version must be unambiguous: the
/// edge names it exactly, or the project holds exactly one copy.
fn resolutions(
    child: &str,
    edge: &EdgeObs,
    parent_version: &str,
    edges: &[EdgeObs],
    installed: &[InstalledObs],
    is_clear: &dyn Fn(&str) -> bool,
) -> Vec<Resolution> {
    let Some(current) = parse_version(parent_version) else {
        return Vec::new();
    };
    let mut found: BTreeMap<Version, Resolution> = BTreeMap::new();
    for other in edges
        .iter()
        .filter(|e| e.parent.eq_ignore_ascii_case(&edge.parent))
    {
        let Some(parent) = installed_parent(installed, other) else {
            continue;
        };
        let Some(newer) = parse_version(&parent.version).filter(|v| *v > current) else {
            continue;
        };
        if found.contains_key(&newer) {
            continue;
        }
        let copies = versions_in(installed, &other.project, child);
        let resolved = match other.child_spec.as_deref() {
            Some(spec) if !is_range(spec) => {
                let exact = strip_spec(spec);
                copies.iter().find(|c| **c == exact).copied()
            }
            Some(spec) => match copies.as_slice() {
                [only] if requirement_admits(spec, only) == Some(true) => Some(*only),
                _ => None,
            },
            None => match copies.as_slice() {
                [only] => Some(*only),
                _ => None,
            },
        };
        let Some(resolved) = resolved.filter(|v| is_clear(v)) else {
            continue;
        };
        found.insert(
            newer,
            Resolution {
                parent_version: parent.version.clone(),
                child_version: resolved.to_string(),
                project: other.project.clone(),
            },
        );
    }
    found.into_values().take(MAX_RESOLUTIONS).collect()
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .to_lowercase()
        .trim_end_matches('/')
        .to_string()
}

/// A requirement range, as opposed to one exact version (a resolution or a pin).
fn is_range(spec: &str) -> bool {
    Version::parse(strip_spec(spec)).is_err()
}

#[cfg(test)]
#[path = "parent_hint_tests.rs"]
mod tests;
