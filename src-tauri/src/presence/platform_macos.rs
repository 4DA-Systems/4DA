// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! macOS presence detection.
//!
//! Three detectors, in the order the answers are most trustworthy:
//!
//! 1. **Display captured** — `CGDisplayIsCaptured`. True when an application has
//!    taken exclusive control of the display, which is what a true-fullscreen
//!    game does.
//! 2. **Focus mode** — the modern Do Not Disturb. Read from
//!    `~/Library/DoNotDisturb/DB/Assertions.json` rather than through an API,
//!    because Apple has never exposed a public one and every objc route to it
//!    is private. The parsing lives in [`super::focus_active_from_assertions`]
//!    so it is unit-tested on every platform, not just this one.
//! 3. **Idle** — `CGEventSourceSecondsSinceLastEventType`, the macOS analogue
//!    of `GetLastInputInfo`. Catches the empty chair.
//!
//! ## Bindings, and what is verified
//!
//! The CoreGraphics functions are hand-declared rather than pulled in through
//! an objc binding crate, matching the mach-API precedent already in
//! `diagnostics.rs`. That keeps the dependency graph unchanged.
//!
//! **This file is not compiled by PR CI.** The project deliberately does not
//! spend macOS runner minutes (`validate.yml`: "the macOS 10x tier is the real
//! billing risk"), so macOS first compiles in `release.yml`. Everything here is
//! therefore kept as small and as boring as possible: three extern
//! declarations, no structs, no ownership, nothing to get wrong at runtime that
//! is not caught by the fail-open contract below.
//!
//! ## Fail-open contract
//!
//! Every path returns `None` (available) on any doubt. A detector that wrongly
//! says "busy" mutes 4DA silently and forever; one that wrongly says
//! "available" costs at most a single mistimed popup.
//!
//! ## Not covered
//!
//! Borderless-windowed fullscreen. The Windows detector catches it by comparing
//! the foreground window's rect to the monitor; the macOS equivalent needs
//! `NSWorkspace`/`NSApplication` presentation options, which is objc-only and
//! cannot be compile-checked here. `CGDisplayIsCaptured` covers exclusive
//! fullscreen; a borderless game still falls through to the idle detector once
//! the player stops typing, which is a partial but honest answer.

use std::path::PathBuf;

use super::BusyReason;

#[allow(non_camel_case_types)]
type CGDirectDisplayID = u32;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGMainDisplayID() -> CGDirectDisplayID;
    /// Returns non-zero when the display has been captured by an application.
    /// C signature returns `boolean_t`, which is `int` on Darwin.
    fn CGDisplayIsCaptured(display: CGDirectDisplayID) -> i32;
    /// Seconds since the last input event of the given type.
    /// C signature returns `CFTimeInterval`, which is `double`.
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
}

/// `kCGEventSourceStateCombinedSessionState` — input across the whole session.
const COMBINED_SESSION_STATE: i32 = 0;

/// `kCGAnyInputEventType` — any input event, not one specific kind.
const ANY_INPUT_EVENT_TYPE: u32 = u32::MAX;

/// How long with no input before the user counts as away. Matches the Windows
/// detector so the behaviour is the same on both platforms.
const AWAY_AFTER_SECS: f64 = 10.0 * 60.0;

/// Query the OS for whether this is a good moment to interrupt.
pub(super) fn detect() -> Option<BusyReason> {
    if display_is_captured() {
        return Some(BusyReason::FullscreenApp);
    }
    if focus_mode_active() {
        return Some(BusyReason::OsQuietTime);
    }
    if is_away() {
        return Some(BusyReason::Away);
    }
    None
}

/// Has an application taken exclusive control of the main display?
#[allow(unsafe_code)]
fn display_is_captured() -> bool {
    // SAFETY: both calls take plain scalars and return plain scalars. No
    // pointers, no allocation, no ownership transfer.
    unsafe { CGDisplayIsCaptured(CGMainDisplayID()) != 0 }
}

/// Seconds since the user last touched the keyboard or mouse.
///
/// `None` when the value is not usable (negative or non-finite), so a bad
/// reading can never be mistaken for a long idle.
#[allow(unsafe_code)]
fn idle_seconds() -> Option<f64> {
    // SAFETY: scalar in, scalar out; no pointers involved.
    let secs = unsafe {
        CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT_TYPE)
    };
    if secs.is_finite() && secs >= 0.0 {
        Some(secs)
    } else {
        None
    }
}

fn is_away() -> bool {
    idle_seconds().is_some_and(|secs| secs >= AWAY_AFTER_SECS)
}

/// Path to the Focus-mode assertions database.
fn assertions_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library/DoNotDisturb/DB/Assertions.json"))
}

/// Is a Focus mode currently asserted?
///
/// A missing file is the normal case when the user has never enabled Focus, so
/// it reads as "not asserted" rather than as an error.
fn focus_mode_active() -> bool {
    let Some(path) = assertions_path() else {
        return false;
    };
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return false;
    };
    super::focus_active_from_assertions(&contents)
}
