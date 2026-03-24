//! Morning briefing generation for 4DA monitoring.
//!
//! Generates formatted briefing text for CLI output and frontend display.
//! Handles morning briefing scheduling, date tracking, and notification content.

use chrono::Timelike;
use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;
use tracing::{info, warn};

use crate::monitoring::MonitoringState;
use crate::monitoring_notifications::truncate_safe;

// ============================================================================
// Morning Briefing Types
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

// ============================================================================
// Morning Briefing Check
// ============================================================================

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
/// The last briefing date is persisted to settings.json so a restart doesn't
/// re-trigger the briefing on the same day.
pub fn check_morning_briefing(state: &MonitoringState) -> Option<BriefingNotification> {
    // 1. Check settings for morning_briefing enabled, briefing_time, and persisted last date
    let (enabled, briefing_time_str, persisted_date) = {
        let settings = crate::get_settings_manager().lock();
        let monitoring = &settings.get().monitoring;
        let enabled = monitoring.morning_briefing.unwrap_or(true);
        let time = monitoring
            .briefing_time
            .clone()
            .unwrap_or_else(|| "08:00".to_string());
        let last = monitoring.last_briefing_date.clone();
        (enabled, time, last)
    };

    if !enabled {
        return None;
    }

    // 2. Check time window (within 30min of configured time)
    let now = chrono::Local::now();
    let (target_hour, target_min) = parse_briefing_time(&briefing_time_str);
    let now_mins = now.hour() * 60 + now.minute();
    let target_mins = target_hour * 60 + target_min;

    // Use modular arithmetic to handle midnight rollover correctly.
    // E.g. briefing_time 23:45 (1425) and now 00:05 (5): diff = (5 - 1425 + 1440) % 1440 = 20
    let diff = ((now_mins as i32 - target_mins as i32) + 1440) % 1440;
    if diff > 30 {
        return None;
    }

    // 3. Check if already fired today — check both persisted and in-memory state
    let today = now.format("%Y-%m-%d").to_string();

    // Check persisted date first (survives restarts)
    if let Some(ref last) = persisted_date {
        if last == &today {
            return None;
        }
    }
    // Also check in-memory state (covers rapid re-checks within same session)
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

    // 5. Mark as fired today — persist to settings AND in-memory state
    {
        let mut last_date = state.last_morning_briefing_date.lock();
        *last_date = Some(today.clone());
    }
    // Persist to settings.json so restart doesn't re-trigger
    {
        let mut settings = crate::get_settings_manager().lock();
        settings.get_mut().monitoring.last_briefing_date = Some(today);
        if let Err(e) = settings.save() {
            warn!(target: "4da::notify", error = %e, "Failed to persist last_briefing_date to settings");
        }
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
    // Build a compact body from the top items (sanitized to prevent control char injection)
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
            lines.push(format!(
                "{}{}",
                signal_tag,
                strip_control_chars(&item.title)
            ));
        }
        if briefing.total_relevant > 3 {
            lines.push(format!("...and {} more", briefing.total_relevant - 3));
        }
        lines.join("\n")
    };

    // Truncate for OS notification limits (~200 chars)
    let body = if body.len() > 200 {
        format!("{}...", truncate_safe(&body, 197))
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

/// Strip control characters (ANSI escape sequences, etc.) from a string
/// to prevent injection in CLI output.
fn strip_control_chars(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

/// Generate a formatted briefing string suitable for CLI output or frontend display.
/// Uses in-memory analysis state first, falls back to DB query.
/// All titles and sources are sanitized to prevent ANSI injection.
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
        let safe_title = strip_control_chars(title);
        let safe_source = strip_control_chars(source_type);
        let signal_tag = signal_type
            .as_deref()
            .map(|s| format!("[{}] ", strip_control_chars(&s.to_uppercase())))
            .unwrap_or_default();
        output.push_str(&format!("  {signal_tag}{safe_title}\n"));
        output.push_str(&format!(
            "    Source: {} | Score: {:.0}%\n\n",
            safe_source,
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
    fn test_briefing_window_midnight_rollover() {
        // If briefing_time is 23:45 and current time is 00:05,
        // diff should be 20 (within 30-min window)
        let target = 23 * 60 + 45; // 1425
        let now = 0 * 60 + 5; // 5
        let diff = ((now as i32 - target as i32) + 1440) % 1440;
        assert_eq!(diff, 20);
        assert!(diff <= 30);
    }

    #[test]
    fn test_briefing_window_midnight_rollover_outside() {
        // If briefing_time is 23:45 and current time is 00:30,
        // diff should be 45 (outside 30-min window)
        let target = 23 * 60 + 45; // 1425
        let now = 0 * 60 + 30; // 30
        let diff = ((now as i32 - target as i32) + 1440) % 1440;
        assert_eq!(diff, 45);
        assert!(diff > 30);
    }

    #[test]
    fn test_briefing_window_normal_case() {
        // If briefing_time is 08:00 and current time is 08:15,
        // diff should be 15 (within 30-min window)
        let target = 8 * 60; // 480
        let now = 8 * 60 + 15; // 495
        let diff = ((now as i32 - target as i32) + 1440) % 1440;
        assert_eq!(diff, 15);
        assert!(diff <= 30);
    }

    #[test]
    fn test_briefing_window_before_target() {
        // If briefing_time is 08:00 and current time is 07:50,
        // diff should be 1430 (well outside 30-min window — not yet time)
        let target = 8 * 60; // 480
        let now = 7 * 60 + 50; // 470
        let diff = ((now as i32 - target as i32) + 1440) % 1440;
        assert_eq!(diff, 1430);
        assert!(diff > 30);
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
