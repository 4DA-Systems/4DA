// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Tests for app setup — scheduled-analysis panic containment.
//!
//! Regression cover for the scheduler wedge: a panic in the scheduled path used
//! to abort the detached task before any `is_checking` clear site could run,
//! latching the gate at `true` and silently disabling background refresh for the
//! rest of the process lifetime.

use super::run_scheduled_cycle_contained;
use crate::monitoring::MonitoringState;
use std::sync::atomic::Ordering;

/// A local `MonitoringState` rather than `get_monitoring_state()`: the gate is a
/// process-wide global, and asserting on it from a test that runs concurrently
/// with the rest of the suite would be reading shared mutable state. The
/// production function takes `&MonitoringState` precisely so this is hermetic.
fn claimed_gate() -> MonitoringState {
    let state = MonitoringState::new();
    // Exactly what monitoring.rs does before emitting `scheduled-analysis`.
    let was_already_checking = state.is_checking.swap(true, Ordering::SeqCst);
    assert!(
        !was_already_checking,
        "fresh MonitoringState must start with the gate clear"
    );
    state
}

/// THE regression test. A panicking cycle must leave the gate clear so the next
/// scheduled tick can run.
///
/// Against the unfixed code — `spawn(async { run_scheduled_analysis(handle).await })`
/// with no `catch_unwind` — the unwind propagates out of the awaited cycle and
/// fails this test instead of reaching the assertion.
#[tokio::test]
async fn panicking_scheduled_cycle_releases_the_in_flight_gate() {
    let state = claimed_gate();

    let completed_normally = run_scheduled_cycle_contained(&state, async {
        // Panic after an await point: the real shape is a byte-slice or index
        // panic deep inside fetch/score, not on the first poll.
        tokio::task::yield_now().await;
        panic!("simulated panic inside the scheduled analysis path");
    })
    .await;

    assert!(
        !completed_normally,
        "a panicking cycle must be reported as not-completed"
    );
    assert!(
        !state.is_checking.load(Ordering::SeqCst),
        "is_checking must be cleared after a panic — otherwise every subsequent \
         scheduled tick is silently skipped for the process lifetime"
    );
}

/// The gate must be released even when the cycle panics before its first await,
/// i.e. the future is dropped without ever having been suspended.
#[tokio::test]
async fn cycle_panicking_before_first_await_releases_the_gate() {
    let state = claimed_gate();

    let completed_normally =
        run_scheduled_cycle_contained(&state, async { panic!("immediate panic") }).await;

    assert!(!completed_normally);
    assert!(!state.is_checking.load(Ordering::SeqCst));
}

/// The complement, and the reason the recovery clear is not unconditional: on a
/// normal completion the wrapper must not touch the gate at all. The real cycle
/// releases it through `complete_scheduled_check`; an unconditional clear here
/// could stomp a gate a later tick has already claimed, allowing two scheduled
/// analyses to run at once.
#[tokio::test]
async fn normal_completion_leaves_the_gate_exactly_as_the_cycle_left_it() {
    let state = claimed_gate();

    // Cycle completes without clearing (stands in for a tick that has already
    // re-claimed the gate for the next cycle).
    let completed_normally = run_scheduled_cycle_contained(&state, async {
        tokio::task::yield_now().await;
    })
    .await;

    assert!(completed_normally, "a clean cycle must report completion");
    assert!(
        state.is_checking.load(Ordering::SeqCst),
        "the wrapper must not clear the gate on the success path"
    );
}

/// And the success path must pass a cycle's own release through untouched.
#[tokio::test]
async fn normal_completion_preserves_the_cycles_own_gate_release() {
    let state = claimed_gate();

    let completed_normally = run_scheduled_cycle_contained(&state, async {
        // Stands in for complete_scheduled_check / the scoring-error arm.
        state.is_checking.store(false, Ordering::SeqCst);
    })
    .await;

    assert!(completed_normally);
    assert!(!state.is_checking.load(Ordering::SeqCst));
}
