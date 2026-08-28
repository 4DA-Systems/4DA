// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Cost profile of one drain item, measured on a real corpus snapshot.
//!
//! The drain's wall-clock is `corpus_size x per_item_cost / threads`. Every
//! optimisation argument turns on which term dominates, and that cannot be read
//! off the source: the per-item DB work is a single sqlite-vec KNN
//! (`find_similar_contexts`) whose cost scales with the CONTEXT corpus, while
//! everything else in `score_item` is pure CPU over the item's own text.
//!
//! This measures the split by ablation — the KNN branch in `pipeline_v2`
//! (`if ctx.cached_context_count > 0 && has_real_embedding`) is skipped
//! entirely when the context count reads zero, so scoring the SAME items under
//! both contexts isolates the KNN's share exactly.
//!
//! It also measures how the KNN scales past the read-pool size, which is the
//! current thread ceiling in `analysis_backfill::score_chunk`.
//!
//! `#[ignore]`d because it needs a database. Point it at a SNAPSHOT, never the
//! live file:
//!
//! ```text
//! FOURDA_DB_PATH=/path/to/snapshot.db cargo test --lib \
//!     live_drain_cost_profile -- --ignored --nocapture
//! ```

use std::time::Instant;

use super::ace_context::ACEContext;
use super::dependencies::load_dependency_intelligence;
use super::{ScoringContext, ScoringInput, ScoringOptions};
use crate::db::{Database, StoredSourceItem};

const KNN_SQL: &str = "SELECT v.id, v.distance, c.source_file, c.text, c.source_type
     FROM context_vec v
     JOIN context_chunks c ON c.id = v.id
     WHERE v.embedding MATCH ?1 AND k = ?2 AND v.grounds = 1
     ORDER BY v.distance";

fn profile_n() -> usize {
    std::env::var("FOURDA_PROFILE_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(400)
}

fn decode(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// The drain's own selection, transcribed — without the licence-tier probe,
/// which would drag the settings manager into a test process.
fn load_items(path: &str, limit: usize) -> Vec<StoredSourceItem> {
    let conn = rusqlite::Connection::open(path).expect("open snapshot for selection");
    let mut stmt = conn
        .prepare(
            "SELECT id, source_type, source_id, url, title, content, content_hash,
                    embedding, created_at, last_seen, COALESCE(detected_lang,'en'),
                    feed_origin, tags, published_at
             FROM source_items
             WHERE relevance_score IS NOT NULL
             ORDER BY
                 CASE WHEN created_at >= datetime('now','-30 days') THEN 0 ELSE 1 END,
                 CASE WHEN content_type IN ('release_notes','platform_update') THEN 0 ELSE 1 END,
                 relevance_score DESC
             LIMIT ?1",
        )
        .expect("prepare selection");
    let rows = stmt
        .query_map(rusqlite::params![limit as i64], |row| {
            let blob: Vec<u8> = row.get(7)?;
            let created: String = row.get(8)?;
            let seen: String = row.get(9)?;
            Ok(StoredSourceItem {
                id: row.get(0)?,
                source_type: row.get(1)?,
                source_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get(4)?,
                content: row.get(5)?,
                content_hash: row.get(6)?,
                embedding: decode(&blob),
                created_at: crate::db::parse_datetime(created),
                last_seen: crate::db::parse_datetime(seen),
                detected_lang: row.get::<_, String>(10).unwrap_or_else(|_| "en".into()),
                feed_origin: row.get(11).ok().flatten(),
                tags: row.get(12).ok().flatten(),
                published_at: crate::db::parse_datetime_opt(
                    row.get::<_, Option<String>>(13).ok().flatten(),
                ),
            })
        })
        .expect("query selection");
    rows.filter_map(std::result::Result::ok).collect()
}

fn ctx_with_context_count(count: i64) -> ScoringContext {
    let (dependency_names, dependency_info) = load_dependency_intelligence();
    ScoringContext::builder()
        .cached_context_count(count)
        .ace_ctx(ACEContext {
            dependency_names,
            dependency_info,
            ..Default::default()
        })
        .build()
}

fn score_all(items: &[StoredSourceItem], ctx: &ScoringContext, db: &Database) -> f64 {
    let options = ScoringOptions {
        apply_freshness: true,
        apply_signals: true,
        trend_topics: vec![],
    };
    let classifier = crate::analysis::signal_classifier();
    let started = Instant::now();
    let mut sink = 0.0f32;
    for item in items {
        let parsed_tags = super::parse_tags_topics(item.tags.as_deref());
        let r = super::score_item(
            &ScoringInput {
                id: item.id as u64,
                title: &item.title,
                url: item.url.as_deref(),
                content: &item.content,
                source_type: &item.source_type,
                embedding: &item.embedding,
                created_at: Some(item.published_at.as_ref().unwrap_or(&item.created_at)),
                detected_lang: &item.detected_lang,
                source_tags: &parsed_tags,
                tags_json: item.tags.as_deref(),
                feed_origin: item.feed_origin.as_deref(),
                source_id: Some(&item.source_id),
            },
            ctx,
            db,
            &options,
            Some(classifier),
        );
        sink += r.top_score; // defeat dead-code elimination
    }
    assert!(sink >= 0.0);
    started.elapsed().as_secs_f64() * 1000.0
}

#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database snapshot"]
fn live_drain_cost_profile() {
    let Ok(path) = std::env::var("FOURDA_DB_PATH") else {
        eprintln!("FOURDA_DB_PATH not set — nothing to profile");
        return;
    };
    let db = Database::new(std::path::Path::new(&path)).expect("open snapshot");
    let chunks = db.context_count().unwrap_or(0);
    let items = load_items(&path, profile_n());
    let count = items.len();
    assert!(count > 0, "snapshot has no items to profile");

    println!("snapshot:       {path}");
    println!("items profiled: {count}");
    println!("context chunks: {chunks}");
    println!("cores:          {:?}", std::thread::available_parallelism());
    println!("read pool:      {}", db.read_pool_len());
    println!();

    // ── 1. The KNN alone, through the production entry point ──────────
    let started = Instant::now();
    let mut hits = 0usize;
    for item in &items {
        hits += db
            .find_similar_contexts(&item.embedding, 3)
            .map(|v| v.len())
            .unwrap_or(0);
    }
    let knn_ms = started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "[1] find_similar_contexts x{count}: {knn_ms:.0} ms total, {:.3} ms/item ({hits} grounding hits)",
        knn_ms / count as f64
    );

    // ── 2. Statement COMPILE cost. find_similar_contexts uses conn.prepare(),
    //        not prepare_cached(), so this is paid once per scored item.
    let started = Instant::now();
    {
        let conn = db.conn.lock();
        for _ in 0..count {
            drop(conn.prepare(KNN_SQL).expect("prepare"));
        }
    }
    let prepare_ms = started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "[2] conn.prepare() x{count} (compile only): {prepare_ms:.0} ms total, {:.3} ms/item = {:.1}% of [1]",
        prepare_ms / count as f64,
        prepare_ms / knn_ms * 100.0
    );

    // ── 3/4. Full score_item, context axis ON then structurally OFF ────
    let ctx_on = ctx_with_context_count(chunks);
    let with_knn = score_all(&items, &ctx_on, &db);
    println!(
        "[3] score_item x{count} (context axis ON):  {with_knn:.0} ms total, {:.3} ms/item",
        with_knn / count as f64
    );
    let ctx_off = ctx_with_context_count(0);
    let without_knn = score_all(&items, &ctx_off, &db);
    println!(
        "[4] score_item x{count} (context axis OFF): {without_knn:.0} ms total, {:.3} ms/item",
        without_knn / count as f64
    );

    println!();
    let knn_share = (with_knn - without_knn) / with_knn * 100.0;
    println!(
        "==> KNN share of one scored item: {knn_share:.1}%  ({:.3} ms of {:.3} ms)",
        (with_knn - without_knn) / count as f64,
        with_knn / count as f64
    );
    println!(
        "==> Pure-CPU share:               {:.1}%  ({:.3} ms)",
        100.0 - knn_share,
        without_knn / count as f64
    );
    println!(
        "==> Projected full-corpus drain at this per-item cost, {} threads: {:.1} min",
        db.read_pool_len().max(1),
        (with_knn / count as f64) * 51_500.0 / db.read_pool_len().max(1) as f64 / 60_000.0
    );

    // ── 5. How far does the KNN actually parallelise? Each thread opens
    //        its OWN read-only connection, so this measures the ceiling the
    //        read-pool cap currently hides.
    println!();
    let mut baseline = 0.0f64;
    for threads in [1usize, 2, 3, 4, 6, 8, 12, 16] {
        let per = count.div_ceil(threads);
        let started = Instant::now();
        std::thread::scope(|s| {
            for chunk in items.chunks(per) {
                let p = path.clone();
                s.spawn(move || {
                    crate::register_sqlite_vec_extension();
                    let conn = rusqlite::Connection::open_with_flags(
                        &p,
                        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                    )
                    .expect("reader");
                    let mut stmt = conn.prepare_cached(KNN_SQL).expect("prepare");
                    for item in chunk {
                        let blob: Vec<u8> = item
                            .embedding
                            .iter()
                            .flat_map(|f| f.to_le_bytes())
                            .collect();
                        let n = stmt
                            .query_map(rusqlite::params![blob, 24i64], |r| r.get::<_, i64>(0))
                            .expect("knn")
                            .count();
                        assert!(n <= 24);
                    }
                });
            }
        });
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        if threads == 1 {
            baseline = ms;
        }
        println!(
            "[5] KNN x{count} @ {threads:>2} threads: {ms:>7.0} ms ({:.3} ms/item) speedup {:.2}x",
            ms / count as f64,
            baseline / ms
        );
    }
}

/// Where does the 52 ms actually go? sqlite-vec's own published benchmark puts
/// 100k x 768 float vectors under 75 ms; this corpus is 15.6k vectors and
/// measures 52 ms, which says the cost is NOT the vector scan. This isolates
/// the four candidates: the vec0 KNN alone, the `k` over-fetch (24 to keep 3),
/// the JOIN back to `context_chunks`, and the TEXT payload that JOIN drags in.
#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database snapshot"]
fn live_knn_query_shape() {
    let Ok(path) = std::env::var("FOURDA_DB_PATH") else {
        return;
    };
    crate::register_sqlite_vec_extension();
    let conn =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open snapshot");

    let items = load_items(&path, 40);
    let blobs: Vec<Vec<u8>> = items
        .iter()
        .map(|i| i.embedding.iter().flat_map(|f| f.to_le_bytes()).collect())
        .collect();

    const BARE: &str = "SELECT id, distance FROM context_vec
                        WHERE embedding MATCH ?1 AND k = ?2 AND grounds = 1 ORDER BY distance";
    const JOIN_NO_TEXT: &str = "SELECT v.id, v.distance, c.source_type
                        FROM context_vec v JOIN context_chunks c ON c.id = v.id
                        WHERE v.embedding MATCH ?1 AND k = ?2 AND v.grounds = 1 ORDER BY v.distance";

    println!("EXPLAIN QUERY PLAN — production query:");
    {
        let mut s = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {KNN_SQL}"))
            .expect("eqp");
        let rows = s
            .query_map(rusqlite::params![&blobs[0], 24i64], |r| {
                Ok(format!(
                    "  id={} parent={} detail={}",
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(3)?
                ))
            })
            .expect("eqp rows");
        for r in rows.flatten() {
            println!("{r}");
        }
    }
    println!();

    let bench = |label: &str, sql: &str, k: i64| {
        let mut stmt = conn.prepare_cached(sql).expect("prepare");
        let started = Instant::now();
        let mut rows = 0usize;
        for b in &blobs {
            rows += stmt
                .query_map(rusqlite::params![b, k], |r| r.get::<_, i64>(0))
                .expect("run")
                .count();
        }
        println!(
            "{label:<34} k={k:<3} {:>7.2} ms/query  ({rows} rows total)",
            started.elapsed().as_secs_f64() * 1000.0 / blobs.len() as f64
        );
    };

    bench("bare vec0 KNN", BARE, 24);
    bench("bare vec0 KNN", BARE, 3);
    bench("+ JOIN, no text column", JOIN_NO_TEXT, 24);
    bench("+ JOIN, no text column", JOIN_NO_TEXT, 3);
    bench("production (JOIN + text)", KNN_SQL, 24);
    bench("production (JOIN + text)", KNN_SQL, 3);
}

/// Would a cached context-match have survived? The context corpus is
/// near-static (15,454 of 15,599 chunks written on the same day, ~0.1%/day
/// churn since), so a per-item cache of the top-K context match invalidated by
/// a context-corpus generation counter should hit ~100% during a drain. This
/// measures the miss rate directly: for each item, does its live top-3 include
/// any chunk that changed after the initial index?
#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database snapshot"]
fn live_context_cache_hit_rate() {
    let Ok(path) = std::env::var("FOURDA_DB_PATH") else {
        return;
    };
    let db = Database::new(std::path::Path::new(&path)).expect("open snapshot");
    let items = load_items(&path, profile_n());

    let conn = rusqlite::Connection::open(&path).expect("open");
    let changed: std::collections::HashSet<i64> = conn
        .prepare("SELECT id FROM context_chunks WHERE date(updated_at) > (SELECT date(MIN(updated_at)) FROM context_chunks)")
        .expect("prepare")
        .query_map([], |r| r.get::<_, i64>(0))
        .expect("query")
        .flatten()
        .collect();
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM context_chunks", [], |r| r.get(0))
        .unwrap_or(0);
    println!(
        "context chunks: {total}, changed since the initial index: {} ({:.2}%)",
        changed.len(),
        changed.len() as f64 * 100.0 / total.max(1) as f64
    );

    let mut affected = 0usize;
    let mut empty = 0usize;
    for item in &items {
        let hits = db
            .find_similar_contexts(&item.embedding, 3)
            .unwrap_or_default();
        if hits.is_empty() {
            empty += 1;
        }
        if hits.iter().any(|h| changed.contains(&h.context_id)) {
            affected += 1;
        }
    }
    let n = items.len();
    println!(
        "items whose top-3 touches a changed chunk: {affected} / {n} ({:.2}%)  [{empty} with no grounding hit]",
        affected as f64 * 100.0 / n as f64
    );
    println!(
        "==> a generation-counter cache invalidated ONLY by real context change\n    would have served {:.2}% of this drain from cache",
        100.0 - affected as f64 * 100.0 / n as f64
    );
}

/// Capability probe: does the sqlite-vec build we actually link (0.1.9) support
/// vec0 `partition key` / metadata columns? The docs describe 0.1.10-alpha, so
/// this must be measured, not assumed — filtering INSIDE the index is the only
/// way to get an exact top-K over grounding-eligible chunks without an
/// unbounded over-fetch.
#[test]
#[ignore = "capability probe — run explicitly"]
fn probe_vec0_filtering_support() {
    crate::register_sqlite_vec_extension();
    let conn = rusqlite::Connection::open_in_memory().expect("open");
    let version: String = conn
        .query_row("SELECT vec_version()", [], |r| r.get(0))
        .expect("vec_version");
    println!("vec_version: {version}");

    let part = conn.execute_batch(
        "CREATE VIRTUAL TABLE tp USING vec0(
             id integer primary key,
             grounds integer partition key,
             embedding float[4]
         );",
    );
    println!("partition key CREATE  -> {:?}", part.as_ref().err());

    let meta = conn.execute_batch(
        "CREATE VIRTUAL TABLE tm USING vec0(
             id integer primary key,
             grounds integer,
             embedding float[4]
         );",
    );
    println!("metadata column CREATE -> {:?}", meta.as_ref().err());

    if part.is_ok() {
        let v = |a: f32, b: f32, c: f32, d: f32| -> Vec<u8> {
            [a, b, c, d].iter().flat_map(|f| f.to_le_bytes()).collect()
        };
        for (id, grounds, e) in [
            (1i64, 1i64, v(1.0, 0.0, 0.0, 0.0)),
            (2, 0, v(0.99, 0.01, 0.0, 0.0)),
            (3, 1, v(0.0, 1.0, 0.0, 0.0)),
            (4, 0, v(0.98, 0.0, 0.02, 0.0)),
        ] {
            let r = conn.execute(
                "INSERT INTO tp(id, grounds, embedding) VALUES (?1, ?2, ?3)",
                rusqlite::params![id, grounds, e],
            );
            assert!(r.is_ok(), "insert {id}: {r:?}");
        }
        let q = conn.prepare(
            "SELECT id, distance FROM tp
             WHERE embedding MATCH ?1 AND k = 2 AND grounds = 1
             ORDER BY distance",
        );
        match q {
            Ok(mut stmt) => {
                let rows: Vec<(i64, f64)> = stmt
                    .query_map(rusqlite::params![v(1.0, 0.0, 0.0, 0.0)], |r| {
                        Ok((r.get(0)?, r.get(1)?))
                    })
                    .expect("query")
                    .flatten()
                    .collect();
                println!("partitioned KNN (grounds=1, k=2) -> {rows:?}");
                assert!(
                    rows.iter().all(|(id, _)| *id == 1 || *id == 3),
                    "partition filter must exclude grounds=0 rows, got {rows:?}"
                );
            }
            Err(e) => println!("partitioned KNN prepare -> {e:?}"),
        }
    }
}

/// Migration 113 on a REAL corpus: does the grounding partition rebuild, and
/// how much does the exact top-3 differ from the old k=24 over-fetch?
///
/// The over-fetch was inexact in one direction only — it could return FEWER
/// than three eligible chunks (or none) when an item's 24 nearest chunks were
/// all prose. This measures how often that happened and what it cost, so the
/// PIPELINE_VERSION bump is justified by a number rather than an argument.
///
/// Point it at a snapshot still at schema 112; the test migrates it in place.
#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a schema-112 snapshot (migrated in place)"]
fn live_grounding_partition_migration() {
    let Ok(path) = std::env::var("FOURDA_DB_PATH") else {
        return;
    };
    let n = profile_n();
    let items = load_items(&path, n);
    assert!(!items.is_empty(), "snapshot has no items");

    // ---- OLD behaviour, measured BEFORE the migration touches anything ----
    crate::register_sqlite_vec_extension();
    let old: Vec<Vec<i64>> = {
        let conn = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("open pre-migration");
        let schema: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .expect("schema_version");
        println!("snapshot schema before: {schema}");
        assert!(
            schema < 113,
            "snapshot must predate migration 113, got {schema}"
        );

        let mut stmt = conn
            .prepare(
                "SELECT v.rowid, c.source_type
                 FROM context_vec v JOIN context_chunks c ON c.id = v.rowid
                 WHERE v.embedding MATCH ?1 AND k = ?2
                 ORDER BY v.distance",
            )
            .expect("old query");
        items
            .iter()
            .map(|it| {
                let blob: Vec<u8> = it.embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
                stmt.query_map(rusqlite::params![blob, 24i64], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
                })
                .expect("run old")
                .flatten()
                .filter(|(_, st)| {
                    st.as_deref().is_some_and(|st| {
                        crate::context_admission::ContextClass::from_source_type(st)
                            .is_some_and(crate::context_admission::ContextClass::grounding_eligible)
                    })
                })
                .map(|(id, _)| id)
                .take(3)
                .collect()
            })
            .collect()
    };

    // ---- Migrate, then measure the exact partitioned answer ----
    let db = Database::new(std::path::Path::new(&path)).expect("migrate snapshot");
    {
        let conn = db.conn.lock();
        let schema: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .expect("schema_version");
        println!("snapshot schema after:  {schema}");
        assert_eq!(schema, 113, "migration 113 must have run");
        let (eligible, ineligible): (i64, i64) = conn
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM context_chunks WHERE source_type IN ('code','config')),
                   (SELECT COUNT(*) FROM context_chunks WHERE source_type NOT IN ('code','config'))",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("partition counts");
        println!(
            "context chunks: {eligible} grounding-eligible / {ineligible} not ({:.1}% of every old scan was unreturnable)",
            ineligible as f64 * 100.0 / (eligible + ineligible).max(1) as f64
        );
    }

    let mut identical = 0usize;
    let mut old_underfilled = 0usize;
    let mut old_empty = 0usize;
    let mut gained = 0usize;
    let mut differing_top1 = 0usize;
    for (it, old_ids) in items.iter().zip(&old) {
        let new_ids: Vec<i64> = db
            .find_similar_contexts(&it.embedding, 3)
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.context_id)
            .collect();
        if old_ids.len() < 3 {
            old_underfilled += 1;
        }
        if old_ids.is_empty() {
            old_empty += 1;
        }
        if new_ids.len() > old_ids.len() {
            gained += 1;
        }
        if *old_ids == new_ids {
            identical += 1;
        }
        if old_ids.first() != new_ids.first() {
            differing_top1 += 1;
        }
    }
    let total = items.len();
    println!();
    println!("items compared:                  {total}");
    println!(
        "identical top-3:                 {identical} ({:.2}%)",
        identical as f64 * 100.0 / total as f64
    );
    println!(
        "old returned FEWER than 3:       {old_underfilled} ({:.2}%)",
        old_underfilled as f64 * 100.0 / total as f64
    );
    println!("old returned NOTHING:            {old_empty}");
    println!("new returns MORE matches:        {gained}");
    println!(
        "top-1 match changed (score-affecting): {differing_top1} ({:.2}%)",
        differing_top1 as f64 * 100.0 / total as f64
    );
}

/// THE correctness gate for the context-match cache: on a real corpus, does the
/// cached answer equal what the vector index says — before AND after the context
/// corpus changes underneath it?
///
/// Unit tests pin the merge algebra. This pins it against 15,599 real 768-dim
/// vectors, where near-ties and clustering are the norm rather than a fixture.
/// A single mismatch here means some item is being scored against grounding
/// evidence that is not its nearest, silently, for ever.
///
/// Point it at a snapshot; the test migrates it and writes to it.
#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a database snapshot (migrated + mutated in place)"]
fn live_context_cache_equivalence() {
    let Ok(path) = std::env::var("FOURDA_DB_PATH") else {
        return;
    };
    let db = Database::new(std::path::Path::new(&path)).expect("open + migrate snapshot");
    let items = load_items(&path, profile_n());
    assert!(!items.is_empty());
    let k = crate::db::context_cache::CONTEXT_MATCH_K;

    let fresh = |it: &StoredSourceItem| -> Vec<(i64, i32)> {
        db.find_similar_contexts(&it.embedding, k)
            .unwrap_or_default()
            .into_iter()
            // Distances are compared at 1e-5 resolution: the merge recomputes
            // them in Rust while vec0 computes them in C, so the last f32 bit
            // is not a contract. Anything coarser would hide a real reordering.
            .map(|m| (m.context_id, (m.distance * 100_000.0).round() as i32))
            .collect()
    };
    let cached = |it: &StoredSourceItem, gen: i64| -> Option<Vec<(i64, i32)>> {
        db.cached_context_matches(it.id, gen)
            .ok()
            .flatten()
            .map(|v| {
                v.into_iter()
                    .map(|m| (m.context_id, (m.distance * 100_000.0).round() as i32))
                    .collect()
            })
    };

    // ---- Phase 1: cold cache, populated by a full recompute ----------------
    let before: Vec<Vec<(i64, i32)>> = items.iter().map(fresh).collect();
    let r1 = crate::scoring::context_cache::refresh_context_cache(
        &db,
        std::time::Duration::from_mins(15),
    );
    let gen1 = db.context_generation().expect("generation");
    println!(
        "phase 1 (cold): merged={} recomputed={} remaining={} gen={gen1} elapsed_ms={}",
        r1.merged, r1.recomputed, r1.remaining, r1.elapsed_ms
    );

    let mut miss = 0usize;
    let mut mismatch = 0usize;
    for (it, want) in items.iter().zip(&before) {
        match cached(it, gen1) {
            None => miss += 1,
            Some(got) if got == *want => {}
            Some(got) => {
                if mismatch < 5 {
                    println!("  MISMATCH item {} want={want:?} got={got:?}", it.id);
                }
                mismatch += 1;
            }
        }
    }
    println!(
        "phase 1: {} items, {miss} miss, {mismatch} mismatch",
        items.len()
    );
    assert_eq!(miss, 0, "a warmed cache must cover every scored item");
    assert_eq!(mismatch, 0, "cached matches must equal the index exactly");

    // ---- Phase 2: mutate the context corpus, then verify the MERGE ---------
    // New chunks derived from real item embeddings, so they land genuinely
    // close to some of the sampled items and must displace cached entries.
    // Routed through the real upsert so context_chunks, context_vec and the
    // change-log trigger all move together — inserting into one of them alone
    // would make this test pass or fail for the wrong reason.
    let seeded = items.len().min(40);
    for (n, it) in items.iter().take(seeded).enumerate() {
        let mut v = it.embedding.clone();
        if v.is_empty() {
            continue;
        }
        // Perturb slightly: close enough to enter a top-3, distinct enough to
        // be a new point rather than a duplicate.
        v[0] += 0.01;
        db.upsert_context(
            &format!("src/synthetic_probe_{n}.rs"),
            &format!("fn probe_{n}() {{ let x = {n}; }}"),
            &v,
        )
        .expect("seed context chunk");
    }
    let gen2 = db.context_generation().expect("generation");
    assert!(
        gen2 > gen1,
        "seeding must advance the generation ({gen1} -> {gen2})"
    );

    let after: Vec<Vec<(i64, i32)>> = items.iter().map(fresh).collect();
    let changed = before.iter().zip(&after).filter(|(b, a)| b != a).count();
    println!(
        "phase 2: seeded {seeded} chunks, gen {gen1} -> {gen2}, {changed} of {} top-3 lists moved",
        items.len()
    );
    assert!(
        changed > 0,
        "the seeded chunks must have changed SOME top-3, or this proves nothing"
    );

    let r2 = crate::scoring::context_cache::refresh_context_cache(
        &db,
        std::time::Duration::from_mins(15),
    );
    println!(
        "phase 2 refresh: merged={} recomputed={} remaining={} elapsed_ms={}",
        r2.merged, r2.recomputed, r2.remaining, r2.elapsed_ms
    );
    assert!(
        r2.merged > 0,
        "the incremental merge path must have been exercised"
    );

    let mut miss2 = 0usize;
    let mut mismatch2 = 0usize;
    for (it, want) in items.iter().zip(&after) {
        match cached(it, gen2) {
            None => miss2 += 1,
            Some(got) if got == *want => {}
            Some(got) => {
                if mismatch2 < 5 {
                    println!("  MERGE MISMATCH item {} want={want:?} got={got:?}", it.id);
                }
                mismatch2 += 1;
            }
        }
    }
    println!(
        "phase 2: {} items, {miss2} miss, {mismatch2} mismatch",
        items.len()
    );
    assert_eq!(
        miss2, 0,
        "every item must be re-cached at the new generation"
    );
    assert_eq!(
        mismatch2, 0,
        "the incremental merge must equal a fresh index query exactly"
    );
    println!();
    println!("==> cache is index-exact before AND after a context-corpus change");
}

/// Recommendation 6, measured rather than assumed: is binary quantisation the
/// right way to make a COLD context scan cheap, or is an exact in-memory scan
/// already fast enough to make the approximation unnecessary?
///
/// The cache removes the KNN from the steady state, but a full re-index blows
/// past the delta-merge cap and forces a whole-corpus recompute — 9m20s on this
/// machine. That is the path worth making cheaper. Three candidates, on the same
/// corpus and the same items:
///
/// 1. sqlite-vec, partitioned — what ships today (exact)
/// 2. Rust f32 brute force — the same maths over a contiguous in-memory matrix,
///    no vtable, no row materialisation (exact)
/// 3. binary quantise + rescore — 768 bits per vector, Hamming top-R, then exact
///    L2 over those R (APPROXIMATE)
///
/// An approximation only earns its place if it is both meaningfully faster than
/// the exact alternative AND lossless in practice, because a missed nearest
/// chunk is a silently wrong grounding axis, not a slow one.
#[test]
#[ignore = "benchmark — run explicitly against a migrated snapshot"]
fn live_cold_scan_strategy_benchmark() {
    let Ok(path) = std::env::var("FOURDA_DB_PATH") else {
        return;
    };
    let db = Database::new(std::path::Path::new(&path)).expect("open snapshot");
    let items = load_items(&path, profile_n().min(120));
    let k = crate::db::context_cache::CONTEXT_MATCH_K;

    // ---- load the grounding-eligible matrix once -------------------------
    let (ids, matrix, dims) = {
        let conn = db.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, embedding FROM context_chunks
                 WHERE source_type IN ('code','config')
                   AND embedding IS NOT NULL AND LENGTH(embedding) > 0",
            )
            .expect("prepare matrix");
        let rows: Vec<(i64, Vec<f32>)> = stmt
            .query_map([], |r| {
                let blob: Vec<u8> = r.get(1)?;
                Ok((
                    r.get::<_, i64>(0)?,
                    blob.chunks_exact(4)
                        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect::<Vec<f32>>(),
                ))
            })
            .expect("query matrix")
            .flatten()
            .collect();
        let dims = rows.first().map_or(0, |(_, v)| v.len());
        let ids: Vec<i64> = rows.iter().map(|(id, _)| *id).collect();
        let mut flat: Vec<f32> = Vec::with_capacity(rows.len() * dims);
        for (_, v) in &rows {
            if v.len() == dims {
                flat.extend_from_slice(v);
            } else {
                flat.extend(std::iter::repeat_n(f32::MAX / 4.0, dims));
            }
        }
        (ids, flat, dims)
    };
    let n = ids.len();
    assert!(n > 0 && dims > 0, "no grounding-eligible vectors");
    println!(
        "grounding matrix: {n} vectors x {dims} dims = {:.1} MB",
        (n * dims * 4) as f64 / 1e6
    );
    println!("items benchmarked: {}", items.len());
    println!();

    // ---- 1. what ships: sqlite-vec, partitioned --------------------------
    let started = Instant::now();
    let vec_top: Vec<Vec<i64>> = items
        .iter()
        .map(|it| {
            db.find_similar_contexts(&it.embedding, k)
                .unwrap_or_default()
                .into_iter()
                .map(|m| m.context_id)
                .collect()
        })
        .collect();
    let vec_ms = started.elapsed().as_secs_f64() * 1000.0 / items.len() as f64;
    println!("[1] sqlite-vec partitioned : {vec_ms:>7.2} ms/item   (exact, shipping)");

    // ---- 2. exact Rust brute force over the flat matrix ------------------
    let exact_topk = |q: &[f32]| -> Vec<i64> {
        let mut best: Vec<(f32, i64)> = Vec::with_capacity(k + 1);
        for (row, id) in ids.iter().enumerate() {
            let base = row * dims;
            let mut acc = 0.0f32;
            for d in 0..dims {
                let diff = q[d] - matrix[base + d];
                acc += diff * diff;
            }
            if best.len() < k {
                best.push((acc, *id));
                best.sort_by(|a, b| a.0.total_cmp(&b.0));
            } else if acc < best[k - 1].0 {
                best[k - 1] = (acc, *id);
                best.sort_by(|a, b| a.0.total_cmp(&b.0));
            }
        }
        best.into_iter().map(|(_, id)| id).collect()
    };
    let started = Instant::now();
    let rust_top: Vec<Vec<i64>> = items
        .iter()
        .filter(|it| it.embedding.len() == dims)
        .map(|it| exact_topk(&it.embedding))
        .collect();
    let rust_ms = started.elapsed().as_secs_f64() * 1000.0 / rust_top.len().max(1) as f64;
    println!(
        "[2] Rust f32 brute force   : {rust_ms:>7.2} ms/item   (exact, {:.1}x vs sqlite-vec)",
        vec_ms / rust_ms
    );

    // ---- 3. binary quantise + Hamming shortlist + exact rescore ----------
    let words = dims.div_ceil(64);
    let quantise = |v: &[f32]| -> Vec<u64> {
        let mut out = vec![0u64; words];
        for (d, x) in v.iter().enumerate().take(dims) {
            if *x > 0.0 {
                out[d / 64] |= 1u64 << (d % 64);
            }
        }
        out
    };
    let codes: Vec<u64> = {
        let mut c = Vec::with_capacity(n * words);
        for row in 0..n {
            c.extend(quantise(&matrix[row * dims..(row + 1) * dims]));
        }
        c
    };
    println!(
        "    binary codes: {:.2} MB ({:.0}x smaller than the f32 matrix)",
        (codes.len() * 8) as f64 / 1e6,
        (n * dims * 4) as f64 / (codes.len() * 8) as f64
    );

    for oversample in [4usize, 10, 32] {
        let shortlist = k * oversample;
        let started = Instant::now();
        let mut exact_hits = 0usize;
        let mut top1_hits = 0usize;
        let mut compared = 0usize;
        for (idx, it) in items.iter().enumerate() {
            if it.embedding.len() != dims {
                continue;
            }
            let q = quantise(&it.embedding);
            // Hamming shortlist
            let mut cand: Vec<(u32, usize)> = Vec::with_capacity(n);
            for row in 0..n {
                let mut dist = 0u32;
                for w in 0..words {
                    dist += (q[w] ^ codes[row * words + w]).count_ones();
                }
                cand.push((dist, row));
            }
            cand.select_nth_unstable_by_key(shortlist.min(n - 1), |(d, _)| *d);
            cand.truncate(shortlist.min(n));
            // Exact L2 rescore over the shortlist
            let mut rescored: Vec<(f32, i64)> = cand
                .iter()
                .map(|(_, row)| {
                    let base = row * dims;
                    let mut acc = 0.0f32;
                    for d in 0..dims {
                        let diff = it.embedding[d] - matrix[base + d];
                        acc += diff * diff;
                    }
                    (acc, ids[*row])
                })
                .collect();
            rescored.sort_by(|a, b| a.0.total_cmp(&b.0));
            let got: Vec<i64> = rescored.into_iter().take(k).map(|(_, id)| id).collect();
            let want = &vec_top[idx];
            compared += 1;
            if got == *want {
                exact_hits += 1;
            }
            if got.first() == want.first() {
                top1_hits += 1;
            }
        }
        let ms = started.elapsed().as_secs_f64() * 1000.0 / compared.max(1) as f64;
        println!(
            "[3] binary x{oversample:<2} + rescore  : {ms:>7.2} ms/item   top-3 exact {:.2}%  top-1 exact {:.2}%",
            exact_hits as f64 * 100.0 / compared as f64,
            top1_hits as f64 * 100.0 / compared as f64
        );
    }
}
