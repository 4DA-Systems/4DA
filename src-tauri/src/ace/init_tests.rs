// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `ACE::new` must stay cheap enough to run inside the scoring-context budget.
//!
//! ACE is initialised lazily from inside `build_scoring_context_cold`, which
//! runs under `scoring::context::BUILD_TIMEOUT_SECS`. Anything ACE::new reads
//! that scales with the size of the shared database file is paid inside that
//! budget on every cold headless run.

use std::sync::{Arc, Mutex as StdMutex};

use parking_lot::Mutex;
use rusqlite::Connection;

use super::ACE;

/// Statements traced on the connection under test. `Connection::trace` takes
/// a plain `fn`, so the sink has to be a static.
static TRACED: StdMutex<Vec<String>> = StdMutex::new(Vec::new());

fn record(sql: &str) {
    TRACED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(sql.to_lowercase());
}

/// Regression: a `PRAGMA quick_check` here read every page of the whole
/// corpus (the file ACE shares with the main DB, which `get_database()`
/// already checks twice per process). Live 2026-09-18..24 it took 45-245s on
/// a busy disk and caused 31 of 33 "Scoring context build timed out after
/// 45s (caller: differential_scoring)" failures.
#[test]
fn ace_new_runs_no_whole_file_integrity_scan() {
    // ACE's schema creates vec0 tables; production registers the extension
    // in open_db_connection before any connection is opened.
    crate::register_sqlite_vec_extension();
    let dir = tempfile::tempdir().expect("tempdir");
    let mut conn = Connection::open(dir.path().join("4da.db")).expect("open temp db");
    conn.trace(Some(record));

    let ace = ACE::new(Arc::new(Mutex::new(conn))).expect("ACE::new on an empty db");
    drop(ace);

    let traced = TRACED.lock().unwrap_or_else(|e| e.into_inner());
    assert!(
        !traced.is_empty(),
        "trace hook recorded nothing — the test would pass vacuously"
    );
    let scans: Vec<&String> = traced
        .iter()
        .filter(|s| s.contains("quick_check") || s.contains("integrity_check"))
        .collect();
    assert!(
        scans.is_empty(),
        "ACE::new ran a whole-file integrity scan inside the scoring-context budget: {scans:?}"
    );
}
