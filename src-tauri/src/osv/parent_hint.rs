// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Which parent package brings a transitive vulnerable copy in, and what the
//! user's own lockfiles show about a newer parent.
//!
//! An upgrade step for a transitive copy used to read "Fixed only upstream —
//! awaits a parent-package update or lockfile refresh" and stop there. Live
//! 2026-10-02, Preemption's rmcp step: rmcp 1.7.0 reaches the bridge project
//! only through victauri-plugin 0.8.4, and another project on the same
//! machine runs victauri-plugin 0.9.0 resolved against rmcp 3.4.1. Both facts
//! were in `dependency_edges` and `dependency_instances`; the step named
//! neither.
//!
//! Everything here is read from the user's own lockfiles (accuracy first —
//! the brief once invented fix versions, 2026-09-10):
//! - a parent is named only when its edge is backed by an installed instance
//!   of that parent in the same project (edges are upserted and never pruned
//!   per rescan, so stale versions linger) and the edge provably leads to the
//!   vulnerable copy;
//! - "a lockfile refresh is enough" is said only when the edge records a real
//!   requirement RANGE (npm `package-lock.json`) that admits the target;
//!   Cargo.lock and pnpm record resolutions, not requirements, so no claim;
//! - a newer parent is cited only where a project actually resolved it, and
//!   only when that resolved child version is outside every advisory 4DA
//!   holds for the package. No registry metadata is consulted: 4DA does not
//!   store a release's dependency list, so a parent release nobody here has
//!   installed is never claimed to fix anything.

use std::collections::BTreeMap;

use semver::{Version, VersionReq};

use crate::db::Database;

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
    /// The parent's requirement on the child, when the lockfile records one
    /// as a range. `None` for a resolution or an unrecorded requirement.
    pub requirement: Option<String>,
    /// `requirement` admits `target`: `Some(true)` = a lockfile refresh is
    /// enough, `Some(false)` = the parent itself must move. `None` = unknown.
    pub admits_target: Option<bool>,
    /// Newer releases of `parent` resolved elsewhere on this machine to a
    /// child version no advisory 4DA holds affects, lowest parent first.
    pub resolutions: Vec<Resolution>,
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
    let any_transitive = lines.iter().any(|l| l.sites.iter().any(|s| !s.is_direct));
    if !any_transitive {
        return Vec::new();
    }
    let Some(edges) = read_edges(db, ecosystem_norm, package) else {
        return Vec::new();
    };
    if edges.is_empty() {
        return Vec::new();
    }
    let mut names: Vec<String> = edges.iter().map(|e| e.parent.clone()).collect();
    names.push(package.to_string());
    names.sort();
    names.dedup();
    let mut installed = Vec::new();
    for name in &names {
        let Ok(rows) = db.get_package_instances(ecosystem_norm, name) else {
            return Vec::new();
        };
        installed.extend(rows.into_iter().map(|r| InstalledObs {
            project: normalize_path(&r.project_path),
            package: r.package_name.to_lowercase(),
            version: r.version,
            is_direct: r.is_direct,
        }));
    }
    let Ok(advisories) = db.get_osv_advisories_for_package(package, ecosystem_norm) else {
        return Vec::new();
    };
    let is_clear = |version: &str| {
        advisories
            .iter()
            .all(|a| !check_version_affected(Some(version), &a.affected_ranges).0)
    };
    parent_links(package, lines, &edges, &installed, &is_clear)
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

/// The pure core of [`load_parent_links`].
pub fn parent_links(
    package: &str,
    lines: &[LineTarget],
    edges: &[EdgeObs],
    installed: &[InstalledObs],
    is_clear: &dyn Fn(&str) -> bool,
) -> Vec<ParentLink> {
    let child = package.to_lowercase();
    let mut out: Vec<ParentLink> = Vec::new();
    for line in lines {
        for site in line.sites.iter().filter(|s| !s.is_direct) {
            let child_copies = versions_in(installed, &site.project_path, &child);
            for edge in edges.iter().filter(|e| e.project == site.project_path) {
                if edge.parent == ROOT_PARENT || edge.parent.eq_ignore_ascii_case(package) {
                    continue;
                }
                let leads_here = match edge.child_spec.as_deref() {
                    Some(spec) => requirement_admits(spec, &line.installed_version) == Some(true),
                    None => child_copies.len() == 1,
                };
                if !leads_here {
                    continue;
                }
                let Some(parent) = installed_parent(installed, edge) else {
                    continue;
                };
                let requirement = edge
                    .child_spec
                    .as_deref()
                    .filter(|s| is_range(s))
                    .map(str::to_string);
                let admits_target = match (&requirement, &line.target_version) {
                    (Some(req), Some(target)) => requirement_admits(req, target),
                    _ => None,
                };
                let link = ParentLink {
                    project: site.project_path.clone(),
                    child_version: line.installed_version.clone(),
                    target: line.target_version.clone(),
                    parent: edge.parent.clone(),
                    parent_version: parent.version.clone(),
                    parent_is_direct: parent.is_direct,
                    requirement,
                    admits_target,
                    resolutions: resolutions(
                        &child,
                        edge,
                        &parent.version,
                        edges,
                        installed,
                        is_clear,
                    ),
                };
                if !out.iter().any(|l| same_link(l, &link)) {
                    out.push(link);
                }
            }
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

/// The spec without a pnpm peer suffix (`4.28.1(postcss@8.4.0)`), a leading
/// `=`/`v`, and surrounding whitespace.
fn strip_spec(spec: &str) -> &str {
    let spec = spec.split('(').next().unwrap_or(spec).trim();
    spec.trim_start_matches('=').trim().trim_start_matches('v')
}

/// A requirement range, as opposed to one exact version (a resolution or a pin).
fn is_range(spec: &str) -> bool {
    Version::parse(strip_spec(spec)).is_err()
}

/// Whether an npm/Cargo requirement admits `version`. `None` when either side
/// cannot be read (dist-tags, `file:`/`git` specs, aliases to another name are
/// read by their range). Never guesses: an unreadable alternative in an `||`
/// set makes a non-match unknown rather than false.
pub(crate) fn requirement_admits(spec: &str, version: &str) -> Option<bool> {
    let version = parse_version(version)?;
    let mut spec = spec.split('(').next().unwrap_or(spec).trim();
    if let Some(alias) = spec.strip_prefix("npm:") {
        spec = alias.rsplit_once('@').map(|(_, range)| range)?;
    }
    if spec.contains(':') || spec.contains('/') {
        return None;
    }
    let mut unknown = false;
    for alternative in spec.split("||") {
        match comparator_set(alternative.trim()) {
            Some(req) if req.matches(&version) => return Some(true),
            Some(_) => {}
            None => unknown = true,
        }
    }
    (!unknown).then_some(false)
}

/// One npm comparator set (space-separated, AND) as a semver `VersionReq`.
fn comparator_set(set: &str) -> Option<VersionReq> {
    if set.is_empty() || set == "*" || set.eq_ignore_ascii_case("x") {
        return Some(VersionReq::STAR);
    }
    if let Some((lo, hi)) = set.split_once(" - ") {
        let (lo, hi) = (strip_spec(lo), strip_spec(hi));
        Version::parse(lo).ok()?;
        Version::parse(hi).ok()?;
        return VersionReq::parse(&format!(">={lo}, <={hi}")).ok();
    }
    let mut comparators: Vec<String> = Vec::new();
    let mut pending_op = String::new();
    for token in set.split_whitespace() {
        if token
            .chars()
            .all(|c| matches!(c, '<' | '>' | '=' | '^' | '~'))
        {
            pending_op.push_str(token);
            continue;
        }
        let token = format!("{}{token}", std::mem::take(&mut pending_op));
        comparators.push(npm_comparator(&token)?);
    }
    if !pending_op.is_empty() || comparators.is_empty() {
        return None;
    }
    VersionReq::parse(&comparators.join(", ")).ok()
}

/// npm comparator -> Cargo comparator. A bare version is EXACT in npm (Cargo
/// would read it as a caret); a partial or x-range bare version is its X-range.
fn npm_comparator(token: &str) -> Option<String> {
    let op_len = token
        .find(|c: char| !matches!(c, '<' | '>' | '=' | '^' | '~'))
        .unwrap_or(token.len());
    let (op, rest) = token.split_at(op_len);
    let rest = rest.trim_start_matches('v');
    let parts: Vec<&str> = rest
        .split('.')
        .take_while(|p| !matches!(*p, "x" | "X" | "*"))
        .collect();
    if parts.is_empty() {
        return match op {
            "" | "=" | "^" | "~" | ">=" | "<=" => Some("*".to_string()),
            _ => None,
        };
    }
    let version = parts.join(".");
    Some(match op {
        "" => format!("={version}"),
        _ => format!("{op}{version}"),
    })
}

#[cfg(test)]
#[path = "parent_hint_tests.rs"]
mod tests;
