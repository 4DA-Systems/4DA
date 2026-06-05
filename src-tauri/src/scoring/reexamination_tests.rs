// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for dependency-change re-examination.

use super::{dep_epoch_hash, requeue_reexaminable_items};
use crate::scoring::ace_context::ACEContext;
use crate::scoring::dependencies::{extract_search_terms, DepInfo};
use crate::scoring::ScoringContext;
use crate::test_utils::{insert_test_item, test_db};

fn dep_info(name: &str) -> DepInfo {
    DepInfo {
        package_name: name.to_string(),
        version: None,
        is_dev: false,
        is_direct: true,
        search_terms: extract_search_terms(name),
        ecosystem: "rust".to_string(),
    }
}

fn ctx_with_deps(names: &[&str]) -> ScoringContext {
    let mut ace = ACEContext::default();
    for n in names {
        ace.dependency_info.insert((*n).to_string(), dep_info(n));
    }
    ScoringContext::builder().ace_ctx(ace).build()
}

/// Mark an item scored (version 5) at the given relevance + content_type.
fn set_scored(db: &crate::db::Database, id: i64, score: f64, content_type: &str) {
    let conn = db.conn.lock();
    conn.execute(
        "UPDATE source_items SET relevance_score=?2, content_type=?3, scored_pipeline_version=5 WHERE id=?1",
        rusqlite::params![id, score, content_type],
    )
    .unwrap();
}

fn version_of(db: &crate::db::Database, id: i64) -> i64 {
    let conn = db.conn.lock();
    conn.query_row(
        "SELECT scored_pipeline_version FROM source_items WHERE id=?1",
        rusqlite::params![id],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn epoch_hash_is_stable_and_changes_with_deps() {
    let a = ctx_with_deps(&["tokio"]);
    let a2 = ctx_with_deps(&["tokio"]);
    let b = ctx_with_deps(&["tokio", "axum"]);
    // Same dep set → same hash (order-independent: dedup+sort inside).
    assert_eq!(dep_epoch_hash(&a), dep_epoch_hash(&a2));
    assert_eq!(
        dep_epoch_hash(&b),
        dep_epoch_hash(&ctx_with_deps(&["axum", "tokio"]))
    );
    // Adding a dep changes the epoch → triggers re-examination.
    assert_ne!(dep_epoch_hash(&a), dep_epoch_hash(&b));
}

#[test]
fn requeues_only_buried_dep_matching_releases_and_advisories() {
    let db = test_db();
    let ctx = ctx_with_deps(&["tokio"]);

    // (1) buried release of a tracked dep → REQUEUE (the "noise becomes signal" case).
    let hit = insert_test_item(
        &db,
        "crates_io",
        "r1",
        "tokio 1.52 released",
        "async runtime update",
    );
    set_scored(&db, hit, 0.02, "release_notes");
    // (2) buried CVE matching the dep → REQUEUE (high-stakes flips on dep match).
    let cve = insert_test_item(&db, "cve", "c1", "advisory affecting tokio", "details");
    set_scored(&db, cve, 0.03, "security_advisory");
    // (3) buried discussion mentioning the dep → NOT (a dep match won't flip a discussion).
    let disc = insert_test_item(&db, "reddit", "d1", "i love tokio for async", "chat");
    set_scored(&db, disc, 0.02, "discussion");
    // (4) already-surfaced release of the dep → NOT (not buried).
    let high = insert_test_item(&db, "crates_io", "r2", "tokio 1.53 released", "x");
    set_scored(&db, high, 0.70, "release_notes");
    // (5) buried release of a dep the user does NOT track → NOT.
    let other = insert_test_item(&db, "npm", "r3", "leftpad 2.0 released", "x");
    set_scored(&db, other, 0.02, "release_notes");

    let n = requeue_reexaminable_items(&db, &ctx, 0.4);
    assert_eq!(
        n, 2,
        "only the buried dep-matching release + advisory requeued"
    );
    assert_eq!(
        version_of(&db, hit),
        0,
        "tracked-dep release requeued for re-score"
    );
    assert_eq!(version_of(&db, cve), 0, "tracked-dep advisory requeued");
    assert_eq!(version_of(&db, disc), 5, "discussion left untouched");
    assert_eq!(
        version_of(&db, high),
        5,
        "already-surfaced item left untouched"
    );
    assert_eq!(
        version_of(&db, other),
        5,
        "untracked-dep release left untouched"
    );
}

#[test]
fn no_candidates_is_a_noop() {
    let db = test_db();
    let ctx = ctx_with_deps(&["tokio"]);
    assert_eq!(requeue_reexaminable_items(&db, &ctx, 0.4), 0);
}
