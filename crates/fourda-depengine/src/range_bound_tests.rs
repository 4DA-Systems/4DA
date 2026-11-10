// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Range bounds that are not semver, read the way OSV.dev reads them
//! ([`super::coerce_bound`]), on the cases the MCP server tests
//! (`osv-range-bounds.test.ts`, W1a): one reading on every surface (Y4).
//! GHSA-92cp-5422-2mw7 and GO-2025-3540 as api.osv.dev served them on
//! 2026-11-10 (ranges only).

use super::{check_version_affected_uncached, coerce_bound};

#[test]
fn a_bare_prerelease_suffix_and_missing_parts() {
    assert_eq!(coerce_bound("9.6.0b1", "Go"), "9.6.0-b1");
    assert_eq!(coerce_bound("v1.2.0rc1", "Go"), "v1.2.0-rc1");
    assert_eq!(coerce_bound("3", "crates.io"), "3.0.0");
    // Already readable (two parts pad): left as written.
    assert_eq!(coerce_bound("1.2", "npm"), "1.2");
    assert_eq!(coerce_bound("1.2.3beta.2", "npm"), "1.2.3-beta.2");
}

#[test]
fn valid_bounds_other_ecosystems_and_unparseable_text_are_left_alone() {
    for v in ["0", "9.6.0", "9.7.0-beta.1", "v0.23.0", "1.0.0+build.1"] {
        assert_eq!(coerce_bound(v, "Go"), v);
    }
    assert_eq!(coerce_bound("2.0b1", "PyPI"), "2.0b1");
    assert_eq!(coerce_bound("deadbeef", "npm"), "deadbeef");
    assert_eq!(coerce_bound("", "npm"), "");
}

const GHSA: &str = r#"[
  {"type":"SEMVER","events":[{"introduced":"9.7.0-beta.1"},{"fixed":"9.7.3"}]},
  {"type":"ECOSYSTEM","events":[{"introduced":"9.6.0b1"},{"fixed":"9.6.3"}]},
  {"type":"SEMVER","events":[{"introduced":"9.5.1"},{"fixed":"9.5.5"}]}
]"#;
const GO: &str = r#"[
  {"type":"SEMVER","events":[{"introduced":"9.5.1"},{"fixed":"9.5.5"}]},
  {"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"9.6.3"}]},
  {"type":"SEMVER","events":[{"introduced":"9.7.0-beta.1"},{"fixed":"9.7.3"}]}
]"#;

/// api.osv.dev /v1/query per version for the GHSA record, 2026-11-10.
#[test]
fn the_go_redis_record_affects_exactly_what_osv_says() {
    let ranges = Some(GHSA.to_string());
    for (v, affected) in [
        ("9.5.0", false),
        ("9.5.1", true),
        ("9.5.4", true),
        ("9.5.5", false),
        ("9.6.0", true),
        ("9.6.1", true),
        ("9.6.2", true),
        ("9.6.3", false),
        ("9.7.0", true),
        ("9.7.3", false),
        ("v9.6.3", false),
    ] {
        let (is_affected, confirmed) = check_version_affected_uncached("Go", Some(v), &ranges);
        assert!(confirmed, "{v} is decided");
        assert_eq!(is_affected, affected, "{v}");
    }
}

/// From v9.5.1 the walk over both records lands on v9.6.3, clear of both
/// (9.5.5 is still inside GO-2025-3540's 0..9.6.3).
#[test]
fn the_fix_target_from_v9_5_1_is_v9_6_3() {
    let ghsa = Some(GHSA.to_string());
    let go = Some(GO.to_string());
    let walk = crate::fix_target::walk_target("Go", "v9.5.1", &[&ghsa, &go]).expect("a target");
    assert_eq!(walk.version.trim_start_matches('v'), "9.6.3");
    assert!(walk.clears_all);
}
