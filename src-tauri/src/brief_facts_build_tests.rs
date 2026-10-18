// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `brief_facts_build`'s direct-fix step (audit E8).

use super::*;

/// Audit E8 in the Brief: a declared copy's "bump" names the command that
/// keeps how it is declared (the tilde survives), read from the project's
/// own manifest; a path that is not a bump is left alone.
#[test]
fn a_bump_names_the_command_for_its_declaration() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"dependencies": {"minimist": "~1.2.5"}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("package-lock.json"),
        r#"{"lockfileVersion": 3}"#,
    )
    .unwrap();
    let project = dir.path().to_string_lossy().to_string();
    let path = with_direct_fix(
        FixPath::Bump {
            to: "1.2.6".into(),
            fix: None,
        },
        &project,
        "npm",
        "minimist",
        Some("1.2.5"),
    );
    assert_eq!(
        crate::brief_facts::fix_clause("minimist", &path),
        "bump minimist to >= 1.2.6: run `npm install \"minimist@~1.2.6\"`"
    );
    // Not declared here: plain "bump", nothing invented.
    let plain = with_direct_fix(
        FixPath::Bump {
            to: "2.0.0".into(),
            fix: None,
        },
        &project,
        "npm",
        "absent",
        Some("1.0.0"),
    );
    assert_eq!(
        crate::brief_facts::fix_clause("absent", &plain),
        "bump absent to >= 2.0.0"
    );
    // A copy the declaration does not admit is another copy: nothing named.
    let other = with_direct_fix(
        FixPath::Bump {
            to: "1.2.6".into(),
            fix: None,
        },
        &project,
        "npm",
        "minimist",
        Some("0.0.8"),
    );
    assert_eq!(
        crate::brief_facts::fix_clause("minimist", &other),
        "bump minimist to >= 1.2.6"
    );
    let refresh = FixPath::Refresh {
        to: "1.2.6".into(),
        inferred: false,
        command: None,
    };
    assert_eq!(
        with_direct_fix(refresh.clone(), &project, "npm", "minimist", Some("1.2.5")),
        refresh
    );
}

/// No installed version (no version-confirmed copy) or an ecosystem the rule
/// does not read: nothing is named, the plain bump stays.
#[test]
fn a_bump_without_a_confirmed_copy_or_known_ecosystem_names_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"dependencies": {"minimist": "~1.2.5"}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("package-lock.json"),
        r#"{"lockfileVersion": 3}"#,
    )
    .unwrap();
    let project = dir.path().to_string_lossy().to_string();
    let bump = || FixPath::Bump {
        to: "1.2.6".into(),
        fix: None,
    };
    assert_eq!(
        with_direct_fix(bump(), &project, "npm", "minimist", None),
        bump()
    );
    assert_eq!(
        with_direct_fix(bump(), &project, "Hex", "minimist", Some("1.2.5")),
        bump()
    );
}

/// One fixture project, seeded the way the scanner seeds it: the Brief's
/// direct row and the Upgrade Plan's direct row must name the SAME command
/// (review of #982, SF3) — the property audit E8's two halves share.
fn seeded_project(tag: &str) -> Option<(tempfile::TempDir, String, crate::db::Database)> {
    let dir = tempfile::Builder::new()
        .prefix(&format!("brief-{tag}-"))
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .expect("tempdir");
    let project = dir.path().to_string_lossy().replace('\\', "/");
    if crate::project_inclusion::is_hard_excluded(&project) {
        eprintln!("SKIPPED brief/plan parity: {project} is excluded from DB writes here");
        return None;
    }
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "app", "dependencies": {"minimist": "~1.2.5"}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("package-lock.json"),
        r#"{"lockfileVersion": 3, "packages": {"": {"dependencies": {"minimist": "~1.2.5"}}, "node_modules/minimist": {"version": "1.2.5"}}}"#,
    )
    .unwrap();
    let db = crate::test_utils::test_db();
    db.upsert_osv_advisory(
        "GHSA-xvch-5gv4-984h",
        "Prototype Pollution in minimist",
        None,
        "minimist",
        "npm",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.2.6"}]}]"#),
        Some(r#"["1.2.6"]"#),
        Some("CVSS_V3"),
        Some(9.8),
        Some("https://osv.dev/GHSA-xvch-5gv4-984h"),
        Some("2026-09-01T00:00:00Z"),
        None,
        None,
    )
    .unwrap();
    Some((dir, project, db))
}

fn seed_copy(db: &crate::db::Database, project: &str) {
    db.store_dependency(project, "minimist", Some("1.2.5"), "npm", false, None)
        .unwrap();
    db.store_dependency_instances(
        project,
        "npm",
        &[crate::db::DependencyInstanceInput {
            package_name: "minimist".to_string(),
            version: "1.2.5".to_string(),
            is_direct: true,
            is_dev: false,
            scope: "runtime".to_string(),
        }],
    )
    .unwrap();
}

fn minimist_alert(project: &str) -> PreemptionAlert {
    PreemptionAlert {
        id: "osv:GHSA-xvch-5gv4-984h".to_string(),
        alert_type: crate::preemption::PreemptionType::SecurityAdvisory,
        title: "minimist@1.2.5: 1 known vulnerability".to_string(),
        explanation: String::new(),
        evidence: vec![],
        affected_projects: vec![project.to_string()],
        affected_dependencies: vec!["minimist".to_string()],
        urgency: AlertUrgency::Critical,
        confidence: 0.95,
        predicted_window: None,
        suggested_actions: vec![],
        created_at: "2026-10-18 00:00:00".to_string(),
        osv_verified: true,
        source_classified: false,
        installed_version: Some("1.2.5".to_string()),
        fixed_version: Some("1.2.6".to_string()),
        is_direct: Some(true),
        is_dev: Some(false),
        platform_inactive: false,
        lockfile_only: false,
    }
}

fn brief_clause(db: &crate::db::Database, project: &str) -> String {
    let (matched, not_compiled) =
        crate::osv::matching::get_matched_advisories_with_not_compiled(db).expect("matcher");
    let novelty = Novelty::default();
    let liveness = crate::evidence::ProjectLiveness::default();
    let ctx = FactCtx {
        db,
        matched: &matched,
        not_compiled: &not_compiled,
        conn: None,
        novelty: &novelty,
        liveness: &liveness,
    };
    let fact = security_fact(&minimist_alert(project), &ctx, &mut LabelCache::default())
        .expect("a fact for the alert");
    crate::brief_facts::fix_clause(&fact.package, &fact.sites[0].fix_path)
}

#[test]
fn the_brief_and_the_plan_name_the_same_command() {
    let Some((_dir, project, db)) = seeded_project("parity") else {
        return;
    };
    seed_copy(&db, &project);
    let plan = crate::evidence::build_upgrade_plan(&db).items;
    let row = plan
        .iter()
        .find(|i| i.title.contains("minimist"))
        .expect("a minimist row");
    let command = "`npm install \"minimist@~1.2.6\"`";
    assert!(
        row.explanation.contains(command),
        "plan: {}",
        row.explanation
    );
    let clause = brief_clause(&db, &project);
    assert!(clause.contains(command), "brief: {clause}");
}

/// SF1: with no version-confirmed copy the version and target are the
/// alert's fallbacks (the raw highest OSV fix, which skips the withdrawn-
/// release rule); the plan names no command there, and neither does the Brief.
#[test]
fn without_a_confirmed_copy_the_brief_names_no_command() {
    let Some((_dir, project, db)) = seeded_project("noconfirm") else {
        return;
    };
    let clause = brief_clause(&db, &project);
    assert_eq!(clause, "bump minimist to >= 1.2.6", "brief: {clause}");
}

/// #988: an upgrade fact is graded over working projects only. A release
/// that only a scratch tree is behind on (here it pins the yanked 1.4.1
/// while the active project already runs 3.0.0) is no candidate, so it can
/// never take the package's single slot from a release the active project
/// needs; and a scratch tree's yanked pin no longer masks the active
/// project's breaking upgrade.
#[test]
fn a_scratch_tree_never_sets_the_class_of_an_upgrade_fact() {
    let grade = |announced: &str, active: &str| {
        release_grade::ReleaseGrade::from_pins(
            "quux",
            semver::Version::parse(announced).expect("version"),
            vec![
                ("/scratch/old".into(), "1.4.1".into(), true, false),
                ("/dev/app".into(), active.into(), true, false),
            ],
            &["1.4.1".to_string()],
        )
    };
    let liveness = crate::evidence::ProjectLiveness::default().with_scratch(&["/scratch/old"]);

    // Only the scratch tree is behind: graded over every project this was
    // a Yanked candidate (and took the slot); now it is none.
    assert!(grade("3.0.0", "3.0.0").class().is_some());
    assert!(upgrade_grade(grade("3.0.0", "3.0.0"), &liveness).is_none());

    // The active project a major behind: Breaking, for that project.
    let up = upgrade_grade(grade("2.0.0", "1.9.0"), &liveness).expect("a candidate");
    assert_eq!(up.class(), Some(ReleaseClass::Breaking));
    let concerned: Vec<&str> = up
        .concerned_pins()
        .iter()
        .map(|p| p.project_path.as_str())
        .collect();
    assert_eq!(concerned, vec!["/dev/app"]);
}
