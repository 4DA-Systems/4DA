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
        cwe_ids: Vec::new(),
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
        cwe_ids: Vec::new(),
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
fn a_still_unknown_cluster_is_medium_and_installed_malware_critical() {
    let unknown = matched("GO-1", &[], None, None);
    assert_eq!(cluster_urgency(&[&unknown]), Urgency::Medium);
    let malware = matched("MAL-2026-2307", &[], None, None);
    assert_eq!(cluster_urgency(&[&malware]), Urgency::Critical);
    // Malware is Critical whatever else grades it.
    let graded_malware = matched("MAL-2099-0001", &[], None, Some("low"));
    assert_eq!(cluster_urgency(&[&graded_malware]), Urgency::Critical);
    let low = matched("GHSA-low", &[], None, Some("low"));
    assert_eq!(cluster_urgency(&[&low]), Urgency::Watch);
}

/// S2 of the #986 review: ONE maintenance predicate, the MCP's since the
/// #1008 review (B1): the summary says "unmaintained" / "no longer
/// maintained" as words; a fix does not stop it being a notice, and
/// "deprecated" alone does not make one. A notice with a graded GHSA alias
/// never inherits a vulnerability grade, whichever table the record comes
/// from; a real bug with the same alias does.
#[test]
fn a_deprecation_notice_never_inherits_a_vulnerability_grade() {
    let mirror = vec![row("GHSA-dep1", "other", None, Some("critical"))];
    let mut notice = matched("RUSTSEC-9", &["GHSA-dep1"], None, None);
    notice.summary = "foo is unmaintained".into();
    let mut bug = matched("RUSTSEC-10", &["GHSA-dep1"], None, None);
    bug.summary = "Out-of-bounds write".into();
    bug.fixed_version = Some("2.0.0".into());
    let mut deprecated = matched("RUSTSEC-11", &["GHSA-dep1"], None, None);
    deprecated.summary = "foo has been deprecated".into();
    let mut matches = vec![notice, bug, deprecated];
    grade_unrated(&mut matches, &mirror);
    assert_eq!(matches[0].severity_label, None);
    assert_eq!(matches[1].severity_from.as_deref(), Some("GHSA-dep1"));
    assert_eq!(matches[2].severity_from.as_deref(), Some("GHSA-dep1"));

    let grades = AliasGrades::from_rows(&mirror);
    let mut stored = row("RUSTSEC-9", "foo", None, None);
    stored.summary = "foo is unmaintained".into();
    stored.aliases = vec!["GHSA-dep1".into()];
    assert!(!takes_alias_grade(&stored));
    assert_eq!(rows_grade(&[&stored], &grades), None);
    // ring's RUSTSEC-2025-0010 shape: a published fix, still a notice.
    stored.fixed_versions = Some(r#"["2.0.0"]"#.into());
    assert_eq!(rows_grade(&[&stored], &grades), None);
    stored.summary = "Out-of-bounds write".into();
    assert_eq!(
        rows_grade(&[&stored], &grades).map(|g| g.label()),
        Some("critical (from GHSA-dep1)".to_string())
    );
}

/// Mirror rows are grouped by alias before grading: two advisories of one
/// package stay two groups, and the worst group decides.
#[test]
fn mirror_rows_group_by_alias_then_the_worst_group_decides() {
    let mut a = row("GHSA-a", "p", None, Some("low"));
    a.aliases = vec!["RUSTSEC-a".into()];
    let mut a2 = row("RUSTSEC-a", "p", None, None);
    a2.aliases = vec!["GHSA-a".into(), "GHSA-up".into()];
    a2.fixed_versions = Some(r#"["1.0.1"]"#.into());
    let b = row("GHSA-b", "p", Some(5.0), None);
    let grades = AliasGrades::from_rows(&[row("GHSA-up", "q", None, Some("critical"))]);
    let rows = [&a, &a2, &b];
    assert_eq!(alias_groups(&rows).len(), 2);
    // Group a: own low beats the inherited critical; group b: medium.
    assert_eq!(
        rows_grade(&rows, &grades).map(|g| g.label()),
        Some("medium".to_string())
    );
}

/// The temporary malware rule: a `MAL-` id OR a `MAL-` alias (a GHSA
/// malware advisory listing its OpenSSF record).
#[test]
fn a_mal_alias_is_installed_malware_too() {
    let ghsa = matched("GHSA-mal1", &["MAL-2025-46969"], None, Some("low"));
    assert!(contains_malware(&[&ghsa]));
    assert_eq!(cluster_urgency(&[&ghsa]), Urgency::Critical);
    let plain = matched("GHSA-plain", &["CVE-1"], None, Some("low"));
    assert!(!contains_malware(&[&plain]));
}

/// GitHub's malware title alone (GHSA-28f8-hqmc-7ph8: no MAL- id, no CWE)
/// is installed malware by the shared predicate, even beside a low grade.
#[test]
fn a_malware_title_alone_is_installed_malware() {
    let mut m = matched("GHSA-28f8-hqmc-7ph8", &[], None, Some("low"));
    m.summary = "Malicious Package in ember-power-timepicker".into();
    assert_eq!(cluster_urgency(&[&m]), Urgency::Critical);
}
