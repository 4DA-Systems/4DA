// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Database-recovery marker — the headless engine's voice for a restore or a
//! quarantine.
//!
//! `state.rs::get_database` repairs a corrupt database before opening it
//! (restore from a `*.db.backup.vN`, or quarantine it and start fresh) and
//! records the outcome in a per-process notice. Only
//! `startup_health::check_database` — the desktop GUI — ever read that notice.
//! The scheduled background refresh runs `run_headless` in its OWN process, so
//! a recovery there was consumed by nobody: the one path where silent data
//! loss is most likely (an unattended engine opening the corpus at 3am) had no
//! voice at all (2026-09-10 audit). The 296 MB corpus renamed away on
//! 2026-08-16 is why that matters.
//!
//! The headless engine now persists the notice as `data/.db-recovered`. The
//! desktop app's startup health check shows it once and removes it; the MCP
//! server's `data_freshness` block surfaces it while it stands. Same shape and
//! lifetime rules as [`crate::engine_block`].

use std::path::{Path, PathBuf};

use tracing::{error, warn};

use crate::db::migrations::CorruptionRecovery;

/// Marker file name, created in the data directory beside `4da.db`.
pub(crate) const MARKER_FILE: &str = ".db-recovered";

/// Longest `detail` the marker keeps: it is a breadcrumb, not a log.
const MAX_DETAIL_CHARS: usize = 500;

/// A persisted recovery outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryMarker {
    /// When the engine recorded it (ISO-8601 UTC).
    pub at: String,
    /// `restored_from_backup`, `quarantined_no_backup`, `recovery_failed`, or
    /// `unreadable_marker` when the file exists but cannot be parsed.
    pub kind: String,
    /// The backup used, the quarantined file, or the failure reason.
    pub detail: String,
}

/// Kind label and detail for a recovery outcome; `None` for the healthy paths,
/// which leave no marker.
fn describe(notice: &CorruptionRecovery) -> Option<(&'static str, String)> {
    match notice {
        CorruptionRecovery::Healthy | CorruptionRecovery::NoExistingDb => None,
        CorruptionRecovery::RestoredFromBackup { restored_from } => {
            Some(("restored_from_backup", restored_from.display().to_string()))
        }
        CorruptionRecovery::QuarantinedNoBackup { quarantined_to } => Some((
            "quarantined_no_backup",
            quarantined_to.display().to_string(),
        )),
        CorruptionRecovery::RecoveryFailed { reason } => Some(("recovery_failed", reason.clone())),
    }
}

/// Write the marker for a non-healthy `notice` into `data_dir`. Returns whether
/// a marker was written. Never panics and never returns an error: a failure to
/// write is logged, because the recovery itself already happened.
pub(crate) fn record_in(data_dir: &Path, notice: &CorruptionRecovery) -> bool {
    let Some((kind, detail)) = describe(notice) else {
        return false;
    };
    let at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let detail: String = detail.chars().take(MAX_DETAIL_CHARS).collect();
    let body = serde_json::json!({ "at": at, "kind": kind, "detail": detail }).to_string();
    match std::fs::write(data_dir.join(MARKER_FILE), body) {
        Ok(()) => {
            error!(
                target: "4da::headless",
                kind,
                detail = %detail,
                "DATABASE RECOVERED by the background refresh — marker written; the app and the MCP server will surface it"
            );
            true
        }
        Err(e) => {
            warn!(
                target: "4da::headless",
                error = %e,
                kind,
                "Database was recovered AND the recovery marker could not be written"
            );
            false
        }
    }
}

/// Consume this process's recovery notice (one-shot) and persist it. The
/// headless engine calls this after its database has been opened; a GUI
/// process never calls it (its startup health check consumes the notice).
pub(crate) fn persist_pending_notice() {
    if let Some(notice) = crate::db::migrations::take_db_recovery_notice() {
        record_in(&data_dir(), &notice);
    }
}

fn data_dir() -> PathBuf {
    crate::runtime_paths::RuntimePaths::get().data_dir.clone()
}

/// The marker in `data_dir`, if one exists. A file that exists but cannot be
/// parsed is still reported (`unreadable_marker`): something wrote it, and
/// silence is the failure this module exists to remove.
pub(crate) fn read(data_dir: &Path) -> Option<RecoveryMarker> {
    let raw = std::fs::read_to_string(data_dir.join(MARKER_FILE)).ok()?;
    let parsed = serde_json::from_str::<serde_json::Value>(&raw)
        .ok()
        .and_then(|v| {
            Some(RecoveryMarker {
                at: v.get("at")?.as_str()?.to_string(),
                kind: v.get("kind")?.as_str()?.to_string(),
                detail: v
                    .get("detail")
                    .and_then(|d| d.as_str())
                    .unwrap_or_default()
                    .to_string(),
            })
        });
    Some(parsed.unwrap_or_else(|| RecoveryMarker {
        at: "unknown".to_string(),
        kind: "unreadable_marker".to_string(),
        detail: raw.chars().take(MAX_DETAIL_CHARS).collect(),
    }))
}

/// Read the marker and remove it: the desktop app reports it exactly once.
pub(crate) fn take(data_dir: &Path) -> Option<RecoveryMarker> {
    let marker = read(data_dir)?;
    if let Err(e) = std::fs::remove_file(data_dir.join(MARKER_FILE)) {
        warn!(target: "4da::startup", error = %e, "Could not remove the database-recovery marker after reporting it");
    }
    Some(marker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_outcomes_leave_no_marker() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(!record_in(dir.path(), &CorruptionRecovery::Healthy));
        assert!(!record_in(dir.path(), &CorruptionRecovery::NoExistingDb));
        assert!(read(dir.path()).is_none());
    }

    #[test]
    fn every_bad_outcome_round_trips() {
        let cases = [
            (
                CorruptionRecovery::RestoredFromBackup {
                    restored_from: PathBuf::from("/data/4da.db.backup.v121"),
                },
                "restored_from_backup",
                "4da.db.backup.v121",
            ),
            (
                CorruptionRecovery::QuarantinedNoBackup {
                    quarantined_to: PathBuf::from("/data/4da.db.corrupt"),
                },
                "quarantined_no_backup",
                "4da.db.corrupt",
            ),
            (
                CorruptionRecovery::RecoveryFailed {
                    reason: "permission denied".to_string(),
                },
                "recovery_failed",
                "permission denied",
            ),
        ];
        for (notice, kind, detail_fragment) in cases {
            let dir = tempfile::tempdir().expect("tempdir");
            assert!(record_in(dir.path(), &notice));
            let marker = read(dir.path()).expect("marker present");
            assert_eq!(marker.kind, kind);
            assert!(marker.detail.contains(detail_fragment), "{marker:?}");
            assert!(marker.at.ends_with('Z'), "ISO UTC stamp: {}", marker.at);
        }
    }

    #[test]
    fn take_reports_once_then_the_marker_is_gone() {
        let dir = tempfile::tempdir().expect("tempdir");
        record_in(
            dir.path(),
            &CorruptionRecovery::QuarantinedNoBackup {
                quarantined_to: PathBuf::from("/data/4da.db.corrupt"),
            },
        );
        assert!(take(dir.path()).is_some());
        assert!(take(dir.path()).is_none(), "one-shot");
        assert!(!dir.path().join(MARKER_FILE).exists());
    }

    #[test]
    fn an_unparseable_marker_is_still_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join(MARKER_FILE), "not json").expect("write");
        let marker = read(dir.path()).expect("reported, not swallowed");
        assert_eq!(marker.kind, "unreadable_marker");
        assert_eq!(marker.detail, "not json");
    }
}
