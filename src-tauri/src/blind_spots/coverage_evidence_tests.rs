// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Evidence-bound risk + any-age registry coverage (audit 2026-10-07, wave 4c).

use rusqlite::params;

use super::super::tests::setup_test_db;
use super::super::{find_uncovered_deps, test_support, DepCoverage};
use super::*;

fn crate_dep(name: &str, projects: usize) -> DepCoverage {
    DepCoverage {
        package_name: name.to_string(),
        ecosystem: "cargo".to_string(),
        projects: (0..projects).map(|i| format!("/proj/{i}")).collect(),
    }
}

/// A crates.io registry row `days_ago` old, exact-linked to `package`.
fn registry_release(conn: &rusqlite::Connection, package: &str, version: &str, days_ago: i64) {
    conn.execute(
        "INSERT INTO source_items (title, source_type, content_type, created_at)
         VALUES (?1, 'crates_io', 'release_notes', datetime('now', ?2))",
        params![
            format!("crates.io: {package} v{version}"),
            format!("-{days_ago} days")
        ],
    )
    .expect("insert registry row");
    conn.execute(
        "INSERT INTO source_item_dependencies (source_item_id, package_name, ecosystem, match_type, confidence)
         VALUES (?1, ?2, 'cargo', 'exact_registry', 0.95)",
        params![conn.last_insert_rowid(), package],
    )
    .expect("link registry row");
}

#[test]
fn never_engaged_is_not_999_days_of_neglect() {
    // Old: days_since=999 (never opened) + 3 projects = "critical".
    assert_eq!(classify_dep_risk(None, 1, 3), "low");
    assert_ne!(classify_dep_risk(None, 0, 9), "critical");
    // A real, old engagement still escalates as before.
    assert_eq!(classify_dep_risk(Some(90), 1, 3), "critical");
    // Volume alone still reaches high.
    assert_eq!(classify_dep_risk(None, 10, 3), "high");
}

#[test]
fn only_security_evidence_licenses_high_or_critical() {
    assert_eq!(evidence_capped_risk("critical", false), "medium");
    assert_eq!(evidence_capped_risk("high", false), "medium");
    assert_eq!(evidence_capped_risk("medium", false), "medium");
    assert_eq!(evidence_capped_risk("low", false), "low");
    assert_eq!(evidence_capped_risk("critical", true), "critical");
}

#[test]
fn a_release_only_gap_is_capped_and_a_zero_signal_gap_is_untouched() {
    let conn = setup_test_db();
    registry_release(&conn, "schemars", "1.0.5", 2);
    test_support::install_test_conn(conn);
    let gap = |name: &str, available: u32| UncoveredDep {
        name: name.to_string(),
        dep_type: "crates.io".to_string(),
        projects_using: vec!["/a".into(), "/b".into(), "/c".into()],
        days_since_last_signal: 999,
        available_signal_count: available,
        risk_level: "critical".to_string(),
        match_type: "exact_registry".to_string(),
        coverage_reason: None,
        adapters_searched: Vec::new(),
        platform_active: true,
    };
    let out = cap_risk_to_evidence(vec![
        gap("schemars (crates.io)", 1),
        gap("quiet (crates.io)", 0),
    ]);
    let risk = |n: &str| {
        out.iter()
            .find(|d| d.name == n)
            .map(|d| d.risk_level.clone())
    };
    assert_eq!(risk("schemars (crates.io)").as_deref(), Some("medium"));
    assert_eq!(risk("quiet (crates.io)").as_deref(), Some("critical"));
}

#[test]
fn quiet_crate_with_an_old_exact_registry_link_is_not_uncovered() {
    // Live 2026-10-07: sha2 0.11.0 / ed25519-dalek 3.0.0 linked 33 days ago,
    // outside the 14-day window -> "Add source coverage for: sha2…".
    let conn = setup_test_db();
    registry_release(&conn, "sha2", "0.11.0", 33);
    registry_release(&conn, "ed25519-dalek", "3.0.0", 33);
    let deps = vec![
        crate_dep("sha2", 2),
        crate_dep("ed25519-dalek", 2),
        crate_dep("nevercovered", 2),
    ];
    let (uncovered, _weak) = find_uncovered_deps(&conn, &deps, 14).expect("query");
    let names: Vec<&str> = uncovered.iter().map(|d| d.name.as_str()).collect();
    assert!(!names.iter().any(|n| n.starts_with("sha2")), "{names:?}");
    assert!(!names.iter().any(|n| n.starts_with("ed25519")), "{names:?}");
    assert!(
        names.iter().any(|n| n.starts_with("nevercovered")),
        "a dep with no registry link at all is still a real gap: {names:?}"
    );
}

#[test]
fn a_registry_link_in_another_ecosystem_is_not_coverage() {
    let mut linked: HashMap<String, Vec<&'static str>> = HashMap::new();
    linked.insert("jsonwebtoken".into(), vec!["npm"]);
    assert!(!is_registry_covered(&linked, "jsonwebtoken", "cargo"));
    assert!(is_registry_covered(&linked, "jsonwebtoken", "npm"));
    assert!(is_registry_covered(&linked, "jsonwebtoken", "custom-eco"));
}
