// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Schema-114 regression guards for `store_dependency_edges` idempotency.
//!
//! The pre-114 writer re-APPENDED the full dependency graph on every ACE scan
//! — measured live 2026-08-31 at ~642k rows for ~12k distinct logical edges
//! (52.7x duplication, ~168 MB with indexes). These tests pin the fix: one row
//! per logical edge `(project_path, ecosystem, parent@version, child@version)`,
//! enforced by `idx_dep_edges_unique` and the writer's matching ON CONFLICT
//! upsert.

use crate::ace::scanner::{DependencyEdge, EdgeScope};
use crate::test_utils::test_db;

fn edge(
    parent: &str,
    parent_version: Option<&str>,
    child: &str,
    child_version: Option<&str>,
    scope: EdgeScope,
) -> DependencyEdge {
    DependencyEdge {
        parent: parent.to_string(),
        parent_version: parent_version.map(str::to_string),
        child: child.to_string(),
        child_version: child_version.map(str::to_string),
        scope,
    }
}

/// Re-storing the same graph must not grow the table. A rescan refreshes
/// scope/detected_at in place; a genuinely new version pair is a new edge.
#[test]
fn test_store_dependency_edges_is_idempotent_across_rescans() {
    let db = test_db();
    let edges = vec![
        edge(
            "app",
            Some("0.1.0"),
            "serde",
            Some("1.0.190"),
            EdgeScope::Runtime,
        ),
        // NULL versions must dedupe too (SQLite treats NULLs as distinct in
        // unique indexes; the COALESCE key is what makes this hold).
        edge("__root__", None, "left-pad", None, EdgeScope::Runtime),
    ];

    // Three "scans" of the same lockfile.
    for _ in 0..3 {
        let n = db
            .store_dependency_edges("/projects/myapp", "rust", &edges)
            .expect("store edges");
        assert_eq!(n, 2, "every edge in the batch is written (upserted)");
    }
    let rows = db
        .get_dependency_edges("/projects/myapp")
        .expect("get edges");
    assert_eq!(rows.len(), 2, "rescans must not duplicate edges");
}

/// A rescan that reclassifies an edge's scope updates the existing row.
#[test]
fn test_store_dependency_edges_scope_change_updates_in_place() {
    let db = test_db();
    let runtime = vec![edge(
        "app",
        Some("0.1.0"),
        "serde",
        Some("1.0.190"),
        EdgeScope::Runtime,
    )];
    let dev = vec![edge(
        "app",
        Some("0.1.0"),
        "serde",
        Some("1.0.190"),
        EdgeScope::Dev,
    )];

    db.store_dependency_edges("/projects/myapp", "rust", &runtime)
        .expect("store runtime");
    db.store_dependency_edges("/projects/myapp", "rust", &dev)
        .expect("store reclassified");

    let rows = db
        .get_dependency_edges("/projects/myapp")
        .expect("get edges");
    assert_eq!(rows.len(), 1, "scope change is an update, not a new row");
    assert_eq!(rows[0].scope, "dev");
}

/// A different resolved version IS a different logical edge — dedupe must not
/// collapse a genuine multi-version graph (npm resolves the same package at
/// several versions in one lockfile).
#[test]
fn test_store_dependency_edges_new_version_pair_is_new_edge() {
    let db = test_db();
    db.store_dependency_edges(
        "/projects/myapp",
        "rust",
        &[
            edge(
                "app",
                Some("0.1.0"),
                "serde",
                Some("1.0.190"),
                EdgeScope::Runtime,
            ),
            edge(
                "app",
                Some("0.1.0"),
                "serde",
                Some("1.0.200"),
                EdgeScope::Runtime,
            ),
        ],
    )
    .expect("store both versions");

    let rows = db
        .get_dependency_edges("/projects/myapp")
        .expect("get edges");
    assert_eq!(rows.len(), 2, "a new version pair is a new edge");
}

/// A rescan replaces the graph: an edge the lockfile no longer has is gone.
/// Before 2026-11-01 the writer only upserted, so a package upgraded away
/// still named its old parent (30% of live rows were stale).
#[test]
fn test_store_dependency_edges_rescan_drops_edges_the_lockfile_lost() {
    let db = test_db();
    let first = vec![
        edge(
            "app",
            Some("0.1.0"),
            "serde",
            Some("1.0.190"),
            EdgeScope::Runtime,
        ),
        edge(
            "app",
            Some("0.1.0"),
            "zip",
            Some("4.6.1"),
            EdgeScope::Runtime,
        ),
    ];
    let upgraded = vec![edge(
        "app",
        Some("0.1.0"),
        "serde",
        Some("1.0.200"),
        EdgeScope::Runtime,
    )];
    db.store_dependency_edges("/projects/myapp", "rust", &first)
        .expect("first scan");
    db.store_dependency_edges("/projects/myapp", "rust", &upgraded)
        .expect("rescan");

    let rows = db
        .get_dependency_edges("/projects/myapp")
        .expect("get edges");
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].child_package, "serde");
    assert_eq!(rows[0].child_version.as_deref(), Some("1.0.200"));
}

/// The replace is scoped to one `(project_path, ecosystem)`: another project,
/// or another ecosystem of the same project, keeps its graph.
#[test]
fn test_store_dependency_edges_replace_is_scoped_to_project_and_ecosystem() {
    let db = test_db();
    let one = |child: &str| vec![edge("app", None, child, None, EdgeScope::Runtime)];
    db.store_dependency_edges("/projects/myapp", "rust", &one("serde"))
        .unwrap();
    db.store_dependency_edges("/projects/myapp", "javascript", &one("left-pad"))
        .unwrap();
    db.store_dependency_edges("/projects/other", "rust", &one("tokio"))
        .unwrap();

    db.store_dependency_edges("/projects/myapp", "rust", &one("anyhow"))
        .unwrap();

    let children = |p: &str| -> Vec<String> {
        let mut c: Vec<String> = db
            .get_dependency_edges(p)
            .unwrap()
            .into_iter()
            .map(|r| r.child_package)
            .collect();
        c.sort();
        c
    };
    assert_eq!(children("/projects/myapp"), ["anyhow", "left-pad"]);
    assert_eq!(children("/projects/other"), ["tokio"]);
}

/// A scan whose lockfiles yield no edges clears the key: a project that moved
/// off its edge-producing lockfile keeps no graph from it.
#[test]
fn test_store_dependency_edges_empty_graph_clears_the_key() {
    let db = test_db();
    db.store_dependency_edges(
        "/projects/myapp",
        "javascript",
        &[edge("__root__", None, "left-pad", None, EdgeScope::Runtime)],
    )
    .unwrap();
    let n = db
        .store_dependency_edges("/projects/myapp", "javascript", &[])
        .unwrap();
    assert_eq!(n, 0);
    assert!(db
        .get_dependency_edges("/projects/myapp")
        .unwrap()
        .is_empty());
}
