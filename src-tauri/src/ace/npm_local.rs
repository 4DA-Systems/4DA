// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! npm package names that are the user's OWN code, never public packages.
//!
//! The npm release watch looks every manifest dependency up on the PUBLIC
//! registry (`registry.npmjs.org/<name>/latest`). The scanner skipped only
//! protocol specs (`file:`, `workspace:`, ...), so a monorepo member referenced
//! by a plain range (`"@4da/agent-framework": "*"` with root
//! `workspaces: ["packages/*"]`) went out to the public registry by name — a
//! private package name leaving the machine (audit 2026-10-07, fresh-profile
//! dev.log: a public 404 for exactly that name). Privacy first: a name that
//! is local is dropped before it can become a registry query.
//!
//! A dependency is local when its name is:
//! - a member of a workspace that encloses the depending manifest (npm/yarn
//!   `workspaces`, either array or `{ packages: [...] }`, or a
//!   `pnpm-workspace.yaml`) — or the workspace root package itself;
//! - the name of ANY scanned `"private": true` package (never publishable);
//! - in a scope an `.npmrc` routes to a non-default registry
//!   (`@scope:registry=https://npm.pkg.github.com/`).
//!
//! What still slips through (a private name with no local trace) is caught by
//! the registry 404 negative cache in `sources::npm_registry`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::scanner::{ManifestType, ProjectSignal};

/// How far above a manifest to look for `.npmrc` files.
const MAX_NPMRC_ANCESTORS: usize = 12;

/// The registries that ARE the public default: a scope routed to one of these
/// is still public.
const PUBLIC_REGISTRY_HOSTS: &[&str] = &["registry.npmjs.org", "registry.yarnpkg.com"];

/// The facts of one scanned `package.json` this module needs.
#[derive(Debug, Default, Clone)]
struct NpmManifestFacts {
    dir: PathBuf,
    name: Option<String>,
    private: bool,
    /// Workspace member globs (from `workspaces` or `pnpm-workspace.yaml`).
    workspaces: Vec<String>,
}

fn read_json(path: &Path) -> Option<serde_json::Value> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// `workspaces` as an array, or `{ "packages": [...] }` (yarn classic).
fn workspace_globs(pkg: &serde_json::Value) -> Vec<String> {
    let list = match pkg.get("workspaces") {
        Some(serde_json::Value::Array(a)) => Some(a),
        Some(serde_json::Value::Object(o)) => o.get("packages").and_then(|p| p.as_array()),
        _ => None,
    };
    list.map(|a| {
        a.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    })
    .unwrap_or_default()
}

/// The `packages:` list of a `pnpm-workspace.yaml` (`  - 'packages/*'`).
pub(crate) fn pnpm_workspace_globs(content: &str) -> Vec<String> {
    let mut globs = Vec::new();
    let mut in_packages = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
            in_packages = trimmed.starts_with("packages:");
            continue;
        }
        if in_packages {
            if let Some(item) = trimmed.strip_prefix('-') {
                let item = item.trim().trim_matches(|c| c == '\'' || c == '"');
                if !item.is_empty() {
                    globs.push(item.to_string());
                }
            }
        }
    }
    globs
}

fn read_facts(manifest_path: &Path) -> Option<NpmManifestFacts> {
    let pkg = read_json(manifest_path)?;
    let dir = manifest_path.parent()?.to_path_buf();
    let mut workspaces = workspace_globs(&pkg);
    if let Ok(yaml) = fs::read_to_string(dir.join("pnpm-workspace.yaml")) {
        workspaces.extend(pnpm_workspace_globs(&yaml));
    }
    Some(NpmManifestFacts {
        dir,
        name: pkg.get("name").and_then(|v| v.as_str()).map(str::to_string),
        private: pkg.get("private").and_then(serde_json::Value::as_bool) == Some(true),
        workspaces,
    })
}

/// Member directories a workspace glob names. Supports the forms workspaces
/// use in practice: a literal path, `dir/*`, and `dir/**` (one level deep is
/// enough to find a member's `package.json`). Negations (`!x`) are ignored.
fn expand_member_dirs(root: &Path, glob: &str) -> Vec<PathBuf> {
    let glob = glob.trim().trim_start_matches("./");
    if glob.starts_with('!') || glob.is_empty() {
        return Vec::new();
    }
    let prefix = glob.trim_end_matches("/**").trim_end_matches("/*");
    if prefix.contains('*') {
        return Vec::new();
    }
    if prefix.len() == glob.len() {
        return vec![root.join(prefix)];
    }
    fs::read_dir(root.join(prefix))
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

/// Every local package name of one workspace root: the root package, every
/// scanned manifest under it, and every member its globs name.
fn workspace_member_names(root: &NpmManifestFacts, all: &[NpmManifestFacts]) -> HashSet<String> {
    let mut names: HashSet<String> = all
        .iter()
        .filter(|f| f.dir.starts_with(&root.dir))
        .filter_map(|f| f.name.clone())
        .collect();
    for glob in &root.workspaces {
        for member in expand_member_dirs(&root.dir, glob) {
            if let Some(name) = read_json(&member.join("package.json"))
                .and_then(|p| p.get("name").and_then(|v| v.as_str()).map(str::to_string))
            {
                names.insert(name);
            }
        }
    }
    names
}

/// Scopes an `.npmrc` routes to a registry other than the public default.
pub(crate) fn private_registry_scopes(npmrc: &str) -> HashSet<String> {
    npmrc
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('@'))
        .filter_map(|l| {
            let (key, value) = l.split_once('=')?;
            let scope = key.trim().strip_suffix(":registry")?;
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            let public = PUBLIC_REGISTRY_HOSTS.iter().any(|h| value.contains(h));
            (!public && !scope.is_empty()).then(|| scope.to_ascii_lowercase())
        })
        .collect()
}

/// Private scopes from the `.npmrc` files in `dir` and its ancestors.
fn npmrc_scopes_above(dir: &Path) -> HashSet<String> {
    dir.ancestors()
        .take(MAX_NPMRC_ANCESTORS)
        .filter_map(|d| fs::read_to_string(d.join(".npmrc")).ok())
        .flat_map(|c| private_registry_scopes(&c))
        .collect()
}

/// Private scopes from the user-level `~/.npmrc`.
fn user_npmrc_scopes() -> HashSet<String> {
    dirs::home_dir()
        .and_then(|h| fs::read_to_string(h.join(".npmrc")).ok())
        .map(|c| private_registry_scopes(&c))
        .unwrap_or_default()
}

fn scope_of(name: &str) -> Option<String> {
    name.starts_with('@')
        .then(|| name.split('/').next().unwrap_or(name).to_ascii_lowercase())
}

/// Drop every local / private npm dependency from the scanned signals. See
/// the module docs for the rules. Reads `~/.npmrc` for user-level scopes.
pub(crate) fn drop_local_npm_deps(signals: &mut [ProjectSignal]) {
    drop_local_npm_deps_with(signals, &user_npmrc_scopes());
}

/// [`drop_local_npm_deps`] with the user-level private scopes supplied, so a
/// test never reads the real home directory.
pub(crate) fn drop_local_npm_deps_with(
    signals: &mut [ProjectSignal],
    user_scopes: &HashSet<String>,
) {
    let facts: Vec<NpmManifestFacts> = signals
        .iter()
        .filter(|s| s.manifest_type == ManifestType::PackageJson)
        .filter_map(|s| read_facts(&s.manifest_path))
        .collect();
    if facts.is_empty() {
        return;
    }
    let private_names: HashSet<String> = facts
        .iter()
        .filter(|f| f.private)
        .filter_map(|f| f.name.clone())
        .collect();
    let workspaces: Vec<(PathBuf, HashSet<String>)> = facts
        .iter()
        .filter(|f| !f.workspaces.is_empty())
        .map(|root| (root.dir.clone(), workspace_member_names(root, &facts)))
        .collect();

    for signal in signals
        .iter_mut()
        .filter(|s| s.manifest_type == ManifestType::PackageJson)
    {
        let Some(dir) = signal.manifest_path.parent().map(Path::to_path_buf) else {
            continue;
        };
        let mut local = private_names.clone();
        for (root, members) in &workspaces {
            if dir.starts_with(root) {
                local.extend(members.iter().cloned());
            }
        }
        let mut scopes = npmrc_scopes_above(&dir);
        scopes.extend(user_scopes.iter().cloned());
        let keep = |name: &String| {
            !local.contains(name) && !scope_of(name).is_some_and(|s| scopes.contains(&s))
        };
        let before = signal.dependencies.len() + signal.dev_dependencies.len();
        signal.dependencies.retain(keep);
        signal.dev_dependencies.retain(keep);
        let dropped = before - signal.dependencies.len() - signal.dev_dependencies.len();
        if dropped > 0 {
            tracing::debug!(
                target: "ace::scanner",
                manifest = %signal.manifest_path.display(),
                dropped,
                "Dropped local/private npm dependencies (never sent to the public registry)"
            );
        }
    }
}

#[cfg(test)]
#[path = "npm_local_tests.rs"]
mod tests;
