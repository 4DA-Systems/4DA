// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Presence detection for platforms without an OS-level implementation yet
//! (macOS, Linux).
//!
//! This is a **known gap, not a no-op by design**. On these platforms the
//! interruption gate still works — quiet hours, Do Not Disturb, and the
//! deferral queue are all platform-independent — but 4DA cannot yet see that a
//! fullscreen app is running, so those two OS-driven reasons never fire.
//!
//! Implementing them means:
//!
//! - **macOS**: `NSWorkspace` occlusion state, or reading
//!   `NSApplicationPresentationOptions` for the frontmost app. Focus modes are
//!   readable from `~/Library/DoNotDisturb/DB/Assertions.json` on Ventura+.
//! - **Linux**: no portable answer. X11 exposes `_NET_WM_STATE_FULLSCREEN` via
//!   `_NET_ACTIVE_WINDOW`; Wayland deliberately does not, so a compositor-
//!   specific path (or the `org.freedesktop.portal.Inhibit` portal) is needed.
//!
//! Returning `None` means "the OS did not tell us the user is busy", which is
//! the same honest answer the Windows detector gives when its query fails.
//! It must never be read as "the user is definitely available".

use super::BusyReason;

/// Always `None`: no OS-level presence signal is available on this platform.
///
/// See the module docs for what implementing it would require.
pub(super) fn detect() -> Option<BusyReason> {
    None
}
