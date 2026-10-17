// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The pure parent-release rules. Registry shapes are the real ones
//! (registry.npmjs.org abbreviated documents, 2026-10-18), cut down; the
//! shared vectors (`parent_release.jsonl`) carry the full live data.

use super::*;

fn v(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn hist(pairs: &[(&str, Option<&str>)]) -> RequirementHistory {
    pairs
        .iter()
        .map(|(ver, req)| {
            (
                ver.to_string(),
                ReleaseReq {
                    reqs: req.iter().map(|r| r.to_string()).collect(),
                    peer: false,
                },
            )
        })
        .collect()
}

fn range(intro: &str, fixed: &str) -> Option<String> {
    Some(format!(
        r#"[{{"type":"SEMVER","events":[{{"introduced":"{intro}"}},{{"fixed":"{fixed}"}}]}}]"#
    ))
}

const PTR: &[&str] = &["0.1.7", "0.1.10", "0.1.12", "0.1.13", "8.4.2"];

fn ptr_clean(v: &str) -> bool {
    let a = Advisories {
        ecosystem: "npm",
        ranges: vec![range("0", "0.1.13"), range("8.0.0", "8.4.0")],
    };
    a.clean_of_fixable(v, Some("8.4.2"))
}

fn express() -> Releases {
    Releases {
        versions: v(&[
            "4.16.4", "4.19.2", "4.20.0", "4.21.2", "4.22.0", "4.22.3", "5.0.0",
        ]),
        latest: Some("5.3.0".into()),
        deprecated: false,
    }
}

fn express_ptr() -> RequirementHistory {
    hist(&[
        ("4.16.4", Some("0.1.7")),
        ("4.19.2", Some("0.1.7")),
        ("4.20.0", Some("0.1.10")),
        ("4.21.2", Some("0.1.12")),
        ("4.22.0", Some("~0.1.12")),
        ("4.22.3", Some("~0.1.13")),
        ("5.0.0", None),
    ])
}

fn ptr_child<'a>(
    installed: &'a str,
    releases: &'a [String],
    clean: &'a dyn Fn(&str) -> bool,
) -> Child<'a> {
    Child {
        ecosystem: "npm",
        installed,
        target: "0.1.13",
        releases,
        clean,
    }
}

#[test]
fn the_smallest_release_whose_requirement_resolves_clean_is_named() {
    let releases = v(PTR);
    let child = ptr_child("0.1.7", &releases, &ptr_clean);
    let found = smallest_admitting(
        &child,
        &express(),
        "4.16.4",
        &express_ptr(),
        &|_| true,
        Dialect::Npm,
    )
    .expect("express 4.22.0");
    assert_eq!(found.version, "4.22.0");
    assert_eq!(found.requirement.as_deref(), Some("~0.1.12"));
    assert_eq!(found.resolves_to.as_deref(), Some("0.1.13"));
    assert!(!found.keeps_installed);
}

#[test]
fn an_install_that_keeps_the_vulnerable_copy_is_flagged() {
    // npm 10.9.2: express 4.21.2 -> 4.22.0 kept path-to-regexp 0.1.12 under ~0.1.12.
    let releases = v(PTR);
    let child = ptr_child("0.1.12", &releases, &ptr_clean);
    let found = smallest_admitting(
        &child,
        &express(),
        "4.21.2",
        &express_ptr(),
        &|_| true,
        Dialect::Npm,
    )
    .expect("express 4.22.0");
    assert_eq!(found.version, "4.22.0");
    assert!(found.keeps_installed);
}

#[test]
fn a_release_affected_by_the_parents_own_advisory_is_skipped() {
    let releases = v(PTR);
    let child = ptr_child("0.1.7", &releases, &ptr_clean);
    let own = |ver: &str| ver != "4.22.0";
    let found = smallest_admitting(
        &child,
        &express(),
        "4.16.4",
        &express_ptr(),
        &own,
        Dialect::Npm,
    )
    .expect("express 4.22.3");
    assert_eq!(found.version, "4.22.3");
}

#[test]
fn without_the_packages_releases_the_requirement_must_admit_the_target() {
    let child = ptr_child("0.1.7", &[], &ptr_clean);
    let found = smallest_admitting(
        &child,
        &express(),
        "4.16.4",
        &express_ptr(),
        &|_| true,
        Dialect::Npm,
    )
    .expect("express 4.22.0");
    assert_eq!(found.resolves_to, None);
    assert_eq!(found.version, "4.22.0");
}

#[test]
fn a_release_that_drops_the_package_or_takes_it_as_a_peer_admits_it() {
    let releases = v(&["1.0.0", "1.1.0"]);
    let clean = |ver: &str| ver != "1.0.0";
    let child = Child {
        ecosystem: "npm",
        installed: "1.0.0",
        target: "1.1.0",
        releases: &releases,
        clean: &clean,
    };
    let par = Releases {
        versions: v(&["1.0.0", "1.1.0"]),
        latest: Some("1.1.0".into()),
        deprecated: false,
    };
    let mut h = hist(&[("1.0.0", Some("1.0.0")), ("1.1.0", None)]);
    let gone =
        smallest_admitting(&child, &par, "1.0.0", &h, &|_| true, Dialect::Npm).expect("drops it");
    assert_eq!((gone.version.as_str(), gone.requirement), ("1.1.0", None));
    h.insert(
        "1.1.0".into(),
        ReleaseReq {
            reqs: v(&["^1.1.0"]),
            peer: true,
        },
    );
    let peer =
        smallest_admitting(&child, &par, "1.0.0", &h, &|_| true, Dialect::Npm).expect("peer");
    assert!(peer.peer);
    assert_eq!(peer.requirement.as_deref(), Some("^1.1.0"));
}

#[test]
fn standing_says_why_no_release_is_named() {
    let releases = v(PTR);
    let child = ptr_child("0.1.7", &releases, &ptr_clean);
    let mut retired = express();
    retired.deprecated = true;
    assert_eq!(
        standing(&child, &retired, "4.16.4", None, &|_| true, Dialect::Npm),
        Standing::Deprecated
    );
    assert_eq!(
        standing(&child, &express(), "5.3.0", None, &|_| true, Dialect::Npm),
        Standing::Latest
    );
    assert_eq!(
        standing(&child, &express(), "4.16.4", None, &|_| true, Dialect::Npm),
        Standing::Unread
    );
    let pinned = hist(&[
        ("4.22.0", Some("0.1.7")),
        ("4.22.3", Some("0.1.7")),
        ("5.0.0", Some("0.1.7")),
    ]);
    assert_eq!(
        standing(
            &child,
            &express(),
            "4.16.4",
            Some(&pinned),
            &|_| true,
            Dialect::Npm
        ),
        Standing::NoneAdmits {
            latest: Some("5.3.0".into())
        }
    );
}

#[test]
fn the_own_row_rises_within_its_major_and_only_to_a_clean_release() {
    let none = |_: &str| false;
    assert_eq!(
        row_target(Some("4.20.0"), &["4.22.0", "4.22.3"], &none, "npm").as_deref(),
        Some("4.22.3")
    );
    assert_eq!(
        row_target(Some("4.20.0"), &["4.19.0"], &none, "npm").as_deref(),
        Some("4.20.0")
    );
    // A major the own target does not cross stays a row of its own.
    assert_eq!(
        row_target(Some("2.1.20"), &["3.1.2"], &none, "npm").as_deref(),
        Some("2.1.20")
    );
    // An advisory of the parent affects the higher release.
    assert_eq!(
        row_target(Some("4.20.0"), &["4.22.3"], &|v| v == "4.22.3", "npm").as_deref(),
        Some("4.20.0")
    );
    assert_eq!(row_target(None, &["4.22.3"], &none, "npm"), None);
}

#[test]
fn a_lower_release_lifts_to_the_own_row_target_when_that_still_admits_it() {
    let releases = v(PTR);
    let child = ptr_child("0.1.7", &releases, &ptr_clean);
    let found = smallest_admitting(
        &child,
        &express(),
        "4.16.4",
        &express_ptr(),
        &|_| true,
        Dialect::Npm,
    )
    .expect("4.22.0");
    let lifted = lift(
        &found,
        Some("4.22.3"),
        &child,
        &express_ptr(),
        &|_| true,
        Dialect::Npm,
    )
    .expect("4.22.3");
    assert_eq!(lifted.version, "4.22.3");
    assert_eq!(lifted.requirement.as_deref(), Some("~0.1.13"));
    assert_eq!(
        lift(
            &found,
            Some("4.22.0"),
            &child,
            &express_ptr(),
            &|_| true,
            Dialect::Npm
        ),
        None
    );
    assert_eq!(
        lift(
            &found,
            Some("5.0.0"),
            &child,
            &express_ptr(),
            &|_| true,
            Dialect::Npm
        ),
        None
    );
}

#[test]
fn decide_agrees_every_row_with_the_parents_own_row() {
    // NodeGoat, cut down: path-to-regexp needs express 4.22.0, qs needs 4.22.3.
    let ptr_releases = v(PTR);
    let qs_releases = v(&["6.5.2", "6.14.2", "6.15.3", "6.16.0", "6.16.1"]);
    let qs_clean = |ver: &str| matches!(ver, "6.16.0" | "6.16.1");
    let qs_hist = hist(&[
        ("4.16.4", Some("6.5.2")),
        ("4.19.2", Some("6.11.0")),
        ("4.20.0", Some("6.11.0")),
        ("4.21.2", Some("6.13.0")),
        ("4.22.0", Some("~6.14.0")),
        ("4.22.3", Some("~6.16.0")),
        ("5.0.0", Some("6.13.0")),
    ]);
    let ptr_hist = express_ptr();
    let rows = [
        RowIn {
            child: ptr_child("0.1.7", &ptr_releases, &ptr_clean),
            history: Some(&ptr_hist),
        },
        RowIn {
            child: Child {
                ecosystem: "npm",
                installed: "6.5.2",
                target: "6.16.0",
                releases: &qs_releases,
                clean: &qs_clean,
            },
            history: Some(&qs_hist),
        },
    ];
    let own = Advisories {
        ecosystem: "npm",
        ranges: vec![range("0", "4.20.0")],
    };
    let d = decide(
        &express(),
        "4.16.4",
        Some("4.20.0"),
        &own,
        &rows,
        Dialect::Npm,
    );
    assert_eq!(d.row_target.as_deref(), Some("4.22.3"));
    let named: Vec<(Option<&str>, Option<&str>)> = d
        .rows
        .iter()
        .map(|r| {
            (
                r.smallest().map(|f| f.version.as_str()),
                r.to.as_ref().map(|f| f.version.as_str()),
            )
        })
        .collect();
    assert_eq!(
        named,
        [
            (Some("4.22.0"), Some("4.22.3")),
            (Some("4.22.3"), Some("4.22.3"))
        ]
    );
    // No own row: each row keeps its own smallest release.
    let d = decide(&express(), "4.16.4", None, &own, &rows, Dialect::Npm);
    assert_eq!(d.row_target, None);
    assert_eq!(
        d.rows[0].to.as_ref().map(|f| f.version.as_str()),
        Some("4.22.0")
    );
}

#[test]
fn a_lever_names_the_release_that_brings_in_a_fixed_parent() {
    // NodeGoat: send is pinned by serve-static, which arrives through express.
    let send_releases = v(&["0.16.2", "0.19.0", "0.19.2"]);
    let send_clean = |ver: &str| ver != "0.16.2";
    let child = Child {
        ecosystem: "npm",
        installed: "0.16.2",
        target: "0.19.0",
        releases: &send_releases,
        clean: &send_clean,
    };
    let serve_static = Releases {
        versions: v(&["1.13.2", "1.16.0", "1.16.2"]),
        latest: Some("2.2.1".into()),
        deprecated: false,
    };
    let ss_send = hist(&[
        ("1.13.2", Some("0.16.2")),
        ("1.16.0", Some("0.19.0")),
        ("1.16.2", Some("0.19.0")),
    ]);
    let parent = HopParent {
        installed: "1.13.2",
        releases: &serve_static,
        history: &ss_send,
        own_clean: &|_| true,
    };
    let ex_ss = hist(&[
        ("4.16.4", Some("1.13.2")),
        ("4.20.0", Some("1.16.0")),
        ("4.21.0", Some("1.16.2")),
        ("4.22.3", Some("~1.16.2")),
    ]);
    let lever = Releases {
        versions: v(&["4.16.4", "4.20.0", "4.21.0", "4.22.3"]),
        latest: Some("5.3.0".into()),
        deprecated: false,
    };
    let hop = through_release(
        &child,
        &parent,
        &lever,
        "4.16.4",
        &ex_ss,
        &|ver| ver != "4.20.0",
        Dialect::Npm,
    );
    assert_eq!(
        hop,
        Hop::Found {
            version: "4.21.0".into(),
            brings: "1.16.2".into(),
            keeps_parent: false
        }
    );
    let mut retired = lever.clone();
    retired.deprecated = true;
    assert_eq!(
        through_release(
            &child,
            &parent,
            &retired,
            "4.16.4",
            &ex_ss,
            &|_| true,
            Dialect::Npm
        ),
        Hop::Deprecated
    );
}
