// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! When does startup pay for a whole-file `PRAGMA quick_check`?
//!
//! `quick_check` reads EVERY page of the database file; it skips only the
//! index-vs-table cross-checks that `integrity_check` adds. So its cost is
//! O(file size), not "cheap": on the founder's 2.0 GB corpus (510,616 pages)
//! it took 8.5 s in optimized SQLite and up to 84 s in a debug build, on every
//! GUI launch and every scheduled `--engine-once` run (2026-10-08).
//!
//! The check guards against corruption that opens cleanly and fails on read.
//! It does not need to run on every open. It runs when:
//!
//! - **no marker** — never verified (first start after this change, a fresh
//!   install upgrading, a marker deleted by hand);
//! - **stale marker** — the last `ok` is older than [`MAX_CHECK_AGE_SECS`];
//! - **unclean exit** — a process that opened this database died without its
//!   clean-shutdown path ([`detect_unclean_exits`]);
//! - **replaced file** — the database's creation time no longer matches the
//!   one recorded with the last `ok` (restored, swapped, recreated);
//! - **unreadable / future-dated marker** — never trusted.
//!
//! Otherwise it is skipped and the pre-flight reports
//! [`CorruptionRecovery::CheckSkipped`]. A skipped check is NOT evidence that
//! the file is intact: `state.rs` runs the check after all when `Database::new`
//! fails, before anything may quarantine the corpus (destruction requires
//! evidence, FAILURE_MODES "The corrupt-database fallback quarantined on ANY
//! error").
//!
//! ## Why per-process session files, not the watchdog's `.running`
//!
//! `startup_watchdog`'s `.running` marker is one file shared by the GUI and the
//! headless engine. The engine runs every ~30 min WHILE the GUI is up, sees the
//! GUI's live marker, and logs `prev_crashed=true` (e.g. 2026-10-08 17:25, six
//! minutes after a clean GUI start). Keyed on that, every engine run — the most
//! frequent opener — would scan anyway. Instead each opener writes
//! `<db>.session.<pid>` before it opens the database and removes it on its
//! clean-shutdown path; a leftover file whose PID is no longer alive is an
//! unclean exit. A live PID is a concurrent opener, not a crash.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use tracing::{debug, info, warn};

use super::migrations::{recover_corrupt_db_if_needed, CorruptionRecovery};

/// A successful check is trusted for this long.
pub(crate) const MAX_CHECK_AGE_SECS: u64 = 24 * 60 * 60;

/// Tolerated clock skew before a future-dated marker is distrusted.
const FUTURE_SKEW_SECS: u64 = 5 * 60;

const MARKER_VERSION: &str = "v1";

/// Why the pre-flight runs the check this time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CheckReason {
    NoMarker,
    MarkerUnreadable,
    MarkerStale { age_secs: u64 },
    MarkerFromFuture,
    DbReplaced,
    UncleanExit { sessions: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CheckDecision {
    Run(CheckReason),
    Skip { verified_at: u64 },
}

/// Contents of `<db>.quick_check_ok`: `v1 <verified_unix> <db_created_unix|0>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Marker {
    pub verified_at: u64,
    /// Creation time of the database file when it was verified; 0 if the
    /// platform/filesystem does not report one.
    pub db_created: u64,
}

pub(crate) fn marker_path(db_path: &Path) -> PathBuf {
    append_to_file_name(db_path, ".quick_check_ok")
}

fn session_prefix(db_path: &Path) -> String {
    let name = db_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "4da.db".to_string());
    format!("{name}.session.")
}

fn append_to_file_name(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn db_created_unix(db_path: &Path) -> u64 {
    std::fs::metadata(db_path)
        .and_then(|m| m.created())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `Ok(None)` = no marker; `Err(())` = a marker exists but cannot be trusted.
fn read_marker(db_path: &Path) -> Result<Option<Marker>, ()> {
    let raw = match std::fs::read_to_string(marker_path(db_path)) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    parse_marker(&raw).map(Some).ok_or(())
}

fn parse_marker(raw: &str) -> Option<Marker> {
    let mut parts = raw.split_whitespace();
    if parts.next()? != MARKER_VERSION {
        return None;
    }
    let verified_at = parts.next()?.parse().ok()?;
    let db_created = parts.next()?.parse().ok()?;
    Some(Marker {
        verified_at,
        db_created,
    })
}

/// Record a successful check. Called ONLY on a `quick_check` that returned `ok`.
fn write_marker(db_path: &Path, verified_at: u64) {
    let body = format!(
        "{MARKER_VERSION} {verified_at} {}\n",
        db_created_unix(db_path)
    );
    if let Err(e) = std::fs::write(marker_path(db_path), body) {
        warn!(target: "4da::db::recovery", error = %e, "Could not write quick_check marker — next start re-checks");
    }
}

/// Forget the last successful check (the file it vouched for was replaced,
/// quarantined, or failed a check).
pub(crate) fn invalidate(db_path: &Path) {
    let _ = std::fs::remove_file(marker_path(db_path));
}

/// Pure policy: should this start run the whole-file check?
pub(crate) fn decide(
    marker: Result<Option<Marker>, ()>,
    now: u64,
    db_created: u64,
    unclean_sessions: usize,
) -> CheckDecision {
    if unclean_sessions > 0 {
        return CheckDecision::Run(CheckReason::UncleanExit {
            sessions: unclean_sessions,
        });
    }
    let marker = match marker {
        Ok(Some(m)) => m,
        Ok(None) => return CheckDecision::Run(CheckReason::NoMarker),
        Err(()) => return CheckDecision::Run(CheckReason::MarkerUnreadable),
    };
    if marker.verified_at > now.saturating_add(FUTURE_SKEW_SECS) {
        return CheckDecision::Run(CheckReason::MarkerFromFuture);
    }
    let age_secs = now.saturating_sub(marker.verified_at);
    if age_secs >= MAX_CHECK_AGE_SECS {
        return CheckDecision::Run(CheckReason::MarkerStale { age_secs });
    }
    if marker.db_created != 0 && db_created != 0 && marker.db_created != db_created {
        return CheckDecision::Run(CheckReason::DbReplaced);
    }
    CheckDecision::Skip {
        verified_at: marker.verified_at,
    }
}

/// Session files left by openers that are no longer running. Our own PID's
/// file can only predate us (PID reuse) — it is stale too.
pub(crate) fn detect_unclean_exits(
    db_path: &Path,
    own_pid: u32,
    is_alive: &dyn Fn(u32) -> bool,
) -> Vec<PathBuf> {
    let Some(dir) = db_path.parent() else {
        return Vec::new();
    };
    let prefix = session_prefix(db_path);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut stale = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(pid) = name
            .strip_prefix(&prefix)
            .and_then(|p| p.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == own_pid || !is_alive(pid) {
            stale.push(entry.path());
        }
    }
    stale
}

/// Pre-flight entry point for `state.rs::get_database()`: decide, run the
/// check if needed, and keep the marker honest.
pub(crate) fn preflight(db_path: &Path) -> CorruptionRecovery {
    preflight_with(db_path, std::process::id(), now_unix(), &|pid| {
        crate::single_instance::is_process_alive(pid)
    })
}

pub(crate) fn preflight_with(
    db_path: &Path,
    own_pid: u32,
    now: u64,
    is_alive: &dyn Fn(u32) -> bool,
) -> CorruptionRecovery {
    if !db_path.exists() {
        invalidate(db_path);
        return CorruptionRecovery::NoExistingDb;
    }
    let stale_sessions = detect_unclean_exits(db_path, own_pid, is_alive);
    let decision = decide(
        read_marker(db_path),
        now,
        db_created_unix(db_path),
        stale_sessions.len(),
    );
    match decision {
        CheckDecision::Skip { verified_at } => {
            debug!(
                target: "4da::db::recovery",
                age_secs = now.saturating_sub(verified_at),
                "quick_check skipped — verified recently and every opener since exited cleanly"
            );
            CorruptionRecovery::CheckSkipped {
                verified_at_unix: verified_at,
            }
        }
        CheckDecision::Run(reason) => {
            info!(target: "4da::db::recovery", ?reason, "Running whole-file PRAGMA quick_check");
            let verdict = run_check(db_path, now);
            // Lock contention means no check happened: keep the evidence of
            // an unclean exit so the next start tries again.
            if !matches!(verdict, CorruptionRecovery::RecoveryFailed { .. }) {
                for f in &stale_sessions {
                    let _ = std::fs::remove_file(f);
                }
            }
            verdict
        }
    }
}

/// Run the real check and make the marker agree with its verdict.
fn run_check(db_path: &Path, now: u64) -> CorruptionRecovery {
    let verdict = recover_corrupt_db_if_needed(db_path);
    match &verdict {
        CorruptionRecovery::Healthy => write_marker(db_path, now),
        // Lock contention: no verdict either way — leave whatever was there.
        // (Prefix shared with both lock arms of `recover_corrupt_db_if_needed`.)
        CorruptionRecovery::RecoveryFailed { reason }
            if reason.starts_with("database locked during recovery") => {}
        _ => invalidate(db_path),
    }
    verdict
}

/// The check the pre-flight skipped, taken now because `Database::new` failed
/// and the caller is about to decide whether the file is corrupt.
pub(crate) fn verify_after_open_failure(db_path: &Path) -> CorruptionRecovery {
    warn!(
        target: "4da::db::recovery",
        "Database open failed after a skipped pre-flight — running quick_check before any recovery decision"
    );
    run_check(db_path, now_unix())
}

static SESSION_FILE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Announce that this process is about to open `db_path`. Written BEFORE the
/// open so a crash inside migrations is detected by the next start.
pub(crate) fn register_session(db_path: &Path) {
    let path = append_to_file_name(db_path, &format!(".session.{}", std::process::id()));
    match std::fs::write(&path, now_unix().to_string()) {
        Ok(()) => *SESSION_FILE.lock() = Some(path),
        Err(e) => {
            warn!(target: "4da::db::recovery", error = %e, "Could not write DB session file")
        }
    }
}

/// Clean-shutdown half of [`register_session`]. Called from
/// `startup_watchdog::mark_clean_shutdown`, which every clean exit path of the
/// GUI and the headless engine already runs.
pub(crate) fn release_session() {
    if let Some(path) = SESSION_FILE.lock().take() {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
#[path = "integrity_gate_tests.rs"]
mod tests;
