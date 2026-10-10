// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! The last good derived result, persisted so a restart never opens cold.
//!
//! Live 2026-10-10 a Blind Spots open in the first minutes after launch took
//! 15.6 s, the first Preemption open more than 60 s and the first Knowledge
//! Gaps read 5.7 s: each in-memory cache starts empty, and its startup warm is
//! queued until after first-light. Every one of those results is a pure
//! function of the stored corpus, so the previous run's result is a truthful
//! answer to "what did the last analysis find". It is served at once while
//! the rebuild runs (stale-while-revalidate across restarts), carrying the
//! time it was computed so the UI can say so.
//!
//! A snapshot is only ever restored into the binary and database that wrote
//! it: the [`Stamp`] pins the snapshot format, the app version, the database
//! schema version and the scoring pipeline version, and any change discards
//! it. Writes are atomic (temp file + rename). The file lives next to the
//! database, so a `FOURDA_DB_PATH` / `FOURDA_DATA_DIR` profile — and every
//! test process, whose database is a throwaway — gets its own. Every name is
//! `data/*.json` (its staging file `data/*.json.tmp`), both gitignored.
//!
//! Users: Blind Spots (`blind_spots_snapshot.json`), Knowledge Gaps
//! (`knowledge_gaps_snapshot.json`) and the Preemption feed, one file per
//! tier scope (`preemption_feed_full.json`, `preemption_feed_free_floor.json`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

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
    /// Unix seconds when it was saved.
    pub saved_at: i64,
}

impl<T> Restored<T> {
    /// When the value was computed, RFC 3339 UTC — what a surface shows as
    /// "computed <age>".
    pub(crate) fn computed_at(&self) -> String {
        chrono::DateTime::from_timestamp(self.saved_at, 0)
            .unwrap_or_else(chrono::Utc::now)
            .to_rfc3339()
    }
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
pub(crate) fn save<T: Serialize + ?Sized>(
    path: &Path,
    stamp: &Stamp,
    value: &T,
) -> std::io::Result<()> {
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
        saved_at: envelope.saved_at,
    })
}

/// One persisted result: its file beside the database, how old it may be
/// when restored, and a latch so the restore happens at most once per
/// process. After the first read of a run, the in-memory cache holds this
/// run's own value — or a failed rebuild that must not be papered over with
/// an old one — so a second restore is never right.
pub(crate) struct SnapshotFile {
    file_name: &'static str,
    max_age: Duration,
    restore_attempted: AtomicBool,
}

impl SnapshotFile {
    pub(crate) const fn new(file_name: &'static str, max_age: Duration) -> Self {
        Self {
            file_name,
            max_age,
            restore_attempted: AtomicBool::new(false),
        }
    }

    pub(crate) fn path(&self) -> PathBuf {
        snapshot_path(self.file_name)
    }

    /// Persist `value` under `conn`'s stamp. Best-effort: logs, never fails.
    pub(crate) fn persist<T: Serialize + ?Sized>(&self, conn: &rusqlite::Connection, value: &T) {
        let Some(stamp) = Stamp::current(conn) else {
            return;
        };
        if let Err(e) = save(&self.path(), &stamp, value) {
            warn!(
                target: "4da::snapshot",
                file = self.file_name,
                error = %e,
                "could not persist the restart snapshot"
            );
        }
    }

    /// [`Self::persist`] through a connection of its own.
    pub(crate) fn persist_detached<T: Serialize + ?Sized>(&self, value: &T) {
        if let Ok(conn) = crate::open_db_connection() {
            self.persist(&conn, value);
        }
    }

    /// The persisted value if the stamp and age allow it, with no latch.
    pub(crate) fn restore<T: DeserializeOwned>(
        &self,
        conn: &rusqlite::Connection,
    ) -> Option<Restored<T>> {
        let stamp = Stamp::current(conn)?;
        load(&self.path(), &stamp, self.max_age)
    }

    /// The previous run's value, read at most once per process.
    pub(crate) fn restore_once<T: DeserializeOwned>(
        &self,
        conn: &rusqlite::Connection,
    ) -> Option<Restored<T>> {
        if self.restore_attempted.swap(true, Ordering::SeqCst) {
            return None;
        }
        let restored = self.restore(conn)?;
        info!(
            target: "4da::snapshot",
            file = self.file_name,
            age_secs = restored.age.as_secs(),
            "restart snapshot restored"
        );
        Some(restored)
    }

    /// [`Self::restore_once`] through a connection of its own.
    pub(crate) fn restore_once_detached<T: DeserializeOwned>(&self) -> Option<Restored<T>> {
        if self.restore_attempted.load(Ordering::SeqCst) {
            return None;
        }
        let conn = crate::open_db_connection().ok()?;
        self.restore_once(&conn)
    }
}

#[cfg(test)]
#[path = "restart_snapshot_tests.rs"]
mod tests;
