// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Two facts a `Cargo.lock` states that the dependency tables used to ignore.
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
use std::path::{Path, PathBuf};

use super::scanner::ProjectScanner;

/// How far above a manifest to look for the lockfile that resolves it.
const MAX_ANCESTORS: usize = 6;

/// The comparison form of a crate name: lowercase, `_` read as `-`.
pub(crate) fn norm_crate_name(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace('_', "-")
}

/// One `[[package]]` entry: name, version, and whether it carries a `source`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LockPackage {
    pub name: String,
    pub version: String,
    pub has_source: bool,
}

/// Every `[[package]]` entry of a `Cargo.lock`.
pub(crate) fn parse_lock_packages(content: &str) -> Vec<LockPackage> {
    let mut out = Vec::new();
    let mut current: Option<(Option<String>, Option<String>, bool)> = None;
    let flush = |out: &mut Vec<LockPackage>,
                 entry: Option<(Option<String>, Option<String>, bool)>| {
        if let Some((Some(name), Some(version), has_source)) = entry {
            out.push(LockPackage {
                name,
                version,
                has_source,
            });
        }
    };
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
pub(crate) fn local_packages(content: &str) -> HashSet<(String, String)> {
    parse_lock_packages(content)
        .into_iter()
        .filter(|p| !p.has_source)
        .map(|p| (p.name, p.version))
        .collect()
}

/// Normalised names that exist in the lockfile ONLY as local packages. A name
/// that also resolves from a registry (a `[patch]` override next to the
/// published crate, or two copies at different versions) is not local.
pub(crate) fn local_only_names(content: &str) -> HashSet<String> {
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

/// The `Cargo.lock` that resolves the manifest in `dir`: the directory's own,
/// else the nearest one above it — at most [`MAX_ANCESTORS`] levels, and never
/// past the directory holding `.git` (a lockfile outside the repository
/// belongs to some other project).
pub(crate) fn nearest_cargo_lock(dir: &Path) -> Option<PathBuf> {
    let mut cursor = Some(dir);
    for _ in 0..=MAX_ANCESTORS {
        let d = cursor?;
        let lock = d.join("Cargo.lock");
        if lock.is_file() {
            return Some(lock);
        }
        if d.join(".git").exists() {
            return None;
        }
        cursor = d.parent();
    }
    None
}

/// Names declared in the nearest enclosing `[workspace.dependencies]` table
/// with a `path` or `git` source — what a member's `x = { workspace = true }`
/// inherits when no lockfile exists yet to say so.
fn workspace_local_declarations(dir: &Path) -> HashSet<String> {
    let mut cursor = Some(dir);
    for _ in 0..=MAX_ANCESTORS {
        let Some(d) = cursor else { break };
        if let Ok(text) = std::fs::read_to_string(d.join("Cargo.toml")) {
            if text.lines().any(|l| l.trim() == "[workspace]") {
                return path_or_git_workspace_deps(&text);
            }
        }
        if d.join(".git").exists() {
            break;
        }
        cursor = d.parent();
    }
    HashSet::new()
}

/// `[workspace.dependencies]` entries whose value names a `path` or `git`.
fn path_or_git_workspace_deps(manifest: &str) -> HashSet<String> {
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

/// Crate names the manifest in `manifest_path` resolves from its own
/// workspace, normalised: the lockfile's local-only names, plus the enclosing
/// workspace's `path`/`git` declarations.
pub(crate) fn workspace_local_crate_names(manifest_path: &Path) -> HashSet<String> {
    let Some(dir) = manifest_path.parent() else {
        return HashSet::new();
    };
    let mut names = workspace_local_declarations(dir);
    if let Some(lock) = nearest_cargo_lock(dir) {
        if let Ok(content) = std::fs::read_to_string(lock) {
            names.extend(local_only_names(&content));
        }
    }
    names
}

/// Remove the project's own crates from a parsed Cargo manifest signal.
pub(crate) fn drop_local_crates(
    local: &HashSet<String>,
    signal: &mut super::scanner::ProjectSignal,
) {
    if local.is_empty() {
        return;
    }
    let keep = |name: &String| !local.contains(&norm_crate_name(name));
    signal.dependencies.retain(keep);
    signal.dev_dependencies.retain(keep);
    signal
        .target_dependencies
        .retain(|(name, _)| !local.contains(&norm_crate_name(name)));
}

/// Where each crate of one lockfile ships, by normalised name.
#[derive(Debug, Default)]
pub(crate) struct CargoDevScope {
    runtime: HashSet<String>,
    dev: HashSet<String>,
}

impl CargoDevScope {
    /// Reachability over the lockfile's edges from the manifest's runtime
    /// roots (`[dependencies]`, `[target.*.dependencies]`) and dev roots
    /// (`[dev-dependencies]`). Edges out of LOCAL packages are not followed:
    /// a workspace member's lockfile entry lists its dev-dependencies too.
    pub(crate) fn new(content: &str, runtime_roots: &[String], dev_roots: &[String]) -> Self {
        let locals: HashSet<String> = local_only_names(content);
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        for edge in ProjectScanner::parse_cargo_lock_edges(content) {
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
    pub(crate) fn is_dev_only(&self, name: &str) -> bool {
        let n = norm_crate_name(name);
        self.dev.contains(&n) && !self.runtime.contains(&n)
    }

    /// `runtime` | `dev` | `unknown`, the `dependency_instances.scope` vocabulary.
    pub(crate) fn label(&self, name: &str) -> &'static str {
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
#[path = "cargo_lock_facts_tests.rs"]
mod tests;
