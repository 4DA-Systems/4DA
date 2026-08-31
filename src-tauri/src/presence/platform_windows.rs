// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Windows presence detection.
//!
//! Two independent detectors, because neither is sufficient alone:
//!
//! 1. `SHQueryUserNotificationState` — the API Windows has shipped since Vista
//!    for exactly this question ("may I show a notification right now?"). It
//!    catches exclusive-fullscreen D3D games, presentation mode, Focus Assist
//!    quiet time, and the locked/screensaver state.
//!
//! 2. Foreground-window geometry — catches what (1) misses. A **borderless
//!    windowed** game (the default for most modern titles) is, to Windows, an
//!    ordinary top-level window that happens to be the size of the monitor, so
//!    `SHQueryUserNotificationState` cheerfully returns
//!    `QUNS_ACCEPTS_NOTIFICATIONS`. The foreground window's rect is compared
//!    against its monitor's *full* rect (not the work area).
//!
//!    Geometry alone is not enough to separate a fullscreen app from a merely
//!    **maximised** one. A maximised window's rect deliberately overhangs the
//!    monitor by the frame width on every side (measured: `-8,-8 - 2568,1448`
//!    on a 2560x1440 display), so it covers the monitor rect by construction.
//!    This code previously relied on a maximised window "stopping at the
//!    taskbar" — true only while the taskbar *reserves* space. Set the taskbar
//!    to auto-hide, or move it to another monitor, and `rcWork` equals
//!    `rcMonitor`: every maximised window then read as a fullscreen game and
//!    silently held the morning brief. So detector 2 additionally requires the
//!    window to be **undecorated** — see [`maximised_decorated`].
//!
//! Detector 2 deliberately ignores windows owned by this process — 4DA's own
//! maximised main window must never read as "the user is busy".

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows_sys::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUERY_USER_NOTIFICATION_STATE, QUNS_ACCEPTS_NOTIFICATIONS,
    QUNS_APP, QUNS_BUSY, QUNS_NOT_PRESENT, QUNS_PRESENTATION_MODE, QUNS_QUIET_TIME,
    QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, IsWindowVisible,
    IsZoomed, GWL_STYLE, WS_CAPTION, WS_THICKFRAME,
};

use super::BusyReason;

/// Query the OS for whether this is a good moment to interrupt.
///
/// Returns `None` when the user appears available. Never panics: every failure
/// path degrades to "available", because wrongly muting the daily brief forever
/// is a worse failure than one mistimed popup.
pub(super) fn detect() -> Option<BusyReason> {
    // If 4DA's own window is in front, the user is literally looking at us —
    // never infer "busy" from the OS in that case. `SHQueryUserNotificationState`
    // answers a question about the DESKTOP and has no notion of who is asking,
    // so a fullscreen 4DA would otherwise report QUNS_BUSY and mute 4DA's own
    // notifications for as long as the user kept it in front.
    //
    // Not reachable today (4DA has no fullscreen mode), so this is an explicit
    // invariant rather than a fix for an observed bug — a future kiosk or
    // fullscreen view would introduce it silently otherwise.
    //
    // Explicit user preferences (Do Not Disturb, quiet hours) are evaluated in
    // `presence::current` BEFORE this function and are unaffected: stated
    // intent always wins over inferred state.
    if foreground_is_own_process() {
        return None;
    }

    if let Some(reason) = query_notification_state() {
        return Some(reason);
    }
    if let Some(reason) = detect_borderless_fullscreen() {
        return Some(reason);
    }
    detect_away()
}

/// Is the foreground window owned by this process?
#[allow(unsafe_code)]
fn foreground_is_own_process() -> bool {
    // SAFETY: no arguments; returns a borrowed HWND or null. We never free it.
    let hwnd: HWND = unsafe { GetForegroundWindow() };
    !hwnd.is_null() && is_own_window(hwnd)
}

/// Detector 1: `SHQueryUserNotificationState`.
#[allow(unsafe_code)]
fn query_notification_state() -> Option<BusyReason> {
    let mut state: QUERY_USER_NOTIFICATION_STATE = 0;
    // SAFETY: `state` is a live, correctly-typed, stack-allocated out-param.
    // The call has no other side effects and does not retain the pointer.
    let hr = unsafe { SHQueryUserNotificationState(&raw mut state) };
    if hr < 0 {
        // S_OK is 0; any negative HRESULT means we learned nothing. Treat an
        // unreadable OS state as "available" rather than muting 4DA silently.
        tracing::debug!(
            target: "4da::presence",
            hresult = hr,
            "SHQueryUserNotificationState failed"
        );
        return None;
    }
    map_notification_state(state)
}

/// Pure mapping from the Win32 state constant to a 4DA busy reason.
///
/// Split out from the FFI call so the policy is unit-testable without Windows
/// in the loop.
pub(super) fn map_notification_state(state: QUERY_USER_NOTIFICATION_STATE) -> Option<BusyReason> {
    match state {
        QUNS_NOT_PRESENT => Some(BusyReason::ScreenLocked),
        QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_APP => Some(BusyReason::FullscreenApp),
        QUNS_PRESENTATION_MODE => Some(BusyReason::Presentation),
        QUNS_QUIET_TIME => Some(BusyReason::OsQuietTime),
        QUNS_ACCEPTS_NOTIFICATIONS => None,
        // Any future state we do not know: assume available rather than
        // muting 4DA forever.
        _ => None,
    }
}

/// Detector 2: is the foreground window a borderless-fullscreen app?
#[allow(unsafe_code)]
fn detect_borderless_fullscreen() -> Option<BusyReason> {
    // SAFETY: no arguments; returns a borrowed HWND or null. We never free it.
    let hwnd: HWND = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return None;
    }

    // SAFETY: `hwnd` is a live foreground handle from the OS.
    if unsafe { IsWindowVisible(hwnd) } == 0 {
        return None;
    }

    if is_own_window(hwnd) {
        return None;
    }

    // A maximised, decorated window is somebody reading email — not a game.
    // Checked before the geometry test because the geometry test cannot tell
    // the two apart (see the module docs).
    if is_maximised_decorated_window(hwnd) {
        return None;
    }

    let window = window_rect(hwnd)?;
    let monitor = monitor_rect(hwnd)?;

    if covers_monitor(&window, &monitor) {
        Some(BusyReason::FullscreenWindow)
    } else {
        None
    }
}

/// Is `hwnd` a maximised *ordinary* window rather than a fullscreen app?
///
/// The discriminator is decoration. "Borderless fullscreen" means the app has
/// dropped its caption bar and resize frame; a window that still carries them
/// and is merely zoomed is a normal application the user has maximised.
///
/// Reads the live window state, then defers to the pure [`maximised_decorated`]
/// for the policy so the rule is unit-testable without a live `HWND`.
#[allow(unsafe_code)]
fn is_maximised_decorated_window(hwnd: HWND) -> bool {
    // SAFETY: `hwnd` is a live foreground handle from the OS.
    let zoomed = unsafe { IsZoomed(hwnd) } != 0;
    // SAFETY: `hwnd` is live and `GWL_STYLE` is a documented index. Window
    // styles are 32-bit, so `GetWindowLongW` is the correct accessor even on
    // 64-bit Windows.
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    maximised_decorated(zoomed, style)
}

/// Pure decoration rule: a *zoomed* window that still has a caption or a
/// resize frame is a maximised ordinary window, not a fullscreen app.
///
/// Deliberately does NOT exclude every zoomed window: an undecorated window
/// that happens to be in the maximised state is still fullscreen from the
/// user's point of view, and must keep holding the brief.
pub(super) const fn maximised_decorated(zoomed: bool, style: u32) -> bool {
    zoomed && (style & (WS_CAPTION | WS_THICKFRAME)) != 0
}

/// True when `hwnd` belongs to this process (4DA's own windows never count as
/// "the user is busy with something else").
#[allow(unsafe_code)]
fn is_own_window(hwnd: HWND) -> bool {
    let mut pid: u32 = 0;
    // SAFETY: `hwnd` is live; `pid` is a valid out-param.
    unsafe { GetWindowThreadProcessId(hwnd, &raw mut pid) };
    // SAFETY: no arguments, no side effects.
    pid != 0 && pid == unsafe { GetCurrentProcessId() }
}

#[allow(unsafe_code)]
fn window_rect(hwnd: HWND) -> Option<RECT> {
    let mut rect = zeroed_rect();
    // SAFETY: `hwnd` is live; `rect` is a valid out-param.
    if unsafe { GetWindowRect(hwnd, &raw mut rect) } == 0 {
        return None;
    }
    Some(rect)
}

/// The *full* bounds of the monitor `hwnd` sits on — deliberately `rcMonitor`
/// and not `rcWork`, so a maximised window (which stops at the taskbar) does
/// not read as fullscreen.
#[allow(unsafe_code)]
fn monitor_rect(hwnd: HWND) -> Option<RECT> {
    // SAFETY: `hwnd` is live; DEFAULTTONEAREST always yields a valid monitor.
    let hmon = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if hmon.is_null() {
        return None;
    }

    let mut info = MONITORINFO {
        cbSize: u32::try_from(size_of::<MONITORINFO>()).ok()?,
        rcMonitor: zeroed_rect(),
        rcWork: zeroed_rect(),
        dwFlags: 0,
    };
    // SAFETY: `hmon` is valid and `info.cbSize` is set as the API requires.
    if unsafe { GetMonitorInfoW(hmon, &raw mut info) } == 0 {
        return None;
    }
    Some(info.rcMonitor)
}

const fn zeroed_rect() -> RECT {
    RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    }
}

/// How long with no keyboard or mouse input before the user counts as away.
///
/// Deliberately generous. The point is to catch a genuinely empty chair — the
/// 08:00 brief that fires while the user is still asleep and is marked
/// delivered forever — not to hold a notification because someone paused to
/// read something. Watching a video reads as away and the brief simply waits
/// until they touch the mouse, which is strictly better than firing at a
/// screen nobody is reading.
const AWAY_AFTER_SECS: u64 = 10 * 60;

/// Detector 3: has there been no input for [`AWAY_AFTER_SECS`]?
///
/// Complements `QUNS_NOT_PRESENT`, which only covers a locked screen or an
/// active screensaver — most people who walk away do neither.
#[allow(unsafe_code)]
fn detect_away() -> Option<BusyReason> {
    let mut info = LASTINPUTINFO {
        cbSize: u32::try_from(size_of::<LASTINPUTINFO>()).ok()?,
        dwTime: 0,
    };
    // SAFETY: `info` is a valid out-param with cbSize set as the API requires.
    if unsafe { GetLastInputInfo(&raw mut info) } == 0 {
        return None;
    }
    // SAFETY: no arguments, no side effects.
    let now = unsafe { GetTickCount() };

    if idle_secs_from_ticks(now, info.dwTime) >= AWAY_AFTER_SECS {
        Some(BusyReason::Away)
    } else {
        None
    }
}

/// Seconds elapsed between `last_input_ms` and `now_ms`, both `GetTickCount`
/// millisecond values.
///
/// `GetTickCount` wraps every ~49.7 days, so this uses `wrapping_sub`: a naive
/// subtraction across the rollover yields a colossal "idle" time and would mute
/// 4DA until the machine rebooted.
pub(super) fn idle_secs_from_ticks(now_ms: u32, last_input_ms: u32) -> u64 {
    u64::from(now_ms.wrapping_sub(last_input_ms)) / 1000
}

/// Tolerance (px) for the fullscreen comparison. Some titles are off by a
/// pixel or two, and some report a 1px-larger rect to defeat compositing.
const EDGE_TOLERANCE: i32 = 2;

/// Does `window` cover the whole of `monitor` (within [`EDGE_TOLERANCE`])?
///
/// Pure and platform-independent so the geometry rule is unit-testable.
pub(super) fn covers_monitor(window: &RECT, monitor: &RECT) -> bool {
    // A degenerate monitor rect would make every window "fullscreen".
    if monitor.right <= monitor.left || monitor.bottom <= monitor.top {
        return false;
    }
    window.left <= monitor.left + EDGE_TOLERANCE
        && window.top <= monitor.top + EDGE_TOLERANCE
        && window.right >= monitor.right - EDGE_TOLERANCE
        && window.bottom >= monitor.bottom - EDGE_TOLERANCE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn d3d_fullscreen_is_busy() {
        assert_eq!(
            map_notification_state(QUNS_RUNNING_D3D_FULL_SCREEN),
            Some(BusyReason::FullscreenApp)
        );
    }

    #[test]
    fn every_documented_state_maps() {
        assert_eq!(
            map_notification_state(QUNS_NOT_PRESENT),
            Some(BusyReason::ScreenLocked)
        );
        assert_eq!(
            map_notification_state(QUNS_BUSY),
            Some(BusyReason::FullscreenApp)
        );
        assert_eq!(
            map_notification_state(QUNS_PRESENTATION_MODE),
            Some(BusyReason::Presentation)
        );
        assert_eq!(
            map_notification_state(QUNS_QUIET_TIME),
            Some(BusyReason::OsQuietTime)
        );
        assert_eq!(
            map_notification_state(QUNS_APP),
            Some(BusyReason::FullscreenApp)
        );
    }

    #[test]
    fn accepts_notifications_is_available() {
        assert_eq!(map_notification_state(QUNS_ACCEPTS_NOTIFICATIONS), None);
    }

    #[test]
    fn unknown_state_degrades_to_available() {
        // A future Windows release adding QUNS_* = 99 must not mute 4DA forever.
        assert_eq!(map_notification_state(99), None);
    }

    #[test]
    fn exact_fullscreen_covers_monitor() {
        let mon = rect(0, 0, 2560, 1440);
        assert!(covers_monitor(&rect(0, 0, 2560, 1440), &mon));
    }

    #[test]
    fn borderless_overshoot_covers_monitor() {
        // Some titles report a slightly larger rect than the monitor.
        let mon = rect(0, 0, 2560, 1440);
        assert!(covers_monitor(&rect(-1, -1, 2561, 1441), &mon));
    }

    #[test]
    fn maximised_window_is_not_fullscreen() {
        // A maximised window stops above the taskbar (~48px) — the exact case
        // that must NOT read as "the user is gaming".
        let mon = rect(0, 0, 2560, 1440);
        assert!(!covers_monitor(&rect(0, 0, 2560, 1392), &mon));
    }

    // -- maximised vs fullscreen (the auto-hide taskbar false positive) ----

    /// The exact window that broke this in the field, captured live on
    /// 2026-09-01: Paint 3D maximised on a 2560x1440 display with the taskbar
    /// set to auto-hide. `GWL_STYLE` was `0x95CF_0000` and `IsZoomed` true.
    /// Geometry said "fullscreen" (rect `-8,-8 - 2568,1448` covers the monitor
    /// rect); the brief was held for an ordinary maximised window.
    #[test]
    fn maximised_paint3d_is_not_fullscreen() {
        assert!(
            maximised_decorated(true, 0x95CF_0000),
            "a maximised window with a caption and a resize frame is not a game"
        );
    }

    #[test]
    fn maximised_window_overhanging_the_monitor_still_covers_it() {
        // Guards the premise of the fix: geometry ALONE cannot reject this
        // window, which is why the decoration check has to exist.
        let mon = rect(0, 0, 2560, 1440);
        assert!(
            covers_monitor(&rect(-8, -8, 2568, 1448), &mon),
            "maximised windows overhang the monitor by the frame width"
        );
    }

    #[test]
    fn borderless_fullscreen_game_is_still_detected() {
        // WS_POPUP | WS_VISIBLE, no caption, no thick frame, not zoomed.
        assert!(!maximised_decorated(false, 0x9000_0000));
    }

    #[test]
    fn undecorated_zoomed_window_is_still_fullscreen() {
        // Some apps go fullscreen by maximising a chrome-less window. Zoomed
        // alone must not disqualify it, or the gate stops holding for them.
        assert!(!maximised_decorated(true, 0x9000_0000));
    }

    #[test]
    fn ordinary_unmaximised_window_is_not_disqualified_by_decoration() {
        // A decorated window that is NOT zoomed falls through to the geometry
        // test, which rejects it on size.
        assert!(!maximised_decorated(false, 0x95CF_0000));
    }

    #[test]
    fn caption_alone_and_frame_alone_both_disqualify() {
        assert!(maximised_decorated(true, WS_CAPTION));
        assert!(maximised_decorated(true, WS_THICKFRAME));
    }

    #[test]
    fn windowed_app_is_not_fullscreen() {
        let mon = rect(0, 0, 2560, 1440);
        assert!(!covers_monitor(&rect(100, 100, 1300, 900), &mon));
    }

    #[test]
    fn fullscreen_on_secondary_monitor_is_detected() {
        // Secondary monitor to the right: origin is non-zero.
        let mon = rect(2560, 0, 5120, 1440);
        assert!(covers_monitor(&rect(2560, 0, 5120, 1440), &mon));
    }

    // -- idle / away -------------------------------------------------------

    #[test]
    fn idle_seconds_from_plain_tick_difference() {
        assert_eq!(idle_secs_from_ticks(10_000, 4_000), 6);
        assert_eq!(idle_secs_from_ticks(1_000, 1_000), 0);
    }

    #[test]
    fn idle_seconds_survive_the_tick_rollover() {
        // GetTickCount wraps every ~49.7 days. A naive `now - last` across the
        // wrap yields ~49 days of "idle" and would mute 4DA until reboot.
        let last = u32::MAX - 100; // 100ms before the wrap
        let now = 100u32; // 100ms after it
        assert_eq!(
            idle_secs_from_ticks(now, last),
            0,
            "201ms of real idle time, not 49 days"
        );
    }

    #[test]
    fn away_threshold_is_generous_enough_not_to_fire_on_a_pause() {
        // Reading a long page or a coffee refill must not read as "away".
        assert!(
            AWAY_AFTER_SECS >= 5 * 60,
            "threshold must not fire on an ordinary pause"
        );
        assert!(
            idle_secs_from_ticks(4 * 60 * 1000, 0) < AWAY_AFTER_SECS,
            "four minutes idle is still present"
        );
        assert!(
            idle_secs_from_ticks(11 * 60 * 1000, 0) >= AWAY_AFTER_SECS,
            "eleven minutes idle is away"
        );
    }

    #[test]
    fn degenerate_monitor_rect_is_never_fullscreen() {
        let mon = rect(0, 0, 0, 0);
        assert!(!covers_monitor(&rect(0, 0, 0, 0), &mon));
    }

    /// Live probe against the real desktop. Ignored by default because the
    /// answer legitimately depends on what the machine is doing right now — it
    /// exists so the FFI can be exercised on demand:
    ///
    /// ```text
    /// cargo test --lib presence::platform::tests::live_probe -- --ignored --nocapture
    /// ```
    ///
    /// Run it once with a normal desktop (expect `None`) and once with a game
    /// or any fullscreen window in front (expect `FullscreenApp` or
    /// `FullscreenWindow`). That pair is the only real proof the detector
    /// works on this hardware.
    #[test]
    #[ignore = "queries the live desktop; run explicitly with --ignored"]
    fn live_probe() {
        let mut raw: QUERY_USER_NOTIFICATION_STATE = 0;
        #[allow(unsafe_code)]
        // SAFETY: valid stack out-param, same contract as query_notification_state.
        let hr = unsafe { SHQueryUserNotificationState(&raw mut raw) };

        println!("SHQueryUserNotificationState -> hr={hr} state={raw}");
        println!("  mapped              -> {:?}", map_notification_state(raw));
        println!("  full detect()       -> {:?}", detect());

        assert!(hr >= 0, "SHQueryUserNotificationState failed with {hr}");
        assert!(
            (1..=7).contains(&raw),
            "state {raw} outside the documented QUNS_* range 1..=7"
        );
    }
}
