// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The parent release end to end through the database: NodeGoat's express
//! 4.16.4 (registry and OSV data of 2026-10-18, cut down).

use crate::osv::fix_target::{InstallSite, LineTarget, UpgradeType};
use crate::osv::parent_release_fixture::nodegoat;

use super::*;

fn ptr_line(project: &str) -> Vec<LineTarget> {
    vec![LineTarget {
        installed_version: "0.1.7".into(),
        target_version: Some("0.1.13".into()),
        clears_all_known: true,
        introduces: Vec::new(),
        malicious: None,
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
    assert!(!r.breaking && r.lever_unknown.is_none() && r.kept.is_none());
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

/// Review of #981 (B1): Preemption's own express alert names the release the
/// route sentences name, not its own advisories' 4.20.0, which still pins a
/// vulnerable path-to-regexp and qs.
#[test]
fn the_express_alert_names_4_22_3_too() {
    let (db, project) = nodegoat();
    let matches = crate::osv::matching::get_matched_advisories(&db).unwrap_or_default();
    let group: Vec<&crate::osv::types::MatchedAdvisory> = matches
        .iter()
        .filter(|m| m.package_name == "express")
        .collect();
    assert!(!group.is_empty(), "express is matched");
    let projects = vec![project];
    let raised = crate::osv::raised_lines::raised_line_targets(&db, &group, &projects);
    let (fix, sentence) = crate::preemption::alert_fix_target(&group, &raised.lines);
    assert_eq!(fix.as_deref(), Some("4.22.3"));
    assert!(sentence.contains("Update to >= 4.22.3"), "{sentence}");
    assert!(raised.note.contains("not 4.20.0"), "{}", raised.note);
}
