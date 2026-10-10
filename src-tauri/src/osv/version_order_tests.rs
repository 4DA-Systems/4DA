// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use std::cmp::Ordering;

use super::*;

fn v(s: &str) -> OrderedVersion {
    OrderedVersion::parse(s).unwrap_or_else(|| panic!("{s} parses"))
}

fn cmp(a: &str, b: &str) -> Option<Ordering> {
    v(a).compare(&v(b))
}

#[test]
fn pep440_orders_a_release_line_like_pip() {
    let ordered = [
        "1.0.dev1",
        "1.0a1.dev1",
        "1.0a1",
        "1.0a1.post1",
        "1.0b2",
        "1.0rc1",
        "1.0",
        "1.0.post1.dev1",
        "1.0.post1",
        "1.0.1",
        "1!0.1",
    ];
    for pair in ordered.windows(2) {
        assert_eq!(cmp(pair[0], pair[1]), Some(Ordering::Less), "{pair:?}");
    }
}

#[test]
fn pep440_four_part_and_trailing_zeros() {
    assert_eq!(cmp("1.2.3.4", "1.2.4"), Some(Ordering::Less));
    assert_eq!(cmp("2.10", "2.10.0"), Some(Ordering::Equal));
    assert_eq!(cmp("2.9.1", "2.10"), Some(Ordering::Less));
    assert_eq!(
        cmp("1.0-1", "1.0"),
        Some(Ordering::Greater),
        "-N is a post release"
    );
}

#[test]
fn build_metadata_is_not_precedence() {
    assert_eq!(cmp("2.0.0+incompatible", "2.0.0"), Some(Ordering::Equal));
    assert_eq!(cmp("2.1.0+incompatible", "2.0.0"), Some(Ordering::Greater));
}

#[test]
fn go_pseudo_versions_order_by_timestamp_and_below_the_release() {
    assert_eq!(
        cmp(
            "0.0.0-20190308221718-c2843e01d9a2",
            "0.0.0-20220314234659-1baeb1ce4c0b"
        ),
        Some(Ordering::Less)
    );
    assert_eq!(
        cmp("0.0.0-20190308221718-c2843e01d9a2", "0.1.0"),
        Some(Ordering::Less)
    );
    assert_eq!(
        cmp("1.2.4-0.20200101000000-abcdefabcdef", "1.2.4"),
        Some(Ordering::Less),
        "a pseudo-version after v1.2.3 is a pre-release of v1.2.4"
    );
}

#[test]
fn introduced_zero_is_below_every_version_including_pseudo_versions() {
    let user = v("0.0.0-20190308221718-c2843e01d9a2");
    let fixed = v("0.0.0-20220314234659-1baeb1ce4c0b");
    let window = in_window(
        &user,
        &Lower::Unbounded,
        Some(Upper {
            version: &fixed,
            inclusive: false,
        }),
    );
    assert_eq!(window, Some(true));
    assert!(matches!(Lower::parse("0"), Some(Lower::Unbounded)));
}

#[test]
fn an_unreadable_pair_is_undecided_not_outside() {
    assert!(OrderedVersion::parse("banana").is_none());
    // semver-only vs PEP-440-only: no common scheme.
    let semver_only = v("1.0.0-alpha.beta");
    let pep_only = v("1.0.post1");
    assert_eq!(semver_only.compare(&pep_only), None);
}
