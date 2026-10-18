// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `osv::severity`: the adapter around the shared rule. The rule itself is
//! pinned by `vectors/severity.jsonl`; these cover what the vectors cannot
//! see (several mirror rows per id, the sentences surfaces print).

use super::*;

fn matched(id: &str, aliases: &[&str], cvss: Option<f64>, label: Option<&str>) -> MatchedAdvisory {
    MatchedAdvisory {
        advisory_id: id.to_string(),
        summary: format!("{id} summary"),
        details: None,
        package_name: "pkg".into(),
        ecosystem: "crates.io".into(),
        installed_version: Some("1.0.0".into()),
        fixed_version: None,
        severity_type: None,
        cvss_score: cvss,
        source_url: None,
        is_version_confirmed: true,
        project_paths: vec!["/p".into()],
        published_at: None,
        dependency_instances: Vec::new(),
        aliases: aliases.iter().map(|s| s.to_string()).collect(),
        severity_label: label.map(str::to_string),
        severity_from: None,
    }
}

fn row(id: &str, package: &str, cvss: Option<f64>, label: Option<&str>) -> StoredAdvisory {
    StoredAdvisory {
        id: 0,
        advisory_id: id.to_string(),
        summary: String::new(),
        details: None,
        package_name: package.to_string(),
        ecosystem: "npm".into(),
        affected_ranges: None,
        fixed_versions: None,
        severity_type: None,
        cvss_score: cvss,
        source_url: None,
        published_at: None,
        modified_at: None,
        withdrawn_at: None,
        synced_at: String::new(),
        aliases: Vec::new(),
        severity_label: label.map(str::to_string),
    }
}

#[test]
fn an_unrated_match_takes_its_alias_grade_from_any_mirror_row() {
    let mut matches = vec![
        matched("GO-1", &["CVE-1", "GHSA-x"], None, None),
        matched("RUSTSEC-1", &["GHSA-x"], None, Some("low")),
    ];
    // One advisory, two packages: the highest row grades it, each row read
    // label first, CVSS band second.
    let mirror = vec![
        row("GHSA-x", "a", Some(9.8), Some("medium")),
        row("ghsa-X", "b", Some(7.5), None),
    ];
    grade_unrated(&mut matches, &mirror);
    assert_eq!(matches[0].severity_label.as_deref(), Some("high"));
    assert_eq!(matches[0].severity_from.as_deref(), Some("GHSA-x"));
    // A record with a grade of its own is untouched.
    assert_eq!(matches[1].severity_label.as_deref(), Some("low"));
    assert_eq!(matches[1].severity_from, None);
}

#[test]
fn a_maintenance_notice_is_never_graded_from_an_alias() {
    let mut matches = vec![matched("RUSTSEC-2", &["GHSA-y"], None, None)];
    matches[0].summary = "`instant` is unmaintained".into();
    grade_unrated(&mut matches, &[row("GHSA-y", "a", None, Some("high"))]);
    assert_eq!(matches[0].severity_label, None);
}

#[test]
fn the_most_severe_own_grade_of_a_cluster_wins_whatever_its_kind() {
    // Before: the highest CVSS band won outright, so a curated CRITICAL label
    // beside a CVSS 5.0 record read medium.
    let label = matched("GHSA-a", &["PYSEC-a"], None, Some("critical"));
    let scored = matched("PYSEC-a", &["GHSA-a"], Some(5.0), None);
    let graded = cluster_severity(&[&label, &scored]).expect("graded");
    assert_eq!((graded.tier, graded.from), ("critical", None));
}

#[test]
fn an_inherited_grade_is_labelled_and_explained() {
    let mut go = matched("GO-2024-3333", &["GHSA-w32m-9786-jp63"], None, None);
    go.severity_label = Some("high".into());
    go.severity_from = Some("GHSA-w32m-9786-jp63".into());
    let graded = group_severity(&[&go]).expect("graded");
    assert_eq!(graded.label(), "high (from GHSA-w32m-9786-jp63)");
    let note = inherited_note(Some(&graded), &[&go]).expect("note");
    assert!(note.starts_with("Severity high (from GHSA-w32m-9786-jp63): GO-2024-3333"));
    // An own grade at the same tier in another cluster makes it the group's own.
    let own = matched("GHSA-other", &[], None, Some("high"));
    let group = group_severity(&[&go, &own]).expect("graded");
    assert_eq!(group.from, None);
    assert!(inherited_note(Some(&group), &[&go, &own]).is_none());
}

#[test]
fn a_still_unknown_cluster_is_medium_and_malware_high() {
    let unknown = matched("GO-1", &[], None, None);
    assert_eq!(cluster_urgency(&[&unknown]), Urgency::Medium);
    let malware = matched("MAL-2026-2307", &[], None, None);
    assert_eq!(cluster_urgency(&[&malware]), Urgency::High);
    let low = matched("GHSA-low", &[], None, Some("low"));
    assert_eq!(cluster_urgency(&[&low]), Urgency::Watch);
}
