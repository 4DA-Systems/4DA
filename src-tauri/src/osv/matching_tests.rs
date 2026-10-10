// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Unit tests for `matching.rs` (split out: file-size gate).

use super::*;

#[test]
fn npm_security_holding_placeholder_is_not_malware() {
    assert!(is_npm_security_holding(
        "MAL-2025-21003",
        "npm",
        Some("0.0.1-security")
    ));
    assert!(!is_npm_security_holding(
        "MAL-2025-21003",
        "npm",
        Some("0.0.2")
    ));
    assert!(!is_npm_security_holding(
        "GHSA-xxxx",
        "npm",
        Some("0.0.1-security")
    ));
    assert!(!is_npm_security_holding(
        "MAL-2025-21003",
        "crates.io",
        Some("0.0.1-security")
    ));
    assert!(!is_npm_security_holding("MAL-2025-21003", "npm", None));
}

#[test]
fn unplaced_copies_fall_back_to_the_fix_that_clears_every_range() {
    assert_eq!(
        highest_listed_fix(&Some(r#"["15.5.24","16.3.3"]"#.to_string())).as_deref(),
        Some("16.3.3")
    );
    assert_eq!(
        highest_listed_fix(&Some(r#"["0.41.0"]"#.to_string())).as_deref(),
        Some("0.41.0")
    );
    assert_eq!(highest_listed_fix(&None), None);
}

#[test]
fn test_version_in_simple_range() {
    let ranges =
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.2.3"}]}]"#.to_string());

    let (affected, confirmed) = check_version_affected(Some("1.2.2"), &ranges);
    assert!(affected, "1.2.2 < 1.2.3 should be affected");
    assert!(confirmed);

    let (affected, confirmed) = check_version_affected(Some("1.2.3"), &ranges);
    assert!(!affected, "1.2.3 == fixed, should NOT be affected");
    assert!(confirmed);

    let (affected, confirmed) = check_version_affected(Some("2.0.0"), &ranges);
    assert!(!affected, "2.0.0 > 1.2.3 should NOT be affected");
    assert!(confirmed);
}

#[test]
fn test_version_in_compound_range() {
    let ranges = Some(
        r#"[{"type":"SEMVER","events":[
            {"introduced":"1.0.0"},{"fixed":"1.0.5"},
            {"introduced":"2.0.0"},{"fixed":"2.1.0"}
        ]}]"#
            .to_string(),
    );

    // In first range
    let (affected, _) = check_version_affected(Some("1.0.3"), &ranges);
    assert!(affected);

    // Between ranges (not affected)
    let (affected, confirmed) = check_version_affected(Some("1.5.0"), &ranges);
    assert!(!affected);
    assert!(confirmed);

    // In second range
    let (affected, _) = check_version_affected(Some("2.0.5"), &ranges);
    assert!(affected);

    // After all ranges
    let (affected, _) = check_version_affected(Some("2.1.0"), &ranges);
    assert!(!affected);
}

#[test]
fn test_no_version_conservative() {
    let ranges =
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.0.0"}]}]"#.to_string());

    let (affected, confirmed) = check_version_affected(None, &ranges);
    assert!(affected, "No version → conservative match");
    assert!(!confirmed, "No version → not confirmed");
}

#[test]
fn test_no_ranges_conservative() {
    let (affected, confirmed) = check_version_affected(Some("1.0.0"), &None);
    assert!(affected, "No ranges → conservative match");
    assert!(!confirmed);
}

#[test]
fn test_introduced_no_fixed() {
    let ranges = Some(r#"[{"type":"SEMVER","events":[{"introduced":"2.0.0"}]}]"#.to_string());

    let (affected, confirmed) = check_version_affected(Some("2.5.0"), &ranges);
    assert!(affected, "After introduced with no fix → affected");
    assert!(confirmed);

    let (affected, confirmed) = check_version_affected(Some("1.9.0"), &ranges);
    assert!(!affected, "Before introduced → not affected");
    assert!(confirmed);
}

#[test]
fn test_last_affected() {
    let ranges = Some(
        r#"[{"type":"SEMVER","events":[{"introduced":"1.0.0"},{"last_affected":"1.5.0"}]}]"#
            .to_string(),
    );

    let (affected, _) = check_version_affected(Some("1.3.0"), &ranges);
    assert!(affected, "1.3.0 <= 1.5.0 (last_affected)");

    let (affected, _) = check_version_affected(Some("1.5.0"), &ranges);
    assert!(affected, "1.5.0 == last_affected → still affected");

    let (affected, _) = check_version_affected(Some("1.5.1"), &ranges);
    assert!(!affected, "1.5.1 > last_affected → not affected");
}

#[test]
fn test_v_prefix_handled() {
    let ranges =
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.0.0"}]}]"#.to_string());

    let (affected, confirmed) = check_version_affected(Some("v1.5.0"), &ranges);
    assert!(affected);
    assert!(confirmed);
}

#[test]
fn test_two_part_version() {
    let ranges =
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.0.0"}]}]"#.to_string());

    let (affected, confirmed) = check_version_affected(Some("1.5"), &ranges);
    assert!(affected, "1.5 → 1.5.0 < 2.0.0");
    assert!(confirmed);
}

#[test]
fn test_unparseable_version_conservative() {
    let ranges =
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.0.0"}]}]"#.to_string());

    let (affected, confirmed) = check_version_affected(Some("banana"), &ranges);
    assert!(affected, "Unparseable → conservative");
    assert!(!confirmed);
}

#[test]
fn test_non_semver_range_type_skipped() {
    let ranges = Some(
        r#"[{"type":"GIT","events":[{"introduced":"abc123"},{"fixed":"def456"}]}]"#.to_string(),
    );

    // GIT ranges are skipped, no SEMVER ranges found → conservative false
    // (because we went through all ranges and found none applicable)
    let (affected, confirmed) = check_version_affected(Some("1.0.0"), &ranges);
    // No SEMVER range matched → not affected (we only skip non-SEMVER ranges)
    assert!(!affected);
    assert!(confirmed);
}

#[test]
fn test_na_unknown_boundary_does_not_match() {
    // Real shape of PYSEC-2025-210 (torch): last_affected "2.5.0-NA"/"2.7.1-NA".
    // OSV's own matcher does NOT return torch 2.3.0 for it; semver parses "-NA" as a
    // prerelease and a naive compare would over-match. An unknown bound must not match.
    let ranges = Some(
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"last_affected":"2.5.0-NA"},{"last_affected":"2.7.1-NA"}]}]"#
            .to_string(),
    );
    let (affected, confirmed) = check_version_affected(Some("2.3.0"), &ranges);
    assert!(!affected, "unknown '-NA' boundary must not ground a match");
    assert!(
        confirmed,
        "we DID evaluate the ranges (just found no usable bound)"
    );

    // A concrete prerelease/build bound still matches normally.
    let rc = Some(
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"2.7.1-rc1"}]}]"#.to_string(),
    );
    let (affected, _) = check_version_affected(Some("2.5.0"), &rc);
    assert!(affected, "concrete prerelease bound still compares");
}

#[test]
fn test_normalize_ecosystem() {
    assert_eq!(normalize_ecosystem("rust"), "crates.io");
    assert_eq!(normalize_ecosystem("javascript"), "npm");
    assert_eq!(normalize_ecosystem("python"), "PyPI");
    assert_eq!(normalize_ecosystem("pip"), "PyPI");
    assert_eq!(normalize_ecosystem("go"), "Go");
    assert_eq!(normalize_ecosystem("golang"), "Go");
    assert_eq!(normalize_ecosystem("unknown"), "unknown");
}

#[test]
fn test_parse_version_formats() {
    assert!(parse_version("1.2.3").is_some());
    assert!(parse_version("v1.2.3").is_some());
    assert!(parse_version("1.2").is_some());
    assert!(parse_version("0.0.0").is_some());
    assert!(parse_version("banana").is_none());
    assert!(parse_version("").is_none());
}

#[test]
fn test_matched_advisories_integration() {
    use crate::test_utils::test_db;

    let db = test_db();

    // Store a dependency
    db.store_dependency("/project/a", "lodash", Some("4.17.20"), "npm", false, None)
        .unwrap();

    // Store an advisory that affects lodash < 4.17.21
    db.upsert_osv_advisory(
        "GHSA-test-001",
        "Prototype pollution in lodash",
        Some("Details here"),
        "lodash",
        "npm",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.17.21"}]}]"#),
        Some(r#"["4.17.21"]"#),
        Some("CVSS_V3"),
        Some(7.5),
        Some("https://github.com/advisories/GHSA-test-001"),
        Some("2026-01-01T00:00:00Z"),
        None,
        None,
    )
    .unwrap();

    let matches = get_matched_advisories(&db).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].advisory_id, "GHSA-test-001");
    assert_eq!(matches[0].installed_version.as_deref(), Some("4.17.20"));
    assert_eq!(matches[0].fixed_version.as_deref(), Some("4.17.21"));
    assert!(matches[0].is_version_confirmed);
    assert_eq!(matches[0].project_paths, vec!["/project/a"]);
    assert_eq!(matches[0].dependency_instances.len(), 1);
    assert!(matches[0].dependency_instances[0].is_direct);
    assert!(!matches[0].dependency_instances[0].is_dev);
}

#[test]
fn test_no_match_when_version_patched() {
    use crate::test_utils::test_db;

    let db = test_db();

    db.store_dependency("/project/a", "lodash", Some("4.17.21"), "npm", false, None)
        .unwrap();

    db.upsert_osv_advisory(
        "GHSA-test-002",
        "Vuln in lodash",
        None,
        "lodash",
        "npm",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.17.21"}]}]"#),
        Some(r#"["4.17.21"]"#),
        Some("CVSS_V3"),
        Some(7.5),
        None,
        None,
        None,
        None,
    )
    .unwrap();

    let matches = get_matched_advisories(&db).unwrap();
    assert!(matches.is_empty(), "Patched version should not match");
}

#[test]
fn test_multiple_projects_same_dep() {
    use crate::test_utils::test_db;

    let db = test_db();

    db.store_dependency("/project/a", "serde", Some("1.0.100"), "rust", false, None)
        .unwrap();
    db.store_dependency("/project/b", "serde", Some("1.0.100"), "rust", false, None)
        .unwrap();

    db.upsert_osv_advisory(
        "GHSA-test-003",
        "Vuln in serde",
        None,
        "serde",
        "crates.io",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.0.200"}]}]"#),
        Some(r#"["1.0.200"]"#),
        None,
        Some(5.0),
        None,
        None,
        None,
        None,
    )
    .unwrap();

    let matches = get_matched_advisories(&db).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].project_paths.len(), 2);
}

// ---- Multi-version inventory (Phase 92) — false-negative fix ----

fn vuln_advisory(db: &Database, id: &str, pkg: &str, eco: &str, fixed: &str) {
    db.upsert_osv_advisory(
        id,
        &format!("Vuln in {pkg}"),
        None,
        pkg,
        eco,
        Some(&format!(
            r#"[{{"type":"SEMVER","events":[{{"introduced":"0"}},{{"fixed":"{fixed}"}}]}}]"#
        )),
        Some(&format!(r#"["{fixed}"]"#)),
        Some("CVSS_V3"),
        Some(7.5),
        None,
        Some("2026-01-01T00:00:00Z"),
        None,
        None,
    )
    .unwrap();
}

fn inst(pkg: &str, version: &str, is_direct: bool) -> crate::db::DependencyInstanceInput {
    crate::db::DependencyInstanceInput {
        package_name: pkg.to_string(),
        version: version.to_string(),
        is_direct,
        is_dev: false,
        scope: "unknown".to_string(),
    }
}

#[test]
fn matcher_surfaces_vulnerable_duplicate_hidden_by_collapse() {
    use crate::test_utils::test_db;
    let db = test_db();

    // The collapsed user_dependencies row keeps only the patched survivor —
    // exactly the state in which test_no_match_when_version_patched (above)
    // correctly reports NO match. But the project ALSO installs a vulnerable
    // transitive copy, retained only by the multi-version inventory.
    db.store_dependency("/project/a", "lodash", Some("4.17.21"), "npm", false, None)
        .unwrap();
    db.store_dependency_instances(
        "/project/a",
        "npm",
        &[
            inst("lodash", "4.17.21", true),
            inst("lodash", "4.17.20", false),
        ],
    )
    .unwrap();
    vuln_advisory(&db, "GHSA-dup-1", "lodash", "npm", "4.17.21");

    let matches = get_matched_advisories(&db).unwrap();
    assert_eq!(
        matches.len(),
        1,
        "the hidden vulnerable 4.17.20 duplicate must surface the advisory"
    );
    assert!(matches[0].is_version_confirmed);
    assert!(
        matches[0]
            .dependency_instances
            .iter()
            .any(|d| d.installed_version.as_deref() == Some("4.17.20") && d.is_version_confirmed),
        "the confirmed-affected instance is the vulnerable duplicate; got {:?}",
        matches[0].dependency_instances
    );
    assert_eq!(matches[0].project_paths, vec!["/project/a"]);
}

#[test]
fn matcher_no_instances_falls_back_to_collapsed_unchanged() {
    use crate::test_utils::test_db;
    let db = test_db();
    // No instance rows (a pre-Phase-92 scan): behavior is identical to before
    // — the collapsed version alone decides the match.
    db.store_dependency("/project/a", "lodash", Some("4.17.21"), "npm", false, None)
        .unwrap();
    vuln_advisory(&db, "GHSA-fallback-1", "lodash", "npm", "4.17.21");
    assert!(
        get_matched_advisories(&db).unwrap().is_empty(),
        "patched collapsed version with no instances still must not match"
    );
}

#[test]
fn matcher_dedups_instance_matching_collapsed_version() {
    use crate::test_utils::test_db;
    let db = test_db();
    db.store_dependency("/project/a", "lodash", Some("4.17.20"), "npm", false, None)
        .unwrap();
    // Instance carries the SAME version as the collapsed survivor.
    db.store_dependency_instances("/project/a", "npm", &[inst("lodash", "4.17.20", true)])
        .unwrap();
    vuln_advisory(&db, "GHSA-dedup-1", "lodash", "npm", "4.17.21");

    let matches = get_matched_advisories(&db).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].dependency_instances.len(),
        1,
        "instance duplicating the collapsed version must not be double-counted"
    );
}

#[test]
fn go_pseudo_version_is_inside_an_introduced_zero_window() {
    // GO-2022-0229 shape: every commit before the fix, pseudo-versions only.
    let ranges = Some(
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"0.0.0-20220314234659-1baeb1ce4c0b"}]}]"#
            .to_string(),
    );
    assert_eq!(
        check_version_affected(Some("0.0.0-20190308221718-c2843e01d9a2"), &ranges),
        (true, true),
        "a pseudo-version of 0.0.0 is above introduced 0"
    );
    assert_eq!(
        check_version_affected(Some("0.0.0-20220315000000-abcdefabcdef"), &ranges),
        (false, true),
        "a later pseudo-version is past the fix"
    );
    let open_ended = Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"}]}]"#.to_string());
    assert_eq!(
        check_version_affected(Some("0.0.0-20200101000000-abcdefabcdef"), &open_ended),
        (true, true)
    );
}

#[test]
fn go_incompatible_build_metadata_does_not_decide_the_window() {
    let ranges =
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"3.2.1"}]}]"#.to_string());
    assert_eq!(
        check_version_affected(Some("3.2.0+incompatible"), &ranges),
        (true, true)
    );
    assert_eq!(
        check_version_affected(Some("3.2.1+incompatible"), &ranges),
        (false, true)
    );
}

#[test]
fn pypi_pep440_versions_are_confirmed_not_conservative() {
    let ranges = Some(
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"2.0.post1"}]}]"#.to_string(),
    );
    assert_eq!(check_version_affected(Some("2.0"), &ranges), (true, true));
    assert_eq!(
        check_version_affected(Some("2.0rc1"), &ranges),
        (true, true)
    );
    assert_eq!(
        check_version_affected(Some("2.0.post1"), &ranges),
        (false, true)
    );
    assert_eq!(
        check_version_affected(Some("1.2.3.4"), &ranges),
        (true, true)
    );
}

#[test]
fn pypi_names_match_across_pep503_spellings() {
    assert_eq!(
        package_key("Typing_Extensions", "PyPI"),
        "typing-extensions"
    );
    assert_eq!(package_key("zope.interface", "python"), "zope-interface");
    assert_eq!(package_key("Foo__Bar", "pypi"), "foo-bar");
    assert_eq!(package_key("@Scope/Pkg_x", "npm"), "@scope/pkg_x");
}

#[test]
fn fix_for_a_pseudo_version_is_its_window_fix() {
    let ranges = Some(
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"0.0.0-20220314234659-1baeb1ce4c0b"}]}]"#
            .to_string(),
    );
    assert_eq!(
        fix_target::fix_for_version("0.0.0-20190308221718-c2843e01d9a2", &ranges).as_deref(),
        Some("0.0.0-20220314234659-1baeb1ce4c0b")
    );
}

#[test]
fn an_advisory_with_only_an_enumerated_versions_list_matches_those_versions() {
    // GHSA-q58r-hwc8-rm9j (bootstrap) carries `versions` and no ranges; it was
    // stored as `[]` and read as confirmed-not-affected.
    let ranges = Some(r#"[{"type":"ENUMERATED","events":["3.4.0","3.4.1"]}]"#.to_string());
    assert_eq!(check_version_affected(Some("3.4.1"), &ranges), (true, true));
    assert_eq!(
        check_version_affected(Some("3.4.2"), &ranges),
        (false, true)
    );
    let pypi = Some(r#"[{"type":"ENUMERATED","events":["19.9"]}]"#.to_string());
    assert_eq!(check_version_affected(Some("19.9.0"), &pypi), (true, true));
    assert_eq!(fix_target::fix_for_version("3.4.1", &ranges), None);
}

#[test]
fn a_bound_with_leading_zeros_is_read_as_its_number() {
    // GHSA-qrqr-3x5j-2xw9 (github.com/docker/docker): fixed "17.06.0-ce".
    let ranges = Some(
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"17.06.0-ce"}]}]"#
            .to_string(),
    );
    assert_eq!(
        check_version_affected(Some("20.10.7+incompatible"), &ranges),
        (false, true)
    );
    assert_eq!(
        check_version_affected(Some("17.03.2-ce"), &ranges),
        (true, true)
    );
}
