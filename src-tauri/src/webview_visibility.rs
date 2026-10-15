// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! A hidden window's WebView2 stops rendering.
//!
//! On Windows, Tauri's `hide()` and a minimise act on the native window only;
//! the WebView2 controller inside it keeps `IsVisible = true` (wry sets it once
//! at creation and never again for a window-level hide). Chromium therefore
//! treats the page as on screen: `document.hidden` stays `false`, timers and
//! `requestAnimationFrame` run at full rate, and an infinite CSS animation keeps
//! the renderer and the GPU process producing frames nobody can see.
//!
//! Measured 2026-10-15. Release build: after a toast and a desktop brief had
//! been shown and hidden, the app idled at 8.6% of a core (one renderer 3.8%,
//! GPU process 3.2%) against 0.6% for the same build that had shown neither.
//! Debug build, both windows hidden: each still reported `visibilityState
//! "visible"`, `requestAnimationFrame` at the display rate, and a running
//! infinite animation (`.priority-dot.alert` `dot-pulse`; the brief's
//! `.synthesis-text` `pulse-opacity`); two renderers at ~3% and the GPU process
//! at 4.6%. Pausing just those animations dropped both renderers to 0% and the
//! GPU process to 0.05%. WebView2's own guidance is to set `IsVisible` to false
//! whenever the host window is hidden or minimised.
//!
//! This plugin subclasses every webview's host window and mirrors the native
//! visibility into the controller: hidden or minimised sets `IsVisible(false)`,
//! shown or restored sets it back. Every hide path (the toast and brief
//! timers, close-to-tray, minimise) goes through the window messages, so no
//! call site has to remember it. As a side effect `document.hidden` now tells
//! the truth, which the frontend's motion gate (`src/lib/motion-gate.ts`)
//! already assumed.
//!
//! The initial state is left alone: a window created hidden keeps loading and
//! running its scripts as before, and only a real show or hide changes the
//! controller. macOS and Linux need nothing here (WKWebView and WebKitGTK follow
//! their window's occlusion), so the plugin is inert there.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;

/// The plugin. Register it once on the app builder.
pub(crate) fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("webview-visibility")
        .on_webview_ready(|webview| {
            #[cfg(windows)]
            windows_impl::track(&webview);
            #[cfg(not(windows))]
            let _ = webview;
        })
        .build()
}

/// What a host-window message means for the webview inside it: `Some(true)`
/// to render, `Some(false)` to stop, `None` to leave it as it is.
///
/// `window_visible` is whether the host window is visible when the message
/// arrives.
#[cfg_attr(not(windows), allow(dead_code))]
fn desired_visibility(msg: Msg, window_visible: bool) -> Option<bool> {
    match msg {
        Msg::Show(shown) => Some(shown),
        Msg::Size(SizeKind::Minimized) => Some(false),
        // A restore or maximise of a window that is still hidden (a hidden
        // window can be resized) must not wake it, nor put to sleep a webview
        // that has not been shown yet.
        Msg::Size(SizeKind::Restored) => window_visible.then_some(true),
        Msg::Size(SizeKind::Other) | Msg::Other => None,
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SizeKind {
    Minimized,
    Restored,
    Other,
}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Msg {
    Show(bool),
    Size(SizeKind),
    Other,
}

#[cfg(windows)]
mod windows_impl {
    use std::cell::Cell;

    use tauri::Runtime;
    use tracing::{debug, warn};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsWindowVisible, SIZE_MAXIMIZED, SIZE_MINIMIZED, SIZE_RESTORED, WM_NCDESTROY,
        WM_SHOWWINDOW, WM_SIZE,
    };

    use super::{desired_visibility, Msg, SizeKind};

    /// Subclass id ("4DAV"); distinct from wry's own subclass on the same window.
    const SUBCLASS_ID: usize = 0x3444_4156;

    /// Owned by the subclass; freed on `WM_NCDESTROY`.
    struct Tracker {
        set_visible: Box<dyn Fn(bool)>,
        /// Last value written. `None` until the first transition: the
        /// controller's creation-time state is wry's, not ours.
        current: Cell<Option<bool>>,
    }

    impl Tracker {
        fn apply(&self, visible: bool) {
            if self.current.get() != Some(visible) {
                self.current.set(Some(visible));
                (self.set_visible)(visible);
            }
        }
    }

    pub(super) fn track<R: Runtime>(webview: &tauri::Webview<R>) {
        let label = webview.label().to_string();
        let hwnd = match webview.window().hwnd() {
            // Carried as an integer: a raw pointer is not `Send`.
            Ok(h) => h.0 as isize,
            Err(e) => {
                warn!(target: "4da::webview", label, error = %e, "No host window handle; webview visibility not tracked");
                return;
            }
        };
        let result = webview.with_webview(move |platform| {
            let controller = platform.controller();
            let tracker = Box::new(Tracker {
                set_visible: Box::new(move |visible| {
                    // SAFETY: called on the UI thread that owns the controller
                    // (window procedures run on the window's thread).
                    #[allow(unsafe_code)]
                    let _ = unsafe { controller.SetIsVisible(visible) };
                }),
                current: Cell::new(None),
            });
            let data = Box::into_raw(tracker) as usize;
            // SAFETY: `hwnd` is this webview's live host window and we are on
            // its thread; `data` stays valid until `WM_NCDESTROY` frees it.
            #[allow(unsafe_code)]
            let ok = unsafe {
                SetWindowSubclass(hwnd as HWND, Some(subclass_proc), SUBCLASS_ID, data)
            };
            if ok == 0 {
                // SAFETY: the subclass was not installed, so nothing else owns `data`.
                #[allow(unsafe_code)]
                drop(unsafe { Box::from_raw(data as *mut Tracker) });
                warn!(target: "4da::webview", label, "SetWindowSubclass failed; webview visibility not tracked");
            } else {
                debug!(target: "4da::webview", label, "Webview visibility follows its window");
            }
        });
        if let Err(e) = result {
            warn!(target: "4da::webview", error = %e, "with_webview failed; webview visibility not tracked");
        }
    }

    fn classify(msg: u32, wparam: WPARAM) -> Msg {
        match msg {
            WM_SHOWWINDOW => Msg::Show(wparam != 0),
            WM_SIZE => Msg::Size(match wparam as u32 {
                SIZE_MINIMIZED => SizeKind::Minimized,
                SIZE_RESTORED | SIZE_MAXIMIZED => SizeKind::Restored,
                _ => SizeKind::Other,
            }),
            _ => Msg::Other,
        }
    }

    #[allow(unsafe_code)]
    unsafe extern "system" fn subclass_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        data: usize,
    ) -> LRESULT {
        let tracker = data as *const Tracker;
        if msg == WM_NCDESTROY {
            // SAFETY: removing our own subclass on our window's thread; after
            // this no message reaches `subclass_proc` with `data`, so the box
            // can be reclaimed exactly once.
            unsafe {
                RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
                drop(Box::from_raw(tracker.cast_mut()));
            }
        } else if !tracker.is_null() {
            // SAFETY: `data` is the live `Tracker` installed in `track`.
            let tracker = unsafe { &*tracker };
            let visible = unsafe { IsWindowVisible(hwnd) } != 0;
            if let Some(want) = desired_visibility(classify(msg, wparam), visible) {
                tracker.apply(want);
            }
        }
        // SAFETY: forwarding the message down the subclass chain.
        unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn window_messages_classify() {
            assert_eq!(classify(WM_SHOWWINDOW, 1), Msg::Show(true));
            assert_eq!(classify(WM_SHOWWINDOW, 0), Msg::Show(false));
            assert_eq!(
                classify(WM_SIZE, SIZE_MINIMIZED as usize),
                Msg::Size(SizeKind::Minimized)
            );
            assert_eq!(
                classify(WM_SIZE, SIZE_RESTORED as usize),
                Msg::Size(SizeKind::Restored)
            );
            assert_eq!(
                classify(WM_SIZE, SIZE_MAXIMIZED as usize),
                Msg::Size(SizeKind::Restored)
            );
            assert_eq!(classify(WM_SIZE, 3), Msg::Size(SizeKind::Other));
            assert_eq!(classify(0x0200, 0), Msg::Other);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hide_and_show_follow_the_window() {
        assert_eq!(desired_visibility(Msg::Show(false), true), Some(false));
        assert_eq!(desired_visibility(Msg::Show(true), false), Some(true));
    }

    #[test]
    fn minimise_stops_rendering() {
        assert_eq!(
            desired_visibility(Msg::Size(SizeKind::Minimized), true),
            Some(false)
        );
    }

    #[test]
    fn restore_wakes_only_a_visible_window() {
        assert_eq!(
            desired_visibility(Msg::Size(SizeKind::Restored), true),
            Some(true)
        );
        // A hidden pre-warmed window can be resized; that changes nothing.
        assert_eq!(
            desired_visibility(Msg::Size(SizeKind::Restored), false),
            None
        );
    }

    #[test]
    fn unrelated_messages_change_nothing() {
        assert_eq!(desired_visibility(Msg::Size(SizeKind::Other), true), None);
        assert_eq!(desired_visibility(Msg::Other, false), None);
    }
}
