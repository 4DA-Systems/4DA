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
            }
            None => {
                index.insert(key, merged.len());
                merged.push(pkg.clone());
            }
        }
    }
    merged
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
    direct: &[String],
    keep: &[String],
) -> u32 {
    let packages = merge_reads(reads);
    let direct_keys: HashSet<String> = direct.iter().map(|d| name_key(ecosystem, d)).collect();
    let is_direct = |name: &str| direct_keys.contains(&name_key(ecosystem, name));

    let instances: Vec<DependencyInstanceInput> = packages
        .iter()
        .map(|p| DependencyInstanceInput {
            package_name: p.name.clone(),
            version: p.version.clone(),
            is_direct: is_direct(&p.name),
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
