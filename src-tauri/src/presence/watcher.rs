// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Watches for the user becoming available again, and releases what the gate
//! held back.
//!
//! ## Why polling
//!
//! Windows exposes no event for "the user stopped being busy" —
//! `SHQueryUserNotificationState` is a query, and there is no matching
//! notification. So we poll. The call is a cheap shell32 round trip with no
//! allocation, so a 15-second cadence costs nothing measurable.
//!
//! ## Why this loop is stateless
//!
//! An earlier shape of this watcher tracked a `was_busy` flag and flushed on
//! the busy -> available *transition*. That has a real failure mode: if a
//! briefing is held at 08:00 and the user quits their game before the watcher's
//! first poll, the transition is never observed and the brief is stranded until
//! some unrelated future busy period happens to end.
//!
//! Testing the *conditions* instead of the *transition* removes that class of
//! bug entirely: if something is held and the user is available, release it.
//! There is no state to get out of sync.

use std::time::Duration;

use tauri::{AppHandle, Runtime};
use tracing::info;

use super::queue;

/// How often to ask whether the user is available again.
const POLL_INTERVAL: Duration = Duration::from_secs(15);

/// How long the user must stay available before held surfaces are released.
///
/// Without this, quitting a game would be answered by a popup appearing the
/// same second the desktop is drawn — which reads as 4DA lying in wait rather
/// than being considerate. Deliberately longer than the poll interval.
const SETTLE_PERIOD: Duration = Duration::from_secs(20);

/// Start the resume watcher. Called once during app setup.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        info!(
            target: "4da::presence",
            poll_secs = POLL_INTERVAL.as_secs(),
            settle_secs = SETTLE_PERIOD.as_secs(),
            "Presence resume watcher started"
        );
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            tick(&app).await;
        }
    });
}

/// One pass: release held surfaces if, and only if, the user is available and
/// stays that way for [`SETTLE_PERIOD`].
async fn tick<R: Runtime>(app: &AppHandle<R>) {
    if queue::held_count() == 0 {
        return;
    }
    if !super::is_available() {
        return;
    }

    // Available now — but wait out the settle period and confirm, so we do not
    // fire into the split second between one fullscreen app closing and the
    // next one opening (alt-tabbing between two games, a match loading screen).
    tokio::time::sleep(SETTLE_PERIOD).await;

    if !super::is_available() {
        info!(
            target: "4da::presence",
            "User became busy again during settle period — keeping surfaces held"
        );
        return;
    }

    queue::flush(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settle_period_exceeds_poll_interval() {
        // If the settle period were shorter than the poll cadence it would not
        // actually damp anything — the next poll would already have fired.
        assert!(
            SETTLE_PERIOD > POLL_INTERVAL,
            "settle must outlast one poll to be meaningful"
        );
    }

    #[test]
    fn worst_case_release_latency_stays_under_a_minute() {
        // The user should never wonder whether 4DA forgot about them.
        let worst_case = POLL_INTERVAL + SETTLE_PERIOD;
        assert!(
            worst_case <= Duration::from_secs(60),
            "worst-case latency was {worst_case:?}"
        );
    }
}
