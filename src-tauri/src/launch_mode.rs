// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! How this GUI process was launched: by the person, or by the OS at sign-in.
//!
//! The launch-at-login entry passes [`AUTOSTART_ARG`]. A sign-in launch stays
//! in the system tray instead of opening the main window: nobody asked for a
//! window while the desktop was still starting. Measured 2026-11-22 on the
//! operator machine: the window opened 90 s after sign-in and sat on its
//! loading screen for three minutes. The window opens the moment the person
//! asks for it: the tray icon, the tray menu, or launching 4DA again (the
//! single-instance handler shows it).
//!
//! The window stays hidden wherever a tray icon is there to bring it back.
//! It does NOT depend on "keep running in background". The first version did
//! (hidden only when that setting was on), and the first live run showed why
//! that was wrong: on the operator machine the setting is off, so the window
//! still opened at sign-in. With a tray, 4DA keeps running after its window is
//! closed whichever way the setting points: the setting picks hide over
//! destroy, and the helper windows and the exit guard (#501) keep the process.
//! So a window at sign-in changed nothing about where 4DA ends up; it was only
//! one more window to dismiss. Without a visible tray icon the window opens,
//! exactly as a click does, because nothing else could reopen it.

use std::sync::OnceLock;

/// Passed by the launch-at-login entry, and by anything else that starts 4DA
/// at sign-in on the person's behalf.
pub(crate) const AUTOSTART_ARG: &str = "--autostart";

static LAUNCHED_AT_SIGN_IN: OnceLock<bool> = OnceLock::new();

/// True when the arguments carry [`AUTOSTART_ARG`].
pub(crate) fn has_autostart_arg(args: impl IntoIterator<Item = String>) -> bool {
    args.into_iter().any(|arg| arg == AUTOSTART_ARG)
}

/// Was this process started at sign-in rather than by the person? Read once
/// from the process arguments.
pub(crate) fn launched_at_sign_in() -> bool {
    *LAUNCHED_AT_SIGN_IN.get_or_init(|| has_autostart_arg(std::env::args().skip(1)))
}

/// Does closing the main window hide it to the tray when the person has not
/// chosen either way? GNOME, Pantheon and Unity show no tray icon without an
/// extension, so a hidden window there could not be reopened from one.
pub(crate) fn tray_resident_by_default() -> bool {
    #[cfg(target_os = "linux")]
    {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_uppercase();
        !desktop.contains("GNOME") && !desktop.contains("PANTHEON") && !desktop.contains("UNITY")
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// Does the main window hide to the tray (rather than leave the screen for
/// good)? `close_to_tray_pref` is the "keep running in background" setting.
/// Never without a tray icon: the window would be unreachable.
pub(crate) fn hides_to_tray(close_to_tray_pref: Option<bool>, tray_available: bool) -> bool {
    close_to_tray_pref.unwrap_or_else(tray_resident_by_default) && tray_available
}

/// Should this launch leave the main window hidden at first light?
pub(crate) fn starts_in_tray(
    launched_at_sign_in: bool,
    close_to_tray_pref: Option<bool>,
    tray_available: bool,
) -> bool {
    starts_in_tray_on(
        launched_at_sign_in,
        close_to_tray_pref,
        tray_available,
        tray_resident_by_default(),
    )
}

/// [`starts_in_tray`] for a stated desktop. `desktop_shows_tray`: the desktop
/// draws tray icons without an extension (everything but GNOME, Pantheon and
/// Unity). On those three a tray icon can be created and still be invisible,
/// so only a person who switched "keep running in background" ON, which says
/// they have a working tray, gets the quiet start there.
fn starts_in_tray_on(
    launched_at_sign_in: bool,
    close_to_tray_pref: Option<bool>,
    tray_available: bool,
    desktop_shows_tray: bool,
) -> bool {
    launched_at_sign_in
        && tray_available
        && (desktop_shows_tray || close_to_tray_pref == Some(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn autostart_arg_is_found_among_other_arguments() {
        assert!(has_autostart_arg(args(&["--autostart"])));
        assert!(has_autostart_arg(args(&["4da://open", "--autostart"])));
    }

    #[test]
    fn a_plain_launch_is_not_a_sign_in_launch() {
        assert!(!has_autostart_arg(args(&[])));
        assert!(!has_autostart_arg(args(&["4da://open"])));
        // Exact match only: a deep link or path that merely contains the
        // text must not silence the window.
        assert!(!has_autostart_arg(args(&["--autostart=1"])));
        assert!(!has_autostart_arg(args(&["C:\\x\\--autostart\\y"])));
    }

    // Arguments of starts_in_tray_on: launched at sign-in, the "keep running
    // in background" setting, a tray icon was created, the desktop draws tray
    // icons without an extension.

    #[test]
    fn a_sign_in_launch_stays_in_the_tray_wherever_the_tray_is_visible() {
        assert!(starts_in_tray_on(true, Some(true), true, true));
        assert!(starts_in_tray_on(true, None, true, true));
    }

    #[test]
    fn background_running_switched_off_still_starts_in_the_tray() {
        // The operator machine, 2026-11-22: the setting was off and the first
        // version opened the window at sign-in because of it. 4DA stays in
        // the tray after its window is closed either way, so the window at
        // sign-in bought nothing.
        assert!(starts_in_tray_on(true, Some(false), true, true));
    }

    #[test]
    fn a_click_always_opens_the_window() {
        assert!(!starts_in_tray_on(false, Some(true), true, true));
        assert!(!starts_in_tray_on(false, None, true, true));
        assert!(!starts_in_tray(false, Some(true), true));
    }

    #[test]
    fn no_tray_icon_means_the_window_opens() {
        assert!(!starts_in_tray_on(true, Some(true), false, true));
        assert!(!starts_in_tray_on(true, None, false, true));
        assert!(!starts_in_tray(true, Some(true), false));
        assert!(!hides_to_tray(Some(true), false));
    }

    #[test]
    fn a_desktop_that_hides_tray_icons_opens_the_window_unless_the_person_has_a_tray() {
        // GNOME, Pantheon, Unity: the icon exists but nothing draws it.
        assert!(!starts_in_tray_on(true, None, true, false));
        assert!(!starts_in_tray_on(true, Some(false), true, false));
        // "Keep running in background" switched ON there says a tray extension is installed.
        assert!(starts_in_tray_on(true, Some(true), true, false));
    }

    #[test]
    fn the_running_desktop_decides_through_the_same_rule() {
        for pref in [None, Some(true), Some(false)] {
            assert_eq!(
                starts_in_tray(true, pref, true),
                starts_in_tray_on(true, pref, true, tray_resident_by_default())
            );
        }
        #[cfg(not(target_os = "linux"))]
        assert!(tray_resident_by_default());
    }

    #[test]
    fn closing_the_window_still_follows_the_setting() {
        // Unchanged by the sign-in rule: hide on close only when the setting
        // (or the desktop default) says so and a tray exists.
        assert!(hides_to_tray(Some(true), true));
        assert!(!hides_to_tray(Some(false), true));
        assert_eq!(hides_to_tray(None, true), tray_resident_by_default());
    }
}
