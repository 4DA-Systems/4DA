// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `source_items_fts` <-> `source_items` synchronisation.
//!
//! `source_items_fts` is an **external content** FTS5 table: SQLite stores only the
//! inverted index and reads the text back out of `source_items`, so nothing about the
//! index is maintained automatically. Every write path has to mirror itself in, and three
//! of them did not:
//!
//! - `batch_upsert_pending_source_items` had no FTS statement at all, so anything that
//!   failed embedding was unsearchable;
//! - nothing in the repository issued an FTS `'delete'`, so retention would have stranded
//!   every removed item's postings;
//! - the paths that *did* write used `INSERT OR REPLACE` **after** updating
//!   `source_items`, and on an external-content table the implicit REPLACE-delete reads
//!   the old values back from the content table — which already held the new text.
//!
//! Schema 104 replaces all of that with `trg_source_items_fts_{insert,update,delete}`.
//!
//! Every test here closes on [`Database::fts_integrity_check`], which asks FTS5 for
//! `('integrity-check', 1)`. That argument is load-bearing: on an external-content table
//! rank 0 checks only that the index's own b-trees are well formed, and it passed on the
//! founder's corrupted 247 MB corpus while rank 1 — which recomputes the index checksum
//! from `source_items` — failed.

use crate::db::Database;
use crate::test_utils::{seed_embedding, test_db};

/// Rowids the FTS index returns for `query`, in ascending order.
fn fts_match_ids(db: &Database, query: &str) -> Vec<i64> {
    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(
            "SELECT rowid FROM source_items_fts WHERE source_items_fts MATCH ?1 ORDER BY rowid",
        )
        .expect("prepare FTS match");
    let ids = stmt
        .query_map([query], |row| row.get::<_, i64>(0))
        .expect("run FTS match")
        .filter_map(Result::ok)
        .collect();
    ids
}

fn upsert(db: &Database, source_id: &str, title: &str, content: &str) -> i64 {
    db.upsert_source_item(
        "rss",
        source_id,
        None,
        title,
        content,
        &seed_embedding(source_id),
    )
    .expect("upsert_source_item")
}

/// The reported defect: `batch_upsert_pending_source_items` wrote `title`/`content` into
/// `source_items` and never indexed them. Items whose embedding failed were silently
/// missing from every keyword search, and the index disagreed with its content table.
#[test]
fn pending_embedding_items_are_indexed_for_search() {
    let db = test_db();

    let stored = db
        .batch_upsert_pending_source_items(&[(
            "hackernews".to_string(),
            "pending-1".to_string(),
            Some("https://example.invalid/1".to_string()),
            "Kubernetes operator reconcile loop".to_string(),
            "controller runtime internals and leader election".to_string(),
            "embed text".to_string(),
        )])
        .expect("batch_upsert_pending_source_items");
    assert_eq!(stored, 1);

    assert_eq!(
        fts_match_ids(&db, "kubernetes").len(),
        1,
        "an item stored by batch_upsert_pending_source_items must be findable via FTS"
    );
    assert_eq!(
        fts_match_ids(&db, "election").len(),
        1,
        "content terms must be indexed, not just the title"
    );
    db.fts_integrity_check()
        .expect("FTS index must agree with source_items after a pending upsert");
}

/// Re-fetching an item whose text changed must retire the old terms.
///
/// This is the failure the "correct" sibling had too: `INSERT OR REPLACE` ran *after* the
/// `source_items` UPDATE, so FTS5 resolved the implicit delete against the NEW text and
/// left the OLD postings in the index forever. Live consequence on the founder's corpus:
/// searching a word that had been edited out of an item still returned that item.
#[test]
fn reupserting_retires_the_terms_an_item_no_longer_contains() {
    let db = test_db();
    let id = upsert(
        &db,
        "story-1",
        "Zephyrion bearings shipped",
        "original body discussing quarkonium tolerances",
    );
    assert_eq!(fts_match_ids(&db, "zephyrion"), vec![id]);
    assert_eq!(fts_match_ids(&db, "quarkonium"), vec![id]);

    let same_id = upsert(
        &db,
        "story-1",
        "Brontide bearings recalled",
        "replacement body discussing pallium tolerances",
    );
    assert_eq!(same_id, id, "same source_id must update in place");

    assert!(
        fts_match_ids(&db, "zephyrion").is_empty(),
        "a title term that was edited out must stop matching"
    );
    assert!(
        fts_match_ids(&db, "quarkonium").is_empty(),
        "a content term that was edited out must stop matching"
    );
    assert_eq!(fts_match_ids(&db, "brontide"), vec![id]);
    assert_eq!(fts_match_ids(&db, "pallium"), vec![id]);
    db.fts_integrity_check()
        .expect("FTS index must agree with source_items after an in-place update");
}

/// Retention had never actually fired on the founder's machine. The first successful run
/// would have deleted rows from `source_items` and left every one of their postings
/// behind, because no code path anywhere issued an FTS `'delete'`.
#[test]
fn retention_delete_removes_the_items_postings() {
    let db = test_db();
    upsert(&db, "old-1", "Palladium filaments", "aged body text");
    let keep = upsert(&db, "new-1", "Tungsten filaments", "fresh body text");

    // Age the first item past the retention boundary. Note this touches `last_seen`
    // only — if the update trigger were not narrowed to title/content it would
    // needlessly reindex here.
    db.conn
        .lock()
        .execute(
            "UPDATE source_items SET last_seen = date('now', '-30 days') WHERE source_id = 'old-1'",
            [],
        )
        .expect("age the old item");

    let deleted = db.cleanup_old_items(7).expect("cleanup_old_items");
    assert_eq!(deleted, 1, "exactly the aged item is deleted");

    assert!(
        fts_match_ids(&db, "palladium").is_empty(),
        "a deleted item must leave no postings behind"
    );
    assert_eq!(
        fts_match_ids(&db, "tungsten"),
        vec![keep],
        "the surviving item must still be searchable"
    );
    db.fts_integrity_check()
        .expect("FTS index must agree with source_items after retention");
}

/// `prune_noise` and `run_maintenance` delete from `source_items` too, and neither ever
/// knew about the index. The delete trigger covers every such path by construction.
#[test]
fn every_delete_path_keeps_the_index_consistent() {
    let db = test_db();
    upsert(&db, "noise-1", "Ytterbium clickbait", "low value body");
    upsert(&db, "signal-1", "Rhenium analysis", "high value body");

    db.conn
        .lock()
        .execute(
            &format!(
                "UPDATE source_items
                    SET relevance_score = 0.01,
                        scored_pipeline_version = {},
                        created_at = datetime('now', '-90 days'),
                        last_seen = datetime('now', '-90 days')
                  WHERE source_id = 'noise-1'",
                crate::scoring::PIPELINE_VERSION
            ),
            [],
        )
        .expect("mark as noise");

    let pruned = db.prune_noise(0.2, 30, 10).expect("prune_noise");
    assert_eq!(pruned, 1, "the noise item is pruned");
    assert!(
        fts_match_ids(&db, "ytterbium").is_empty(),
        "prune_noise must not strand postings"
    );
    db.fts_integrity_check()
        .expect("FTS index must agree with source_items after prune_noise");

    db.run_maintenance(1).expect("run_maintenance");
    db.fts_integrity_check()
        .expect("FTS index must agree with source_items after run_maintenance");
}

/// The update trigger is deliberately `OF title, content` with a `WHEN old IS NOT new`
/// guard: the scoring drain stamps thousands of `relevance_score` updates per run and
/// must not reindex a single row for them.
#[test]
fn scoring_updates_do_not_disturb_the_index() {
    let db = test_db();
    let id = upsert(
        &db,
        "scored-1",
        "Osmium throughput",
        "body about throughput",
    );

    db.conn
        .lock()
        .execute(
            "UPDATE source_items
                SET relevance_score = 0.87, scored_pipeline_version = 42, summary = 'a summary'
              WHERE id = ?1",
            [id],
        )
        .expect("score the item");

    assert_eq!(
        fts_match_ids(&db, "osmium"),
        vec![id],
        "a scoring update must leave the item searchable"
    );
    db.fts_integrity_check()
        .expect("FTS index must agree with source_items after a scoring update");

    // A no-op rewrite of identical text is the common case on re-fetch and must also
    // leave the index alone.
    db.conn
        .lock()
        .execute(
            "UPDATE source_items SET title = title, content = content WHERE id = ?1",
            [id],
        )
        .expect("no-op rewrite");
    assert_eq!(fts_match_ids(&db, "osmium"), vec![id]);
    db.fts_integrity_check()
        .expect("FTS index must survive a no-op title/content rewrite");
}

/// The batch path used by the fetch pipeline, through both its insert and update branches.
#[test]
fn batch_upsert_indexes_inserts_and_updates() {
    let db = test_db();
    let row = |source_id: &str, title: &str, content: &str| {
        (
            "github".to_string(),
            source_id.to_string(),
            None,
            title.to_string(),
            content.to_string(),
            seed_embedding(source_id),
            "en".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
    };

    db.batch_upsert_source_items(&[
        row("repo-1", "Niobium release notes", "shipped niobium support"),
        row("repo-2", "Hafnium release notes", "shipped hafnium support"),
    ])
    .expect("batch insert");

    assert_eq!(fts_match_ids(&db, "niobium").len(), 1);
    assert_eq!(fts_match_ids(&db, "hafnium").len(), 1);
    db.fts_integrity_check()
        .expect("FTS index must agree with source_items after a batch insert");

    db.batch_upsert_source_items(&[row(
        "repo-1",
        "Iridium release notes",
        "shipped iridium support",
    )])
    .expect("batch update");

    assert!(
        fts_match_ids(&db, "niobium").is_empty(),
        "the batch update path must retire replaced terms too"
    );
    assert_eq!(fts_match_ids(&db, "iridium").len(), 1);
    db.fts_integrity_check()
        .expect("FTS index must agree with source_items after a batch update");
}

/// Reproduces the pre-schema-104 world — a writer that changes `source_items` with no FTS
/// write — to prove two things at once: that `fts_integrity_check` actually detects the
/// divergence (a check that cannot fail proves nothing), and that `rebuild_fts_index`
/// repairs it. This is the one-time repair the schema-104 migration performs.
#[test]
fn rebuild_repairs_an_index_that_diverged_from_source_items() {
    let db = test_db();
    let id = upsert(&db, "drift-1", "Chromium bearings", "body about chromium");
    db.fts_integrity_check().expect("clean to start with");

    {
        let conn = db.conn.lock();
        conn.execute_batch("DROP TRIGGER trg_source_items_fts_update;")
            .expect("drop the update trigger");
        conn.execute(
            "UPDATE source_items SET title = 'Vanadium bearings', content = 'body about vanadium'
              WHERE id = ?1",
            [id],
        )
        .expect("unsynchronised update");
    }

    assert!(
        db.fts_integrity_check().is_err(),
        "an index that no longer matches source_items must be reported as corrupt"
    );
    assert_eq!(
        fts_match_ids(&db, "chromium"),
        vec![id],
        "the stale term is exactly the observed symptom: it still matches"
    );

    db.rebuild_fts_index().expect("rebuild");

    db.fts_integrity_check()
        .expect("rebuild must restore agreement with source_items");
    assert!(fts_match_ids(&db, "chromium").is_empty());
    assert_eq!(fts_match_ids(&db, "vanadium"), vec![id]);
}

/// The layer the app actually queries. `hybrid_search` fuses a BM25 leg over
/// `source_items_fts` with a vector leg over `source_vec`, so a stale posting does not
/// merely fail a search — it feeds a wrong keyword rank into the fused ranking. The
/// vector leg returns the item either way, so this asserts on `bm25_rank` specifically.
#[test]
fn hybrid_search_bm25_leg_tracks_the_current_text() {
    let db = test_db();
    let id = upsert(
        &db,
        "hs-1",
        "Molybdenum pipeline",
        "body about molybdenum stages",
    );
    upsert(&db, "hs-2", "Unrelated ledger", "nothing in common here");
    let probe = seed_embedding("hs-1");

    let hit = db
        .hybrid_search("molybdenum", &probe, 10, 0.4, 0.6)
        .into_iter()
        .find(|h| h.item_id == id)
        .expect("item must be in the fused results");
    assert!(
        hit.bm25_rank.is_some(),
        "the keyword leg must match the item's current text"
    );

    upsert(&db, "hs-1", "Rhodium pipeline", "body about rhodium stages");

    let stale = db
        .hybrid_search("molybdenum", &probe, 10, 0.4, 0.6)
        .into_iter()
        .find(|h| h.item_id == id)
        .expect("the vector leg still returns the item");
    assert!(
        stale.bm25_rank.is_none(),
        "the keyword leg must not still match text the item no longer contains"
    );

    let fresh = db
        .hybrid_search("rhodium", &probe, 10, 0.4, 0.6)
        .into_iter()
        .find(|h| h.item_id == id)
        .expect("item must be in the fused results");
    assert!(
        fresh.bm25_rank.is_some(),
        "the keyword leg must match the replacement text"
    );
    db.fts_integrity_check()
        .expect("FTS index must agree with source_items after hybrid search churn");
}

/// A fresh database must come up with all three triggers installed — the whole point of
/// moving this into the schema is that no future call site can forget it.
#[test]
fn fresh_database_installs_the_fts_triggers() {
    let db = test_db();
    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master
              WHERE type = 'trigger' AND name LIKE 'trg_source_items_fts_%'
              ORDER BY name",
        )
        .expect("prepare trigger query");
    let triggers: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query triggers")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(
        triggers,
        vec![
            "trg_source_items_fts_delete".to_string(),
            "trg_source_items_fts_insert".to_string(),
            "trg_source_items_fts_update".to_string(),
        ]
    );
}
