// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use semver::Version;

use super::releases::{change_of, release_change, requirements_admit, RegistryRow};
use super::*;
use crate::brief_facts::{FactStatus, NotCompiledNote};
use crate::evidence::{validate_item, ProjectLiveness, ACTION_IDS};
use crate::osv::fix_path::Requirement;
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
        &liveness
    )
    .is_none());
    let major = grade("vitest", "4.0.0", &[(p, "3.1.0", true, true)]);
    let change = release_change(
        &major,
        &row("npm_registry", "npm: vitest v4.0.0"),
        &liveness,
    )
    .expect("a breaking dev release still changes something");
    assert!(change.dev_only);
    assert_eq!(release_item(&change, NOW).item.urgency, Urgency::Watch);
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
    )
    .expect("minor");
    let mut newer_major = release_change(
        &grade("tokio", "2.0.0", &[(p, "1.0.0", true, false)]),
        &row("crates_io", "crates.io: tokio v2.0.0"),
        &liveness,
    )
    .expect("major");
    newer_major.published = Some("2026-10-05".to_string());
    let older_breaking = release_change(
        &grade("lopdf", "0.45.0", &[(p, "0.42.0", true, false)]),
        &row("crates_io", "crates.io: lopdf v0.45.0"),
        &liveness,
    )
    .expect("breaking");
    let dev_major = release_change(
        &grade("vitest", "4.0.0", &[(p, "3.0.0", true, true)]),
        &row("npm_registry", "npm: vitest v4.0.0"),
        &liveness,
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
    let items = collect(&db, true, MAX_ITEMS);
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
