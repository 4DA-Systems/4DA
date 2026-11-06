// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The memoized `check_version_affected` and `requirement_admits` answer
//! exactly what the evaluations behind them answer, on every pinned OSV
//! record of the conformance corpus, asked twice (the second answer comes
//! from the memo), and a repeated question costs one evaluation.

use std::path::PathBuf;

use fourda_depengine::npm_range::{requirement_admits, requirement_admits_uncached};
use fourda_depengine::osv::{stored_records, OsvRecord};
use fourda_depengine::range::{
    check_version_affected, check_version_affected_uncached, range_evaluations,
};

fn osv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/tests/conformance/osv")
}

/// Every version string an event of `ranges` names, plus a few around them.
fn probes(ranges: &str) -> Vec<String> {
    let mut out: Vec<String> = [
        "0.0.1",
        "0.1.0",
        "1.0.0",
        "1.2.3",
        "2.0.0",
        "99.0.0",
        "not-a-version",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    let parsed: serde_json::Value = serde_json::from_str(ranges).unwrap_or_default();
    for range in parsed.as_array().into_iter().flatten() {
        for event in range["events"].as_array().into_iter().flatten() {
            for v in event.as_object().into_iter().flat_map(|o| o.values()) {
                if let Some(v) = v.as_str() {
                    out.push(v.to_string());
                }
            }
        }
    }
    out
}

#[test]
fn the_memoized_range_check_answers_what_the_evaluation_answers() {
    let mut checked = 0usize;
    let entries = std::fs::read_dir(osv_dir()).map_err(|e| e.to_string());
    for entry in entries.into_iter().flatten().flatten().take(1_500) {
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(record) = serde_json::from_str::<OsvRecord>(&text) else {
            continue;
        };
        for stored in stored_records(&record) {
            let Some(ranges) = stored.affected_ranges.clone() else {
                continue;
            };
            let json = Some(ranges.clone());
            for version in probes(&ranges) {
                let want =
                    check_version_affected_uncached(&stored.ecosystem, Some(&version), &json);
                for _ in 0..2 {
                    assert_eq!(
                        check_version_affected(&stored.ecosystem, Some(&version), &json),
                        want,
                        "{} {version}",
                        stored.advisory_id
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(
        checked > 1_000,
        "only {checked} pairs checked: the corpus was not read"
    );
    for (eco, version, json) in [
        ("npm", None, Some("[]".to_string())),
        ("npm", Some("1.0.0"), None),
    ] {
        assert_eq!(
            check_version_affected(eco, version, &json),
            check_version_affected_uncached(eco, version, &json)
        );
    }
}

#[test]
fn the_memoized_npm_requirement_answers_what_the_evaluation_answers() {
    let specs = [
        "^1.2.3",
        "~1.2.3",
        ">=1.0.0 <2.0.0",
        "1.x",
        "*",
        "",
        "1.2.3 - 2.3.4",
        "^0.1.2 || ^1.0.0",
        "npm:other@^2.0.0",
        "git+https://x/y",
        "file:../z",
        "latest",
        ">=1.0.0-beta.1",
    ];
    let versions = [
        "0.1.2",
        "1.0.0",
        "1.2.3",
        "1.9.9",
        "2.0.0",
        "2.3.4",
        "1.0.0-beta.2",
        "x",
    ];
    for spec in specs {
        for version in versions {
            let want = requirement_admits_uncached(spec, version);
            assert_eq!(requirement_admits(spec, version), want, "{spec} {version}");
            assert_eq!(
                requirement_admits(spec, version),
                want,
                "{spec} {version} (memo)"
            );
        }
    }
}

/// The work bound: a thousand asks of the same (advisory, version), the
/// walk's access pattern, cost one evaluation; a new version costs one more.
#[test]
fn a_repeated_range_question_is_evaluated_once() {
    let json = Some(
        r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.2.4"}]},{"type":"SEMVER","events":[{"introduced":"2.0.0"},{"fixed":"2.0.9"}]}]"#
            .to_string(),
    );
    let before = range_evaluations();
    for _ in 0..1_000 {
        assert_eq!(
            check_version_affected("npm", Some("1.2.3"), &json),
            (true, true)
        );
    }
    assert_eq!(range_evaluations() - before, 1);
    assert_eq!(
        check_version_affected("npm", Some("1.2.4"), &json),
        (false, true)
    );
    assert_eq!(range_evaluations() - before, 2);
}

/// The memoized version parse reads what the parse reads: outside PyPI it is
/// `OrderedVersion::parse`, asked twice (the second from the memo); PyPI's
/// PEP 440 reading is kept apart from the semver reading of the same text.
#[test]
fn the_memoized_version_parse_answers_what_the_parse_answers() {
    use fourda_depengine::version_order::OrderedVersion;
    let raws = [
        "1.2.3",
        "v1.2.3",
        "1.0.0-1",
        "1.0.0-a10",
        "2.3.0+cpu",
        "1.0.0.post1",
        "0",
        "x",
        "",
        "25.0.13+incompatible",
        "1.2",
        "4.0.0-beta.2",
    ];
    for raw in raws {
        let plain = format!("{:?}", OrderedVersion::parse(raw));
        for _ in 0..2 {
            assert_eq!(
                format!("{:?}", OrderedVersion::parse_in(raw, "npm")),
                plain,
                "{raw}"
            );
        }
        let pypi = format!("{:?}", OrderedVersion::parse_in(raw, "PyPI"));
        assert_eq!(
            format!("{:?}", OrderedVersion::parse_in(raw, "PyPI")),
            pypi,
            "{raw}"
        );
        // The npm answer is not served for PyPI, nor the reverse.
        assert_eq!(
            format!("{:?}", OrderedVersion::parse_in(raw, "npm")),
            plain,
            "{raw}"
        );
    }
    let order = OrderedVersion::parse_in("1.0.0-1", "PyPI")
        .zip(OrderedVersion::parse_in("1.0.0", "PyPI"))
        .and_then(|(post, base)| post.compare(&base));
    assert_eq!(
        order,
        Some(std::cmp::Ordering::Greater),
        "PEP 440 post release"
    );
}
