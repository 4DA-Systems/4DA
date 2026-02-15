//! Continuous Monitoring Module for 4DA
//!
//! Provides background analysis scheduling, system tray integration,
//! and native OS notifications for new relevant items.

// parking_lot::Mutex available if needed for state management
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder},
    AppHandle, Emitter, Manager, Runtime,
};
use tauri_plugin_notification::NotificationExt;
use tracing::{info, warn};

// ============================================================================
// Monitoring State
// ============================================================================

/// A notification that was below the quality threshold and batched for the next briefing
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields populated for diagnostic use
pub struct BatchedNotification {
    pub title: String,
    pub source_type: String,
    pub score: f32,
    pub signal_priority: Option<String>,
}

/// Global monitoring state
pub struct MonitoringState {
    /// Whether monitoring is enabled
    pub enabled: AtomicBool,
    /// Interval between checks in seconds
    pub interval_secs: AtomicU64,
    /// Whether a check is currently running
    pub is_checking: AtomicBool,
    /// Last check timestamp (unix seconds)
    pub last_check: AtomicU64,
    /// Number of new relevant items found in last check
    pub last_relevant_count: AtomicU64,
    /// Total checks performed
    pub total_checks: AtomicU64,
    /// Last health check timestamp (unix seconds)
    pub last_health_check: AtomicU64,
    /// Last anomaly detection timestamp (unix seconds)
    pub last_anomaly_check: AtomicU64,
    /// Last behavior decay timestamp (unix seconds)
    pub last_decay: AtomicU64,
    /// Items below notification threshold, batched for next briefing
    pub batched_items: parking_lot::Mutex<Vec<BatchedNotification>>,
}

impl Default for MonitoringState {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            interval_secs: AtomicU64::new(1800), // 30 minutes default
            is_checking: AtomicBool::new(false),
            last_check: AtomicU64::new(0),
            last_relevant_count: AtomicU64::new(0),
            total_checks: AtomicU64::new(0),
            last_health_check: AtomicU64::new(0),
            last_anomaly_check: AtomicU64::new(0),
            last_decay: AtomicU64::new(0),
            batched_items: parking_lot::Mutex::new(Vec::new()),
        }
    }
}

impl MonitoringState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn get_interval(&self) -> u64 {
        self.interval_secs.load(Ordering::Relaxed)
    }

    pub fn set_interval(&self, secs: u64) {
        self.interval_secs.store(secs, Ordering::Relaxed);
    }
}

// ============================================================================
// System Tray
// ============================================================================

/// Set up the system tray with menu
pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> Result<TrayIcon<R>, String> {
    // Create menu items
    let show_item = MenuItem::with_id(app, "show", "Show 4DA Home", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let analyze_item = MenuItem::with_id(app, "analyze", "Analyze Now", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let separator = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let monitoring_item = MenuItem::with_id(
        app,
        "toggle_monitoring",
        "Start Monitoring",
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let separator2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let quit_item =
        MenuItem::with_id(app, "quit", "Quit", true, None::<&str>).map_err(|e| e.to_string())?;

    // Build menu
    let menu = Menu::with_items(
        app,
        &[
            &show_item,
            &analyze_item,
            &separator,
            &monitoring_item,
            &separator2,
            &quit_item,
        ],
    )
    .map_err(|e| e.to_string())?;

    // Build tray icon
    let tray = TrayIconBuilder::new()
        .icon(
            app.default_window_icon()
                .ok_or_else(|| "No default window icon configured".to_string())?
                .clone(),
        )
        .menu(&menu)
        .tooltip("4DA Home - All signal. No feed.")
        .on_menu_event(move |app, event| {
            match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "analyze" => {
                    // Emit event to trigger analysis
                    let _ = app.emit("tray-analyze", ());
                }
                "toggle_monitoring" => {
                    // Emit event to toggle monitoring
                    let _ = app.emit("tray-toggle-monitoring", ());
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)
        .map_err(|e| e.to_string())?;

    info!(target: "4da::tray", "System tray initialized");
    Ok(tray)
}

// ============================================================================
// Background Scheduler
// ============================================================================

// Background job intervals (in seconds)
const HEALTH_CHECK_INTERVAL: u64 = 300; // 5 minutes
const ANOMALY_CHECK_INTERVAL: u64 = 3600; // 1 hour
const BEHAVIOR_DECAY_INTERVAL: u64 = 86400; // 24 hours (daily)

/// Start the background monitoring scheduler
pub fn start_scheduler<R: Runtime>(app: AppHandle<R>, state: Arc<MonitoringState>) {
    info!(target: "4da::monitor", "Starting background scheduler");
    info!(target: "4da::monitor",
        health_interval_min = HEALTH_CHECK_INTERVAL / 60,
        anomaly_interval_hr = ANOMALY_CHECK_INTERVAL / 3600,
        decay_interval_hr = BEHAVIOR_DECAY_INTERVAL / 3600,
        "Background job intervals configured"
    );

    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60)); // Check every minute

        loop {
            interval.tick().await;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            // ================================================================
            // Background Jobs (run regardless of monitoring enabled state)
            // ================================================================

            // Health check - every 5 minutes
            let last_health = state.last_health_check.load(Ordering::Relaxed);
            if now - last_health >= HEALTH_CHECK_INTERVAL {
                state.last_health_check.store(now, Ordering::Relaxed);
                match crate::run_background_health_check().await {
                    Ok(result) => {
                        info!(target: "4da::monitor", result = %result, "Health check completed");
                    }
                    Err(e) => {
                        warn!(target: "4da::monitor", error = %e, "Health check failed");
                    }
                }
            }

            // Anomaly detection - every hour
            let last_anomaly = state.last_anomaly_check.load(Ordering::Relaxed);
            if now - last_anomaly >= ANOMALY_CHECK_INTERVAL {
                state.last_anomaly_check.store(now, Ordering::Relaxed);
                match crate::run_background_anomaly_detection().await {
                    Ok(result) => {
                        info!(target: "4da::monitor", result = %result, "Anomaly detection completed");
                    }
                    Err(e) => {
                        warn!(target: "4da::monitor", error = %e, "Anomaly detection failed");
                    }
                }
            }

            // Behavior decay - daily
            let last_decay = state.last_decay.load(Ordering::Relaxed);
            if now - last_decay >= BEHAVIOR_DECAY_INTERVAL {
                state.last_decay.store(now, Ordering::Relaxed);
                match crate::run_background_behavior_decay().await {
                    Ok(result) => {
                        info!(target: "4da::monitor", result = %result, "Behavior decay completed");
                    }
                    Err(e) => {
                        warn!(target: "4da::monitor", error = %e, "Behavior decay failed");
                    }
                }
            }

            // ================================================================
            // Scheduled Analysis (only when monitoring is enabled)
            // ================================================================

            // Check if monitoring is enabled for scheduled analysis
            if !state.is_enabled() {
                continue;
            }

            // Check if enough time has passed since last check
            let last = state.last_check.load(Ordering::Relaxed);
            let interval_secs = state.get_interval();

            if now - last < interval_secs {
                continue;
            }

            // Check if already running
            if state.is_checking.swap(true, Ordering::SeqCst) {
                continue;
            }

            info!(target: "4da::monitor", "Running background analysis...");
            state.last_check.store(now, Ordering::Relaxed);
            state.total_checks.fetch_add(1, Ordering::Relaxed);

            // Run analysis directly (silently, with differential scoring)
            let bg_app = app.clone();
            let bg_state = state.clone();
            tauri::async_runtime::spawn(async move {
                crate::analysis::run_background_analysis(&bg_app, &bg_state).await;
            });
        }
    });
}

/// Signal summary extracted from analysis results
pub struct SignalSummary {
    pub critical_count: usize,
    pub high_count: usize,
    pub top_signal: Option<(String, String)>, // (signal_type, action)
}

/// Called when a scheduled analysis completes
pub fn complete_scheduled_check<R: Runtime>(
    app: &AppHandle<R>,
    state: &MonitoringState,
    new_relevant_count: usize,
    total_count: usize,
    signal_summary: Option<SignalSummary>,
) {
    state.is_checking.store(false, Ordering::Relaxed);
    state
        .last_relevant_count
        .store(new_relevant_count as u64, Ordering::Relaxed);

    // Read notification threshold from settings
    let threshold = {
        let settings = crate::get_settings_manager().lock();
        settings.get().monitoring.notification_threshold.clone()
    };

    // Send signal-aware notifications respecting the threshold
    match threshold.as_str() {
        "critical_only" => {
            // Only notify on critical signals
            if let Some(ref summary) = signal_summary {
                if summary.critical_count > 0 {
                    send_signal_notification(app, "critical", summary);
                } else if new_relevant_count > 0 {
                    // Batch everything below critical
                    batch_generic_items(state, new_relevant_count, &signal_summary);
                }
            } else if new_relevant_count > 0 {
                batch_generic_items(state, new_relevant_count, &signal_summary);
            }
        }
        "all" => {
            // Legacy behavior: always notify
            if let Some(ref summary) = signal_summary {
                if summary.critical_count > 0 {
                    send_signal_notification(app, "critical", summary);
                } else if summary.high_count > 0 {
                    send_signal_notification(app, "high", summary);
                } else if new_relevant_count > 0 {
                    send_notification(app, new_relevant_count, total_count);
                }
            } else if new_relevant_count > 0 {
                send_notification(app, new_relevant_count, total_count);
            }
        }
        _ => {
            // "high_and_above" (default): notify on critical + high, batch regular items
            if let Some(ref summary) = signal_summary {
                if summary.critical_count > 0 {
                    send_signal_notification(app, "critical", summary);
                } else if summary.high_count > 0 {
                    send_signal_notification(app, "high", summary);
                } else if new_relevant_count > 0 {
                    // Regular relevant items get batched instead of notified
                    batch_generic_items(state, new_relevant_count, &signal_summary);
                }
            } else if new_relevant_count > 0 {
                batch_generic_items(state, new_relevant_count, &signal_summary);
            }
        }
    }

    info!(
        target: "4da::monitor",
        relevant = new_relevant_count,
        total = total_count,
        threshold = %threshold,
        critical = signal_summary.as_ref().map(|s| s.critical_count).unwrap_or(0),
        high = signal_summary.as_ref().map(|s| s.high_count).unwrap_or(0),
        "Scheduled check complete"
    );
}

/// Batch generic relevant items that don't meet the notification threshold
fn batch_generic_items(
    state: &MonitoringState,
    count: usize,
    _signal_summary: &Option<SignalSummary>,
) {
    let mut guard = state.batched_items.lock();
    // Push a summary entry for the batch of items
    guard.push(BatchedNotification {
        title: format!("{} new relevant items", count),
        source_type: "mixed".to_string(),
        score: 0.0,
        signal_priority: None,
    });
    info!(
        target: "4da::monitor",
        count = count,
        batched_total = guard.len(),
        "Items batched for next briefing (below notification threshold)"
    );
}

/// Drain all batched notifications, returning them and clearing the buffer
pub fn drain_batched_notifications(state: &MonitoringState) -> Vec<BatchedNotification> {
    let mut guard = state.batched_items.lock();
    std::mem::take(&mut *guard)
}

// ============================================================================
// Notifications
// ============================================================================

/// Send a notification about new relevant items
pub fn send_notification<R: Runtime>(
    app: &AppHandle<R>,
    relevant_count: usize,
    _total_count: usize,
) {
    let title = "4DA - New Relevant Items";
    let body = if relevant_count == 1 {
        "1 new item matches your interests".to_string()
    } else {
        format!("{} new items match your interests", relevant_count)
    };

    if let Err(e) = app.notification().builder().title(title).body(&body).show() {
        warn!(target: "4da::notify", error = %e, "Failed to send notification");
    } else {
        info!(target: "4da::notify", body = %body, "Sent notification");
    }
}

/// Send a signal-aware notification for critical/high priority signals
pub fn send_signal_notification<R: Runtime>(
    app: &AppHandle<R>,
    priority: &str,
    summary: &SignalSummary,
) {
    let (title, body) = match priority {
        "critical" => {
            let title = format!(
                "4DA - {} Critical Signal{}",
                summary.critical_count,
                if summary.critical_count > 1 { "s" } else { "" }
            );
            let body = if let Some((ref sig_type, ref action)) = summary.top_signal {
                let label = match sig_type.as_str() {
                    "security_alert" => "Security Alert",
                    "breaking_change" => "Breaking Change",
                    _ => "Alert",
                };
                format!("{}: {}", label, action)
            } else {
                format!(
                    "{} critical items need your attention",
                    summary.critical_count
                )
            };
            (title, body)
        }
        "high" => {
            let title = format!(
                "4DA - {} High Priority Signal{}",
                summary.high_count,
                if summary.high_count > 1 { "s" } else { "" }
            );
            let body = if let Some((_, ref action)) = summary.top_signal {
                action.clone()
            } else {
                format!("{} high priority items found", summary.high_count)
            };
            (title, body)
        }
        _ => (
            "4DA - New Signals".to_string(),
            "New actionable signals detected".to_string(),
        ),
    };

    if let Err(e) = app
        .notification()
        .builder()
        .title(&title)
        .body(&body)
        .show()
    {
        warn!(target: "4da::notify", error = %e, "Failed to send signal notification");
    } else {
        info!(target: "4da::notify", priority = priority, body = %body, "Sent signal notification");
    }
}

// ============================================================================
// Tray Menu Updates
// ============================================================================

/// Update the tray menu text based on monitoring state
#[allow(dead_code)] // Future: dynamic tray menu updates
pub fn update_tray_menu<R: Runtime>(
    tray: &TrayIcon<R>,
    app: &AppHandle<R>,
    monitoring_enabled: bool,
) -> Result<(), String> {
    let show_item = MenuItem::with_id(app, "show", "Show 4DA Home", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let analyze_item = MenuItem::with_id(app, "analyze", "Analyze Now", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let separator = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;

    let monitoring_text = if monitoring_enabled {
        "Stop Monitoring"
    } else {
        "Start Monitoring"
    };
    let monitoring_item = MenuItem::with_id(
        app,
        "toggle_monitoring",
        monitoring_text,
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;

    let separator2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let quit_item =
        MenuItem::with_id(app, "quit", "Quit", true, None::<&str>).map_err(|e| e.to_string())?;

    let menu = Menu::with_items(
        app,
        &[
            &show_item,
            &analyze_item,
            &separator,
            &monitoring_item,
            &separator2,
            &quit_item,
        ],
    )
    .map_err(|e| e.to_string())?;

    tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;

    // Update tooltip
    let tooltip = if monitoring_enabled {
        "4DA Home - Monitoring active"
    } else {
        "4DA Home - All signal. No feed."
    };
    tray.set_tooltip(Some(tooltip)).map_err(|e| e.to_string())?;

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitoring_state_default() {
        let state = MonitoringState::default();
        assert!(!state.is_enabled());
        assert_eq!(state.get_interval(), 1800);
    }

    #[test]
    fn test_monitoring_state_toggle() {
        let state = MonitoringState::new();
        assert!(!state.is_enabled());
        state.set_enabled(true);
        assert!(state.is_enabled());
        state.set_enabled(false);
        assert!(!state.is_enabled());
    }
}
