// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for dependency-change re-examination.

use super::{dependency_pin_epoch, requeue_reexaminable_items};
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
        project_paths: Vec::new(),
        project_relevance: 1.0,
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

/// Since v37 a release is graded against each project's pin, so the epoch is the
/// PINS, not the names: a version bump, a new project pinning a known package, or
/// a dependency turning direct all change what a release means.
#[test]
fn pin_epoch_is_stable_and_changes_with_any_pin() {
    let db = test_db();
    db.store_dependency("/proj/app", "sha2", Some("0.11.0"), "rust", false, None)
        .unwrap();
    let base = dependency_pin_epoch(&db);
    // Re-recording the same pin (a rescan) changes nothing.
    db.store_dependency("/proj/app", "sha2", Some("0.11.0"), "rust", false, None)
        .unwrap();
    assert_eq!(dependency_pin_epoch(&db), base, "a rescan is not a change");

    // A new project pinning an ALREADY-known package: the names are unchanged,
    // the grade of every sha2 release is not.
    db.store_dependency("/proj/core", "sha2", Some("0.10.9"), "rust", false, None)
        .unwrap();
    let with_core = dependency_pin_epoch(&db);
    assert_ne!(with_core, base, "a new project's pin is a change");

    // A version bump.
    db.store_dependency("/proj/core", "sha2", Some("0.11.0"), "rust", false, None)
        .unwrap();
    let bumped = dependency_pin_epoch(&db);
    assert_ne!(bumped, with_core, "a version bump is a change");

    // Direct -> transitive.
    {
        let conn = db.conn.lock();
        conn.execute(
            "UPDATE user_dependencies SET is_direct = 0 WHERE project_path LIKE '%core'",
            [],
        )
        .unwrap();
    }
    assert_ne!(dependency_pin_epoch(&db), bumped, "directness is a change");
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

fn verdict_of(db: &crate::db::Database, id: i64) -> (Option<i64>, Option<String>) {
    let conn = db.conn.lock();
    conn.query_row(
        "SELECT feed_relevant, feed_verdict_reason FROM source_items WHERE id=?1",
        rusqlite::params![id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap()
}

/// Make `id` a scored dependency release carrying a CURRENT-version verdict.
fn make_dependency_release(
    db: &crate::db::Database,
    id: i64,
    score: f64,
    kept: i64,
    reason: Option<&str>,
) {
    let conn = db.conn.lock();
    conn.execute(
        "UPDATE source_items SET content_type='release_notes', relevance_score=?2,
                scored_pipeline_version=37, feed_relevant=?3, feed_verdict_source='score',
                feed_verdict_reason=?4, feed_verdict_version=37
         WHERE id=?1",
        rusqlite::params![id, score, kept, reason],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO scoring_explanations (source_item_id, pipeline_version, breakdown, scored_at)
         VALUES (?1, 37, '{\"breakdown\":{\"matched_deps\":[\"sha2\"]}}', datetime('now'))",
        rusqlite::params![id],
    )
    .unwrap();
}

/// Live 2026-09-25: `crates.io: ed25519-dalek v3.0.0` was demoted `stale_version` at
/// the CURRENT version while the pin that makes it a breaking upgrade was mid-scan;
/// it re-scored 0.892 and stayed out of the feed, because the drain writes scores,
/// never verdicts, and the risen sweep only re-admits superseded-version verdicts.
/// A pin change re-queues every dependency release and withdraws its SCORE-derived
/// verdict; judgments in their own right stay.
#[test]
fn a_pin_change_withdraws_score_verdicts_on_dependency_releases_only() {
    let db = test_db();
    let ctx = ctx_with_deps(&["sha2"]);
    let stuck = insert_test_item(
        &db,
        "crates_io",
        "crate-sha2@0.11.0",
        "crates.io: sha2 v0.11.0",
        "",
    );
    make_dependency_release(&db, stuck, 0.892, 0, Some("stale_version"));
    let kept = insert_test_item(
        &db,
        "crates_io",
        "crate-sha2@0.12.0",
        "crates.io: sha2 v0.12.0",
        "",
    );
    make_dependency_release(&db, kept, 0.85, 1, None);
    let superseded = insert_test_item(
        &db,
        "crates_io",
        "crate-sha2@0.10.9",
        "crates.io: sha2 v0.10.9",
        "",
    );
    make_dependency_release(&db, superseded, 0.80, 0, Some("superseded_release"));
    let rejected = insert_test_item(
        &db,
        "crates_io",
        "crate-sha2@0.10.8",
        "crates.io: sha2 v0.10.8",
        "",
    );
    make_dependency_release(&db, rejected, 0.80, 0, Some("llm_reject"));

    let n = requeue_reexaminable_items(&db, &ctx, 0.4);

    assert_eq!(n, 4, "every dependency release is re-queued for a re-score");
    for id in [stuck, kept, superseded, rejected] {
        assert_eq!(version_of(&db, id), 0);
    }
    assert_eq!(
        verdict_of(&db, stuck),
        (None, None),
        "the stale demotion is withdrawn"
    );
    assert_eq!(
        verdict_of(&db, kept),
        (None, None),
        "a score verdict is re-judged too"
    );
    assert_eq!(
        verdict_of(&db, superseded),
        (Some(0), Some("superseded_release".to_string())),
        "a release-train decision is a judgment, not a score"
    );
    assert_eq!(
        verdict_of(&db, rejected),
        (Some(0), Some("llm_reject".to_string()))
    );
}
