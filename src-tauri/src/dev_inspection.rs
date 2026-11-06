// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! The two programmatic inspection channels of a DEBUG build: the WebView2
//! DevTools Protocol (CDP) port and the Victauri MCP server's port.
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

#[cfg(debug_assertions)]
use std::net::{Ipv4Addr, TcpListener};
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

// ============================================================================
// Victauri port
// ============================================================================

/// The Victauri plugin's preferred port and the size of its fallback range.
#[cfg(debug_assertions)]
const VICTAURI_DEFAULT_PORT: u16 = 7373;
#[cfg(debug_assertions)]
const VICTAURI_FALLBACK_RANGE: u16 = 10;

/// A port for the Victauri server when its whole 7373-7383 range is unusable.
///
/// Windows reserves dynamic 100-port blocks for Hyper-V/WinNAT (`netsh int
/// ipv4 show excludedportrange protocol=tcp`), different on every boot; when
/// one covers 7373-7383 every bind fails with `os error 10013` and victauri-plugin
/// 0.10.3 gives up. Then this hands it a port the OS assigns; clients find it in
/// the discovery directory as usual. `None` (use the plugin's own choice) when
/// `VICTAURI_PORT` is set or any port in the range binds.
///
/// Remove once 4DA depends on a victauri-plugin release that falls back to an
/// OS-assigned port itself.
#[cfg(debug_assertions)]
pub(crate) fn victauri_fallback_port() -> Option<u16> {
    if std::env::var_os("VICTAURI_PORT").is_some() {
        return None;
    }
    let chosen = choose_victauri_port(can_bind, os_assigned_port);
    if let Some(port) = chosen {
        tracing::warn!(
            target: "4da::victauri",
            port,
            "Ports {}-{} are unavailable (reserved?); Victauri will use OS-assigned port {}",
            VICTAURI_DEFAULT_PORT,
            VICTAURI_DEFAULT_PORT + VICTAURI_FALLBACK_RANGE,
            port
        );
    }
    chosen
}

#[cfg(debug_assertions)]
fn choose_victauri_port(
    bindable: impl Fn(u16) -> bool,
    os_port: impl FnOnce() -> Option<u16>,
) -> Option<u16> {
    let range = VICTAURI_DEFAULT_PORT..=VICTAURI_DEFAULT_PORT + VICTAURI_FALLBACK_RANGE;
    if range.into_iter().any(bindable) {
        return None;
    }
    os_port()
}

#[cfg(debug_assertions)]
fn can_bind(port: u16) -> bool {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok()
}

#[cfg(debug_assertions)]
fn os_assigned_port() -> Option<u16> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
    listener.local_addr().ok().map(|a| a.port())
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

    #[cfg(debug_assertions)]
    #[test]
    fn victauri_keeps_its_own_choice_while_any_range_port_binds() {
        let only_7380 = |p: u16| p == 7380;
        assert_eq!(choose_victauri_port(only_7380, || Some(50_123)), None);
        assert_eq!(choose_victauri_port(|_| true, || Some(50_123)), None);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn victauri_gets_an_os_port_when_the_whole_range_is_reserved() {
        let checked = std::cell::RefCell::new(Vec::new());
        let none_bind = |p: u16| {
            checked.borrow_mut().push(p);
            false
        };
        assert_eq!(
            choose_victauri_port(none_bind, || Some(50_123)),
            Some(50_123)
        );
        assert_eq!(*checked.borrow(), (7373..=7383).collect::<Vec<_>>());
        assert_eq!(choose_victauri_port(|_| false, || None), None);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn os_assigned_port_is_a_real_bindable_port() {
        let port = os_assigned_port().expect("loopback bind");
        assert_ne!(port, 0);
        assert!(can_bind(port));
    }
}
