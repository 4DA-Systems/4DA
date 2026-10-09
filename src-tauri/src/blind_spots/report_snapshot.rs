// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! The last good derived result, persisted so a restart never opens cold.
//!
//! Live 2026-10-10 a Blind Spots open in the first minutes after launch took
//! 15.6 s: the in-memory per-cycle cache starts empty, and its startup warm
//! is queued until after first-light. The report is a pure function of the
//! stored corpus, so the previous run's report is a truthful answer to "what
//! did the last analysis find" — it is served at once while the rebuild runs
//! (stale-while-revalidate across restarts), carrying the time it was
//! computed so the UI can say so.
//!
//! A snapshot is only ever restored into the binary and database that wrote
//! it: the [`Stamp`] pins the snapshot format, the app version, the database
//! schema version and the scoring pipeline version, and any change discards
//! it. Writes are atomic (temp file + rename). The file lives next to the
//! database, so a `FOURDA_DB_PATH` / `FOURDA_DATA_DIR` profile — and every
//! test process, whose database is a throwaway — gets its own.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Bump when a persisted type changes shape incompatibly.
const FORMAT: u32 = 1;

/// What must be unchanged for a snapshot to be restored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Stamp {
    format: u32,
    app_version: String,
    schema_version: i64,
    pipeline_version: i32,
}

impl Stamp {
    /// The stamp for this binary against `conn`'s database. `None` when the
    /// schema version cannot be read — then nothing is saved or restored.
    pub(crate) fn current(conn: &rusqlite::Connection) -> Option<Self> {
        let schema_version = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .ok()?;
        Some(Self::with_schema(schema_version))
    }

    pub(crate) fn with_schema(schema_version: i64) -> Self {
        Self {
            format: FORMAT,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            schema_version,
            pipeline_version: crate::scoring::PIPELINE_VERSION,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Envelope<T> {
    stamp: Stamp,
    /// Unix seconds when the value was saved (it was computed just before).
    saved_at: i64,
    value: T,
}

/// A value read back from disk and how old it is.
pub(crate) struct Restored<T> {
    pub value: T,
    pub age: Duration,
}

/// `<db dir>/<file_name>`.
pub(crate) fn snapshot_path(file_name: &str) -> PathBuf {
    crate::state::get_db_path().with_file_name(file_name)
}

fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Persist `value` atomically. Best-effort by contract: a failed write only
/// costs the next launch its warm start.
pub(crate) fn save<T: Serialize>(path: &Path, stamp: &Stamp, value: &T) -> std::io::Result<()> {
    let envelope = Envelope {
        stamp: stamp.clone(),
        saved_at: now_unix(),
        value,
    };
    let bytes = serde_json::to_vec(&envelope).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// The persisted value when its stamp matches `current` and it is younger
/// than `max_age`; `None` for a missing, unreadable, mismatched or too-old
/// snapshot (a clock that went backwards counts as too old).
pub(crate) fn load<T: DeserializeOwned>(
    path: &Path,
    current: &Stamp,
    max_age: Duration,
) -> Option<Restored<T>> {
    let bytes = std::fs::read(path).ok()?;
    let envelope: Envelope<T> = serde_json::from_slice(&bytes).ok()?;
    if envelope.stamp != *current {
        return None;
    }
    let age_secs = u64::try_from(now_unix() - envelope.saved_at).ok()?;
    let age = Duration::from_secs(age_secs);
    (age <= max_age).then_some(Restored {
        value: envelope.value,
        age,
    })
}

#[cfg(test)]
#[path = "report_snapshot_tests.rs"]
mod tests;
