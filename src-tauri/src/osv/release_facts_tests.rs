// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Registry answers trimmed from the live registries (2026-10-16).

use serde_json::json;

use super::*;

#[test]
fn npm_excludes_a_deprecated_release_while_latest_is_live() {
    // registry.npmjs.org/lodash, abbreviated packument.
    let doc = json!({
        "dist-tags": {"latest": "4.18.1"},
        "versions": {
            "4.17.21": {},
            "4.17.23": {},
            "4.18.0": {"deprecated": "Bad release. Please use lodash@4.17.21 instead."},
            "4.18.1": {}
        }
    });
    let r = parse_npm(&doc).unwrap_or_default();
    assert_eq!(r.published.len(), 4);
    assert_eq!(r.excluded, ["4.18.0"]);
}

#[test]
fn npm_excludes_nothing_when_the_whole_package_is_retired() {
    let doc = json!({
        "dist-tags": {"latest": "2.88.2"},
        "versions": {
            "2.88.0": {"deprecated": "request has been deprecated"},
            "2.88.2": {"deprecated": "request has been deprecated"}
        }
    });
    let r = parse_npm(&doc).unwrap_or_default();
    assert_eq!(r.published.len(), 2);
    assert!(r.excluded.is_empty());
    assert!(parse_npm(&json!({"error": "not found"})).is_none());
}

#[test]
fn crates_excludes_yanked_versions() {
    // crates.io/api/v1/crates/h2: 0.3.17 is yanked.
    let doc = json!({"versions": [
        {"num": "0.3.18", "yanked": false},
        {"num": "0.3.17", "yanked": true},
        {"num": "0.3.16", "yanked": false}
    ]});
    let r = parse_crates(&doc).unwrap_or_default();
    assert_eq!(r.published, ["0.3.18", "0.3.17", "0.3.16"]);
    assert_eq!(r.excluded, ["0.3.17"]);
}

#[test]
fn pypi_excludes_a_release_whose_every_file_is_yanked() {
    let doc = json!({"releases": {
        "1.0.0": [{"yanked": false}],
        "1.0.1": [{"yanked": true}, {"yanked": true}],
        "1.0.2": [{"yanked": true}, {"yanked": false}],
        "1.0.3": []
    }});
    let r = parse_pypi(&doc).unwrap_or_default();
    assert_eq!(r.published, ["1.0.0", "1.0.1", "1.0.2"]);
    assert_eq!(r.excluded, ["1.0.1"]);
}

#[test]
fn the_index_keys_like_the_matcher_and_drops_month_old_facts() {
    let now = Utc::now();
    let pkg = |key: &str, age_days: i64| StoredPackage {
        ecosystem: "npm".into(),
        key: key.into(),
        name: key.into(),
        fetched_at: now - Duration::days(age_days),
        releases: RegistryReleases {
            published: vec!["1.0.0".into()],
            excluded: vec![],
        },
    };
    let idx = index(vec![pkg("lodash", 1), pkg("old", 40)], now);
    assert!(idx.contains_key(&("lodash".to_string(), "npm".to_string())));
    assert!(!idx.contains_key(&("old".to_string(), "npm".to_string())));

    // A stored document round-trips with the facts flattened in.
    let text = serde_json::to_string(&vec![pkg("lodash", 0)]).unwrap_or_default();
    assert!(text.contains("\"published\":[\"1.0.0\"]"));
    let back: Vec<StoredPackage> = serde_json::from_str(&text).unwrap_or_default();
    assert_eq!(back.len(), 1);
}

#[test]
fn only_registries_with_release_facts_are_covered() {
    assert_eq!(covered("javascript"), Some("npm"));
    assert_eq!(covered("rust"), Some("crates.io"));
    assert_eq!(covered("pip"), Some("PyPI"));
    assert_eq!(covered("go"), None);
}
