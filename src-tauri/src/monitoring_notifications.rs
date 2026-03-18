//! Notification subsystem for 4DA monitoring.
//!
//! Handles OS notification dispatch, signal-aware notification routing,
//! and batching of below-threshold items for digest briefings.

use chrono::Timelike;
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
        format!("{}...", &forecast[..177])
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

// ============================================================================
// Morning Briefing Notifications
// ============================================================================

/// Item for morning briefing notification
#[derive(Debug, Clone)]
pub struct BriefingItem {
    pub title: String,
    pub source_type: String,
    pub score: f32,
    pub signal_type: Option<String>,
}

/// Morning briefing notification content
#[derive(Debug, Clone)]
pub struct BriefingNotification {
    pub title: String,
    pub items: Vec<BriefingItem>,
    pub total_relevant: usize,
}

/// Parse "HH:MM" time string, returning (hour, minute). Defaults to (8, 0) on parse failure.
fn parse_briefing_time(time_str: &str) -> (u32, u32) {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() == 2 {
        let hour = parts[0].parse::<u32>().unwrap_or(8);
        let minute = parts[1].parse::<u32>().unwrap_or(0);
        (hour.min(23), minute.min(59))
    } else {
        (8, 0)
    }
}

/// Check if morning briefing should fire and generate notification content.
/// Returns None if disabled, outside the briefing window, or already fired today.
pub fn check_morning_briefing(state: &MonitoringState) -> Option<BriefingNotification> {
    // 1. Check settings for morning_briefing enabled and briefing_time
    let (enabled, briefing_time_str) = {
        let settings = crate::get_settings_manager().lock();
        let monitoring = &settings.get().monitoring;
        let enabled = monitoring.morning_briefing.unwrap_or(true);
        let time = monitoring
            .briefing_time
            .clone()
            .unwrap_or_else(|| "08:00".to_string());
        (enabled, time)
    };

    if !enabled {
        return None;
    }

    // 2. Check time window (within 30min of configured time)
    let now = chrono::Local::now();
    let (target_hour, target_min) = parse_briefing_time(&briefing_time_str);
    let now_mins = now.hour() * 60 + now.minute();
    let target_mins = target_hour * 60 + target_min;

    if now_mins < target_mins || now_mins > target_mins + 30 {
        return None;
    }

    // 3. Check if already fired today
    let today = now.format("%Y-%m-%d").to_string();
    {
        let last_date = state.last_morning_briefing_date.lock();
        if let Some(ref last) = *last_date {
            if last == &today {
                return None;
            }
        }
    }

    // 4. Get top relevant items from in-memory analysis state or DB
    let items: Vec<BriefingItem> = {
        let analysis_state = crate::get_analysis_state().lock();
        if let Some(ref results) = analysis_state.results {
            results
                .iter()
                .filter(|r| r.relevant && !r.excluded)
                .take(5)
                .map(|r| BriefingItem {
                    title: r.title.clone(),
                    source_type: r.source_type.clone(),
                    score: r.top_score,
                    signal_type: r.signal_type.clone(),
                })
                .collect()
        } else {
            // Fall back to DB query for recent items
            if let Ok(db) = crate::get_database() {
                let period_start = chrono::Utc::now() - chrono::Duration::hours(24);
                db.get_relevant_items_since(period_start, 0.1, 5)
                    .ok()
                    .map(|db_items| {
                        db_items
                            .into_iter()
                            .map(|i| BriefingItem {
                                title: i.title,
                                source_type: i.source_type,
                                score: i.relevance_score.unwrap_or(0.0) as f32,
                                signal_type: None,
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                vec![]
            }
        }
    };

    if items.is_empty() {
        return None;
    }

    let total_relevant = items.len();

    // 5. Mark as fired today
    {
        let mut last_date = state.last_morning_briefing_date.lock();
        *last_date = Some(today);
    }

    Some(BriefingNotification {
        title: format!("4DA Morning Briefing — {}", now.format("%d %b %Y")),
        items,
        total_relevant,
    })
}

/// Send a morning briefing notification via the OS notification system.
pub fn send_morning_briefing_notification<R: Runtime>(
    app: &AppHandle<R>,
    briefing: &BriefingNotification,
) {
    // Build a compact body from the top items
    let body = if briefing.items.is_empty() {
        "No new signals since last check.".to_string()
    } else {
        let mut lines: Vec<String> = Vec::new();
        for item in briefing.items.iter().take(3) {
            let signal_tag = item
                .signal_type
                .as_deref()
                .map(|s| format!("[{}] ", s.to_uppercase()))
                .unwrap_or_default();
            lines.push(format!("{}{}", signal_tag, item.title));
        }
        if briefing.total_relevant > 3 {
            lines.push(format!("...and {} more", briefing.total_relevant - 3));
        }
        lines.join("\n")
    };

    // Truncate for OS notification limits (~200 chars)
    let body = if body.len() > 200 {
        format!("{}...", &body[..197])
    } else {
        body
    };

    if let Err(e) = app
        .notification()
        .builder()
        .title(&briefing.title)
        .body(&body)
        .show()
    {
        warn!(target: "4da::notify", error = %e, "Failed to send morning briefing notification");
    } else {
        info!(
            target: "4da::notify",
            items = briefing.total_relevant,
            "Sent morning briefing notification"
        );
    }
}

// ============================================================================
// CLI Briefing Generator
// ============================================================================

/// Generate a formatted briefing string suitable for CLI output or frontend display.
/// Uses in-memory analysis state first, falls back to DB query.
pub fn generate_briefing_text() -> String {
    let now = chrono::Local::now();
    let mut output = format!("4DA Morning Briefing — {}\n\n", now.format("%d %b %Y"));

    // Try in-memory results first
    let items: Vec<(String, String, f32, Option<String>)> = {
        let state = crate::get_analysis_state().lock();
        if let Some(ref results) = state.results {
            results
                .iter()
                .filter(|r| r.relevant && !r.excluded)
                .take(10)
                .map(|r| {
                    (
                        r.title.clone(),
                        r.source_type.clone(),
                        r.top_score,
                        r.signal_type.clone(),
                    )
                })
                .collect()
        } else {
            vec![]
        }
    };

    // Fall back to DB if no in-memory results
    let items = if items.is_empty() {
        if let Ok(db) = crate::get_database() {
            let period_start = chrono::Utc::now() - chrono::Duration::hours(24);
            db.get_relevant_items_since(period_start, 0.1, 10)
                .ok()
                .map(|db_items| {
                    db_items
                        .into_iter()
                        .map(|i| {
                            (
                                i.title,
                                i.source_type,
                                i.relevance_score.unwrap_or(0.0) as f32,
                                None::<String>,
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        } else {
            vec![]
        }
    } else {
        items
    };

    if items.is_empty() {
        output.push_str("  No new signals since last check.\n");
        return output;
    }

    for (title, source_type, score, signal_type) in items.iter().take(10) {
        let signal_tag = signal_type
            .as_deref()
            .map(|s| format!("[{}] ", s.to_uppercase()))
            .unwrap_or_default();
        output.push_str(&format!("  {}{}\n", signal_tag, title));
        output.push_str(&format!(
            "    Source: {} | Score: {:.0}%\n\n",
            source_type,
            score * 100.0
        ));
    }

    output.push_str(&format!("  {} total relevant items\n", items.len()));
    output
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_briefing_time_valid() {
        assert_eq!(parse_briefing_time("08:00"), (8, 0));
        assert_eq!(parse_briefing_time("23:59"), (23, 59));
        assert_eq!(parse_briefing_time("00:00"), (0, 0));
        assert_eq!(parse_briefing_time("12:30"), (12, 30));
    }

    #[test]
    fn test_parse_briefing_time_invalid() {
        // Invalid formats fall back to (8, 0)
        assert_eq!(parse_briefing_time("invalid"), (8, 0));
        assert_eq!(parse_briefing_time(""), (8, 0));
        assert_eq!(parse_briefing_time("8"), (8, 0));
    }

    #[test]
    fn test_parse_briefing_time_clamped() {
        // Hours and minutes clamped to valid range
        assert_eq!(parse_briefing_time("99:99"), (23, 59));
        assert_eq!(parse_briefing_time("30:70"), (23, 59));
    }

    #[test]
    fn test_briefing_item_construction() {
        let item = BriefingItem {
            title: "Rust 2026 Edition announced".to_string(),
            source_type: "hackernews".to_string(),
            score: 0.85,
            signal_type: Some("new_release".to_string()),
        };
        assert_eq!(item.title, "Rust 2026 Edition announced");
        assert_eq!(item.source_type, "hackernews");
        assert!(item.score > 0.8);
        assert_eq!(item.signal_type, Some("new_release".to_string()));
    }

    #[test]
    fn test_briefing_notification_construction() {
        let briefing = BriefingNotification {
            title: "4DA Morning Briefing — 19 Mar 2026".to_string(),
            items: vec![
                BriefingItem {
                    title: "Critical CVE in tokio".to_string(),
                    source_type: "github".to_string(),
                    score: 0.95,
                    signal_type: Some("security_alert".to_string()),
                },
                BriefingItem {
                    title: "Tauri 3.0 beta released".to_string(),
                    source_type: "hackernews".to_string(),
                    score: 0.80,
                    signal_type: Some("new_release".to_string()),
                },
            ],
            total_relevant: 2,
        };
        assert_eq!(briefing.items.len(), 2);
        assert_eq!(briefing.total_relevant, 2);
        assert!(briefing.title.contains("Morning Briefing"));
    }

    #[test]
    fn test_morning_briefing_date_tracking() {
        let state = MonitoringState::new();
        // Initially no date
        assert!(state.last_morning_briefing_date.lock().is_none());

        // Set a date
        {
            let mut guard = state.last_morning_briefing_date.lock();
            *guard = Some("2026-03-19".to_string());
        }

        // Verify it's set
        let guard = state.last_morning_briefing_date.lock();
        assert_eq!(*guard, Some("2026-03-19".to_string()));
    }
}
