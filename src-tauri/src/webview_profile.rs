// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Wait for a previous instance's WebView2 to release the shared profile.
//!
//! Every instance of the app uses one WebView2 profile,
//! `<local data>/<identifier>/EBWebView`. The WebView2 browser process holds
//! `EBWebView/lockfile` open for as long as it runs, and it can outlive the
//! app that started it by tens of seconds. A new instance that creates its
//! window in that gap gets `0x800700AA` ("The requested resource is in use")
//! from WebView2, and Tauri panics with "Failed to setup app". The operator's
//! app died that way twice (2026-10-24 08:44, 2026-10-25 13:47), each time
//! launched within ~30 s of the previous one quitting. The updater's restart
//! after an install is the same quick relaunch.
//!
//! So before the window is created, wait (bounded) until the lockfile is gone
//! or can be opened. The single-instance lock is already held at that point,
//! so whatever holds the profile is a previous instance shutting down.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tracing::{info, warn};

/// Longest wait for the profile. A WebView2 browser process that has not
/// released it by then is not shutting down; creation then fails as before.
const RELEASE_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// How a wait for the profile ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProfileWait {
    /// Nothing held it.
    Free,
    /// A previous instance held it and released it after this long.
    Released(Duration),
    /// Still held when the wait gave up.
    StillBusy(Duration),
}

/// `<local data>/<identifier>/EBWebView/lockfile`, or `None` for an
/// identifier that could escape that folder.
pub(crate) fn profile_lockfile(local_data_dir: &Path, identifier: &str) -> Option<PathBuf> {
    let service_worker = crate::app_setup::webview_service_worker_dir(local_data_dir, identifier)?;
    // .../EBWebView/Default/Service Worker -> .../EBWebView
    let profile = service_worker.parent()?.parent()?;
    Some(profile.join("lockfile"))
}

/// Whether a running WebView2 browser process holds the profile. It keeps the
/// lockfile open without write sharing, so opening it for writing fails while
/// it runs. An absent lockfile, or one left behind that opens, is free.
pub(crate) fn is_profile_busy(lockfile: &Path) -> bool {
    lockfile.exists()
        && std::fs::OpenOptions::new()
            .write(true)
            .open(lockfile)
            .is_err()
}

/// Poll `busy` until it reports free or `timeout` passes.
fn wait_until_released(
    timeout: Duration,
    poll: Duration,
    mut busy: impl FnMut() -> bool,
) -> ProfileWait {
    let started = Instant::now();
    if !busy() {
        return ProfileWait::Free;
    }
    loop {
        std::thread::sleep(poll);
        if !busy() {
            return ProfileWait::Released(started.elapsed());
        }
        if started.elapsed() >= timeout {
            return ProfileWait::StillBusy(started.elapsed());
        }
    }
}

/// Block until this app's WebView2 profile is free, up to [`RELEASE_TIMEOUT`].
/// Call from the GUI entry point after the single-instance lock, before
/// `Builder::build` creates the window. Time spent waiting is credited back to
/// the startup watchdog, so it never reads as a stall.
pub(crate) fn wait_for_webview_profile(identifier: &str) -> ProfileWait {
    let Some(local_data_dir) = dirs::data_local_dir() else {
        return ProfileWait::Free;
    };
    let Some(lockfile) = profile_lockfile(&local_data_dir, identifier) else {
        return ProfileWait::Free;
    };
    let outcome = wait_until_released(RELEASE_TIMEOUT, POLL_INTERVAL, || {
        is_profile_busy(&lockfile)
    });
    match outcome {
        ProfileWait::Free => {}
        ProfileWait::Released(waited) => {
            crate::startup_watchdog::exclude_from_startup_clock(waited);
            info!(
                target: "4da::startup",
                waited_ms = waited.as_millis() as u64,
                "WebView2 profile released by the previous instance"
            );
        }
        ProfileWait::StillBusy(waited) => {
            crate::startup_watchdog::exclude_from_startup_clock(waited);
            warn!(
                target: "4da::startup",
                waited_ms = waited.as_millis() as u64,
                path = %lockfile.display(),
                "WebView2 profile still in use by another process; window creation may fail"
            );
        }
    }
    outcome
}

#[cfg(test)]
#[path = "webview_profile_tests.rs"]
mod tests;
