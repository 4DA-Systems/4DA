// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One directory's lockfiles, merged per ecosystem and written ONCE.
//!
//! Each format used to have its own processor, and each one REPLACED the
//! directory's whole `(project, ecosystem)` slice: `store_dependency_instances`
//! deletes before it inserts, and the per-file prune deleted every
//! `user_dependencies` row its own file did not name. A directory with
//! `poetry.lock` and `requirements.txt` kept only the requirements pins; one
//! with `package-lock.json` and `yarn.lock` kept only the yarn copies. Now
//! every lockfile's copies are merged first (scopes combined, the project's
//! own resolution kept as the collapsed row's version), then stored and
//! pruned against the union.
//!
//! Dev scope comes from the lockfile graph (AD-046): an instance is dev only
//! when its lockfile says so and nothing reaches it at runtime; unknown is
//! never dev. The collapsed per-name row is dev only when EVERY copy is.

use std::collections::{HashMap, HashSet};

use crate::ace::lockfile::{DepScope, LockedPackage, LockfileRead};
use crate::db::{Database, DependencyInstanceInput};

use super::prune_stale_rows;

/// Every copy the reads name, once per `(name, version)`: scopes merged
/// (runtime > dev > unknown) and `primary` kept if any read marks it.
pub(super) fn merge_reads(reads: &[&LockfileRead]) -> Vec<LockedPackage> {
    let mut index: HashMap<(String, String), usize> = HashMap::new();
    let mut merged: Vec<LockedPackage> = Vec::new();
    for pkg in reads.iter().flat_map(|r| r.packages.iter()) {
        let key = (pkg.name.clone(), pkg.version.clone());
        match index.get(&key) {
            Some(&i) => {
                merged[i].scope = merged[i].scope.merge(pkg.scope);
                merged[i].primary |= pkg.primary;
                // A version held in the root slot AND a member's is the root
                // slot's copy: any workspace package resolves to it.
                if pkg.top_level && (pkg.owner.is_none() || !merged[i].top_level) {
                    merged[i].owner.clone_from(&pkg.owner);
                }
                merged[i].top_level |= pkg.top_level;
                merged[i].specs.extend(pkg.specs.iter().cloned());
            }
            None => {
                index.insert(key, merged.len());
                merged.push(pkg.clone());
            }
        }
    }
    merged
}

/// What one directory's manifests declare for an ecosystem.
#[derive(Debug, Default)]
pub(super) struct Declarations {
    /// Every declared name, runtime and dev: what makes a placed copy direct.
    pub names: Vec<String>,
    /// `name@range` declarations (JavaScript), for placing yarn.lock copies.
    pub specs: Vec<String>,
    /// Names the manifest declares ONLY for development (PEP 735
    /// `[dependency-groups]`, uv `dev-dependencies`, Poetry dev groups).
    pub dev_only: Vec<String>,
    /// JavaScript workspaces: what the root manifest itself declares...
    pub root: Vec<String>,
    /// ...and what each member declares. Empty for a single-root project,
    /// where `names` is the root's own list.
    pub members: Vec<MemberDeclarations>,
}

/// One workspace member's declarations, and the spellings a lockfile uses
/// for its slot (npm: its directory relative to the root, `packages/ui`;
/// bun: its package name).
#[derive(Debug, Default)]
pub(super) struct MemberDeclarations {
    pub keys: Vec<String>,
    pub names: Vec<String>,
}

/// Whether each copy is DIRECT (A-3): it sits where a declaration of its
/// name resolves. A member's own copy is direct when THAT member declares
/// the name. A root-slot copy is direct when the root declares it, or a
/// member does that has no copy of its own (npm and bun hoist a member's
/// dependency to the root; a member with its own nested copy resolves to
/// that one instead). A copy in no declared slot is a dependency's.
fn direct_copies(
    packages: &[LockedPackage],
    ecosystem: &str,
    declared: &Declarations,
) -> Vec<bool> {
    let keys = |names: &[String]| -> HashSet<String> {
        names.iter().map(|n| name_key(ecosystem, n)).collect()
    };
    let all = keys(&declared.names);
    if declared.members.is_empty() {
        return packages
            .iter()
            .map(|p| p.top_level && all.contains(&name_key(ecosystem, &p.name)))
            .collect();
    }
    let root = keys(&declared.root);
    let members: Vec<(&[String], HashSet<String>)> = declared
        .members
        .iter()
        .map(|m| (m.keys.as_slice(), keys(&m.names)))
        .collect();
    let has_own_copy = |member_keys: &[String], name: &str| {
        packages.iter().any(|q| {
            q.top_level
                && name_key(ecosystem, &q.name) == name
                && q.owner.as_ref().is_some_and(|o| member_keys.contains(o))
        })
    };
    packages
        .iter()
        .map(|p| {
            let name = name_key(ecosystem, &p.name);
            if !p.top_level {
                return false;
            }
            match &p.owner {
                Some(owner) => members
                    .iter()
                    .find(|(k, _)| k.contains(owner))
                    .map_or(all.contains(&name), |(_, names)| names.contains(&name)),
                None => {
                    root.contains(&name)
                        || members
                            .iter()
                            .any(|(k, names)| names.contains(&name) && !has_own_copy(k, &name))
                }
            }
        })
        .collect()
}

/// A declared copy whose lockfile records no scope (uv.lock, pdm.lock, a
/// requirements pin) takes the manifest's: declared only in a development
/// group, it is dev (A-4). A lockfile that recorded a scope keeps it
/// (AD-046), and a copy that is not the declared one is not touched.
fn mark_manifest_dev(packages: &mut [LockedPackage], ecosystem: &str, dev_only: &[String]) {
    let dev: HashSet<String> = dev_only.iter().map(|d| name_key(ecosystem, d)).collect();
    for p in packages
        .iter_mut()
        .filter(|p| p.top_level && p.scope == DepScope::Unknown)
    {
        if dev.contains(&name_key(ecosystem, &p.name)) {
            p.scope = DepScope::Dev;
        }
    }
}

/// Place the copies whose lockfile records no position (yarn.lock): a copy
/// one of whose descriptors is a manifest's declared `name@range` is where
/// that declaration resolves, so it is the declared copy (and the collapsed
/// row's version). Then a declared name with NO placed copy at all keeps
/// every copy placed: the lockfile could not say which one the manifest
/// resolves to, and understating directness is not the safe failure.
pub(super) fn place_declared_copies(
    packages: &mut [LockedPackage],
    ecosystem: &str,
    direct: &[String],
    declared_specs: &[String],
) {
    let specs: HashSet<&str> = declared_specs.iter().map(String::as_str).collect();
    for p in packages.iter_mut().filter(|p| !p.top_level) {
        if p.specs.iter().any(|s| specs.contains(s.as_str())) {
            p.top_level = true;
            p.primary = true;
        }
    }
    let placed: HashSet<String> = packages
        .iter()
        .filter(|p| p.top_level)
        .map(|p| name_key(ecosystem, &p.name))
        .collect();
    let declared: HashSet<String> = direct.iter().map(|d| name_key(ecosystem, d)).collect();
    for p in packages.iter_mut() {
        let key = name_key(ecosystem, &p.name);
        if declared.contains(&key) && !placed.contains(&key) {
            p.top_level = true;
        }
    }
}

/// How the ecosystem compares a package name with a manifest's declaration.
fn name_key(ecosystem: &str, name: &str) -> String {
    if ecosystem == "python" {
        crate::osv::matching::pep503_name(name)
    } else {
        name.to_string()
    }
}

/// One collapsed `user_dependencies` row per name.
struct CollapsedRow {
    name: String,
    version: String,
    primary: bool,
    all_dev: bool,
}

fn collapse(packages: &[LockedPackage]) -> Vec<CollapsedRow> {
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut rows: Vec<CollapsedRow> = Vec::new();
    for p in packages {
        let dev = p.scope == DepScope::Dev;
        match index.get(&p.name.to_ascii_lowercase()) {
            Some(&i) => {
                let row = &mut rows[i];
                row.all_dev &= dev;
                if p.primary && !row.primary {
                    row.version.clone_from(&p.version);
                    row.primary = true;
                }
            }
            None => {
                index.insert(p.name.to_ascii_lowercase(), rows.len());
                rows.push(CollapsedRow {
                    name: p.name.clone(),
                    version: p.version.clone(),
                    primary: p.primary,
                    all_dev: dev,
                });
            }
        }
    }
    rows
}

/// Store one ecosystem's merged copies for a directory: the instance
/// inventory, one collapsed row per name, the graph edges, then the prune.
/// Returns the number of transitive (non-direct) names stored.
pub(super) fn store_ecosystem(
    db: &Database,
    project_path: &str,
    ecosystem: &str,
    reads: &[&LockfileRead],
    declared: &Declarations,
    keep: &[String],
) -> u32 {
    let direct = declared.names.as_slice();
    let mut packages = merge_reads(reads);
    place_declared_copies(&mut packages, ecosystem, direct, &declared.specs);
    mark_manifest_dev(&mut packages, ecosystem, &declared.dev_only);
    let direct_keys: HashSet<String> = direct.iter().map(|d| name_key(ecosystem, d)).collect();
    let is_direct = |name: &str| direct_keys.contains(&name_key(ecosystem, name));

    // A copy is direct only where the declaration resolves: a nested copy of
    // a declared name (`node_modules/foo/node_modules/debug`) is `foo`'s
    // transitive, and grading it direct lifted the transitive severity cap
    // and offered it a direct-upgrade command it does not have (A-3).
    let direct_flags = direct_copies(&packages, ecosystem, declared);
    let instances: Vec<DependencyInstanceInput> = packages
        .iter()
        .zip(&direct_flags)
        .map(|(p, &is_direct)| DependencyInstanceInput {
            package_name: p.name.clone(),
            version: p.version.clone(),
            is_direct,
            is_dev: p.scope == DepScope::Dev,
            scope: p.scope.as_str().to_string(),
        })
        .collect();
    db.store_dependency_instances(project_path, ecosystem, &instances)
        .ok();
    for read in reads.iter().filter(|r| !r.edges.is_empty()) {
        db.store_dependency_edges(project_path, ecosystem, &read.edges)
            .ok();
    }

    let mut count = 0u32;
    for row in collapse(&packages) {
        let version = Some(row.version.as_str());
        if is_direct(&row.name) {
            db.store_dependency(
                project_path,
                &row.name,
                version,
                ecosystem,
                row.all_dev,
                None,
            )
            .ok();
        } else {
            db.store_transitive_dependency(
                project_path,
                &row.name,
                version,
                ecosystem,
                row.all_dev,
            )
            .ok();
            count += 1;
        }
    }
    let names: Vec<(String, String)> = packages
        .iter()
        .map(|p| (p.name.clone(), p.version.clone()))
        .collect();
    let mut keep_names: Vec<String> = keep.to_vec();
    keep_names.extend(direct.iter().cloned());
    prune_stale_rows(db, project_path, ecosystem, &names, &keep_names);
    count
}

#[cfg(test)]
#[path = "dependencies_store_tests.rs"]
mod tests;
