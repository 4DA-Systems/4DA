// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Dependency reachability engine (Step 1 foundation — ships SILENT).
//!
//! Computes, purely in memory, whether a package is reachable from a project's
//! runtime roots, only from its dev roots, or unreachable entirely. This is the
//! groundwork for lifting the "transitive Critical capped at High until
//! reachability proven" cap in a later increment.
//!
//! Internal computation only — no `EvidenceItem`-competing type, no surfaced
//! output. The function is pure over in-memory edges so it is trivially testable.
//!
//! Increment 1 ships this engine SILENT: it is exercised by its own unit tests
//! but not yet consumed by the OSV/preemption ranking path (that wiring lands in
//! Increment 2). The `allow(dead_code)` below covers that intentional gap.
// REMOVE BY 2026-07-31: consumed by the OSV/preemption reachability ranking in increment 2
#![allow(dead_code)]

use std::collections::{HashMap, HashSet, VecDeque};

use crate::db::DependencyEdgeRow;

/// Reachability classification for a single package within one project.
///
/// `RuntimeReachable` strictly wins over `DevReachable`: a package reachable via
/// any runtime path is runtime-reachable even if a dev path also reaches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reachability {
    /// Reachable from a runtime root following only runtime-scope edges.
    RuntimeReachable,
    /// Reachable only when dev edges / dev roots are included.
    DevReachable,
    /// Never reached from any root.
    Unreachable,
}

/// Build an adjacency map (parent -> children) from edges, optionally restricting
/// to runtime-scope edges only.
fn build_adjacency(edges: &[DependencyEdgeRow], runtime_only: bool) -> HashMap<&str, Vec<&str>> {
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges {
        if runtime_only && edge.scope != "runtime" {
            continue;
        }
        adj.entry(edge.parent_package.as_str())
            .or_default()
            .push(edge.child_package.as_str());
    }
    adj
}

/// BFS from the given roots over the adjacency map, returning the set of all
/// reachable package names (roots included if they appear as nodes).
fn bfs_reachable<'a>(adj: &HashMap<&'a str, Vec<&'a str>>, roots: &[String]) -> HashSet<String> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    for root in roots {
        if visited.insert(root.clone()) {
            queue.push_back(root.clone());
        }
    }

    while let Some(node) = queue.pop_front() {
        if let Some(children) = adj.get(node.as_str()) {
            for &child in children {
                if visited.insert(child.to_string()) {
                    queue.push_back(child.to_string());
                }
            }
        }
    }

    visited
}

/// Compute reachability for every package appearing in `edges`.
///
/// - BFS over RUNTIME-scope edges from `runtime_roots` => `RuntimeReachable`.
/// - BFS over ALL edges from (`runtime_roots` + `dev_roots`) => remaining nodes
///   in that set become `DevReachable`.
/// - Anything never visited => `Unreachable`.
///
/// `RuntimeReachable` always wins over `DevReachable`.
pub fn compute_reachability(
    edges: &[DependencyEdgeRow],
    runtime_roots: &[String],
    dev_roots: &[String],
) -> HashMap<String, Reachability> {
    // Runtime pass: only runtime edges, only runtime roots.
    let runtime_adj = build_adjacency(edges, true);
    let runtime_set = bfs_reachable(&runtime_adj, runtime_roots);

    // Full pass: all edges, all roots.
    let full_adj = build_adjacency(edges, false);
    let mut all_roots: Vec<String> = Vec::with_capacity(runtime_roots.len() + dev_roots.len());
    all_roots.extend_from_slice(runtime_roots);
    all_roots.extend_from_slice(dev_roots);
    let dev_set = bfs_reachable(&full_adj, &all_roots);

    // Collect every package node mentioned in the graph (parents and children).
    let mut all_nodes: HashSet<&str> = HashSet::new();
    for edge in edges {
        all_nodes.insert(edge.parent_package.as_str());
        all_nodes.insert(edge.child_package.as_str());
    }

    let mut out = HashMap::new();
    for node in all_nodes {
        let class = if runtime_set.contains(node) {
            Reachability::RuntimeReachable
        } else if dev_set.contains(node) {
            Reachability::DevReachable
        } else {
            Reachability::Unreachable
        };
        out.insert(node.to_string(), class);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(parent: &str, child: &str, scope: &str) -> DependencyEdgeRow {
        DependencyEdgeRow {
            id: 0,
            project_path: "/p".to_string(),
            ecosystem: "rust".to_string(),
            parent_package: parent.to_string(),
            parent_version: None,
            child_package: child.to_string(),
            child_version: None,
            scope: scope.to_string(),
            detected_at: String::new(),
        }
    }

    #[test]
    fn runtime_dev_and_unreachable_are_classified() {
        // root -> A -> B (runtime); devroot -> C (dev); orphan D (no path).
        let edges = vec![
            edge("root", "A", "runtime"),
            edge("A", "B", "runtime"),
            edge("devroot", "C", "dev"),
            edge("orphan", "D", "runtime"),
        ];
        let runtime_roots = vec!["root".to_string()];
        let dev_roots = vec!["devroot".to_string()];

        let r = compute_reachability(&edges, &runtime_roots, &dev_roots);

        assert_eq!(r.get("B"), Some(&Reachability::RuntimeReachable));
        assert_eq!(r.get("A"), Some(&Reachability::RuntimeReachable));
        assert_eq!(r.get("C"), Some(&Reachability::DevReachable));
        // D is only reachable from "orphan", which is not a root.
        assert_eq!(r.get("D"), Some(&Reachability::Unreachable));
        assert_eq!(r.get("orphan"), Some(&Reachability::Unreachable));
    }

    #[test]
    fn dev_only_path_does_not_mark_runtime() {
        // A runtime root reaches X only through a DEV edge => X must be DevReachable.
        let edges = vec![edge("root", "X", "dev")];
        let runtime_roots = vec!["root".to_string()];
        let dev_roots: Vec<String> = vec![];

        let r = compute_reachability(&edges, &runtime_roots, &dev_roots);

        assert_eq!(
            r.get("X"),
            Some(&Reachability::DevReachable),
            "a dev-scope edge must not produce a runtime-reachable child"
        );
    }

    #[test]
    fn runtime_wins_over_dev_when_both_paths_exist() {
        // root reaches Z via both a runtime path and a dev path => RuntimeReachable.
        let edges = vec![edge("root", "Z", "runtime"), edge("root", "Z", "dev")];
        let runtime_roots = vec!["root".to_string()];
        let dev_roots: Vec<String> = vec![];

        let r = compute_reachability(&edges, &runtime_roots, &dev_roots);

        assert_eq!(r.get("Z"), Some(&Reachability::RuntimeReachable));
    }

    #[test]
    fn empty_edges_yield_empty_map() {
        let r = compute_reachability(&[], &["root".to_string()], &[]);
        assert!(r.is_empty());
    }
}
