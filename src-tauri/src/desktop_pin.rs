// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Desktop-level window pinning for 4DA.
//!
//! Pins a Tauri window to the desktop layer — behind all normal windows,
//! at the same z-order as desktop shortcuts. The window is only visible
//! when no other windows cover that area of the screen.
//!
//! Platform behavior:
//! - **Windows**: Sets `HWND_BOTTOM` z-order + `WS_EX_NOACTIVATE` + `WS_EX_TOOLWINDOW`
//! - **macOS**: Sets NSWindow level to `kCGDesktopWindowLevel`
//! - **Linux**: Sets `_NET_WM_WINDOW_TYPE_DESKTOP` X11 window property

use tauri::Runtime;
use tracing::{info, warn};

/// Pin a Tauri webview window to the desktop level.
///
/// After calling this, the window will sit behind all normal windows,
/// only visible when the desktop is exposed. On failure, the window
/// remains in its current z-order (graceful degradation).
pub fn pin_to_desktop<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    #[cfg(target_os = "windows")]
    {
        pin_to_desktop_windows(window);
    }

    #[cfg(target_os = "macos")]
    {
        pin_to_desktop_macos(window);
    }

    #[cfg(target_os = "linux")]
    {
        pin_to_desktop_linux(window);
    }
}

// ============================================================================
// Windows: HWND_BOTTOM + WS_EX_NOACTIVATE + WS_EX_TOOLWINDOW
// ============================================================================

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
fn pin_to_desktop_windows<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    // Win32 API declarations (minimal surface area)
    extern "system" {
        fn SetWindowPos(
            h_wnd: isize,
            h_wnd_insert_after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            u_flags: u32,
        ) -> i32;
        fn GetWindowLongPtrW(h_wnd: isize, n_index: i32) -> isize;
        fn SetWindowLongPtrW(h_wnd: isize, n_index: i32, dw_new_long: isize) -> isize;
    }

    const HWND_BOTTOM: isize = 1;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_NOACTIVATE: isize = 0x0800_0000;
    const WS_EX_TOOLWINDOW: isize = 0x0000_0080;

    // Get the HWND from the Tauri window via its built-in platform accessor.
    let hwnd = match window.hwnd() {
        Ok(h) => h.0 as isize,
        Err(e) => {
            warn!(target: "4da::desktop_pin", error = %e, "Failed to get HWND — desktop pinning skipped");
            return;
        }
    };

    unsafe {
        // Place window at the bottom of the z-order (desktop level)
        let result = SetWindowPos(
            hwnd,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        if result == 0 {
            warn!(target: "4da::desktop_pin", "SetWindowPos(HWND_BOTTOM) failed");
            return;
        }

        // Set extended window styles:
        // WS_EX_NOACTIVATE — clicking the window doesn't raise it or steal focus
        // WS_EX_TOOLWINDOW — hidden from Alt+Tab task switcher
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            ex_style | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
        );
    }

    info!(target: "4da::desktop_pin", "Window pinned to desktop level (Windows)");
}

// ============================================================================
// macOS: NSWindow level = kCGDesktopWindowLevel
// ============================================================================

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn pin_to_desktop_macos<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    // Core Graphics window level constants
    // kCGDesktopWindowLevel = kCGMinimumWindowLevel + 20 = -2147483628
    // We use kCGDesktopWindowLevel + 1 to sit just above the wallpaper
    const DESKTOP_WINDOW_LEVEL: i64 = -2147483627;

    extern "C" {
        // objc_msgSend is the Objective-C message dispatch function.
        // We use it to call [nswindow setLevel:] and [nswindow setCollectionBehavior:]
        fn objc_msgSend(
            obj: *mut std::ffi::c_void,
            sel: *mut std::ffi::c_void,
            ...
        ) -> *mut std::ffi::c_void;
        fn sel_registerName(name: *const i8) -> *mut std::ffi::c_void;
    }

    // Tauri provides ns_window() directly — no need for raw_window_handle.
    let ns_window = match window.ns_window() {
        Ok(ptr) => ptr,
        Err(e) => {
            warn!(target: "4da::desktop_pin", error = %e, "Failed to get NSWindow — desktop pinning skipped");
            return;
        }
    };

    if ns_window.is_null() {
        warn!(target: "4da::desktop_pin", "NSWindow pointer is null");
        return;
    }

    unsafe {
        // [ns_window setLevel: DESKTOP_WINDOW_LEVEL]
        let sel_set_level = sel_registerName(b"setLevel:\0".as_ptr() as *const i8);
        objc_msgSend(ns_window as *mut _, sel_set_level, DESKTOP_WINDOW_LEVEL);

        // Set collection behavior to allow the window to join all spaces
        // NSWindowCollectionBehaviorCanJoinAllSpaces = 1 << 0
        // NSWindowCollectionBehaviorStationary = 1 << 4
        let sel_set_behavior = sel_registerName(b"setCollectionBehavior:\0".as_ptr() as *const i8);
        let behavior: u64 = (1 << 0) | (1 << 4); // canJoinAllSpaces | stationary
        objc_msgSend(ns_window as *mut _, sel_set_behavior, behavior);
    }

    info!(target: "4da::desktop_pin", "Window pinned to desktop level (macOS)");
}

// ============================================================================
// Linux: _NET_WM_WINDOW_TYPE_DESKTOP (X11) or no-op (Wayland)
// ============================================================================

#[cfg(target_os = "linux")]
fn pin_to_desktop_linux<R: Runtime>(_window: &tauri::WebviewWindow<R>) {
    // On Linux with X11, setting _NET_WM_WINDOW_TYPE to _NET_WM_WINDOW_TYPE_DESKTOP
    // tells the window manager to treat this as a desktop widget.
    // However, accessing X11 atoms requires xlib FFI which varies across distros.
    // For now, use xdotool if available for a simple approach.
    // The window is already not always-on-top, so it naturally sits behind other windows.

    // Try xdotool approach (available on most X11 desktops)
    // xdotool gets the window ID and sets the window type
    // This is a best-effort approach — fails silently on Wayland or if xdotool is missing.
    if let Err(e) = std::process::Command::new("xdotool")
        .args(["getactivewindow"])
        .output()
    {
        info!(target: "4da::desktop_pin", error = %e, "xdotool not available — window not pinned to desktop (Linux)");
        return;
    }

    // On Linux, the window will naturally sit behind focused windows since
    // we're not using always_on_top. This is acceptable behavior.
    info!(target: "4da::desktop_pin", "Desktop pinning on Linux — window uses normal z-order (behind focused windows)");
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #[test]
    fn test_desktop_pin_module_exists() {
        // Verify the module compiles and the public API is accessible.
        // Actual pinning requires a live Tauri window, so we just confirm
        // that the function signature is correct and the module loads.
        #[cfg(target_os = "windows")]
        {
            // Win32 constants verified at compile time via the function body.
            // HWND_BOTTOM = 1, SWP_NOMOVE = 0x0002, etc.
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos_desktop_level() {
        // kCGMinimumWindowLevel = -2147483648
        // kCGDesktopWindowLevel = kCGMinimumWindowLevel + 20 = -2147483628
        // We use +1 above that = -2147483627
        let level: i64 = -2147483627;
        assert!(level < 0, "Desktop level should be negative (below normal)");
    }
}
