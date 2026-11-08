// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! The WebView2 DevTools Protocol (CDP) port of a DEBUG build. (The Victauri MCP
//! server's port is victauri-plugin's own: since 0.10.4 it falls back to an
//! OS-assigned port when all of 7373-7383 is unavailable.)
//!
//! A release build has neither. [`webview_browser_args`] is `None` there, so
//! every webview keeps wry's own arguments and no remote debugging port can be
//! opened; the code that builds the debug arguments does not exist in a
//! release binary (`cfg(debug_assertions)`).
//!
//! ## Why the arguments are passed per webview
//!
//! The old mechanism, `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`, never took
//! effect: wry always passes explicit `AdditionalBrowserArguments`
//! (`--disable-features=...`), and the browser process came up without the flag
//! even with the variable set in the launch environment (verified 2026-11-05).
//!
//! Every webview in the process shares one WebView2 user-data folder, and
//! WebView2 refuses a second environment on that folder whose options differ
//! (`HRESULT 0x8007139F`, `ERROR_INVALID_STATE`). So the main window, the
//! notification window and the briefing window must all get the IDENTICAL
//! string — computed once and cached here.

use std::sync::OnceLock;

use tauri::{Manager, Runtime, WebviewWindowBuilder};

/// wry's default arguments. Passing any arguments replaces them, so a custom
/// string must repeat them (mini menu, PDF mini menu, SmartScreen off).
#[cfg_attr(not(all(debug_assertions, windows)), allow(dead_code))]
const WRY_DEFAULT_BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

/// The CDP port a debug build opens unless `FOURDA_CDP_PORT` names another.
#[cfg(all(debug_assertions, windows))]
pub(crate) const DEFAULT_CDP_PORT: u16 = 9222;

/// The browser arguments every webview of this process is created with, or
/// `None` to leave wry's defaults alone (every release build, and non-Windows).
pub(crate) fn webview_browser_args() -> Option<&'static str> {
    static ARGS: OnceLock<Option<String>> = OnceLock::new();
    ARGS.get_or_init(compute_browser_args).as_deref()
}

#[cfg(all(debug_assertions, windows))]
fn compute_browser_args() -> Option<String> {
    let raw = std::env::var("FOURDA_CDP_PORT").ok();
    Some(debug_browser_args(cdp_port(raw.as_deref())))
}

#[cfg(not(all(debug_assertions, windows)))]
fn compute_browser_args() -> Option<String> {
    None
}

/// wry's defaults plus the CDP port. Chromium binds it on 127.0.0.1 only.
#[cfg(all(debug_assertions, windows))]
fn debug_browser_args(port: u16) -> String {
    format!("{WRY_DEFAULT_BROWSER_ARGS} --remote-debugging-port={port}")
}

/// `FOURDA_CDP_PORT` when it is a usable port, else [`DEFAULT_CDP_PORT`] — so a
/// second debug app (or anything already on 9222) can take another port.
#[cfg(all(debug_assertions, windows))]
fn cdp_port(raw: Option<&str>) -> u16 {
    raw.and_then(|v| v.trim().parse::<u16>().ok())
        .filter(|&p| p != 0)
        .unwrap_or(DEFAULT_CDP_PORT)
}

/// Give every window declared in `tauri.conf.json` the shared arguments. They
/// are created by `Builder::build`, and `ensure_main_window` recreates `main`
/// from the same (now amended) config.
pub(crate) fn apply_to_config(config: &mut tauri::Config) {
    if let Some(args) = webview_browser_args() {
        for window in &mut config.app.windows {
            window.additional_browser_args = Some(args.to_string());
        }
    }
}

/// Give a window built in code the shared arguments.
pub(crate) fn apply_to_builder<R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'_, R, M>,
) -> WebviewWindowBuilder<'_, R, M> {
    match webview_browser_args() {
        Some(args) => builder.additional_browser_args(args),
        None => builder,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The guarantee that matters most: remote debugging exists only in a
    /// debug build on Windows. A release build (`cargo test --release`) and
    /// every other platform get no arguments at all.
    #[test]
    fn remote_debugging_only_in_debug_windows_builds() {
        let args = webview_browser_args();
        if cfg!(all(debug_assertions, windows)) {
            let args = args.expect("debug Windows builds pass browser args");
            assert!(args.contains("--remote-debugging-port="), "{args}");
            assert!(args.starts_with(WRY_DEFAULT_BROWSER_ARGS), "{args}");
        } else {
            assert_eq!(args, None, "release/non-Windows builds must not open CDP");
        }
    }

    #[test]
    fn every_window_gets_the_identical_string() {
        // Differing options on the shared user-data folder fail with 0x8007139F.
        let first = webview_browser_args();
        assert_eq!(first, webview_browser_args());
        let mut config = tauri::Config::default();
        config.app.windows = vec![Default::default(), Default::default()];
        apply_to_config(&mut config);
        for window in &config.app.windows {
            assert_eq!(window.additional_browser_args.as_deref(), first);
        }
    }

    #[cfg(all(debug_assertions, windows))]
    #[test]
    fn cdp_port_honours_a_valid_override_only() {
        assert_eq!(cdp_port(None), DEFAULT_CDP_PORT);
        assert_eq!(cdp_port(Some("9333")), 9333);
        assert_eq!(cdp_port(Some(" 9444 ")), 9444);
        assert_eq!(cdp_port(Some("0")), DEFAULT_CDP_PORT);
        assert_eq!(cdp_port(Some("70000")), DEFAULT_CDP_PORT);
        assert_eq!(cdp_port(Some("nope")), DEFAULT_CDP_PORT);
    }

    #[cfg(all(debug_assertions, windows))]
    #[test]
    fn debug_args_keep_wry_defaults_and_add_the_port() {
        assert_eq!(
            debug_browser_args(9333),
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
             --remote-debugging-port=9333"
        );
    }
}
