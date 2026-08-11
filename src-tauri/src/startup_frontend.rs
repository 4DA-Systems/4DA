// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Frontend first-light gate for the hidden main window.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{Listener, Manager};
use tracing::{debug, info, warn};

static FRONTEND_READY: AtomicBool = AtomicBool::new(false);
static MAIN_WINDOW_SHOWN: AtomicBool = AtomicBool::new(false);

/// Start the window visibility/recovery gate as soon as Tauri gives us an
/// AppHandle. This intentionally runs before background warmup so live
/// dogfood tooling cannot reach Victauri while the webview is still hidden
/// or caught in recovery navigation.
pub(crate) fn start_frontend_readiness_gate(app_handle: tauri::AppHandle) {
    #[cfg(not(debug_assertions))]
    {
        mark_frontend_ready_inner(&app_handle, "production frontend");
    }

    #[cfg(debug_assertions)]
    start_debug_frontend_readiness_gate(app_handle);
}

pub(crate) async fn wait_until_frontend_ready(timeout: std::time::Duration) -> bool {
    let began = std::time::Instant::now();
    while began.elapsed() < timeout {
        if FRONTEND_READY.load(Ordering::SeqCst) {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    FRONTEND_READY.load(Ordering::SeqCst)
}

pub(crate) fn background_grace_after_first_light() -> std::time::Duration {
    if cfg!(debug_assertions) {
        if victauri_e2e_active() {
            return std::time::Duration::from_mins(5);
        }
        // Dev/Victauri boots pay Vite + Tailwind first-request compilation costs.
        // Keep CPU-heavy Rust warmups out of that window.
        std::time::Duration::from_secs(90)
    } else {
        std::time::Duration::from_secs(20)
    }
}

pub(crate) fn heavy_startup_work_grace_after_first_light() -> std::time::Duration {
    if cfg!(debug_assertions) {
        if victauri_e2e_active() {
            // Live dogfood should verify the shell, IPC, and webview while the
            // process is idle. Heavy scans can still be triggered explicitly by
            // tests that own that scenario.
            std::time::Duration::from_mins(5)
        } else {
            // Normal dev boots should remain useful, but not start a project
            // tree walk while Vite, WebView2, and first-mount IPC are settling.
            std::time::Duration::from_mins(3)
        }
    } else {
        std::time::Duration::from_secs(20)
    }
}

pub(crate) fn victauri_e2e_active() -> bool {
    std::env::var_os("VICTAURI_E2E").is_some()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupRuntimeFlags {
    victauri_e2e: bool,
    debug_build: bool,
    background_grace_secs: u64,
    heavy_startup_grace_secs: u64,
}

/// Explicit frontend readiness command. The event path is still supported,
/// but this gives the module graph a typed IPC path that Victauri can observe
/// and does not depend on injected-JS globals.
#[tauri::command]
pub async fn mark_frontend_ready(app_handle: tauri::AppHandle) -> Result<(), String> {
    mark_frontend_ready_inner(&app_handle, "frontend-ready command");
    Ok(())
}

#[tauri::command]
pub async fn get_startup_runtime_flags() -> StartupRuntimeFlags {
    StartupRuntimeFlags {
        victauri_e2e: victauri_e2e_active(),
        debug_build: cfg!(debug_assertions),
        background_grace_secs: background_grace_after_first_light().as_secs(),
        heavy_startup_grace_secs: heavy_startup_work_grace_after_first_light().as_secs(),
    }
}

fn mark_frontend_ready_inner(app_handle: &tauri::AppHandle, source: &'static str) {
    FRONTEND_READY.store(true, Ordering::SeqCst);
    show_main_window_once(app_handle, source);
}

fn show_main_window_once(app_handle: &tauri::AppHandle, source: &'static str) {
    if MAIN_WINDOW_SHOWN.swap(true, Ordering::SeqCst) {
        return;
    }

    show_main_window(app_handle);
    info!(target: "4da::startup", source, "Main window shown");
    crate::startup_watchdog::mark_phase0_complete();
}

fn show_main_window_fallback(app_handle: &tauri::AppHandle) {
    if MAIN_WINDOW_SHOWN.swap(true, Ordering::SeqCst) {
        return;
    }

    show_main_window(app_handle);
    warn!(target: "4da::startup", "Main window shown (dev server poll fallback)");
    crate::startup_watchdog::mark_phase0_complete();
}

fn show_main_window(app_handle: &tauri::AppHandle) {
    if let Some(w) = app_handle.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg(debug_assertions)]
fn start_debug_frontend_readiness_gate(app_handle: tauri::AppHandle) {
    {
        let show_handle = app_handle.clone();
        app_handle.listen("frontend-ready", move |_| {
            mark_frontend_ready_inner(&show_handle, "frontend-ready event");
        });
    }

    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .no_proxy()
            .build()
            .unwrap_or_default();

        let dev_url = app_handle
            .config()
            .build
            .dev_url
            .clone()
            .unwrap_or_else(|| {
                url::Url::parse("http://localhost:4444/").expect("hardcoded dev URL is valid")
            });

        for attempt in 1..=60u32 {
            if MAIN_WINDOW_SHOWN.load(Ordering::SeqCst) || FRONTEND_READY.load(Ordering::SeqCst) {
                return;
            }

            if let Ok(r) = client.get(dev_url.as_str()).send().await {
                if r.status().is_success() {
                    info!(target: "4da::startup", attempt, "Dev server ready - navigating webview");
                    if let Some(w) = app_handle.get_webview_window("main") {
                        let _ = w.navigate(dev_url.clone());
                    }

                    for _ in 0..24u32 {
                        if FRONTEND_READY.load(Ordering::SeqCst) {
                            return;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    }
                    break;
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        show_main_window_fallback(&app_handle);
        run_bounded_dev_recovery(app_handle, client, dev_url).await;
    });
}

#[cfg(debug_assertions)]
async fn run_bounded_dev_recovery(
    app_handle: tauri::AppHandle,
    client: reqwest::Client,
    dev_url: url::Url,
) {
    let recovery_began = std::time::Instant::now();
    let recovery_max = std::time::Duration::from_mins(2);
    let max_navigates = 4_u32;
    let mut consecutive_navigates = 0_u32;
    let mut backoff = std::time::Duration::from_secs(3);

    while recovery_began.elapsed() < recovery_max && consecutive_navigates < max_navigates {
        tokio::time::sleep(backoff).await;
        if FRONTEND_READY.load(Ordering::SeqCst) {
            debug!(target: "4da::startup", "Recovery loop: frontend-ready fired, exiting");
            break;
        }

        if let Some(w) = app_handle.get_webview_window("main") {
            let _ = w.eval(
                r"
                (() => {
                  if ((document.querySelector('#root')?.childElementCount ?? 0) === 0) return;
                  const invoke = window.__TAURI__?.core?.invoke || window.__TAURI_INTERNALS__?.invoke;
                  if (typeof invoke === 'function') {
                    Promise.resolve(invoke('mark_frontend_ready', {})).catch(() => {});
                  }
                })();
                ",
            );
        }

        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        if FRONTEND_READY.load(Ordering::SeqCst) {
            debug!(target: "4da::startup", "Recovery loop: page already loaded, exiting");
            break;
        }

        let server_up = client
            .get(dev_url.as_str())
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        if server_up {
            consecutive_navigates += 1;
            warn!(
                target: "4da::startup",
                consecutive_navigates,
                "Recovery: dev server reachable but frontend not ready - re-navigating webview"
            );
            if let Some(w) = app_handle.get_webview_window("main") {
                let _ = w.navigate(dev_url.clone());
            }
            backoff = (backoff * 2).min(std::time::Duration::from_secs(24));
        }
    }

    if consecutive_navigates >= max_navigates {
        warn!(
            target: "4da::startup",
            max_navigates,
            "Recovery: gave up re-navigating - frontend likely broken"
        );
    }

    debug!(target: "4da::startup", "Recovery loop exited");
}
