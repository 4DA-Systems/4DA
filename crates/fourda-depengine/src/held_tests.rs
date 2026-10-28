// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The held-dependency rule (audit 6.2.5 D-4), on the shapes
//! `@4da/mcp-server`'s `held-dependencies.test.ts` uses: NodeGoat's express
//! 4.16.4, release lists cut down to the ones that decide it.

use std::collections::{BTreeMap, HashMap};

use serde_json::json;

use super::*;

#[test]
fn exact_pins_and_narrow_ranges() {
    assert_eq!(exact_pin_version("6.11.0").as_deref(), Some("6.11.0"));
    assert_eq!(exact_pin_version("=6.11.0").as_deref(), Some("6.11.0"));
    assert_eq!(exact_pin_version("= 6.11.0").as_deref(), Some("6.11.0"));
    assert_eq!(
        exact_pin_version("v1.2.3-beta.1").as_deref(),
        Some("1.2.3-beta.1")
    );
    assert_eq!(exact_pin_version("1.2.3+build.5").as_deref(), Some("1.2.3"));
    for spec in [
        "^6.11.0",
        "~6.11.0",
        "6.x",
        ">=6",
        "*",
        "latest",
        "npm:qs@6.11.0",
        "git+https://x/y.git",
    ] {
        assert_eq!(exact_pin_version(spec), None, "{spec}");
    }
    for spec in [
        "~6.14.0",
        "^0.1.12",
        "0.1.x",
        "1.2",
        "1.2.*",
        ">=1.2.0 <1.3.0",
        "1.2.0 - 1.4.0",
    ] {
        assert!(is_narrow_range(spec), "{spec}");
    }
    for spec in [
        "^6.14.0",
        ">=1",
        "*",
        "latest",
        "6.11.0",
        "1.x || 2.x",
        "file:../x",
        "npm:qs@~6.1.0",
        "x/y",
        "1.x",
    ] {
        assert!(!is_narrow_range(spec), "{spec}");
    }
}

#[test]
fn held_specs_read_dependencies_and_optional_only() {
    let deps = json!({"qs": "6.11.0", "send": "~0.19.0", "debug": "^2.6.9", "cookie": "0.6.0"});
    let optional = json!({"fsevents": "1.2.9", "cookie": "^1.0.0"});
    let held = held_specs_of(Some(&deps), Some(&optional));
    let want: BTreeMap<String, String> =
        [("qs", "6.11.0"), ("send", "~0.19.0"), ("fsevents", "1.2.9")]
            .into_iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
    // cookie: the optional spec (a wide caret) wins, so it is not held. Names
    // come in name order (the MCP keeps the document's order).
    assert_eq!(held, want);
}

#[test]
fn a_narrow_range_is_judged_at_latest_else_its_highest_release() {
    let facts = HeldFacts {
        published: ["6.14.0", "6.14.2", "6.14.3-rc.1", "6.15.3", "6.16.0"]
            .map(str::to_string)
            .to_vec(),
        latest: Some("6.16.0".into()),
    };
    assert_eq!(
        judged_version("~6.16.0", Some(&facts)).as_deref(),
        Some("6.16.0")
    );
    assert_eq!(
        judged_version("~6.14.0", Some(&facts)).as_deref(),
        Some("6.14.2")
    );
    assert_eq!(
        judged_version("~6.14.3-rc.0", Some(&facts)).as_deref(),
        Some("6.14.3-rc.1")
    );
    assert_eq!(judged_version("~6.17.0", Some(&facts)), None);
    assert_eq!(
        judged_version("~6.14.0", None),
        None,
        "no facts: cannot be said"
    );
    assert_eq!(judged_version("=6.11.0", None).as_deref(), Some("6.11.0"));
}

#[test]
fn the_caret_line() {
    assert!(same_npm_line("4.20.0", "4.22.3"));
    assert!(!same_npm_line("4.20.0", "5.0.0"));
    assert!(same_npm_line("0.1.10", "0.1.13"));
    assert!(!same_npm_line("0.1.10", "0.2.0"));
}

/// The cut-down world of the MCP test: what each express release holds, the
/// registry facts and advisories of what it holds.
struct World {
    facts: HashMap<&'static str, HeldFacts>,
    advisories: HashMap<&'static str, Vec<(&'static str, Option<String>)>>,
    down: bool,
}

fn window(fixed: &str) -> Option<String> {
    Some(json!([{"type": "SEMVER", "events": [{"introduced": "0"}, {"fixed": fixed}]}]).to_string())
}

fn world() -> World {
    let facts = |published: &[&str], latest: &str| HeldFacts {
        published: published.iter().map(|s| s.to_string()).collect(),
        latest: Some(latest.into()),
    };
    World {
        facts: HashMap::from([
            (
                "qs",
                facts(
                    &[
                        "6.5.2", "6.11.0", "6.13.0", "6.14.0", "6.14.2", "6.15.3", "6.16.0",
                    ],
                    "6.16.0",
                ),
            ),
            (
                "path-to-regexp",
                facts(&["0.1.7", "0.1.10", "0.1.12", "0.1.13", "8.0.0"], "8.0.0"),
            ),
            ("cookie", facts(&["0.3.1", "0.6.0", "0.7.1"], "0.7.1")),
            ("debug", facts(&["2.6.9", "4.3.6"], "4.3.6")),
        ]),
        advisories: HashMap::from([
            ("qs", vec![("GHSA-4mjr-xmp4-gh2g", window("6.16.0"))]),
            (
                "path-to-regexp",
                vec![("GHSA-37ch-88jc-xwx2", window("0.1.13"))],
            ),
            ("cookie", vec![]),
            ("debug", vec![]),
        ]),
        down: false,
    }
}

impl HeldSources for World {
    fn facts(&self, name: &str) -> Option<HeldFacts> {
        self.facts.get(name).cloned()
    }

    fn affected(&self, name: &str, version: &str) -> Option<Vec<String>> {
        if self.down {
            return None;
        }
        let records = self.advisories.get(name)?;
        Some(
            records
                .iter()
                .filter(|(_, r)| check_version_affected("npm", Some(version), r).0)
                .map(|(id, _)| (*id).to_string())
                .collect(),
        )
    }
}

fn specs(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

fn express() -> BTreeMap<&'static str, BTreeMap<String, String>> {
    BTreeMap::from([
        (
            "4.20.0",
            specs(&[
                ("qs", "6.11.0"),
                ("path-to-regexp", "0.1.10"),
                ("cookie", "0.6.0"),
                ("debug", "2.6.9"),
            ]),
        ),
        (
            "4.21.2",
            specs(&[
                ("qs", "6.13.0"),
                ("path-to-regexp", "0.1.12"),
                ("cookie", "0.7.1"),
                ("debug", "2.6.9"),
            ]),
        ),
        (
            "4.22.0",
            specs(&[
                ("qs", "~6.14.0"),
                ("path-to-regexp", "~0.1.12"),
                ("cookie", "~0.7.1"),
                ("debug", "2.6.9"),
            ]),
        ),
        (
            "4.22.3",
            specs(&[
                ("qs", "~6.16.0"),
                ("path-to-regexp", "~0.1.13"),
                ("cookie", "~0.7.1"),
                ("debug", "2.6.9"),
            ]),
        ),
    ])
}

fn candidates() -> Vec<String> {
    let releases: Vec<String> = [
        "4.16.4", "4.19.2", "4.20.0", "4.21.2", "4.22.0", "4.22.3", "5.0.0",
    ]
    .map(str::to_string)
    .to_vec();
    held_candidates("4.20.0", &releases, |_| true)
}

#[test]
fn express_4_20_0_moves_to_4_22_3_and_says_why() {
    let w = world();
    let held = express();
    assert_eq!(candidates(), ["4.20.0", "4.21.2", "4.22.0", "4.22.3"]);
    let out = judge_target(&candidates(), |v| check_release(&held[v], &w)).expect("held");
    assert_eq!(out.target(), "4.22.3");
    assert!(out.is_clean());
    let note = held_note(&out);
    assert!(
        note.starts_with(
            "4.20.0 exact-pins path-to-regexp 0.1.10 (GHSA-37ch-88jc-xwx2) and qs 6.11.0 (GHSA-4mjr-xmp4-gh2g)"
        ),
        "{note}"
    );
    assert!(
        note.contains(
            "; 2 later releases (4.21.2 to 4.22.0) still bring in an affected path-to-regexp, qs; so \
             4.22.3, the smallest later release on its line"
        ),
        "{note}"
    );
}

#[test]
fn no_clean_release_on_the_line_is_not_a_clean_fix() {
    let w = world();
    let mut held = express();
    held.insert(
        "4.22.3",
        specs(&[
            ("qs", "6.13.0"),
            ("path-to-regexp", "0.1.13"),
            ("cookie", "0.7.1"),
        ]),
    );
    let out = judge_target(&candidates(), |v| check_release(&held[v], &w)).expect("held");
    assert_eq!(out.target(), "4.20.0");
    assert!(!out.is_clean());
    let note = held_note(&out);
    assert!(
        note.contains(
            "no release on its line up to 4.22.3 that its own advisories leave clean clears them, so \
             4.20.0 is NOT a clean fix"
        ),
        "{note}"
    );
}

#[test]
fn fails_closed_when_the_lookup_cannot_run() {
    let mut w = world();
    w.down = true;
    let held = express();
    let out = judge_target(&candidates(), |v| check_release(&held[v], &w)).expect("held");
    assert!(matches!(out, HeldOutcome::Unchecked { ref target, .. } if target == "4.20.0"));
    assert!(held_note(&out)
        .contains("4.20.0 exact-pins or narrowly ranges are affected was NOT CHECKED"));
}

#[test]
fn a_release_that_holds_nothing_affected_is_left_alone() {
    let w = world();
    let held = express();
    assert_eq!(
        judge_target(&["4.22.3".to_string()], |v| check_release(&held[v], &w)),
        None
    );
    assert_eq!(
        check_release(&BTreeMap::new(), &w),
        HeldCheck::Checked(Vec::new())
    );
}

#[test]
fn a_concrete_finding_stands_beside_an_unread_range() {
    let mut w = world();
    w.facts.remove("cookie");
    let held = express();
    // 4.20.0's pins decide it although cookie's facts are missing (it is exact there).
    assert!(
        matches!(check_release(&held["4.20.0"], &w), HeldCheck::Checked(ref a) if a.len() == 2)
    );
    // 4.22.3 holds nothing affected but cookie "~0.7.1" cannot be judged.
    assert!(
        matches!(check_release(&held["4.22.3"], &w), HeldCheck::Unchecked(ref r) if r.contains("cookie \"~0.7.1\""))
    );
}

#[test]
fn the_search_stops_at_ten_releases() {
    let releases: Vec<String> = (0..15).map(|p| format!("1.0.{p}")).collect();
    let got = held_candidates("1.0.0", &releases, |_| true);
    assert_eq!(got.len(), HELD_CANDIDATES);
    assert_eq!(got.last().map(String::as_str), Some("1.0.9"));
    let own_clean = held_candidates("1.0.0", &releases, |v| v != "1.0.1");
    assert!(!own_clean.contains(&"1.0.1".to_string()));
}

#[test]
fn own_advisories_with_a_fix_rule_a_release_out() {
    let fixed = window("1.0.5");
    let open = Some(json!([{"type": "SEMVER", "events": [{"introduced": "0"}]}]).to_string());
    let releases: Vec<String> = ["1.0.0", "1.0.4", "1.0.5", "1.0.6"]
        .map(str::to_string)
        .to_vec();
    let kinds = [
        (&fixed, AdvisoryKind::default()),
        (&open, AdvisoryKind::default()),
    ];
    let own = OwnAdvisories::new(Some("1.0.0"), &releases, &kinds);
    assert!(!own.clean("1.0.4"), "a fixed advisory still affects it");
    assert!(own.clean("1.0.5"), "only the unfixed one remains");
    assert!(!own.clears_all("1.0.5"));
}

/// Review of #1039, S4: an unchecked release mid-walk is passed, never
/// called clean; when nothing clean follows, the note names it NOT CHECKED
/// instead of claiming every release up to the last was judged.
#[test]
fn an_unchecked_release_mid_walk_is_named_not_checked() {
    let affected = |v: &str| {
        HeldCheck::Checked(vec![HeldDependency {
            name: "qs".into(),
            spec: "6.11.0".into(),
            exact: true,
            version: v.into(),
            ids: vec!["GHSA-4mjr-xmp4-gh2g".into()],
        }])
    };
    let c: Vec<String> = ["1.0.0", "1.0.1", "1.0.2", "1.0.3"]
        .map(str::to_string)
        .to_vec();
    // 1.0.1 cannot be judged, 1.0.2 is clean: moved past it.
    let moved = judge_target(&c, |v| match v {
        "1.0.1" => HeldCheck::Unchecked("offline".into()),
        "1.0.2" => HeldCheck::Checked(Vec::new()),
        _ => affected("6.11.0"),
    });
    assert!(matches!(moved, Some(HeldOutcome::Moved { ref to, .. }) if to == "1.0.2"));
    // Nothing clean: Blocked, and the unchecked releases are named.
    let blocked = judge_target(&c, |v| match v {
        "1.0.1" | "1.0.3" => HeldCheck::Unchecked("offline".into()),
        _ => affected("6.11.0"),
    });
    let Some(out) = blocked else {
        unreachable!("the target holds an affected dependency")
    };
    assert!(
        matches!(out, HeldOutcome::Blocked { ref unchecked, .. } if *unchecked == ["1.0.1", "1.0.3"])
    );
    let note = held_note(&out);
    assert!(
        note.contains(
            "no release on its line up to 1.0.3 that its own advisories leave clean clears it, of \
             the releases that could be checked (1.0.1 and 1.0.3 NOT CHECKED), so 1.0.0 is NOT a \
             clean fix"
        ),
        "{note}"
    );
    // A Blocked from before this field still reads.
    let old: HeldOutcome = serde_json::from_value(json!({
        "status": "blocked", "target": "1.0.0", "affected": [], "searched": ["1.0.0"]
    }))
    .unwrap_or(HeldOutcome::Unchecked {
        target: String::new(),
        reason: String::new(),
    });
    assert!(matches!(old, HeldOutcome::Blocked { ref unchecked, .. } if unchecked.is_empty()));
}

/// N1: a host's memoized `judged` is what the release check uses.
#[test]
fn the_check_reads_the_hosts_judged_version() {
    struct Fixed;
    impl HeldSources for Fixed {
        fn facts(&self, _: &str) -> Option<HeldFacts> {
            None
        }
        fn affected(&self, _: &str, version: &str) -> Option<Vec<String>> {
            Some(if version == "6.14.2" {
                vec!["GHSA-x".into()]
            } else {
                vec![]
            })
        }
        fn judged(&self, _: &str, _: &str) -> Option<String> {
            Some("6.14.2".into())
        }
    }
    let got = check_release(&specs(&[("qs", "~6.14.0")]), &Fixed);
    assert!(matches!(got, HeldCheck::Checked(ref a) if a.len() == 1 && a[0].version == "6.14.2"));
}
