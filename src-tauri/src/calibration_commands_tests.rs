// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Calibration fitter discovery: correct pairs, linear cost, off the runtime.

use rusqlite::{params, Connection, StatementStatus};

use super::*;

/// The candidate query as it stood before the 2026-10-19 runtime stall: a
/// self-join of every sample against every provenance row of its task.
const QUADRATIC_PAIRS_SQL: &str =
    "SELECT DISTINCT cs.model_identity_hash, cs.task, p.provider, p.model
     FROM calibration_samples cs
     LEFT JOIN provenance p
       ON p.model_identity_hash = cs.model_identity_hash
       AND p.task = cs.task
     WHERE cs.processed_at IS NULL
       AND cs.created_at <= datetime('now', ?1)
     GROUP BY cs.model_identity_hash, cs.task";

/// The real tables and indexes the discovery reads.
fn test_conn() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE calibration_samples (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_item_id INTEGER NOT NULL,
            model_identity_hash TEXT NOT NULL,
            task TEXT NOT NULL,
            prompt_version TEXT NOT NULL,
            raw_score REAL NOT NULL,
            confidence REAL NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            processed_at TEXT
         );
         CREATE INDEX idx_cal_samples_unfit
           ON calibration_samples(model_identity_hash, task, processed_at);
         CREATE TABLE provenance (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            artifact_kind TEXT NOT NULL,
            artifact_id TEXT NOT NULL,
            model_identity_hash TEXT NOT NULL,
            provider TEXT NOT NULL,
            model TEXT NOT NULL,
            task TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
         );
         CREATE INDEX idx_provenance_model ON provenance(model_identity_hash);
         CREATE INDEX idx_provenance_task ON provenance(task);",
    )
    .expect("schema");
    conn
}

fn add_sample(conn: &Connection, hash: &str, age_hours: i64, processed: bool) {
    conn.execute(
        "INSERT INTO calibration_samples
            (source_item_id, model_identity_hash, task, prompt_version,
             raw_score, confidence, created_at, processed_at)
         VALUES (1, ?1, 'judge', 'p', 0.5, 0.5, datetime('now', ?2),
                 CASE WHEN ?3 THEN datetime('now') END)",
        params![hash, format!("-{age_hours} hours"), processed],
    )
    .expect("sample");
}

fn add_provenance(conn: &Connection, hash: &str, provider: &str, model: &str) {
    conn.execute(
        "INSERT INTO provenance (artifact_kind, artifact_id, model_identity_hash, provider, model, task)
         VALUES ('item', 'x', ?1, ?2, ?3, 'judge')",
        params![hash, provider, model],
    )
    .expect("provenance");
}

/// Sum of SQLite VM steps for one run of `sql` with `args`.
fn vm_steps(conn: &Connection, sql: &str, args: impl rusqlite::Params) -> i64 {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let mut rows = stmt.query(args).expect("query");
    while rows.next().expect("step").is_some() {}
    drop(rows);
    i64::from(stmt.get_status(StatementStatus::VmStep))
}

#[test]
fn discovery_names_each_unprocessed_old_pair_with_its_model() {
    let conn = test_conn();
    add_sample(&conn, "a", 48, false);
    add_sample(&conn, "a", 30, false);
    add_sample(&conn, "b", 48, false); // no provenance row
    add_sample(&conn, "c", 48, true); // already processed
    add_sample(&conn, "d", 1, false); // too young to fit
    add_provenance(&conn, "a", "ollama", "gemma4:12b");
    add_provenance(&conn, "a", "ollama", "gemma4:12b");
    add_provenance(&conn, "c", "anthropic", "claude-haiku-4-5");

    let mut got = discover_fit_candidates(&conn).expect("discover");
    got.sort();
    assert_eq!(
        got,
        vec![
            (
                "a".into(),
                "judge".into(),
                "ollama".into(),
                "gemma4:12b".into()
            ),
            ("b".into(), "judge".into(), String::new(), String::new()),
        ]
    );
}

/// The old self-join cost samples x provenance rows: on the operator's 28k x
/// 28k corpus it held the writer lock ~160 s and froze every tokio worker.
/// The discovery must stay linear in the corpus.
#[test]
fn discovery_cost_is_linear_not_samples_times_provenance() {
    const N: i64 = 1_000;
    let conn = test_conn();
    for i in 0..N {
        let hash = format!("h{}", i % 4);
        add_sample(&conn, &hash, 48, false);
        add_provenance(&conn, &hash, "ollama", "m");
    }
    let age = format!("-{} hours", crate::calibration_fitter::MIN_AGE_HOURS);

    let pairs_steps = vm_steps(&conn, FIT_CANDIDATE_PAIRS_SQL, params![age]);
    let lookup_steps: i64 = (0..4)
        .map(|i| {
            vm_steps(
                &conn,
                FIT_PAIR_PROVENANCE_SQL,
                params![format!("h{i}"), "judge"],
            )
        })
        .sum();
    let new_steps = pairs_steps + lookup_steps;
    let old_steps = vm_steps(&conn, QUADRATIC_PAIRS_SQL, params![age]);

    assert!(
        new_steps < 100 * N,
        "candidate discovery took {new_steps} VM steps for {N} samples (linear bound {})",
        100 * N
    );
    // Control: the bound is meaningful because the old query blows through it.
    assert!(
        old_steps > 10 * new_steps,
        "control: quadratic query took only {old_steps} steps vs {new_steps}"
    );
    assert_eq!(discover_fit_candidates(&conn).expect("discover").len(), 4);
}

/// The fit is synchronous SQLite; run on a tokio worker it parks that worker
/// for the whole run and holds the writer lock other workers then wait on.
#[test]
fn the_fit_command_runs_on_the_blocking_pool() {
    let src = include_str!("calibration_commands.rs");
    let start = src
        .find("pub async fn fit_calibration_curves_now")
        .expect("command present");
    let body = &src[start..src[start..].find("\n}\n").map_or(src.len(), |e| start + e)];
    assert!(
        body.contains("off_ui_thread(") && body.contains("fit_calibration_curves_blocking"),
        "fit_calibration_curves_now must hand its body to the blocking pool:\n{body}"
    );
    assert!(
        !body.contains("db.conn.lock()"),
        "the async command must not take the writer lock itself"
    );
}
