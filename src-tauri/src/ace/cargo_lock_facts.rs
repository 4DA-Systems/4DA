// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Two facts a `Cargo.lock` states that the dependency tables used to ignore:
//! which crates are the project's own, and which crates only the dev build
//! pulls in. The rules over lockfile and manifest TEXT live in the engine
//! crate (`fourda_depengine::lockfile::cargo_facts`, single-engine C0.3) and
//! are re-exported here; this module keeps the file-system half: finding the
//! lockfile that resolves a manifest and the workspace manifest enclosing it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(crate) use fourda_depengine::lockfile::cargo_facts::{
    declared_copies, local_only_names, local_packages, CargoDevScope,
};
use fourda_depengine::lockfile::cargo_facts::{norm_crate_name, path_or_git_workspace_deps};

/// How far above a manifest to look for the lockfile that resolves it.
const MAX_ANCESTORS: usize = 6;

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

#[cfg(test)]
#[path = "cargo_lock_facts_tests.rs"]
mod tests;
