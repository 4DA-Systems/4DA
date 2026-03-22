// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Custom notification window system for 4DA.
//!
//! Replaces OS native notifications with a custom transparent window
//! featuring GAME shader atmospheres for visual priority signaling.
//! The window is pre-created on app startup and reused for each notification.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewUrl};
use tracing::{info, warn};

/// Label used for the notification window throughout the module.
const WINDOW_LABEL: &str = "notification";

/// Notification window dimensions (logical pixels).
const WINDOW_WIDTH: u32 = 416;
const WINDOW_HEIGHT: u32 = 146;

/// Margin from screen edges (pixels).
const MARGIN_RIGHT: i32 = 16;
/// Bottom margin to clear the OS taskbar.
const MARGIN_BOTTOM: i32 = 64;

// ============================================================================
// Auto-dismiss cancellation (module-level state)
// ============================================================================

static DISMISS_CANCEL: once_cell::sync::Lazy<std::sync::Mutex<Option<Arc<AtomicBool>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(None));

// ============================================================================
// Data Types
// ============================================================================

/// Payload emitted to the notification window for rendering.
///
/// The `variant` field selects the visual layout and GAME shader atmosphere.
/// All fields are optional beyond `variant`, `priority`, `title`, and `time_ago`
/// to accommodate the different notification shapes (single signal, chain,
/// multi-item digest, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationData {
    /// Visual variant: "signal", "chain", "multi", "digest", "briefing".
    pub variant: String,
    /// Priority level: "critical", "high", "medium", "low".
    pub priority: String,
    /// Signal classification (e.g. "security_alert", "breaking_change").
    pub signal_type: Option<String>,
    /// Main content title.
    pub title: String,
    /// Suggested action text.
    pub action: Option<String>,
    /// Source type identifier (hackernews, reddit, cve, etc.).
    pub source: Option<String>,
    /// Dependency names matched by this notification.
    #[serde(default)]
    pub matched_deps: Vec<String>,
    /// Item count for multi/digest variants.
    pub count: Option<usize>,
    /// Source progression for chain variant.
    pub chain_sources: Option<Vec<String>>,
    /// Chain phase: "nascent", "active", "escalating", "peak".
    pub chain_phase: Option<String>,
    /// How many chain links are filled.
    pub chain_links_filled: Option<usize>,
    /// Total chain links.
    pub chain_links_total: Option<usize>,
    /// Relative time string (e.g. "2m ago").
    pub time_ago: String,
}

// ============================================================================
// Window Lifecycle
// ============================================================================

/// Pre-create the notification window in a hidden state.
///
/// Called once during app startup so the window is warm and can be shown
/// instantly when a notification arrives. The window is transparent,
/// borderless, always-on-top, and excluded from the taskbar.
pub fn init_notification_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let _window = tauri::webview::WebviewWindowBuilder::new(
        app,
        WINDOW_LABEL,
        WebviewUrl::App("notification.html".into()),
    )
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .resizable(false)
    .visible(false)
    .inner_size(f64::from(WINDOW_WIDTH), f64::from(WINDOW_HEIGHT))
    .build()?;

    info!(target: "4da::notify", "Notification window pre-warmed");
    Ok(())
}

/// Show the notification window with the given data payload.
///
/// Positions the window in the bottom-right corner of the primary monitor,
/// emits the data payload to the frontend, and starts an auto-dismiss timer.
/// If the window has not been created yet it will be initialised on the fly.
pub fn show_notification<R: Runtime>(app: &AppHandle<R>, data: NotificationData) {
    // Cancel any existing dismiss timer before doing anything else.
    cancel_dismiss_timer();

    // Ensure the window exists.
    let window = match app.get_webview_window(WINDOW_LABEL) {
        Some(w) => w,
        None => {
            if let Err(e) = init_notification_window(app) {
                warn!(target: "4da::notify", error = %e, "Failed to create notification window");
                return;
            }
            match app.get_webview_window(WINDOW_LABEL) {
                Some(w) => w,
                None => {
                    warn!(target: "4da::notify", "Notification window missing after init");
                    return;
                }
            }
        }
    };

    // Position bottom-right, accounting for DPI scale factor.
    let positioned = (|| -> tauri::Result<()> {
        let monitor = window
            .primary_monitor()?
            .or_else(|| window.available_monitors().ok()?.into_iter().next());

        if let Some(monitor) = monitor {
            let scale = monitor.scale_factor();
            let screen = monitor.size();
            let monitor_pos = monitor.position();

            // Physical pixel coordinates for the window origin.
            let px = monitor_pos.x
                + (screen.width as i32)
                - ((WINDOW_WIDTH as f64 * scale) as i32)
                - ((MARGIN_RIGHT as f64 * scale) as i32);
            let py = monitor_pos.y
                + (screen.height as i32)
                - ((WINDOW_HEIGHT as f64 * scale) as i32)
                - ((MARGIN_BOTTOM as f64 * scale) as i32);

            window.set_position(PhysicalPosition::new(px, py))?;
        } else {
            warn!(target: "4da::notify", "No monitor detected — centering notification window");
            window.center()?;
        }
        Ok(())
    })();

    if let Err(e) = positioned {
        warn!(target: "4da::notify", error = %e, "Failed to position notification window, using default");
    }

    // Emit data to the notification webview.
    if let Err(e) = app.emit_to(WINDOW_LABEL, "notification-data", &data) {
        warn!(target: "4da::notify", error = %e, "Failed to emit notification data");
    }

    // Show the window.
    if let Err(e) = window.show() {
        warn!(target: "4da::notify", error = %e, "Failed to show notification window");
        return;
    }

    info!(
        target: "4da::notify",
        variant = %data.variant,
        priority = %data.priority,
        "Custom notification shown"
    );

    // Start auto-dismiss timer.
    let dismiss_ms = match data.priority.as_str() {
        "critical" => 8000u64,
        "high" => 6000,
        "medium" => 5000,
        _ => 4000,
    };

    let cancelled = Arc::new(AtomicBool::new(false));
    {
        if let Ok(mut guard) = DISMISS_CANCEL.lock() {
            *guard = Some(Arc::clone(&cancelled));
        }
    }

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(dismiss_ms)).await;
        if !cancelled.load(Ordering::Relaxed) {
            hide_notification(&app_handle);
        }
    });
}

/// Hide the notification window (no-op if the window does not exist).
pub fn hide_notification<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.hide();
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Called from the notification frontend when the user clicks the notification.
///
/// Hides the notification, brings the main window to focus, and emits a
/// navigation event so the main window can switch to the signals view.
#[tauri::command]
pub async fn notification_clicked(app: AppHandle) {
    hide_notification(&app);

    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.show();
        let _ = main_window.set_focus();
    }

    if let Err(e) = app.emit_to("main", "navigate-to-signals", ()) {
        warn!(target: "4da::notify", error = %e, "Failed to emit navigate-to-signals");
    }

    info!(target: "4da::notify", "Notification clicked — navigating to signals");
}

// ============================================================================
// Internal Helpers
// ============================================================================

/// Cancel any in-flight dismiss timer so a fresh notification gets a full
/// display duration.
fn cancel_dismiss_timer() {
    if let Ok(mut guard) = DISMISS_CANCEL.lock() {
        if let Some(flag) = guard.take() {
            flag.store(true, Ordering::Relaxed);
        }
    }
}
