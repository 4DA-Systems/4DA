// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Linux presence detection, via the EWMH hints every major X11 window manager
//! implements.
//!
//! Ask the root window for `_NET_ACTIVE_WINDOW`, then ask that window whether
//! `_NET_WM_STATE` contains `_NET_WM_STATE_FULLSCREEN`. That is the same
//! question the Windows detector answers by comparing rectangles, but here the
//! window manager has already computed the answer.
//!
//! ## What this does and does not cover
//!
//! - **X11 sessions**: works.
//! - **Wayland sessions running a game through XWayland** (which is how Proton,
//!   and therefore most Linux gaming, actually runs): the game is an X11 client,
//!   so it appears here. Whether the XWayland root's `_NET_ACTIVE_WINDOW`
//!   tracks the Wayland compositor's focus is compositor-specific, so treat
//!   this as best-effort rather than guaranteed.
//! - **Native Wayland applications**: invisible to this. Wayland deliberately
//!   denies clients any view of other clients' windows, and there is no portable
//!   replacement. A native-Wayland fullscreen app will not be detected, and
//!   quiet hours plus Do Not Disturb remain the only protection there.
//!
//! That last case is a real, permanent gap, not an oversight — it is why
//! `PresenceStatus::os_detection_supported` exists and why the settings panel
//! says so rather than implying a capability 4DA does not have.
//!
//! ## Fail-open
//!
//! No display, no X server, a denied connection, a missing property, a
//! compositor that does not set EWMH hints: every one of them returns `None`
//! (available). Wrongly muting 4DA forever is a worse failure than one
//! mistimed popup.
//!
//! Unlike the macOS module, this file **is** compile-verified in PR CI —
//! `hermetic.yml`'s fresh-clone matrix builds on `ubuntu-22.04`.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

use super::BusyReason;

/// Query the window manager for whether the active window is fullscreen.
pub(super) fn detect() -> Option<BusyReason> {
    if active_window_is_fullscreen()? {
        Some(BusyReason::FullscreenWindow)
    } else {
        None
    }
}

/// `Some(true)` when the active window advertises `_NET_WM_STATE_FULLSCREEN`,
/// `Some(false)` when it demonstrably does not, `None` when we could not ask.
///
/// The three cases are kept distinct so that "could not ask" never masquerades
/// as "definitely not fullscreen" in future callers. An empty answer from a
/// question you could not ask is not a negative.
fn active_window_is_fullscreen() -> Option<bool> {
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen_num)?.root;

    let active_atom = intern(&conn, b"_NET_ACTIVE_WINDOW")?;
    let state_atom = intern(&conn, b"_NET_WM_STATE")?;
    let fullscreen_atom = intern(&conn, b"_NET_WM_STATE_FULLSCREEN")?;

    // _NET_ACTIVE_WINDOW holds a single window id on the root window.
    let active = conn
        .get_property(false, root, active_atom, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?
        .value32()
        .and_then(|mut values| values.next())?;

    if active == 0 {
        // No active window — an empty desktop is not a reason to hold.
        return Some(false);
    }

    // _NET_WM_STATE is a list of atoms; 32 is far more than any real window uses.
    let state = conn
        .get_property(false, active, state_atom, AtomEnum::ATOM, 0, 32)
        .ok()?
        .reply()
        .ok()?;

    Some(
        state
            .value32()
            .is_some_and(|mut atoms| atoms.any(|atom| atom == fullscreen_atom)),
    )
}

/// Resolve an EWMH atom name, or `None` if the server will not give it to us.
fn intern<C: Connection>(conn: &C, name: &[u8]) -> Option<u32> {
    Some(conn.intern_atom(false, name).ok()?.reply().ok()?.atom)
}
