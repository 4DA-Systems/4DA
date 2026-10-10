// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Phase 126 (AD-054): interests opt-in, and one vector store.

use rusqlite::params;

use super::Database;
use crate::test_utils::{insert_test_item, seed_embedding, test_db};

fn enabled(db: &Database, source_type: &str) -> Option<bool> {
    db.conn
        .lock()
        .query_row(
            "SELECT enabled FROM sources WHERE source_type = ?1",
            [source_type],
            |r| r.get::<_, i64>(0),
        )
        .ok()
        .map(|v| v != 0)
}

#[test]
fn the_schema_reaches_126_and_the_reembed_trigger_is_gone() {
    let db = test_db();
    let conn = db.conn.lock();
    let version: i64 = conn
        .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
        .expect("version");
    assert_eq!(version, 126);
    let trigger: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = 'item_context_cache_reembed'",
            [],
            |r| r.get(0),
        )
        .expect("trigger count");
    assert_eq!(trigger, 0);
}

/// A new install: registration seeds stack sources on and interests off,
/// and a user's later choice survives the next boot's registration.
#[test]
fn registration_seeds_by_class_and_keeps_the_users_choice() {
    let db = test_db();
    db.register_source("osv", "OSV.dev").expect("register");
    db.register_source("hackernews", "Hacker News")
        .expect("register");
    assert_eq!(enabled(&db, "osv"), Some(true));
    assert_eq!(enabled(&db, "hackernews"), Some(false));
    assert!(!db.is_source_enabled("hackernews"));
    assert!(!db.is_source_enabled("never_registered_interest"));
    assert!(
        db.is_source_enabled("crates_io"),
        "an unregistered stack source is on"
    );

    assert!(db
        .set_source_enabled("hackernews", "Hacker News", true)
        .expect("set"));
    db.register_source("hackernews", "Hacker News")
        .expect("re-register");
    assert_eq!(
        enabled(&db, "hackernews"),
        Some(true),
        "re-registration keeps the choice"
    );

    assert!(
        !db.set_source_enabled("osv", "OSV.dev", false).expect("set"),
        "a stack source cannot be turned off"
    );
    assert_eq!(enabled(&db, "osv"), Some(true));
    assert!(db.disabled_source_types().expect("disabled").is_empty());
}

/// An existing install: every interest is turned off, stack sources stay on.
#[test]
fn existing_installs_turn_interests_off() {
    let db = test_db();
    {
        let conn = db.conn.lock();
        for st in ["hackernews", "rss", "github", "crates_io", "osv", "cve"] {
            conn.execute(
                "INSERT INTO sources (source_type, name, enabled) VALUES (?1, ?1, 1)",
                [st],
            )
            .expect("seed");
        }
        let changed = Database::turn_interests_off(&conn).expect("migrate");
        assert_eq!(changed, 3);
        assert_eq!(
            Database::turn_interests_off(&conn).expect("again"),
            0,
            "idempotent"
        );
    }
    for st in ["hackernews", "rss", "github"] {
        assert_eq!(enabled(&db, st), Some(false), "{st}");
    }
    for st in ["crates_io", "osv", "cve"] {
        assert_eq!(enabled(&db, st), Some(true), "{st}");
    }
}

/// The BLOB column is emptied; a vector only the BLOB held moves into
/// `source_vec`; an unindexable one sends its row back to `pending`; the
/// context-match cache survives the emptying.
#[test]
fn embeddings_move_to_one_store_without_wiping_the_context_cache() {
    let db = test_db();
    let kept = insert_test_item(&db, "hackernews", "kept", "Kept", "body");
    let conn = db.conn.lock();
    // A pre-126 row: vector in both stores, and a cached context match.
    let kept_blob = super::embedding_to_blob(&seed_embedding("hackernews:kept"));
    conn.execute(
        "UPDATE source_items SET embedding = ?1 WHERE id = ?2",
        params![kept_blob, kept],
    )
    .expect("old-style blob");
    conn.execute(
        "INSERT INTO item_context_cache (item_id, generation, builder) VALUES (?1, 1, 1)",
        [kept],
    )
    .expect("cache row");
    // A BLOB-only vector, and an unindexable BLOB.
    let orphan_blob = super::embedding_to_blob(&seed_embedding("orphan"));
    conn.execute(
        "INSERT INTO source_items (source_type, source_id, title, content_hash, embedding)
         VALUES ('rss', 'orphan', 'Orphan', 'h1', ?1)",
        [&orphan_blob],
    )
    .expect("orphan");
    let orphan = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO source_items (source_type, source_id, title, content_hash, embedding)
         VALUES ('rss', 'short', 'Short', 'h2', X'00000000')",
        [],
    )
    .expect("short");
    let short = conn.last_insert_rowid();
    // Old trigger back in place, as on a real pre-126 database.
    conn.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS item_context_cache_reembed
             AFTER UPDATE OF embedding ON source_items
             WHEN old.embedding IS NOT new.embedding
         BEGIN
             DELETE FROM item_context_cache WHERE item_id = new.id;
         END;",
    )
    .expect("old trigger");

    let (moved, parked, cleared) = Database::store_embeddings_once(&conn).expect("migrate");
    assert_eq!((moved, parked, cleared), (1, 1, 3));
    assert_eq!(
        Database::store_embeddings_once(&conn).expect("again"),
        (0, 0, 0),
        "idempotent"
    );

    let nonempty: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM source_items WHERE length(embedding) > 0",
            [],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(nonempty, 0);
    let cache: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM item_context_cache WHERE item_id = ?1",
            [kept],
            |r| r.get(0),
        )
        .expect("cache");
    assert_eq!(cache, 1, "emptying the BLOB must not wipe the cache");
    let vectors =
        super::item_embeddings::load_item_vectors(&conn, &[kept, orphan, short]).expect("vectors");
    assert_eq!(vectors.get(&orphan), Some(&seed_embedding("orphan")));
    assert!(vectors.contains_key(&kept));
    assert!(!vectors.contains_key(&short));
    let status: String = conn
        .query_row(
            "SELECT embedding_status FROM source_items WHERE id = ?1",
            [short],
            |r| r.get(0),
        )
        .expect("status");
    assert_eq!(status, "pending");
}

/// A source the user turned off is invisible to every selection that feeds
/// embedding, scoring, judging and the feed; turning it back on restores it,
/// verdict included.
#[test]
fn readers_skip_a_turned_off_source_and_turning_it_on_restores_it() {
    let db = test_db();
    let hn = insert_test_item(&db, "hackernews", "h1", "HN story", "body");
    let npm = insert_test_item(&db, "npm_registry", "n1", "left-pad 2.0.0", "body");
    db.register_source("hackernews", "Hacker News")
        .expect("register");
    db.register_source("npm_registry", "npm Registry")
        .expect("register");
    {
        let conn = db.conn.lock();
        conn.execute(
            "UPDATE source_items SET feed_relevant = 1, scored_pipeline_version = 0 WHERE id = ?1",
            [hn],
        )
        .expect("curate");
    }

    let ids =
        |items: Vec<super::StoredSourceItem>| -> Vec<i64> { items.iter().map(|i| i.id).collect() };
    let window = ids(db
        .get_items_balanced_by_source(24, 50, 100)
        .expect("window"));
    assert_eq!(window, vec![npm], "hackernews is off on a new install");
    assert_eq!(
        ids(db
            .get_items_since_timestamp("2000-01-01", 100)
            .expect("diff")),
        vec![npm]
    );
    assert_eq!(
        ids(db.get_freshness_refresh_batch(24, 100).expect("fresh")),
        vec![npm]
    );
    assert_eq!(
        ids(db.get_unscored_backlog_chunk(100).expect("backlog")),
        vec![npm]
    );
    assert_eq!(db.count_unscored_backlog().expect("count"), 1);

    // The persist boundary writes no verdict for a source that is off — not
    // even a first one, which it would otherwise apply at once.
    let unjudged = insert_test_item(&db, "hackernews", "h2", "Another HN story", "body");
    db.persist_feed_verdicts_with_reasons(
        &[(unjudged, true, super::VerdictSource::Score, None)],
        crate::scoring::PIPELINE_VERSION,
    )
    .expect("persist");
    let verdict: Option<i64> = db
        .conn
        .lock()
        .query_row(
            "SELECT feed_relevant FROM source_items WHERE id = ?1",
            [unjudged],
            |r| r.get(0),
        )
        .expect("verdict");
    assert_eq!(
        verdict, None,
        "no verdict is written for a source that is off"
    );

    db.set_source_enabled("hackernews", "Hacker News", true)
        .expect("on");
    let mut window = ids(db
        .get_items_balanced_by_source(24, 50, 100)
        .expect("window"));
    window.sort_unstable();
    assert_eq!(window, vec![hn, npm, unjudged]);
}
