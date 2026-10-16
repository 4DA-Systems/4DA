// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Withdrawn releases in the fix-target walk (FX-H5). The app's
//! `osv::fix_target_tests` and the shared rule vectors cover the rest.

use super::*;

/// One advisory's stored ranges JSON (`Some`, as the mirror stores it).
#[allow(clippy::unnecessary_wraps)]
fn ranges(events: &str) -> Option<String> {
    Some(format!(r#"[{{"type":"SEMVER","events":{events}}}]"#))
}

fn facts(published: &[&str], excluded: &[&str]) -> RegistryReleases {
    RegistryReleases {
        published: published.iter().map(ToString::to_string).collect(),
        excluded: excluded.iter().map(ToString::to_string).collect(),
    }
}

fn walk_in(installed: &str, all: &[Option<String>], registry: &RegistryReleases) -> WalkOutcome {
    let refs: Vec<&Option<String>> = all.iter().collect();
    walk_target_in("crates.io", installed, &refs, registry)
}

/// h2 0.3.17 fixed RUSTSEC-2023-0034 and was yanked; 0.3.18 is the release.
#[test]
fn a_yanked_fix_steps_to_the_next_release_the_registry_offers() {
    let h2 = [ranges(r#"[{"introduced":"0"},{"fixed":"0.3.17"}]"#)];
    let none = walk_in("0.3.16", &h2, &RegistryReleases::default());
    assert_eq!(none.target.map(|t| t.version).as_deref(), Some("0.3.17"));

    let registry = facts(&["0.3.16", "0.3.17", "0.3.18", "0.3.19"], &["0.3.17"]);
    let out = walk_in("0.3.16", &h2, &registry);
    assert_eq!(
        out.target,
        Some(WalkTarget {
            version: "0.3.18".into(),
            clears_all: true
        })
    );
    assert_eq!(out.withheld, ["0.3.17"]);
}

#[test]
fn every_withdrawn_release_on_the_way_is_named_and_skipped() {
    let a = [ranges(r#"[{"introduced":"0"},{"fixed":"1.0.1"}]"#)];
    let registry = facts(&["1.0.0", "1.0.1", "1.0.2", "1.0.3"], &["1.0.1", "1.0.2"]);
    let out = walk_in("1.0.0", &a, &registry);
    assert_eq!(out.target.map(|t| t.version).as_deref(), Some("1.0.3"));
    assert_eq!(out.withheld, ["1.0.1", "1.0.2"]);
}

#[test]
fn when_every_clearing_release_is_withdrawn_there_is_no_target_and_it_says_why() {
    let a = [ranges(r#"[{"introduced":"0"},{"fixed":"1.0.1"}]"#)];
    let out = walk_in("1.0.0", &a, &facts(&["1.0.0", "1.0.1"], &["1.0.1"]));
    assert_eq!(out.target, None);
    assert_eq!(out.withheld, ["1.0.1"]);

    // Another advisory still has a way out: the walk takes it, and does not
    // claim to clear the one whose only fix was withdrawn.
    let both = [
        ranges(r#"[{"introduced":"0"},{"fixed":"2.0.0"}]"#),
        ranges(r#"[{"introduced":"0"},{"fixed":"1.1.0"}]"#),
    ];
    let registry = facts(&["1.0.0", "1.1.0", "2.0.0"], &["2.0.0"]);
    let out = walk_in("1.0.0", &both, &registry);
    assert_eq!(
        out.target,
        Some(WalkTarget {
            version: "1.1.0".into(),
            clears_all: false
        })
    );
    assert_eq!(out.withheld, ["2.0.0"]);
}

#[test]
fn a_withdrawn_release_above_a_last_affected_bound_is_passed_over() {
    let a = [ranges(r#"[{"introduced":"0"},{"last_affected":"2.0.0"}]"#)];
    let registry = facts(&["2.0.0", "2.0.1", "2.0.2"], &["2.0.1"]);
    let out = walk_in("1.5.0", &a, &registry);
    assert_eq!(out.target.map(|t| t.version).as_deref(), Some("2.0.2"));
    assert_eq!(out.withheld, ["2.0.1"]);
}

#[test]
fn published_releases_let_the_walk_name_one_no_advisory_mentions() {
    // Without the registry list nothing above the last_affected bound is
    // known; with it, the first release past it is.
    let a = [ranges(r#"[{"introduced":"0"},{"last_affected":"2.0.0"}]"#)];
    assert_eq!(
        walk_in("1.5.0", &a, &RegistryReleases::default()).target,
        None
    );
    let out = walk_in("1.5.0", &a, &facts(&["2.0.0", "2.1.0-rc.1", "2.1.0"], &[]));
    assert_eq!(out.target.map(|t| t.version).as_deref(), Some("2.1.0"));
    assert!(out.withheld.is_empty());
}

#[test]
fn an_unreadable_advisory_withholds_nothing() {
    let all = [ranges(r#"[{"introduced":"0"},{"fixed":"1.0.1"}]"#), None];
    let out = walk_in("1.0.0", &all, &facts(&["1.0.1", "1.0.2"], &["1.0.1"]));
    assert_eq!(out, WalkOutcome::default());
}

#[test]
fn installable_fix_skips_a_release_the_same_advisory_still_affects() {
    // Withdrawn 1.0.1 ends the first window; 1.0.2 opens a second one.
    let a = ranges(
        r#"[{"introduced":"0"},{"fixed":"1.0.1"},{"introduced":"1.0.2"},{"fixed":"1.0.3"}]"#,
    );
    let registry = facts(&["1.0.1", "1.0.2", "1.0.3"], &["1.0.1"]);
    let fix = installable_line_fix("npm", "1.0.0", &a, &registry);
    assert_eq!(fix.fix.as_deref(), Some("1.0.3"));
    assert_eq!(fix.withheld, ["1.0.1"]);
    // Not withdrawn: the line fix as before.
    let fix = installable_line_fix("npm", "1.0.0", &a, &RegistryReleases::default());
    assert_eq!(fix.fix.as_deref(), Some("1.0.1"));
    assert!(fix.withheld.is_empty());
    // No line fix at all: nothing, and nothing withheld.
    let open = ranges(r#"[{"introduced":"0"}]"#);
    assert_eq!(
        installable_line_fix("npm", "1.0.0", &open, &registry),
        LineFix::default()
    );
}
