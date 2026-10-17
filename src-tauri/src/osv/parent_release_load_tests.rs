// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The parent release end to end through the database: NodeGoat's express
//! 4.16.4 (registry and OSV data of 2026-10-18, cut down).

use crate::db::DependencyInstanceInput;
use crate::osv::fix_target::{InstallSite, LineTarget, UpgradeType};
use crate::osv::parent_release::{ReleaseReq, RequirementHistory};

use super::*;

fn hist(pairs: &[(&str, Option<&str>)]) -> RequirementHistory {
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

const EXPRESS: &[&str] = &[
    "4.16.4", "4.19.2", "4.20.0", "4.21.2", "4.22.0", "4.22.3", "5.0.0",
];

/// NodeGoat, cut down: express 4.16.4 pinning path-to-regexp 0.1.7 and qs
/// 6.5.2; express's own advisories end at 4.20.0. A synthetic project path
/// (the DB write guard refuses temp and agent-worktree paths), so no manifest
/// is on disk and no command is named here: the shared vectors check the
/// commands through `direct_fix` on a real project.
fn nodegoat() -> (Database, String) {
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

fn ptr_line(project: &str) -> Vec<LineTarget> {
    vec![LineTarget {
        installed_version: "0.1.7".into(),
        target_version: Some("0.1.13".into()),
        clears_all_known: true,
        upgrade_type: Some(UpgradeType::Patch),
        sites: vec![InstallSite {
            project_path: project.to_string(),
            is_direct: false,
            is_dev: false,
        }],
    }]
}

#[test]
fn nodegoat_names_express_4_22_3_on_the_path_to_regexp_row() {
    let (db, project) = nodegoat();
    let routes =
        super::super::parent_hint::load_routes(&db, "npm", "path-to-regexp", &ptr_line(&project));
    let link = routes.links.first().expect("express links it");
    let ParentStanding::Admits(r) = &link.standing else {
        panic!("no release named: {:?}", link.standing);
    };
    // 4.22.0 is the smallest; qs needs 4.22.3, and express's own row moves there.
    assert_eq!(r.to, "4.22.3");
    assert_eq!(r.smallest.as_deref(), Some("4.22.0"));
    assert_eq!(r.requirement.as_deref(), Some("~0.1.13"));
    assert_eq!(r.resolves_to.as_deref(), Some("0.1.13"));
    assert!(!r.breaking && !r.lever_unknown && r.kept.is_none());
    assert_eq!(
        r.command, None,
        "no manifest on disk to keep the operator of"
    );

    // The express row agrees: 4.22.3, not its own advisories' 4.20.0.
    let raised = raised_row_target(&db, "npm", &project, "express", "4.16.4", "4.20.0");
    assert_eq!(
        raised,
        Some((
            "4.22.3".to_string(),
            vec!["path-to-regexp".to_string(), "qs".to_string()]
        ))
    );
}

#[test]
fn a_parent_never_read_keeps_the_registry_row_standing() {
    let (db, project) = nodegoat();
    crate::osv::release_facts::store_for_tests(&db, vec![]);
    let routes =
        super::super::parent_hint::load_routes(&db, "npm", "path-to-regexp", &ptr_line(&project));
    let link = routes.links.first().expect("express links it");
    assert!(
        !matches!(link.standing, ParentStanding::Admits(_)),
        "{:?}",
        link.standing
    );
}

#[test]
fn the_walk_asks_for_the_parents_that_pin_a_vulnerable_copy() {
    let (db, _project) = nodegoat();
    let matches = crate::osv::matching::get_matched_advisories(&db).unwrap_or_default();
    let wanted = wanted_parents(&db, &matches);
    assert!(
        wanted.contains(&("express".to_string(), "path-to-regexp".to_string())),
        "{wanted:?}"
    );
    assert!(
        wanted.contains(&("express".to_string(), "qs".to_string())),
        "{wanted:?}"
    );
}

/// Before: "express must move (4DA holds no record of an express release that
/// admits path-to-regexp >= 0.1.13): upgrade express if one does". After: the
/// plan names express 4.22.3 on all three rows.
#[test]
fn the_plan_names_one_express_release_on_every_row() {
    let (db, _project) = nodegoat();
    let items = crate::evidence::build_upgrade_plan(&db).items;
    let find = |pkg: &str| {
        items
            .iter()
            .find(|i| i.affected_deps.iter().any(|d| d == pkg))
            .unwrap_or_else(|| panic!("no {pkg} step"))
    };
    let express = find("express");
    assert!(express.title.contains(">= 4.22.3"), "{}", express.title);
    assert!(
        express
            .explanation
            .contains("Upgrade to express 4.22.3, not 4.20.0"),
        "{}",
        express.explanation
    );
    let ptr = find("path-to-regexp");
    assert!(
        ptr.explanation
            .contains("so one bump serves both, and no advisory of express affects it: upgrade express to 4.22.3"),
        "{}",
        ptr.explanation
    );
    assert!(
        !ptr.explanation.contains("4DA holds no record"),
        "{}",
        ptr.explanation
    );
    assert!(
        ptr.evidence
            .iter()
            .any(|c| c.title == "express 4.22.3 -> path-to-regexp ~0.1.13"),
        "{:?}",
        ptr.evidence.iter().map(|c| &c.title).collect::<Vec<_>>()
    );
    let qs = find("qs");
    assert!(
        qs.explanation.contains("upgrade express to 4.22.3"),
        "{}",
        qs.explanation
    );
}
