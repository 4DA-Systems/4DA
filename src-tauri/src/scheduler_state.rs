// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Persisted scheduler state — the cold-boot stampede killer.
//!
//! Background jobs in `monitoring.rs` track their last-run time as in-memory
//! `AtomicU64`s that default to 0 on every cold boot. The scheduler then
//! checks `now - last_X >= INTERVAL`, which is *always* true on first tick
//! when `last_X == 0`. The result: every "scheduled" job (anomaly detection,
//! VACUUM, autophagy, dependency health, accuracy recording, etc.) fires
//! within the first 60 seconds of every cold boot — the visible "stampede"
//! the user reported in screenshots 1898/1899/1900.
//!
//! This module persists those timestamps in a tiny `scheduler_state` table
//! (created in migration Phase 51) so they survive restart. On startup we
//! hydrate the in-memory atomics from the table; on each job completion we
//! write the new timestamp back. A user who closes 4DA at 9:00 AM and
//! reopens it at 9:05 AM gets *zero* scheduled jobs running on cold boot —
//! only jobs whose interval has actually elapsed.
//!
//! ## Design notes
//!
//! - **Best-effort writes**: persistence failures must never crash the
//!   scheduler. Worst case is one job re-fires on the next boot.
//! - **Stable job names**: the names below are the public contract with
//!   the database. Renaming them is a breaking change.
//! - **No locks held during persist**: we always copy the timestamp out of
//!   the atomic before opening a DB connection, and never hold the
//!   monitoring state lock across the I/O.

use std::sync::atomic::Ordering;

use tracing::{debug, warn};

use crate::monitoring::MonitoringState;
use crate::open_db_connection;

/// Stable, schema-stable job names. Changing these breaks persistence.
pub mod jobs {
    pub const HEALTH_CHECK: &str = "health_check";
    pub const DB_MAINTENANCE: &str = "db_maintenance";
    pub const VACUUM: &str = "vacuum";
    pub const ANOMALY_DETECTION: &str = "anomaly_detection";
    pub const CVE_SCAN: &str = "cve_scan";
    pub const DEP_HEALTH: &str = "dep_health";
    pub const BEHAVIOR_DECAY: &str = "behavior_decay";
    pub const AUTOPHAGY: &str = "autophagy";
    pub const ACCURACY_RECORD: &str = "accuracy_record";
    pub const TEMPORAL_SNAPSHOT: &str = "temporal_snapshot";
    pub const BACKFILL: &str = "scoring_backfill";
    pub const CALIBRATION_MONITOR: &str = "calibration_monitor";
    /// Proactive signal-chain prediction notifications. Persisted so a restart
    /// cannot reset the cadence to "fire immediately" — without this the job
    /// re-fires on every cold boot.
    pub const CHAIN_NOTIFY: &str = "chain_notify";
}

/// kv_store key holding the dependency-set epoch HASH used by the
/// re-examination job. This lived in `scheduler_state.last_run_unix` as the
/// `dep_epoch_hash` "job" until schema 114 — a 63-bit hash in a timestamp
/// column poisoned every consumer doing time math on it
/// (`boot_context::last_scheduler_run` takes `MAX(last_run_unix)`, so
/// process-recency detection always saw a run "just now"). A hash is a value,
/// not a schedule; it belongs in kv_store.
pub const DEP_EPOCH_KV_KEY: &str = "dep_epoch_hash";

/// Hydrate in-memory monitoring atomics from persisted scheduler_state.
///
/// Called once during `setup_app`, immediately after `start_scheduler` is
/// invoked but BEFORE the first scheduler tick. Any rows missing from the
/// table are left at the in-memory default (0), which means the job will
/// run after the cold-boot grace period elapses — the safe default.
pub fn hydrate_from_db(state: &MonitoringState) {
    let conn = match open_db_connection() {
        Ok(c) => c,
        Err(e) => {
            warn!(target: "4da::scheduler", error = %e, "Could not open DB to hydrate scheduler state");
            return;
        }
    };

    let mut stmt = match conn
        .prepare("SELECT job_name, last_run_unix FROM scheduler_state WHERE last_run_unix > 0")
    {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::scheduler", error = %e, "scheduler_state table not yet migrated");
            return;
        }
    };

    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::scheduler", error = %e, "scheduler_state hydrate query failed");
            return;
        }
    };

    let mut hydrated = 0_u32;
    for row in rows.flatten() {
        let (name, ts) = row;
        let ts_u64 = ts.max(0) as u64;
        match name.as_str() {
            jobs::HEALTH_CHECK => {
                state.last_health_check.store(ts_u64, Ordering::Relaxed);
                hydrated += 1;
            }
            jobs::ANOMALY_DETECTION => {
                state.last_anomaly_check.store(ts_u64, Ordering::Relaxed);
                hydrated += 1;
            }
            jobs::CVE_SCAN => {
                state.last_cve_scan.store(ts_u64, Ordering::Relaxed);
                hydrated += 1;
            }
            jobs::DEP_HEALTH => {
                state.last_dep_health_check.store(ts_u64, Ordering::Relaxed);
                hydrated += 1;
            }
            jobs::CHAIN_NOTIFY => {
                state.last_chain_notify.store(ts_u64, Ordering::Relaxed);
                hydrated += 1;
            }
            jobs::BEHAVIOR_DECAY | jobs::AUTOPHAGY => {
                // BEHAVIOR_DECAY and AUTOPHAGY share `last_decay` because they
                // run together inside the daily decay block in monitoring.rs.
                // Take the LATER of the two so we don't double-fire either.
                let cur = state.last_decay.load(Ordering::Relaxed);
                if ts_u64 > cur {
                    state.last_decay.store(ts_u64, Ordering::Relaxed);
                }
                hydrated += 1;
            }
            jobs::ACCURACY_RECORD | jobs::TEMPORAL_SNAPSHOT => {
                let cur = state.last_accuracy_check.load(Ordering::Relaxed);
                if ts_u64 > cur {
                    state.last_accuracy_check.store(ts_u64, Ordering::Relaxed);
                }
                hydrated += 1;
            }
            // VACUUM and DB_MAINTENANCE are tracked via static atomics inside
            // monitoring.rs (LAST_VACUUM, LAST_MAINTENANCE). They are hydrated
            // separately by `hydrate_static_atomics`.
            _ => {}
        }
    }

    if hydrated > 0 {
        tracing::info!(
            target: "4da::scheduler",
            hydrated,
            "Hydrated scheduler state from DB (cold-boot stampede prevention active)"
        );
    } else {
        debug!(target: "4da::scheduler", "No persisted scheduler state to hydrate (fresh DB or first run)");
    }
}

/// Get a persisted timestamp by job name. Returns 0 if missing.
///
/// Used for the static atomics inside `monitoring.rs` (`LAST_VACUUM`,
/// `LAST_MAINTENANCE`) which can't be hydrated by `hydrate_from_db`
/// because they're function-local statics.
pub fn get_persisted_timestamp(job_name: &str) -> u64 {
    let conn = match open_db_connection() {
        Ok(c) => c,
        Err(_) => return 0,
    };
    conn.query_row(
        "SELECT last_run_unix FROM scheduler_state WHERE job_name = ?1",
        rusqlite::params![job_name],
        |row| row.get::<_, i64>(0),
    )
    .map(|v| v.max(0) as u64)
    .unwrap_or(0)
}

/// Persist a job's completion timestamp. Best-effort — failures are logged
/// but never propagated, so a transient DB lock cannot crash the scheduler.
pub fn persist_run(job_name: &str, unix_ts: u64) {
    let conn = match open_db_connection() {
        Ok(c) => c,
        Err(e) => {
            debug!(
                target: "4da::scheduler",
                job = %job_name,
                error = %e,
                "Could not persist scheduler run (will retry on next completion)"
            );
            return;
        }
    };

    let result = conn.execute(
        "INSERT INTO scheduler_state (job_name, last_run_unix, run_count, updated_at)
         VALUES (?1, ?2, 1, datetime('now'))
         ON CONFLICT(job_name) DO UPDATE SET
            last_run_unix = excluded.last_run_unix,
            run_count = scheduler_state.run_count + 1,
            updated_at = datetime('now')",
        rusqlite::params![job_name, unix_ts as i64],
    );

    if let Err(e) = result {
        debug!(
            target: "4da::scheduler",
            job = %job_name,
            error = %e,
            "scheduler_state upsert failed (non-fatal)"
        );
    }
}

/// Longest outcome text persisted to `scheduler_state.last_outcome`.
/// Errors are truncated, never dropped — a cut-off message still names the
/// failure class, which is what the ops surface needs.
const OUTCOME_MAX_LEN: usize = 200;

/// Format a job outcome for `last_outcome`: `"ok"` or `"error: <truncated>"`.
fn format_outcome(error: Option<&str>) -> String {
    match error {
        None => "ok".to_string(),
        Some(e) => {
            let mut msg = format!("error: {e}");
            if msg.len() > OUTCOME_MAX_LEN {
                let mut cut = OUTCOME_MAX_LEN;
                while cut > 0 && !msg.is_char_boundary(cut) {
                    cut -= 1;
                }
                msg.truncate(cut);
            }
            msg
        }
    }
}

/// Record how the most recent run of a job actually went, plus how long it
/// took. Companion to [`persist_run`], which stamps WHEN a job fired — these
/// two columns existed since Phase 51 but nothing ever wrote them, so
/// `last_outcome` / `last_duration_ms` were NULL for every job (2026-08-31
/// live audit). Best-effort like everything else here: failures are logged,
/// never propagated.
pub fn record_outcome(job_name: &str, error: Option<&str>, duration_ms: u64) {
    let conn = match open_db_connection() {
        Ok(c) => c,
        Err(e) => {
            debug!(
                target: "4da::scheduler",
                job = %job_name,
                error = %e,
                "Could not open DB to record job outcome (non-fatal)"
            );
            return;
        }
    };
    record_outcome_on(&conn, job_name, error, duration_ms);
}

/// Connection-injected implementation of [`record_outcome`] (hermetic tests).
/// Upserts so an outcome can never be lost to a missing row — jobs that were
/// never pre-seeded in Phase 51 (e.g. `calibration_monitor`) get their row
/// created here with `last_run_unix = 0` and let `persist_run` fill it in.
pub(crate) fn record_outcome_on(
    conn: &rusqlite::Connection,
    job_name: &str,
    error: Option<&str>,
    duration_ms: u64,
) {
    let outcome = format_outcome(error);
    let result = conn.execute(
        "INSERT INTO scheduler_state (job_name, last_run_unix, run_count, last_outcome, last_duration_ms, updated_at)
         VALUES (?1, 0, 0, ?2, ?3, datetime('now'))
         ON CONFLICT(job_name) DO UPDATE SET
            last_outcome = excluded.last_outcome,
            last_duration_ms = excluded.last_duration_ms,
            updated_at = datetime('now')",
        rusqlite::params![job_name, outcome, i64::try_from(duration_ms).unwrap_or(i64::MAX)],
    );
    if let Err(e) = result {
        debug!(
            target: "4da::scheduler",
            job = %job_name,
            error = %e,
            "scheduler_state outcome write failed (non-fatal)"
        );
    }
}

/// Read the persisted dependency-set epoch hash from kv_store. Returns 0 when
/// unset or unparseable (both mean "treat the epoch as changed", which at
/// worst triggers one harmless re-examination pass).
pub fn get_dep_epoch_hash() -> u64 {
    let conn = match open_db_connection() {
        Ok(c) => c,
        Err(_) => return 0,
    };
    get_dep_epoch_hash_on(&conn)
}

/// Connection-injected implementation of [`get_dep_epoch_hash`].
/// `CAST(value AS TEXT)` normalizes whatever affinity wrote the value —
/// kv_store's `value` column is typeless, and the schema-114 migration copies
/// the legacy integer straight out of `scheduler_state.last_run_unix`.
pub(crate) fn get_dep_epoch_hash_on(conn: &rusqlite::Connection) -> u64 {
    conn.query_row(
        "SELECT CAST(value AS TEXT) FROM kv_store WHERE key = ?1",
        rusqlite::params![DEP_EPOCH_KV_KEY],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|v| v.trim().parse::<u64>().ok())
    .unwrap_or(0)
}

/// Persist the dependency-set epoch hash to kv_store. Best-effort — a lost
/// write means one extra (harmless) re-examination pass on the next check.
pub fn persist_dep_epoch_hash(hash: u64) {
    let conn = match open_db_connection() {
        Ok(c) => c,
        Err(e) => {
            debug!(
                target: "4da::scheduler",
                error = %e,
                "Could not open DB to persist dep epoch hash (non-fatal)"
            );
            return;
        }
    };
    persist_dep_epoch_hash_on(&conn, hash);
}

/// Connection-injected implementation of [`persist_dep_epoch_hash`].
pub(crate) fn persist_dep_epoch_hash_on(conn: &rusqlite::Connection, hash: u64) {
    let result = conn.execute(
        "INSERT OR REPLACE INTO kv_store (key, value, updated_at)
         VALUES (?1, ?2, datetime('now'))",
        rusqlite::params![DEP_EPOCH_KV_KEY, hash.to_string()],
    );
    if let Err(e) = result {
        debug!(
            target: "4da::scheduler",
            error = %e,
            "kv_store dep epoch write failed (non-fatal)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jobs_are_lowercase_underscores() {
        // Stable contract with the migration: any rename is breaking.
        assert_eq!(jobs::HEALTH_CHECK, "health_check");
        assert_eq!(jobs::AUTOPHAGY, "autophagy");
        assert_eq!(jobs::VACUUM, "vacuum");
    }

    fn mem_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE scheduler_state (
                job_name TEXT PRIMARY KEY NOT NULL,
                last_run_unix INTEGER NOT NULL DEFAULT 0,
                last_duration_ms INTEGER,
                run_count INTEGER NOT NULL DEFAULT 0,
                last_outcome TEXT,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             CREATE TABLE kv_store (
                key TEXT PRIMARY KEY NOT NULL,
                value,
                updated_at TEXT DEFAULT (datetime('now'))
             );",
        )
        .expect("schema");
        conn
    }

    fn read_outcome(conn: &rusqlite::Connection, job: &str) -> (Option<String>, Option<i64>) {
        conn.query_row(
            "SELECT last_outcome, last_duration_ms FROM scheduler_state WHERE job_name = ?1",
            rusqlite::params![job],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("row exists")
    }

    #[test]
    fn record_outcome_writes_ok_and_duration() {
        let conn = mem_conn();
        conn.execute(
            "INSERT INTO scheduler_state (job_name, last_run_unix) VALUES ('health_check', 123)",
            [],
        )
        .unwrap();

        record_outcome_on(&conn, "health_check", None, 42);

        let (outcome, duration) = read_outcome(&conn, "health_check");
        assert_eq!(outcome.as_deref(), Some("ok"));
        assert_eq!(duration, Some(42));

        // The schedule stamp is untouched — outcome and cadence are separate.
        let ts: i64 = conn
            .query_row(
                "SELECT last_run_unix FROM scheduler_state WHERE job_name = 'health_check'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ts, 123);
    }

    #[test]
    fn record_outcome_writes_truncated_error() {
        let conn = mem_conn();
        let long_error = "x".repeat(500);
        record_outcome_on(&conn, "anomaly_detection", Some(&long_error), 7);

        let (outcome, duration) = read_outcome(&conn, "anomaly_detection");
        let outcome = outcome.expect("outcome written");
        assert!(outcome.starts_with("error: xxx"), "got {outcome}");
        assert_eq!(outcome.len(), OUTCOME_MAX_LEN);
        assert_eq!(duration, Some(7));
    }

    #[test]
    fn record_outcome_creates_missing_row_without_faking_a_run() {
        // calibration_monitor was never pre-seeded by Phase 51.
        let conn = mem_conn();
        record_outcome_on(&conn, "calibration_monitor", None, 5);

        let (ts, count): (i64, i64) = conn
            .query_row(
                "SELECT last_run_unix, run_count FROM scheduler_state WHERE job_name = 'calibration_monitor'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(ts, 0, "outcome write must not fabricate a run timestamp");
        assert_eq!(count, 0, "outcome write must not fabricate a run count");
    }

    #[test]
    fn format_outcome_truncates_on_char_boundary() {
        // Multi-byte characters straddling the cap must not panic truncate().
        let error = "é".repeat(300);
        let formatted = format_outcome(Some(&error));
        assert!(formatted.len() <= OUTCOME_MAX_LEN);
        assert!(formatted.starts_with("error: é"));
    }

    #[test]
    fn dep_epoch_hash_round_trips_through_kv_store() {
        let conn = mem_conn();
        assert_eq!(get_dep_epoch_hash_on(&conn), 0, "unset reads as 0");

        // The live poisoned value from the 2026-08-31 audit.
        persist_dep_epoch_hash_on(&conn, 4_748_353_192_844_586_074);
        assert_eq!(get_dep_epoch_hash_on(&conn), 4_748_353_192_844_586_074);

        // Overwrite wins.
        persist_dep_epoch_hash_on(&conn, 17);
        assert_eq!(get_dep_epoch_hash_on(&conn), 17);
    }

    #[test]
    fn dep_epoch_hash_reads_integer_affinity_values() {
        // The schema-114 migration copies the legacy value with INTEGER
        // affinity; the reader must normalize it.
        let conn = mem_conn();
        conn.execute(
            "INSERT INTO kv_store (key, value) VALUES (?1, 12345)",
            rusqlite::params![DEP_EPOCH_KV_KEY],
        )
        .unwrap();
        assert_eq!(get_dep_epoch_hash_on(&conn), 12345);
    }
}
