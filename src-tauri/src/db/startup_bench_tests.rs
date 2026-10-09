// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Startup-latency benchmark for `get_database()`-equivalent init on a REAL
//! corpus copy. `#[ignore]`d: it needs a multi-GB database and is a measuring
//! instrument, not a regression gate.
//!
//! ```text
//! FOURDA_BENCH_DB=E:\dbcheck-copy\4da.db \
//!   cargo test --lib db::startup_bench_tests -- --ignored --nocapture
//! ```
//!
//! Point it at a COPY (never the live file): it opens read-write, may write the
//! integrity marker next to the copy, and checkpoints the copy's WAL.

use std::path::PathBuf;
use std::time::Instant;

use rusqlite::Connection;

use super::Database;

fn bench_db() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var("FOURDA_BENCH_DB").ok()?);
    p.exists().then_some(p)
}

fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    println!("BENCH {label}: {:.2}s", t.elapsed().as_secs_f64());
    out
}

#[test]
#[ignore = "needs FOURDA_BENCH_DB pointing at a multi-GB corpus copy"]
fn bench_startup_db_init_on_corpus_copy() {
    let Some(path) = bench_db() else {
        println!("BENCH skipped: FOURDA_BENCH_DB unset or missing");
        return;
    };
    println!(
        "BENCH profile: debug_assertions={} (libsqlite3-sys opt-level comes from Cargo.toml)",
        cfg!(debug_assertions)
    );

    // Warm the OS page cache once so every figure below is a warm-cache number
    // (Python's 3.9-4.8s reference was also warm).
    time("warm-up PRAGMA quick_check (cold-ish OS cache)", || {
        let conn = Connection::open(&path).expect("open copy");
        let _: String = conn
            .query_row("PRAGMA quick_check", [], |r| r.get(0))
            .expect("warm quick_check");
    });

    let conn = Connection::open(&path).expect("open copy");
    let status: String = time("raw PRAGMA quick_check", || {
        conn.query_row("PRAGMA quick_check", [], |r| r.get(0))
            .expect("quick_check")
    });
    assert_eq!(status, "ok", "bench copy must be intact");

    let heavy = "SELECT si.source_type, COUNT(*), SUM(LENGTH(si.content)) \
                 FROM source_items si JOIN scoring_explanations se ON se.source_item_id = si.id \
                 GROUP BY si.source_type";
    let rows: usize = time("heavy join+aggregate over source_items", || {
        let mut stmt = conn.prepare(heavy).expect("prepare heavy");
        stmt.query_map([], |_| Ok(())).expect("query heavy").count()
    });
    println!("BENCH heavy rows: {rows}");
    let fts: i64 = time("FTS MATCH count", || {
        conn.query_row(
            "SELECT COUNT(*) FROM source_items_fts WHERE source_items_fts MATCH 'rust OR sqlite OR react OR python'",
            [],
            |r| r.get(0),
        )
        .expect("fts")
    });
    println!("BENCH fts hits: {fts}");
    drop(conn);

    // (i) First start after this change: no marker, so the scan runs.
    super::integrity_gate::invalidate(&path);
    println!("BENCH --- start (i): no marker ---");
    run_init_sequence(&path);
    // (ii) Normal start: marker just written, no unclean exits.
    println!("BENCH --- start (ii): fresh marker, clean previous exit ---");
    run_init_sequence(&path);
    super::integrity_gate::invalidate(&path);
}

/// The `get_database()` sequence minus the OnceCell: pre-flight, then
/// `Database::new`. Timed as a whole and per step.
fn run_init_sequence(path: &std::path::Path) {
    let total = Instant::now();
    let recovery = time("pre-flight integrity_gate::preflight", || {
        super::integrity_gate::preflight(path)
    });
    println!("BENCH pre-flight verdict: {recovery:?}");
    let db = time("Database::new", || {
        Database::new(path).expect("Database::new")
    });
    println!(
        "BENCH TOTAL get_database()-equivalent init: {:.2}s",
        total.elapsed().as_secs_f64()
    );
    drop(db);
}
