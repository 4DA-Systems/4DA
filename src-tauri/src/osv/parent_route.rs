// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Which declared dependencies a transitive parent arrives through: the
//! packages the project can actually move (review of #981, S2; the MCP's
//! `findRoute`). Every path up `dependency_edges` from the parent is walked to
//! the first package a project declares; all of them are named (each must
//! move). When any path ends at a package nobody declares, or the walk runs
//! past its bound, the lever is unknown and no command is named: naming only
//! the paths that resolved would leave the others pinning the old parent.

use crate::db::Database;

use super::fix_path::requirement_admits;
use super::parent_hint::{installed_parent, normalize_path, read_edges, EdgeObs, InstalledObs};

/// The lockfile root sentinel (`fourda_depengine::lockfile::graph::ROOT_PARENT`).
const ROOT_PARENT: &str = "__root__";
/// Graph entries visited before the walk gives up (the MCP's
/// `MAX_THROUGH_NODES`).
const MAX_NODES: usize = 40;

/// The declared dependencies above a parent, or why none can be named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Route {
    /// Every declared dependency the parent arrives through, by name.
    Levers(Vec<(String, String)>),
    Unknown(String),
}

/// One package's installed parents in `project`, each backed by an installed
/// copy whose requirement admits `version`.
fn parents_of(
    db: &Database,
    eco: &str,
    project: &str,
    name: &str,
    version: &str,
) -> (Vec<InstalledObs>, bool) {
    let edges: Vec<EdgeObs> = read_edges(db, eco, name)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.project == project)
        .filter(|e| {
            e.child_spec
                .as_deref()
                .is_none_or(|s| requirement_admits(s, version) != Some(false))
        })
        .collect();
    let rooted = edges.iter().any(|e| e.parent == ROOT_PARENT);
    let mut out: Vec<InstalledObs> = Vec::new();
    for edge in edges.iter().filter(|e| e.parent != ROOT_PARENT) {
        let installed: Vec<InstalledObs> = db
            .get_package_instances(eco, &edge.parent)
            .unwrap_or_default()
            .into_iter()
            .map(|r| InstalledObs {
                project: normalize_path(&r.project_path),
                package: r.package_name.to_lowercase(),
                version: r.version,
                is_direct: r.is_direct,
            })
            .collect();
        if let Some(copy) = installed_parent(&installed, edge) {
            if !out.contains(copy) {
                out.push(copy.clone());
            }
        }
    }
    (out, rooted)
}

/// Walk up from `parent` `parent_version` in `project` to every declared
/// dependency above it.
pub(crate) fn declared_route(
    db: &Database,
    eco: &str,
    project: &str,
    parent: &str,
    parent_version: &str,
) -> Route {
    let mut queue: Vec<(String, String)> = vec![(parent.to_string(), parent_version.to_string())];
    let mut seen: Vec<String> = Vec::new();
    let mut levers: Vec<(String, String)> = Vec::new();
    while let Some((name, version)) = queue.pop() {
        let key = format!("{}@{version}", name.to_lowercase());
        if seen.contains(&key) {
            continue;
        }
        if seen.len() >= MAX_NODES {
            return Route::Unknown(format!(
                "lever not determined (dependency graph too large: the walk up from {parent} \
                 stopped after {MAX_NODES} entries)"
            ));
        }
        seen.push(key);
        let (parents, rooted) = parents_of(db, eco, project, &name, &version);
        // The project's own requirement (the lockfile root edge) counts with
        // the other parents: a package the project requires that no manifest
        // declares is a path no declared lever covers.
        if rooted {
            return Route::Unknown(format!(
                "{name} {version} is required by the project, but no manifest declares it"
            ));
        }
        if parents.is_empty() {
            return Route::Unknown(format!("{name} {version} leads to no declared dependency"));
        }
        for p in parents {
            if p.is_direct {
                if !levers
                    .iter()
                    .any(|(n, v)| n.eq_ignore_ascii_case(&p.package) && *v == p.version)
                {
                    levers.push((p.package.clone(), p.version.clone()));
                }
            } else {
                queue.push((p.package.clone(), p.version.clone()));
            }
        }
    }
    levers.sort();
    if levers.is_empty() {
        return Route::Unknown(format!(
            "no declared dependency above {parent} in the lockfile"
        ));
    }
    Route::Levers(levers)
}
