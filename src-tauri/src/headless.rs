// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Headless engine entry point — runs 4DA's fetch+score pipeline without a GUI window.
//!
//! The pipeline (`source_fetching::fill_cache_background` + `analysis::analyze_cached_content_silent`)
//! is already Rust-native and writes everything to `data/4da.db`; it only borrows an `AppHandle` for
//! best-effort UI `emit`s. So "headless" = build a Tauri app with **zero windows** (no WebView2 ever
//! spawns), take its `AppHandle`, and drive the same pipeline. Every cycle writes an `engine_runs`
//! freshness receipt so the MCP server — and an external verifier — can tell fresh data from stale.
//!
//! Two modes:
//! - [`HeadlessMode::Once`]: one fetch+score cycle, write a receipt, exit `0` on success, `1` on a
//!   scoring failure, `2` if the app could not be built. This is the unit a Task Scheduler / cron
//!   invokes, and the `Command`-proof target an external verifier re-runs.
//! - [`HeadlessMode::Daemon`]: a self-contained interval loop (cadence from `monitoring.interval_minutes`)
//!   that runs a cycle, sleeps, repeats, until Ctrl-C. Deliberately does NOT depend on the GUI event
//!   loop or Tauri's emit/listen — the OS-scheduled `--once` path is the recommended deployment and
//!   the daemon is a self-contained convenience.

use std::time::{Duration, Instant};

use tauri::AppHandle;
use tracing::{error, info, warn};

use crate::engine_runs::RunReceipt;

/// Floor on the daemon refresh interval — a misconfigured `interval_minutes = 0` must never spin
/// the loop. Five minutes matches the tightest cadence the GUI scheduler runs.
const MIN_DAEMON_INTERVAL_MINUTES: u64 = 5;
/// Fallback cadence when settings are readable but unset.
const DEFAULT_DAEMON_INTERVAL_MINUTES: u64 = 30;

/// How the headless engine should run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadlessMode {
    /// One fetch+score cycle, then exit.
    Once,
    /// Loop on the monitoring interval until terminated.
    Daemon,
}

/// Hide the console window when the OS scheduler (or a double-click) spawned one for this headless
/// run, so a background refresh never shows an unexplained black window. Windows only.
///
/// It distinguishes the two ways a console-subsystem build acquires a console:
/// - **Scheduler / double-click launch** — a *new* console is created for us and we are its sole
///   process (`GetConsoleProcessList` returns 1) → hide it.
/// - **Manual run from a terminal** (`fourda-engine --once`) — we inherit the parent shell's console
///   (process count > 1) → leave it visible so developers and verifiers keep their logs.
///
/// It is a no-op on a windows-subsystem build (release `fourda.exe`): there is no console, so
/// `GetConsoleWindow` returns null and we return early. Hiding the window does not touch the
/// stdout/stderr handles, so an external verifier that redirects the engine's output still captures it.
#[cfg(target_os = "windows")]
#[allow(unsafe_code)] // Intentional: FFI to Win32 console/window APIs to hide the scheduler's console.
fn hide_scheduler_spawned_console() {
    use windows_sys::Win32::System::Console::{GetConsoleProcessList, GetConsoleWindow};

    // `ShowWindow` lives in the `Win32_UI_WindowsAndMessaging` feature, which this crate does not
    // enable. Declare just this one call rather than widen the shared (peer-contended) Cargo.toml;
    // `GetConsoleWindow`/`GetConsoleProcessList` are already in the enabled `Win32_System_Console`.
    #[link(name = "user32")]
    extern "system" {
        #[link_name = "ShowWindow"]
        fn show_window(hwnd: *mut core::ffi::c_void, cmd: i32) -> i32;
    }
    const SW_HIDE: i32 = 0;

    // SAFETY: three plain Win32 calls over a fixed-size stack buffer; no pointers outlive the call.
    unsafe {
        let hwnd = GetConsoleWindow();
        if hwnd.is_null() {
            return; // windows-subsystem build / no attached console — nothing to hide.
        }
        // A 2-slot buffer is enough to tell "exactly one owner" from "more than one"; we never read
        // the pids, only the returned count. A 0 return means the query failed — leave the window.
        let mut owners = [0u32; 2];
        if GetConsoleProcessList(owners.as_mut_ptr(), owners.len() as u32) == 1 {
            show_window(hwnd, SW_HIDE);
        }
    }
}

/// Entry point for the `fourda-engine` binary. Initializes 4DA's global services, builds a
/// windowless Tauri app for its `AppHandle`, and runs the pipeline in the requested mode.
/// Never returns — always terminates the process with an explicit exit code.
///
/// `force` bypasses the freshness gate. Without it, a cycle is skipped when the data is already
/// fresh (last fetch within the refresh interval) — so running alongside a GUI that is actively
/// refreshing does not double-fetch against rate-limited sources.
pub fn run_headless(mode: HeadlessMode, force: bool) -> ! {
    // A scheduled background refresh must never flash a console window at the user. An unexplained
    // black window that pops up every 30 minutes reads as malware — the founder's first instinct on
    // seeing it was to kill it immediately. Release `fourda.exe` is already windowless
    // (`windows_subsystem = "windows"`); this additionally hides the stray console a *debug* build, or
    // the console-subsystem `fourda-engine` binary, is handed when the OS scheduler launches it. Done
    // first so the window is gone before any slower init can let it paint.
    #[cfg(target_os = "windows")]
    hide_scheduler_spawned_console();

    // Globals: runtime paths, logging, relevance threshold, database, context engine, source
    // registry. Identical to the GUI's pre-Tauri init, but WITHOUT the single-instance lock — the
    // headless engine deliberately coexists with a running GUI over the same WAL database.
    crate::app_setup::initialize_pre_tauri(false);
    info!(target: "4da::headless", ?mode, force, "Headless engine starting (no window, no WebView2)");

    // Build a Tauri app with the configured windows stripped: we need a real AppHandle for the
    // pipeline's best-effort emits, but with no window WebView2 never instantiates.
    let mut context = crate::app_context();
    context.config_mut().app.windows.clear();

    let app = match tauri::Builder::default().build(context) {
        Ok(app) => app,
        Err(e) => {
            error!(target: "4da::headless", error = %e, "Failed to build headless Tauri app");
            std::process::exit(2);
        }
    };
    let handle = app.handle().clone();

    let code = match mode {
        HeadlessMode::Once => {
            if !force && is_cycle_fresh() {
                info!(
                    target: "4da::headless",
                    "Feed and dependency intelligence already fresh — nothing to do. \
                     Pass --force to refresh anyway."
                );
                0
            } else {
                tauri::async_runtime::block_on(run_one_cycle(&handle, "headless_once", force))
            }
        }
        HeadlessMode::Daemon => {
            tauri::async_runtime::block_on(run_daemon_loop(&handle, force));
            0
        }
    };

    // Hold `app` until all work is done, then exit explicitly — there is no event loop to spin.
    drop(app);
    std::process::exit(code);
}

/// Run a single fetch+score+dependency-audit cycle and record a freshness receipt. Returns the
/// process exit code: `0` success, `1` if scoring or dependency refresh failed. A fetch failure is
/// non-fatal (we still score the existing cache) but is reflected in the receipt counts.
async fn run_one_cycle(handle: &AppHandle, trigger: &'static str, force_osv: bool) -> i32 {
    let started = Instant::now();
    let mut receipt = RunReceipt::begin(trigger);
    // Attribution token: a verifier (e.g. Verax) injects FOURDA_ENGINE_NONCE when it invokes the
    // engine for a specific task; the engine stamps it into the receipt so an attribution proof can
    // require the receipt's nonce to match the task's — defeating a free-ride on a concurrent refresh
    // (whose receipt carries no such nonce). Unset for normal/scheduled/daemon runs.
    receipt.nonce = std::env::var("FOURDA_ENGINE_NONCE")
        .ok()
        .filter(|s| !s.is_empty());

    // Step 0 — ensure context exists. On a fresh data dir (FOURDA_DATA_DIR pointing at a new
    // location, or a first headless-only install) `context_chunks` is empty because context
    // indexing normally runs from the GUI's onboarding/auto-discovery flow. Scoring against an
    // empty context marks every item irrelevant, so a headless-first deployment silently produces
    // zero intelligence forever. If directories are configured but nothing is indexed, index now.
    // Never clears or re-indexes existing context — strictly a cold-start self-heal.
    ensure_context_indexed().await;

    // Step 1 — fetch (fills the cache; writes/touches source_items, stamps sources.last_fetch).
    info!(target: "4da::headless", "Cycle step 1/3: fetching sources...");
    match crate::source_fetching::fill_cache_background(handle).await {
        Ok(summary) => {
            receipt.sources_succeeded = summary.succeeded;
            receipt.sources_failed = summary.failed;
            receipt.sources_skipped = summary.skipped_disabled;
            receipt.new_items = summary.new_items;
            receipt.cached_touches = summary.cached_touches;
            info!(
                target: "4da::headless",
                succeeded = summary.succeeded,
                failed = summary.failed,
                new_items = summary.new_items,
                "Fetch complete"
            );
        }
        Err(e) => {
            warn!(target: "4da::headless", error = %e, "Fetch failed — scoring existing cache anyway");
        }
    }

    // Step 2 — score (embeds + PASIFA; writes relevance_score). Silent variant: no UI progress events.
    info!(target: "4da::headless", "Cycle step 2/3: scoring cached content...");
    match crate::analysis::analyze_cached_content_silent(handle).await {
        Ok(results) => {
            receipt.items_scored = results.len();
            receipt.relevant_count = results.iter().filter(|r| r.relevant).count();

            // Persist scores. The GUI path persists via the analysis_status completion
            // handler, and the backfill scheduler stamps the rest — neither runs headless,
            // so without this block a headless-only deployment computes scores every cycle
            // and writes none of them (relevance_score stays NULL for every item; verified
            // live on a fresh FOURDA_DATA_DIR). Mirrors analysis_status.rs exactly:
            // scores for top_score > 0, a pipeline-version stamp for every scored item.
            if let Ok(db) = crate::get_database() {
                let score_data: Vec<(i64, f32, Option<String>, Option<String>)> = results
                    .iter()
                    .filter(|r| r.top_score > 0.0)
                    .map(|r| {
                        (
                            r.id as i64,
                            r.top_score,
                            r.signal_type.clone(),
                            r.signal_priority.clone(),
                        )
                    })
                    .collect();
                if !score_data.is_empty() {
                    if let Err(e) = db.persist_analysis_scores(&score_data) {
                        warn!(target: "4da::headless", error = %e, "Failed to persist relevance scores");
                    }
                }
                let scored_ids: Vec<i64> = results.iter().map(|r| r.id as i64).collect();
                if let Err(e) =
                    db.mark_items_scored_version(&scored_ids, crate::scoring::PIPELINE_VERSION)
                {
                    warn!(target: "4da::headless", error = %e, "Failed to stamp scored pipeline version");
                }
            }

            info!(
                target: "4da::headless",
                scored = receipt.items_scored,
                relevant = receipt.relevant_count,
                "Scoring complete"
            );
        }
        Err(e) => {
            error!(target: "4da::headless", error = %e, "Scoring failed");
            receipt.ok = false;
            append_receipt_error(&mut receipt, e.to_string());
        }
    }

    // Step 3 — dependency audit. This has its own six-hour freshness gate:
    // source-feed freshness must never suppress a stale security mirror.
    info!(target: "4da::headless", "Cycle step 3/3: refreshing dependency intelligence...");
    match crate::get_database() {
        Ok(db) => {
            let needs_sync = force_osv
                || crate::osv::sync::needs_sync(&db, crate::osv::sync::osv_sync_max_age_hours())
                    .unwrap_or(true);
            if needs_sync {
                match crate::osv::sync::sync(&db).await {
                    Ok(result) => {
                        if result.errors.is_empty() {
                            info!(
                                target: "4da::headless",
                                ecosystems = ?result.ecosystems_synced,
                                stored = result.advisories_stored,
                                matched = result.advisories_matched,
                                "Dependency intelligence refresh complete"
                            );
                        } else {
                            warn!(
                                target: "4da::headless",
                                errors = ?result.errors,
                                "Dependency intelligence refresh completed with errors"
                            );
                            append_receipt_error(
                                &mut receipt,
                                format!("OSV sync partial failure: {}", result.errors.join("; ")),
                            );
                            if result.ecosystems_synced.is_empty() {
                                receipt.ok = false;
                            }
                        }
                    }
                    Err(e) => {
                        error!(target: "4da::headless", error = %e, "Dependency intelligence refresh failed");
                        receipt.ok = false;
                        append_receipt_error(&mut receipt, format!("OSV sync failed: {e}"));
                    }
                }
            } else {
                info!(target: "4da::headless", "Dependency intelligence already fresh — skipping OSV sync");
            }
        }
        Err(e) => {
            error!(target: "4da::headless", error = %e, "Dependency intelligence refresh could not access database");
            receipt.ok = false;
            append_receipt_error(&mut receipt, format!("OSV sync database unavailable: {e}"));
        }
    }

    receipt.duration_ms = started.elapsed().as_millis() as u64;
    let exit_code = i32::from(!receipt.ok);
    crate::engine_runs::record(receipt);

    // Log the resulting ground-truth freshness so a tail of the run shows the real DB state — this
    // is what an external verifier asserts against (count moved / watermark advanced / fingerprint changed).
    if let Ok(snap) = crate::engine_runs::freshness_snapshot() {
        info!(
            target: "4da::headless",
            source_items = snap.source_items_total,
            watermark = ?snap.max_item_created_at,
            last_fetch = ?snap.last_source_fetch,
            fingerprint = %snap.content_fingerprint,
            "Post-cycle freshness snapshot"
        );
    }
    exit_code
}

/// Cold-start self-heal: index configured context directories when nothing is indexed yet.
/// No-op when context already exists (the common case — one cheap COUNT) or when no
/// `context_dirs` are configured (nothing to index; the GUI onboarding owns that setup).
/// Failures are logged and non-fatal: a cycle with empty context still fetches and writes
/// honest receipts — it just scores nothing relevant, which the log now explains.
async fn ensure_context_indexed() {
    let chunk_count = match crate::get_database().and_then(|db| {
        db.context_count()
            .map_err(|e| format!("context_count failed: {e}").into())
    }) {
        Ok(count) => count,
        Err(e) => {
            warn!(target: "4da::headless", error = %e, "Context check failed — skipping context indexing");
            return;
        }
    };
    if chunk_count > 0 {
        return;
    }
    let dirs = crate::get_context_dirs();
    if dirs.is_empty() {
        info!(
            target: "4da::headless",
            "No context indexed and no context_dirs configured — scoring will have no project context"
        );
        return;
    }
    info!(
        target: "4da::headless",
        dirs = dirs.len(),
        "Cycle step 0/3: no context indexed yet — indexing configured context directories..."
    );
    match crate::context_commands::index_context().await {
        Ok(summary) => info!(target: "4da::headless", %summary, "Context indexing complete"),
        Err(e) => {
            warn!(target: "4da::headless", error = %e, "Context indexing failed — cycle continues without context");
        }
    }

    // Same cold-start condition, second profile: the dependency axis (and OSV matching) read the
    // ACE-detected project/dependency tables, which the GUI's onboarding scan normally populates.
    // Run the full ACE scan over the same configured directories so a headless-first deployment
    // gets a real dependency profile too. Non-fatal on failure for the same reason as above.
    let dir_strings: Vec<String> = dirs
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    match crate::ace_commands::ace_full_scan(dir_strings).await {
        Ok(_) => info!(target: "4da::headless", "ACE dependency scan complete"),
        Err(e) => {
            warn!(target: "4da::headless", error = %e, "ACE dependency scan failed — cycle continues without dependency profile");
        }
    }
}

fn append_receipt_error(receipt: &mut RunReceipt, error: String) {
    receipt.error = Some(match receipt.error.take() {
        Some(existing) => format!("{existing}; {error}"),
        None => error,
    });
}

/// Self-contained daemon loop: run a cycle, sleep for the monitoring interval, repeat until Ctrl-C.
/// Cadence is read fresh each iteration so a settings change takes effect on the next sleep.
async fn run_daemon_loop(handle: &AppHandle, force: bool) {
    info!(target: "4da::headless", "Daemon mode — entering refresh loop (Ctrl-C to stop)");
    loop {
        if force || !is_cycle_fresh() {
            run_one_cycle(handle, "headless_daemon", force).await;
        } else {
            info!(target: "4da::headless", "Feed and dependency intelligence already fresh — skipping this tick");
        }

        let interval = daemon_interval();
        info!(
            target: "4da::headless",
            next_in_secs = interval.as_secs(),
            "Cycle done — sleeping until next refresh"
        );
        tokio::select! {
            () = tokio::time::sleep(interval) => {}
            r = tokio::signal::ctrl_c() => {
                match r {
                    Ok(()) => info!(target: "4da::headless", "Ctrl-C received — stopping daemon"),
                    Err(e) => warn!(target: "4da::headless", error = %e, "Signal listener error — stopping daemon"),
                }
                break;
            }
        }
    }
}

/// The refresh interval from `monitoring.interval_minutes`, clamped to a sane floor so a
/// misconfigured value can't tighten the loop below [`MIN_DAEMON_INTERVAL_MINUTES`].
///
/// `FOURDA_ENGINE_INTERVAL_SECS` overrides this with an explicit seconds cadence (clamped to a 10s
/// floor) — for tests that need to observe several cycles quickly, and for power users who want a
/// tighter loop than the settings UI exposes.
fn daemon_interval() -> Duration {
    if let Ok(raw) = std::env::var("FOURDA_ENGINE_INTERVAL_SECS") {
        if let Ok(secs) = raw.trim().parse::<u64>() {
            return Duration::from_secs(secs.max(10));
        }
    }
    let minutes = {
        let settings = crate::get_settings_manager().lock();
        settings.get_monitoring_config().interval_minutes
    };
    let minutes = if minutes == 0 {
        DEFAULT_DAEMON_INTERVAL_MINUTES
    } else {
        minutes
    }
    .max(MIN_DAEMON_INTERVAL_MINUTES);
    Duration::from_secs(minutes * 60)
}

/// Whether the database is already fresh: the most recent `sources.last_fetch` is within the
/// refresh interval. Returns `false` (i.e. "refresh needed") if nothing has ever fetched or the
/// snapshot/timestamp can't be read — failing toward refreshing rather than silently skipping.
/// `sources.last_fetch` is stamped `datetime('now')` (UTC, `YYYY-MM-DD HH:MM:SS`).
fn is_data_fresh() -> bool {
    let interval_minutes = (daemon_interval().as_secs() / 60).max(1);
    let snap = match crate::engine_runs::freshness_snapshot() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let Some(last) = snap.last_source_fetch else {
        return false; // never fetched → not fresh
    };
    let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&last, "%Y-%m-%d %H:%M:%S") else {
        return false; // unparseable → treat as stale and refresh
    };
    let age_minutes = chrono::Utc::now()
        .signed_duration_since(naive.and_utc())
        .num_minutes();
    age_minutes >= 0 && (age_minutes as u64) < interval_minutes
}

fn is_osv_fresh() -> bool {
    let db = match crate::get_database() {
        Ok(db) => db,
        Err(_) => return false,
    };
    crate::osv::sync::needs_sync(&db, crate::osv::sync::osv_sync_max_age_hours())
        .map(|needs_sync| !needs_sync)
        .unwrap_or(false)
}

fn is_cycle_fresh() -> bool {
    is_data_fresh() && is_osv_fresh()
}
