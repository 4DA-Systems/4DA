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
}

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
}
