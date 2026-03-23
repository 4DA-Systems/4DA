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
const WINDOW_WIDTH: u32 = 440;
const WINDOW_HEIGHT: u32 = 160;

/// Margin from screen edges (pixels).
const MARGIN_RIGHT: i32 = 16;
/// Bottom margin to clear the OS taskbar.
const MARGIN_BOTTOM: i32 = 64;

// ============================================================================
// Auto-dismiss cancellation (module-level state)
// ============================================================================

static DISMISS_CANCEL: std::sync::LazyLock<std::sync::Mutex<Option<Arc<AtomicBool>>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Whether the notification window's JS listener is registered and ready.
static WINDOW_READY: AtomicBool = AtomicBool::new(false);

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
    /// Database ID of the top signal item (for deep-linking on click).
    #[serde(default)]
    pub item_id: Option<i64>,
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

/// Mark the notification window as ready (JS listener registered).
/// Called when the frontend emits `notification-ready`.
pub fn mark_ready() {
    WINDOW_READY.store(true, Ordering::Relaxed);
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
            let px = monitor_pos.x + (screen.width as i32)
                - ((WINDOW_WIDTH as f64 * scale) as i32)
                - ((MARGIN_RIGHT as f64 * scale) as i32);
            let py = monitor_pos.y + (screen.height as i32)
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
    // If the JS listener isn't ready yet, retry after a short delay.
    if !WINDOW_READY.load(Ordering::Relaxed) {
        info!(target: "4da::notify", "Window not ready, deferring notification by 500ms");
        let app_deferred = app.clone();
        let data_deferred = data.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if let Err(e) = app_deferred.emit_to(WINDOW_LABEL, "notification-data", &data_deferred)
            {
                warn!(target: "4da::notify", error = %e, "Deferred emit failed");
            }
        });
    } else if let Err(e) = app.emit_to(WINDOW_LABEL, "notification-data", &data) {
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
    let dismiss_ms = dismiss_duration_ms(&data.priority);

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
/// Optionally includes the item_id for deep-linking to the specific signal.
#[tauri::command]
pub async fn notification_clicked(app: AppHandle, item_id: Option<i64>) {
    hide_notification(&app);

    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.show();
        let _ = main_window.set_focus();
    }

    let payload = serde_json::json!({ "item_id": item_id });
    if let Err(e) = app.emit_to("main", "navigate-to-signals", &payload) {
        warn!(target: "4da::notify", error = %e, "Failed to emit navigate-to-signals");
    }

    info!(target: "4da::notify", item_id = ?item_id, "Notification clicked — navigating to signals");
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

/// Map priority string to auto-dismiss duration in milliseconds.
fn dismiss_duration_ms(priority: &str) -> u64 {
    match priority {
        "critical" => 8000,
        "high" => 6000,
        "medium" => 5000,
        _ => 4000,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_data_serialization_roundtrip() {
        let data = NotificationData {
            variant: "signal".to_string(),
            priority: "critical".to_string(),
            signal_type: Some("security_alert".to_string()),
            title: "CVE-2026-1234 in SQLite".to_string(),
            action: Some("Update dependency immediately".to_string()),
            source: Some("cve".to_string()),
            matched_deps: vec!["sqlite".to_string(), "rusqlite".to_string()],
            count: None,
            chain_sources: None,
            chain_phase: None,
            chain_links_filled: None,
            chain_links_total: None,
            time_ago: "just now".to_string(),
            item_id: Some(42),
        };

        let json = serde_json::to_string(&data).unwrap();
        let roundtrip: NotificationData = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.variant, "signal");
        assert_eq!(roundtrip.priority, "critical");
        assert_eq!(roundtrip.signal_type.as_deref(), Some("security_alert"));
        assert_eq!(roundtrip.matched_deps.len(), 2);
        assert_eq!(roundtrip.item_id, Some(42));
    }

    #[test]
    fn notification_data_minimal_fields() {
        // Only required fields — optional fields default correctly
        let json = r#"{"variant":"digest","priority":"low","title":"5 items","time_ago":"2m ago"}"#;
        let data: NotificationData = serde_json::from_str(json).unwrap();
        assert_eq!(data.variant, "digest");
        assert!(data.signal_type.is_none());
        assert!(data.matched_deps.is_empty());
        assert!(data.item_id.is_none());
    }

    #[test]
    fn dismiss_duration_by_priority() {
        assert_eq!(dismiss_duration_ms("critical"), 8000);
        assert_eq!(dismiss_duration_ms("high"), 6000);
        assert_eq!(dismiss_duration_ms("medium"), 5000);
        assert_eq!(dismiss_duration_ms("low"), 4000);
        assert_eq!(dismiss_duration_ms("unknown"), 4000);
        assert_eq!(dismiss_duration_ms(""), 4000);
    }

    #[test]
    fn cancel_dismiss_timer_sets_flag() {
        // Set up a flag
        let flag = Arc::new(AtomicBool::new(false));
        {
            let mut guard = DISMISS_CANCEL.lock().unwrap();
            *guard = Some(Arc::clone(&flag));
        }

        // Cancel should set the flag to true
        cancel_dismiss_timer();
        assert!(flag.load(Ordering::Relaxed));

        // Guard should now be None
        let guard = DISMISS_CANCEL.lock().unwrap();
        assert!(guard.is_none());
    }

    #[test]
    fn cancel_dismiss_timer_no_op_when_empty() {
        // Ensure guard is empty
        {
            let mut guard = DISMISS_CANCEL.lock().unwrap();
            *guard = None;
        }

        // Should not panic
        cancel_dismiss_timer();

        let guard = DISMISS_CANCEL.lock().unwrap();
        assert!(guard.is_none());
    }

    #[test]
    fn chain_notification_data() {
        let data = NotificationData {
            variant: "chain".to_string(),
            priority: "critical".to_string(),
            signal_type: None,
            title: "lodash vulnerability".to_string(),
            action: Some("Act within ~8h".to_string()),
            source: None,
            matched_deps: vec![],
            count: None,
            chain_sources: Some(vec![
                "hackernews".to_string(),
                "reddit".to_string(),
                "github".to_string(),
            ]),
            chain_phase: Some("escalating".to_string()),
            chain_links_filled: Some(3),
            chain_links_total: Some(4),
            time_ago: "just now".to_string(),
            item_id: None,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("escalating"));
        assert!(json.contains("hackernews"));
        assert!(json.contains("chain_links_filled"));
    }
}
