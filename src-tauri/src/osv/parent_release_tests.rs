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

fn adv(id: &str, ranges: Option<String>) -> Advisory {
    Advisory::new(id, &[], &[], "", ranges)
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
        list: vec![
            adv("GHSA-37ch", range("0", "0.1.13")),
            adv("GHSA-27v5", range("8.0.0", "8.4.0")),
        ],
    };
    a.usable(v, Some("8.4.2"), None)
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
    // Parity decision 3: the only fix across a major raises the row too
    // (flagged breaking where it is worded, `osv::raised_lines`).
    assert_eq!(
        row_target(Some("2.1.20"), &["3.1.2"], &none, "npm").as_deref(),
        Some("3.1.2")
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
    // Parity decision 3: across a major too (express 5.0.0 no longer depends
    // on path-to-regexp at all).
    let major = lift(
        &found,
        Some("5.0.0"),
        &child,
        &express_ptr(),
        &|_| true,
        Dialect::Npm,
    )
    .expect("lifts across the major");
    assert_eq!((major.version.as_str(), major.requirement), ("5.0.0", None));
}

/// Parity decision 2: a release affected by an advisory the installed
/// version does not have is never named, fixed or not; one the installed
/// version already has and no release fixes does not block it.
#[test]
fn a_release_never_introduces_an_advisory() {
    let own = Advisories {
        ecosystem: "npm",
        list: vec![
            adv("GHSA-new", range("1.1.0", "1.2.0")),
            adv(
                "GHSA-old-unfixed",
                Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"}]}]"#.into()),
            ),
        ],
    };
    let newest = Some("1.2.0");
    assert!(
        !own.usable("1.1.0", newest, Some("1.0.0")),
        "brings in GHSA-new"
    );
    assert!(
        own.usable("1.2.0", newest, Some("1.0.0")),
        "keeps only what 1.0.0 has"
    );
    assert_eq!(own.introduced_at("1.1.0", "1.0.0"), ["GHSA-new"]);
    assert_eq!(own.unfixed_at("1.2.0", newest), ["GHSA-old-unfixed"]);
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
            unknown: None,
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
            unknown: None,
        },
    ];
    let own = Advisories {
        ecosystem: "npm",
        list: vec![adv("GHSA-qw6h", range("0", "4.20.0"))],
    };
    let d = decide(
        &express(),
        "4.16.4",
        Some("4.20.0"),
        &own,
        &rows,
        &[],
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
    let d = decide(&express(), "4.16.4", None, &own, &rows, &[], Dialect::Npm);
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

/// Review of #981, B2: the one predicate. `par` 1.0.0 pins leaf 1.0.0
/// (fixed 1.1.0); par 1.1.0 still pins it; par 2.0.0 requires ^1.1.0.
fn par_case(par_advisories: Vec<Advisory>) -> Decision {
    let leaf = v(&["1.0.0", "1.1.0"]);
    let leaf_clean = |ver: &str| ver != "1.0.0";
    let history = hist(&[
        ("1.0.0", Some("1.0.0")),
        ("1.1.0", Some("1.0.0")),
        ("2.0.0", Some("^1.1.0")),
    ]);
    let par = Releases {
        versions: v(&["1.0.0", "1.1.0", "2.0.0"]),
        latest: Some("2.0.0".into()),
        deprecated: false,
    };
    let rows = [RowIn {
        child: Child {
            ecosystem: "npm",
            installed: "1.0.0",
            target: "1.1.0",
            releases: &leaf,
            clean: &leaf_clean,
        },
        history: Some(&history),
        unknown: None,
    }];
    let own = Advisories {
        ecosystem: "npm",
        list: par_advisories,
    };
    decide(&par, "1.0.0", None, &own, &rows, &[], Dialect::Npm)
}

#[test]
fn a_cwe_506_record_with_no_mal_alias_is_malware_too() {
    // crossenv GHSA-c2m4-w5hm-vqjw as OSV serves it (2026-10-18).
    let crossenv = Advisory::new(
        "GHSA-c2m4-w5hm-vqjw",
        &["CVE-2017-16074".to_string()],
        &["CWE-506".to_string()],
        "crossenv is malware",
        Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"}]}]"#.into()),
    );
    assert!(crossenv.malware);
    // An installed malicious parent is removed or replaced, never moved, and
    // never "remains affected (no fix published)".
    let d = par_case(vec![crossenv]);
    assert_eq!(
        d.rows[0].standing,
        Standing::Malicious("GHSA-c2m4-w5hm-vqjw".into())
    );
    assert_eq!(d.rows[0].to, None);
}

#[test]
fn a_malicious_release_is_never_named_nor_the_newest() {
    let mal = || vec![adv("MAL-2026-0001", range("2.0.0", "2.0.1"))];
    let d = par_case(mal());
    assert_eq!(
        d.rows[0].standing,
        Standing::NoneAdmits {
            latest: Some("2.0.0".into())
        },
        "par 2.0.0 is malicious: never Admits"
    );
    let own = Advisories {
        ecosystem: "npm",
        list: mal(),
    };
    let par = Releases {
        versions: v(&["1.0.0", "1.1.0", "2.0.0"]),
        ..Releases::default()
    };
    assert_eq!(own.newest(&par).as_deref(), Some("1.1.0"));
    assert!(!own.usable("2.0.0", Some("1.1.0"), None));
    // An alias counts too.
    let aliased = Advisory::new(
        "GHSA-xxxx",
        &["MAL-2026-0002".to_string()],
        &[],
        "",
        range("0", "9.0.0"),
    );
    assert!(aliased.malware);
    // An installed malicious parent is removed or replaced, never moved.
    let d = par_case(vec![adv("MAL-2026-0003", range("1.0.0", "1.0.1"))]);
    assert_eq!(
        d.rows[0].standing,
        Standing::Malicious("MAL-2026-0003".into())
    );
}

#[test]
fn an_advisory_whose_ranges_cannot_be_read_names_nothing() {
    for ranges in [None, Some("[]".to_string()), Some("not json".to_string())] {
        let d = par_case(vec![adv("GHSA-norange", ranges)]);
        assert!(
            matches!(&d.rows[0].standing, Standing::Undecided(why) if why.contains("GHSA-norange")),
            "{:?}",
            d.rows[0].standing
        );
        assert_eq!(d.rows[0].to, None);
    }
}

#[test]
fn an_unfixed_advisory_does_not_block_the_move_and_is_reported() {
    // CVE-unfixed affects every par release (no fix published).
    let unfixed = || {
        vec![adv(
            "CVE-2026-0001",
            Some(r#"[{"type":"SEMVER","events":[{"introduced":"0"}]}]"#.into()),
        )]
    };
    let d = par_case(unfixed());
    assert_eq!(
        d.rows[0].to.as_ref().map(|f| f.version.as_str()),
        Some("2.0.0")
    );
    let own = Advisories {
        ecosystem: "npm",
        list: unfixed(),
    };
    assert_eq!(own.unfixed_at("2.0.0", Some("2.0.0")), ["CVE-2026-0001"]);
}
