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
