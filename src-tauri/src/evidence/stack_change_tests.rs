// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use semver::Version;

use super::releases::{change_of, release_change, requirements_admit, RegistryRow};
use super::*;
use crate::brief_facts::{FactStatus, NotCompiledNote};
use crate::evidence::{validate_item, ProjectLiveness, ACTION_IDS};
use crate::osv::fix_path::Requirement;
use crate::osv::release_advisories::{AffectedRelease, ReleaseAdvisories, UnknownReason};
use crate::preemption::AlertUrgency;
use crate::scoring::release_grade::ReleaseGrade;

const NOW: i64 = 1_760_000_000_000;

fn v(s: &str) -> Version {
    Version::parse(s).expect("version")
}

fn grade(pkg: &str, announced: &str, pins: &[(&str, &str, bool, bool)]) -> ReleaseGrade {
    ReleaseGrade::from_pins(
        pkg,
        v(announced),
        pins.iter()
            .map(|(p, inst, direct, dev)| (p.to_string(), inst.to_string(), *direct, *dev))
            .collect(),
        &[],
    )
}

fn row(source_type: &str, title: &str) -> RegistryRow {
    RegistryRow {
        source_type: source_type.to_string(),
        title: title.to_string(),
        content: String::new(),
        url: Some("https://crates.io/crates/x".to_string()),
        published: Some("2026-10-01 08:00:00".to_string()),
    }
}

fn site(label: &str, installed: &str, fix_path: FixPath) -> SecuritySite {
    SecuritySite {
        label: label.to_string(),
        installed: Some(installed.to_string()),
        dev_only: false,
        scratch: false,
        dormant_days: None,
        fix_path,
    }
}

fn fact(package: &str, urgency: AlertUrgency, sites: Vec<SecuritySite>) -> SecurityFact {
    SecurityFact {
        key: format!("crates.io:{package}:4da/src-tauri"),
        package: package.to_string(),
        ecosystem: "crates.io".to_string(),
        urgency,
        worst_tier: Some("high".to_string()),
        worst_tier_from: None,
        advisory_count: 1,
        advisory_ids: vec!["GHSA-aaaa-bbbb-cccc".to_string()],
        title: "Session table grows without bound".to_string(),
        sites,
        not_compiled: Vec::new(),
        first_seen: Some("2026-10-03".to_string()),
        status: FactStatus::New,
    }
}

// ---------------------------------------------------------------- grading --

#[test]
fn a_grade_maps_to_the_change_it_asks_for() {
    let p = "/dev/app";
    assert_eq!(
        change_of(&grade("tokio", "2.0.0", &[(p, "1.40.0", true, false)])),
        Some(StackChange::Major)
    );
    assert_eq!(
        change_of(&grade("lopdf", "0.45.0", &[(p, "0.42.0", true, false)])),
        Some(StackChange::Breaking),
        "below 1.0 a minor is breaking"
    );
    assert_eq!(
        change_of(&grade("serde", "1.2.0", &[(p, "1.0.100", true, false)])),
        Some(StackChange::Minor)
    );
    assert_eq!(
        change_of(&grade("serde", "1.0.101", &[(p, "1.0.100", true, false)])),
        None,
        "a patch is not news on its own"
    );
    assert_eq!(
        change_of(&grade("serde", "1.0.100", &[(p, "1.0.100", true, false)])),
        None,
        "already installed"
    );
    assert_eq!(
        change_of(&grade("tokio", "2.0.0", &[(p, "1.40.0", false, false)])),
        None,
        "a transitive copy is moved by its parent, not by a release row"
    );
}

#[test]
fn a_yanked_pin_is_its_own_change() {
    let g = ReleaseGrade::from_pins(
        "quux",
        v("1.4.2"),
        vec![("/dev/app".to_string(), "1.4.1".to_string(), true, false)],
        &["1.4.1".to_string()],
    );
    assert_eq!(change_of(&g), Some(StackChange::Yanked));
}

#[test]
fn requirements_admit_only_when_every_one_read_does() {
    assert_eq!(requirements_admit(&[], "1.2.0"), None);
    assert_eq!(
        requirements_admit(&[Requirement::cargo("1.0")], "1.2.0"),
        Some(true)
    );
    assert_eq!(
        requirements_admit(&[Requirement::cargo("=1.0.100")], "1.2.0"),
        Some(false)
    );
    assert_eq!(
        requirements_admit(
            &[Requirement::cargo("1.0"), Requirement::cargo("~1.0.5")],
            "1.2.0"
        ),
        Some(false)
    );
    assert_eq!(
        requirements_admit(&[Requirement::npm("^1.0.0")], "1.2.0"),
        Some(true)
    );
}

#[test]
fn a_minor_inside_the_declared_range_names_the_refresh_command() {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"app\"\n\n[dependencies]\nserde = \"1.0\"\n",
    )
    .expect("manifest");
    std::fs::write(dir.path().join("Cargo.lock"), "version = 4\n").expect("lock");
    let path = dir.path().to_string_lossy().into_owned();
    let g = grade("serde", "1.2.0", &[(&path, "1.0.100", true, false)]);
    let change = release_change(
        &g,
        &row("crates_io", "crates.io: serde v1.2.0"),
        &ProjectLiveness::default(),
        ReleaseAdvisories::Clear,
    )
    .expect("a minor for a direct runtime dependency");
    assert_eq!(change.change, StackChange::Minor);
    assert_eq!(change.ecosystem, "crates.io");
    assert_eq!(change.sites.len(), 1);
    assert_eq!(change.sites[0].admitted, Some(true));
    assert_eq!(
        change.sites[0].command.as_deref(),
        Some("cargo update -p serde@1.0.100 --precise 1.2.0")
    );
    let item = release_item(&change, NOW).item;
    assert!(item
        .suggested_actions
        .iter()
        .any(|a| a.action_id == "run_command"
            && a.label == "cargo update -p serde@1.0.100 --precise 1.2.0"));
    assert!(validate_item(&item).is_ok());
}

#[test]
fn a_minor_outside_the_declared_range_names_no_command() {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[dependencies]\nserde = \"=1.0.100\"\n",
    )
    .expect("manifest");
    std::fs::write(dir.path().join("Cargo.lock"), "version = 4\n").expect("lock");
    let path = dir.path().to_string_lossy().into_owned();
    let g = grade("serde", "1.2.0", &[(&path, "1.0.100", true, false)]);
    let change = release_change(
        &g,
        &row("crates_io", "crates.io: serde v1.2.0"),
        &ProjectLiveness::default(),
        ReleaseAdvisories::Clear,
    )
    .expect("still a change");
    assert_eq!(change.sites[0].admitted, Some(false));
    assert_eq!(change.sites[0].command, None);
    let item = release_item(&change, NOW).item;
    assert!(!item
        .suggested_actions
        .iter()
        .any(|a| a.action_id == "run_command"));
    assert!(
        item.explanation.contains("excludes 1.2.0"),
        "{}",
        item.explanation
    );
}

#[test]
fn a_breaking_release_never_names_a_refresh_command() {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[dependencies]\ntokio = \"1\"\n",
    )
    .expect("manifest");
    std::fs::write(dir.path().join("Cargo.lock"), "version = 4\n").expect("lock");
    let path = dir.path().to_string_lossy().into_owned();
    let g = grade("tokio", "2.0.0", &[(&path, "1.40.0", true, false)]);
    let change = release_change(
        &g,
        &row("crates_io", "crates.io: tokio v2.0.0"),
        &ProjectLiveness::default(),
        ReleaseAdvisories::Clear,
    )
    .expect("major");
    assert_eq!(change.change, StackChange::Major);
    assert!(change.sites.iter().all(|s| s.command.is_none()));
    let ranked = release_item(&change, NOW);
    assert_eq!(ranked.item.urgency, Urgency::Medium);
    assert_eq!(ranked.item.id, "stack-change:major:crates.io:tokio@2.0.0");
    assert!(ranked
        .item
        .title
        .starts_with("tokio 1.40.0 \u{2192} 2.0.0: major release"));
}

#[test]
fn a_dev_tooling_minor_is_left_out_but_a_dev_breaking_release_stays_low() {
    let p = "/dev/app";
    let liveness = ProjectLiveness::default();
    let minor = grade("vitest", "3.2.0", &[(p, "3.1.0", true, true)]);
    assert!(release_change(
        &minor,
        &row("npm_registry", "npm: vitest v3.2.0"),
        &liveness,
        ReleaseAdvisories::Clear
    )
    .is_none());
    let major = grade("vitest", "4.0.0", &[(p, "3.1.0", true, true)]);
    let change = release_change(
        &major,
        &row("npm_registry", "npm: vitest v4.0.0"),
        &liveness,
        ReleaseAdvisories::Clear,
    )
    .expect("a breaking dev release still changes something");
    assert!(change.dev_only);
    assert_eq!(release_item(&change, NOW).item.urgency, Urgency::Watch);
}

// ------------------------------------------- release advisories (defect A) --

/// An npm project whose `^4.1.0` admits 4.2.0, with a package-lock.json.
fn npm_project() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"app","dependencies":{"left-padder":"^4.1.0"}}"#,
    )
    .expect("manifest");
    std::fs::write(dir.path().join("package-lock.json"), "{}").expect("lock");
    let path = dir.path().to_string_lossy().into_owned();
    (dir, path)
}

fn npm_minor(path: &str, advisories: ReleaseAdvisories) -> Option<ReleaseChange> {
    let g = grade("left-padder", "4.2.0", &[(path, "4.1.3", true, false)]);
    release_change(
        &g,
        &row("npm_registry", "npm: left-padder v4.2.0"),
        &ProjectLiveness::default(),
        advisories,
    )
}

fn malicious(covered_by_installed: bool) -> ReleaseAdvisories {
    ReleaseAdvisories::Affected(AffectedRelease {
        ids: vec!["MAL-2026-1234".to_string()],
        malware: true,
        confirmed: true,
        covered_by_installed,
    })
}

fn has_run(item: &EvidenceItem) -> bool {
    item.suggested_actions
        .iter()
        .any(|a| a.action_id == "run_command")
}

#[test]
fn a_clean_minor_inside_the_range_keeps_its_refresh_command() {
    let (_dir, path) = npm_project();
    let change = npm_minor(&path, ReleaseAdvisories::Clear).expect("a minor");
    assert_eq!(
        change.sites[0].command.as_deref(),
        Some("npm update left-padder")
    );
    let item = release_item(&change, NOW).item;
    assert!(has_run(&item));
    assert_eq!(item.urgency, Urgency::Watch);
    assert!(item.title.contains("new minor"), "{}", item.title);
    assert!(!item.explanation.contains("Advisory status"));
}

#[test]
fn a_malicious_minor_names_no_command_and_says_do_not_install() {
    let (_dir, path) = npm_project();
    let change = npm_minor(&path, malicious(false)).expect("still announced, as a warning");
    assert_eq!(change.sites[0].admitted, Some(true));
    assert_eq!(
        change.sites[0].command, None,
        "never `npm update` toward malware"
    );
    let item = release_item(&change, NOW).item;
    assert!(!has_run(&item), "{:?}", item.suggested_actions);
    assert_eq!(item.urgency, Urgency::High);
    assert!(
        item.title.contains("malicious release, do not install"),
        "{}",
        item.title
    );
    assert!(
        item.explanation.contains("MAL-2026-1234")
            && item.explanation.contains("do not install it")
            && item.explanation.contains("lockfile refresh"),
        "{}",
        item.explanation
    );
    assert!(
        !item.explanation.contains("picks it up"),
        "the upgrade advice is replaced: {}",
        item.explanation
    );
    assert_eq!(item.suggested_actions[0].action_id, "review_security");
    assert!(item
        .evidence
        .iter()
        .any(|c| c.source == "osv" && c.title == "MAL-2026-1234"));
    assert!(validate_item(&item).is_ok());
}

#[test]
fn a_vulnerable_minor_names_its_advisories_and_no_command() {
    let (_dir, path) = npm_project();
    let verdict = ReleaseAdvisories::Affected(AffectedRelease {
        ids: vec!["GHSA-aaaa-bbbb-cccc".to_string()],
        malware: false,
        confirmed: true,
        covered_by_installed: false,
    });
    let item = release_item(&npm_minor(&path, verdict).expect("announced"), NOW).item;
    assert!(!has_run(&item));
    assert!(item.title.contains("affected by GHSA-aaaa-bbbb-cccc"));
    assert!(item.explanation.contains("names no command"));
    assert_eq!(item.urgency, Urgency::Watch);
    assert!(validate_item(&item).is_ok());
}

#[test]
fn an_unchecked_release_names_no_command_and_says_so() {
    let (_dir, path) = npm_project();
    let change = npm_minor(
        &path,
        ReleaseAdvisories::Unknown(UnknownReason::Stale {
            synced_at: Some("2026-10-01 00:00:00".to_string()),
        }),
    )
    .expect("announced");
    assert_eq!(change.sites[0].command, None);
    let item = release_item(&change, NOW).item;
    assert!(!has_run(&item));
    assert!(
        item.explanation.contains("Advisory status unknown")
            && item.explanation.contains("more than three days ago"),
        "{}",
        item.explanation
    );
    assert!(validate_item(&item).is_ok());
}

#[test]
fn a_withdrawn_release_names_no_command() {
    let (_dir, path) = npm_project();
    let item = release_item(
        &npm_minor(&path, ReleaseAdvisories::Withdrawn).expect("announced"),
        NOW,
    )
    .item;
    assert!(!has_run(&item));
    assert!(item.explanation.contains("registry withdrew"));
}

#[test]
fn a_clean_release_inside_the_hold_names_no_command_and_says_why() {
    let (_dir, path) = npm_project();
    let change = npm_minor(
        &path,
        ReleaseAdvisories::Held {
            published: "2026-10-17".to_string(),
            days: 1,
        },
    )
    .expect("announced");
    assert_eq!(change.sites[0].command, None);
    let item = release_item(&change, NOW).item;
    assert!(!has_run(&item));
    assert!(
        item.explanation
            .contains("published 1 day ago (2026-10-17)")
            && item.explanation.contains("younger than 3 days")
            && item.explanation.contains("caught and pulled first"),
        "{}",
        item.explanation
    );
    assert!(validate_item(&item).is_ok());
}

/// Review B2: coverage by the advisory lane is judged on ACTIVE copies. A
/// scratch project's 2.0.5 (hit by the MAL- record) used to "cover" the
/// release for an active project on 2.0.0 whose `^2.0.0` admits the
/// malicious 2.1.0 — and the item was dropped.
#[test]
fn a_scratch_copy_never_covers_malware_for_an_active_project() {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"app","dependencies":{"x":"^2.0.0"}}"#,
    )
    .expect("manifest");
    std::fs::write(dir.path().join("package-lock.json"), "{}").expect("lock");
    let active = dir.path().to_string_lossy().into_owned();
    let scratch = "/scratch/old";
    let g = grade(
        "x",
        "2.1.0",
        &[
            (scratch, "2.0.5", true, false),
            (&active, "2.0.0", true, false),
        ],
    );
    let liveness = ProjectLiveness::default().with_scratch(&[scratch]);
    let active_grade = super::releases::active_grade(&g, &liveness);
    let installed: Vec<String> = super::releases::active_pins(&active_grade)
        .iter()
        .map(|p| p.installed.to_string())
        .collect();
    assert_eq!(installed, vec!["2.0.0".to_string()]);
    let mal = crate::osv::release_advisories::AdvisoryWindow {
        advisory_id: "MAL-2026-77".to_string(),
        summary: "Malicious code in x (npm)".to_string(),
        aliases: Vec::new(),
        cwe_ids: Vec::new(),
        affected_ranges: Some(r#"[{"type":"ENUMERATED","events":["2.0.5","2.1.0"]}]"#.into()),
    };
    let verdict = crate::osv::release_advisories::assess(
        "npm",
        "2.1.0",
        &installed,
        &[mal],
        false,
        &[],
        chrono::Utc::now().naive_utc(),
        false,
        None,
    );
    let change = release_change(
        &g,
        &row("npm_registry", "npm: x v2.1.0"),
        &liveness,
        verdict,
    )
    .expect("the active project is warned");
    let item = release_item(&change, NOW).item;
    assert!(!has_run(&item));
    assert!(item.title.contains("malicious release"), "{}", item.title);
    assert!(
        item.explanation.contains("lockfile refresh"),
        "{}",
        item.explanation
    );
}

#[test]
fn malware_is_never_left_to_the_advisory_lane() {
    let (_dir, path) = npm_project();
    assert!(
        npm_minor(&path, malicious(true)).is_some(),
        "even when every active copy is hit too"
    );
    let vuln = ReleaseAdvisories::Affected(AffectedRelease {
        ids: vec!["GHSA-aaaa-bbbb-cccc".to_string()],
        malware: false,
        confirmed: true,
        covered_by_installed: true,
    });
    assert!(npm_minor(&path, vuln).is_none());
}

/// Review S3: install scripts run for dev dependencies too, so a malicious
/// minor of a dev-only dependency is said, with its range read.
#[test]
fn a_malicious_dev_only_minor_is_still_announced() {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"app","devDependencies":{"left-padder":"^4.1.0"}}"#,
    )
    .expect("manifest");
    std::fs::write(dir.path().join("package-lock.json"), "{}").expect("lock");
    let path = dir.path().to_string_lossy().into_owned();
    let g = grade("left-padder", "4.2.0", &[(&path, "4.1.3", true, true)]);
    let r = row("npm_registry", "npm: left-padder v4.2.0");
    let liveness = ProjectLiveness::default();
    assert!(release_change(&g, &r, &liveness, ReleaseAdvisories::Clear).is_none());
    let change = release_change(&g, &r, &liveness, malicious(false)).expect("said");
    assert_eq!(change.sites[0].admitted, Some(true));
    assert!(!has_run(&release_item(&change, NOW).item));
}

/// Review S4: a dependency only a workspace member declares is admitted by
/// the member's range: the refresh warning appears; no command, since the
/// root's own manifest does not declare it.
#[test]
fn a_member_declared_range_is_read_for_the_refresh_warning() {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
    )
    .expect("root manifest");
    std::fs::create_dir_all(dir.path().join("packages/ui")).expect("member dir");
    std::fs::write(
        dir.path().join("packages/ui/package.json"),
        r#"{"name":"@me/ui","dependencies":{"left-padder":"^4.1.0"}}"#,
    )
    .expect("member manifest");
    std::fs::write(dir.path().join("package-lock.json"), "{}").expect("lock");
    let path = dir.path().to_string_lossy().into_owned();
    let change = npm_minor(&path, malicious(false)).expect("said");
    assert_eq!(change.sites[0].admitted, Some(true));
    let item = release_item(&change, NOW).item;
    assert!(
        item.explanation.contains("lockfile refresh"),
        "{}",
        item.explanation
    );
    let clean = npm_minor(&path, ReleaseAdvisories::Clear).expect("a minor");
    assert_eq!(clean.sites[0].command, None, "the root does not declare it");
}

// ------------------------------------------ active pins graded first (#983 follow-up)

/// A scratch tree a MAJOR behind used to set the class ("major") and be the
/// only concerned project; the scratch filter then dropped it and the
/// release said nothing to the active project a minor behind. Graded over
/// the active projects first, it is that project's minor.
#[test]
fn a_scratch_project_a_major_behind_no_longer_hides_an_active_minor() {
    let (_dir, active) = npm_project();
    let scratch = "/scratch/old";
    let g = grade(
        "left-padder",
        "4.2.0",
        &[
            (scratch, "3.0.0", true, false),
            (&active, "4.1.3", true, false),
        ],
    );
    // Before: the class over every project is the scratch tree's major.
    assert_eq!(change_of(&g), Some(StackChange::Major));
    let liveness = ProjectLiveness::default().with_scratch(&[scratch]);
    let change = release_change(
        &g,
        &row("npm_registry", "npm: left-padder v4.2.0"),
        &liveness,
        ReleaseAdvisories::Clear,
    )
    .expect("after: the active project's minor");
    assert_eq!(change.change, StackChange::Minor);
    assert_eq!(change.sites.len(), 1);
    assert_eq!(change.sites[0].installed, "4.1.3");
    assert_eq!(
        change.sites[0].command.as_deref(),
        Some("npm update left-padder")
    );
}

/// Same shape, malicious release: the active project is warned.
#[test]
fn a_scratch_project_a_major_behind_never_hides_malware_from_an_active_one() {
    let (_dir, active) = npm_project();
    let scratch = "/scratch/old";
    let g = grade(
        "left-padder",
        "4.2.0",
        &[
            (scratch, "3.0.0", true, false),
            (&active, "4.1.3", true, false),
        ],
    );
    let liveness = ProjectLiveness::default().with_scratch(&[scratch]);
    let change = release_change(
        &g,
        &row("npm_registry", "npm: left-padder v4.2.0"),
        &liveness,
        malicious(false),
    )
    .expect("warned");
    assert_eq!(change.change, StackChange::Security);
    let item = release_item(&change, NOW).item;
    assert!(!has_run(&item));
    assert!(item.title.contains("malicious release"), "{}", item.title);
    assert_eq!(item.urgency, Urgency::High);
    assert!(item
        .id
        .starts_with("stack-change:security:npm:left-padder@4.2.0"));
}

/// "Patches are not news" must not hide a malicious patch: a caret range
/// admits it, so it is the first thing a refresh installs.
#[test]
fn a_malicious_patch_for_an_active_project_surfaces_as_security() {
    let (_dir, path) = npm_project();
    let g = grade("left-padder", "4.1.4", &[(&path, "4.1.3", true, false)]);
    let r = row("npm_registry", "npm: left-padder v4.1.4");
    let liveness = ProjectLiveness::default();
    assert!(
        release_change(&g, &r, &liveness, ReleaseAdvisories::Clear).is_none(),
        "a clean patch is still not news"
    );
    let change = release_change(&g, &r, &liveness, malicious(false)).expect("surfaced");
    assert_eq!(change.change, StackChange::Security);
    assert_eq!(change.sites[0].admitted, Some(true));
    assert_eq!(change.sites[0].command, None);
    let item = release_item(&change, NOW).item;
    assert!(!has_run(&item));
    assert!(
        item.explanation.contains("lockfile refresh"),
        "{}",
        item.explanation
    );
    assert!(validate_item(&item).is_ok());
}

/// An npm project declaring `left-padder` with `spec`.
fn npm_project_with(spec: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(
        dir.path().join("package.json"),
        format!(r#"{{"name":"app","dependencies":{{"left-padder":"{spec}"}}}}"#),
    )
    .expect("manifest");
    std::fs::write(dir.path().join("package-lock.json"), "{}").expect("lock");
    let path = dir.path().to_string_lossy().into_owned();
    (dir, path)
}

/// Review of #988, B1: release 4.2.1, A on 4.1.0, B on 4.2.0. The
/// largest-gap set is A alone; an affected release names EVERY active
/// project behind it, and its coverage is judged on all of them.
#[test]
fn an_affected_release_names_every_active_project_behind_it() {
    let (_a_dir, a) = npm_project_with("^4.1.0");
    let (_b_dir, b) = npm_project_with("^4.2.0");
    let g = grade(
        "left-padder",
        "4.2.1",
        &[(&a, "4.1.0", true, false), (&b, "4.2.0", true, false)],
    );
    let liveness = ProjectLiveness::default();
    let active = super::releases::active_grade(&g, &liveness);
    let largest: Vec<String> = super::releases::active_pins(&active)
        .iter()
        .map(|p| p.installed.to_string())
        .collect();
    assert_eq!(largest, vec!["4.1.0"], "the largest-gap set is A alone");
    let r = row("npm_registry", "npm: left-padder v4.2.1");
    let check = |advisory: &str| {
        let installed: Vec<String> = super::releases::behind_pins(&active)
            .iter()
            .map(|p| p.installed.to_string())
            .collect();
        let window = crate::osv::release_advisories::AdvisoryWindow {
            advisory_id: advisory.to_string(),
            summary: "ReDoS".to_string(),
            aliases: Vec::new(),
            cwe_ids: Vec::new(),
            affected_ranges: Some(
                r#"[{"type":"ENUMERATED","events":["4.1.0","4.2.1"]}]"#.to_string(),
            ),
        };
        crate::osv::release_advisories::assess(
            "npm",
            "4.2.1",
            &installed,
            &[window],
            false,
            &[],
            chrono::Utc::now().naive_utc(),
            false,
            None,
        )
    };
    // An advisory hitting 4.2.1 and A's 4.1.0 but not B's 4.2.0: not
    // covered, so the item stays, for both.
    let verdict = check("GHSA-aaaa-bbbb-cccc");
    assert!(
        matches!(&verdict, ReleaseAdvisories::Affected(h) if !h.covered_by_installed),
        "{verdict:?}"
    );
    let change = release_change(&g, &r, &liveness, verdict).expect("kept for B");
    let installed: Vec<&str> = change.sites.iter().map(|s| s.installed.as_str()).collect();
    assert!(installed.contains(&"4.2.0"), "{installed:?}");

    // Malware: both named.
    let change = release_change(&g, &r, &liveness, malicious(false)).expect("warned");
    assert_eq!(change.sites.len(), 2);
}

/// The sets differ when the gaps do: B a patch behind, A a minor behind.
/// The unaffected minor concerns A only; the malicious release names both.
#[test]
fn a_patch_behind_project_is_named_for_malware_but_not_for_the_minor() {
    let (_a_dir, a) = npm_project_with("^4.1.0");
    let (_b_dir, b) = npm_project_with("^4.2.0");
    let g = grade(
        "left-padder",
        "4.2.1",
        &[(&a, "4.1.0", true, false), (&b, "4.2.0", true, false)],
    );
    let liveness = ProjectLiveness::default();
    let r = row("npm_registry", "npm: left-padder v4.2.1");
    let clean = release_change(&g, &r, &liveness, ReleaseAdvisories::Clear).expect("minor");
    assert_eq!(clean.sites.len(), 1, "the minor concerns A");
    let bad = release_change(&g, &r, &liveness, malicious(false)).expect("warned");
    let mut installed: Vec<&str> = bad.sites.iter().map(|s| s.installed.as_str()).collect();
    installed.sort_unstable();
    assert_eq!(installed, vec!["4.1.0", "4.2.0"]);
    assert!(bad.sites.iter().all(|s| s.admitted == Some(true)));
}

/// Review S2/S3: a non-malicious affected release ranks and keeps the
/// urgency its grade gives it (a runtime major stays Medium, behind a
/// yanked pin), and its version line names the release as affected, with
/// no "installed -> announced" arrow that would read as the fix.
#[test]
fn an_affected_major_keeps_its_rank_and_names_the_release_as_affected() {
    let (_dir, path) = npm_project();
    let g = grade("left-padder", "5.0.0", &[(&path, "4.1.3", true, false)]);
    let verdict = ReleaseAdvisories::Affected(AffectedRelease {
        ids: vec!["GHSA-aaaa-bbbb-cccc".to_string()],
        malware: false,
        confirmed: true,
        covered_by_installed: false,
    });
    let change = release_change(
        &g,
        &row("npm_registry", "npm: left-padder v5.0.0"),
        &ProjectLiveness::default(),
        verdict,
    )
    .expect("kept");
    assert_eq!(change.change, StackChange::Security);
    let ranked = release_item(&change, NOW);
    assert_eq!(ranked.change, StackChange::Major, "ranked as its grade");
    assert_eq!(ranked.item.urgency, Urgency::Medium);
    assert!(ranked.item.id.starts_with("stack-change:security:"));
    assert_eq!(
        ranked.item.evidence[0].title,
        "npm \u{b7} left-padder 5.0.0 (affected)"
    );

    let yanked = ReleaseGrade::from_pins(
        "quux",
        v("1.4.2"),
        vec![("/dev/app".to_string(), "1.4.1".to_string(), true, false)],
        &["1.4.1".to_string()],
    );
    let yanked = release_item(
        &release_change(
            &yanked,
            &row("crates_io", "crates.io: quux v1.4.2"),
            &ProjectLiveness::default(),
            ReleaseAdvisories::Clear,
        )
        .expect("yanked"),
        NOW,
    );
    let mut ranked_list = vec![ranked, yanked];
    order(&mut ranked_list);
    assert_eq!(ranked_list[0].change, StackChange::Yanked);
}

#[test]
fn an_npm_security_placeholder_is_still_never_announced() {
    // npm replaces a removed malicious package with `0.0.N-security`: a
    // prerelease, never a release row (`release_grade`'s rule), unchanged.
    let p = "/dev/app";
    assert_eq!(
        change_of(&grade(
            "evil",
            "0.0.1-security",
            &[(p, "0.0.0", true, false)]
        )),
        None
    );
}

// --------------------------------------------------------------- security --

#[test]
fn a_security_fact_becomes_an_item_with_the_fix_and_the_command() {
    let f = fact(
        "rmcp",
        AlertUrgency::High,
        vec![site(
            "4da/src-tauri",
            "1.7.0",
            FixPath::Refresh {
                to: "1.7.1".to_string(),
                inferred: false,
                command: Some("cargo update -p rmcp@1.7.0 --precise 1.7.1".to_string()),
            },
        )],
    );
    let r = security_item(&f, NOW);
    let item = &r.item;
    assert_eq!(
        item.id,
        "stack-change:security:crates.io:rmcp:4da/src-tauri"
    );
    assert_eq!(item.title, "rmcp 1.7.0 \u{2192} 1.7.1: security fix (high)");
    assert_eq!(item.urgency, Urgency::High);
    assert_eq!(item.affected_deps, vec!["rmcp".to_string()]);
    assert_eq!(item.affected_projects, vec!["4da/src-tauri".to_string()]);
    assert_eq!(item.evidence[0].source, "version_context");
    assert_eq!(item.evidence[0].relevance_note, "security");
    assert_eq!(
        item.evidence[1].url.as_deref(),
        Some("https://osv.dev/vulnerability/GHSA-aaaa-bbbb-cccc")
    );
    let cmd = item
        .suggested_actions
        .iter()
        .find(|a| a.action_id == "run_command")
        .expect("command");
    assert_eq!(cmd.label, "cargo update -p rmcp@1.7.0 --precise 1.7.1");
    assert_eq!(cmd.description, "Run in 4da/src-tauri");
    assert!(
        item.explanation
            .contains("refreshing the lockfile reaches rmcp >= 1.7.1"),
        "{}",
        item.explanation
    );
    assert!(validate_item(item).is_ok());
}

/// Live 2026-10-12, Signal lane 1: "undici 7.28.0: high advisory, no fix
/// published" for navcal, whose undici copies (5.28.4 and 7.28.0) are each
/// fixed on their own line. Every copy is named with its own target, and the
/// title states both targets.
#[test]
fn copies_fixed_on_separate_lines_are_never_called_unfixed() {
    let refresh = |to: &str| FixPath::Refresh {
        to: to.to_string(),
        inferred: true,
        command: None,
    };
    let f = fact(
        "undici",
        AlertUrgency::Medium,
        vec![
            site("navcal", "5.28.4", refresh("6.28.1")),
            site("navcal", "7.28.0", refresh("7.29.1")),
        ],
    );
    let item = security_item(&f, NOW).item;
    assert_eq!(
        item.title,
        "undici 5.28.4 (+1 other version) \u{2192} 6.28.1 / 7.29.1: security fix (high)"
    );
    assert!(!item.title.contains("no fix"), "{}", item.title);
    assert!(
        item.explanation
            .contains("navcal undici 5.28.4: transitive;")
            && item
                .explanation
                .contains("navcal undici 7.28.0: transitive;"),
        "{}",
        item.explanation
    );
    assert_eq!(item.affected_projects, vec!["navcal".to_string()]);
    assert!(validate_item(&item).is_ok());

    // A copy the brief could not place on a line still states each fix.
    let per_line = fact(
        "brace-expansion",
        AlertUrgency::Medium,
        vec![SecuritySite {
            installed: None,
            ..site(
                "navcal",
                "?",
                FixPath::PerLine {
                    lines: vec![
                        crate::brief_facts::LineFix {
                            installed: "1.1.12".into(),
                            to: "1.1.21".into(),
                        },
                        crate::brief_facts::LineFix {
                            installed: "2.0.2".into(),
                            to: "2.1.7".into(),
                        },
                    ],
                },
            )
        }],
    );
    let item = security_item(&per_line, NOW).item;
    assert_eq!(
        item.title,
        "brace-expansion \u{2192} 1.1.21 / 2.1.7: security fix (high)"
    );
    assert!(
        item.explanation.contains(
            "fixed on each line: brace-expansion 1.1.12 needs >= 1.1.21; brace-expansion 2.0.2 \
             needs >= 2.1.7"
        ),
        "{}",
        item.explanation
    );
}

#[test]
fn a_fact_with_no_published_fix_says_so() {
    let mut f = fact(
        "left-pad",
        AlertUrgency::Medium,
        vec![site("app/web", "1.0.0", FixPath::NoFix)],
    );
    f.worst_tier = None;
    f.not_compiled = vec![NotCompiledNote {
        advisory_id: "GHSA-zzzz".to_string(),
        summary: "client only".to_string(),
    }];
    let item = security_item(&f, NOW).item;
    assert_eq!(
        item.title,
        "left-pad 1.0.0: medium advisory, no fix published"
    );
    assert!(!item
        .suggested_actions
        .iter()
        .any(|a| a.action_id == "run_command"));
    assert!(item.explanation.contains("Not counted"));
    assert!(validate_item(&item).is_ok());
}

#[test]
fn a_fact_with_no_advisory_id_never_names_an_empty_one() {
    let mut f = fact(
        "@modelcontextprotocol/server",
        AlertUrgency::High,
        vec![site(
            "4da/mcp-memory-server",
            "2.0.0",
            FixPath::Reinstall {
                to: "2.3.1".to_string(),
            },
        )],
    );
    f.advisory_ids = Vec::new();
    let item = security_item(&f, NOW).item;
    assert!(
        !item.explanation.contains("affected range"),
        "{}",
        item.explanation
    );
    assert!(!item.explanation.contains(" ."), "{}", item.explanation);
    assert!(validate_item(&item).is_ok());
}

#[test]
fn a_long_title_is_cut_on_a_char_boundary_under_the_limit() {
    let long = format!(
        "{} \u{2192} 2.0.0: security fix (critical)",
        "a\u{e9}".repeat(80)
    );
    let cut = fit_title(long);
    assert!(cut.len() <= TITLE_MAX);
    assert!(cut.ends_with('…'));
}

// ------------------------------------------------------------------ order --

#[test]
fn the_lane_orders_security_then_yanked_then_breaking_then_minor() {
    let liveness = ProjectLiveness::default();
    let p = "/dev/app";
    let minor = release_change(
        &grade("serde", "1.2.0", &[(p, "1.0.0", true, false)]),
        &row("crates_io", "crates.io: serde v1.2.0"),
        &liveness,
        ReleaseAdvisories::Clear,
    )
    .expect("minor");
    let mut newer_major = release_change(
        &grade("tokio", "2.0.0", &[(p, "1.0.0", true, false)]),
        &row("crates_io", "crates.io: tokio v2.0.0"),
        &liveness,
        ReleaseAdvisories::Clear,
    )
    .expect("major");
    newer_major.published = Some("2026-10-05".to_string());
    let older_breaking = release_change(
        &grade("lopdf", "0.45.0", &[(p, "0.42.0", true, false)]),
        &row("crates_io", "crates.io: lopdf v0.45.0"),
        &liveness,
        ReleaseAdvisories::Clear,
    )
    .expect("breaking");
    let dev_major = release_change(
        &grade("vitest", "4.0.0", &[(p, "3.0.0", true, true)]),
        &row("npm_registry", "npm: vitest v4.0.0"),
        &liveness,
        ReleaseAdvisories::Clear,
    )
    .expect("dev major");
    let yanked = release_change(
        &ReleaseGrade::from_pins(
            "quux",
            v("1.4.2"),
            vec![(p.to_string(), "1.4.1".to_string(), true, false)],
            &["1.4.1".to_string()],
        ),
        &row("crates_io", "crates.io: quux v1.4.2"),
        &liveness,
        ReleaseAdvisories::Clear,
    )
    .expect("yanked");
    let medium = fact(
        "left-pad",
        AlertUrgency::Medium,
        vec![site("app", "1.0.0", FixPath::NoFix)],
    );
    let critical = fact(
        "rmcp",
        AlertUrgency::Critical,
        vec![site("app", "1.0.0", FixPath::NoFix)],
    );

    let ranked = vec![
        release_item(&minor, NOW),
        release_item(&dev_major, NOW),
        release_item(&older_breaking, NOW),
        security_item(&medium, NOW),
        release_item(&yanked, NOW),
        release_item(&newer_major, NOW),
        security_item(&critical, NOW),
    ];
    let items = finish(ranked, 50);
    let deps: Vec<&str> = items.iter().map(|i| i.affected_deps[0].as_str()).collect();
    assert_eq!(
        deps,
        vec!["rmcp", "left-pad", "quux", "tokio", "lopdf", "vitest", "serde"]
    );
    for item in &items {
        assert!(validate_item(item).is_ok(), "{}", item.id);
        assert!(item.id.starts_with("stack-change:"));
        for a in &item.suggested_actions {
            assert!(ACTION_IDS.contains(&a.action_id.as_str()));
        }
    }
    assert_eq!(
        finish(
            items
                .iter()
                .cloned()
                .map(|item| Ranked {
                    change: StackChange::Minor,
                    dev_only: false,
                    published: None,
                    package: item.affected_deps[0].clone(),
                    item,
                })
                .collect(),
            3
        )
        .len(),
        3,
        "the cap holds"
    );
}

// --------------------------------------------------------------- snapshot --

/// Lane 1 on a SNAPSHOT of a real database (`recipe-live-verify-rust-on-db-snapshot`),
/// for the G5 measurement. Point BOTH `FOURDA_DB_PATH` (what the Brief's facts
/// open) and `FOURDA_VERIFY_DB` at the same COPY, and `FOURDA_DATA_DIR` at a
/// scratch directory; the stream is written to `FOURDA_LANE1_OUT` as JSON.
///   `cargo test --lib lane1_on_a_snapshot -- --ignored --nocapture`
#[test]
#[ignore = "requires FOURDA_VERIFY_DB pointing at a database snapshot copy"]
fn lane1_on_a_snapshot() {
    let Ok(path) = std::env::var("FOURDA_VERIFY_DB") else {
        panic!("set FOURDA_VERIFY_DB to a snapshot copy");
    };
    crate::register_sqlite_vec_extension();
    let db = Database::new(std::path::Path::new(&path)).expect("open snapshot");
    let items = collect(&db, true, ALL_ITEMS);
    let mut by_change = std::collections::BTreeMap::<&str, usize>::new();
    for item in &items {
        validate_item(item).unwrap_or_else(|e| panic!("invalid item {}: {e:?}", item.id));
        let change = item.id.split(':').nth(1).unwrap_or("?");
        *by_change.entry(change).or_default() += 1;
    }
    println!("lane 1: {} items {by_change:?}", items.len());
    if let Ok(out) = std::env::var("FOURDA_LANE1_OUT") {
        std::fs::write(&out, serde_json::to_string_pretty(&items).expect("json")).expect("write");
        println!("written to {out}");
    }
}
