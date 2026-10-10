// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! End-to-end plan tests for the parent hint, the runtime-over-dev tiebreak
//! and severity truth — real advisory ids, ranges and versions from the live
//! 2026-10-02 Preemption audit (project paths neutralised).

use crate::ace::scanner::{DependencyEdge, EdgeScope};
use crate::db::{Database, DependencyInstanceInput};
use crate::evidence::{validate_item, EvidenceItem, Urgency};
use crate::test_utils::test_db;

const BRIDGE: &str = "d:/work/bridge/src-tauri";
const APP: &str = "d:/work/app/src-tauri";
const WEB: &str = "d:/work/web";
const MCP: &str = "d:/work/app/mcp-server";

fn plan(db: &Database) -> Vec<EvidenceItem> {
    crate::evidence::upgrade_plan::build_upgrade_plan_with_drops(db).0
}

fn advisory(db: &Database, id: &str, package: &str, eco: &str, fixed: &str, cvss: f64) {
    db.upsert_osv_advisory(
        id,
        &format!("{package} advisory {id}"),
        None,
        package,
        eco,
        Some(&format!(
            r#"[{{"type":"SEMVER","events":[{{"introduced":"0"}},{{"fixed":"{fixed}"}}]}}]"#
        )),
        Some(&format!(r#"["{fixed}"]"#)),
        Some("CVSS_V3"),
        Some(cvss),
        Some(&format!("https://osv.dev/{id}")),
        Some("2026-09-01T00:00:00Z"),
        None,
        None,
    )
    .unwrap();
}

fn instance(name: &str, version: &str, direct: bool, dev: bool) -> DependencyInstanceInput {
    DependencyInstanceInput {
        package_name: name.to_string(),
        version: version.to_string(),
        is_direct: direct,
        is_dev: dev,
        scope: if dev { "dev" } else { "runtime" }.to_string(),
    }
}

fn cargo_edge(parent: &str, version: &str, child: &str) -> DependencyEdge {
    DependencyEdge {
        parent: parent.to_string(),
        parent_version: Some(version.to_string()),
        child: child.to_string(),
        child_version: None,
        scope: EdgeScope::Runtime,
    }
}

/// The bridge holds rmcp 1.7.0 only through victauri-plugin 0.8.4; the app
/// runs victauri-plugin 0.9.0 resolved against rmcp 3.4.1.
fn seed_rmcp(db: &Database) {
    db.store_transitive_dependency(BRIDGE, "rmcp", Some("1.7.0"), "rust", false)
        .unwrap();
    db.store_dependency_instances(
        BRIDGE,
        "rust",
        &[
            instance("rmcp", "1.7.0", false, false),
            instance("victauri-plugin", "0.8.4", true, false),
        ],
    )
    .unwrap();
    db.store_dependency_edges(
        BRIDGE,
        "rust",
        &[
            cargo_edge("bridge", "0.1.0", "victauri-plugin"),
            cargo_edge("victauri-plugin", "0.8.4", "rmcp"),
        ],
    )
    .unwrap();
    db.store_dependency(APP, "victauri-plugin", Some("0.9.0"), "rust", false, None)
        .unwrap();
    db.store_dependency_instances(
        APP,
        "rust",
        &[
            instance("rmcp", "3.4.1", false, false),
            instance("victauri-plugin", "0.9.0", true, false),
        ],
    )
    .unwrap();
    db.store_dependency_edges(
        APP,
        "rust",
        &[
            // A stale edge from an older install the rescan never deleted.
            cargo_edge("victauri-plugin", "0.8.5", "rmcp"),
            cargo_edge("victauri-plugin", "0.9.0", "rmcp"),
        ],
    )
    .unwrap();
    advisory(db, "GHSA-33f5-2c5q-wgwj", "rmcp", "crates.io", "2.0.0", 8.2);
    advisory(db, "GHSA-9pj6-vhgr-3mwh", "rmcp", "crates.io", "2.0.0", 7.5);
    advisory(db, "GHSA-9g45-5xwm-f3wc", "rmcp", "crates.io", "2.1.0", 6.8);
}

fn row<'a>(plan: &'a [EvidenceItem], package: &str) -> &'a EvidenceItem {
    plan.iter()
        .find(|i| i.affected_deps == vec![package.to_string()])
        .unwrap_or_else(|| {
            panic!(
                "{package} missing: {:?}",
                plan.iter().map(|i| &i.title).collect::<Vec<_>>()
            )
        })
}

#[test]
fn a_transitive_step_names_its_parent_and_the_newer_parent_seen_here() {
    let db = test_db();
    seed_rmcp(&db);
    let plan = plan(&db);
    let rmcp = row(&plan, "rmcp");
    validate_item(rmcp).expect("schema-valid");
    assert_eq!(
        rmcp.title,
        "Upgrade rmcp to >= 2.1.0 (major) via victauri-plugin — clears 3 advisories across 1 project"
    );
    assert!(
        rmcp.explanation
            .contains("rmcp 1.7.0 comes in through victauri-plugin 0.8.4 (a direct dependency)"),
        "{}",
        rmcp.explanation
    );
    assert!(
        rmcp.explanation
            .contains("victauri-plugin 0.9.0 resolves rmcp 3.4.1 (d:/work/app/src-tauri)"),
        "{}",
        rmcp.explanation
    );
    // The stale 0.8.5 edge is not an install: never cited.
    assert!(!rmcp.explanation.contains("0.8.5"), "{}", rmcp.explanation);
    let paths: Vec<&str> = rmcp
        .evidence
        .iter()
        .filter(|c| c.source == "dependency-path")
        .map(|c| c.title.as_str())
        .collect();
    assert_eq!(
        paths,
        vec![
            "rmcp 1.7.0 <- victauri-plugin 0.8.4",
            "victauri-plugin 0.9.0 -> rmcp 3.4.1"
        ]
    );
}

/// No recorded parent: the route rests on semver alone, and says so. rmcp
/// 1.7.0 -> 2.1.0 crosses a semver boundary, so a refresh cannot reach it.
#[test]
fn without_a_recorded_parent_the_route_is_the_semver_inference() {
    let db = test_db();
    db.store_transitive_dependency(BRIDGE, "rmcp", Some("1.7.0"), "rust", false)
        .unwrap();
    advisory(
        &db,
        "GHSA-9g45-5xwm-f3wc",
        "rmcp",
        "crates.io",
        "2.1.0",
        6.8,
    );
    let plan = plan(&db);
    let rmcp = row(&plan, "rmcp");
    assert!(
        rmcp.explanation.contains(
            "Fixed only upstream: rmcp 1.7.0 is transitive (the lockfile names no parent 4DA can \
             read); 2.1.0 is a semver-incompatible jump, so the dependency that pulls it in must \
             be updated"
        ),
        "{}",
        rmcp.explanation
    );
    assert!(!rmcp.title.contains(" via "), "{}", rmcp.title);
}

/// Live 2026-10-02: both HIGH rows tied on urgency and the dev-only vitest
/// (CVSS 9.8, discounted to High for being dev-only) led the runtime rmcp
/// session leak (8.2, High) because it is fixable now. Runtime leads.
#[test]
fn a_runtime_exposure_outranks_a_dev_only_one_at_equal_urgency() {
    let db = test_db();
    seed_rmcp(&db);
    db.store_dependency(WEB, "vitest", Some("3.2.4"), "npm", true, None)
        .unwrap();
    db.store_dependency_instances(WEB, "npm", &[instance("vitest", "3.2.4", true, true)])
        .unwrap();
    advisory(&db, "GHSA-5xrq-8626-4rwp", "vitest", "npm", "3.2.6", 9.8);
    advisory(&db, "GHSA-82fw-gwwq-j7x9", "vitest", "npm", "4.1.11", 5.9);

    let plan = plan(&db);
    let pos = |p: &str| {
        plan.iter()
            .position(|i| i.affected_deps == vec![p.to_string()])
    };
    assert_eq!(
        row(&plan, "vitest").urgency,
        Urgency::High,
        "Critical, dev-only"
    );
    assert_eq!(row(&plan, "rmcp").urgency, Urgency::High);
    assert!(
        row(&plan, "vitest").explanation.contains("dev-only"),
        "the dev-only discount is labelled"
    );
    assert!(
        pos("rmcp") < pos("vitest"),
        "{:?}",
        plan.iter().map(|i| &i.title).collect::<Vec<_>>()
    );
}

/// GHSA-hxh3-vqpv-xpqv (hono/jsx XSS, CVSS 4.7, fixed 4.13.7): the app's MCP
/// server runs hono 4.13.9 and must not be named; a dormant clone on 4.11.1
/// is, at the advisory's own MEDIUM — never Critical.
#[test]
fn a_patched_hono_is_not_listed_and_the_affected_copy_keeps_advisory_severity() {
    let db = test_db();
    db.store_dependency(MCP, "hono", Some("4.13.9"), "npm", false, None)
        .unwrap();
    db.store_dependency_instances(MCP, "npm", &[instance("hono", "4.13.9", true, false)])
        .unwrap();
    db.store_transitive_dependency(WEB, "hono", Some("4.11.1"), "npm", false)
        .unwrap();
    db.store_dependency_instances(WEB, "npm", &[instance("hono", "4.11.1", false, false)])
        .unwrap();
    advisory(&db, "GHSA-hxh3-vqpv-xpqv", "hono", "npm", "4.13.7", 4.7);

    // The matcher every Preemption surface (plan, alerts, free floor, brief)
    // reads: the patched copy is not an instance of the advisory at all.
    let matches = crate::osv::matching::get_matched_advisories(&db).unwrap();
    let hxh3 = matches
        .iter()
        .find(|m| m.advisory_id == "GHSA-hxh3-vqpv-xpqv")
        .expect("the 4.11.1 copy matches");
    assert_eq!(hxh3.project_paths, vec![WEB.to_string()]);
    assert!(hxh3
        .dependency_instances
        .iter()
        .all(|d| d.installed_version.as_deref() != Some("4.13.9")));

    let plan = plan(&db);
    let hono = row(&plan, "hono");
    assert_eq!(hono.affected_projects, vec![WEB.to_string()]);
    assert_eq!(hono.urgency, Urgency::Medium);
    assert!(
        plan.iter()
            .all(|i| !i.affected_projects.contains(&MCP.to_string())),
        "the patched MCP server is named nowhere"
    );
}

/// Live 2026-10-02: a brace-expansion row whose parent range already admits
/// the fix also read "Seen in your projects: minimatch 10.2.6 resolves
/// brace-expansion 5.0.12" — a major parent jump nobody needs. When a refresh
/// is enough, no newer parent is offered.
#[test]
fn a_refresh_is_enough_so_no_newer_parent_is_offered() {
    use crate::osv::fix_path::{Basis, Refresh};
    use crate::osv::parent_hint::{ParentLink, Resolution, TransitiveRoutes};
    let link = ParentLink {
        project: WEB.to_string(),
        child_version: "1.1.15".to_string(),
        target: Some("1.1.21".to_string()),
        parent: "minimatch".to_string(),
        parent_version: "3.1.5".to_string(),
        parent_is_direct: false,
        requirement: Some("^1.1.7".to_string()),
        refresh: Refresh::Enough(Basis::Requirement),
        command: Some("npm update brace-expansion".to_string()),
        resolutions: vec![Resolution {
            parent_version: "10.2.6".to_string(),
            child_version: "5.0.12".to_string(),
            project: APP.to_string(),
        }],
    };
    let links = std::slice::from_ref(&link);
    let routes = TransitiveRoutes {
        links: vec![link.clone()],
        unlinked: Vec::new(),
    };
    assert_eq!(
        super::upstream_note("brace-expansion", &routes, false),
        "No manifest change needed: brace-expansion 1.1.15 comes in through minimatch 3.1.5, \
         whose requirement ^1.1.7 already admits 1.1.21 — a lockfile refresh is enough. \
         Run: `npm update brace-expansion`"
    );
    let cites = super::path_citations("brace-expansion", links);
    assert_eq!(cites.len(), 1, "{cites:?}");
    assert_eq!(super::single_parent(links), None);
}
