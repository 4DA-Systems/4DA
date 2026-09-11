// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Which npm packages a project reaches ONLY through its development tooling.
//!
//! The lockfile walk never computed this. Every `dependency_instances` row it
//! wrote said `is_dev = 0, scope = 'unknown'` — measured on the founder
//! instance 2026-09-10: all 8,346 rows, direct devDependencies included. The
//! cost was a ranking: `sandbox@3.1.2` reaches `D:\4DA\paddle-webhook` only as
//! `vercel` -> `sandbox` -> `@vercel/sandbox`, and that project declares no
//! `dependencies` at all — only `devDependencies` (vercel, typescript,
//! @types/node) — yet its advisory was Preemption's #1 row, above a live,
//! fixable-now auth bypass in `jsonwebtoken`. Two gaps made that possible:
//! pnpm v9 moved every dependency edge out of `packages:` into `snapshots:`,
//! which `ProjectScanner::parse_pnpm_lock_edges` never read (fixed alongside
//! this module), and nothing walked the graph from the importer's roots.
//!
//! The rule (AD-046):
//! - Runtime roots are an importer's `dependencies` + `optionalDependencies`;
//!   dev roots are its `devDependencies`. Every importer counts: the walk
//!   attributes a workspace's whole lockfile to the directory that holds it.
//! - Reachability follows EVERY child edge of a reached package — a runtime
//!   package's own optional dependencies ship with it.
//! - A package is dev-only iff a dev root reaches it and no runtime root does.
//!   Reached from neither (an orphan, a lockfile shape this parser does not
//!   know) is NOT dev: unknown is never dev, because a security surface must
//!   not understate risk.
//! - `package-lock.json` already carries npm's own verdict — `"dev": true` on
//!   the entry. `devOptional` is NOT dev-only: the package is also an
//!   optional runtime dependency.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::ace::scanner::{
    parse_pnpm_package_key, split_pnpm_name_at_version, strip_pnpm_peer_suffix, ProjectScanner,
};

/// A package instance: lowercased name + version without any peer suffix —
/// the identity `parse_pnpm_lock_yaml` / `parse_package_lock_json` give the
/// rows `dependency_instances` stores.
type Node = (String, String);

fn node(name: &str, version: &str) -> Node {
    (name.to_ascii_lowercase(), version.trim().to_string())
}

/// Where one lockfile says each of its packages ships.
#[derive(Debug, Default)]
pub(crate) struct DevScope {
    runtime: HashSet<Node>,
    dev: HashSet<Node>,
}

impl DevScope {
    /// From a `pnpm-lock.yaml` (v5, v6 or v9).
    pub(crate) fn from_pnpm_lock(content: &str) -> Self {
        let roots = pnpm_roots(content);
        let graph = pnpm_graph(content);
        Self {
            runtime: reach(&graph, &roots.runtime),
            dev: reach(&graph, &roots.dev),
        }
    }

    /// From a `package-lock.json` (v1 `dependencies` tree, v2/v3 `packages`).
    pub(crate) fn from_package_lock(content: &str) -> Self {
        let mut scope = Self::default();
        let Ok(lock) = serde_json::from_str::<serde_json::Value>(content) else {
            return scope;
        };
        if let Some(packages) = lock.get("packages").and_then(|v| v.as_object()) {
            for (key, entry) in packages {
                // Installed copies only (`node_modules/<name>` at any depth).
                // The root `""` and workspace member dirs are not packages.
                let Some((_, name)) = key.rsplit_once("node_modules/") else {
                    continue;
                };
                if let Some(version) = entry.get("version").and_then(|v| v.as_str()) {
                    scope.record(name, version, npm_marks_dev(entry));
                }
            }
        } else if let Some(tree) = lock.get("dependencies").and_then(|v| v.as_object()) {
            // v1 nests copies under each entry's own `dependencies`.
            let mut pending = vec![tree];
            while let Some(level) = pending.pop() {
                for (name, entry) in level {
                    if let Some(version) = entry.get("version").and_then(|v| v.as_str()) {
                        scope.record(name, version, npm_marks_dev(entry));
                    }
                    if let Some(nested) = entry.get("dependencies").and_then(|v| v.as_object()) {
                        pending.push(nested);
                    }
                }
            }
        }
        scope
    }

    /// Reached from a dev root and from no runtime root.
    pub(crate) fn is_dev_only(&self, name: &str, version: &str) -> bool {
        let n = node(name, version);
        self.dev.contains(&n) && !self.runtime.contains(&n)
    }

    /// The `dependency_instances.scope` vocabulary: `runtime` when any runtime
    /// root reaches it, `dev` when only dev roots do, `unknown` otherwise.
    pub(crate) fn label(&self, name: &str, version: &str) -> &'static str {
        let n = node(name, version);
        if self.runtime.contains(&n) {
            "runtime"
        } else if self.dev.contains(&n) {
            "dev"
        } else {
            "unknown"
        }
    }

    fn record(&mut self, name: &str, version: &str, dev: bool) {
        let n = node(name, version);
        if dev {
            self.dev.insert(n);
        } else {
            self.runtime.insert(n);
        }
    }
}

/// npm's own verdict. `devOptional` stays runtime: it is also optional there.
fn npm_marks_dev(entry: &serde_json::Value) -> bool {
    entry
        .get("dev")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy)]
enum RootKind {
    Runtime,
    Dev,
}

#[derive(Debug, Default)]
struct Roots {
    runtime: Vec<Node>,
    dev: Vec<Node>,
}

impl Roots {
    fn push(&mut self, kind: RootKind, n: Node) {
        match kind {
            RootKind::Runtime => self.runtime.push(n),
            RootKind::Dev => self.dev.push(n),
        }
    }
}

fn root_kind(header: &str) -> Option<RootKind> {
    match header {
        "dependencies:" | "optionalDependencies:" => Some(RootKind::Runtime),
        "devDependencies:" => Some(RootKind::Dev),
        _ => None,
    }
}

/// Every importer's declared roots. v6/v9 (and v5 workspaces) nest the maps
/// under `importers:` -> `<dir>:` at indent 4; v5/v6 single projects write
/// them at the top level. An entry is `name: <ref>` (v5) or `name:` with a
/// nested `version: <ref>` (v6/v9).
fn pnpm_roots(content: &str) -> Roots {
    let mut roots = Roots::default();
    let mut in_importers = false;
    // The dependency map being read, and the indent of its header line.
    let mut section: Option<(RootKind, usize)> = None;
    // v6/v9: an entry whose version is on a nested line.
    let mut pending: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let indent = indent_of(line);
        if indent == 0 {
            in_importers = trimmed == "importers:";
            section = root_kind(trimmed).map(|kind| (kind, 0));
            pending = None;
            continue;
        }
        if in_importers && indent <= 4 {
            // Indent 2 is an importer (`.:`, `packages/api:`); 4 is one of its maps.
            section = (indent == 4)
                .then(|| root_kind(trimmed).map(|kind| (kind, 4)))
                .flatten();
            pending = None;
            continue;
        }
        let Some((kind, header)) = section else {
            continue;
        };
        if indent == header + 2 {
            pending = None;
            let Some((name, reference)) = trimmed.split_once(':') else {
                continue;
            };
            let name = unquote(name);
            let reference = unquote(reference);
            if reference.is_empty() {
                pending = Some(name.to_string());
            } else if let Some(n) = resolve_ref(name, reference) {
                roots.push(kind, n);
            }
        } else if indent == header + 4 {
            if let Some(reference) = trimmed.strip_prefix("version:") {
                if let Some(name) = pending.take() {
                    if let Some(n) = resolve_ref(&name, unquote(reference)) {
                        roots.push(kind, n);
                    }
                }
            }
        }
    }
    roots
}

/// Parent -> children, from the same edges the walk stores.
fn pnpm_graph(content: &str) -> HashMap<Node, Vec<Node>> {
    let mut graph: HashMap<Node, Vec<Node>> = HashMap::new();
    for edge in ProjectScanner::parse_pnpm_lock_edges(content) {
        let Some(parent_version) = edge.parent_version.as_deref() else {
            continue;
        };
        let Some(child) = edge
            .child_version
            .as_deref()
            .and_then(|reference| resolve_ref(&edge.child, reference))
        else {
            continue;
        };
        graph
            .entry(node(&edge.parent, parent_version))
            .or_default()
            .push(child);
    }
    graph
}

/// Resolve a version reference — from an importer map or a child map — to the
/// package it names:
/// - `4.18.2`, `18.2.0(react@18.2.0)` (v6/v9), `18.2.0_react@18.2.0` (v5) -> `name`
/// - `string-width@4.2.3` (v9 alias) -> the aliased package
/// - `/string-width@4.2.3` (v6), `/string-width/4.2.3` (v5) -> parsed as a key
/// - `link:../pkg` -> nothing: a workspace link is another importer, walked
///   from its own roots.
fn resolve_ref(name: &str, reference: &str) -> Option<Node> {
    let reference = strip_pnpm_peer_suffix(reference);
    if reference.is_empty() || reference.starts_with("link:") {
        return None;
    }
    if reference.starts_with('/') {
        let (name, version) = parse_pnpm_package_key(reference)?;
        return Some(node(&name, &version));
    }
    if reference.starts_with(|c: char| c.is_ascii_digit()) {
        let version = reference.split('_').next().unwrap_or(reference);
        return Some(node(name, version));
    }
    if let Some((name, version)) = split_pnpm_name_at_version(reference) {
        return Some(node(&name, &version));
    }
    // `file:`/tarball references: kept verbatim, matching only themselves.
    Some(node(name, reference))
}

/// Every node reachable from `roots`, the roots included.
fn reach(graph: &HashMap<Node, Vec<Node>>, roots: &[Node]) -> HashSet<Node> {
    let mut seen: HashSet<Node> = HashSet::new();
    let mut queue: VecDeque<&Node> = roots.iter().collect();
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current.clone()) {
            continue;
        }
        if let Some(children) = graph.get(current) {
            queue.extend(children.iter().filter(|child| !seen.contains(*child)));
        }
    }
    seen
}

fn indent_of(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 2 } else { 1 })
        .sum()
}

fn unquote(s: &str) -> &str {
    s.trim().trim_matches('\'').trim_matches('"')
}

#[cfg(test)]
#[path = "dep_scope_tests.rs"]
mod tests;
