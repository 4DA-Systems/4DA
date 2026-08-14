// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Panic containment for detached background tasks.
//!
//! A panic inside a detached `spawn` is invisible. The `JoinHandle` is dropped,
//! so nothing ever observes the `Err(JoinError)`, and every line after the
//! panic point is skipped — including whatever releases the in-flight gate the
//! task claimed on entry. The task simply stops existing, silently.
//!
//! When the skipped line is a gate release, a panic is not a lost cycle but a
//! permanent wedge. `MonitoringState::is_checking` is claimed with
//! `swap(true, SeqCst)` *before* the `scheduled-analysis` event is emitted, and
//! every site that clears it sits *after* the work. One unwind anywhere in
//! fetch-or-score therefore latches the gate at `true` for the rest of the
//! process lifetime: every later tick sees `is_checking == true`, skips itself,
//! and background refresh is dead with no error surfaced anywhere — the feed
//! just quietly stops updating. `void_engine::heartbeat` already carries a
//! "check if monitoring is_checking stuck (simple heuristic)" probe, which is
//! this exact state observed from the outside with no recovery path.
//!
//! [`contain`] converts that class of failure from a permanent wedge into a
//! skipped cycle: the unwind is caught and logged with its payload, and the
//! caller gets `None` so it can run the cleanup the panicking path missed and
//! let the next tick retry.
//!
//! Distinct from [`crate::crash_guard`], which installs the process-wide panic
//! *hook* that zeroizes secrets before a crash dump is written. That hook runs
//! on every unwind, contained or not; this module decides whether the unwind
//! ends the task's owner or merely the task.

use futures::FutureExt;
use std::any::Any;
use std::future::Future;
use std::panic::AssertUnwindSafe;

/// Best-effort human-readable text for a caught panic payload.
///
/// `panic!("literal")` boxes a `&'static str` and `panic!("{fmt}", ..)` boxes a
/// `String`; anything else (a `panic_any` with a custom type) has no message to
/// recover, so it is reported by shape rather than dropped silently.
fn payload_text(payload: &(dyn Any + Send)) -> &str {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.as_str()
    } else {
        "<non-string panic payload>"
    }
}

/// Await `fut`, containing any panic it unwinds.
///
/// Returns `Some(output)` on normal completion, `None` if the future panicked.
///
/// The caller owns whatever cleanup the panicking path skipped — most
/// importantly releasing any in-flight gate it claimed, so the next cycle is
/// not locked out forever. Containment alone only stops the panic from
/// propagating; it cannot know which invariants the dead task left broken.
pub(crate) async fn contain<F>(label: &'static str, fut: F) -> Option<F::Output>
where
    F: Future,
{
    match AssertUnwindSafe(fut).catch_unwind().await {
        Ok(output) => Some(output),
        Err(payload) => {
            tracing::error!(
                target: "4da::task_guard",
                task = label,
                panic = payload_text(&*payload),
                "Background task panicked — contained; caller must release any gate it claimed"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn contain_passes_through_normal_completion() {
        let out = contain("test-ok", async { 42_u32 }).await;
        assert_eq!(out, Some(42));
    }

    #[tokio::test]
    async fn contain_catches_panic_and_returns_none() {
        // If containment were absent this `.await` would unwind through the
        // test body and fail the test outright.
        let out: Option<u32> = contain("test-panic", async { panic!("boom") }).await;
        assert_eq!(out, None);
    }

    #[tokio::test]
    async fn contain_catches_panic_after_an_await_point() {
        // The real failure shape: the panic happens deep inside an already
        // suspended future (mid-fetch, mid-score), not on the first poll.
        let out: Option<u32> = contain("test-panic-late", async {
            tokio::task::yield_now().await;
            panic!("late boom")
        })
        .await;
        assert_eq!(out, None);
    }

    #[test]
    fn payload_text_recovers_both_panic_message_shapes() {
        let stat: Box<dyn Any + Send> = Box::new("static literal");
        assert_eq!(payload_text(&*stat), "static literal");

        let owned: Box<dyn Any + Send> = Box::new(String::from("formatted 1"));
        assert_eq!(payload_text(&*owned), "formatted 1");

        let other: Box<dyn Any + Send> = Box::new(7_u8);
        assert_eq!(payload_text(&*other), "<non-string panic payload>");
    }
}
