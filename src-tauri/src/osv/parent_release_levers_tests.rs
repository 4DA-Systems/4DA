// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Review of #981: every declared dependency a parent arrives through is
//! named (S2), a lever's release agrees with its own row (S4), one release
//! per lever on every row and surface (the persisted vitest plan), and the
//! newest release named is never an affected one.

use crate::evidence::EvidenceItem;
use crate::osv::fix_target::{InstallSite, LineTarget, UpgradeType};
use crate::osv::parent_hint::{load_routes, ParentLink, ParentStanding};
use crate::osv::parent_release_fixture::{graph, levers, vitest};

fn line(installed: &str, target: &str, project: &str) -> Vec<LineTarget> {
    vec![LineTarget {
        installed_version: installed.into(),
        target_version: Some(target.into()),
        clears_all_known: true,
        introduces: Vec::new(),
        malicious: Vec::new(),
        upgrade_type: Some(UpgradeType::Minor),
        sites: vec![InstallSite {
            project_path: project.to_string(),
            is_direct: false,
            is_dev: false,
        }],
    }]
}

fn leaf_link(db: &crate::db::Database, project: &str) -> ParentLink {
    let routes = load_routes(db, "npm", "leaf", &line("1.0.0", "1.1.0", project));
    routes.links.into_iter().next().expect("mid links leaf")
}

/// (lever, its named release, the mid release it brings in).
fn lever_releases(link: &ParentLink) -> Vec<(String, Option<String>, Option<String>)> {
    link.through
        .iter()
        .flat_map(|t| t.all())
        .map(|l| match &l.standing {
            ParentStanding::Admits(r) => (l.name.clone(), Some(r.to.clone()), r.brings.clone()),
            _ => (l.name.clone(), None, None),
        })
        .collect()
}

#[test]
fn a_parent_reached_through_two_declared_dependencies_names_both() {
    let (db, project) = levers(false, false);
    let link = leaf_link(&db, &project);
    assert!(link.lever_unknown.is_none(), "{:?}", link.lever_unknown);
    assert_eq!(
        lever_releases(&link),
        [
            (
                "a".to_string(),
                Some("1.1.0".to_string()),
                Some("2.1.0".to_string())
            ),
            (
                "b".to_string(),
                Some("1.1.0".to_string()),
                Some("2.1.0".to_string())
            ),
        ]
    );
    let text = crate::evidence::transitive_route_note(
        &db,
        "npm",
        "leaf",
        &line("1.0.0", "1.1.0", &project),
        false,
    )
    .unwrap_or_default();
    assert!(
        text.contains(
            "mid arrives through a 1.0.0 and b 1.0.0, direct dependencies, each of which must move"
        ),
        "{text}"
    );
    assert!(
        text.contains("upgrade a to 1.1.0") && text.contains("upgrade b to 1.1.0"),
        "{text}"
    );
}

#[test]
fn a_path_ending_at_an_undeclared_package_names_no_lever_and_no_command() {
    let (db, project) = levers(true, false);
    let link = leaf_link(&db, &project);
    assert!(link.through.is_none());
    assert!(
        link.lever_unknown
            .as_deref()
            .is_some_and(|w| w.contains('c')),
        "{:?}",
        link.lever_unknown
    );
    let ParentStanding::Admits(r) = &link.standing else {
        panic!("mid's release is still named: {:?}", link.standing);
    };
    assert_eq!(r.to, "2.1.0");
    assert!(r.command.is_none() && r.lever_unknown.is_some());
    let text = crate::evidence::transitive_route_note(
        &db,
        "npm",
        "leaf",
        &line("1.0.0", "1.1.0", &project),
        false,
    )
    .unwrap_or_default();
    assert!(text.contains("could not be worked out"), "{text}");
    assert!(
        !text.contains("upgrade a ") && !text.contains("upgrade b "),
        "{text}"
    );
}

#[test]
fn a_levers_release_agrees_with_its_own_row() {
    // a's own row: 1.0.1 for its own advisory, raised to 1.2.0 by zed; the
    // leaf row's smallest a release (1.1.0) lifts to it. b has no row: 1.1.0.
    let (db, project) = levers(false, true);
    let link = leaf_link(&db, &project);
    assert_eq!(
        lever_releases(&link),
        [
            (
                "a".to_string(),
                Some("1.2.0".to_string()),
                Some("2.1.0".to_string())
            ),
            (
                "b".to_string(),
                Some("1.1.0".to_string()),
                Some("2.1.0".to_string())
            ),
        ]
    );
    let raised = super::raised_row_target(&db, "npm", &project, "a", "1.0.0", "1.0.1");
    assert_eq!(raised.map(|(to, _)| to).as_deref(), Some("1.2.0"));
}

fn item<'a>(items: &'a [EvidenceItem], pkg: &str) -> &'a EvidenceItem {
    items
        .iter()
        .find(|i| i.affected_deps.iter().any(|d| d == pkg))
        .unwrap_or_else(|| panic!("no {pkg} step"))
}

/// The persisted plan named vitest 4.1.11 on the vitest and @vitest/mocker
/// rows while the tinypool row said "upgrade vitest (vitest 5.0.2 is the
/// newest release ...)". One release per lever, everywhere.
#[test]
fn one_vitest_release_on_every_row_and_surface() {
    let (db, project) = vitest();
    let items = crate::evidence::build_upgrade_plan(&db).items;
    let own = item(&items, "vitest");
    assert!(own.title.contains(">= 4.1.11"), "{}", own.title);
    let mocker = item(&items, "@vitest/mocker");
    assert!(
        mocker.explanation.contains("upgrade vitest to 4.1.11"),
        "{}",
        mocker.explanation
    );
    let pool = item(&items, "tinypool");
    assert!(
        pool.explanation
            .contains("no vitest release up to its newest"),
        "{}",
        pool.explanation
    );
    for i in [own, mocker, pool] {
        for other in [
            "vitest 5.0.2",
            "upgrade vitest (",
            "upgrade vitest to 4.1.0",
        ] {
            assert!(!i.explanation.contains(other), "{other}: {}", i.explanation);
        }
    }
    // Preemption's own vitest alert.
    let matches = crate::osv::matching::get_matched_advisories(&db).unwrap_or_default();
    let group: Vec<_> = matches
        .iter()
        .filter(|m| m.package_name == "vitest")
        .collect();
    let raised = crate::osv::raised_lines::raised_line_targets(&db, &group, &[project]);
    let (fix, _) = crate::preemption::alert_fix_target(&group, &raised.lines);
    assert_eq!(fix.as_deref(), Some("4.1.11"));
}

/// The coordinator's parity check: the newest release a parent's standing
/// names is the highest stable one that is not withdrawn and that no stored
/// advisory of it affects (a malware release is never "the newest").
#[test]
fn the_newest_release_named_is_never_an_affected_one() {
    let db = graph(
        "d:/work/newest",
        &[("par", "1.0.0", true)],
        &[],
        &[("MAL-2026-0001", "par", "2.0.0", "2.0.1")],
    );
    crate::osv::release_facts::store_for_tests(
        &db,
        vec![(
            "npm",
            "par",
            vec!["1.0.0", "1.1.0", "2.0.0"],
            Some("2.0.0"),
            false,
            vec![],
        )],
    );
    assert_eq!(
        crate::osv::parent_hint::parent_standing(&db, "npm", "par", "1.0.0"),
        ParentStanding::Newer {
            latest: "1.1.0".into()
        }
    );
}

/// Review of #994, B3: newer releases exist, and only an advisory the
/// installed parent does not have rules them out. That is said, never
/// "already the newest release".
#[test]
fn newer_releases_that_bring_in_an_advisory_are_not_the_newest() {
    let db = graph(
        "d:/work/introduces",
        &[("par", "1.0.0", true)],
        &[],
        &[("GHSA-synt-b3", "par", "1.1.0", "9.0.0")],
    );
    crate::osv::release_facts::store_for_tests(
        &db,
        vec![(
            "npm",
            "par",
            vec!["1.0.0", "1.1.0"],
            Some("1.1.0"),
            false,
            vec![],
        )],
    );
    assert_eq!(
        crate::osv::parent_hint::parent_standing(&db, "npm", "par", "1.0.0"),
        ParentStanding::Introduces {
            advisories: vec!["GHSA-synt-b3".into()],
        }
    );
    // A deprecated package keeps its deprecated outcome first, as the MCP
    // decides (review of #1008, S3): never Introduces.
    db.upsert_source_item(
        "npm",
        "dep-par",
        None,
        "[DEPRECATED] par",
        "",
        &crate::test_utils::seed_embedding("dep-par"),
    )
    .expect("deprecation row");
    assert_eq!(
        crate::osv::parent_hint::parent_standing(&db, "npm", "par", "1.0.0"),
        ParentStanding::Newest { deprecated: true }
    );
}

/// Follow-up of #994, B3: with no usable release up to the installed one
/// (the installed par has a fixed advisory) and the newer ones only bringing
/// an advisory in, the standing is Introduces, not Unknown.
#[test]
fn nothing_usable_with_newer_releases_that_bring_one_in_is_introduces() {
    let db = graph(
        "d:/work/introduces-unknown",
        &[("par", "1.0.0", true)],
        &[],
        &[
            ("GHSA-synt-own", "par", "0", "1.0.5"),
            ("GHSA-synt-new", "par", "1.1.0", "9.0.0"),
        ],
    );
    crate::osv::release_facts::store_for_tests(
        &db,
        vec![(
            "npm",
            "par",
            vec!["1.0.0", "1.1.0"],
            Some("1.1.0"),
            false,
            vec![],
        )],
    );
    assert_eq!(
        crate::osv::parent_hint::parent_standing(&db, "npm", "par", "1.0.0"),
        ParentStanding::Introduces {
            advisories: vec!["GHSA-synt-new".into()],
        }
    );
}

/// Review of #981, B2c and B1: the release named says what unfixed advisory
/// it still carries, and a lifted lever names its own smallest release
/// apart from the parent release it brings in.
#[test]
fn the_named_release_states_what_it_still_carries_and_where_it_was_lifted_from() {
    use crate::osv::parent_hint::{ParentRelease, Through};
    let r = ParentRelease {
        to: "2.0.0".into(),
        requirement: Some("^1.1.0".into()),
        remains: vec!["CVE-2026-0001".into()],
        ..ParentRelease::default()
    };
    let text = crate::evidence::parent_move(
        "leaf",
        "1.1.0",
        "par",
        "1.0.0",
        &ParentStanding::Admits(Box::new(r)),
        None,
        None,
    );
    assert!(
        text.contains("though par 2.0.0 remains affected by CVE-2026-0001 (no fix published)"),
        "{text}"
    );
    let lever = Through {
        standing: ParentStanding::Admits(Box::new(ParentRelease {
            to: "4.22.3".into(),
            smallest: Some("4.21.0".into()),
            floor: Some("1.16.1".into()),
            brings: Some("1.16.3".into()),
            ..ParentRelease::default()
        })),
        ..Through::declared("express", "4.16.4")
    };
    let parent = ParentRelease {
        to: "1.16.1".into(),
        requirement: Some("0.19.0".into()),
        ..ParentRelease::default()
    };
    let text = crate::evidence::parent_move(
        "send",
        "0.19.0",
        "serve-static",
        "1.13.2",
        &ParentStanding::Admits(Box::new(parent)),
        Some(&lever),
        None,
    );
    assert!(
        text.contains(
            "express 4.21.0 is the smallest express release that brings in a serve-static that \
             admits it, and express 4.22.3, the release express's own upgrade moves to, also \
             brings in serve-static 1.16.3 (>= serve-static 1.16.1): upgrade express to 4.22.3"
        ),
        "{text}"
    );
}

/// Parity decision 2: the MCP's exact words when every fixing release
/// brings in an advisory the installed version does not have.
#[test]
fn every_fixing_release_bringing_in_an_advisory_names_none() {
    let text = crate::evidence::parent_move(
        "leaf",
        "1.1.0",
        "par",
        "1.0.0",
        &ParentStanding::Introduces {
            advisories: vec!["GHSA-synt-0000-0002".into()],
        },
        None,
        None,
    );
    assert!(
        text.contains(
            "every fixing release brings in GHSA-synt-0000-0002, which the installed version does \
             not have"
        ),
        "{text}"
    );
    assert!(!text.contains("upgrade par to"), "{text}");
}
