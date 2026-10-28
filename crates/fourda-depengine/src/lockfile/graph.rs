// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Parent -> child edges from Cargo.lock, package-lock.json and
//! pnpm-lock.yaml. Moved from `ProjectScanner` in the app
//! (`ace/scanner.rs`), which keeps its `parse_*_edges` names as calls into
//! these and re-exports [`DependencyEdge`], [`EdgeScope`] and
//! [`ROOT_PARENT`].

use serde_json::{Map, Value};

use super::flat::parse_pnpm_package_key;
use super::npm_bundle::{npm_bundle_owner, npm_lock_entry_bundled, BUNDLED_SUFFIX};

/// The dependency scope of an edge. Cargo.lock does not separate dev in its
/// resolved graph (everything is `Runtime`); npm/pnpm distinguish dev/runtime.
/// The `scope` column keeps its `DEFAULT 'unknown'` for rows written before the
/// parsers emitted a scope — no Rust variant produces that string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeScope {
    Runtime,
    Dev,
}

impl EdgeScope {
    /// Canonical lowercase string stored in the `scope` column.
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeScope::Runtime => "runtime",
            EdgeScope::Dev => "dev",
        }
    }
}

/// A single parent->child dependency edge extracted from a lockfile.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DependencyEdge {
    pub parent: String,
    pub parent_version: Option<String>,
    pub child: String,
    pub child_version: Option<String>,
    pub scope: EdgeScope,
}

/// Sentinel parent name for edges that originate from a lockfile's root project
/// node (e.g. the `""` entry in package-lock v2/v3). Reachability treats this as
/// a synthetic root so direct deps become BFS entry points.
pub const ROOT_PARENT: &str = "__root__";

/// Parse a Cargo.lock and return parent->child dependency edges.
/// Each `[[package]]` block carries `name`, `version`, and an optional
/// `dependencies = [ "dep", "dep 1.2.3", ... ]` array. We emit one edge per
/// child. Cargo.lock does not separate dev deps in its resolved graph, so all
/// edges are `Runtime`. Robust to malformed input — returns what it can.
pub fn cargo_lock_edges(content: &str) -> Vec<DependencyEdge> {
    // Flush the current package's dependency list as edges.
    fn flush(
        edges: &mut Vec<DependencyEdge>,
        name: Option<&String>,
        version: Option<&String>,
        children: &[(String, Option<String>)],
    ) {
        if let Some(parent) = name {
            for (child, child_version) in children {
                edges.push(DependencyEdge {
                    parent: parent.clone(),
                    parent_version: version.cloned(),
                    child: child.clone(),
                    child_version: child_version.clone(),
                    scope: EdgeScope::Runtime,
                });
            }
        }
    }

    let mut edges = Vec::new();
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut in_deps = false;
    let mut children: Vec<(String, Option<String>)> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "[[package]]" {
            flush(&mut edges, name.as_ref(), version.as_ref(), &children);
            name = None;
            version = None;
            children.clear();
            in_deps = false;
            continue;
        }

        if in_deps {
            // Inside a `dependencies = [` array until the closing `]`.
            if trimmed.contains(']') {
                in_deps = false;
                // A single-line array may close on the same logical line; the
                // entries themselves are handled below for the multi-line form.
                let inner = trimmed.trim_end_matches(']');
                for part in inner.split(',') {
                    let part = part.trim();
                    if part.is_empty() || part == "[" {
                        continue;
                    }
                    push_cargo_child(&mut children, part);
                }
                continue;
            }
            let part = trimmed.trim_end_matches(',');
            if !part.is_empty() {
                push_cargo_child(&mut children, part);
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("name = ") {
            name = Some(rest.trim_matches('"').to_string());
        } else if let Some(rest) = trimmed.strip_prefix("version = ") {
            version = Some(rest.trim_matches('"').to_string());
        } else if let Some(rest) = trimmed.strip_prefix("dependencies = ") {
            let rest = rest.trim();
            // Inline single-line array: dependencies = ["a", "b 1.0"]
            if let Some(inner) = rest.strip_prefix('[') {
                if let Some(inner) = inner.strip_suffix(']') {
                    for part in inner.split(',') {
                        let part = part.trim();
                        if !part.is_empty() {
                            push_cargo_child(&mut children, part);
                        }
                    }
                } else {
                    // Multi-line array begins here.
                    in_deps = true;
                }
            }
        }
    }

    flush(&mut edges, name.as_ref(), version.as_ref(), &children);
    edges
}

fn push_cargo_child(children: &mut Vec<(String, Option<String>)>, part: &str) {
    let (c, v) = split_cargo_dep_spec(part);
    if !c.is_empty() {
        children.push((c, v));
    }
}

/// Parse a package-lock.json (v2/v3 `packages` map, v1 `dependencies` tree)
/// and return parent->child edges. Root edges use the [`ROOT_PARENT`] sentinel.
/// `dependencies` keys are `Runtime`, `devDependencies` are `Dev`.
/// Robust to malformed input — returns an empty Vec on parse failure.
pub fn package_lock_edges(content: &str) -> Vec<DependencyEdge> {
    let Ok(lock) = serde_json::from_str::<Value>(content) else {
        return Vec::new();
    };
    // v2/v3: a `packages` map keyed by node_modules path. "" is the root.
    if let Some(pkgs) = lock.get("packages").and_then(Value::as_object) {
        return package_lock_v2_edges(pkgs);
    }
    // v1 fallback: a `dependencies` tree where each node may carry `requires`
    // (its runtime children) and a `dev` flag. Emit root->dep edges and
    // dep->requires edges.
    let mut edges = Vec::new();
    if let Some(deps) = lock.get("dependencies").and_then(Value::as_object) {
        for (name, value) in deps {
            let is_dev = value.get("dev").and_then(Value::as_bool).unwrap_or(false);
            let scope = if is_dev {
                EdgeScope::Dev
            } else {
                EdgeScope::Runtime
            };
            let version = value
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_string);
            edges.push(DependencyEdge {
                parent: ROOT_PARENT.to_string(),
                parent_version: None,
                child: name.clone(),
                child_version: version.clone(),
                scope,
            });
            if let Some(requires) = value.get("requires").and_then(Value::as_object) {
                for (child, spec) in requires {
                    edges.push(DependencyEdge {
                        parent: name.clone(),
                        parent_version: version.clone(),
                        child: child.clone(),
                        child_version: spec.as_str().map(str::to_string),
                        scope,
                    });
                }
            }
        }
    }
    edges
}

/// The v2/v3 `packages` map's edges.
///
/// Nested keys are parents too: `node_modules/glob/node_modules/minimatch`
/// is minimatch 9.0.5, whose `^2.0.1` is what holds the nested
/// brace-expansion copy. First-level edges only left every such copy
/// "no parent 4DA can read" (audit A7, navcal 2026-10-12).
///
/// A bundled copy (`inBundle`) ships inside its bundle owner's tarball:
/// no refresh moves it, only a new owner release does. It gets ONE edge,
/// from the owner, pinned to the exact version shipped, and the bundle's
/// inner packages declare no edges of their own (audit E8: `npm update
/// debug` cannot reach debug inside nyc, npm or fsevents).
fn package_lock_v2_edges(pkgs: &Map<String, Value>) -> Vec<DependencyEdge> {
    let mut edges = Vec::new();
    for (key, value) in pkgs {
        // Bundled inside a DEPENDENCY's tarball: one owner edge, no edges
        // of its own. A project's or workspace member's own bundle is the
        // user's tree and keeps its ordinary edges.
        if npm_lock_entry_bundled(pkgs, key) {
            if let Some(edge) = bundled_edge(pkgs, key, value) {
                edges.push(edge);
            }
            continue;
        }
        let Some(name) = package_lock_name(key) else {
            continue;
        };
        let parent = if key.is_empty() {
            ROOT_PARENT.to_string()
        } else {
            name.to_string()
        };
        let parent_version = value
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_string);

        for (section, scope) in [
            ("dependencies", EdgeScope::Runtime),
            ("devDependencies", EdgeScope::Dev),
            ("optionalDependencies", EdgeScope::Dev),
            ("peerDependencies", EdgeScope::Runtime),
        ] {
            let Some(deps) = value.get(section).and_then(Value::as_object) else {
                continue;
            };
            for (child, spec) in deps {
                // An owner declares what it bundles (npm requires it;
                // npm 3.10.10 declares all 82 of its bundled packages).
                // That copy's edge is the bundled one: its range would
                // read as "a refresh is enough".
                let nested = if key.is_empty() {
                    format!("node_modules/{child}")
                } else {
                    format!("{key}/node_modules/{child}")
                };
                if npm_lock_entry_bundled(pkgs, &nested) {
                    continue;
                }
                edges.push(DependencyEdge {
                    parent: parent.clone(),
                    parent_version: parent_version.clone(),
                    child: child.clone(),
                    child_version: spec.as_str().map(str::to_string),
                    scope,
                });
            }
        }
    }
    edges
}

/// Parse a pnpm-lock.yaml and return parent->child edges. An entry's nested
/// `dependencies:` sub-map yields `Runtime` children;
/// `devDependencies:`/`optionalDependencies:` yield `Dev` children.
///
/// The child maps live in `packages:` for v5/v6 and in `snapshots:` for v9,
/// which moved every dependency map out of `packages:` (a v9 `packages:`
/// entry carries only resolution/engines metadata). Reading `packages:`
/// alone and stopping at the next top-level key found ZERO edges in every
/// v9 lockfile — live 2026-09-10, `paddle-webhook`, so `sandbox`, reached
/// only through `vercel`'s dev tree, could never be shown to be dev-only
/// (AD-046). Both sections are read now.
///
/// Line-based on pnpm's fixed layout: entries at indent 2 (`key:`, or
/// `key: {}` for a v9 snapshot with no fields), an entry's fields at 4, a
/// child map's entries at 6. A scalar field that follows a child map
/// (`dev: false`, `optional: true`) ends the map — it is not an edge to a
/// package named `dev`. The raw child reference is kept as the best-effort
/// `child_version` hint (peer suffix included). Robust to malformed input.
pub fn pnpm_lock_edges(content: &str) -> Vec<DependencyEdge> {
    let mut edges = Vec::new();
    let mut in_graph = false;
    let mut current_parent: Option<(String, Option<String>)> = None;
    let mut current_scope: Option<EdgeScope> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match pnpm_indent(line) {
            0 => {
                // A top-level key opens (or closes) a graph section.
                in_graph = matches!(trimmed, "packages:" | "snapshots:");
                current_parent = None;
                current_scope = None;
            }
            _ if !in_graph => {}
            2 => {
                current_parent = pnpm_entry_key(trimmed);
                current_scope = None;
            }
            4 => {
                // Any field other than a dependency map ends the child map.
                current_scope = match trimmed {
                    "dependencies:" => Some(EdgeScope::Runtime),
                    "devDependencies:" | "optionalDependencies:" => Some(EdgeScope::Dev),
                    _ => None,
                };
            }
            6 => {
                let (Some((parent, parent_version)), Some(scope)) =
                    (current_parent.as_ref(), current_scope)
                else {
                    continue;
                };
                let Some((child, spec)) = trimmed.split_once(':') else {
                    continue;
                };
                let child = child.trim().trim_matches('\'').trim_matches('"');
                if child.is_empty() {
                    continue;
                }
                let spec = spec.trim().trim_matches('\'').trim_matches('"');
                edges.push(DependencyEdge {
                    parent: parent.clone(),
                    parent_version: parent_version.clone(),
                    child: child.to_string(),
                    child_version: (!spec.is_empty()).then(|| spec.to_string()),
                    scope,
                });
            }
            _ => {}
        }
    }

    edges
}

/// The package a `package-lock.json` v2/v3 `packages` key installs: the name
/// after the LAST `node_modules/` (`node_modules/a/node_modules/@s/b` is
/// `@s/b`). `""` is the root. A key with no `node_modules/` (a workspace
/// member's folder, `packages/web`) is kept as written, as before.
fn package_lock_name(key: &str) -> Option<&str> {
    let name = key.rsplit_once("node_modules/").map_or(key, |(_, n)| n);
    (!name.is_empty() || key.is_empty()).then_some(name)
}

/// The edge for a copy bundled inside a dependency: from its bundle owner
/// ([`npm_bundle_owner`]) to the exact version shipped, spelled as an npm
/// pin with the bundled marker. `None` when the owner cannot be read.
fn bundled_edge(pkgs: &Map<String, Value>, key: &str, value: &Value) -> Option<DependencyEdge> {
    let child = package_lock_name(key)?.to_string();
    let child_version = value.get("version")?.as_str()?.to_string();
    let owner_key = npm_bundle_owner(pkgs, key)?;
    let owner = pkgs.get(owner_key)?;
    let scope = if value.get("dev").and_then(Value::as_bool) == Some(true) {
        EdgeScope::Dev
    } else {
        EdgeScope::Runtime
    };
    Some(DependencyEdge {
        parent: package_lock_name(owner_key)?.to_string(),
        parent_version: owner.get("version")?.as_str().map(str::to_string),
        child,
        child_version: Some(format!("{child_version}{BUNDLED_SUFFIX}")),
        scope,
    })
}

/// Leading indentation of a pnpm-lock line, a tab counting as two spaces.
fn pnpm_indent(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 2 } else { 1 })
        .sum()
}

/// A `packages:`/`snapshots:` entry line (`key:` or `key: {}`, optionally
/// quoted) as `(name, version)`. A key the parser cannot split keeps its name
/// with no version rather than being dropped.
fn pnpm_entry_key(trimmed: &str) -> Option<(String, Option<String>)> {
    let key = trimmed
        .strip_suffix(": {}")
        .or_else(|| trimmed.strip_suffix(':'))?;
    let key = key.trim().trim_matches('\'').trim_matches('"');
    let (name, version) = match parse_pnpm_package_key(key) {
        Some((name, version)) => (name, Some(version)),
        None => (key.trim_start_matches('/').to_string(), None),
    };
    (!name.is_empty()).then_some((name, version))
}

/// Split a Cargo.lock `dependencies` array entry into (name, optional version).
/// Entries look like `"serde"` or `"serde 1.0.0"` or `"serde 1.0.0 (registry+...)"`.
fn split_cargo_dep_spec(spec: &str) -> (String, Option<String>) {
    let spec = spec.trim().trim_matches('"').trim();
    match spec.split_once(' ') {
        Some((name, rest)) => {
            // Version is the first whitespace-separated token after the name.
            let version = rest.split_whitespace().next().map(str::to_string);
            (name.to_string(), version)
        }
        None => (spec.to_string(), None),
    }
}
