// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! NodeGoat, cut down, in a test database (registry and OSV data of
//! 2026-10-18): the fixture the parent-release tests on every surface share.

use crate::db::{Database, DependencyInstanceInput};
use crate::osv::parent_release::{ReleaseReq, RequirementHistory};

use super::parent_hint::normalize_path;

pub(crate) fn hist(pairs: &[(&str, Option<&str>)]) -> RequirementHistory {
    pairs
        .iter()
        .map(|(v, r)| {
            (
                v.to_string(),
                ReleaseReq {
                    reqs: r.iter().map(|s| s.to_string()).collect(),
                    peer: false,
                },
            )
        })
        .collect()
}

fn inst(name: &str, version: &str, direct: bool) -> DependencyInstanceInput {
    DependencyInstanceInput {
        package_name: name.to_string(),
        version: version.to_string(),
        is_direct: direct,
        is_dev: false,
        scope: "unknown".to_string(),
    }
}

fn advisory(db: &Database, id: &str, package: &str, intro: &str, fixed: &str) {
    let ranges = format!(
        r#"[{{"type":"SEMVER","events":[{{"introduced":"{intro}"}},{{"fixed":"{fixed}"}}]}}]"#
    );
    db.upsert_osv_advisory(
        id,
        id,
        None,
        package,
        "npm",
        Some(&ranges),
        Some(&format!(r#"["{fixed}"]"#)),
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("advisory");
}

pub(crate) const EXPRESS: &[&str] = &[
    "4.16.4", "4.19.2", "4.20.0", "4.21.2", "4.22.0", "4.22.3", "5.0.0",
];

/// NodeGoat, cut down: express 4.16.4 pinning path-to-regexp 0.1.7 and qs
/// 6.5.2; express's own advisories end at 4.20.0. A synthetic project path
/// (the DB write guard refuses temp and agent-worktree paths), so no manifest
/// is on disk and no command is named here: the shared vectors check the
/// commands through `direct_fix` on a real project.
pub(crate) fn nodegoat() -> (Database, String) {
    let project = "d:/work/nodegoat".to_string();
    let db = crate::test_utils::test_db();
    db.store_dependency(
        &project,
        "express",
        Some("4.16.4"),
        "javascript",
        false,
        None,
    )
    .expect("declared");
    for (name, version) in [("path-to-regexp", "0.1.7"), ("qs", "6.5.2")] {
        db.store_transitive_dependency(&project, name, Some(version), "javascript", false)
            .expect("transitive");
    }
    db.store_dependency_instances(
        &project,
        "javascript",
        &[
            inst("express", "4.16.4", true),
            inst("path-to-regexp", "0.1.7", false),
            inst("qs", "6.5.2", false),
        ],
    )
    .expect("instances");
    let edge = |child: &str, spec: &str| crate::ace::scanner::DependencyEdge {
        parent: "express".to_string(),
        parent_version: Some("4.16.4".to_string()),
        child: child.to_string(),
        child_version: Some(spec.to_string()),
        scope: crate::ace::scanner::EdgeScope::Runtime,
    };
    db.store_dependency_edges(
        &project,
        "javascript",
        // With no lockfile on disk to name the tool, an exact pin is not read
        // as a requirement; the same pins, spelled as ranges, are.
        &[
            edge("path-to-regexp", ">=0.1.7 <0.1.8"),
            edge("qs", ">=6.5.2 <6.5.3"),
        ],
    )
    .expect("edges");
    advisory(&db, "GHSA-37ch-88jc-xwx2", "path-to-regexp", "0", "0.1.13");
    advisory(&db, "GHSA-4mjr-xmp4-gh2g", "qs", "2.2.5", "6.16.0");
    advisory(&db, "GHSA-qw6h-vgh9-j6wx", "express", "0", "4.20.0");
    let ptr = hist(&[
        ("4.16.4", Some("0.1.7")),
        ("4.19.2", Some("0.1.7")),
        ("4.20.0", Some("0.1.10")),
        ("4.21.2", Some("0.1.12")),
        ("4.22.0", Some("~0.1.12")),
        ("4.22.3", Some("~0.1.13")),
        ("5.0.0", None),
    ]);
    let qs = hist(&[
        ("4.16.4", Some("6.5.2")),
        ("4.19.2", Some("6.11.0")),
        ("4.20.0", Some("6.11.0")),
        ("4.21.2", Some("6.13.0")),
        ("4.22.0", Some("~6.14.0")),
        ("4.22.3", Some("~6.16.0")),
        ("5.0.0", Some("6.13.0")),
    ]);
    crate::osv::release_facts::store_for_tests(
        &db,
        vec![
            (
                "npm",
                "express",
                EXPRESS.to_vec(),
                Some("5.3.0"),
                false,
                vec![("path-to-regexp", ptr), ("qs", qs)],
            ),
            (
                "npm",
                "path-to-regexp",
                vec!["0.1.7", "0.1.10", "0.1.12", "0.1.13"],
                Some("8.4.2"),
                false,
                vec![],
            ),
            (
                "npm",
                "qs",
                vec!["6.5.2", "6.14.2", "6.15.3", "6.16.0", "6.16.1"],
                Some("6.16.1"),
                false,
                vec![],
            ),
        ],
    );
    (db, normalize_path(&project))
}

/// One installed package of a fixture graph: name, version, declared.
pub(crate) type Node<'a> = (&'a str, &'a str, bool);
/// One edge: parent, parent version, child, the parent's requirement (spelled
/// as a range: with no lockfile on disk an exact pin is not read as one).
pub(crate) type Edge<'a> = (&'a str, &'a str, &'a str, &'a str);

/// A project holding `nodes` joined by `edges`, with `advisories`
/// (id, package, introduced, fixed).
pub(crate) fn graph(
    project: &str,
    nodes: &[Node<'_>],
    edges: &[Edge<'_>],
    advisories: &[(&str, &str, &str, &str)],
) -> Database {
    let db = crate::test_utils::test_db();
    for (name, version, direct) in nodes {
        if *direct {
            db.store_dependency(project, name, Some(version), "javascript", false, None)
                .expect("declared");
        } else {
            db.store_transitive_dependency(project, name, Some(version), "javascript", false)
                .expect("transitive");
        }
    }
    let installed: Vec<DependencyInstanceInput> =
        nodes.iter().map(|(n, v, d)| inst(n, v, *d)).collect();
    db.store_dependency_instances(project, "javascript", &installed)
        .expect("instances");
    let edges: Vec<crate::ace::scanner::DependencyEdge> = edges
        .iter()
        .map(|(p, pv, c, req)| crate::ace::scanner::DependencyEdge {
            parent: p.to_string(),
            parent_version: Some(pv.to_string()),
            child: c.to_string(),
            child_version: Some(req.to_string()),
            scope: crate::ace::scanner::EdgeScope::Runtime,
        })
        .collect();
    db.store_dependency_edges(project, "javascript", &edges)
        .expect("edges");
    for (id, package, intro, fixed) in advisories {
        advisory(&db, id, package, intro, fixed);
    }
    db
}

/// The persisted-plan repro (review of #981): one declared lever, vitest
/// 4.0.0, reached from three rows: its own advisory (fixed 4.1.0), a child
/// whose fix needs a higher vitest (@vitest/mocker, fixed 4.1.11, pinned
/// exactly by each vitest), and a child no vitest release admits a fixed copy
/// of (tinypool, fixed 3.0.0; vitest 5.0.2 still requires ^2). One release,
/// vitest 4.1.11, must appear on every row and surface.
pub(crate) fn vitest() -> (Database, String) {
    let project = "d:/work/vitest-app";
    let db = graph(
        project,
        &[
            ("vitest", "4.0.0", true),
            ("@vitest/mocker", "4.0.0", false),
            ("tinypool", "1.0.0", false),
        ],
        &[
            ("vitest", "4.0.0", "@vitest/mocker", ">=4.0.0 <4.0.1"),
            ("vitest", "4.0.0", "tinypool", "^1.0.0"),
        ],
        &[
            ("GHSA-vite-0000-0001", "vitest", "0", "4.1.0"),
            ("GHSA-mock-0000-0001", "@vitest/mocker", "0", "4.1.11"),
            ("GHSA-pool-0000-0001", "tinypool", "0", "3.0.0"),
        ],
    );
    let vitest = ["4.0.0", "4.1.0", "4.1.11", "5.0.2"];
    let mocker = hist(&vitest.map(|v| (v, Some(v))));
    let pool = hist(&[
        ("4.0.0", Some("^1.0.0")),
        ("4.1.0", Some("^1.0.0")),
        ("4.1.11", Some("^1.1.0")),
        ("5.0.2", Some("^2.0.0")),
    ]);
    let pools = vec!["1.0.0", "1.1.0", "2.0.0", "3.0.0"];
    crate::osv::release_facts::store_for_tests(
        &db,
        vec![
            (
                "npm",
                "vitest",
                vitest.to_vec(),
                Some("5.0.2"),
                false,
                vec![("@vitest/mocker", mocker), ("tinypool", pool)],
            ),
            (
                "npm",
                "@vitest/mocker",
                vitest.to_vec(),
                Some("5.0.2"),
                false,
                vec![],
            ),
            ("npm", "tinypool", pools, Some("3.0.0"), false, vec![]),
        ],
    );
    (db, normalize_path(project))
}

/// leaf 1.0.0 (fixed 1.1.0) pinned by mid 2.0.0, which arrives through the
/// declared `a` 1.0.0 and `b` 1.0.0 (review of #981, S2) and, with
/// `undeclared`, also through `c` 1.0.0, which no manifest declares. With
/// `a_row`, `a` has a row of its own (an advisory fixed in 1.0.1) that is
/// raised to 1.2.0 by zed 1.0.0, which `a` pins and only a 1.2.0 frees: the
/// leaf row's smallest `a` release, 1.1.0, then lifts to 1.2.0 (S4).
pub(crate) fn levers(undeclared: bool, a_row: bool) -> (Database, String) {
    let project = "d:/work/levers";
    let mut nodes: Vec<Node<'_>> = vec![
        ("a", "1.0.0", true),
        ("b", "1.0.0", true),
        ("mid", "2.0.0", false),
        ("leaf", "1.0.0", false),
    ];
    let mut edges: Vec<Edge<'_>> = vec![
        ("a", "1.0.0", "mid", ">=2.0.0 <2.0.1"),
        ("b", "1.0.0", "mid", ">=2.0.0 <2.0.1"),
        ("mid", "2.0.0", "leaf", ">=1.0.0 <1.0.1"),
    ];
    if undeclared {
        nodes.push(("c", "1.0.0", false));
        edges.push(("c", "1.0.0", "mid", ">=2.0.0 <2.0.1"));
    }
    let mut advisories = vec![("GHSA-leaf-0000-0001", "leaf", "0", "1.1.0")];
    if a_row {
        nodes.push(("zed", "1.0.0", false));
        edges.push(("a", "1.0.0", "zed", ">=1.0.0 <1.0.1"));
        advisories.push(("GHSA-aaaa-0000-0001", "a", "0", "1.0.1"));
        advisories.push(("GHSA-zzzz-0000-0001", "zed", "0", "1.1.0"));
    }
    let db = graph(project, &nodes, &edges, &advisories);
    let on_mid = hist(&[
        ("1.0.0", Some("2.0.0")),
        ("1.0.1", Some("2.0.0")),
        ("1.1.0", Some("2.1.0")),
        ("1.2.0", Some("2.1.0")),
    ]);
    let on_zed = hist(&[
        ("1.0.0", Some("1.0.0")),
        ("1.0.1", Some("1.0.0")),
        ("1.1.0", Some("1.0.0")),
        ("1.2.0", Some("1.1.0")),
    ]);
    let on_leaf = hist(&[("2.0.0", Some("1.0.0")), ("2.1.0", Some("^1.1.0"))]);
    let a_releases = vec!["1.0.0", "1.0.1", "1.1.0", "1.2.0"];
    let pairs = vec!["1.0.0", "1.1.0"];
    crate::osv::release_facts::store_for_tests(
        &db,
        vec![
            (
                "npm",
                "a",
                a_releases.clone(),
                Some("1.2.0"),
                false,
                vec![("mid", on_mid.clone()), ("zed", on_zed)],
            ),
            (
                "npm",
                "b",
                a_releases,
                Some("1.2.0"),
                false,
                vec![("mid", on_mid)],
            ),
            (
                "npm",
                "mid",
                vec!["2.0.0", "2.1.0"],
                Some("2.1.0"),
                false,
                vec![("leaf", on_leaf)],
            ),
            ("npm", "leaf", pairs.clone(), Some("1.1.0"), false, vec![]),
            ("npm", "zed", pairs, Some("1.1.0"), false, vec![]),
        ],
    );
    (db, normalize_path(project))
}
