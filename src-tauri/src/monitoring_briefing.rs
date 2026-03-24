//! Morning briefing generation for 4DA monitoring.
//!
//! Generates formatted briefing text for CLI output and frontend display.
//! Handles morning briefing scheduling, date tracking, and notification content.

use chrono::Timelike;
use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;
use tracing::{info, warn};

use crate::monitoring::MonitoringState;
use crate::monitoring_notifications::truncate_safe;

// ============================================================================
// Morning Briefing Types
// ============================================================================

/// Item for morning briefing notification.
/// Enriched with url, item_id, matched_deps for the center-screen briefing window.
#[derive(Debug, Clone, Serialize)]
pub struct BriefingItem {
    pub title: String,
    pub source_type: String,
    pub score: f32,
    pub signal_type: Option<String>,
    /// URL for opening in system browser
    #[serde(default)]
    pub url: Option<String>,
    /// Database ID for deep-linking to item in main window
    #[serde(default)]
    pub item_id: Option<i64>,
    /// Signal priority tier (watch, advisory, alert, critical)
    #[serde(default)]
    pub signal_priority: Option<String>,
    /// Short description or action text
    #[serde(default)]
    pub description: Option<String>,
    /// Matched dependency names (why this matters to the user)
    #[serde(default)]
    pub matched_deps: Vec<String>,
}

/// A topic the user hasn't seen intelligence about recently
#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeGap {
    pub topic: String,
    pub days_since_last: i64,
}

/// Morning briefing notification content.
/// Enriched for center-screen briefing window with knowledge gaps and ongoing topics.
#[derive(Debug, Clone, Serialize)]
pub struct BriefingNotification {
    pub title: String,
    pub items: Vec<BriefingItem>,
    pub total_relevant: usize,
    /// Topics that appeared in recent briefings (ongoing, not novel)
    #[serde(default)]
    pub ongoing_topics: Vec<String>,
    /// Declared tech topics with no recent signals
    #[serde(default)]
    pub knowledge_gaps: Vec<KnowledgeGap>,
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
                .take(8) // 8 items for center-screen briefing (was 5 for toast)
                .map(|r| BriefingItem {
                    title: r.title.clone(),
                    source_type: r.source_type.clone(),
                    score: r.top_score,
                    signal_type: r.signal_type.clone(),
                    url: r.url.clone(),
                    item_id: Some(r.id as i64),
                    signal_priority: r.signal_priority.clone(),
                    description: r.signal_action.clone(),
                    matched_deps: r.signal_triggers.clone().unwrap_or_default(),
                })
                .collect()
        } else {
            // Fall back to DB query for recent items
            if let Ok(db) = crate::get_database() {
                let period_start = chrono::Utc::now() - chrono::Duration::hours(24);
                db.get_relevant_items_since(period_start, 0.1, 8)
                    .ok()
                    .map(|db_items| {
                        db_items
                            .into_iter()
                            .map(|i| BriefingItem {
                                title: i.title,
                                source_type: i.source_type,
                                score: i.relevance_score.unwrap_or(0.0) as f32,
                                signal_type: None,
                                url: i.url,
                                item_id: Some(i.id),
                                signal_priority: None,
                                description: None,
                                matched_deps: vec![],
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

    // 6. Detect knowledge gaps: declared tech with no recent signals
    let knowledge_gaps = detect_knowledge_gaps();

    Some(BriefingNotification {
        title: format!("4DA Intelligence Briefing — {}", now.format("%d %b %Y")),
        items,
        total_relevant,
        ongoing_topics: vec![], // Populated by novelty detection in future
        knowledge_gaps,
    })
}

/// Send a morning briefing via the center-screen briefing window.
///
/// Falls back to OS native notification if the briefing window is unavailable.
pub fn send_morning_briefing_notification<R: Runtime>(
    app: &AppHandle<R>,
    briefing: &BriefingNotification,
) {
    // Primary: center-screen intelligence briefing window
    crate::briefing_window::show_briefing(app, briefing);

    // Also send a subtle OS notification as a backup (e.g. if user has multiple monitors
    // and the briefing window is on the wrong one, or if window init failed).
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

    let body = if body.len() > 200 {
        format!("{}...", truncate_safe(&body, 197))
    } else {
        body
    };

    // OS notification as companion (non-blocking)
    if let Err(e) = app
        .notification()
        .builder()
        .title(&briefing.title)
        .body(&body)
        .show()
    {
        warn!(target: "4da::briefing", error = %e, "OS notification fallback failed (briefing window is primary)");
    }

    info!(
        target: "4da::briefing",
        items = briefing.total_relevant,
        gaps = briefing.knowledge_gaps.len(),
        "Intelligence briefing delivered"
    );
}

// ============================================================================
// Knowledge Gap Detection
// ============================================================================

/// Detect knowledge gaps: declared tech topics with no recent source items.
/// Returns topics the user cares about but hasn't seen intelligence on in 5+ days.
fn detect_knowledge_gaps() -> Vec<KnowledgeGap> {
    let declared_tech: Vec<String> = {
        match crate::get_context_engine() {
            Ok(engine) => {
                if let Ok(identity) = engine.get_static_identity() {
                    identity.tech_stack.clone()
                } else {
                    return vec![];
                }
            }
            Err(_) => return vec![],
        }
    };

    if declared_tech.is_empty() {
        return vec![];
    }

    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut gaps = Vec::new();

    for tech in &declared_tech {
        let tech_lower = tech.to_lowercase();
        // Find the most recent item mentioning this tech
        let last_seen: Option<i64> = conn
            .query_row(
                "SELECT CAST(julianday('now') - julianday(MAX(created_at)) AS INTEGER)
                 FROM source_items
                 WHERE LOWER(title) LIKE ?1",
                rusqlite::params![format!("%{}%", tech_lower)],
                |row| row.get(0),
            )
            .ok();

        if let Some(days) = last_seen {
            if days >= 5 {
                gaps.push(KnowledgeGap {
                    topic: tech.clone(),
                    days_since_last: days,
                });
            }
        }
    }

    // Sort by stalest first
    gaps.sort_by(|a, b| b.days_since_last.cmp(&a.days_since_last));
    gaps.truncate(5); // Max 5 gaps in briefing
    gaps
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
            url: Some("https://example.com/rust-2026".to_string()),
            item_id: Some(42),
            signal_priority: Some("alert".to_string()),
            description: Some("Review Rust 2026 changes".to_string()),
            matched_deps: vec!["rust".to_string()],
        };
        assert_eq!(item.title, "Rust 2026 Edition announced");
        assert_eq!(item.source_type, "hackernews");
        assert!(item.score > 0.8);
        assert_eq!(item.signal_type, Some("new_release".to_string()));
        assert!(item.url.is_some());
        assert_eq!(item.item_id, Some(42));
        assert_eq!(item.matched_deps.len(), 1);
    }

    #[test]
    fn test_briefing_notification_construction() {
        let briefing = BriefingNotification {
            title: "4DA Intelligence Briefing — 19 Mar 2026".to_string(),
            items: vec![
                BriefingItem {
                    title: "Critical CVE in tokio".to_string(),
                    source_type: "github".to_string(),
                    score: 0.95,
                    signal_type: Some("security_alert".to_string()),
                    url: Some("https://example.com/cve".to_string()),
                    item_id: Some(1),
                    signal_priority: Some("critical".to_string()),
                    description: Some("Patch tokio immediately".to_string()),
                    matched_deps: vec!["tokio".to_string()],
                },
                BriefingItem {
                    title: "Tauri 3.0 beta released".to_string(),
                    source_type: "hackernews".to_string(),
                    score: 0.80,
                    signal_type: Some("new_release".to_string()),
                    url: None,
                    item_id: Some(2),
                    signal_priority: Some("advisory".to_string()),
                    description: None,
                    matched_deps: vec![],
                },
            ],
            total_relevant: 2,
            ongoing_topics: vec![],
            knowledge_gaps: vec![KnowledgeGap {
                topic: "sqlite".to_string(),
                days_since_last: 7,
            }],
        };
        assert_eq!(briefing.items.len(), 2);
        assert_eq!(briefing.total_relevant, 2);
        assert!(briefing.title.contains("Intelligence Briefing"));
        assert_eq!(briefing.knowledge_gaps.len(), 1);
        assert_eq!(briefing.knowledge_gaps[0].days_since_last, 7);
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
