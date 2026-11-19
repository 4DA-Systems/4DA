// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `osv_live`: batch attribution, CVSS grading, merged ranges and
//! the grounded items the live path builds.

use super::*;

fn vuln_with(affected: Vec<OsvAffected>) -> OsvVulnerability {
    OsvVulnerability {
        id: "GHSA-test".to_string(),
        summary: Some("s".to_string()),
        details: None,
        severity: Some(vec![OsvSeverity {
            severity_type: "CVSS_V3".to_string(),
            score: "5.3".to_string(),
        }]),
        affected: Some(affected),
        references: Some(vec![OsvReference {
            ref_type: "ADVISORY".to_string(),
            url: "https://example.com/a".to_string(),
        }]),
        published: Some("2026-06-15T00:00:00Z".to_string()),
        modified: None,
    }
}
fn pkg(name: &str, eco: &str) -> OsvPackage {
    OsvPackage {
        name: name.to_string(),
        ecosystem: eco.to_string(),
    }
}
fn semver(intro: &str, fixed: &str) -> OsvRange {
    OsvRange {
        range_type: "SEMVER".to_string(),
        events: Some(vec![
            serde_json::json!({ "introduced": intro }),
            serde_json::json!({ "fixed": fixed }),
        ]),
    }
}

fn batch(json: &str) -> OsvBatchResponse {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{e}: {json}"))
}

/// OSV.dev answers a querybatch with one result per query, in query
/// order, and leaves `vulns` out of a result no advisory affects. An
/// answer that does not carry one result per query cannot be attributed
/// to its dependencies, and a result whose `vulns` is `null` is not the
/// "none" answer: either makes the batch an error (the strict path then
/// falls back to the mirror), never a run of clean dependencies. Padding
/// the short tail with empty lists read every unanswered dependency as
/// clean (twin of the @4da/mcp-server audit of 2026-11-19, S1 F04b/F05).
#[test]
fn a_batch_answer_that_cannot_be_attributed_is_an_error_not_clean() {
    let ok = batch(r#"{"results":[{"vulns":[{"id":"A"}]},{},{"vulns":[{"id":"B"},{"id":"C"}]}]}"#);
    let ids = batch_ids(ok, 3).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        ids,
        vec![vec!["A".to_string()], vec![], vec!["B".into(), "C".into()]]
    );
    for short in [
        r#"{"results":[{"vulns":[{"id":"A"}]},{}]}"#,
        r#"{"results":[]}"#,
        "{}",
    ] {
        assert!(batch_ids(batch(short), 3).is_err(), "{short}");
    }
    let long = batch(r#"{"results":[{},{},{},{}]}"#);
    assert!(batch_ids(long, 3).is_err(), "more results than queries");
    let null = batch(r#"{"results":[{},{"vulns":null},{}]}"#);
    assert!(batch_ids(null, 3).is_err(), "vulns: null");
}

/// The live path grades as the mirror does: the highest valid CVSS
/// score (PYSEC-2026-1943's v3 0.0 beside a v4 8.7; a v3 7.5 beside a v4
/// 9.3), not the v3 entry because it comes first.
#[test]
fn best_severity_is_the_highest_valid_score() {
    let with = |entries: &[(&str, &str)]| {
        let mut v = vuln_with(Vec::new());
        v.severity = Some(
            entries
                .iter()
                .map(|(kind, score)| OsvSeverity {
                    severity_type: (*kind).to_string(),
                    score: (*score).to_string(),
                })
                .collect(),
        );
        best_severity(&v)
    };
    let v3_none = ("CVSS_V3", "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:N");
    let v4_high = (
        "CVSS_V4",
        "CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:N/VI:N/VA:H/SC:N/SI:N/SA:N",
    );
    let (kind, score) = with(&[v3_none, v4_high]);
    assert_eq!(kind.as_deref(), Some("CVSS_V4"));
    assert!(score.is_some_and(|s| (s - 8.7).abs() < 1e-9), "{score:?}");
    let (kind, score) = with(&[
        ("CVSS_V3", "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H"),
        (
            "CVSS_V4",
            "CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:H/VI:H/VA:H/SC:N/SI:N/SA:N",
        ),
    ]);
    assert_eq!(kind.as_deref(), Some("CVSS_V4"));
    assert!(score.is_some_and(|s| (s - 9.3).abs() < 1e-9), "{score:?}");
    // A record whose entry has no score string still parses.
    let parsed: OsvVulnerability =
        serde_json::from_str(r#"{"id":"PYSEC-2099-3","severity":[{"type":"CVSS_V3"}]}"#)
            .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(best_severity(&parsed), (Some("CVSS_V3".to_string()), None));
}

#[test]
fn test_merged_ranges_union_multi_branch() {
    // Real shape of GHSA-vfv6 (next): affected in TWO branches as separate entries. The merged
    // row must carry BOTH ranges so the ledger can match a pin in EITHER (the live-cycle gap).
    let v = vuln_with(vec![
        OsvAffected {
            package: Some(pkg("next", "npm")),
            ranges: Some(vec![semver("13.4.6", "15.5.16")]),
        },
        OsvAffected {
            package: Some(pkg("next", "npm")),
            ranges: Some(vec![semver("16.0.0", "16.2.5")]),
        },
    ]);
    let merged = merged_ranges_by_package(&v);
    assert_eq!(merged.len(), 1, "one merged row for (next, npm)");
    let (name, eco, ranges_json, fixed_json) = &merged[0];
    assert_eq!(name, "next");
    assert_eq!(eco, "npm");
    let rj = ranges_json.as_ref().unwrap();
    assert!(
        rj.contains("13.4.6") && rj.contains("16.0.0"),
        "BOTH branches present: {rj}"
    );
    let fj = fixed_json.as_ref().unwrap();
    assert!(
        fj.contains("15.5.16") && fj.contains("16.2.5"),
        "both fixes present: {fj}"
    );
}

/// FX-H5: "Fixed in: 4.18.0" for lodash, a release npm deprecated.
#[test]
fn a_withdrawn_fixed_in_release_is_annotated() {
    let v = vuln_with(vec![OsvAffected {
        package: Some(pkg("lodash", "npm")),
        ranges: Some(vec![semver("0", "4.18.0")]),
    }]);
    let facts = fourda_depengine::registry::RegistryReleases {
        published: vec!["4.18.0".into(), "4.18.1".into()],
        excluded: vec!["4.18.0".into()],
    };
    assert_eq!(
        withdrawn_note(&v, "lodash", "npm", Some(&facts)).as_deref(),
        Some(" (withdrawn by the registry; use 4.18.1)")
    );
    let only = fourda_depengine::registry::RegistryReleases {
        published: vec!["4.18.0".into()],
        excluded: vec!["4.18.0".into()],
    };
    assert!(withdrawn_note(&v, "lodash", "npm", Some(&only))
        .is_some_and(|n| n.contains("no later release")));
    let live = fourda_depengine::registry::RegistryReleases::default();
    assert_eq!(withdrawn_note(&v, "lodash", "npm", Some(&live)), None);
    assert_eq!(withdrawn_note(&v, "lodash", "npm", None), None);
}

#[test]
fn test_live_vuln_to_grounded_item_title_and_fields() {
    let v = vuln_with(vec![OsvAffected {
        package: Some(pkg("js-yaml", "npm")),
        ranges: Some(vec![semver("0", "4.2.0")]),
    }]);
    let item = live_vuln_to_grounded_item(&v, "js-yaml", "npm", "4.1.1");
    assert_eq!(item.source_type, "osv");
    assert_eq!(item.source_id, "GHSA-test");
    assert_eq!(item.title, "[GHSA-test] js-yaml: s");
    let after_id = item
        .title
        .trim_start_matches('[')
        .splitn(2, ']')
        .nth(1)
        .unwrap()
        .trim();
    assert_eq!(after_id.split([' ', ':']).next().unwrap(), "js-yaml");
    assert!(item.content.contains("js-yaml (npm)"));
    assert!(item.content.contains("Installed: 4.1.1"));
    assert!(item.content.contains("Fixed in: 4.2.0"));
    let md = item.metadata.unwrap();
    assert_eq!(md["fixed_version"], "4.2.0");
    assert_eq!(md["cvss_score"], 5.3);
    assert_eq!(md["manifest_grounded"], true);
}

#[test]
fn test_fixed_version_skips_git_and_na() {
    let v = vuln_with(vec![OsvAffected {
        package: Some(pkg("torch", "PyPI")),
        ranges: Some(vec![
            OsvRange {
                range_type: "GIT".to_string(),
                events: Some(vec![serde_json::json!({"fixed": "7c35874deadbeef"})]),
            },
            semver("0", "2.2.0"),
        ]),
    }]);
    assert_eq!(
        fixed_version_for(&v, "torch", "PyPI").as_deref(),
        Some("2.2.0")
    );
}

#[test]
fn test_dep_ecosystem_to_osv() {
    assert_eq!(dep_ecosystem_to_osv("rust"), Some("crates.io"));
    assert_eq!(dep_ecosystem_to_osv("npm"), Some("npm"));
    assert_eq!(dep_ecosystem_to_osv("typescript"), Some("npm"));
    assert_eq!(dep_ecosystem_to_osv("pypi"), Some("PyPI"));
    assert_eq!(dep_ecosystem_to_osv("go"), Some("Go"));
    // Language-name keys must resolve (ACE tags scanned deps by language) — regression guard
    // for the gap that surfaced ZERO C#/PHP/Dart advisories in strict mode.
    assert_eq!(dep_ecosystem_to_osv("csharp"), Some("NuGet"));
    assert_eq!(dep_ecosystem_to_osv("nuget"), Some("NuGet"));
    assert_eq!(dep_ecosystem_to_osv("php"), Some("Packagist"));
    assert_eq!(dep_ecosystem_to_osv("packagist"), Some("Packagist"));
    assert_eq!(dep_ecosystem_to_osv("dart"), Some("Pub"));
    assert_eq!(dep_ecosystem_to_osv("flutter"), Some("Pub"));
    assert_eq!(dep_ecosystem_to_osv("pub"), Some("Pub"));
    assert_eq!(dep_ecosystem_to_osv("cocoapods"), None);
}
