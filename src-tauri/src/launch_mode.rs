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
//! The window stays hidden only where the person can bring it back, which is
//! the test closing the window already uses: "keep running in background" is
//! on and a tray icon exists. Anywhere else a sign-in launch opens the window
//! exactly as a click does.

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
    launched_at_sign_in && hides_to_tray(close_to_tray_pref, tray_available)
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

    #[test]
    fn a_sign_in_launch_stays_in_the_tray_when_the_tray_can_reopen_it() {
        assert!(starts_in_tray(true, Some(true), true));
    }

    #[test]
    fn a_click_always_opens_the_window() {
        assert!(!starts_in_tray(false, Some(true), true));
        assert!(!starts_in_tray(false, None, true));
    }

    #[test]
    fn no_tray_icon_means_the_window_opens() {
        assert!(!starts_in_tray(true, Some(true), false));
        assert!(!hides_to_tray(Some(true), false));
    }

    #[test]
    fn background_running_switched_off_means_the_window_opens() {
        // Closing the window quits 4DA for this person, so a start they
        // cannot see would be a process they never asked to keep.
        assert!(!starts_in_tray(true, Some(false), true));
    }

    #[test]
    fn an_unset_preference_follows_the_desktop_default() {
        assert_eq!(starts_in_tray(true, None, true), tray_resident_by_default());
        #[cfg(not(target_os = "linux"))]
        assert!(tray_resident_by_default());
    }
}
