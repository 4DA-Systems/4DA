// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Readiness of the pre-warmed toast and brief windows.
//!
//! Each page emits a ready event (`notification-ready`, `briefing-ready`) once
//! its listeners are registered. If that event is lost (it was emitted before
//! Rust listened: measured 2026-10-18, 8 of 16 cold starts for the toast), the
//! first show destroyed and recreated the window and sent the toast into a
//! page that was still loading. When the page has demonstrably finished
//! loading, the show now asks it again (the page answers a ping event with its
//! ready event) and only recreates the window if that gets no answer either.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Runtime};
use tracing::{info, warn};

/// How long a show waits for the page to answer a re-request.
const PING_WAIT: Duration = Duration::from_millis(1500);
const PING_POLL: Duration = Duration::from_millis(50);

/// What a show must do about the page's readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Recovery {
    /// The page reported ready: show at once.
    Ready,
    /// The page finished loading but its ready event never arrived: ask again.
    Ping,
    /// The page never loaded (or the window is gone): recreate the window.
    Recreate,
}

pub(crate) fn recovery(ready: bool, page_loaded: bool, window_exists: bool) -> Recovery {
    match (ready, page_loaded && window_exists) {
        (true, _) => Recovery::Ready,
        (false, true) => Recovery::Ping,
        (false, false) => Recovery::Recreate,
    }
}

/// Ask the page in `label` to report ready again and wait up to [`PING_WAIT`]
/// for `ready` to flip. Returns whether it did.
pub(crate) async fn ping_and_wait<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    ping_event: &str,
    ready: &AtomicBool,
) -> bool {
    if let Err(e) = app.emit_to(label, ping_event, ()) {
        warn!(target: "4da::notify", label, error = %e, "Ready re-request could not be sent");
        return false;
    }
    let mut waited = Duration::ZERO;
    while waited < PING_WAIT {
        if ready.load(Ordering::Relaxed) {
            info!(target: "4da::notify", label, waited_ms = waited.as_millis() as u64, "Window ready re-requested and confirmed");
            return true;
        }
        tokio::time::sleep(PING_POLL).await;
        waited += PING_POLL;
    }
    warn!(target: "4da::notify", label, "Window ready re-requested without an answer; recreating it");
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ready_page_shows_at_once() {
        assert_eq!(recovery(true, false, true), Recovery::Ready);
        assert_eq!(recovery(true, true, true), Recovery::Ready);
    }

    #[test]
    fn a_loaded_page_whose_ready_was_lost_is_asked_again() {
        assert_eq!(recovery(false, true, true), Recovery::Ping);
    }

    #[test]
    fn a_page_that_never_loaded_or_a_missing_window_is_recreated() {
        assert_eq!(recovery(false, false, true), Recovery::Recreate);
        assert_eq!(recovery(false, true, false), Recovery::Recreate);
    }
}
