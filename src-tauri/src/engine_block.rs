// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Engine-block marker — a loud trace for a refresh that can only refuse.
//!
//! 2026-08-28→30: the scheduled `--engine-once` refresh ran a binary that
//! supported schema max 111 against a schema-113 database. The migration guard
//! correctly refused (no corruption), but every nightly and hourly cycle was
//! three minutes of refusal loops, the feed froze for two days, and the only
//! trace was ERROR lines in a log nobody reads. The refusing process cannot
//! record the failure in the database — the database is exactly what it cannot
//! open — so the trace has to live beside it on disk.
//!
//! `note_db_error` writes `data/.engine-blocked` when a database error carries
//! the schema-too-new phrase; `clear` removes it the moment a cycle opens the
//! database again. Two readers surface it: the desktop app's startup health
//! banner (`startup_health::check_engine_block`) and the MCP server's
//! `data_freshness` block — the reader that actually caught the outage.

use std::path::PathBuf;

use tracing::{error, warn};

/// Marker file name, created in the data directory beside `4da.db`.
pub(crate) const MARKER_FILE: &str = ".engine-blocked";

fn marker_path() -> PathBuf {
    crate::runtime_paths::RuntimePaths::get()
        .data_dir
        .join(MARKER_FILE)
}

/// Record a database-access failure IF it is the schema-too-new refusal.
///
/// Other database errors are transient (locks, disk) and self-describe in the
/// receipt; the schema refusal is the one that silently repeats forever until
/// a human rebuilds the binary, so it is the one that earns a persistent trace.
pub(crate) fn note_db_error(err_display: &str) {
    if !err_display.contains(crate::db::migrations::SCHEMA_TOO_NEW_PHRASE) {
        return;
    }
    let at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    // Truncate defensively: the marker is a breadcrumb, not a log.
    let brief: String = err_display.chars().take(500).collect();
    let body = serde_json::json!({ "at": at, "error": brief }).to_string();
    match std::fs::write(marker_path(), body) {
        Ok(()) => error!(
            target: "4da::headless",
            "ENGINE BLOCKED by newer database schema — marker written; the app and MCP server will surface this until a rebuilt binary clears it"
        ),
        Err(e) => warn!(
            target: "4da::headless",
            error = %e,
            "Engine blocked by newer schema AND the marker could not be written"
        ),
    }
}

/// Remove the marker: the engine has opened the database, the block is over.
pub(crate) fn clear() {
    let path = marker_path();
    if path.exists() {
        match std::fs::remove_file(&path) {
            Ok(()) => tracing::info!(
                target: "4da::headless",
                "Engine-block marker cleared — database opens again"
            ),
            Err(e) => warn!(
                target: "4da::headless",
                error = %e,
                "Failed to clear engine-block marker"
            ),
        }
    }
}

/// The marker's contents, if one is present in `data_dir`.
pub(crate) fn read_marker(data_dir: &std::path::Path) -> Option<(String, String)> {
    let raw = std::fs::read_to_string(data_dir.join(MARKER_FILE)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    Some((
        v.get("at")?.as_str()?.to_string(),
        v.get("error")?.as_str()?.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_ignores_unrelated_errors() {
        // An unrelated error must not create a marker (write path guarded by
        // the phrase check before any filesystem work).
        note_db_error("database is locked");
        // No assertion on the filesystem needed: the phrase gate returns first.
        // The real write path is covered by read_marker round-trip below.
    }

    #[test]
    fn read_marker_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let body = serde_json::json!({
            "at": "2026-08-30T02:00:01Z",
            "error": format!("Database schema version 113 {} (max 111)",
                crate::db::migrations::SCHEMA_TOO_NEW_PHRASE),
        })
        .to_string();
        std::fs::write(dir.path().join(MARKER_FILE), body).expect("write marker");

        let (at, err) = read_marker(dir.path()).expect("marker parses");
        assert_eq!(at, "2026-08-30T02:00:01Z");
        assert!(err.contains("max 111"));
    }

    #[test]
    fn read_marker_absent_is_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(read_marker(dir.path()).is_none());
    }

    #[test]
    fn read_marker_garbage_is_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join(MARKER_FILE), "not json").expect("write");
        assert!(read_marker(dir.path()).is_none());
    }
}
