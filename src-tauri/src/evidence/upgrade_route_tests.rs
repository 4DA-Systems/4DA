// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The route a transitive copy's fix takes, decided from the data the plan
//! already holds (audit 2026-11-23, registry facts of 2026-11-22): a parent
//! whose installed declared dependency already admits the release that fixes
//! the copy moves by a lockfile refresh, and nothing the project declares is
//! upgraded. The plan named "tailwindcss 3.4.19" for navcal's glob and
//! "@vscode/vsce 3.9.2" for victauri's @azure/msal-node, both `parent_update`,
//! while `npm update sucrase` / `npm update @azure/identity` fix them. A copy
//! no parent release admits is blocked upstream, with its hold.

use crate::ace::scanner::{DependencyEdge, EdgeScope};
use crate::db::{Database, DependencyInstanceInput};
use crate::evidence::upgrade_steps::{Mechanism, UpgradeStep};
use crate::evidence::{validate_item, EvidenceItem};
use crate::osv::parent_release_fixture::hist;
use crate::test_utils::test_db;

fn advisory(db: &Database, id: &str, package: &str, fixed: &str) {
    db.upsert_osv_advisory(
        id,
        &format!("{package} advisory {id}"),
        None,
        package,
        "npm",
        Some(&format!(
            r#"[{{"type":"SEMVER","events":[{{"introduced":"0"}},{{"fixed":"{fixed}"}}]}}]"#
        )),
        Some(&format!(r#"["{fixed}"]"#)),
        Some("CVSS_V3"),
        Some(5.3),
        Some(&format!("https://osv.dev/{id}")),
        Some("2026-09-01T00:00:00Z"),
        None,
        None,
    )
    .unwrap();
}

fn dev(name: &str, version: &str, direct: bool) -> DependencyInstanceInput {
    DependencyInstanceInput {
        package_name: name.to_string(),
        version: version.to_string(),
        is_direct: direct,
        is_dev: true,
        scope: "dev".to_string(),
    }
}

fn edge(parent: &str, version: &str, child: &str, spec: &str) -> DependencyEdge {
    DependencyEdge {
        parent: parent.to_string(),
        parent_version: Some(version.to_string()),
        child: child.to_string(),
        child_version: Some(spec.to_string()),
        scope: EdgeScope::Runtime,
    }
}

fn plan(db: &Database) -> (Vec<EvidenceItem>, Vec<UpgradeStep>) {
    let plan = crate::evidence::build_upgrade_plan(db).expect("upgrade plan");
    assert_eq!(plan.drops, 0, "no plan item may fail validation");
    (plan.items, plan.steps)
}

fn row_and_step(db: &Database, package: &str) -> (EvidenceItem, UpgradeStep) {
    let (items, steps) = plan(db);
    let item = items
        .into_iter()
        .find(|i| i.affected_deps.iter().any(|d| d == package))
        .unwrap_or_else(|| panic!("no {package} row"));
    validate_item(&item).expect("schema-valid");
    let step = steps
        .into_iter()
        .find(|s| s.package == package)
        .unwrap_or_else(|| panic!("no {package} step"));
    (item, step)
}

/// navcal's glob: 10.5.0 through sucrase 3.35.0 (`^10.3.10`); sucrase 3.35.1
/// no longer depends on glob; the installed tailwindcss 3.4.18 requires
/// sucrase `^3.35.0`, and so does 3.4.19.
fn seed_glob_under_tailwind(db: &Database, project: &str) {
    db.store_transitive_dependency(project, "glob", Some("10.5.0"), "npm", true)
        .unwrap();
    db.store_dependency_instances(
        project,
        "npm",
        &[
            dev("tailwindcss", "3.4.18", true),
            dev("sucrase", "3.35.0", false),
            dev("glob", "10.5.0", false),
        ],
    )
    .unwrap();
    advisory(db, "GHSA-3g3m-2fjr-6pw4", "glob", "14.0.2");
    crate::osv::release_facts::store_for_tests(
        db,
        vec![
            (
                "npm",
                "tailwindcss",
                vec!["3.4.18", "3.4.19"],
                Some("3.4.19"),
                false,
                vec![(
                    "sucrase",
                    hist(&[("3.4.18", Some("^3.35.0")), ("3.4.19", Some("^3.35.0"))]),
                )],
            ),
            (
                "npm",
                "sucrase",
                vec!["3.35.0", "3.35.1"],
                Some("3.35.1"),
                false,
                vec![(
                    "glob",
                    hist(&[("3.35.0", Some("^10.3.10")), ("3.35.1", None)]),
                )],
            ),
            (
                "npm",
                "glob",
                vec!["10.5.0", "14.0.2"],
                Some("14.0.2"),
                false,
                vec![],
            ),
        ],
    );
}

/// The decision, on the stored graph: a refresh of sucrase, never "upgrade
/// tailwindcss to 3.4.19".
#[test]
fn a_parent_its_installed_declared_dependency_already_admits_is_a_lockfile_refresh() {
    let project = "d:/work/navcal";
    let db = test_db();
    seed_glob_under_tailwind(&db, project);
    db.store_dependency_edges(
        project,
        "npm",
        &[
            edge("tailwindcss", "3.4.18", "sucrase", "^3.35.0"),
            edge("sucrase", "3.35.0", "glob", "^10.3.10"),
        ],
    )
    .unwrap();
    let (item, step) = row_and_step(&db, "glob");
    let text = &item.explanation;
    assert!(
        text.contains(
            "sucrase arrives through tailwindcss 3.4.18, a direct dependency whose installed \
             release already admits sucrase 3.35.1, so refreshing sucrase in the lockfile is \
             enough and nothing this project declares has to be upgraded"
        ),
        "{text}"
    );
    assert!(text.contains("No manifest change needed"), "{text}");
    assert!(!text.contains("upgrade tailwindcss"), "{text}");
    assert!(!text.contains("3.4.19"), "{text}");
    assert_eq!(step.mechanism, Mechanism::LockfileRefresh, "{step:#?}");
}

/// The same project on disk: the command is the one that moves sucrase.
#[test]
fn the_in_range_parent_refresh_names_its_command() {
    let dir = tempfile::Builder::new()
        .prefix("glob-refresh-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .expect("tempdir");
    let project = dir.path().to_string_lossy().replace('\\', "/");
    if crate::project_inclusion::is_hard_excluded(&project) {
        eprintln!("SKIPPED the_in_range_parent_refresh_names_its_command: {project} is excluded from DB writes here");
        return;
    }
    let lock = r#"{"name": "app", "lockfileVersion": 3, "packages": {
        "": { "name": "app", "devDependencies": { "tailwindcss": "^3.4.18" } },
        "node_modules/tailwindcss": { "version": "3.4.18", "dev": true, "dependencies": { "sucrase": "^3.35.0" } },
        "node_modules/sucrase": { "version": "3.35.0", "dev": true, "dependencies": { "glob": "^10.3.10" } },
        "node_modules/glob": { "version": "10.5.0", "dev": true }
    }}"#;
    std::fs::write(dir.path().join("package-lock.json"), lock).unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "devDependencies": {"tailwindcss": "^3.4.18"}}"#,
    )
    .unwrap();
    let db = test_db();
    seed_glob_under_tailwind(&db, &project);
    let edges = crate::ace::scanner::ProjectScanner::parse_package_lock_edges(lock);
    db.store_dependency_edges(&project, "npm", &edges).unwrap();
    let (item, step) = row_and_step(&db, "glob");
    let text = &item.explanation;
    assert!(
        text.contains("nothing this project declares has to be upgraded: `npm update sucrase`"),
        "{text}"
    );
    assert!(!text.contains("npm update tailwindcss"), "{text}");
    assert_eq!(step.mechanism, Mechanism::LockfileRefresh, "{step:#?}");
}

/// victauri's @azure/msal-node: 5.2.2 through @azure/identity 4.13.1
/// (`^5.1.0`); @azure/identity 4.14.1 requires `^7.1.0`; the installed
/// @vscode/vsce 3.9.1 already requires @azure/identity `^4.1.0`.
#[test]
fn a_major_fix_through_an_in_range_parent_is_still_a_refresh() {
    let project = "d:/work/victauri/editors/vscode";
    let db = test_db();
    db.store_transitive_dependency(project, "@azure/msal-node", Some("5.2.2"), "npm", true)
        .unwrap();
    db.store_dependency_instances(
        project,
        "npm",
        &[
            dev("@vscode/vsce", "3.9.1", true),
            dev("@azure/identity", "4.13.1", false),
            dev("@azure/msal-node", "5.2.2", false),
        ],
    )
    .unwrap();
    db.store_dependency_edges(
        project,
        "npm",
        &[
            edge("@vscode/vsce", "3.9.1", "@azure/identity", "^4.1.0"),
            edge("@azure/identity", "4.13.1", "@azure/msal-node", "^5.1.0"),
        ],
    )
    .unwrap();
    advisory(&db, "GHSA-xcx6-jx8p-x3h5", "@azure/msal-node", "7.1.0");
    crate::osv::release_facts::store_for_tests(
        &db,
        vec![
            (
                "npm",
                "@vscode/vsce",
                vec!["3.9.1", "3.9.2"],
                Some("3.9.2"),
                false,
                vec![(
                    "@azure/identity",
                    hist(&[("3.9.1", Some("^4.1.0")), ("3.9.2", Some("^4.1.0"))]),
                )],
            ),
            (
                "npm",
                "@azure/identity",
                vec!["4.13.1", "4.14.1"],
                Some("4.14.1"),
                false,
                vec![(
                    "@azure/msal-node",
                    hist(&[("4.13.1", Some("^5.1.0")), ("4.14.1", Some("^7.1.0"))]),
                )],
            ),
            (
                "npm",
                "@azure/msal-node",
                vec!["5.2.2", "7.1.0", "7.2.3"],
                Some("7.2.3"),
                false,
                vec![],
            ),
        ],
    );
    let (item, step) = row_and_step(&db, "@azure/msal-node");
    let text = &item.explanation;
    assert!(
        text.contains(
            "@azure/identity arrives through @vscode/vsce 3.9.1, a direct dependency whose \
             installed release already admits @azure/identity 4.14.1"
        ),
        "{text}"
    );
    assert!(!text.contains("upgrade @vscode/vsce"), "{text}");
    assert!(!text.contains("3.9.2"), "{text}");
    assert_eq!(step.mechanism, Mechanism::LockfileRefresh, "{step:#?}");
}

/// When the installed declared dependency does NOT admit a fixed parent, the
/// move is still its upgrade (`parent_update`): the rule above changes only
/// the in-range case.
#[test]
fn a_declared_dependency_whose_range_excludes_the_fixed_parent_still_moves() {
    let project = "d:/work/navcal";
    let db = test_db();
    seed_glob_under_tailwind(&db, project);
    // tailwindcss 3.4.18 pins sucrase exactly; 3.4.19 takes the fixed one.
    crate::osv::release_facts::store_for_tests(
        &db,
        vec![
            (
                "npm",
                "tailwindcss",
                vec!["3.4.18", "3.4.19"],
                Some("3.4.19"),
                false,
                vec![(
                    "sucrase",
                    hist(&[("3.4.18", Some("3.35.0")), ("3.4.19", Some("^3.35.1"))]),
                )],
            ),
            (
                "npm",
                "sucrase",
                vec!["3.35.0", "3.35.1"],
                Some("3.35.1"),
                false,
                vec![(
                    "glob",
                    hist(&[("3.35.0", Some("^10.3.10")), ("3.35.1", None)]),
                )],
            ),
            (
                "npm",
                "glob",
                vec!["10.5.0", "14.0.2"],
                Some("14.0.2"),
                false,
                vec![],
            ),
        ],
    );
    db.store_dependency_edges(
        project,
        "npm",
        &[
            edge("tailwindcss", "3.4.18", "sucrase", ">=3.35.0 <3.35.1"),
            edge("sucrase", "3.35.0", "glob", "^10.3.10"),
        ],
    )
    .unwrap();
    let (item, step) = row_and_step(&db, "glob");
    assert!(
        item.explanation.contains("upgrade tailwindcss to 3.4.19"),
        "{}",
        item.explanation
    );
    assert_eq!(step.mechanism, Mechanism::ParentUpdate, "{step:#?}");
}

/// minimatch 10.2.6 under @vscode/vsce 4.0.0, whose every release up to
/// 4.1.1 requires `^10.2.6` (D:/4DA's editors/vscode/4da), and, with
/// `movable`, a second project whose copy a parent release does admit (the
/// repository root's @typescript-eslint/typescript-estree 8.70.1 -> 8.73.0).
fn seed_minimatch(db: &Database, movable: bool) {
    let blocked = "d:/work/4da/editors/vscode/4da";
    db.store_transitive_dependency(blocked, "minimatch", Some("10.2.6"), "npm", true)
        .unwrap();
    db.store_dependency_instances(
        blocked,
        "npm",
        &[
            dev("@vscode/vsce", "4.0.0", true),
            dev("minimatch", "10.2.6", false),
        ],
    )
    .unwrap();
    db.store_dependency_edges(
        blocked,
        "npm",
        &[edge("@vscode/vsce", "4.0.0", "minimatch", "^10.2.6")],
    )
    .unwrap();
    let mut facts = vec![
        (
            "npm",
            "@vscode/vsce",
            vec!["4.0.0", "4.1.1"],
            Some("4.1.1"),
            false,
            vec![(
                "minimatch",
                hist(&[("4.0.0", Some("^10.2.6")), ("4.1.1", Some("^10.2.6"))]),
            )],
        ),
        (
            "npm",
            "minimatch",
            vec!["10.2.6", "11.0.1"],
            Some("11.0.1"),
            false,
            vec![],
        ),
    ];
    if movable {
        let root = "d:/work/4da";
        db.store_transitive_dependency(root, "minimatch", Some("10.2.6"), "npm", true)
            .unwrap();
        db.store_dependency_instances(
            root,
            "npm",
            &[
                dev("tse", "8.70.1", true),
                dev("minimatch", "10.2.6", false),
            ],
        )
        .unwrap();
        db.store_dependency_edges(
            root,
            "npm",
            &[edge("tse", "8.70.1", "minimatch", "^10.2.2")],
        )
        .unwrap();
        facts.push((
            "npm",
            "tse",
            vec!["8.70.1", "8.73.0"],
            Some("8.73.0"),
            false,
            vec![(
                "minimatch",
                hist(&[("8.70.1", Some("^10.2.2")), ("8.73.0", Some("^11.0.1"))]),
            )],
        ));
    }
    advisory(db, "GHSA-g3hv-75wg-m249", "minimatch", "11.0.1");
    crate::osv::release_facts::store_for_tests(db, facts);
}

/// The copy no parent release admits is decided `blocked_upstream`, with its
/// hold. When the same step also holds a copy elsewhere that a parent upgrade
/// does fix, one per-package mechanism cannot be both: the step stays
/// `lockfile_or_parent_update` ("the copies' routes differ") and the blocked
/// copy keeps its hold, never a single wrong definite route.
#[test]
fn a_copy_no_parent_release_admits_is_blocked_and_a_mixed_step_keeps_its_hold() {
    let db = test_db();
    seed_minimatch(&db, false);
    let (_, step) = row_and_step(&db, "minimatch");
    assert_eq!(step.mechanism, Mechanism::BlockedUpstream, "{step:#?}");
    assert_eq!(step.blocked_upstream.len(), 1, "{step:#?}");
    let hold = &step.blocked_upstream[0];
    assert_eq!(
        (
            hold.held_by.as_str(),
            hold.held_by_version.as_str(),
            hold.held_by_newest.as_str()
        ),
        ("@vscode/vsce", "4.0.0", "4.1.1")
    );

    let db = test_db();
    seed_minimatch(&db, true);
    let (_, step) = row_and_step(&db, "minimatch");
    assert_eq!(
        step.mechanism,
        Mechanism::LockfileOrParentUpdate,
        "{step:#?}"
    );
    assert_eq!(step.blocked_upstream.len(), 1, "{step:#?}");
    assert_eq!(
        step.blocked_upstream[0].project,
        "d:/work/4da/editors/vscode/4da"
    );
}
