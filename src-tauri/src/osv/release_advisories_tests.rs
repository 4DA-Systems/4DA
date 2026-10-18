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
        cwe_ids: Vec::new(),
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
    // The title template is anchored: prose that mentions malice is not it.
    assert!(!window("GHSA-c", "Malicious-looking prose", "[]").is_malware());
}

/// GHSA-28f8-hqmc-7ph8 as OSV serves it: no `MAL-` alias and no CWE, only
/// GitHub's malware title. It stays malware (review of #992, B1).
#[test]
fn a_ghsa_malware_record_with_no_alias_and_no_cwe_counts_by_its_title() {
    let w = window(
        "GHSA-28f8-hqmc-7ph8",
        "Malicious Package in ember-power-timepicker",
        r#"[{"type":"SEMVER","events":[{"introduced":"0"}]}]"#,
    );
    assert!(w.aliases.is_empty() && w.cwe_ids.is_empty());
    assert!(w.is_malware());
}

/// crossenv GHSA-c2m4-w5hm-vqjw: CWE-506, no `MAL-` alias, and a summary
/// outside the title template. Malware by its CWE.
#[test]
fn a_ghsa_malware_record_counts_by_its_cwe() {
    let mut crossenv = window(
        "GHSA-c2m4-w5hm-vqjw",
        "crossenv is malware",
        r#"[{"type":"SEMVER","events":[{"introduced":"0"}]}]"#,
    );
    crossenv.aliases = vec!["CVE-2017-16074".to_string()];
    crossenv.cwe_ids = vec!["CWE-506".to_string()];
    assert!(crossenv.is_malware());
}

fn fixed_before() -> AdvisoryWindow {
    window(
        "GHSA-old",
        "ReDoS",
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.0.1"}]}]"#,
    )
}

#[test]
fn a_clean_release_past_the_hold_is_clear() {
    let v = assess_npm(
        "4.2.0",
        &["4.1.3"],
        &[fixed_before()],
        &synced("npm", "2026-10-18 06:00:00"),
        Some("2026-10-14T09:00:00Z"),
    );
    assert_eq!(v, ReleaseAdvisories::Clear, "4 days old");
}

#[test]
fn a_clean_release_younger_than_the_hold_is_held() {
    let v = assess_npm(
        "4.2.0",
        &["4.1.3"],
        &[fixed_before()],
        &synced("npm", "2026-10-18 06:00:00"),
        Some("2026-10-17T09:00:00Z"),
    );
    assert_eq!(
        v,
        ReleaseAdvisories::Held {
            published: "2026-10-17".to_string(),
            days: 1,
        }
    );
    assert!(!v.is_clear());
}

#[test]
fn a_bare_publish_date_ages_from_its_last_second() {
    // 2026-10-15 read as midnight would be 3.5 days old (clear); read as
    // 23:59:59 it is 2.5 days old: still held.
    let v = assess_npm(
        "4.2.0",
        &[],
        &[],
        &synced("npm", "2026-10-18 06:00:00"),
        Some("2026-10-15"),
    );
    assert!(
        matches!(v, ReleaseAdvisories::Held { days: 2, .. }),
        "{v:?}"
    );
}

#[test]
fn an_undated_clean_release_is_unknown_not_clear() {
    let v = assess_npm(
        "4.2.0",
        &["4.1.3"],
        &[fixed_before()],
        &synced("npm", "2026-10-18 06:00:00"),
        None,
    );
    assert_eq!(v, ReleaseAdvisories::Unknown(UnknownReason::Undated));
}

/// The hold is the repository's own (`minimumReleaseAge`, minutes) and the
/// MCP server's `WAIT_DAYS`; and it is never shorter than the trust horizon,
/// which is what makes a release past it older than any ready sync.
#[test]
fn the_hold_matches_the_repo_minimum_release_age_and_covers_the_trust_horizon() {
    let yaml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../pnpm-workspace.yaml"),
    )
    .expect("pnpm-workspace.yaml");
    let minutes: i64 = yaml
        .lines()
        .find_map(|l| l.trim().strip_prefix("minimumReleaseAge:"))
        .and_then(|v| v.trim().parse().ok())
        .expect("minimumReleaseAge");
    assert_eq!(minutes, 4320);
    assert_eq!(RELEASE_HOLD_DAYS * 24 * 60, minutes);
    assert!(RELEASE_HOLD_DAYS * 24 >= readiness::TRUST_HORIZON_HOURS);
}

/// Coverage is judged on the copies passed in (the ACTIVE ones): an
/// advisory that hits one active copy but not another is not covered.
#[test]
fn coverage_needs_every_active_copy_affected() {
    let mal = window(
        "MAL-2026-77",
        "Malicious code in x (npm)",
        r#"[{"type":"ENUMERATED","events":["1.0.5","2.1.0"]}]"#,
    );
    let sync = synced("npm", "2026-10-18 06:00:00");
    let covered =
        |installed: &[&str]| match assess_npm("2.1.0", installed, &[mal.clone()], &sync, None) {
            ReleaseAdvisories::Affected(hit) => hit.covered_by_installed,
            other => panic!("expected affected, got {other:?}"),
        };
    assert!(covered(&["1.0.5"]));
    assert!(!covered(&["1.0.5", "2.0.0"]), "the 2.0.0 copy is not hit");
    assert!(!covered(&[]), "no active copy: nothing reports it");
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
    let old = (chrono::Utc::now() - chrono::Duration::days(10)).to_rfc3339();
    let good = checker.check(&db, "npm", "left-padder", "4.3.0", &installed, Some(&old));
    assert_eq!(good, ReleaseAdvisories::Clear);
}

#[test]
fn an_undecodable_advisory_row_makes_the_read_unknown() {
    let db = crate::test_utils::test_db();
    db.upsert_osv_advisory(
        "GHSA-x", "x", None, "pkg", "npm", None, None, None, None, None, None, None, None,
    )
    .expect("upsert");
    // A summary that is not TEXT cannot decode as a String.
    db.conn
        .lock()
        .execute("UPDATE osv_advisories SET summary = x'00ff'", [])
        .expect("corrupt");
    assert!(stored_advisories(&db, "npm", "pkg").is_none());
    db.update_osv_sync_status("npm", 1, None).expect("status");
    let v = ReleaseChecker::load(&db).check(&db, "npm", "pkg", "1.0.0", &[], None);
    assert_eq!(v, ReleaseAdvisories::Unknown(UnknownReason::Unreadable));
}
