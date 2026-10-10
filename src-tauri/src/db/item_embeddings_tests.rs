// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The single vector store (Phase 126): writes land in `source_vec` only, and
//! the chunked bulk reader agrees with vec0's own point query.

use super::*;
use crate::test_utils::{insert_test_item, seed_embedding, test_db};

#[test]
fn upsert_writes_the_vector_once_and_readers_get_it_back() {
    let db = test_db();
    let id = insert_test_item(&db, "hackernews", "one", "Title", "Body");
    let conn = db.conn.lock();
    let blob_len: i64 = conn
        .query_row(
            "SELECT length(embedding) FROM source_items WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .expect("row");
    assert_eq!(blob_len, 0, "source_items.embedding is no longer a store");
    let vec = load_item_vectors(&conn, &[id]).expect("load");
    assert_eq!(vec.get(&id), Some(&seed_embedding("hackernews:one")));
    drop(conn);

    let item = db
        .get_source_item("hackernews", "one")
        .expect("query")
        .expect("item");
    assert_eq!(item.embedding, seed_embedding("hackernews:one"));
    let by_id = db.get_source_item_by_id(id).expect("query").expect("item");
    assert_eq!(by_id.embedding.len(), crate::EMBEDDING_DIMS);
}

/// Across more than one vec0 chunk (1,024 vectors each), after deletes and
/// rewrites, the chunked reader returns exactly what the point query does.
#[test]
fn chunked_reader_matches_point_queries_across_chunks() {
    let db = test_db();
    let mut ids = Vec::new();
    for i in 0..1100 {
        ids.push(insert_test_item(&db, "rss", &format!("i{i}"), "t", "c"));
    }
    let conn = db.conn.lock();
    conn.execute("DELETE FROM source_vec WHERE rowid = ?1", [ids[5]])
        .expect("delete");
    let rewritten = seed_embedding("rewritten");
    write_item_vector(
        &conn,
        ids[1050],
        &super::super::embedding_to_blob(&rewritten),
    )
    .expect("rewrite");

    let chunked = load_chunked(&conn, &ids).expect("chunked read");
    let point = load_point(&conn, &ids).expect("point read");
    assert_eq!(chunked.len(), ids.len() - 1, "a deleted vector is absent");
    assert_eq!(chunked, point);
    assert_eq!(
        blob_to_embedding(&chunked[&ids[1050]]),
        rewritten,
        "the second chunk is read at the right offset"
    );
}

#[test]
fn a_changed_vector_invalidates_the_context_match_and_an_unchanged_one_does_not() {
    let db = test_db();
    let id = insert_test_item(&db, "hackernews", "c", "t", "c");
    let conn = db.conn.lock();
    let seed_cache = |conn: &Connection| {
        conn.execute(
            "INSERT OR REPLACE INTO item_context_cache (item_id, generation, builder) VALUES (?1, 1, 1)",
            [id],
        )
        .expect("cache row");
    };
    let cached = |conn: &Connection| -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM item_context_cache WHERE item_id = ?1",
            [id],
            |r| r.get(0),
        )
        .expect("count")
    };
    seed_cache(&conn);
    let same = super::super::embedding_to_blob(&seed_embedding("hackernews:c"));
    assert!(!write_item_vector(&conn, id, &same).expect("write"));
    assert_eq!(cached(&conn), 1, "an identical vector keeps the cache");

    let other = super::super::embedding_to_blob(&seed_embedding("other"));
    assert!(write_item_vector(&conn, id, &other).expect("write"));
    assert_eq!(cached(&conn), 0, "a new vector drops the stale match");
}

#[test]
fn pending_items_read_as_empty_vectors() {
    let db = test_db();
    db.batch_upsert_pending_source_items(&[(
        "hackernews".into(),
        "p".into(),
        None,
        "Pending".into(),
        "body".into(),
        "embed me".into(),
    )])
    .expect("pending insert");
    let items = db.get_source_items("hackernews", 10).expect("items");
    assert_eq!(items.len(), 1);
    assert!(items[0].embedding.is_empty());
}

/// Run the schema migrations (Phase 126 among them) on a COPY of a real
/// corpus and report what moved. A measuring instrument, not a gate:
///
/// ```text
/// FOURDA_PHASE126_DB=<copy>.db cargo test --lib item_embeddings::tests::phase126_on_corpus_copy -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs FOURDA_PHASE126_DB pointing at a corpus copy"]
fn phase126_on_corpus_copy() {
    let Ok(path) = std::env::var("FOURDA_PHASE126_DB") else {
        println!("PHASE126 skipped: FOURDA_PHASE126_DB unset");
        return;
    };
    crate::register_sqlite_vec_extension();
    let started = std::time::Instant::now();
    let db = super::super::Database::new(std::path::Path::new(&path)).expect("open + migrate copy");
    println!(
        "PHASE126 Database::new (incl. backup + migrations): {:.1}s",
        started.elapsed().as_secs_f64()
    );
    let conn = db.conn.lock();
    let one = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).expect(sql) };
    println!(
        "PHASE126 schema_version {}",
        one("SELECT version FROM schema_version")
    );
    println!(
        "PHASE126 items {}",
        one("SELECT COUNT(*) FROM source_items")
    );
    println!(
        "PHASE126 non-empty blobs {}",
        one("SELECT COUNT(*) FROM source_items WHERE length(embedding) > 0")
    );
    println!(
        "PHASE126 source_vec rows {}",
        one("SELECT COUNT(*) FROM source_vec")
    );
    println!(
        "PHASE126 sources off {}",
        one("SELECT COUNT(*) FROM sources WHERE enabled = 0")
    );
    let t = std::time::Instant::now();
    let items = db_items_sample(&conn);
    println!(
        "PHASE126 5000-item vector load: {} in {:.3}s",
        items,
        t.elapsed().as_secs_f64()
    );
}

fn db_items_sample(conn: &Connection) -> usize {
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM source_items ORDER BY id DESC LIMIT 5000")
        .and_then(|mut s| s.query_map([], |r| r.get(0))?.collect())
        .expect("ids");
    load_item_vectors(conn, &ids).expect("vectors").len()
}
