// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Cold vs warm search on a real corpus and the real local Ollama.
//!
//! Run on a SNAPSHOT (opening it migrates it; never the live file), with the same
//! embedding model the app uses, already pulled. Nothing here loads, unloads or
//! pulls a model; the background batches it sends are the app's own embed path.
//!
//! ```text
//! FOURDA_SEARCH_LIVE=1 FOURDA_DB_PATH=<snapshot> FOURDA_DATA_DIR=<scratch> \
//!   cargo test --lib live_search_cold_vs_warm -- --ignored --nocapture --test-threads=1
//! ```

use std::time::{Duration, Instant};

use super::*;
use crate::natural_language_search::natural_language_search_timing::{EmbedSource, SearchTimings};

fn line(label: &str, total_ms: u128, t: &SearchTimings, top: &str) {
    let h = &t.hybrid;
    println!(
        "{label:<34} total={total_ms:>6} ms | parse={} ctx={} embed={} ({}, load={:?}) ace={} \
         conn_wait={} exact={} fts={} knn={} fuse={} rank={} | top: {top}",
        t.parse_ms,
        t.context_ms,
        t.embed_ms,
        t.embed_source.as_str(),
        t.embed_load_ms,
        t.ace_ms,
        h.conn_wait_ms,
        h.exact_ms,
        h.fts_ms,
        h.knn_ms,
        h.fuse_ms,
        t.rank_ms,
    );
}

async fn search(label: &str, q: &str) -> (Vec<i64>, SearchTimings, bool) {
    let began = Instant::now();
    let (result, timings) = crate::natural_language_search::run_search(q.to_string(), true)
        .await
        .expect("search on snapshot");
    let top = result
        .items
        .first()
        .and_then(|i| i.file_name.clone())
        .unwrap_or_default();
    line(label, began.elapsed().as_millis(), &timings, &top);
    let ids = result.items.iter().map(|i| i.id).collect();
    (ids, timings, result.semantic_pending)
}

/// 32 texts of the size the background drain embeds (title twice + body).
fn bulk_batch(tag: usize) -> Vec<String> {
    (0..32)
        .map(|i| {
            format!(
                "Release {tag}.{i}: scheduler rework\n\nRelease {tag}.{i}: scheduler rework\n\n{}",
                "The runtime now steals work across worker threads and the blocking pool \
                 grows on demand; connection pools and query planners see fewer stalls. "
                    .repeat(6)
            )
        })
        .collect()
}

/// Queue background batches through the app's own embed path, the way a
/// fetch/scoring cycle (or a second 4DA process) does.
fn spawn_bulk(n: usize, tag: usize) -> Vec<tokio::task::JoinHandle<u128>> {
    (0..n)
        .map(|k| {
            tokio::spawn(async move {
                let t = Instant::now();
                let _ = crate::embeddings::embed_texts(&bulk_batch(tag * 10 + k)).await;
                t.elapsed().as_millis()
            })
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs FOURDA_SEARCH_LIVE=1, FOURDA_DB_PATH=<snapshot> and a local Ollama"]
async fn live_search_cold_vs_warm() {
    if std::env::var("FOURDA_SEARCH_LIVE").is_err() || std::env::var("FOURDA_DB_PATH").is_err() {
        return;
    }
    let t = Instant::now();
    let db = crate::get_database().expect("open snapshot");
    println!("open database: {} ms", t.elapsed().as_millis());

    // FOURDA_SEARCH_PREWARM=1: what startup / palette-open does, before the first search.
    if std::env::var("FOURDA_SEARCH_PREWARM").is_ok() {
        let t = Instant::now();
        let r = run_prewarm().await;
        println!(
            "pre-warm: {} ms (embed {} ms load {:?}, ace {} ms, db+knn {} ms)",
            t.elapsed().as_millis(),
            r.embed_ms,
            r.embed_load_ms,
            r.ace_ms,
            r.db_ms
        );
    }

    // 1. First search in a fresh process, then warm repeats.
    let (warm_ids, _, _) = search("first search (fresh process)", "rusqlite").await;
    for i in 1..=3 {
        search(&format!("warm repeat {i}"), "rusqlite").await;
    }
    search("warm, new text (cache miss)", "sqlite vector search").await;

    // 2. The incident: the query embed queues behind background batches in
    //    Ollama's single slot (OLLAMA_NUM_PARALLEL=1).
    let bulk = spawn_bulk(2, 1);
    tokio::time::sleep(Duration::from_millis(150)).await;
    let t = Instant::now();
    let old = embed_query_text_within("tokio blocking pool", Duration::from_mins(2)).await;
    println!(
        "BEFORE (unbounded embed wait) under 2 queued batches: query embed {} ms ({})",
        t.elapsed().as_millis(),
        old.source.as_str()
    );
    for (k, h) in bulk.into_iter().enumerate() {
        println!("  background batch {k}: {} ms", h.await.unwrap_or(0));
    }

    let bulk = spawn_bulk(2, 2);
    tokio::time::sleep(Duration::from_millis(150)).await;
    let (kw_ids, timings, pending) = search("AFTER under 2 queued batches", "tauri updater").await;
    assert!(
        pending == (timings.embed_source == EmbedSource::TimedOut),
        "semantic_pending mirrors a timed-out embed"
    );
    let landed = Instant::now();
    let (full_ids, _, _) = loop {
        let r = search("  retry (palette semantic_pending)", "tauri updater").await;
        if !r.2 || landed.elapsed() > Duration::from_mins(1) {
            break r;
        }
        tokio::time::sleep(Duration::from_millis(1_500)).await;
    };
    println!(
        "  semantic merged after {} ms",
        landed.elapsed().as_millis()
    );
    let kept = kw_ids.iter().filter(|id| full_ids.contains(id)).count();
    println!(
        "  keyword-first answer: {} ids, {kept} of them in the full answer",
        kw_ids.len()
    );
    for h in bulk {
        let _ = h.await;
    }
    let (again, _, _) = search("warm again after the load", "rusqlite").await;
    assert_eq!(again.first(), warm_ids.first(), "same top result warm");

    // 3. A scoring drain holds every pooled reader; the writer is busy too.
    let readers: Vec<_> = (0..db.read_pool_len()).map(|_| db.read_conn()).collect();
    let writer = db.conn.lock();
    let pool_free = db.read_pool_len() - readers.len();
    std::thread::scope(|s| {
        let probe = s.spawn(|| {
            let t = Instant::now();
            drop(db.interactive_conn());
            t.elapsed().as_millis()
        });
        println!(
            "pool exhausted ({pool_free} free) + writer held: interactive_conn wait {} ms",
            probe.join().unwrap_or(0)
        );
        let blocked = s.spawn(|| {
            let t = Instant::now();
            drop(db.read_conn());
            t.elapsed().as_millis()
        });
        std::thread::sleep(Duration::from_secs(2));
        drop(writer);
        println!(
            "pool exhausted + writer held 2 s: read_conn (old path) wait {} ms",
            blocked.join().unwrap_or(0)
        );
    });
    drop(readers);

    // 4. Background batch size: one 32-text request vs four 8-text requests.
    let texts = bulk_batch(99);
    let t = Instant::now();
    let _ = crate::embeddings::embed_texts(&texts).await;
    let one = t.elapsed().as_millis();
    let t = Instant::now();
    for chunk in bulk_batch(98).chunks(8) {
        let _ = crate::embeddings::embed_texts(chunk).await;
    }
    println!(
        "32 texts: 1x32 {one} ms, 4x8 {} ms (idle Ollama)",
        t.elapsed().as_millis()
    );
}
