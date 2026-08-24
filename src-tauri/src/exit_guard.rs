// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Exit-path guard and attribution (issue #501).
//!
//! 4DA is a tray-resident app: the process must outlive its windows. On
//! Windows, tao's event loop `process::exit()`s the moment it terminates, and
//! tauri-runtime-wry terminates it when the LAST window is destroyed unless
//! the `RunEvent::ExitRequested { code: None }` it fires first is answered
//! with `api.prevent_exit()`. Before this module existed nothing answered it,
//! so the app was one `window.destroy()` away from silent death whenever the
//! live window count reached zero. That moment is reachable in normal
//! operation: the notification/briefing "JS never loaded — recreating"
//! recovery destroys the stale window and only then creates its replacement,
//! and the destroy is processed on the event loop BEFORE the replacement
//! exists — if it was the last window, the runtime's is-empty check fires and
//! the process dies. Because tao exits via `process::exit`, the non-blocking
//! log appender's buffered tail (including the destroy line itself) is lost,
//! which is why such deaths look like the log "ends mid-flight".
//!
//! Forensic classification for future silent deaths (#501):
//!
//! - `RunEvent::Exit` ran ("Application shutting down - cleaning up..."):
//!   orderly exit — the preceding "Exit requested" line names the code.
//! - Console-ctrl fingerprint only ("Console control event received", or
//!   "Clean shutdown markers removed" with NO shutdown line): the attached
//!   console delivered Ctrl+C / closed — the hidden cmd or bash job that
//!   launched a detached run was torn down. Observed live 2026-08-24 12:19:20Z.
//! - Neither fingerprint AND `.running` left behind (`prev_crashed=true` on
//!   the next boot): external `TerminateProcess` — e.g. `taskkill /F`, which
//!   reports exit code exactly 1 and which `scripts/kill-fourda.cjs` issues
//!   against every fourda.exe built from this tree at the head of every
//!   `pnpm run dev`. No in-process code can observe that kill; attribute it
//!   from the killer's side. Observed live 2026-08-24 (~10:20Z death,
//!   `prev_crashed=true` at the 12:07:59Z boot).

/// Decide whether a `RunEvent::ExitRequested` should be prevented.
///
/// `code == None` means the exit was requested by window teardown — the last
/// window was destroyed — NOT by an explicit `app.exit(..)` or
/// `app.restart()` (those carry `Some(code)`, restart specifically
/// `Some(i32::MAX)`, and must always proceed). A tray-resident app must
/// survive window teardown, but only while the tray actually exists: with no
/// tray AND no windows there is no surface left through which the user could
/// ever reach the process again, so letting it exit is the correct call.
///
/// Called from `app_setup::handle_run_event`, which must invoke
/// `api.prevent_exit()` synchronously inside the event callback — the runtime
/// checks the answer with `try_recv` immediately after the callback returns.
#[must_use]
pub fn should_prevent_exit(code: Option<i32>, tray_alive: bool) -> bool {
    code.is_none() && tray_alive
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The #501 hole: last-window teardown with a live tray must NOT exit
    /// the process. This pins the guard that `handle_run_event` relies on —
    /// if this test starts failing, the app is again one `window.destroy()`
    /// away from silent death.
    #[test]
    fn window_teardown_with_tray_is_prevented() {
        assert!(should_prevent_exit(None, true));
    }

    /// Explicit exits always proceed: tray quit (`app.exit(0)`), error exits
    /// (`app.exit(1)`), and restart (`Some(i32::MAX)`, where `prevent_exit`
    /// would be ignored by tauri anyway).
    #[test]
    fn explicit_exit_codes_always_proceed() {
        assert!(!should_prevent_exit(Some(0), true));
        assert!(!should_prevent_exit(Some(1), true));
        assert!(!should_prevent_exit(Some(i32::MAX), true));
        assert!(!should_prevent_exit(Some(0), false));
    }

    /// No tray and no windows = an unreachable process; let it exit rather
    /// than leak a zombie the user cannot see or quit.
    #[test]
    fn window_teardown_without_tray_proceeds() {
        assert!(!should_prevent_exit(None, false));
    }
}
