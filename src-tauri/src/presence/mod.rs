// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! The interruption gate: the single place that answers "may 4DA raise a
//! window right now?".
//!
//! ## Why this exists
//!
//! 4DA raised its 560x780 always-on-top intelligence briefing over a fullscreen
//! game (observed 2026-08-31). The briefing fired on a clock and asked nobody
//! whether the moment was appropriate. Windows has answered that exact question
//! since Vista via `SHQueryUserNotificationState`; 4DA never asked.
//!
//! ## The contract
//!
//! Every **autonomous** surface — anything 4DA decides to show on its own —
//! passes through [`current`] before it appears. Every **explicit user action**
//! (tray "Show today's brief", the settings preview button, a manual trigger)
//! bypasses it entirely: the user asking for something is not an interruption,
//! and gating it would be a bug, not politeness.
//!
//! ## Held, never dropped
//!
//! A blocked surface is *deferred*, not discarded (see [`queue`]). When the
//! user becomes available again the queue flushes, coalesced, after a short
//! settle delay. This matters because the briefing marks itself as fired for
//! the day the moment it is generated — silently swallowing it would cost the
//! user that day's intelligence entirely.
//!
//! ## Deliberate policy: nothing breaks through
//!
//! There is no severity escape hatch. A critical CVE does not paint over a
//! fullscreen game, because a user mid-firefight cannot act on it — and a
//! notification that cannot be acted on is noise that teaches the user to
//! distrust every future one. It is delivered the moment they are back, which
//! is the first moment it was ever actionable.

use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{Local, Timelike};
use serde::{Deserialize, Serialize};

pub mod queue;
pub mod watcher;

#[cfg(windows)]
#[path = "platform_windows.rs"]
mod platform;

#[cfg(not(windows))]
#[path = "platform_stub.rs"]
mod platform;

// ============================================================================
// Types
// ============================================================================

/// Why 4DA is holding back a surface.
///
/// Ordered loosely by how emphatically the user has said "not now": explicit
/// user settings outrank inferred OS state, so [`Self::DoNotDisturb`] wins
/// over [`Self::FullscreenApp`] when both apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BusyReason {
    /// The user turned on Do Not Disturb.
    DoNotDisturb,
    /// Inside the user's configured quiet hours.
    QuietHours,
    /// An exclusive-fullscreen app (typically a game) owns the display.
    FullscreenApp,
    /// A borderless-windowed app covers the whole monitor (also a game,
    /// usually — this is the case the OS API alone does not catch).
    FullscreenWindow,
    /// Presentation mode / screen is being shared.
    Presentation,
    /// Windows Focus Assist (or equivalent) is suppressing notifications.
    OsQuietTime,
    /// Screen locked, screensaver running, or the user is otherwise away.
    ScreenLocked,
}

impl BusyReason {
    /// Stable machine-readable tag for logs, IPC, and tests.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DoNotDisturb => "do_not_disturb",
            Self::QuietHours => "quiet_hours",
            Self::FullscreenApp => "fullscreen_app",
            Self::FullscreenWindow => "fullscreen_window",
            Self::Presentation => "presentation",
            Self::OsQuietTime => "os_quiet_time",
            Self::ScreenLocked => "screen_locked",
        }
    }

    /// Short phrase shown to the user, e.g. "Held while you were in a game".
    ///
    /// Deliberately describes *their* state, not 4DA's internals.
    pub fn user_text(self) -> &'static str {
        match self {
            Self::DoNotDisturb => "while Do Not Disturb was on",
            Self::QuietHours => "during your quiet hours",
            Self::FullscreenApp | Self::FullscreenWindow => "while you were in a fullscreen app",
            Self::Presentation => "while you were presenting",
            Self::OsQuietTime => "while Focus Assist was on",
            Self::ScreenLocked => "while you were away",
        }
    }
}

/// Whether this is an acceptable moment to interrupt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// Go ahead.
    Available,
    /// Hold; the payload says why.
    Busy(BusyReason),
}

impl Presence {
    pub fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }

    pub fn busy_reason(self) -> Option<BusyReason> {
        match self {
            Self::Available => None,
            Self::Busy(reason) => Some(reason),
        }
    }
}

// ============================================================================
// Kill switch
// ============================================================================

/// Disables the gate entirely. Set only by the E2E harness, so automated tests
/// can drive the briefing without a real desktop underneath them.
static GATE_DISABLED: AtomicBool = AtomicBool::new(false);

/// Disable the interruption gate for this process (test harness only).
pub fn disable_gate_for_testing() {
    GATE_DISABLED.store(true, Ordering::Relaxed);
}

// ============================================================================
// Evaluation
// ============================================================================

/// The interruption gate. Evaluates, in priority order:
///
/// 1. Manual Do Not Disturb (explicit user intent, outranks everything)
/// 2. Configured quiet hours
/// 3. OS presence state — fullscreen, presentation, Focus Assist, locked
///
/// Step 3 is skipped when the user has turned off "respect fullscreen and
/// focus"; steps 1 and 2 always apply.
pub fn current() -> Presence {
    if GATE_DISABLED.load(Ordering::Relaxed) {
        return Presence::Available;
    }

    let config = InterruptionConfig::load();

    if config.dnd_active_at(Local::now()) {
        return Presence::Busy(BusyReason::DoNotDisturb);
    }

    if config.in_quiet_hours_at(Local::now()) {
        return Presence::Busy(BusyReason::QuietHours);
    }

    if config.respect_focus {
        if let Some(reason) = platform::detect() {
            return Presence::Busy(reason);
        }
    }

    Presence::Available
}

/// Convenience wrapper for call sites that only need a yes/no.
pub fn is_available() -> bool {
    current().is_available()
}

// ============================================================================
// Configuration
// ============================================================================

/// The user-facing interruption settings, resolved from `settings.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptionConfig {
    /// Honour fullscreen apps, presentations, and OS Focus Assist.
    pub respect_focus: bool,
    /// Start of quiet hours as minutes past local midnight.
    pub quiet_start: Option<u32>,
    /// End of quiet hours as minutes past local midnight.
    pub quiet_end: Option<u32>,
    /// Do Not Disturb is on until this instant. `None` means DND is off.
    pub dnd_until: Option<chrono::DateTime<Local>>,
    /// DND is on with no expiry (until the user turns it off).
    pub dnd_indefinite: bool,
}

impl InterruptionConfig {
    /// Read the current configuration from the settings manager.
    pub fn load() -> Self {
        let settings = crate::get_settings_manager().lock();
        let monitoring = &settings.get().monitoring;
        Self {
            respect_focus: monitoring.respect_focus.unwrap_or(true),
            quiet_start: monitoring
                .quiet_hours_start
                .as_deref()
                .and_then(parse_hhmm_to_minutes),
            quiet_end: monitoring
                .quiet_hours_end
                .as_deref()
                .and_then(parse_hhmm_to_minutes),
            dnd_until: monitoring
                .dnd_until
                .as_deref()
                .and_then(parse_rfc3339_local),
            dnd_indefinite: monitoring.dnd_indefinite.unwrap_or(false),
        }
    }

    /// Is Do Not Disturb in force at `now`?
    pub fn dnd_active_at(&self, now: chrono::DateTime<Local>) -> bool {
        if self.dnd_indefinite {
            return true;
        }
        self.dnd_until.is_some_and(|until| now < until)
    }

    /// Is `now` inside the configured quiet-hours window?
    pub fn in_quiet_hours_at(&self, now: chrono::DateTime<Local>) -> bool {
        let (Some(start), Some(end)) = (self.quiet_start, self.quiet_end) else {
            return false;
        };
        let minutes = now.hour() * 60 + now.minute();
        in_window(minutes, start, end)
    }
}

/// Is `minutes` inside the window `[start, end)`, which may wrap midnight?
///
/// A zero-width window (`start == end`) is treated as *disabled*, not as
/// "quiet all day" — the latter would silently mute 4DA forever if a user
/// set both ends to the same time.
pub fn in_window(minutes: u32, start: u32, end: u32) -> bool {
    if start == end {
        return false;
    }
    if start < end {
        minutes >= start && minutes < end
    } else {
        // Wraps midnight, e.g. 22:00 -> 07:00.
        minutes >= start || minutes < end
    }
}

/// Parse `"HH:MM"` into minutes past midnight.
///
/// Returns `None` for anything malformed or out of range, so a typo disables
/// quiet hours rather than silently meaning midnight.
pub fn parse_hhmm_to_minutes(value: &str) -> Option<u32> {
    let (hours, minutes) = value.trim().split_once(':')?;
    let hours: u32 = hours.parse().ok()?;
    let minutes: u32 = minutes.parse().ok()?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    Some(hours * 60 + minutes)
}

fn parse_rfc3339_local(value: &str) -> Option<chrono::DateTime<Local>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Local))
}

// ============================================================================
// Do Not Disturb control
// ============================================================================

/// Turn Do Not Disturb on for `minutes`, or indefinitely when `minutes` is
/// `None`. Persisted so it survives a restart.
pub fn set_do_not_disturb(minutes: Option<u64>) -> crate::error::Result<()> {
    let (until, indefinite) = match minutes {
        None => (None, true),
        Some(mins) => {
            let until = Local::now() + chrono::Duration::minutes(mins.min(24 * 60) as i64);
            (Some(until.to_rfc3339()), false)
        }
    };

    let mut settings = crate::get_settings_manager().lock();
    let monitoring = &mut settings.get_mut().monitoring;
    monitoring.dnd_until = until;
    monitoring.dnd_indefinite = Some(indefinite);
    settings.save()?;

    tracing::info!(
        target: "4da::presence",
        minutes = ?minutes,
        "Do Not Disturb enabled"
    );
    Ok(())
}

/// Turn Do Not Disturb off.
pub fn clear_do_not_disturb() -> crate::error::Result<()> {
    {
        let mut settings = crate::get_settings_manager().lock();
        let monitoring = &mut settings.get_mut().monitoring;
        monitoring.dnd_until = None;
        monitoring.dnd_indefinite = Some(false);
        settings.save()?;
    }
    tracing::info!(target: "4da::presence", "Do Not Disturb cleared");
    Ok(())
}

/// Is Do Not Disturb currently on?
pub fn is_do_not_disturb_on() -> bool {
    InterruptionConfig::load().dnd_active_at(Local::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(start: Option<u32>, end: Option<u32>) -> InterruptionConfig {
        InterruptionConfig {
            respect_focus: true,
            quiet_start: start,
            quiet_end: end,
            dnd_until: None,
            dnd_indefinite: false,
        }
    }

    // -- quiet-hours window ------------------------------------------------

    #[test]
    fn daytime_window_contains_midpoint() {
        // 09:00 -> 17:00, checked at 12:00
        assert!(in_window(12 * 60, 9 * 60, 17 * 60));
    }

    #[test]
    fn daytime_window_excludes_outside() {
        assert!(!in_window(8 * 60, 9 * 60, 17 * 60));
        assert!(!in_window(18 * 60, 9 * 60, 17 * 60));
    }

    #[test]
    fn window_is_start_inclusive_end_exclusive() {
        assert!(in_window(9 * 60, 9 * 60, 17 * 60));
        assert!(!in_window(17 * 60, 9 * 60, 17 * 60));
    }

    #[test]
    fn overnight_window_wraps_midnight() {
        // 22:00 -> 07:00 — the common case, and the one a naive
        // `start <= now && now < end` gets wrong.
        let (start, end) = (22 * 60, 7 * 60);
        assert!(in_window(23 * 60, start, end), "23:00 is quiet");
        assert!(in_window(2 * 60, start, end), "02:00 is quiet");
        assert!(in_window(0, start, end), "midnight is quiet");
        assert!(!in_window(12 * 60, start, end), "noon is not quiet");
        assert!(!in_window(8 * 60, start, end), "08:00 is not quiet");
    }

    #[test]
    fn zero_width_window_is_disabled_not_always_on() {
        // Guards against muting 4DA forever if both ends are set the same.
        for minute in [0, 6 * 60, 12 * 60, 23 * 60 + 59] {
            assert!(!in_window(minute, 9 * 60, 9 * 60));
        }
    }

    // -- HH:MM parsing -----------------------------------------------------

    #[test]
    fn parses_valid_times() {
        assert_eq!(parse_hhmm_to_minutes("00:00"), Some(0));
        assert_eq!(parse_hhmm_to_minutes("08:00"), Some(480));
        assert_eq!(parse_hhmm_to_minutes("23:59"), Some(1439));
        assert_eq!(parse_hhmm_to_minutes(" 09:30 "), Some(570));
    }

    #[test]
    fn rejects_malformed_times() {
        // Each of these must disable quiet hours, never silently mean 00:00.
        for bad in ["", "8", "8:00pm", "24:00", "09:60", "aa:bb", "09-30", "::"] {
            assert_eq!(parse_hhmm_to_minutes(bad), None, "should reject {bad:?}");
        }
    }

    // -- config predicates -------------------------------------------------

    #[test]
    fn quiet_hours_disabled_when_either_end_missing() {
        let now = Local::now();
        assert!(!cfg(Some(0), None).in_quiet_hours_at(now));
        assert!(!cfg(None, Some(600)).in_quiet_hours_at(now));
        assert!(!cfg(None, None).in_quiet_hours_at(now));
    }

    #[test]
    fn quiet_hours_covering_all_day_matches_now() {
        // 00:00 -> 23:59 covers every minute except the last one.
        let mut config = cfg(Some(0), Some(23 * 60 + 59));
        config.respect_focus = false;
        let now = Local::now();
        let expected = now.hour() * 60 + now.minute() < 23 * 60 + 59;
        assert_eq!(config.in_quiet_hours_at(now), expected);
    }

    #[test]
    fn dnd_expires_at_its_deadline() {
        let now = Local::now();
        let mut config = cfg(None, None);
        config.dnd_until = Some(now + chrono::Duration::minutes(30));
        assert!(config.dnd_active_at(now), "active before deadline");
        assert!(
            !config.dnd_active_at(now + chrono::Duration::minutes(31)),
            "expired after deadline"
        );
    }

    #[test]
    fn dnd_indefinite_never_expires() {
        let mut config = cfg(None, None);
        config.dnd_indefinite = true;
        let far_future = Local::now() + chrono::Duration::days(365);
        assert!(config.dnd_active_at(far_future));
    }

    #[test]
    fn dnd_off_by_default() {
        assert!(!cfg(None, None).dnd_active_at(Local::now()));
    }

    // -- reason metadata ---------------------------------------------------

    #[test]
    fn every_reason_has_distinct_tag_and_user_text() {
        let all = [
            BusyReason::DoNotDisturb,
            BusyReason::QuietHours,
            BusyReason::FullscreenApp,
            BusyReason::FullscreenWindow,
            BusyReason::Presentation,
            BusyReason::OsQuietTime,
            BusyReason::ScreenLocked,
        ];
        let mut tags: Vec<&str> = all.iter().map(|r| r.as_str()).collect();
        tags.sort_unstable();
        let count = tags.len();
        tags.dedup();
        assert_eq!(tags.len(), count, "busy reason tags must be unique");

        for reason in all {
            assert!(!reason.as_str().is_empty());
            assert!(!reason.user_text().is_empty());
            // User-facing copy describes the user's state, not 4DA's.
            assert!(
                reason.user_text().starts_with("while") || reason.user_text().starts_with("during"),
                "{} should read as a time clause",
                reason.as_str()
            );
        }
    }

    #[test]
    fn presence_accessors() {
        assert!(Presence::Available.is_available());
        assert_eq!(Presence::Available.busy_reason(), None);
        let busy = Presence::Busy(BusyReason::FullscreenApp);
        assert!(!busy.is_available());
        assert_eq!(busy.busy_reason(), Some(BusyReason::FullscreenApp));
    }

    #[test]
    fn busy_reason_serde_roundtrip_is_snake_case() {
        let json = serde_json::to_string(&BusyReason::FullscreenWindow).unwrap();
        assert_eq!(json, "\"fullscreen_window\"");
        let back: BusyReason = serde_json::from_str(&json).unwrap();
        assert_eq!(back, BusyReason::FullscreenWindow);
    }
}
