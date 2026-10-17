// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Unit tests for `matching.rs`. The app's `osv::matching_tests` still runs
//! the same rules end to end through its DB adapter.

use super::*;

fn pkg(eco: &str, project: &str, name: &str, version: Option<&str>, primary: bool) -> Package {
    Package {
        ecosystem: eco.to_string(),
        project: project.to_string(),
        name: name.to_string(),
        version: version.map(str::to_string),
        is_direct: primary,
        is_dev: false,
        primary,
    }
}

fn record(id: &str, eco: &str, name: &str, ranges: &str, fixed: Option<&str>) -> AdvisoryRecord {
    AdvisoryRecord {
        advisory_id: id.to_string(),
        package_name: name.to_string(),
        ecosystem: eco.to_string(),
        affected_ranges: Some(ranges.to_string()),
        fixed_versions: fixed.map(str::to_string),
    }
}

const BELOW_1_2_4: &str = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.2.4"}]}]"#;

#[test]
fn an_installed_copy_is_matched_even_when_the_resolved_row_is_patched() {
    let inventory = vec![
        pkg(
            "javascript",
            "C:\\Work\\App\\",
            "Lodash",
            Some("1.3.0"),
            true,
        ),
        pkg("npm", "c:/work/app", "lodash", Some("1.2.3"), false),
        pkg("npm", "c:/work/app", "lodash", Some("1.3.0"), false),
    ];
    let records = vec![record(
        "GHSA-1",
        "npm",
        "lodash",
        BELOW_1_2_4,
        Some(r#"["1.2.4"]"#),
    )];
    let findings = match_inventory(&inventory, &records);
    assert_eq!(findings.len(), 1);
    let f = &findings[0];
    assert_eq!(f.record, 0);
    assert!(f.is_version_confirmed);
    assert_eq!(f.installed_version.as_deref(), Some("1.2.3"));
    assert_eq!(f.fixed_version.as_deref(), Some("1.2.4"));
    assert_eq!(f.project_paths, vec!["c:/work/app".to_string()]);
    assert_eq!(f.dependency_instances.len(), 1);
    let copy = &f.dependency_instances[0];
    assert_eq!(copy.clean_version.as_deref(), Some("1.2.4"));
    assert_eq!(copy.target_version.as_deref(), Some("1.2.4"));
}

#[test]
fn no_resolved_row_means_no_audit_and_the_hook_can_drop_copies() {
    let copies_only = vec![pkg("npm", "/p", "a", Some("1.0.0"), false)];
    let records = vec![record("GHSA-1", "npm", "a", BELOW_1_2_4, None)];
    assert!(match_inventory(&copies_only, &records).is_empty());

    let inventory = vec![pkg("npm", "/p", "a", Some("1.0.0"), true)];
    assert_eq!(match_inventory(&inventory, &records).len(), 1);
    let dropped = match_inventory_with(
        &inventory,
        &records,
        &RegistryIndex::new(),
        |_, _, copies| copies.clear(),
    );
    assert!(dropped.is_empty());
}

fn facts(published: &[&str], excluded: &[&str]) -> RegistryReleases {
    RegistryReleases {
        published: published.iter().map(ToString::to_string).collect(),
        excluded: excluded.iter().map(ToString::to_string).collect(),
    }
}

/// FX-H5: lodash 4.17.21's advisories are all fixed in 4.18.0, which npm
/// marks deprecated ("Bad release"). No field may name it.
#[test]
fn a_withdrawn_fix_is_never_a_copy_target_or_a_finding_fix() {
    let inventory = vec![pkg("npm", "/p", "lodash", Some("4.17.21"), true)];
    let ranges = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.18.0"}]}]"#;
    let records = vec![record(
        "GHSA-1",
        "npm",
        "lodash",
        ranges,
        Some(r#"["4.18.0"]"#),
    )];
    let mut registry = RegistryIndex::new();
    registry.insert(
        ("lodash".into(), "npm".into()),
        facts(&["4.17.21", "4.17.23", "4.18.0", "4.18.1"], &["4.18.0"]),
    );
    let f = &match_inventory_with(&inventory, &records, &registry, |_, _, _| {})[0];
    let copy = &f.dependency_instances[0];
    assert_eq!(copy.fixed_version.as_deref(), Some("4.18.1"));
    assert_eq!(copy.clean_version.as_deref(), Some("4.18.1"));
    assert_eq!(copy.target_version.as_deref(), Some("4.18.1"));
    assert_eq!(copy.withheld, ["4.18.0"]);
    assert_eq!(f.fixed_version.as_deref(), Some("4.18.1"));

    // Without registry facts: today's answer, unchanged.
    let copy = &match_inventory(&inventory, &records)[0].dependency_instances[0];
    assert_eq!(copy.target_version.as_deref(), Some("4.18.0"));
    assert!(copy.withheld.is_empty());

    // Every clearing release withdrawn: no target, and it says why.
    registry.insert(
        ("lodash".into(), "npm".into()),
        facts(&["4.17.21", "4.18.0"], &["4.18.0"]),
    );
    let f = &match_inventory_with(&inventory, &records, &registry, |_, _, _| {})[0];
    let copy = &f.dependency_instances[0];
    assert_eq!(copy.fixed_version, None);
    assert_eq!(copy.target_version, None);
    assert_eq!(copy.clean_version, None);
    assert_eq!(copy.withheld, ["4.18.0"]);
    assert_eq!(f.fixed_version, None);
}

/// A copy no window could place still never reports a withdrawn listed fix.
#[test]
fn an_unconfirmed_finding_skips_a_withdrawn_listed_fix() {
    let inventory = vec![pkg("npm", "/p", "lodash", None, true)];
    let ranges = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"4.18.0"}]}]"#;
    let records = vec![record(
        "GHSA-1",
        "npm",
        "lodash",
        ranges,
        Some(r#"["4.18.0"]"#),
    )];
    let mut registry = RegistryIndex::new();
    registry.insert(
        ("lodash".into(), "npm".into()),
        facts(&["4.18.0", "4.18.1"], &["4.18.0"]),
    );
    let f = &match_inventory_with(&inventory, &records, &registry, |_, _, _| {})[0];
    assert!(!f.is_version_confirmed);
    assert_eq!(f.fixed_version.as_deref(), Some("4.18.1"));
}

#[test]
fn a_version_less_row_is_a_conservative_match_and_a_repeat_row_reports_once() {
    let inventory = vec![pkg("PyPI", "/p", "Typing_Extensions", None, true)];
    let records = vec![
        record(
            "PYSEC-1",
            "PyPI",
            "typing-extensions",
            BELOW_1_2_4,
            Some(r#"["1.2.4","0.9"]"#),
        ),
        record("PYSEC-1", "pypi", "typing-extensions", BELOW_1_2_4, None),
    ];
    let findings = match_inventory(&inventory, &records);
    assert_eq!(findings.len(), 1);
    assert!(!findings[0].is_version_confirmed);
    assert_eq!(findings[0].fixed_version.as_deref(), Some("1.2.4"));
}

#[test]
fn security_holding_placeholders_and_names() {
    // Any advisory id: the placeholder has no code (crossenv's GHSA-c2m4 too).
    assert!(is_npm_security_holding(
        "javascript",
        Some("0.0.1-security")
    ));
    assert!(is_npm_security_holding("npm", Some("0.0.2-security")));
    assert!(is_npm_security_holding("npm", Some("0.0.12-security")));
    assert!(!is_npm_security_holding("npm", Some("1.0.1")));
    // A publisher's own `-security` prerelease is real code, not npm's placeholder.
    assert!(!is_npm_security_holding("npm", Some("1.0.0-security")));
    assert!(!is_npm_security_holding("npm", Some("0.0.-security")));
    assert!(!is_npm_security_holding("npm", Some("0.0.1-security.2")));
    assert!(!is_npm_security_holding("PyPI", Some("0.0.1-security")));
    assert_eq!(pep503_name(" Typing__Extensions.x "), "typing-extensions-x");
    assert_eq!(package_key("Django", "pip"), "django");
    assert_eq!(package_key("Foo_Bar", "rust"), "foo_bar");
    assert_eq!(
        highest_listed_fix(&Some(r#"["15.5.24","16.3.3"]"#.into())).as_deref(),
        Some("16.3.3")
    );
    assert_eq!(highest_listed_fix(&None), None);
}
