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

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tracing::{debug, info, warn};

use crate::state::get_db_path;

/// Time budget for Phase 0 (first-light). Exceeding this logs a warning
/// and triggers the `.stalled` marker. Phase 0 should end when the window
/// becomes visible.
///
/// Release builds get a tight 5-second budget because the bundled frontend
/// loads instantly and there's no dev-server poll. Debug builds get 10
/// seconds because:
///   - Vite dev server bootstrap can take 1-3s
///   - The dev-mode webview poll watchdog adds up to 2s wait-for-frontend
///   - Larger development databases (200MB+) take longer to migrate
///
/// A release build hitting 5s OR a debug build hitting 10s is a real
/// incident and warrants the `.stalled` marker for the next launch to
/// surface as a recovery toast.
#[cfg(not(debug_assertions))]
const PHASE0_BUDGET_SECS: u64 = 5;
#[cfg(debug_assertions)]
const PHASE0_BUDGET_SECS: u64 = 10;

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

/// Did the PREVIOUS session crash? Captured during `begin_startup_watch` and
/// consumed by `check_crash_loop`. Separating the read from the append lets
/// us keep `begin_startup_watch`'s sole responsibility unchanged (rotate the
/// running-marker) while letting the crash-loop detector know whether the
/// last boot was clean.
static PREV_CRASHED: AtomicBool = AtomicBool::new(false);

/// Whether the previous session exited uncleanly, as observed by the most
/// recent call to `begin_startup_watch`. Used by `check_crash_loop`.
pub fn previous_session_crashed() -> bool {
    PREV_CRASHED.load(Ordering::SeqCst)
}

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

    // Expose to the crash-loop detector (called later in pre-Tauri init).
    PREV_CRASHED.store(prev_crashed, Ordering::SeqCst);

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
    // A clean shutdown is the recovery signal for crash-loop detection —
    // clear the history so future boots start from a clean slate.
    clear_crash_history();
    info!(target: "4da::watchdog", "Clean shutdown markers removed");
}

/// Resolve the data directory from the canonical DB path.
fn data_dir() -> Option<PathBuf> {
    get_db_path().parent().map(std::path::Path::to_path_buf)
}

// ============================================================================
// Crash-loop protection
// ============================================================================

/// Result of the crash-loop inspection at startup.
///
/// - `Normal`: no crash burst detected, proceed with full init.
/// - `Warning(count)`: several recent crashes (3+ in the last 5 minutes) —
///   emit a banner to the frontend but keep normal init.
/// - `Critical(count)`: aggressive crash burst (3+ in the last 60 seconds) —
///   enter safe mode: skip background fetches, AWE, scheduler, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashLoopStatus {
    Normal,
    Warning(u32),
    Critical(u32),
}

/// Filename of the persisted crash history inside the data directory.
const CRASH_HISTORY_FILENAME: &str = ".crash_history.json";

/// Maximum entries kept in the crash history file. Older entries are trimmed.
const CRASH_HISTORY_MAX: usize = 5;

/// Window for Warning-level crash-loop detection.
const CRASH_WARNING_WINDOW_SECS: u64 = 5 * 60;

/// Window for Critical-level crash-loop detection.
const CRASH_CRITICAL_WINDOW_SECS: u64 = 60;

/// Minimum number of recent crashes to trigger a status upgrade.
const CRASH_LOOP_THRESHOLD: u32 = 3;

/// Global flag: when Critical, the rest of the code should short-circuit
/// background tasks, AWE calls, and the monitoring scheduler.
static SAFE_MODE: AtomicBool = AtomicBool::new(false);

/// Returns `true` if the current boot has been flagged as safe mode by
/// `check_crash_loop`. Callers that do heavy or network work should
/// bail out when this is true.
///
/// Wired into consumers in a follow-up wave (scheduler, AWE sync, background
/// fetchers). Allowed dead for now because the primary enforcement is via
/// the frontend `startup-crash-loop-critical` event.
#[allow(dead_code)]
pub fn is_safe_mode() -> bool {
    SAFE_MODE.load(Ordering::SeqCst)
}

/// Record a timestamp in the crash-history file and classify the current
/// startup against the recent history.
///
/// Call this **very early** — after the data_dir is known and the `.running`
/// marker has been inspected, but before any heavy init. The function is
/// infallible: filesystem errors degrade to `CrashLoopStatus::Normal` so
/// a broken history file can never itself trigger safe mode.
///
/// ## Algorithm
///
/// 1. Read the existing history (JSON array of unix-second timestamps).
/// 2. If the PREVIOUS session exited cleanly (detected by absence of the
///    `.running` marker at begin_startup_watch, signalled via
///    `prev_crashed` argument), do NOT append. A clean boot after a
///    crash-burst is the recovery signal — don't poison the history.
/// 3. If the previous session crashed, append the current timestamp.
/// 4. Trim to the most recent CRASH_HISTORY_MAX entries.
/// 5. Write back.
/// 6. Classify:
///    - 3+ entries within 60s → Critical (safe mode).
///    - 3+ entries within 5 min → Warning.
///    - else → Normal.
pub fn check_crash_loop(data_dir: &Path, prev_crashed: bool) -> CrashLoopStatus {
    let history_path = data_dir.join(CRASH_HISTORY_FILENAME);
    if let Err(e) = std::fs::create_dir_all(data_dir) {
        debug!(target: "4da::watchdog", error = %e, "Crash-loop check: could not ensure data dir");
        return CrashLoopStatus::Normal;
    }

    // Load existing history (best-effort).
    let mut history: Vec<u64> = read_crash_history(&history_path);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Only record a crash if the previous session didn't exit cleanly.
    if prev_crashed {
        history.push(now);
    }

    // Keep only the most recent N entries, sorted ascending so the
    // classifier can slice off the tail trivially.
    history.sort_unstable();
    if history.len() > CRASH_HISTORY_MAX {
        let drop_n = history.len() - CRASH_HISTORY_MAX;
        history.drain(0..drop_n);
    }

    // Persist back (best-effort; an error here doesn't change our status).
    if let Err(e) = write_crash_history(&history_path, &history) {
        debug!(target: "4da::watchdog", error = %e, "Crash-loop check: could not write history");
    }

    let status = classify_crash_loop(&history, now);

    match status {
        CrashLoopStatus::Critical(n) => {
            warn!(
                target: "4da::watchdog",
                crashes = n,
                window_secs = CRASH_CRITICAL_WINDOW_SECS,
                "CRITICAL crash loop detected — entering safe mode"
            );
            SAFE_MODE.store(true, Ordering::SeqCst);
        }
        CrashLoopStatus::Warning(n) => {
            warn!(
                target: "4da::watchdog",
                crashes = n,
                window_secs = CRASH_WARNING_WINDOW_SECS,
                "Recent crash activity — frontend will show recovery banner"
            );
        }
        CrashLoopStatus::Normal => {
            debug!(target: "4da::watchdog", history_entries = history.len(), "Crash history OK");
        }
    }

    status
}

/// Clear the crash history — call after a successful clean shutdown OR
/// after the app has been running for long enough that any prior burst
/// can no longer reasonably be considered a loop.
pub fn clear_crash_history() {
    let Some(dir) = data_dir() else {
        return;
    };
    let path = dir.join(CRASH_HISTORY_FILENAME);
    match std::fs::remove_file(&path) {
        Ok(()) => info!(target: "4da::watchdog", "Crash history cleared (clean session)"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => debug!(target: "4da::watchdog", error = %e, "Failed to clear crash history"),
    }
    // Also clear the safe-mode flag — a clean shutdown means we're recovered.
    SAFE_MODE.store(false, Ordering::SeqCst);
}

/// Pure classifier — broken out so it can be unit-tested without the
/// filesystem. `history` MUST be sorted ascending by timestamp.
fn classify_crash_loop(history: &[u64], now_secs: u64) -> CrashLoopStatus {
    // Count entries within the critical window first (stricter threshold).
    let critical_cutoff = now_secs.saturating_sub(CRASH_CRITICAL_WINDOW_SECS);
    let critical_count = history.iter().filter(|&&t| t >= critical_cutoff).count() as u32;
    if critical_count >= CRASH_LOOP_THRESHOLD {
        return CrashLoopStatus::Critical(critical_count);
    }

    let warning_cutoff = now_secs.saturating_sub(CRASH_WARNING_WINDOW_SECS);
    let warning_count = history.iter().filter(|&&t| t >= warning_cutoff).count() as u32;
    if warning_count >= CRASH_LOOP_THRESHOLD {
        return CrashLoopStatus::Warning(warning_count);
    }

    CrashLoopStatus::Normal
}

fn read_crash_history(path: &Path) -> Vec<u64> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    if bytes.is_empty() {
        return Vec::new();
    }
    // Parse a simple JSON array of numbers. A corrupt file shouldn't
    // propagate deserialization errors up — we just start fresh.
    match serde_json::from_slice::<Vec<u64>>(&bytes) {
        Ok(v) => v,
        Err(e) => {
            debug!(target: "4da::watchdog", error = %e, "Crash history parse failed; starting fresh");
            Vec::new()
        }
    }
}

fn write_crash_history(path: &Path, history: &[u64]) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(history).map_err(std::io::Error::other)?;
    std::fs::write(path, bytes)
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
        // regression gets caught. 5s in release / 10s in debug is the
        // current compromise — debug needs more headroom for the Vite
        // dev-server bootstrap and the webview poll watchdog.
        assert!(PHASE0_BUDGET_SECS >= 2);
        assert!(PHASE0_BUDGET_SECS <= 15);
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

    // ------------------------------------------------------------------
    // Crash-loop classifier tests
    // ------------------------------------------------------------------

    #[test]
    fn classify_empty_history_is_normal() {
        assert_eq!(classify_crash_loop(&[], 1_000_000), CrashLoopStatus::Normal);
    }

    #[test]
    fn classify_three_crashes_within_60s_is_critical() {
        // Three timestamps all within 60 seconds of "now" must escalate
        // to Critical — that's the safe-mode trigger.
        let now: u64 = 10_000;
        let history = [now - 30, now - 20, now - 5];
        assert_eq!(
            classify_crash_loop(&history, now),
            CrashLoopStatus::Critical(3)
        );
    }

    #[test]
    fn classify_three_crashes_within_5min_is_warning_not_critical() {
        // Three crashes spread across several minutes should not trigger
        // Critical (which would needlessly disable background services),
        // but should show a Warning banner.
        let now: u64 = 10_000;
        let history = [now - 270, now - 180, now - 90]; // all within 5 min, none within 60s
        assert_eq!(
            classify_crash_loop(&history, now),
            CrashLoopStatus::Warning(3)
        );
    }

    #[test]
    fn classify_old_crashes_are_ignored() {
        // A history of old crashes (more than 5 minutes ago) is considered
        // recovered — Normal.
        let now: u64 = 100_000;
        let history = [now - 3_600, now - 1_800, now - 700]; // all > 5 min old
        assert_eq!(classify_crash_loop(&history, now), CrashLoopStatus::Normal);
    }

    #[test]
    fn check_crash_loop_appends_when_previous_crashed() {
        use std::io::Write;
        let tmp = tempfile::TempDir::new().expect("tempdir");
        // Seed history with two recent crashes so one more will trip Critical.
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let seed = format!("[{a},{b}]", a = now - 30, b = now - 20);
        let path = tmp.path().join(CRASH_HISTORY_FILENAME);
        let mut f = std::fs::File::create(&path).expect("seed");
        f.write_all(seed.as_bytes()).expect("write seed");
        drop(f);

        let status = check_crash_loop(tmp.path(), /* prev_crashed */ true);
        assert!(
            matches!(status, CrashLoopStatus::Critical(n) if n >= 3),
            "three crashes within 60s should be Critical, got {status:?}"
        );
        assert!(is_safe_mode(), "Critical must set safe-mode flag");

        // Cleanup: clear safe mode so other tests aren't polluted.
        SAFE_MODE.store(false, Ordering::SeqCst);
    }

    #[test]
    fn check_crash_loop_does_not_append_on_clean_previous() {
        // If previous session exited cleanly, prev_crashed=false, and the
        // function MUST NOT append a spurious timestamp.
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let status = check_crash_loop(tmp.path(), /* prev_crashed */ false);
        assert_eq!(status, CrashLoopStatus::Normal);
        let history_path = tmp.path().join(CRASH_HISTORY_FILENAME);
        let contents = std::fs::read_to_string(&history_path).unwrap_or_default();
        // History should be empty array (or the file may not exist if
        // write failed silently on CI) — either way no timestamps.
        let parsed: Vec<u64> = serde_json::from_str(&contents).unwrap_or_default();
        assert!(
            parsed.is_empty(),
            "clean-previous boot must not add a crash timestamp; got {parsed:?}"
        );
    }
}
