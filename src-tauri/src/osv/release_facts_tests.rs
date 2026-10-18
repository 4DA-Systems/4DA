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
        parents: ParentFacts::default(),
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

/// Audit H1: the same abbreviated document carries each release's
/// requirement on the packages a parent was read for (registry.npmjs.org
/// express and body-parser, 2026-10-18, cut down).
#[test]
fn npm_keeps_each_releases_requirement_on_the_packages_read_for() {
    let doc = json!({
        "dist-tags": {"latest": "5.3.0"},
        "versions": {
            "4.16.4": {"dependencies": {"path-to-regexp": "0.1.7", "qs": "6.5.2"}},
            "4.22.0": {"dependencies": {"path-to-regexp": "~0.1.12", "qs": "~6.14.0"}},
            "5.0.0": {"dependencies": {"qs": "6.13.0"}},
            "5.3.0": {"peerDependencies": {"path-to-regexp": "^8.0.0"}}
        }
    });
    let children: BTreeSet<String> = ["path-to-regexp".to_string()].into();
    let p = parse_npm_parent(&doc, &children);
    assert_eq!(p.latest.as_deref(), Some("5.3.0"));
    assert!(!p.retired);
    assert_eq!(p.requires.len(), 1, "only the packages asked for");
    let h = &p.requires["path-to-regexp"];
    assert_eq!(h["4.22.0"].reqs, ["~0.1.12"]);
    assert!(h["5.0.0"].reqs.is_empty(), "5.0.0 does not depend on it");
    assert!(h["5.3.0"].peer);
    // Stored compactly and read back.
    let text = serde_json::to_string(h).unwrap_or_default();
    assert!(text.contains(r#""4.16.4":{"r":["0.1.7"]}"#), "{text}");
    assert!(text.contains(r#""5.0.0":{}"#), "{text}");
}

#[test]
fn a_retired_package_is_marked_and_a_fresh_package_missing_a_wanted_requirement_is_due() {
    let doc = json!({
        "dist-tags": {"latest": "2.88.2"},
        "versions": {"2.88.2": {"deprecated": "request has been deprecated", "dependencies": {"qs": "~6.5.2"}}}
    });
    let children: BTreeSet<String> = ["qs".to_string()].into();
    assert!(parse_npm_parent(&doc, &children).retired);

    let now = Utc::now();
    let stored: HashMap<(String, String), StoredPackage> = [(
        ("express".to_string(), "npm".to_string()),
        StoredPackage {
            ecosystem: "npm".into(),
            key: "express".into(),
            name: "express".into(),
            fetched_at: now,
            releases: RegistryReleases::default(),
            parents: parse_npm_parent(&json!({"versions": {}}), &["qs".to_string()].into()),
        },
    )]
    .into();
    let mut wanted: Wants = BTreeMap::new();
    wanted.insert(("npm".into(), "express".into()), ["qs".to_string()].into());
    assert!(
        due(wanted.clone(), &stored, now).is_empty(),
        "fresh, and qs was read"
    );
    wanted.insert(
        ("npm".into(), "express".into()),
        ["qs".to_string(), "path-to-regexp".to_string()].into(),
    );
    assert_eq!(
        due(wanted, &stored, now).len(),
        1,
        "path-to-regexp was never read"
    );
}

/// Review of #981 (N8): one parse per build, reused for 60 s, re-read after a
/// write in this process or once the copy is older; the clock is injected.
#[test]
fn the_parsed_document_is_reused_until_a_write_or_sixty_seconds() {
    let db = crate::test_utils::test_db();
    let cache = std::sync::Mutex::new(None);
    let t0 = std::time::Instant::now();
    store_for_tests(&db, vec![("npm", "a", vec!["1.0.0"], None, false, vec![])]);
    let first = book_cached(&db, &cache, t0, 0);
    assert!(first.releases("npm", "a").is_some());
    // A write another process makes (straight to kv_store, no generation
    // bump) is not seen inside the window...
    db.set_kv(KV_KEY, "[]").unwrap();
    let reused = book_cached(&db, &cache, t0 + std::time::Duration::from_secs(10), 0);
    assert!(std::sync::Arc::ptr_eq(&first, &reused));
    // ...and is past it.
    let later = book_cached(&db, &cache, t0 + std::time::Duration::from_secs(61), 0);
    assert!(later.releases("npm", "a").is_none());
    // A write in this process is seen at once.
    store_for_tests(&db, vec![("npm", "b", vec!["1.0.0"], None, false, vec![])]);
    let after = book_cached(&db, &cache, t0 + std::time::Duration::from_secs(62), 1);
    assert!(after.releases("npm", "b").is_some());
}
