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

/// Entry point for the `fourda-engine` binary. Initializes 4DA's global services, builds a
/// windowless Tauri app for its `AppHandle`, and runs the pipeline in the requested mode.
/// Never returns — always terminates the process with an explicit exit code.
///
/// `force` bypasses the freshness gate. Without it, a cycle is skipped when the data is already
/// fresh (last fetch within the refresh interval) — so running alongside a GUI that is actively
/// refreshing does not double-fetch against rate-limited sources.
pub fn run_headless(mode: HeadlessMode, force: bool) -> ! {
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
            if !force && is_data_fresh() {
                info!(
                    target: "4da::headless",
                    "Data already fresh (last fetch within the refresh interval) — nothing to do. \
                     Pass --force to refresh anyway."
                );
                0
            } else {
                tauri::async_runtime::block_on(run_one_cycle(&handle, "headless_once"))
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

/// Run a single fetch+score cycle and record a freshness receipt. Returns the process exit code:
/// `0` success, `1` if scoring failed. A fetch failure is non-fatal (we still score the existing
/// cache) but is reflected in the receipt counts.
async fn run_one_cycle(handle: &AppHandle, trigger: &'static str) -> i32 {
    let started = Instant::now();
    let mut receipt = RunReceipt::begin(trigger);

    // Step 1 — fetch (fills the cache; writes/touches source_items, stamps sources.last_fetch).
    info!(target: "4da::headless", "Cycle step 1/2: fetching sources...");
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
    info!(target: "4da::headless", "Cycle step 2/2: scoring cached content...");
    match crate::analysis::analyze_cached_content_silent(handle).await {
        Ok(results) => {
            receipt.items_scored = results.len();
            receipt.relevant_count = results.iter().filter(|r| r.relevant).count();
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
            receipt.error = Some(e.to_string());
        }
    }

    receipt.duration_ms = started.elapsed().as_millis() as u64;
    let exit_code = if receipt.ok { 0 } else { 1 };
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

/// Self-contained daemon loop: run a cycle, sleep for the monitoring interval, repeat until Ctrl-C.
/// Cadence is read fresh each iteration so a settings change takes effect on the next sleep.
async fn run_daemon_loop(handle: &AppHandle, force: bool) {
    info!(target: "4da::headless", "Daemon mode — entering refresh loop (Ctrl-C to stop)");
    loop {
        if force || !is_data_fresh() {
            run_one_cycle(handle, "headless_daemon").await;
        } else {
            info!(target: "4da::headless", "Data already fresh — skipping this tick");
        }

        let interval = daemon_interval();
        info!(
            target: "4da::headless",
            next_in_secs = interval.as_secs(),
            "Cycle done — sleeping until next refresh"
        );
        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
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
fn daemon_interval() -> Duration {
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
    let interval_minutes = daemon_interval().as_secs() / 60;
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
