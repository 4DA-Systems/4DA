// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
//!    the UI can surface it. The trail belongs to the process holding the
//!    single-instance lock (`claim_crash_trail`); the headless engine and a
//!    rejected second launch never read or remove it.
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
/// loads instantly and there's no dev-server poll. Debug builds get 45
/// seconds because:
///   - Vite dev server bootstrap can take 1-3s
///   - On a COLD dev start Vite transforms the entire module graph on the
///     first request (~15-20s) before `main.tsx` executes and emits
///     `frontend-ready`; the poll loop in app_setup waits up to 40s for it
///   - Larger development databases (200MB+) take longer to migrate
/// A debug start exceeding 45s is a real stall (not just a cold transform).
///
/// A release build hitting 5s OR a debug build hitting 10s is a real
/// incident and warrants the `.stalled` marker for the next launch to
/// surface as a recovery toast.
#[cfg(not(debug_assertions))]
const PHASE0_BUDGET_SECS: u64 = 5;
#[cfg(debug_assertions)]
const PHASE0_BUDGET_SECS: u64 = 45;

/// Phase 1 (process start → `setup_app` essential services done) is three
/// segments with different owners, each budgeted on its own. One 10 s budget
/// for the sum said nothing about WHICH part regressed: a fresh debug profile
/// measured 20.9 s = pre-Tauri 7.5 s + Tauri build 9.1 s (first WebView2
/// profile) + setup_app 2.2 s (audit 2026-10-07). Every segment still warns
/// when it exceeds its budget, so a real regression is never averaged away.
///
/// - `pre_tauri`: logging, crash gates, DB open + migrations, context engine.
/// - `tauri_build`: `Builder::build` up to the `setup` callback (plugins,
///   window + WebView2 creation — a first-run profile is created here).
/// - `setup_app`: tray, monitoring, scheduler (`setup_app` phase 0).
#[cfg(not(debug_assertions))]
const PRE_TAURI_BUDGET: Duration = Duration::from_secs(3);
#[cfg(debug_assertions)]
const PRE_TAURI_BUDGET: Duration = Duration::from_secs(8);
#[cfg(not(debug_assertions))]
const TAURI_BUILD_BUDGET: Duration = Duration::from_secs(5);
#[cfg(debug_assertions)]
const TAURI_BUILD_BUDGET: Duration = Duration::from_secs(12);
const SETUP_APP_BUDGET: Duration = Duration::from_secs(3);

/// When pre-Tauri init finished / when `setup_app` began. Unset in the
/// headless engine, which never builds a Tauri app.
static PRE_TAURI_DONE: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
static SETUP_BEGAN: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// How often to write the heartbeat file (steady state).
const HEARTBEAT_INTERVAL_SECS: u64 = 60;

/// One-shot guard so phase-0 logging fires exactly once regardless of how
/// many times `mark_phase0_complete` gets called (defensive).
static PHASE0_MARKED: AtomicBool = AtomicBool::new(false);

/// Startup clock — set in `begin_startup_watch`, read by the phase-mark fns.
static mut STARTUP_BEGAN: Option<Instant> = None;

/// Did the PREVIOUS session crash? Captured by `claim_crash_trail` and
/// consumed by `check_crash_loop`. Separating the read from the append lets
/// the claim keep its sole responsibility (rotate the running-marker) while
/// the crash-loop detector learns whether the last boot was clean.
static PREV_CRASHED: AtomicBool = AtomicBool::new(false);

/// Whether this process claimed the crash trail, i.e. holds the
/// single-instance lock. Only the owner rotates `.running` at start and
/// clears the markers and the crash history at a clean exit.
static OWNS_CRASH_TRAIL: AtomicBool = AtomicBool::new(false);

/// Whether the previous session exited uncleanly, as observed by
/// `claim_crash_trail`. Used by `check_crash_loop`.
pub fn previous_session_crashed() -> bool {
    PREV_CRASHED.load(Ordering::SeqCst)
}

/// Start the startup clock. Call once from `initialize_pre_tauri()` before
/// any other setup work. Touches no marker file: every process starts the
/// clock (the headless engine, and a second launch the single-instance lock
/// is about to turn away), but only the lock holder owns the crash trail
/// (`claim_crash_trail`).
pub fn begin_startup_watch() {
    // Safety: called exactly once during startup, before any background
    // tasks exist. No concurrent readers.
    #[allow(unsafe_code)]
    unsafe {
        STARTUP_BEGAN = Some(Instant::now());
    }
}

/// Inspect the previous session's crash trail and rotate the `.running`
/// marker. GUI only, called once the single-instance lock is held.
///
/// Until 2026-11-01 every process did this at `begin_startup_watch`. The
/// headless engine (every 10 minutes, alongside the GUI) and a second launch
/// the lock rejected both read the LIVE GUI's `.running` as a crash and
/// deleted its `.stalled` marker. Then the engine's clean exit deleted the
/// GUI's `.running` and cleared the crash history, so a GUI crash after the
/// next engine run was never detected.
pub fn claim_crash_trail() {
    let Some(dir) = data_dir() else {
        warn!(target: "4da::watchdog", "Could not resolve data dir for the crash trail");
        return;
    };
    let (prev_crashed, prev_stalled) = claim_crash_trail_in(&dir);
    // Expose to the crash-loop detector (called next in pre-Tauri init).
    PREV_CRASHED.store(prev_crashed, Ordering::SeqCst);
    OWNS_CRASH_TRAIL.store(true, Ordering::SeqCst);
    info!(
        target: "4da::watchdog",
        prev_crashed,
        prev_stalled,
        "Startup watchdog initialized"
    );
}

/// [`claim_crash_trail`] against an explicit data directory. Returns
/// `(prev_crashed, prev_stalled)`.
fn claim_crash_trail_in(data_dir: &Path) -> (bool, bool) {
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
    let _ = std::fs::create_dir_all(data_dir);
    if let Err(e) = std::fs::write(&running_marker, now_secs.to_string().as_bytes()) {
        debug!(target: "4da::watchdog", error = %e, "Could not write .running marker");
    }
    (prev_crashed, prev_stalled)
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

/// Leave `waited` out of the startup phases: time spent waiting on something
/// outside the app (a previous instance's WebView2 releasing the profile) is
/// not a stall of ours, so it must not trip the budgets or the `.stalled`
/// marker. Moves the startup clock forward. Call during single-threaded
/// startup, before Phase 0 or Phase 1 is marked.
pub fn exclude_from_startup_clock(waited: Duration) {
    // Safety: as for `begin_startup_watch`, this runs on the startup thread
    // before the phase marks read the clock, with no concurrent readers.
    #[allow(unsafe_code)]
    unsafe {
        STARTUP_BEGAN = STARTUP_BEGAN.map(|t| t + waited);
    }
}

/// Pre-Tauri init is done; `Builder::build` starts next. GUI only.
pub fn mark_pre_tauri_complete() {
    let _ = PRE_TAURI_DONE.set(Instant::now());
}

/// The Tauri `setup` callback started: `Builder::build` has created the
/// plugins and the window. GUI only.
pub fn mark_setup_began() {
    let _ = SETUP_BEGAN.set(Instant::now());
}

/// One startup segment: how long it took against its own budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Segment {
    name: &'static str,
    elapsed: Duration,
    budget: Duration,
}

impl Segment {
    fn over_budget(&self) -> bool {
        self.elapsed >= self.budget
    }
}

/// Split process-start → now at the recorded boundaries. A missing boundary
/// (headless, or a mark that never ran) drops the segments it would bound
/// rather than inventing a number.
fn phase1_segments(
    began: Instant,
    pre_tauri_done: Option<Instant>,
    setup_began: Option<Instant>,
    now: Instant,
) -> Vec<Segment> {
    let (Some(pre), Some(setup)) = (pre_tauri_done, setup_began) else {
        return Vec::new();
    };
    vec![
        Segment {
            name: "pre_tauri",
            elapsed: pre.saturating_duration_since(began),
            budget: PRE_TAURI_BUDGET,
        },
        Segment {
            name: "tauri_build",
            elapsed: setup.saturating_duration_since(pre),
            budget: TAURI_BUILD_BUDGET,
        },
        Segment {
            name: "setup_app",
            elapsed: now.saturating_duration_since(setup),
            budget: SETUP_APP_BUDGET,
        },
    ]
}

/// Mark Phase 1 (essential services) complete and attribute its time to
/// pre-Tauri init, Tauri build and `setup_app`, each against its own budget.
/// Logging only — no stalled marker: Phase 1 does not gate first paint.
pub fn mark_phase1_complete() {
    #[allow(unsafe_code)]
    let began = unsafe { STARTUP_BEGAN };
    let Some(began) = began else {
        return;
    };
    let now = Instant::now();
    let segments = phase1_segments(
        began,
        PRE_TAURI_DONE.get().copied(),
        SETUP_BEGAN.get().copied(),
        now,
    );
    for seg in &segments {
        let ms = seg.elapsed.as_millis();
        let budget_ms = seg.budget.as_millis();
        if seg.over_budget() {
            warn!(target: "4da::watchdog", segment = seg.name, elapsed_ms = ms, budget_ms, "Startup segment exceeded budget");
        } else {
            info!(target: "4da::watchdog", segment = seg.name, elapsed_ms = ms, budget_ms, "Startup segment within budget");
        }
    }
    info!(
        target: "4da::watchdog",
        elapsed_ms = now.saturating_duration_since(began).as_millis(),
        over_budget = segments.iter().filter(|s| s.over_budget()).count(),
        "Phase 1 (essential services) complete"
    );
}

/// Times a pre-Tauri init step: enters a tracing span named `step` for its
/// lifetime and logs `elapsed_ms` when dropped, so the cold-boot log
/// attributes time that used to be unaccounted for.
pub struct StepTimer {
    step: &'static str,
    began: Instant,
    _span: tracing::span::EnteredSpan,
}

/// Start a [`StepTimer`]. Hold the guard for the duration of the step.
pub fn time_step(step: &'static str) -> StepTimer {
    StepTimer {
        step,
        began: Instant::now(),
        _span: tracing::info_span!(target: "4da::startup", "startup_step", step).entered(),
    }
}

impl Drop for StepTimer {
    fn drop(&mut self) {
        info!(target: "4da::startup", step = self.step, elapsed_ms = self.began.elapsed().as_millis(), "Startup step timed");
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
///
/// Every process releases its own integrity session. Only the crash-trail
/// owner (`claim_crash_trail`) removes `.running` / `.healthy` and clears the
/// crash history: the headless engine and the console-control handler call
/// this too, and they must not erase a live GUI's trail.
pub fn mark_clean_shutdown() {
    let Some(dir) = data_dir() else {
        return;
    };
    let owns_trail = OWNS_CRASH_TRAIL.load(Ordering::SeqCst);
    clean_shutdown_in(&dir, owns_trail);
    // This process's `<db>.session.<pid>`: a clean exit is what lets the next
    // start skip the whole-file integrity scan (`db::integrity_gate`).
    crate::db::integrity_gate::release_session();
    if owns_trail {
        info!(target: "4da::watchdog", "Clean shutdown markers removed");
    } else {
        info!(target: "4da::watchdog", "Clean shutdown; the crash-trail markers belong to the instance-lock holder and stay");
    }
}

/// The marker half of [`mark_clean_shutdown`] against an explicit directory.
/// A no-op unless this process owns the crash trail.
fn clean_shutdown_in(dir: &Path, owns_trail: bool) {
    if !owns_trail {
        return;
    }
    let _ = std::fs::remove_file(dir.join(".running"));
    let _ = std::fs::remove_file(dir.join(".healthy"));
    // A clean shutdown is the recovery signal for crash-loop detection —
    // clear the history so future boots start from a clean slate.
    clear_crash_history_in(dir);
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
///   enter safe mode: skip background fetches, scheduler, etc.
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
/// background tasks and the monitoring scheduler.
static SAFE_MODE: AtomicBool = AtomicBool::new(false);

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
///    `.running` marker at claim_crash_trail, signalled via
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

    // In debug builds, Cargo watch kills and restarts the process routinely.
    // These are not real crashes — cap at Warning so safe mode never activates
    // in dev and the crash-loop-critical frontend banner never fires.
    #[cfg(debug_assertions)]
    let status = if let CrashLoopStatus::Critical(n) = status {
        if n > 0 {
            info!(
                target: "4da::watchdog",
                crashes = n,
                "Dev-mode: downgrading Critical to Warning (dev restarts are not real crashes)"
            );
            CrashLoopStatus::Warning(n)
        } else {
            status
        }
    } else {
        status
    };

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

/// Clear the crash history in `dir`. Called by the crash-trail owner's clean
/// shutdown (`clean_shutdown_in`): a clean exit means any prior burst is over.
fn clear_crash_history_in(dir: &Path) {
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
    fn phase0_budget_is_reasonable() {
        // Phase 0 budget has to be larger than a typical cold-start window
        // paint (<500ms on release builds) but tight enough that a real
        // regression gets caught. The budget is profile-split (see the const
        // above): 5s in release stays tight; debug gets 45s of headroom for the
        // Vite dev-server bootstrap and the webview poll watchdog on a slow
        // cold boot. The assertion must mirror that split, or `cargo test`
        // (a debug build, where the const is 45) fails its own ceiling.
        assert!(PHASE0_BUDGET_SECS >= 2);
        #[cfg(not(debug_assertions))]
        assert!(
            PHASE0_BUDGET_SECS <= 15,
            "release Phase-0 budget must stay tight"
        );
        #[cfg(debug_assertions)]
        assert!(
            PHASE0_BUDGET_SECS <= 60,
            "debug Phase-0 budget allows a slow dev-server cold boot"
        );
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

    #[test]
    fn phase1_time_is_attributed_to_each_segment() {
        let t0 = Instant::now();
        let pre = t0 + Duration::from_millis(5_600);
        let setup = pre + Duration::from_millis(9_100);
        let now = setup + Duration::from_millis(2_200);
        let segs = phase1_segments(t0, Some(pre), Some(setup), now);
        let names: Vec<_> = segs.iter().map(|s| s.name).collect();
        assert_eq!(names, ["pre_tauri", "tauri_build", "setup_app"]);
        assert_eq!(segs[0].elapsed, Duration::from_millis(5_600));
        assert_eq!(segs[1].elapsed, Duration::from_millis(9_100));
        assert_eq!(segs[2].elapsed, Duration::from_millis(2_200));
        // Segments sum to the whole: nothing is hidden or double-counted.
        let sum: Duration = segs.iter().map(|s| s.elapsed).sum();
        assert_eq!(sum, now - t0);
    }

    #[test]
    fn a_slow_segment_is_flagged_even_when_others_are_fast() {
        let t0 = Instant::now();
        let pre = t0 + Duration::from_millis(100);
        let setup = pre + Duration::from_millis(100);
        let now = setup + SETUP_APP_BUDGET + Duration::from_millis(1);
        let segs = phase1_segments(t0, Some(pre), Some(setup), now);
        let over: Vec<_> = segs
            .iter()
            .filter(|s| s.over_budget())
            .map(|s| s.name)
            .collect();
        assert_eq!(over, ["setup_app"]);
    }

    #[test]
    fn missing_boundaries_report_no_invented_segments() {
        let t0 = Instant::now();
        assert!(phase1_segments(t0, None, None, t0).is_empty());
        assert!(phase1_segments(t0, Some(t0), None, t0).is_empty());
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
        // Debug builds downgrade Critical → Warning (dev restarts aren't real crashes).
        #[cfg(debug_assertions)]
        {
            assert!(
                matches!(status, CrashLoopStatus::Warning(n) if n >= 3),
                "three crashes within 60s should be Warning in debug (downgraded from Critical), got {status:?}"
            );
            assert!(
                !SAFE_MODE.load(Ordering::SeqCst),
                "Debug builds must never enter safe mode"
            );
        }
        #[cfg(not(debug_assertions))]
        {
            assert!(
                matches!(status, CrashLoopStatus::Critical(n) if n >= 3),
                "three crashes within 60s should be Critical, got {status:?}"
            );
            assert!(
                SAFE_MODE.load(Ordering::SeqCst),
                "Critical must set safe-mode flag"
            );
        }

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

    #[test]
    fn claiming_a_fresh_trail_reports_a_clean_previous_session() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        assert_eq!(claim_crash_trail_in(tmp.path()), (false, false));
        assert!(
            tmp.path().join(".running").exists(),
            "the owner marks its own session as running"
        );
    }

    #[test]
    fn claiming_after_a_crash_reports_it_and_consumes_the_stall_marker() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        std::fs::write(tmp.path().join(".running"), "1").unwrap();
        std::fs::write(tmp.path().join(".stalled"), "phase0").unwrap();
        assert_eq!(claim_crash_trail_in(tmp.path()), (true, true));
        assert!(!tmp.path().join(".stalled").exists());
        let rotated = std::fs::read_to_string(tmp.path().join(".running")).unwrap();
        assert_ne!(rotated, "1", "the marker is rewritten for this session");
    }

    /// The headless engine and the console-control handler call
    /// `mark_clean_shutdown` too. Without the trail they must leave a live
    /// GUI's markers and crash history alone (2026-11-01: the engine's exit
    /// deleted them every 10 minutes).
    #[test]
    fn a_clean_exit_without_the_trail_leaves_the_owners_markers() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        for f in [".running", ".healthy", CRASH_HISTORY_FILENAME] {
            std::fs::write(tmp.path().join(f), "[1]").unwrap();
        }
        clean_shutdown_in(tmp.path(), false);
        for f in [".running", ".healthy", CRASH_HISTORY_FILENAME] {
            assert!(tmp.path().join(f).exists(), "{f} must survive");
        }
    }

    #[test]
    fn a_clean_exit_with_the_trail_removes_the_markers_and_history() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        for f in [".running", ".healthy", CRASH_HISTORY_FILENAME] {
            std::fs::write(tmp.path().join(f), "[1]").unwrap();
        }
        clean_shutdown_in(tmp.path(), true);
        for f in [".running", ".healthy", CRASH_HISTORY_FILENAME] {
            assert!(!tmp.path().join(f).exists(), "{f} must be removed");
        }
    }
}
