// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The compact search index (Phase 127): it tracks `source_vec` through every
//! write and delete, skips turned-off sources, and ranks like the float KNN.

use super::*;
use crate::db::item_embeddings::{load_item_vectors, write_item_vector};
use crate::db::{embedding_to_blob, Database};
use crate::test_utils::{insert_test_item, test_db};

/// Deterministic unit vectors (xorshift), so the tests need no extra crate.
fn unit_vectors(n: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut s = seed | 1;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f32 / (1u64 << 53) as f32 - 0.5
    };
    (0..n)
        .map(|_| {
            let v: Vec<f32> = (0..DIMS).map(|_| next()).collect();
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            v.into_iter().map(|x| x / norm).collect()
        })
        .collect()
}

fn l2(a: &[f32], b: &[f32]) -> f64 {
    f64::from(a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f32>()).sqrt()
}

fn rows(db: &Database, table: &str, id: i64) -> i64 {
    db.conn
        .lock()
        .query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE id = ?1"),
            [id],
            |r| r.get(0),
        )
        .expect("count")
}

fn put(db: &Database, id: i64, v: &[f32]) {
    write_item_vector(&db.conn.lock(), id, &embedding_to_blob(v)).expect("write vector");
}

#[test]
fn int8_distance_is_within_a_rounding_step_of_the_float_distance() {
    let vs = unit_vectors(50, 7);
    let q = &vs[0];
    for v in &vs[1..] {
        let approx = approx_l2(q, &quantize_i8(v)).expect("packed");
        let exact = l2(q, v);
        // Cosine = 1 - d²/2 is what the UI shows; the audit measured ≤ 0.0013 on real vectors.
        let cos_err = ((1.0 - exact * exact / 2.0) - (1.0 - approx * approx / 2.0)).abs();
        assert!(cos_err < 0.005, "cosine error {cos_err}");
    }
    assert_eq!(
        approx_l2(q, &[0u8; 3]),
        None,
        "a malformed row is skipped, not misread"
    );
}

#[test]
fn the_index_follows_every_vector_write_and_item_delete() {
    let db = test_db();
    let id = insert_test_item(&db, "npm_registry", "left-pad", "left-pad 2.0", "");
    assert_eq!(
        (rows(&db, "item_vec_bits", id), rows(&db, "item_vec_i8", id)),
        (1, 1)
    );

    let replacement = unit_vectors(1, 3).remove(0);
    put(&db, id, &replacement);
    let packed: Vec<u8> = db
        .conn
        .lock()
        .query_row("SELECT v FROM item_vec_i8 WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .expect("row");
    assert_eq!(
        packed,
        quantize_i8(&replacement),
        "a re-embed rewrites the compact copy"
    );

    db.conn
        .lock()
        .execute("DELETE FROM source_items WHERE id = ?1", [id])
        .expect("delete item");
    assert_eq!(
        (rows(&db, "item_vec_bits", id), rows(&db, "item_vec_i8", id)),
        (0, 0)
    );
}

#[test]
fn a_blob_that_is_not_a_full_vector_removes_the_compact_copy() {
    let db = test_db();
    let id = insert_test_item(&db, "npm_registry", "x", "x", "");
    write(&db.conn.lock(), id, &[0u8; 12]).expect("write");
    assert_eq!(
        (rows(&db, "item_vec_bits", id), rows(&db, "item_vec_i8", id)),
        (0, 0)
    );
}

#[test]
fn knn_ranks_like_the_float_index() {
    let db = test_db();
    let vs = unit_vectors(1_500, 42);
    let mut ids = Vec::new();
    for (i, v) in vs.iter().enumerate() {
        let id = insert_test_item(&db, "crates_io", &format!("c{i}"), "t", "");
        put(&db, id, v);
        ids.push(id);
    }
    let conn = db.conn.lock();
    let k = 20;
    let (mut hit, mut total) = (0usize, 0usize);
    for q in unit_vectors(10, 99) {
        let mut exact: Vec<(i64, f64)> = ids
            .iter()
            .zip(&vs)
            .map(|(&id, v)| (id, l2(&q, v)))
            .collect();
        exact.sort_by(|a, b| a.1.total_cmp(&b.1));
        let got = knn(&conn, &q, k).expect("knn").expect("index present");
        assert_eq!(got.len(), k);
        assert!(got.windows(2).all(|w| w[0].1 <= w[1].1), "nearest first");
        let want: std::collections::HashSet<i64> = exact.iter().take(k).map(|e| e.0).collect();
        hit += got.iter().filter(|g| want.contains(&g.0)).count();
        total += k;
        for (id, d) in &got {
            let true_d = exact
                .iter()
                .find(|e| e.0 == *id)
                .map(|e| e.1)
                .expect("known id");
            assert!((d - true_d).abs() < 0.02, "distance {d} vs {true_d}");
        }
    }
    let recall = hit as f64 / total as f64;
    // Random vectors are the hard case for sign bits (real embeddings measured 0.99).
    assert!(recall >= 0.9, "recall@{k} = {recall}");
}

#[test]
fn turned_off_sources_are_skipped_in_the_scan_and_k_still_comes_back() {
    let db = test_db();
    {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO sources (source_type, name, enabled) VALUES ('hackernews', 'HN', 0)",
            [],
        )
        .expect("source");
    }
    let vs = unit_vectors(300, 5);
    for (i, v) in vs.iter().enumerate() {
        // 9 in 10 items are a turned-off interest, as on the founder's corpus.
        let st = if i % 10 == 0 {
            "npm_registry"
        } else {
            "hackernews"
        };
        let id = insert_test_item(&db, st, &format!("i{i}"), "t", "");
        put(&db, id, v);
    }
    let conn = db.conn.lock();
    let got = knn(&conn, &vs[1], 25).expect("knn").expect("index");
    assert_eq!(got.len(), 25, "the 30 enabled items fill k");
    for (id, _) in got {
        let st: String = conn
            .query_row(
                "SELECT source_type FROM source_items WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .expect("row");
        assert_eq!(st, "npm_registry");
    }
}

#[test]
fn an_empty_index_hands_back_to_the_float_knn() {
    let db = test_db();
    let q = unit_vectors(1, 1).remove(0);
    assert_eq!(knn(&db.conn.lock(), &q, 10).expect("knn"), None);
}

#[test]
fn backfill_indexes_every_stored_vector_and_skips_items_without_one() {
    let db = test_db();
    let a = insert_test_item(&db, "pypi", "a", "a", "");
    let b = insert_test_item(&db, "pypi", "b", "b", "");
    let conn = db.conn.lock();
    conn.execute("DELETE FROM source_vec WHERE rowid = ?1", [b])
        .expect("drop b's vector");
    conn.execute_batch("DELETE FROM item_vec_bits; DELETE FROM item_vec_i8;")
        .expect("clear");

    let (indexed, without) = create_and_backfill(&conn).expect("backfill");
    assert_eq!((indexed, without), (1, 1));
    let stored = load_item_vectors(&conn, &[a]).expect("load");
    let packed: Vec<u8> = conn
        .query_row("SELECT v FROM item_vec_i8 WHERE id = ?1", [a], |r| r.get(0))
        .expect("a indexed");
    assert_eq!(packed, quantize_i8(&stored[&a]));
}

/// Live check on a COPY of a real corpus (opening it migrates it to Phase 127;
/// never point this at the live file):
///
/// ```text
/// FOURDA_QUANT_LIVE_DB=<copy of data/4da.db> \
///   cargo test --lib quant_live -- --ignored --nocapture --test-threads=1
/// ```
///
/// Prints the backfill cost, table sizes, and per-query recall and latency of
/// the compact leg against the exact answer and against the float vec0 leg it
/// replaces (k × 4 over-fetch, then the enabled filter).
#[test]
#[ignore = "needs a copy of a real corpus in FOURDA_QUANT_LIVE_DB"]
fn quant_live_recall_and_latency_on_a_snapshot() {
    use std::time::Instant;
    let Ok(path) = std::env::var("FOURDA_QUANT_LIVE_DB") else {
        return;
    };
    crate::register_sqlite_vec_extension();
    let started = Instant::now();
    let db = Database::new(std::path::Path::new(&path)).expect("open snapshot");
    println!("open + migrate: {:?}", started.elapsed());
    let conn = db.conn.lock();
    let count = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).expect(sql);
    println!(
        "schema {} | items {} | bits rows {} | i8 rows {} | bits MB {:.1} | i8 MB {:.1}",
        count("SELECT MAX(version) FROM schema_version"),
        count("SELECT COUNT(*) FROM source_items"),
        count("SELECT COUNT(*) FROM item_vec_bits"),
        count("SELECT COUNT(*) FROM item_vec_i8"),
        count("SELECT SUM(length(bits)) FROM item_vec_bits") as f64 / 1_048_576.0,
        count("SELECT SUM(length(v)) FROM item_vec_i8") as f64 / 1_048_576.0,
    );
    let enabled: Vec<i64> = conn
        .prepare(
            "SELECT id FROM source_items
             WHERE source_type NOT IN (SELECT source_type FROM sources WHERE enabled = 0) ORDER BY id",
        )
        .expect("prepare")
        .query_map([], |r| r.get(0))
        .expect("query")
        .collect::<rusqlite::Result<_>>()
        .expect("ids");
    let vectors = load_item_vectors(&conn, &enabled).expect("vectors");
    let corpus: Vec<(i64, &Vec<f32>)> = enabled
        .iter()
        .filter_map(|id| vectors.get(id).map(|v| (*id, v)))
        .collect();
    println!("enabled items with a vector: {}", corpus.len());

    let k = 90; // hybrid_search: (limit 30 * 3).max(50)
    let step = (corpus.len() / 40).max(1);
    let (mut r10, mut r30, mut r90, mut n) = (0.0, 0.0, 0.0, 0.0);
    let (mut quant_ms, mut float_ms, mut float_short) = (Vec::new(), Vec::new(), 0usize);
    let mut cos_err: f64 = 0.0;
    for (_, q) in corpus.iter().step_by(step).take(40) {
        let mut exact: Vec<(i64, f64)> = corpus.iter().map(|(id, v)| (*id, l2(q, v))).collect();
        exact.sort_by(|a, b| a.1.total_cmp(&b.1));
        let t = Instant::now();
        let got = knn(&conn, q, k).expect("knn").expect("index");
        quant_ms.push(t.elapsed().as_secs_f64() * 1e3);

        let t = Instant::now();
        let float_rows: Vec<i64> = conn
            .prepare(
                "SELECT sv.rowid FROM source_vec sv JOIN source_items si ON si.id = sv.rowid
                 WHERE sv.embedding MATCH ?1 AND k = ?2
                   AND si.source_type NOT IN (SELECT source_type FROM sources WHERE enabled = 0)",
            )
            .expect("prepare")
            .query_map(params![embedding_to_blob(q), (k * 4) as i64], |r| r.get(0))
            .expect("float knn")
            .collect::<rusqlite::Result<_>>()
            .expect("rows");
        float_ms.push(t.elapsed().as_secs_f64() * 1e3);
        if float_rows.len() < k {
            float_short += 1;
        }

        let overlap = |m: usize| {
            let want: std::collections::HashSet<i64> = exact.iter().take(m).map(|e| e.0).collect();
            got.iter().take(m).filter(|g| want.contains(&g.0)).count() as f64 / m as f64
        };
        r10 += overlap(10);
        r30 += overlap(30);
        r90 += overlap(90);
        n += 1.0;
        for (id, d) in &got {
            if let Some(e) = exact.iter().find(|e| e.0 == *id) {
                cos_err = cos_err.max(((1.0 - e.1 * e.1 / 2.0) - (1.0 - d * d / 2.0)).abs());
            }
        }
    }
    let stats = |v: &[f64]| {
        let mut s = v.to_vec();
        s.sort_by(f64::total_cmp);
        (
            s.iter().sum::<f64>() / s.len() as f64,
            s[s.len() / 2],
            s[s.len() - 1],
        )
    };
    println!(
        "queries {n} | recall@10 {:.3} @30 {:.3} @90 {:.3} | max cosine error {cos_err:.4}",
        r10 / n,
        r30 / n,
        r90 / n
    );
    println!("compact leg ms (mean, median, max): {:?}", stats(&quant_ms));
    println!(
        "float leg   ms (mean, median, max): {:?} | short of k in {float_short}/{n} queries",
        stats(&float_ms)
    );
}
