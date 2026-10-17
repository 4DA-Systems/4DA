// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Registry answers trimmed from the live registries (2026-10-16/17).

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

const OLD_GLOB: &str = "Old versions of glob are not supported, and contain widely publicized \
                        security vulnerabilities, which have been fixed in the current version.";

/// Review of #976 (2026-10-17): glob deprecates every release below 12 with
/// one "please upgrade" notice. That is not a withdrawn release: 10.5.0 (the
/// fix for GHSA-5j98-mcp5-4vw2 on the 10.x line) must stay a target.
#[test]
fn npm_keeps_a_release_whose_whole_major_is_deprecated() {
    let doc = json!({
        "dist-tags": {"latest": "13.0.6"},
        "versions": {
            "10.4.5": {"deprecated": OLD_GLOB},
            "10.5.0": {"deprecated": OLD_GLOB},
            "11.1.0": {"deprecated": OLD_GLOB},
            "12.0.0": {},
            "13.0.6": {}
        }
    });
    let r = parse_npm(&doc).unwrap_or_default();
    assert!(r.excluded.is_empty(), "{:?}", r.excluded);
}

/// tar deprecates 7.0.0..7.5.7 and every 6.x; 7.5.8 is clean. Only the 7.x
/// releases with a clean later 7.x are withdrawn; 6.2.1 stays.
#[test]
fn npm_excludes_a_deprecated_release_only_when_its_line_has_a_clean_successor() {
    let doc = json!({
        "dist-tags": {"latest": "7.5.22"},
        "versions": {
            "6.2.1": {"deprecated": "Old versions of tar are not supported"},
            "7.5.7": {"deprecated": "Old versions of tar are not supported"},
            "7.5.8": {},
            "7.5.22": {},
            "8.0.0-rc.1": {}
        }
    });
    let r = parse_npm(&doc).unwrap_or_default();
    assert_eq!(r.excluded, ["7.5.7"]);
}

#[test]
fn npm_caret_line_for_0x_is_the_minor_and_prereleases_do_not_count() {
    let doc = json!({
        "dist-tags": {"latest": "0.3.0"},
        "versions": {
            "0.2.1": {"deprecated": "bad"},
            "0.2.2-beta.1": {},
            "0.3.0": {},
            "0.4.1": {"deprecated": "bad"},
            "0.4.2": {}
        }
    });
    let r = parse_npm(&doc).unwrap_or_default();
    // 0.2.1 has no clean stable 0.2.x after it (0.3.0 is another caret line,
    // 0.2.2-beta.1 a pre-release); 0.4.1 has 0.4.2.
    assert_eq!(r.excluded, ["0.4.1"]);
}

#[test]
fn a_disabled_registry_source_is_never_contacted() {
    let wanted = vec![
        ("npm".to_string(), "lodash".to_string()),
        ("crates.io".to_string(), "h2".to_string()),
        ("PyPI".to_string(), "jinja2".to_string()),
    ];
    let kept = gate(wanted.clone(), |s| s != "npm_registry");
    assert_eq!(kept, wanted[1..].to_vec());
    assert!(gate(wanted.clone(), |_| false).is_empty());
    assert_eq!(gate(wanted.clone(), |_| true), wanted);
}

#[test]
fn a_disabled_source_row_gates_the_database_path_too() {
    let db = crate::test_utils::test_db();
    {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO sources (source_type, name, enabled) VALUES ('crates_io', 'crates.io', 0)
             ON CONFLICT(source_type) DO UPDATE SET enabled = 0",
            [],
        )
        .unwrap();
    }
    let wanted = vec![
        ("crates.io".to_string(), "h2".to_string()),
        ("npm".to_string(), "lodash".to_string()),
    ];
    let kept = gate(wanted, |s| db.is_source_enabled(s));
    assert_eq!(kept, [("npm".to_string(), "lodash".to_string())]);
}

/// Review of #976 (S2): the morning audit (G4) must accept a target only
/// the registry names (lodash 4.18.1 is in no advisory and no feed title).
#[test]
fn the_morning_audit_accepts_a_release_the_registry_lists() {
    let db = crate::test_utils::test_db();
    let fetched_at = Utc::now().to_rfc3339();
    db.set_kv(
        KV_KEY,
        &format!(
            r#"[{{"ecosystem":"npm","key":"lodash","name":"lodash","fetched_at":"{fetched_at}","published":["4.18.0","4.18.1"],"excluded":["4.18.0"]}}]"#
        ),
    )
    .unwrap();
    let conn = db.conn.lock();
    assert!(registry_lists(&conn, "lodash", "4.18.1"));
    assert!(registry_lists(&conn, "Lodash", "4.18.1"));
    assert!(!registry_lists(&conn, "lodash", "4.19.0"));
    assert!(!registry_lists(&conn, "underscore", "4.18.1"));
    assert!(crate::morning_audit::version_is_true(
        &conn, "lodash", "4.18.1"
    ));
    assert!(!crate::morning_audit::version_is_true(
        &conn, "lodash", "4.19.0"
    ));
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
