// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for per-line upgrade targets (extracted via #[path]).

use super::*;
use crate::osv::types::MatchedDependency;

/// GHSA-p293-qw3h-jr36 as OSV publishes it: one fix per release line.
const NEXT_RCE_RANGES: &str = r#"[{"type":"SEMVER","events":[{"introduced":"13.4.0"},{"fixed":"15.5.24"}]},{"type":"SEMVER","events":[{"introduced":"16.0.0"},{"fixed":"16.3.3"}]}]"#;

/// The live brace-expansion advisory set (OSV, 2026-10-01): every advisory
/// fixes each major line separately. Two of nine shown; the newest one
/// (GHSA-q2hr) is the line ceiling.
const BRACE_Q2HR: &str = r#"[{"type":"SEMVER","events":[{"introduced":"4.0.0"},{"fixed":"5.0.12"}]},{"type":"SEMVER","events":[{"introduced":"3.0.0"},{"fixed":"3.0.9"}]},{"type":"SEMVER","events":[{"introduced":"2.0.0"},{"fixed":"2.1.7"}]},{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.1.21"}]}]"#;
const BRACE_3JXR: &str = r#"[{"type":"SEMVER","events":[{"introduced":"3.0.0"},{"fixed":"5.0.7"}]},{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.1.16"}]},{"type":"SEMVER","events":[{"introduced":"2.0.0"},{"fixed":"2.1.2"}]}]"#;

/// Live undici windows that move a 5.28.4 install: the 5.x fixes end at
/// 5.29.0, but two advisories cover `0..6.28.x`, and GHSA-3wwx opens a new
/// window at 6.25.0 — so the clean path crosses into 6.x and lands on 6.28.1.
const UNDICI_CXRH: &str = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"5.29.0"}]},{"type":"SEMVER","events":[{"introduced":"6.0.0"},{"fixed":"6.21.2"}]},{"type":"SEMVER","events":[{"introduced":"7.0.0"},{"fixed":"7.5.0"}]}]"#;
const UNDICI_8XCM: &str = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"6.28.0"}]},{"type":"SEMVER","events":[{"introduced":"7.0.0"},{"fixed":"7.29.0"}]},{"type":"SEMVER","events":[{"introduced":"8.0.0"},{"fixed":"8.9.0"}]}]"#;
const UNDICI_3WWX: &str = r#"[{"type":"SEMVER","events":[{"introduced":"6.25.0"},{"fixed":"6.28.1"}]},{"type":"SEMVER","events":[{"introduced":"7.28.0"},{"fixed":"7.29.1"}]},{"type":"SEMVER","events":[{"introduced":"8.1.0"},{"fixed":"8.10.2"}]}]"#;
const UNDICI_2GQQ: &str = r#"[{"type":"SEMVER","events":[{"introduced":"7.1.0"},{"fixed":"7.29.1"}]},{"type":"SEMVER","events":[{"introduced":"8.0.0"},{"fixed":"8.10.2"}]}]"#;

fn some(s: &str) -> Option<String> {
    Some(s.to_string())
}

#[test]
fn fix_is_the_one_for_the_installed_release_line() {
    let ranges = some(NEXT_RCE_RANGES);
    assert_eq!(
        fix_for_version("", "16.2.10", &ranges).as_deref(),
        Some("16.3.3")
    );
    assert_eq!(
        fix_for_version("", "15.1.0", &ranges).as_deref(),
        Some("15.5.24")
    );
    assert_eq!(fix_for_version("", "16.3.3", &ranges), None, "not affected");
    assert_eq!(fix_for_version("", "12.0.0", &ranges), None, "not affected");
}

#[test]
fn last_affected_window_has_no_fix() {
    let ranges =
        some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"last_affected":"0.9.6"}]}]"#);
    assert_eq!(fix_for_version("", "0.9.6", &ranges), None);
}

/// The navcal defect: a 1.1.12 copy stays on the 1.x line.
#[test]
fn clean_version_stays_on_the_installed_line() {
    let (q2hr, jxr) = (some(BRACE_Q2HR), some(BRACE_3JXR));
    let ranges = [&q2hr, &jxr];
    assert_eq!(
        clean_version("", "1.1.12", &ranges).as_deref(),
        Some("1.1.21")
    );
    assert_eq!(
        clean_version("", "1.1.15", &ranges).as_deref(),
        Some("1.1.21")
    );
    assert_eq!(
        clean_version("", "5.0.9", &ranges).as_deref(),
        Some("5.0.12")
    );
    assert_eq!(clean_version("", "5.0.12", &ranges), None, "already clean");
}

/// The navcal undici defect: the per-advisory maximum (6.28.0) sits inside
/// GHSA-3wwx's 6.25.0 window; the walk continues to 6.28.1. A copy on 7.x
/// never leaves 7.x.
#[test]
fn clean_version_walks_past_a_window_the_first_fix_lands_in() {
    let all = [
        some(UNDICI_CXRH),
        some(UNDICI_8XCM),
        some(UNDICI_3WWX),
        some(UNDICI_2GQQ),
    ];
    let ranges: Vec<&Option<String>> = all.iter().collect();
    assert_eq!(
        clean_version("", "5.28.4", &ranges).as_deref(),
        Some("6.28.1")
    );
    assert_eq!(
        clean_version("", "7.28.0", &ranges).as_deref(),
        Some("7.29.1")
    );
    assert_eq!(
        clean_version("", "6.28.0", &ranges).as_deref(),
        Some("6.28.1")
    );
}

#[test]
fn clean_version_refuses_a_window_without_a_fix_or_an_unreadable_advisory() {
    let fixed = some(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.0.0"}]}]"#);
    let open_ended = some(r#"[{"type":"SEMVER","events":[{"introduced":"1.5.0"}]}]"#);
    assert_eq!(
        clean_version("", "1.0.0", &[&fixed, &open_ended]),
        None,
        "2.0.0 is inside an open-ended window: nothing is clean"
    );
    let unreadable: Option<String> = None;
    assert_eq!(
        clean_version("", "1.0.0", &[&fixed, &unreadable]),
        None,
        "an advisory with no range data cannot be cleared"
    );
}

#[test]
fn upgrade_type_follows_caret_compatibility() {
    assert_eq!(upgrade_type("1.1.12", "1.1.21"), Some(UpgradeType::Patch));
    assert_eq!(upgrade_type("4.17.20", "4.18.0"), Some(UpgradeType::Minor));
    assert_eq!(upgrade_type("5.28.4", "6.28.1"), Some(UpgradeType::Major));
    assert_eq!(upgrade_type("1.1.12", "5.0.12"), Some(UpgradeType::Major));
    assert_eq!(upgrade_type("0.8.5", "0.8.6"), Some(UpgradeType::Patch));
    assert_eq!(upgrade_type("0.8.5", "0.9.0"), Some(UpgradeType::Major));
    assert_eq!(upgrade_type("0.0.3", "0.0.4"), Some(UpgradeType::Major));
    assert_eq!(upgrade_type("banana", "1.0.0"), None);
}

fn copy(project: &str, version: &str, fix: Option<&str>, clean: Option<&str>) -> MatchedDependency {
    MatchedDependency {
        project_path: project.to_string(),
        installed_version: Some(version.to_string()),
        is_direct: false,
        is_dev: true,
        is_version_confirmed: true,
        fixed_version: fix.map(str::to_string),
        clean_version: clean.map(str::to_string),
        target_version: clean.map(str::to_string),
        withheld: Vec::new(),
    }
}

fn matched(id: &str, instances: Vec<MatchedDependency>) -> MatchedAdvisory {
    let mut projects: Vec<String> = instances.iter().map(|d| d.project_path.clone()).collect();
    projects.sort();
    projects.dedup();
    MatchedAdvisory {
        advisory_id: id.to_string(),
        summary: id.to_string(),
        details: None,
        package_name: "brace-expansion".to_string(),
        ecosystem: "npm".to_string(),
        installed_version: None,
        fixed_version: Some("5.0.12".to_string()),
        severity_type: None,
        cvss_score: None,
        source_url: None,
        is_version_confirmed: true,
        project_paths: projects,
        published_at: None,
        dependency_instances: instances,
        aliases: vec![],
        severity_label: None,
        cwe_ids: Vec::new(),
    }
}

/// AD-044 for targets: another project's copy (5.0.9 in /webhook) can
/// neither set nor raise /navcal's target, even though the advisory's
/// machine-wide `fixed_version` says 5.0.12.
#[test]
fn line_targets_are_scoped_to_the_named_projects() {
    let adv = matched(
        "GHSA-q2hr",
        vec![
            copy("/navcal", "1.1.12", Some("1.1.21"), Some("1.1.21")),
            copy("/webhook", "1.1.18", Some("1.1.21"), Some("1.1.21")),
            copy("/webhook", "5.0.9", Some("5.0.12"), Some("5.0.12")),
        ],
    );
    let navcal = line_targets(&[&adv], &["/navcal".to_string()]);
    assert_eq!(navcal.len(), 1);
    assert_eq!(navcal[0].installed_version, "1.1.12");
    assert_eq!(navcal[0].target_version.as_deref(), Some("1.1.21"));
    assert!(navcal[0].clears_all_known);
    assert_eq!(navcal[0].upgrade_type, Some(UpgradeType::Patch));
    assert_eq!(distinct_targets(&navcal), vec!["1.1.21".to_string()]);

    let webhook = line_targets(&[&adv], &["/webhook".to_string()]);
    assert_eq!(
        webhook
            .iter()
            .map(|l| (l.installed_version.as_str(), l.target_version.as_deref()))
            .collect::<Vec<_>>(),
        vec![("1.1.18", Some("1.1.21")), ("5.0.9", Some("5.0.12"))],
        "two copies, two lines, two targets — sorted by installed version"
    );
    assert_eq!(
        describe_lines(&webhook),
        "1.1.18 -> 1.1.21, 5.0.9 -> 5.0.12"
    );
}

/// No clean version (another advisory has no fix): fall back to the highest
/// line fix and say it does not clear everything.
#[test]
fn line_target_falls_back_to_the_line_fix_when_nothing_is_clean() {
    let a = matched("GHSA-a", vec![copy("/p", "1.0.0", Some("1.2.0"), None)]);
    let b = matched("GHSA-b", vec![copy("/p", "1.0.0", Some("1.3.0"), None)]);
    let lines = line_targets(&[&a, &b], &["/p".to_string()]);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].target_version.as_deref(), Some("1.3.0"));
    assert!(!lines[0].clears_all_known);
    assert_eq!(lines[0].sites.len(), 1);
    assert!(lines[0].sites[0].is_dev);
    assert!(!lines[0].sites[0].is_direct);
}

#[test]
fn major_jump_is_flagged_and_described() {
    let adv = matched(
        "GHSA-undici",
        vec![copy("/navcal", "5.28.4", Some("6.28.0"), Some("6.28.1"))],
    );
    let lines = line_targets(&[&adv], &["/navcal".to_string()]);
    assert!(lines[0].is_major());
    assert_eq!(describe_lines(&lines), "5.28.4 -> 6.28.1 (major)");
}

// ============================================================================
// `last_affected` windows and the walk (rule vectors ft-043, ft-052)
// ============================================================================

fn ranges(json: &[&str]) -> Vec<Option<String>> {
    json.iter().map(|j| some(j)).collect()
}

fn walk(eco: &str, installed: &str, all: &[Option<String>]) -> Option<WalkTarget> {
    let refs: Vec<&Option<String>> = all.iter().collect();
    walk_target(eco, installed, &refs)
}

fn landed(version: &str, clears_all: bool) -> Option<WalkTarget> {
    Some(WalkTarget {
        version: version.to_string(),
        clears_all,
    })
}

/// mistune 0.8.3, the pinned records reduced to the windows that decide it
/// (OSV 2026-10-10). GHSA-8g87 ends at `last_affected 3.2.0`, so the walk
/// used to give up; the plan then printed the highest line fix, 3.3.0, the
/// version GHSA-6m44 / PYSEC-2026-3863 open at. OSV: 3.3.3 is clean.
#[test]
fn mistune_walks_past_last_affected_and_the_window_its_highest_fix_opens() {
    let all = ranges(&[
        // GHSA-8g87-j6q8-g93x
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"last_affected":"3.2.0"}]},{"type":"ENUMERATED","events":["0.8.3","3.1.4","3.2.0"]}]"#,
        // GHSA-2hm2-hc3v-44h9 (and the others fixed in 3.3.0)
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"3.3.0"}]},{"type":"ENUMERATED","events":["0.8.3","3.1.4","3.2.0","3.2.1"]}]"#,
        // GHSA-6m44-fpc8-c3rq / PYSEC-2026-3863
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"3.3.0"},{"fixed":"3.3.3"}]},{"type":"ENUMERATED","events":["3.3.0","3.3.1","3.3.2"]}]"#,
    ]);
    assert_eq!(walk("PyPI", "0.8.3", &all), landed("3.3.3", true));
    // The old fallback target is still vulnerable: never offer it.
    assert_eq!(
        crate::osv::matching::check_version_affected("PyPI", Some("3.3.0"), &all[2]),
        (true, true)
    );
}

/// pyjwt 2.9.0: PYSEC-2025-183 ends at `last_affected 2.10.1`; the releases
/// above it are named by other records. The target (2.15.0) was right, but
/// the engine said it cleared only the fixed advisories; OSV: it is clean.
#[test]
fn pyjwt_last_affected_no_longer_declines_a_clean_target() {
    let all = ranges(&[
        // PYSEC-2025-183
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"last_affected":"2.10.1"}]},{"type":"ENUMERATED","events":["2.9.0","2.10.0","2.10.1"]}]"#,
        // GHSA-42vr-xj54-vc7v
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"2.0.0a1"},{"fixed":"2.15.0"}]}]"#,
        // GHSA-gvp8-978c-rx2q
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"2.11.0"},{"last_affected":"2.13.0"}]},{"type":"ENUMERATED","events":["2.11.0","2.12.0","2.12.1","2.13.0"]}]"#,
    ]);
    assert_eq!(walk("PyPI", "2.9.0", &all), landed("2.15.0", true));
}

/// AD-055 ¶5 still holds where it should: braces' GHSA-vfj7 ends at
/// `last_affected 3.0.3` and the data names no release above it, so 3.0.3
/// clears only the advisories that have a fix.
#[test]
fn last_affected_with_nothing_known_above_it_declines_the_clean_claim() {
    let all = ranges(&[
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"3.0.3"}]}]"#,
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"last_affected":"3.0.3"}]}]"#,
    ]);
    assert_eq!(walk("npm", "3.0.2", &all), landed("3.0.3", false));
    let refs: Vec<&Option<String>> = all.iter().collect();
    assert_eq!(clean_version("npm", "3.0.2", &refs), None);
}

/// With an advisory no release leaves, the walk still clears every one that
/// HAS a way out. The old fallback (highest own-line fix, 2.0.0) sat inside
/// the second window.
#[test]
fn a_target_that_cannot_clear_everything_still_clears_every_fixable_window() {
    let all = ranges(&[
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.0.0"}]}]"#,
        r#"[{"type":"SEMVER","events":[{"introduced":"2.0.0"},{"fixed":"2.5.0"}]}]"#,
        r#"[{"type":"SEMVER","events":[{"introduced":"1.5.0"}]}]"#,
    ]);
    assert_eq!(walk("npm", "1.0.0", &all), landed("2.5.0", false));
    // Nothing with a way out affects the copy: no target to name.
    let open_only = ranges(&[r#"[{"type":"SEMVER","events":[{"introduced":"1.5.0"}]}]"#]);
    assert_eq!(walk("npm", "1.6.0", &open_only), None);
}

/// The release past a `last_affected` bound is a final release, never a
/// pre-release the data happens to name.
#[test]
fn last_affected_successor_skips_pre_releases() {
    let all = ranges(&[
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"last_affected":"1.0.0"}]}]"#,
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"1.1.0rc1"},{"fixed":"1.1.0"}]}]"#,
    ]);
    assert_eq!(walk("PyPI", "0.9.0", &all), landed("1.1.0", true));
}

/// An enumerated `versions` list is left at the first known release it does
/// not name, in one step rather than one hop per listed version.
#[test]
fn enumerated_versions_are_left_at_the_first_unlisted_release() {
    let all = ranges(&[
        r#"[{"type":"ENUMERATED","events":["1.0.0","1.0.1","1.0.2","1.0.3"]}]"#,
        r#"[{"type":"SEMVER","events":[{"introduced":"1.0.4"},{"fixed":"1.0.5"}]}]"#,
    ]);
    assert_eq!(walk("npm", "1.0.0", &all), landed("1.0.5", true));
}

/// Two advisories spell the same Go fix with and without `+incompatible`
/// (docker/docker on the fixture corpus): the target is the published tag,
/// whichever order the advisories come in.
#[test]
fn a_tie_between_spellings_keeps_the_published_go_tag() {
    let plain = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"25.0.13"}]}]"#;
    let tagged =
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"25.0.13+incompatible"}]}]"#;
    for order in [[plain, tagged], [tagged, plain]] {
        assert_eq!(
            walk("Go", "20.10.7+incompatible", &ranges(&order)),
            landed("25.0.13+incompatible", true)
        );
    }
}

/// PyPI windows order by PEP 440 in the fix lookup too: `1.0.0-1` is the
/// post release 1.0.0.post1, past a fix at 1.0.0.
#[test]
fn pypi_fix_lookup_reads_post_releases_as_pep440() {
    let fixed = some(r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"1.0.0"}]}]"#);
    assert_eq!(fix_for_version("PyPI", "1.0.0-1", &fixed), None);
    assert_eq!(
        fix_for_version("npm", "1.0.0-1", &fixed).as_deref(),
        Some("1.0.0"),
        "npm reads it as a pre-release of 1.0.0"
    );
}
