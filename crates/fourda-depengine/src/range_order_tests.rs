// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! A range's events are evaluated the way the OSV schema's evaluation reads
//! them: sorted by version (equal versions keep their listed order), each
//! `introduced` opening a span that the next `fixed` or `last_affected`
//! closes. Pairing them in listed order read npm `derby`'s
//! GHSA-82jv-9wjw-pqh6 range `introduced 4.0.0-beta1, fixed 4.0.0-beta.11`
//! (the fix sorts BELOW its own start) as an empty window, so derby 4.0.0 and
//! 4.2.3 read "not affected" while OSV.dev reports both affected (the twin of
//! `@4da/mcp-server` audit 2026-11-19, ranges R2).
//!
//! The expected values are OSV.dev's own answers: api.osv.dev `/v1/query` per
//! version, read 2026-11-19 (each case says which), never this engine's.

use super::check_version_affected_uncached;
use crate::fix_target::walk_target;

fn ranges(json: &str) -> Option<String> {
    Some(json.to_string())
}

fn verdicts(ecosystem: &str, json: &str, cases: &[(&str, bool)]) {
    let ranges = ranges(json);
    for (version, want) in cases {
        let (affected, confirmed) =
            check_version_affected_uncached(ecosystem, Some(version), &ranges);
        assert!(confirmed, "{version}: the range is decided");
        assert_eq!(affected, *want, "{version}");
    }
}

/// GHSA-82jv-9wjw-pqh6 as the mirror stores it for derby (three affected
/// entries merged), the ranges exactly as api.osv.dev serves them.
const DERBY: &str = r#"[
  {"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.3.2"}]},
  {"type":"SEMVER","events":[{"introduced":"3.0.0"},{"fixed":"3.0.2"}]},
  {"type":"SEMVER","events":[{"introduced":"4.0.0-beta1"},{"fixed":"4.0.0-beta.11"}]}
]"#;

/// api.osv.dev `/v1/query` for npm derby, 2026-11-19: GHSA-82jv-9wjw-pqh6 is
/// returned for exactly the `true` versions.
#[test]
fn derby_reads_as_osv_answers_it() {
    verdicts(
        "npm",
        DERBY,
        &[
            ("2.3.1", true),
            ("2.3.2", false),
            ("3.0.0", true),
            ("3.0.1", true),
            ("3.0.2", false),
            ("3.5.0", false),
            ("4.0.0-beta.11", false),
            ("4.0.0-beta1", true),
            ("4.0.0", true),
            ("4.2.3", true),
        ],
    );
}

/// The OSV schema's evaluation sorts the events, so a range listed fix first
/// is the span it sorts into.
#[test]
fn events_listed_out_of_order_pair_as_they_sort() {
    verdicts(
        "npm",
        r#"[{"type":"SEMVER","events":[{"fixed":"2.0.0"},{"introduced":"1.0.0"}]}]"#,
        &[
            ("0.5.0", false),
            ("1.0.0", true),
            ("1.9.9", true),
            ("2.0.0", false),
            ("3.0.0", false),
        ],
    );
}

/// An `introduced` inside an open span changes nothing: the span runs from
/// the first `introduced` to the next closing event (1.5.0 read "not
/// affected" when the second `introduced` replaced the first).
#[test]
fn an_introduced_inside_an_open_span_changes_nothing() {
    verdicts(
        "npm",
        r#"[{"type":"SEMVER","events":[{"introduced":"1.0.0"},{"introduced":"2.0.0"},{"fixed":"3.0.0"}]}]"#,
        &[
            ("0.5.0", false),
            ("1.5.0", true),
            ("2.5.0", true),
            ("3.0.0", false),
        ],
    );
}

/// A closing event with no span open closes nothing and opens nothing, as the
/// schema's evaluation reads it: api.osv.dev 2026-11-19 returns
/// PYSEC-2025-128 for lunasvg 3.0.0 and not for 3.0.1, and PYSEC-2022-43170
/// for neither tryton 2.4.15 nor 5.0.12. (The TypeScript engine keeps a
/// cautious "presumed" reading here that OSV.dev's live answer overrules;
/// this engine has no live answer to defer to, so it reads what OSV reads.)
#[test]
fn a_closing_event_with_nothing_open_closes_nothing() {
    verdicts(
        "PyPI",
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"last_affected":"3.0.0"},{"last_affected":"3.1.0"}]}]"#,
        &[
            ("3.0.0", true),
            ("3.0.1", false),
            ("3.1.0", false),
            ("3.2.0", false),
        ],
    );
    verdicts(
        "PyPI",
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"5.0.0"},{"fixed":"5.0.12"},{"introduced":"6.0.0"},{"fixed":"6.0.5"},{"introduced":"6.2.0"},{"fixed":"6.2.2"},{"fixed":"6.2.6"},{"fixed":"6.0.16"},{"fixed":"5.0.46"}]}]"#,
        &[
            ("2.4.15", false),
            ("5.0.0", true),
            ("5.0.11", true),
            ("5.0.12", false),
            ("6.0.4", true),
            ("6.0.5", false),
            ("6.2.1", true),
            ("6.2.2", false),
            ("7.0.0", false),
        ],
    );
}

/// An unknown `-NA` bound (OSV's PYSEC import, "boundary not available")
/// closes nothing: api.osv.dev 2026-11-19 returns PYSEC-2024-266
/// (`last_affected: "2.10.0-NA"`, no `versions` list) for none of
/// apache-airflow 2.8.0, 2.9.0, 2.10.0 and 3.4.0. Pinned here because the
/// TypeScript engine now reads such a record as undecidable (audit R1) and
/// defers to OSV.dev's live answer, which is this reading.
#[test]
fn an_unknown_na_bound_closes_nothing_as_osv_answers() {
    verdicts(
        "PyPI",
        r#"[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"last_affected":"2.10.0-NA"}]}]"#,
        &[
            ("2.8.0", false),
            ("2.9.0", false),
            ("2.10.0", false),
            ("3.4.0", false),
        ],
    );
}

/// A target never lands inside a span the sorted events leave open: an
/// advisory whose fix sorts below its start affects every release from that
/// start, so the line fix of the other advisory brings it in and no target
/// is named. Paired in listed order the span was empty and 2.0.0 was named
/// clean.
#[test]
fn a_target_never_lands_in_a_span_the_sorted_events_leave_open() {
    let fixed = ranges(r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.0.0"}]}]"#);
    let open = ranges(
        r#"[{"type":"SEMVER","events":[{"introduced":"1.5.0-beta1"},{"fixed":"1.5.0-beta.11"}]}]"#,
    );
    assert_eq!(walk_target("npm", "1.0.0", &[&fixed, &open]), None);
    // From 1.0.0 with only the fixed advisory, 2.0.0 is the clean target.
    let only = walk_target("npm", "1.0.0", &[&fixed]).map(|t| (t.version, t.clears_all));
    assert_eq!(only, Some(("2.0.0".to_string(), true)));
}

/// derby's own line: 2.3.1 is fixed in 2.3.2, which none of its spans holds.
#[test]
fn the_derby_line_fix_is_2_3_2() {
    let derby = ranges(DERBY);
    let walk = walk_target("npm", "2.3.1", &[&derby]).map(|t| (t.version, t.clears_all));
    assert_eq!(walk, Some(("2.3.2".to_string(), true)));
}
