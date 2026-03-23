//! Notification subsystem for 4DA monitoring.
//!
//! Handles OS notification dispatch, signal-aware notification routing,
//! and batching of below-threshold items for digest briefings.

use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;
use tracing::{info, warn};

use crate::monitoring::{BatchedNotification, MonitoringState};
use crate::notification_window::NotificationData;

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
    /// Database ID of the highest-priority signal item (for deep-linking).
    pub top_item_id: Option<i64>,
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
            let title = format!(
                "{} critical, {} high priority alerts found",
                summary.critical_count, summary.high_count
            );

            if notification_style() == "custom" {
                crate::notification_window::show_notification(
                    app,
                    NotificationData {
                        variant: "multi".to_string(),
                        priority: "critical".to_string(),
                        signal_type: Some("security_alert".to_string()),
                        title: title.clone(),
                        action: Some("Open 4DA for details".to_string()),
                        source: None,
                        matched_deps: vec![],
                        count: Some(total_security),
                        chain_sources: None,
                        chain_phase: None,
                        chain_links_filled: None,
                        chain_links_total: None,
                        time_ago: "just now".to_string(),
                        item_id: summary.top_item_id,
                    },
                );
            } else {
                let native_title = format!("4DA — {} Security Alerts Detected", total_security);
                if let Err(e) = app
                    .notification()
                    .builder()
                    .title(&native_title)
                    .body(&title)
                    .show()
                {
                    warn!(target: "4da::notify", error = %e, "Failed to send summary security notification");
                }
            }

            info!(target: "4da::notify", total_security, "Sent summary security notification (flood cap)");
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

/// Read the notification style setting ("custom" or "native").
fn notification_style() -> String {
    let settings = crate::get_settings_manager().lock();
    settings.get().monitoring.notification_style.clone()
}

/// Send a notification about new relevant items
pub fn send_notification<R: Runtime>(
    app: &AppHandle<R>,
    relevant_count: usize,
    _total_count: usize,
) {
    let title = if relevant_count == 1 {
        "1 new item matches your interests".to_string()
    } else {
        format!("{} new items match your interests", relevant_count)
    };

    if notification_style() == "custom" {
        crate::notification_window::show_notification(
            app,
            NotificationData {
                variant: "digest".to_string(),
                priority: "low".to_string(),
                signal_type: None,
                title,
                action: Some("Click to review in briefing".to_string()),
                source: None,
                matched_deps: vec![],
                count: Some(relevant_count),
                chain_sources: None,
                chain_phase: None,
                chain_links_filled: None,
                chain_links_total: None,
                time_ago: "just now".to_string(),
                item_id: None,
            },
        );
        return;
    }

    // Native OS notification fallback
    let native_title = "4DA - New Relevant Items";
    if let Err(e) = app
        .notification()
        .builder()
        .title(native_title)
        .body(&title)
        .show()
    {
        warn!(target: "4da::notify", error = %e, "Failed to send notification");
    } else {
        info!(target: "4da::notify", body = %title, "Sent notification");
    }
}

/// Send a signal-aware notification for critical/high priority signals
pub fn send_signal_notification<R: Runtime>(
    app: &AppHandle<R>,
    priority: &str,
    summary: &SignalSummary,
) {
    let count = match priority {
        "critical" => summary.critical_count,
        "high" => summary.high_count,
        _ => 1,
    };

    let (signal_type, action_text) = summary
        .top_signal
        .as_ref()
        .map(|(st, act)| (Some(st.clone()), Some(act.clone())))
        .unwrap_or((None, None));

    // Build the display title for the notification body
    let display_title = if let Some(ref action) = action_text {
        let label = match signal_type.as_deref() {
            Some("security_alert") => "Security Alert",
            Some("breaking_change") => "Breaking Change",
            _ => "Alert",
        };
        format!("{}: {}", label, action)
    } else {
        format!("{} {} priority items found", count, priority)
    };

    if notification_style() == "custom" {
        // Determine variant: multi if >1 signal, else single signal
        let variant = if count > 1 { "multi" } else { "signal" };
        crate::notification_window::show_notification(
            app,
            NotificationData {
                variant: variant.to_string(),
                priority: priority.to_string(),
                signal_type,
                title: display_title,
                action: action_text,
                source: None,
                matched_deps: vec![],
                count: if count > 1 { Some(count) } else { None },
                chain_sources: None,
                chain_phase: None,
                chain_links_filled: None,
                chain_links_total: None,
                time_ago: "just now".to_string(),
                item_id: summary.top_item_id,
            },
        );
        return;
    }

    // Native OS notification fallback
    let native_title = match priority {
        "critical" => format!(
            "4DA - {} Critical Signal{}",
            count,
            if count > 1 { "s" } else { "" }
        ),
        "high" => format!(
            "4DA - {} High Priority Signal{}",
            count,
            if count > 1 { "s" } else { "" }
        ),
        _ => "4DA - New Signals".to_string(),
    };

    if let Err(e) = app
        .notification()
        .builder()
        .title(&native_title)
        .body(&display_title)
        .show()
    {
        warn!(target: "4da::notify", error = %e, "Failed to send signal notification");
    } else {
        info!(target: "4da::notify", priority = priority, body = %display_title, "Sent signal notification");
    }
}

/// Send a notification about an escalating or peak signal chain prediction
pub fn send_chain_prediction_notification<R: Runtime>(
    app: &AppHandle<R>,
    chain_name: &str,
    phase: &str,
    forecast: &str,
) {
    let priority = match phase {
        "escalating" | "peak" => "critical",
        "active" => "high",
        _ => "medium",
    };

    // Truncate forecast for display
    let body = if forecast.len() > 180 {
        format!("{}...", truncate_safe(forecast, 177))
    } else {
        forecast.to_string()
    };

    if notification_style() == "custom" {
        crate::notification_window::show_notification(
            app,
            NotificationData {
                variant: "chain".to_string(),
                priority: priority.to_string(),
                signal_type: None,
                title: chain_name.to_string(),
                action: Some(body.clone()),
                source: None,
                matched_deps: vec![],
                count: None,
                chain_sources: None, // Caller can extend this later
                chain_phase: Some(phase.to_string()),
                chain_links_filled: None,
                chain_links_total: Some(4),
                time_ago: "just now".to_string(),
                item_id: None,
            },
        );
        return;
    }

    // Native OS notification fallback
    let title = match phase {
        "escalating" => format!("4DA — {} Escalating", chain_name),
        "peak" => format!("4DA — {} at Peak", chain_name),
        _ => format!("4DA — {} Signal Chain", chain_name),
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
