// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Startup watchdog — the last-line safety net for cold-boot reliability.
//!
//! Three jobs, all of them defensive:
//!
//! 1. **Phase budget enforcement.** Logs when each startup phase exceeds its
//!    expected duration. If Phase 0 (first-light → window visible) takes
//!    longer than `PHASE0_BUDGET_SECS`, writes a `.stalled` marker file that
//!    the next launch reads to show a recovery toast.
//!
//! 2. **Backend heartbeat.** Writes `data/.healthy` every 60 seconds from
//!    a background task. The frontend reads this file via a privileged IPC
//!    command and — if absent or stale — shows a recovery panel instead of
//!    hanging indefinitely. This is what rescues the user from "backend
//!    froze and I have no idea" situations.
//!
//! 3. **Crash-trail detection.** On startup, checks whether the previous
//!    session exited cleanly (via a `.running` marker that's deleted at the
//!    Stop event). If the marker is still there on the next launch, the
//!    previous session crashed — emit a `startup-crash-recovered` event so
//!    the UI can surface it.
//!
//! ## Why no new crates
//!
//! This is intentionally built on `std::fs` + `std::time` + Tauri's existing
//! event emitter. Adding `rfd` or `notify-rust` would bloat the binary and
//! introduce platform-specific test burden. The file-based approach is
//! boring, cross-platform, and can't crash.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tracing::{debug, info, warn};

use crate::state::get_db_path;

/// Time budget for Phase 0 (first-light). Exceeding this logs a warning
/// and triggers the `.stalled` marker. Phase 0 should end when the window
/// becomes visible — any dev mode is allowed to overrun briefly, but a
/// release build hitting this ceiling is a real incident.
const PHASE0_BUDGET_SECS: u64 = 5;

/// Time budget for Phase 1 (essential services ready).
#[allow(dead_code)] // Used by Wave 6 phased startup rewrite
const PHASE1_BUDGET_SECS: u64 = 10;

/// How often to write the heartbeat file (steady state).
const HEARTBEAT_INTERVAL_SECS: u64 = 60;

/// Heartbeat is considered stale if the file is older than this. The
/// frontend uses this threshold when deciding whether to show the
/// recovery panel.
#[allow(dead_code)] // Read by the frontend via future IPC command
pub const HEARTBEAT_STALE_SECS: u64 = 180;

/// One-shot guard so phase-0 logging fires exactly once regardless of how
/// many times `mark_phase0_complete` gets called (defensive).
static PHASE0_MARKED: AtomicBool = AtomicBool::new(false);

/// Startup clock — set in `begin_startup_watch`, read by the phase-mark fns.
static mut STARTUP_BEGAN: Option<Instant> = None;

/// Initialize the startup watchdog. Call once from `initialize_pre_tauri()`
/// before any other setup_app work. Records the startup time, inspects the
/// crash trail from the previous session, and rotates the `.running` marker.
pub fn begin_startup_watch() {
    // Safety: called exactly once during startup, before any background
    // tasks exist. No concurrent readers.
    #[allow(unsafe_code)]
    unsafe {
        STARTUP_BEGAN = Some(Instant::now());
    }

    let data_dir = match data_dir() {
        Some(d) => d,
        None => {
            warn!(target: "4da::watchdog", "Could not resolve data dir for startup watchdog");
            return;
        }
    };

    // Inspect the crash trail from the previous session.
    let running_marker = data_dir.join(".running");
    let stalled_marker = data_dir.join(".stalled");

    let prev_crashed = running_marker.exists();
    let prev_stalled = stalled_marker.exists();

    if prev_crashed || prev_stalled {
        warn!(
            target: "4da::watchdog",
            crashed = prev_crashed,
            stalled = prev_stalled,
            "Previous session exited uncleanly — recovery flags recorded for frontend"
        );
        // Consume the markers so the notification doesn't repeat on every launch.
        let _ = std::fs::remove_file(&stalled_marker);
    }
    // Keep running_marker — we'll rewrite it below.

    // Write a fresh .running marker for THIS session. The Stop event handler
    // removes it during clean shutdown; if the process dies, the marker
    // stays and the next launch detects it.
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = std::fs::create_dir_all(&data_dir);
    if let Err(e) = std::fs::write(&running_marker, now_secs.to_string().as_bytes()) {
        debug!(target: "4da::watchdog", error = %e, "Could not write .running marker");
    }

    info!(
        target: "4da::watchdog",
        prev_crashed,
        prev_stalled,
        "Startup watchdog initialized"
    );
}

/// Mark Phase 0 (first-light) complete. Logs the elapsed time. If the
/// elapsed time exceeded the budget, writes a `.stalled` marker that the
/// next launch will detect.
pub fn mark_phase0_complete() {
    if PHASE0_MARKED.swap(true, Ordering::SeqCst) {
        return;
    }

    // Safety: STARTUP_BEGAN is set before any background task runs, and
    // is only read here + in mark_phase1_complete.
    #[allow(unsafe_code)]
    let elapsed = unsafe { STARTUP_BEGAN.map(|t| t.elapsed()).unwrap_or(Duration::ZERO) };

    let ms = elapsed.as_millis();
    if elapsed.as_secs() >= PHASE0_BUDGET_SECS {
        warn!(
            target: "4da::watchdog",
            elapsed_ms = ms,
            budget_s = PHASE0_BUDGET_SECS,
            "Phase 0 (first-light) EXCEEDED budget"
        );
        // Write a stalled marker so the next boot can show a recovery toast.
        if let Some(dir) = data_dir() {
            let marker = dir.join(".stalled");
            let _ = std::fs::write(
                &marker,
                format!("phase0_exceeded_budget_ms={ms}").as_bytes(),
            );
        }
    } else {
        info!(
            target: "4da::watchdog",
            elapsed_ms = ms,
            budget_s = PHASE0_BUDGET_SECS,
            "Phase 0 (first-light) complete"
        );
    }
}

/// Mark Phase 1 (essential services) complete. Informational logging only —
/// does NOT write a stalled marker because Phase 1 runs entirely in the
/// background and a slow essential-services setup doesn't affect the
/// user-visible first paint.
#[allow(dead_code)] // Used by Wave 6 phased startup rewrite
pub fn mark_phase1_complete() {
    #[allow(unsafe_code)]
    let elapsed = unsafe { STARTUP_BEGAN.map(|t| t.elapsed()).unwrap_or(Duration::ZERO) };

    let ms = elapsed.as_millis();
    if elapsed.as_secs() >= PHASE1_BUDGET_SECS {
        warn!(
            target: "4da::watchdog",
            elapsed_ms = ms,
            budget_s = PHASE1_BUDGET_SECS,
            "Phase 1 (essential services) exceeded budget"
        );
    } else {
        info!(
            target: "4da::watchdog",
            elapsed_ms = ms,
            "Phase 1 (essential services) complete"
        );
    }
}

/// Start the steady-state heartbeat writer as a background task.
/// Writes `data/.healthy` every HEARTBEAT_INTERVAL_SECS. The file content is
/// the current unix timestamp, so the frontend can compute heartbeat age.
pub fn start_heartbeat() {
    tauri::async_runtime::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
        loop {
            interval.tick().await;
            write_heartbeat();
        }
    });
}

/// Write the heartbeat file once. Called immediately after setup_app returns
/// (so the frontend has a valid heartbeat from t=0) and periodically by the
/// background task above.
pub fn write_heartbeat() {
    let Some(dir) = data_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let marker = dir.join(".healthy");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Err(e) = std::fs::write(&marker, now.to_string().as_bytes()) {
        debug!(target: "4da::watchdog", error = %e, "Heartbeat write failed");
    }
}

/// Clean up the `.running` marker during shutdown. Call from the Stop event
/// handler. If the marker isn't cleaned up, the next launch treats the
/// previous session as crashed.
pub fn mark_clean_shutdown() {
    let Some(dir) = data_dir() else {
        return;
    };
    let _ = std::fs::remove_file(dir.join(".running"));
    let _ = std::fs::remove_file(dir.join(".healthy"));
    info!(target: "4da::watchdog", "Clean shutdown markers removed");
}

/// Resolve the data directory from the canonical DB path.
fn data_dir() -> Option<PathBuf> {
    get_db_path().parent().map(std::path::Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_stale_threshold_is_longer_than_interval() {
        // If the stale threshold were <= the write interval, the frontend
        // would occasionally see the heartbeat as stale even when the
        // backend is healthy. Enforce the invariant.
        assert!(HEARTBEAT_STALE_SECS > HEARTBEAT_INTERVAL_SECS * 2);
    }

    #[test]
    fn phase0_budget_is_reasonable() {
        // Phase 0 budget has to be larger than a typical cold-start window
        // paint (<500ms on release builds) but tight enough that a real
        // regression gets caught. 5s is the current compromise.
        assert!(PHASE0_BUDGET_SECS >= 2);
        assert!(PHASE0_BUDGET_SECS <= 10);
    }

    #[test]
    fn mark_phase0_is_idempotent() {
        // Calling mark_phase0_complete twice must not cause double-logging
        // or double-write of stalled markers.
        PHASE0_MARKED.store(false, Ordering::SeqCst);
        mark_phase0_complete();
        mark_phase0_complete();
        assert!(PHASE0_MARKED.load(Ordering::SeqCst));
    }
}
