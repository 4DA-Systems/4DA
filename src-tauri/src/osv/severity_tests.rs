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
        informational: None,
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
        informational: None,
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

/// A pinned conformance record, parsed as the sync parses OSV's.
fn pinned(id: &str) -> super::super::types::Vulnerability {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/conformance/osv")
        .join(format!("{id}.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{id}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// The mirror end to end (sync -> row -> match -> urgency): RustSec's
/// per-affected `informational: unmaintained` makes tempdir's
/// RUSTSEC-2018-0017 a notice although its summary only says "deprecated",
/// so it is Watch with no fix target; smallvec's per-affected `unsound`
/// RUSTSEC-2018-0018 stays a vulnerability and keeps its fix 0.6.13.
#[test]
fn the_per_affected_informational_flag_reaches_the_mirror_and_the_match() {
    let db = crate::test_utils::test_db();
    db.store_dependency("/p", "tempdir", Some("0.3.7"), "crates.io", false, None)
        .unwrap();
    db.store_dependency("/p", "smallvec", Some("0.6.10"), "crates.io", false, None)
        .unwrap();
    let mut seen = std::collections::HashSet::new();
    for id in ["RUSTSEC-2018-0017", "RUSTSEC-2018-0018"] {
        super::super::sync::store_vulnerability(&db, &pinned(id), &mut seen).unwrap();
    }
    let row = |pkg: &str| db.get_osv_advisories_for_package(pkg, "crates.io").unwrap();
    assert_eq!(row("tempdir")[0].informational, Some(true));
    assert!(
        !maintenance_text(&row("tempdir")[0].summary),
        "the summary alone misses it"
    );
    assert!(row("tempdir")[0].is_informational());
    assert_eq!(row("smallvec")[0].informational, Some(false));

    let matches = super::super::matching::get_matched_advisories(&db).unwrap();
    let by_pkg = |pkg: &str| {
        matches
            .iter()
            .find(|m| m.package_name == pkg)
            .unwrap_or_else(|| panic!("{pkg} matched"))
    };
    let tempdir = by_pkg("tempdir");
    assert!(tempdir.is_informational());
    assert_eq!(cluster_urgency(&[tempdir]), Urgency::Watch);
    assert!(tempdir
        .dependency_instances
        .iter()
        .all(|d| d.fixed_version.is_none()));
    let smallvec = by_pkg("smallvec");
    assert!(!smallvec.is_informational());
    assert_eq!(cluster_urgency(&[smallvec]), Urgency::Medium);
    assert_eq!(smallvec.fixed_version.as_deref(), Some("0.6.13"));
}

/// Phase 129/130 backfill: a row stored before a verdict was (it reads 0)
/// gets one on the next sync, which the migration makes due; an upsert that
/// does not carry a verdict keeps the stored one.
#[test]
fn a_pre_migration_row_gets_its_notice_flag_on_the_next_sync() {
    let db = crate::test_utils::test_db();
    db.store_dependency("/p", "tempdir", Some("0.3.7"), "crates.io", false, None)
        .unwrap();
    let vuln = pinned("RUSTSEC-2018-0017");
    super::super::sync::store_vulnerability(&db, &vuln, &mut Default::default()).unwrap();
    db.conn
        .lock()
        .execute("UPDATE osv_advisories SET informational = 0", [])
        .unwrap();
    db.update_osv_sync_status("crates.io", 1, None).unwrap();
    let stored = || {
        db.get_osv_advisories_for_package("tempdir", "crates.io")
            .unwrap()
    };
    assert!(
        !stored()[0].is_informational(),
        "as a pre-129 binary left it"
    );
    let max_age = super::super::sync::osv_sync_max_age_hours();
    assert!(!super::super::sync::needs_sync(&db, max_age).unwrap());
    crate::db::Database::make_osv_sync_due(&db.conn.lock()).unwrap();
    assert!(super::super::sync::needs_sync(&db, max_age).unwrap());
    super::super::sync::store_vulnerability(&db, &vuln, &mut Default::default()).unwrap();
    assert_eq!(stored()[0].informational, Some(true));
    // The legacy upsert (no flag) leaves it alone.
    db.upsert_osv_advisory(
        "RUSTSEC-2018-0017",
        "s",
        None,
        "tempdir",
        "crates.io",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(stored()[0].informational, Some(true));
}

/// RUSTSEC-2025-0068 (serde_yml) as api.osv.dev serves it (2026-10-26): the
/// per-affected class is `unsound` and the summary says "unsound and
/// unmaintained". The stated class decides (F-7): a vulnerability, never a
/// notice, so it is counted (Medium, nothing grades it) instead of folded
/// into Watch. A Phase-129 row stored it as a notice (1); the re-sync Phase
/// 130 makes due rewrites the verdict. A row with no verdict (0) still lets
/// the summary decide, as Phase 129 did.
#[test]
fn a_stated_unsound_class_beats_unmaintained_wording_end_to_end() {
    let vuln: super::super::types::Vulnerability = serde_json::from_str(
        r#"{"id":"RUSTSEC-2025-0068","summary":"serde_yml crate is unsound and unmaintained",
          "aliases":["GHSA-hhw4-xg65-fp2x"],"modified":"2025-10-28T06:29:25.168149Z",
          "published":"2025-09-11T12:00:00Z","database_specific":{"license":"CC0-1.0"},
          "affected":[{"package":{"name":"serde_yml","ecosystem":"crates.io","purl":"pkg:cargo/serde_yml"},
            "ranges":[{"type":"SEMVER","events":[{"introduced":"0.0.0-0"}]}],
            "database_specific":{"categories":[],"cvss":null,"informational":"unsound"}}]}"#,
    )
    .unwrap();
    let db = crate::test_utils::test_db();
    db.store_dependency("/p", "serde_yml", Some("0.0.12"), "crates.io", false, None)
        .unwrap();
    super::super::sync::store_vulnerability(&db, &vuln, &mut Default::default()).unwrap();
    let stored = || {
        db.get_osv_advisories_for_package("serde_yml", "crates.io")
            .unwrap()
    };
    assert_eq!(stored()[0].informational, Some(false));
    assert!(
        maintenance_text(&stored()[0].summary),
        "the wording alone would say notice"
    );
    assert!(!stored()[0].is_informational());
    let matches = super::super::matching::get_matched_advisories(&db).unwrap();
    let m = matches
        .iter()
        .find(|m| m.package_name == "serde_yml")
        .expect("serde_yml matched");
    assert!(!m.is_informational());
    assert_eq!(cluster_urgency(&[m]), Urgency::Medium);

    let set = |v: i64| {
        db.conn
            .lock()
            .execute("UPDATE osv_advisories SET informational = ?1", [v])
            .unwrap();
    };
    set(1);
    assert!(stored()[0].is_informational(), "as Phase 129 left it");
    super::super::sync::store_vulnerability(&db, &vuln, &mut Default::default()).unwrap();
    assert_eq!(
        stored()[0].informational,
        Some(false),
        "the re-sync rewrites it"
    );
    set(0);
    assert_eq!(stored()[0].informational, None);
    assert!(
        stored()[0].is_informational(),
        "no verdict: the summary decides"
    );
}

/// The alias-group notice rule (F-7): a record that is a notice only by its
/// wording, aliased to a record whose stated class is `unsound`, is part of a
/// vulnerability; two notice records stay a notice group.
#[test]
fn an_alias_group_is_a_notice_only_when_every_record_is() {
    let mut worded = matched("GHSA-w", &["RUSTSEC-u"], None, None);
    worded.summary = "x crate is unsound and unmaintained".into();
    let mut unsound = matched("RUSTSEC-u", &["GHSA-w"], None, None);
    unsound.summary = worded.summary.clone();
    unsound.informational = Some(false);
    let mut group = vec![worded.clone(), unsound];
    assert!(
        group[0].is_informational(),
        "a notice on its own, by wording"
    );
    settle_group_notices(&mut group);
    assert!(group.iter().all(|m| !m.is_informational()));
    let refs: Vec<&MatchedAdvisory> = group.iter().collect();
    assert!(!is_notice_group(&refs));
    assert_eq!(cluster_urgency(&refs), Urgency::Medium);

    let mut other = matched("RUSTSEC-n", &["GHSA-w"], None, None);
    other.informational = Some(true);
    let mut notices = vec![worded, other];
    settle_group_notices(&mut notices);
    assert!(notices.iter().all(|m| m.is_informational()));
    let refs: Vec<&MatchedAdvisory> = notices.iter().collect();
    assert!(is_notice_group(&refs));
    assert_eq!(cluster_urgency(&refs), Urgency::Watch);

    // Unlinked records are separate groups: a vulnerability elsewhere does
    // not touch a notice.
    let mut apart = vec![
        matched("A-1", &[], None, None),
        matched("B-1", &[], None, None),
    ];
    apart[0].informational = Some(true);
    apart[1].informational = Some(false);
    settle_group_notices(&mut apart);
    assert!(apart[0].is_informational());
}

/// The mirror read applies the group rule: serde_yml's GHSA (no class,
/// worded "unmaintained") and its RUSTSEC alias (class `unsound`) both read
/// as vulnerabilities from every list read and from the single-id read.
#[test]
fn the_mirror_reads_apply_the_alias_group_rule() {
    let parse =
        |json: &str| -> super::super::types::Vulnerability { serde_json::from_str(json).unwrap() };
    let ghsa = parse(
        r#"{"id":"GHSA-hhw4-xg65-fp2x","summary":"serde_yml crate is unsound and unmaintained",
          "aliases":["RUSTSEC-2025-0068"],"database_specific":{"severity":"MODERATE","cwe_ids":["CWE-787"]},
          "affected":[{"package":{"name":"serde_yml","ecosystem":"crates.io"},
            "ranges":[{"type":"SEMVER","events":[{"introduced":"0"},{"last_affected":"0.0.12"}]}]}]}"#,
    );
    let rustsec = parse(
        r#"{"id":"RUSTSEC-2025-0068","summary":"serde_yml crate is unsound and unmaintained",
          "aliases":["GHSA-hhw4-xg65-fp2x"],"database_specific":{"license":"CC0-1.0"},
          "affected":[{"package":{"name":"serde_yml","ecosystem":"crates.io"},
            "ranges":[{"type":"SEMVER","events":[{"introduced":"0.0.0-0"}]}],
            "database_specific":{"categories":[],"cvss":null,"informational":"unsound"}}]}"#,
    );
    let db = crate::test_utils::test_db();
    db.store_dependency("/p", "serde_yml", Some("0.0.12"), "crates.io", false, None)
        .unwrap();
    for v in [&ghsa, &rustsec] {
        super::super::sync::store_vulnerability(&db, v, &mut Default::default()).unwrap();
    }
    let rows = db
        .get_osv_advisories_for_package("serde_yml", "crates.io")
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| !r.is_informational()));
    assert!(db
        .get_all_osv_advisories()
        .unwrap()
        .iter()
        .all(|r| !r.is_informational()));
    let one = db
        .get_osv_advisory_by_id("GHSA-hhw4-xg65-fp2x")
        .unwrap()
        .expect("the GHSA row");
    assert_eq!(one.advisory_id, "GHSA-hhw4-xg65-fp2x");
    assert!(!one.is_informational());
    let matches = super::super::matching::get_matched_advisories(&db).unwrap();
    let refs: Vec<&MatchedAdvisory> = matches
        .iter()
        .filter(|m| m.package_name == "serde_yml")
        .collect();
    assert_eq!(refs.len(), 2);
    assert!(refs.iter().all(|m| !m.is_informational()), "0 notices");
    assert_eq!(
        super::super::identity::cluster_by_vulnerability(&refs).len(),
        1,
        "1 vulnerability"
    );
    assert_eq!(cluster_urgency(&refs), Urgency::Medium);
}
