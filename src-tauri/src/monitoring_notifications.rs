//! Notification subsystem for 4DA monitoring.
//!
//! Handles OS notification dispatch, signal-aware notification routing,
//! and batching of below-threshold items for digest briefings.

use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;
use tracing::{info, warn};

use crate::monitoring::{BatchedNotification, MonitoringState};

// ============================================================================
// Signal Summary
// ============================================================================

/// Signal summary extracted from analysis results
pub struct SignalSummary {
    pub critical_count: usize,
    pub high_count: usize,
    pub top_signal: Option<(String, String)>, // (signal_type, action)
}

// ============================================================================
// Scheduled Check Completion
// ============================================================================

/// Called when a scheduled analysis completes
pub fn complete_scheduled_check<R: Runtime>(
    app: &AppHandle<R>,
    state: &MonitoringState,
    new_relevant_count: usize,
    total_count: usize,
    signal_summary: Option<SignalSummary>,
) {
    use std::sync::atomic::Ordering;

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

// ============================================================================
// Batching
// ============================================================================

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
// Notification Dispatch
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
