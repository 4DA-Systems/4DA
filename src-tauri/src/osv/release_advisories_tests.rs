// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use chrono::NaiveDateTime;

use super::*;

fn at(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").expect("time")
}

fn synced(ecosystem: &str, when: &str) -> Vec<SyncStatus> {
    vec![SyncStatus {
        ecosystem: ecosystem.to_string(),
        last_synced_at: Some(when.to_string()),
        advisory_count: 10,
        error: None,
    }]
}

fn window(id: &str, summary: &str, ranges: &str) -> AdvisoryWindow {
    AdvisoryWindow {
        advisory_id: id.to_string(),
        summary: summary.to_string(),
        aliases: Vec::new(),
        affected_ranges: Some(ranges.to_string()),
    }
}

/// OSV's shape for a malicious release: the one bad version enumerated.
fn malware(version: &str) -> AdvisoryWindow {
    window(
        "MAL-2026-1234",
        "Malicious code in left-padder (npm)",
        &format!(r#"[{{"type":"ENUMERATED","events":["{version}"]}}]"#),
    )
}

const NOW: &str = "2026-10-18 12:00:00";

fn assess_npm(
    release: &str,
    installed: &[&str],
    advisories: &[AdvisoryWindow],
    statuses: &[SyncStatus],
    published: Option<&str>,
) -> ReleaseAdvisories {
    let installed: Vec<String> = installed.iter().map(|s| s.to_string()).collect();
    assess(
        "npm",
        release,
        &installed,
        advisories,
        false,
        statuses,
        at(NOW),
        false,
        published,
    )
}

#[test]
fn a_malicious_release_is_affected_and_named_as_malware() {
    let v = assess_npm(
        "4.2.0",
        &["4.1.3"],
        &[malware("4.2.0")],
        &synced("npm", "2026-10-18 06:00:00"),
        Some("2026-10-17 09:00:00"),
    );
    let ReleaseAdvisories::Affected(hit) = v else {
        panic!("expected affected, got {v:?}");
    };
    assert!(hit.malware);
    assert!(hit.confirmed);
    assert_eq!(hit.ids, vec!["MAL-2026-1234".to_string()]);
    assert!(
        !hit.covered_by_installed,
        "4.1.3 is not the malicious version: the advisory lane says nothing"
    );
}

#[test]
fn an_advisory_is_reported_even_when_the_mirror_is_stale() {
    let v = assess_npm(
        "4.2.0",
        &["4.1.3"],
        &[malware("4.2.0")],
        &synced("npm", "2026-09-01 00:00:00"),
        None,
    );
    assert!(matches!(v, ReleaseAdvisories::Affected(_)), "{v:?}");
}

#[test]
fn a_whole_package_advisory_that_already_hits_the_installed_copy_is_covered() {
    let all = window(
        "MAL-2026-9",
        "Malicious code in evil (npm)",
        r#"[{"type":"SEMVER","events":[{"introduced":"0"}]}]"#,
    );
    let v = assess_npm(
        "2.0.0",
        &["1.0.0"],
        &[all],
        &synced("npm", "2026-10-18 06:00:00"),
        None,
    );
    let ReleaseAdvisories::Affected(hit) = v else {
        panic!("expected affected");
    };
    assert!(hit.covered_by_installed);
}

#[test]
fn a_ghsa_malware_summary_counts_as_malware() {
    let w = window(
        "GHSA-xxxx-yyyy-zzzz",
        "Malicious code in foo",
        r#"[{"type":"SEMVER","events":[{"introduced":"0"}]}]"#,
    );
    assert!(w.is_malware());
    let mut aliased = window("GHSA-a", "Something", "[]");
    aliased.aliases = vec!["MAL-2026-1".to_string()];
    assert!(aliased.is_malware());
    assert!(!window("GHSA-b", "Prototype pollution", "[]").is_malware());
}

#[test]
fn a_clean_release_checked_after_it_was_published_is_clear() {
    let fixed_before = window(
        "GHSA-old",
        "ReDoS",
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.0.1"}]}]"#,
    );
    let v = assess_npm(
        "4.2.0",
        &["4.1.3"],
        &[fixed_before],
        &synced("npm", "2026-10-18 06:00:00"),
        Some("2026-10-17T09:00:00Z"),
    );
    assert_eq!(v, ReleaseAdvisories::Clear);
}

#[test]
fn a_window_the_matcher_cannot_read_is_not_clear() {
    // The `is_clear` predicate's conservative answer: no ranges -> affected.
    let unreadable = AdvisoryWindow {
        affected_ranges: None,
        ..window("GHSA-norange", "Something", "")
    };
    let v = assess_npm(
        "4.2.0",
        &["4.1.3"],
        &[unreadable],
        &synced("npm", "2026-10-18 06:00:00"),
        None,
    );
    let ReleaseAdvisories::Affected(hit) = v else {
        panic!("expected affected");
    };
    assert!(!hit.confirmed);
}

#[test]
fn a_mirror_older_than_the_release_is_unknown_not_clear() {
    let v = assess_npm(
        "4.2.0",
        &["4.1.3"],
        &[],
        &synced("npm", "2026-10-17 06:00:00"),
        Some("2026-10-17 09:00:00"),
    );
    assert!(
        matches!(
            v,
            ReleaseAdvisories::Unknown(UnknownReason::SyncedBeforeRelease { .. })
        ),
        "{v:?}"
    );
}

#[test]
fn a_stale_failed_or_missing_mirror_is_unknown() {
    let stale = assess_npm(
        "4.2.0",
        &[],
        &[],
        &synced("npm", "2026-10-01 00:00:00"),
        None,
    );
    assert!(matches!(
        stale,
        ReleaseAdvisories::Unknown(UnknownReason::Stale { .. })
    ));
    let never = assess_npm("4.2.0", &[], &[], &synced("crates.io", NOW), None);
    assert_eq!(
        never,
        ReleaseAdvisories::Unknown(UnknownReason::NeverSynced),
        "another ecosystem's sync says nothing about npm"
    );
    let failed = assess_npm(
        "4.2.0",
        &[],
        &[],
        &[SyncStatus {
            ecosystem: "npm".into(),
            last_synced_at: Some("2026-10-18 06:00:00".into()),
            advisory_count: 0,
            error: Some("offline".into()),
        }],
        None,
    );
    assert_eq!(failed, ReleaseAdvisories::Unknown(UnknownReason::Failed));
    assert!(!failed.is_clear());
}

#[test]
fn a_release_the_registry_withdrew_is_never_clear() {
    let v = assess(
        "npm",
        "4.18.0",
        &[],
        &[],
        true,
        &synced("npm", "2026-10-18 06:00:00"),
        at(NOW),
        false,
        None,
    );
    assert_eq!(v, ReleaseAdvisories::Withdrawn);
}

#[test]
fn stored_advisories_match_the_package_as_the_registry_names_it() {
    let db = crate::test_utils::test_db();
    for (id, name, eco) in [
        ("MAL-1", "left-padder", "npm"),
        ("GHSA-2", "left_padder", "npm"),
        ("RUSTSEC-3", "serde_json", "crates.io"),
    ] {
        db.upsert_osv_advisory(
            id,
            "x",
            None,
            name,
            eco,
            Some("[]"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("upsert");
    }
    let ids = |eco: &str, pkg: &str| -> Vec<String> {
        stored_advisories(&db, eco, pkg)
            .expect("readable")
            .into_iter()
            .map(|a| a.advisory_id)
            .collect()
    };
    assert_eq!(
        ids("npm", "Left-Padder"),
        vec!["MAL-1"],
        "npm `_` is another package"
    );
    assert_eq!(ids("crates.io", "serde-json"), vec!["RUSTSEC-3"]);
}

#[test]
fn the_checker_reads_the_mirror_and_the_sync_rows() {
    let db = crate::test_utils::test_db();
    db.upsert_osv_advisory(
        "MAL-2026-1234",
        "Malicious code in left-padder (npm)",
        None,
        "left-padder",
        "npm",
        Some(r#"[{"type":"ENUMERATED","events":["4.2.0"]}]"#),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("upsert");
    db.update_osv_sync_status("npm", 1, None).expect("status");
    let checker = ReleaseChecker::load(&db);
    let installed = vec!["4.1.3".to_string()];
    let bad = checker.check(&db, "npm", "left-padder", "4.2.0", &installed, None);
    assert!(
        matches!(bad, ReleaseAdvisories::Affected(ref h) if h.malware),
        "{bad:?}"
    );
    let good = checker.check(&db, "npm", "left-padder", "4.3.0", &installed, None);
    assert_eq!(good, ReleaseAdvisories::Clear);
}
