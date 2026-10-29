// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The held-dependency pass over matched copies (audit 6.2.5 D-4). The rule
//! lines themselves are `fix_target.jsonl` ft-079..ft-092 (`held_vectors`);
//! these pin the host's side: no registry facts, the surfaces' words.

use serde_json::json;

use crate::osv::fix_target::{held_clean, held_sentences, line_targets};

fn express_db(project: &str) -> crate::db::Database {
    super::super::parent_release_fixture::graph(
        project,
        &[("express", "4.16.4", true)],
        &[],
        &[("GHSA-qw6h-vgh9-j6wx", "express", "0", "4.20.0")],
    )
}

fn express_line(db: &crate::db::Database) -> crate::osv::fix_target::LineTarget {
    let matches = crate::osv::matching::get_matched_advisories(db).expect("matches");
    let group: Vec<_> = matches
        .iter()
        .filter(|m| m.package_name == "express")
        .collect();
    let projects: Vec<String> = group
        .iter()
        .flat_map(|m| {
            m.dependency_instances
                .iter()
                .map(|d| d.project_path.clone())
        })
        .collect();
    line_targets(&group, &projects).remove(0)
}

/// Offline, or the npm source off: no registry facts, so the target is the
/// advisories' own and every surface says what it holds was NOT CHECKED.
#[test]
fn without_registry_facts_the_target_stays_and_is_not_checked() {
    let db = express_db("d:/held/offline");
    let line = express_line(&db);
    assert_eq!(line.target_version.as_deref(), Some("4.20.0"));
    assert!(!held_clean(&line));
    let words = held_sentences("express", std::slice::from_ref(&line));
    assert!(
        words.contains("whether the dependencies 4.20.0 exact-pins or narrowly ranges are affected was NOT CHECKED"),
        "{words}"
    );
}

/// The plan's citation and work order never call such a target clean.
#[test]
fn the_work_order_withholds_the_clean_claim() {
    let db = express_db("d:/held/order");
    store_express(&db, "6.11.0");
    let line = express_line(&db);
    // 4.20.0 pins an affected qs and no later release is published: NOT clean.
    assert_eq!(line.target_version.as_deref(), Some("4.20.0"));
    assert!(matches!(
        line.held,
        Some(fourda_depengine::held::HeldOutcome::Blocked { .. })
    ));
    let words = held_sentences("express", std::slice::from_ref(&line));
    assert!(
        words.contains(
            "express 4.16.4: 4.20.0 exact-pins qs 6.11.0 (GHSA-4mjr-xmp4-gh2g); no release on its line \
             that its own advisories leave clean clears it, so 4.20.0 is NOT a clean fix."
        ),
        "{words}"
    );
    assert!(line.clears_all_known, "its own advisories are all cleared");
}

/// A clean pin moves nothing and says nothing.
#[test]
fn a_target_that_holds_nothing_affected_says_nothing() {
    let db = express_db("d:/held/clean");
    store_express(&db, "6.16.0");
    let line = express_line(&db);
    assert_eq!(line.target_version.as_deref(), Some("4.20.0"));
    assert_eq!(line.held, None);
    assert!(held_clean(&line));
    assert_eq!(held_sentences("express", &[line]), "");
}

/// express and qs documents, express 4.20.0 exact-pinning `qs`; qs's
/// advisory stored and its name recorded as synced.
fn store_express(db: &crate::db::Database, qs: &str) {
    let docs = vec![
        (
            "express".to_string(),
            json!({"dist-tags": {"latest": "4.20.0"}, "versions": {
                "4.16.4": {"dependencies": {"qs": "6.5.2"}},
                "4.20.0": {"dependencies": {"qs": qs}}
            }}),
        ),
        (
            "qs".to_string(),
            json!({"dist-tags": {"latest": "6.16.0"}, "versions": {"6.11.0": {}, "6.16.0": {}}}),
        ),
    ];
    crate::osv::release_facts::store_docs_for_tests(db, &docs);
    super::super::parent_release_fixture::advisory(db, "GHSA-4mjr-xmp4-gh2g", "qs", "0", "6.16.0");
    crate::osv::release_facts::record_held_synced(db, &["qs".to_string()]);
}

/// Review of #1039, B1: facts read for a higher installed copy (from 4.21.0)
/// have no entry for 4.20.0, which must not read as "holds nothing".
#[test]
fn a_target_below_the_read_floor_is_not_checked() {
    let db = express_db("d:/held/floor");
    let docs = vec![(
        "express".to_string(),
        json!({"dist-tags": {"latest": "4.21.0"}, "versions": {
            "4.16.4": {"dependencies": {"qs": "6.5.2"}},
            "4.20.0": {"dependencies": {"qs": "6.11.0"}},
            "4.21.0": {"dependencies": {"qs": "6.16.0"}}
        }}),
    )];
    crate::osv::release_facts::store_docs_from_for_tests(&db, &docs, Some("4.21.0"));
    let line = express_line(&db);
    assert_eq!(line.target_version.as_deref(), Some("4.20.0"));
    match &line.held {
        Some(fourda_depengine::held::HeldOutcome::Unchecked { reason, .. }) => assert_eq!(
            reason,
            "the npm registry was read for express from 4.21.0 up; 4.20.0 is below it"
        ),
        other => panic!("want Unchecked, got {other:?}"),
    }
    assert!(!held_clean(&line));
}

/// N5: a held dependency a malware record covers is affected like any other.
#[test]
fn a_malicious_held_dependency_blocks_the_target() {
    let db = express_db("d:/held/mal");
    store_express(&db, "6.16.0");
    // qs 6.16.0 is clean of GHSA-4mjr, but a malware record covers it.
    super::super::parent_release_fixture::advisory(&db, "MAL-2026-0001", "qs", "6.16.0", "6.16.1");
    let line = express_line(&db);
    match &line.held {
        Some(fourda_depengine::held::HeldOutcome::Blocked { affected, .. }) => {
            assert_eq!(affected[0].ids, ["MAL-2026-0001"]);
        }
        other => panic!("want Blocked, got {other:?}"),
    }
}

/// N5: a held package neither installed nor in the mirror is read only once
/// a successful npm sync included it; until then, NOT CHECKED, never clean.
#[test]
fn a_held_package_the_mirror_does_not_cover_is_not_checked() {
    let db = express_db("d:/held/cover");
    let docs = vec![(
        "express".to_string(),
        json!({"dist-tags": {"latest": "4.20.0"}, "versions": {
            "4.16.4": {},
            "4.20.0": {"dependencies": {"cookie": "0.7.1"}}
        }}),
    )];
    crate::osv::release_facts::store_docs_for_tests(&db, &docs);
    let line = express_line(&db);
    assert!(
        matches!(&line.held, Some(fourda_depengine::held::HeldOutcome::Unchecked { reason, .. })
            if reason.contains("advisories of cookie are not in the local advisory mirror")),
        "{:?}",
        line.held
    );
    crate::osv::release_facts::record_held_synced(&db, &["cookie".to_string()]);
    assert_eq!(express_line(&db).held, None, "synced, no advisory: clean");
}

/// Review of #1039, S1: a raised npm release is judged itself, even when
/// the release it replaced held nothing. NodeGoat's express own target
/// 4.20.0 holds nothing here; the raise names 4.22.3, which exact-pins an
/// affected cookie: NOT a clean fix, never a silent one.
#[test]
fn a_raised_release_is_judged_for_what_it_holds() {
    let (db, project) = super::super::parent_release_fixture::nodegoat();
    let text = db
        .get_kv("osv_registry_releases")
        .ok()
        .flatten()
        .unwrap_or_default();
    let mut stored: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    for p in stored.as_array_mut().into_iter().flatten() {
        if p["name"] == "express" {
            p["held"] = json!({"read": true, "specs": {"4.22.3": {"cookie": "0.6.0"}}});
        }
    }
    db.set_kv("osv_registry_releases", &stored.to_string())
        .expect("store");
    super::super::parent_release_fixture::advisory(
        &db,
        "GHSA-pxg6-pf52-xh8x",
        "cookie",
        "0",
        "0.7.0",
    );
    crate::osv::release_facts::record_held_synced(&db, &["cookie".to_string()]);

    let matches = crate::osv::matching::get_matched_advisories(&db).unwrap_or_default();
    let group: Vec<_> = matches
        .iter()
        .filter(|m| m.package_name == "express")
        .collect();
    let own = line_targets(&group, std::slice::from_ref(&project));
    assert_eq!(own[0].target_version.as_deref(), Some("4.20.0"));
    assert_eq!(own[0].held, None, "4.20.0 holds nothing");

    let raised = crate::osv::raised_lines::raised_line_targets(&db, &group, &[project]);
    let line = &raised.lines[0];
    assert_eq!(line.target_version.as_deref(), Some("4.22.3"));
    assert!(
        matches!(&line.held, Some(fourda_depengine::held::HeldOutcome::Blocked { target, .. }) if target == "4.22.3"),
        "{:?}",
        line.held
    );
    assert!(!held_clean(line));
    assert!(
        raised.note.contains("4.22.3 is NOT a clean fix"),
        "{}",
        raised.note
    );
}
