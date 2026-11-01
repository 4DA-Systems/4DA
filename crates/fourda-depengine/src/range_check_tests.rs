// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The parts of `range_check` the shared vectors leave out: withdrawn
//! releases, held dependencies, pre-release-only ranges, undecidable
//! windows, maintenance notices and unreadable input. The vectors
//! (`tests/range_check_vectors.rs`, and the app's gate) pin the rest.

use super::*;

fn fixed(id: &str, introduced: &str, fixed: &str) -> RangeAdvisory {
    RangeAdvisory {
        id: id.to_string(),
        ranges: Some(format!(
            r#"[{{"type":"SEMVER","events":[{{"introduced":"{introduced}"}},{{"fixed":"{fixed}"}}]}}]"#
        )),
        informational: false,
    }
}

fn registry(published: &[&str], excluded: &[&str]) -> RegistryReleases {
    RegistryReleases {
        published: published.iter().map(|s| (*s).to_string()).collect(),
        excluded: excluded.iter().map(|s| (*s).to_string()).collect(),
    }
}

fn check(
    constraint: &str,
    from: Option<&str>,
    reg: &RegistryReleases,
    advisories: &[RangeAdvisory],
    held: &[String],
) -> RangeCheck {
    range_check(&RangeQuery {
        ecosystem: "npm",
        constraint,
        from,
        registry: reg,
        advisories,
        held_unsafe: held,
    })
    .unwrap_or_default()
}

#[test]
fn smallest_newest_and_upgrade() {
    let reg = registry(&["1.0.0", "1.0.1", "1.0.2", "1.1.0", "2.0.0"], &[]);
    let adv = [fixed("A", "0", "1.0.2")];
    let r = check("^1.0.0", Some("1.0.0"), &reg, &adv, &[]);
    assert_eq!(r.releases_in_range, 4);
    assert_eq!(r.vulnerable_in_range, 2);
    assert_eq!(r.smallest_safe_in_range.as_deref(), Some("1.0.2"));
    assert_eq!(r.newest_safe_in_range.as_deref(), Some("1.1.0"));
    assert_eq!(r.safe_versions, ["1.0.2", "1.1.0"]);
    assert_eq!(r.smallest_safe_upgrade, Some(Some("1.0.2".to_string())));
    assert_eq!(r.affecting_all, None);
    // Without `from`, no upgrade field at all.
    assert_eq!(
        check("^1.0.0", None, &reg, &adv, &[]).smallest_safe_upgrade,
        None
    );
    // An unreadable `from` names no upgrade.
    assert_eq!(
        check("^1.0.0", Some("nope"), &reg, &adv, &[]).smallest_safe_upgrade,
        Some(None)
    );
}

#[test]
fn nothing_safe_names_what_affects_all_and_the_newest_safe_outside() {
    let reg = registry(&["1.0.0", "1.0.1", "2.0.0", "3.0.0-rc.1"], &[]);
    let adv = [fixed("A", "0", "2.0.0"), fixed("B", "1.0.1", "2.0.0")];
    let r = check("1.x", None, &reg, &adv, &[]);
    assert_eq!(r.smallest_safe_in_range, None);
    assert_eq!(
        r.affecting_all,
        Some(vec!["A".to_string()]),
        "B misses 1.0.0"
    );
    assert_eq!(
        r.newest_safe_outside_range,
        Some(Some("2.0.0".to_string())),
        "never a pre-release"
    );
}

#[test]
fn withdrawn_releases_are_not_candidates_but_are_named_below_the_pick() {
    let reg = registry(&["1.0.0", "1.0.1", "1.0.2", "1.0.3"], &["1.0.1", "1.0.2"]);
    let adv = [fixed("A", "0", "1.0.1")];
    let r = check("~1.0.0", None, &reg, &adv, &[]);
    assert_eq!(r.releases_in_range, 2);
    assert_eq!(r.smallest_safe_in_range.as_deref(), Some("1.0.3"));
    assert_eq!(r.withheld, ["1.0.1", "1.0.2"]);
}

#[test]
fn held_dependencies_rule_releases_out() {
    let reg = registry(&["1.0.0", "1.0.1", "1.0.2"], &[]);
    let r = check("1.x", None, &reg, &[], &["1.0.2".to_string()]);
    assert_eq!(r.newest_safe_in_range.as_deref(), Some("1.0.1"));
    assert_eq!(r.held_excluded, 1);
    assert_eq!(r.vulnerable_in_range, 1);
}

#[test]
fn prereleases_count_only_when_the_range_admits_nothing_else() {
    let reg = registry(&["1.0.0", "2.0.0-rc.1", "2.0.0-rc.2"], &[]);
    let r = check(">=2.0.0-rc.1", None, &reg, &[], &[]);
    assert_eq!(r.releases_in_range, 2);
    assert_eq!(r.smallest_safe_in_range.as_deref(), Some("2.0.0-rc.1"));
    let r = check("*", None, &reg, &[], &[]);
    assert_eq!(
        r.releases_in_range, 1,
        "a stable release is in range: pre-releases are not candidates"
    );
}

#[test]
fn an_undecidable_window_is_never_clean() {
    let reg = registry(&["1.0.0", "1.0.1"], &[]);
    let bad = RangeAdvisory {
        id: "X".into(),
        ranges: Some(
            r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"not.a.version"}]}]"#.into(),
        ),
        informational: false,
    };
    let r = check("1.x", None, &reg, &[bad], &[]);
    assert_eq!(r.smallest_safe_in_range, None);
    assert_eq!(r.unconfirmed, ["X"]);
}

#[test]
fn a_maintenance_notice_never_makes_a_release_unsafe() {
    let reg = registry(&["1.0.0"], &[]);
    let notice = RangeAdvisory {
        informational: true,
        ..fixed("N", "0", "9.0.0")
    };
    let r = check("1.x", None, &reg, &[notice], &[]);
    assert_eq!(r.smallest_safe_in_range.as_deref(), Some("1.0.0"));
}

#[test]
fn an_unreadable_constraint_is_an_error_not_an_empty_range() {
    let reg = registry(&["1.0.0"], &[]);
    let q = |eco, constraint| RangeQuery {
        ecosystem: eco,
        constraint,
        from: None,
        registry: &reg,
        advisories: &[],
        held_unsafe: &[],
    };
    assert!(range_check(&q("npm", "latest")).is_err());
    assert!(range_check(&q("PyPI", "~=3")).is_err());
    assert!(range_check(&q("Maven", "[1,2)")).is_err());
    // A readable constraint that admits nothing is an empty range.
    let r = range_check(&q("npm", "9.x")).unwrap_or_default();
    assert_eq!((r.range.as_str(), r.releases_in_range), ("9.x", 0));
}

#[test]
fn go_pseudo_versions_are_prereleases() {
    assert!(is_pseudo_version("v0.0.0-20190308221718-c2843e01d9a2"));
    assert!(is_pseudo_version("v1.2.4-0.20190308221718-c2843e01d9a2"));
    assert!(!is_pseudo_version("v1.2.3"));
}

#[test]
fn package_advisories_select_one_package() {
    let row = |name: &str, eco: &str| AdvisoryRecord {
        advisory_id: format!("{eco}:{name}"),
        package_name: name.to_string(),
        ecosystem: eco.to_string(),
        affected_ranges: None,
        fixed_versions: None,
        malware: false,
        informational: false,
    };
    let rows = [
        row("Jinja2", "PyPI"),
        row("jinja2", "npm"),
        row("markupsafe", "PyPI"),
    ];
    let got = package_advisories(&rows, "pypi", "jinja2");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].id, "PyPI:Jinja2");
}
