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
    assert!(matches!(Lower::parse_in("0", ""), Some(Lower::Unbounded)));
}

#[test]
fn an_unreadable_pair_is_undecided_not_outside() {
    assert!(OrderedVersion::parse("banana").is_none());
    // semver-only vs PEP-440-only: no common scheme.
    let semver_only = v("1.0.0-alpha.beta");
    let pep_only = v("1.0.post1");
    assert_eq!(semver_only.compare(&pep_only), None);
}

fn cmp_in(eco: &str, a: &str, b: &str) -> Option<Ordering> {
    OrderedVersion::parse_in(a, eco)
        .zip(OrderedVersion::parse_in(b, eco))
        .and_then(|(x, y)| x.compare(&y))
}

/// Rule vectors vo-042..044: a PyPI pair that also parses as semver still
/// orders by PEP 440.
#[test]
fn pypi_orders_by_pep440_even_when_both_sides_parse_as_semver() {
    for eco in ["PyPI", "pypi", "pip"] {
        assert_eq!(
            cmp_in(eco, "1.0.0-1", "1.0.0"),
            Some(Ordering::Greater),
            "{eco}"
        );
        assert_eq!(cmp_in(eco, "1.0.0-post1", "1.0.0"), Some(Ordering::Greater));
        assert_eq!(
            cmp_in(eco, "1.0.0-a10", "1.0.0-a9"),
            Some(Ordering::Greater)
        );
        assert_eq!(cmp_in(eco, "1.0.0-dev", "1.0.0"), Some(Ordering::Less));
        assert_eq!(cmp_in(eco, "2.0.0-rc.1", "2.0.0rc1"), Some(Ordering::Equal));
    }
}

/// Every other ecosystem keeps semver: `1.0.0-1` is a pre-release there, and
/// numeric identifiers compare numerically.
#[test]
fn other_ecosystems_keep_semver_order() {
    for eco in ["npm", "crates.io", "Go", ""] {
        assert_eq!(
            cmp_in(eco, "1.0.0-1", "1.0.0"),
            Some(Ordering::Less),
            "{eco}"
        );
        assert_eq!(
            cmp_in(eco, "1.0.0-alpha.10", "1.0.0-alpha.9"),
            Some(Ordering::Greater)
        );
        assert_eq!(
            cmp_in(eco, "2.0.0+incompatible", "2.0.0"),
            Some(Ordering::Equal)
        );
    }
}

/// Rule vectors vo-013..015: PEP 440's local-label order.
#[test]
fn pep440_local_labels_order() {
    let ordered = [
        "1.0",
        "1.0+abc.5",
        "1.0+abc.7",
        "1.0+5",
        "1.0.post456.dev34",
    ];
    for pair in ordered.windows(2) {
        assert_eq!(
            cmp_in("PyPI", pair[0], pair[1]),
            Some(Ordering::Less),
            "{pair:?}"
        );
    }
    assert_eq!(
        cmp_in("PyPI", "2.3.0+CPU", "2.3.0+cpu"),
        Some(Ordering::Equal)
    );
    assert_eq!(cmp_in("PyPI", "1.0+abc", "1.0+abc.1"), Some(Ordering::Less));
}

/// A PyPI-read version against a generically parsed bound orders by PEP 440.
#[test]
fn a_pypi_side_decides_the_scheme() {
    let user = OrderedVersion::parse_in("1.0.0-1", "PyPI").expect("parses");
    let bound = v("1.0.0");
    assert_eq!(user.compare(&bound), Some(Ordering::Greater));
    assert!(!user.is_prerelease());
    assert!(OrderedVersion::parse_in("1.1.0rc1", "PyPI").is_some_and(|x| x.is_prerelease()));
    assert!(OrderedVersion::parse_in("1.1.0-rc.1", "npm").is_some_and(|x| x.is_prerelease()));
}
