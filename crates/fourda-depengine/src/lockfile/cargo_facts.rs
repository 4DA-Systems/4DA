// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Two facts a `Cargo.lock` states that the dependency tables used to ignore.
//! The pure half of the app's `ace/cargo_lock_facts.rs`, which keeps the
//! file-system half (finding the lockfile and the enclosing workspace
//! manifest) and re-exports these.
//!
//! **Which crates are the project's own.** Cargo writes no `source =` line for
//! a workspace member or a path dependency; every registry or git package has
//! one. The lockfile walk stored every `[[package]]` regardless, and the
//! manifest scan skipped a literal `x = { path = ".." }` but not a member's
//! `x = { workspace = true }` that inherits one. Live 2026-10-02: a workspace's
//! own crates sat in `user_dependencies` as installed copies of themselves
//! (`victauri-core 0.8.8` in four of that workspace's projects), and
//! Preemption, advisory matching and release grading all read those rows.
//!
//! **Which crates only the dev build pulls in.** The walk wrote `is_dev = 0`
//! for every crate and so overwrote the manifest's `[dev-dependencies]` flag.
//! A registry package's lockfile entry lists its normal and build dependencies
//! only, never its dev-dependencies, so reachability from the manifest's
//! runtime and dev roots separates the two (the npm walk's rule, AD-046).
//!
//! Cargo treats `-` and `_` in a crate name as the same name, so every
//! comparison here goes through [`norm_crate_name`].

use std::collections::{HashMap, HashSet, VecDeque};

use super::graph::cargo_lock_edges;

/// The comparison form of a crate name: lowercase, `_` read as `-`.
pub fn norm_crate_name(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace('_', "-")
}

/// One `[[package]]` entry: name, version, and whether it carries a `source`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockPackage {
    pub name: String,
    pub version: String,
    pub has_source: bool,
}

/// Every `[[package]]` entry of a `Cargo.lock`.
pub fn parse_lock_packages(content: &str) -> Vec<LockPackage> {
    type Pending = (Option<String>, Option<String>, bool);
    let flush = |out: &mut Vec<LockPackage>, entry: Option<Pending>| {
        if let Some((Some(name), Some(version), has_source)) = entry {
            out.push(LockPackage {
                name,
                version,
                has_source,
            });
        }
    };
    let mut out = Vec::new();
    let mut current: Option<Pending> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            flush(&mut out, current.take());
            current = Some((None, None, false));
            continue;
        }
        if trimmed.starts_with('[') {
            // `[metadata]` and any other table ends the package list.
            flush(&mut out, current.take());
            continue;
        }
        let Some(entry) = current.as_mut() else {
            continue;
        };
        if let Some(rest) = trimmed.strip_prefix("name = ") {
            entry.0 = Some(rest.trim_matches('"').to_string());
        } else if let Some(rest) = trimmed.strip_prefix("version = ") {
            entry.1 = Some(rest.trim_matches('"').to_string());
        } else if trimmed.starts_with("source = ") {
            entry.2 = true;
        }
    }
    flush(&mut out, current);
    out
}

/// `(name, version)` of every package the lockfile builds from the project's
/// own tree (no `source`): its workspace members and path dependencies.
pub fn local_packages(content: &str) -> HashSet<(String, String)> {
    parse_lock_packages(content)
        .into_iter()
        .filter(|p| !p.has_source)
        .map(|p| (p.name, p.version))
        .collect()
}

/// `(name, version)` of every registry copy one of the project's OWN crates
/// (a package with no `source`: the root, a workspace member, a path crate)
/// lists in its Cargo.lock `dependencies` — the copies a manifest declaration
/// resolves to, `[dependencies]`, `[dev-dependencies]` and
/// `[build-dependencies]` alike, a workspace member's included. Cargo writes
/// a bare name when the lockfile holds one version of it and `name version`
/// when it holds several, so a second copy (`rand 0.7.3` under `tempfile`
/// beside the project's `rand 0.8.5`) is never one of these.
pub fn declared_copies(content: &str) -> HashSet<(String, String)> {
    let packages = parse_lock_packages(content);
    let local: HashSet<(String, String)> = packages
        .iter()
        .filter(|p| !p.has_source)
        .map(|p| (p.name.clone(), p.version.clone()))
        .collect();
    let mut versions: HashMap<String, Vec<String>> = HashMap::new();
    for p in packages.iter().filter(|p| p.has_source) {
        versions
            .entry(p.name.clone())
            .or_default()
            .push(p.version.clone());
    }
    let mut out = HashSet::new();
    for edge in cargo_lock_edges(content) {
        let parent = (
            edge.parent.clone(),
            edge.parent_version.clone().unwrap_or_default(),
        );
        if !local.contains(&parent) {
            continue;
        }
        let Some(candidates) = versions.get(&edge.child) else {
            continue; // another local crate
        };
        match edge.child_version {
            Some(v) if candidates.contains(&v) => {
                out.insert((edge.child.clone(), v));
            }
            Some(_) => {}
            None => out.extend(candidates.iter().map(|v| (edge.child.clone(), v.clone()))),
        }
    }
    out
}

/// Normalised names that exist in the lockfile ONLY as local packages. A name
/// that also resolves from a registry (a `[patch]` override next to the
/// published crate, or two copies at different versions) is not local.
pub fn local_only_names(content: &str) -> HashSet<String> {
    let packages = parse_lock_packages(content);
    let sourced: HashSet<String> = packages
        .iter()
        .filter(|p| p.has_source)
        .map(|p| norm_crate_name(&p.name))
        .collect();
    packages
        .iter()
        .filter(|p| !p.has_source)
        .map(|p| norm_crate_name(&p.name))
        .filter(|n| !sourced.contains(n))
        .collect()
}

/// `[workspace.dependencies]` entries whose value names a `path` or `git`
/// (normalised names): what a member's `x = { workspace = true }` inherits
/// when no lockfile exists yet to say so.
pub fn path_or_git_workspace_deps(manifest: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut in_table = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_table = trimmed == "[workspace.dependencies]";
            continue;
        }
        if !in_table || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let (name, field) = key.split_once('.').unwrap_or((key, ""));
        let value = value.trim();
        let local = matches!(field.trim(), "path" | "git")
            || (value.starts_with('{') && (inline_key(value, "path") || inline_key(value, "git")));
        if local {
            out.insert(norm_crate_name(name.trim().trim_matches('"')));
        }
    }
    out
}

/// `key` used as a key (followed by `=`) inside an inline table.
fn inline_key(table: &str, key: &str) -> bool {
    table
        .trim_matches(|c| c == '{' || c == '}')
        .split(',')
        .filter_map(|part| part.split_once('='))
        .any(|(k, _)| k.trim() == key)
}

/// Where each crate of one lockfile ships, by normalised name.
#[derive(Debug, Default)]
pub struct CargoDevScope {
    runtime: HashSet<String>,
    dev: HashSet<String>,
}

impl CargoDevScope {
    /// Reachability over the lockfile's edges from the manifest's runtime
    /// roots (`[dependencies]`, `[target.*.dependencies]`) and dev roots
    /// (`[dev-dependencies]`). Edges out of LOCAL packages are not followed:
    /// a workspace member's lockfile entry lists its dev-dependencies too.
    pub fn new(content: &str, runtime_roots: &[String], dev_roots: &[String]) -> Self {
        let locals: HashSet<String> = local_only_names(content);
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        for edge in cargo_lock_edges(content) {
            let parent = norm_crate_name(&edge.parent);
            if locals.contains(&parent) {
                continue;
            }
            graph
                .entry(parent)
                .or_default()
                .push(norm_crate_name(&edge.child));
        }
        Self {
            runtime: reach(&graph, runtime_roots),
            dev: reach(&graph, dev_roots),
        }
    }

    /// Reached from a dev root and from no runtime root.
    pub fn is_dev_only(&self, name: &str) -> bool {
        let n = norm_crate_name(name);
        self.dev.contains(&n) && !self.runtime.contains(&n)
    }

    /// `runtime` | `dev` | `unknown`, the `dependency_instances.scope` vocabulary.
    pub fn label(&self, name: &str) -> &'static str {
        let n = norm_crate_name(name);
        if self.runtime.contains(&n) {
            "runtime"
        } else if self.dev.contains(&n) {
            "dev"
        } else {
            "unknown"
        }
    }
}

fn reach(graph: &HashMap<String, Vec<String>>, roots: &[String]) -> HashSet<String> {
    let mut seen = HashSet::new();
    let mut queue: VecDeque<String> = roots.iter().map(|r| norm_crate_name(r)).collect();
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current.clone()) {
            continue;
        }
        if let Some(children) = graph.get(&current) {
            queue.extend(children.iter().filter(|c| !seen.contains(*c)).cloned());
        }
    }
    seen
}

#[cfg(test)]
#[path = "cargo_facts_tests.rs"]
mod tests;
