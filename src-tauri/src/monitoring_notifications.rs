//! Notification subsystem for 4DA monitoring.
//!
//! Handles OS notification dispatch, signal-aware notification routing,
//! and batching of below-threshold items for digest briefings.

use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;
use tracing::{info, warn};

use crate::monitoring::{BatchedNotification, MonitoringState};

// Re-export briefing types and functions so existing `monitoring_notifications::X` paths still work
#[allow(unused_imports)]
pub use crate::monitoring_briefing::{
    check_morning_briefing, generate_briefing_text, send_morning_briefing_notification,
    BriefingItem, BriefingNotification,
};

// ============================================================================
// Helpers
// ============================================================================

/// Truncate a string to at most `max_bytes` bytes without splitting a
/// multi-byte UTF-8 character (which would panic on slice).
pub(crate) fn truncate_safe(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

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

/// Maximum security-alert notifications per scan cycle.
/// If more alerts are found, a single summary notification is sent instead.
const MAX_SECURITY_NOTIFICATIONS_PER_CYCLE: usize = 5;

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

    // Cap security-alert notifications: if too many, send summary instead
    if let Some(ref summary) = signal_summary {
        let total_security = summary.critical_count + summary.high_count;
        if total_security > MAX_SECURITY_NOTIFICATIONS_PER_CYCLE {
            let title = format!("4DA — {} Security Alerts Detected", total_security);
            let body = format!(
                "{} critical, {} high priority alerts found. Open 4DA for details.",
                summary.critical_count, summary.high_count
            );
            if let Err(e) = app
                .notification()
                .builder()
                .title(&title)
                .body(&body)
                .show()
            {
                warn!(target: "4da::notify", error = %e, "Failed to send summary security notification");
            } else {
                info!(target: "4da::notify", total_security, "Sent summary security notification (flood cap)");
            }
            // Batch the rest for briefing
            batch_generic_items(state, new_relevant_count, &signal_summary);
            return;
        }
    }

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

/// Send a notification about an escalating or peak signal chain prediction
pub fn send_chain_prediction_notification<R: Runtime>(
    app: &AppHandle<R>,
    chain_name: &str,
    phase: &str,
    forecast: &str,
) {
    let title = match phase {
        "escalating" => format!("4DA — {} Escalating", chain_name),
        "peak" => format!("4DA — {} at Peak", chain_name),
        _ => format!("4DA — {} Signal Chain", chain_name),
    };

    // Truncate forecast for notification body (OS limits ~200 chars)
    let body = if forecast.len() > 180 {
        format!("{}...", truncate_safe(forecast, 177))
    } else {
        forecast.to_string()
    };

    if let Err(e) = app
        .notification()
        .builder()
        .title(&title)
        .body(&body)
        .show()
    {
        warn!(target: "4da::notify", error = %e, "Failed to send chain prediction notification");
    } else {
        info!(target: "4da::notify", chain = %chain_name, phase, "Sent chain prediction notification");
    }
}
