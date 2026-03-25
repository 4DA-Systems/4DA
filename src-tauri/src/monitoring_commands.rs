//! Monitoring Tauri commands for background analysis scheduling.
//!
//! Extracted from lib.rs. Provides commands for checking monitoring status,
//! enabling/disabling monitoring, and setting intervals.

use tauri::AppHandle;
use tracing::{info, warn};

use crate::error::Result;
use crate::{get_monitoring_state, get_settings_manager, monitoring, settings};

// ============================================================================
// Monitoring Commands (Phase 3)
// ============================================================================

/// Get monitoring status
#[tauri::command]
pub async fn get_monitoring_status() -> Result<serde_json::Value> {
    let state = get_monitoring_state();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let last_check = state.last_check.load(std::sync::atomic::Ordering::Relaxed);
    let secs_since_check = if last_check > 0 { now - last_check } else { 0 };

    // Include settings from config
    let (notification_threshold, close_to_tray, notification_style) = {
        let settings = get_settings_manager().lock();
        let m = &settings.get().monitoring;
        (
            m.notification_threshold.clone(),
            m.close_to_tray.unwrap_or(true),
            m.notification_style.clone(),
        )
    };

    Ok(serde_json::json!({
        "enabled": state.is_enabled(),
        "interval_secs": state.get_interval(),
        "interval_mins": state.get_interval() / 60,
        "is_checking": state.is_checking.load(std::sync::atomic::Ordering::Relaxed),
        "last_check": last_check,
        "secs_since_check": secs_since_check,
        "last_relevant_count": state.last_relevant_count.load(std::sync::atomic::Ordering::Relaxed),
        "total_checks": state.total_checks.load(std::sync::atomic::Ordering::Relaxed),
        "notification_threshold": notification_threshold,
        "close_to_tray": close_to_tray,
        "notification_style": notification_style
    }))
}

/// Enable or disable monitoring
#[tauri::command]
pub async fn set_monitoring_enabled(enabled: bool) -> Result<serde_json::Value> {
    let state = get_monitoring_state();
    state.set_enabled(enabled);

    if enabled {
        // Set last_check to 0 to trigger immediate check
        state
            .last_check
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    // Persist to settings (preserve existing notification_threshold)
    {
        let mut settings = get_settings_manager().lock();
        let interval = state.get_interval() / 60;
        let existing = settings.get().monitoring.clone();
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            enabled,
            interval_minutes: interval,
            ..existing
        });
    }

    info!(target: "4da::monitor", enabled = enabled, "Monitoring state persisted");

    Ok(serde_json::json!({
        "enabled": enabled,
        "message": if enabled { "Monitoring started" } else { "Monitoring stopped" }
    }))
}

/// Set monitoring interval
#[tauri::command]
pub async fn set_monitoring_interval(minutes: u64) -> Result<serde_json::Value> {
    let minutes = minutes.clamp(1, 1440);

    // Tier-based minimum interval enforcement
    {
        let manager = get_settings_manager();
        let guard = manager.lock();
        let license = &guard.get().license;
        let is_paid = matches!(
            license.tier.as_str(),
            "signal" | "pro" | "team" | "enterprise"
        ) || settings::is_trial_active(license);

        if is_paid {
            if minutes < 5 {
                return Err("Minimum monitoring interval is 5 minutes.".into());
            }
        } else if minutes < 30 {
            return Err(
                "Free tier minimum monitoring interval is 30 minutes. Upgrade to Signal for intervals as low as 5 minutes."
                    .into(),
            );
        }
    }

    let state = get_monitoring_state();
    let secs = minutes * 60;
    state.set_interval(secs);

    // Persist to settings (preserve existing notification_threshold)
    {
        let mut settings = get_settings_manager().lock();
        let existing = settings.get().monitoring.clone();
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            enabled: state.is_enabled(),
            interval_minutes: minutes,
            ..existing
        });
    }

    info!(target: "4da::monitor", interval_mins = minutes, "Interval persisted");

    Ok(serde_json::json!({
        "interval_mins": minutes,
        "interval_secs": secs
    }))
}

/// Set notification quality threshold
#[tauri::command]
pub async fn set_notification_threshold(threshold: String) -> Result<serde_json::Value> {
    // Validate the threshold value
    let valid = ["critical_only", "high_and_above", "all"];
    if !valid.contains(&threshold.as_str()) {
        return Err(format!(
            "Invalid threshold '{}'. Must be one of: {}",
            threshold,
            valid.join(", ")
        )
        .into());
    }

    // Persist to settings (preserve existing enabled/interval)
    {
        let mut settings = get_settings_manager().lock();
        let state = crate::get_monitoring_state();
        let existing = settings.get().monitoring.clone();
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            enabled: state.is_enabled(),
            interval_minutes: state.get_interval() / 60,
            notification_threshold: threshold.clone(),
            ..existing
        });
    }

    info!(target: "4da::monitor", threshold = %threshold, "Notification threshold updated");

    Ok(serde_json::json!({
        "threshold": threshold,
        "message": format!("Notification threshold set to {}", threshold)
    }))
}

/// Test notification delivery (default: low digest)
#[tauri::command]
pub async fn trigger_notification_test(app: AppHandle) -> Result<serde_json::Value> {
    monitoring::send_notification(&app, 3, 30);
    Ok(serde_json::json!({
        "success": true,
        "message": "Test notification sent"
    }))
}

/// Test notification with specific priority and content
#[tauri::command]
pub async fn trigger_notification_preview(
    app: AppHandle,
    priority: String,
) -> Result<serde_json::Value> {
    let data = match priority.as_str() {
        "critical" => crate::notification_window::NotificationData {
            variant: "signal".to_string(),
            priority: "critical".to_string(),
            signal_type: Some("security_alert".to_string()),
            title: "CVE-2026-1234 in SQLite: RCE vulnerability".to_string(),
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
        },
        "alert" => crate::notification_window::NotificationData {
            variant: "signal".to_string(),
            priority: "alert".to_string(),
            signal_type: Some("breaking_change".to_string()),
            title: "React 20 drops class components — migration guide".to_string(),
            action: Some("Check migration path".to_string()),
            source: Some("hackernews".to_string()),
            matched_deps: vec!["react".to_string(), "react-dom".to_string()],
            count: None,
            chain_sources: None,
            chain_phase: None,
            chain_links_filled: None,
            chain_links_total: None,
            time_ago: "5m ago".to_string(),
            item_id: None,
        },
        "advisory" => crate::notification_window::NotificationData {
            variant: "signal".to_string(),
            priority: "advisory".to_string(),
            signal_type: Some("tool_discovery".to_string()),
            title: "Show HN: A new Rust testing framework".to_string(),
            action: Some("via hackernews".to_string()),
            source: Some("hackernews".to_string()),
            matched_deps: vec!["rust".to_string()],
            count: None,
            chain_sources: None,
            chain_phase: None,
            chain_links_filled: None,
            chain_links_total: None,
            time_ago: "12m ago".to_string(),
            item_id: None,
        },
        _ => crate::notification_window::NotificationData {
            variant: "digest".to_string(),
            priority: "watch".to_string(),
            signal_type: None,
            title: "3 new items match your interests".to_string(),
            action: Some("Click to review in briefing".to_string()),
            source: None,
            matched_deps: vec![],
            count: Some(3),
            chain_sources: None,
            chain_phase: None,
            chain_links_filled: None,
            chain_links_total: None,
            time_ago: "just now".to_string(),
            item_id: None,
        },
    };

    crate::notification_window::show_notification(&app, data);
    Ok(serde_json::json!({ "success": true, "priority": priority }))
}

/// Set notification style ("custom" for GAME-powered or "native" for OS toasts)
#[tauri::command]
pub async fn set_notification_style(style: String) -> Result<serde_json::Value> {
    let valid = ["custom", "native"];
    if !valid.contains(&style.as_str()) {
        return Err(
            format!("Invalid notification style '{style}'. Must be 'custom' or 'native'.").into(),
        );
    }

    {
        let mut settings = get_settings_manager().lock();
        let existing = settings.get().monitoring.clone();
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            notification_style: style.clone(),
            ..existing
        });
    }

    info!(target: "4da::monitor", style = %style, "Notification style updated");

    Ok(serde_json::json!({
        "notification_style": style
    }))
}

/// Toggle close-to-tray behavior
#[tauri::command]
pub async fn set_close_to_tray(enabled: bool) -> Result<serde_json::Value> {
    {
        let mut settings = get_settings_manager().lock();
        let state = crate::get_monitoring_state();
        let m = settings.get().monitoring.clone();
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            close_to_tray: Some(enabled),
            ..m
        });
        let _ = state; // keep reference alive
    }

    info!(target: "4da::monitor", close_to_tray = enabled, "Close-to-tray updated");

    Ok(serde_json::json!({
        "close_to_tray": enabled,
        "message": if enabled { "Window will hide to tray on close" } else { "Window will quit on close" }
    }))
}

/// Set whether 4DA launches at system startup.
#[tauri::command]
pub async fn set_launch_at_startup(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<serde_json::Value> {
    // Update the autostart plugin
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    if enabled {
        if let Err(e) = autostart.enable() {
            warn!(target: "4da::settings", error = %e, "Failed to enable autostart");
        }
    } else if let Err(e) = autostart.disable() {
        warn!(target: "4da::settings", error = %e, "Failed to disable autostart");
    }

    // Persist in settings
    {
        let mut settings = get_settings_manager().lock();
        let m = settings.get().monitoring.clone();
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            launch_at_startup: Some(enabled),
            ..m
        });
    }

    info!(target: "4da::settings", launch_at_startup = enabled, "Startup launch updated");

    Ok(serde_json::json!({
        "launch_at_startup": enabled,
        "message": if enabled { "4DA will launch at system startup" } else { "4DA will not launch at system startup" }
    }))
}

/// Get current autostart state.
#[tauri::command]
pub async fn get_launch_at_startup(app: tauri::AppHandle) -> Result<bool> {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    Ok(autostart.is_enabled().unwrap_or(false))
}

// ============================================================================
// Morning Briefing Configuration
// ============================================================================

/// Set whether the morning intelligence briefing is enabled.
#[tauri::command]
pub async fn set_morning_briefing_enabled(enabled: bool) -> Result<serde_json::Value> {
    {
        let mut settings = get_settings_manager().lock();
        let m = settings.get().monitoring.clone();
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            morning_briefing: Some(enabled),
            ..m
        });
    }
    info!(target: "4da::settings", morning_briefing = enabled, "Morning briefing updated");
    Ok(serde_json::json!({
        "morning_briefing": enabled,
        "message": if enabled { "Morning briefing enabled" } else { "Morning briefing disabled" }
    }))
}

/// Get the current morning briefing configuration.
#[tauri::command]
pub async fn get_morning_briefing_config() -> Result<serde_json::Value> {
    let settings = get_settings_manager().lock();
    let m = &settings.get().monitoring;
    Ok(serde_json::json!({
        "enabled": m.morning_briefing.unwrap_or(true),
        "time": m.briefing_time.clone().unwrap_or_else(|| "08:00".to_string()),
    }))
}

/// Set the morning briefing time (HH:MM format).
#[tauri::command]
pub async fn set_briefing_time(time: String) -> Result<serde_json::Value> {
    // Validate HH:MM format
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() != 2 {
        return Err("Invalid time format — use HH:MM".into());
    }
    let hour = parts[0].parse::<u32>().map_err(|_| "Invalid hour")?;
    let minute = parts[1].parse::<u32>().map_err(|_| "Invalid minute")?;
    if hour > 23 || minute > 59 {
        return Err("Time out of range (00:00–23:59)".into());
    }
    let validated = format!("{:02}:{:02}", hour, minute);

    {
        let mut settings = get_settings_manager().lock();
        let m = settings.get().monitoring.clone();
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            briefing_time: Some(validated.clone()),
            ..m
        });
    }
    info!(target: "4da::settings", briefing_time = %validated, "Briefing time updated");
    Ok(serde_json::json!({
        "briefing_time": validated,
        "message": format!("Briefing time set to {}", validated)
    }))
}

/// Trigger the intelligence briefing window manually for testing/preview.
#[tauri::command]
pub async fn trigger_briefing_preview(app: tauri::AppHandle) -> Result<serde_json::Value> {
    use crate::monitoring_briefing::{
        BriefingItem, BriefingNotification, ChainSummary, KnowledgeGap,
    };

    let now = chrono::Local::now();
    let preview = BriefingNotification {
        title: format!("4DA Intelligence Briefing — {}", now.format("%d %b %Y")),
        items: vec![
            BriefingItem {
                title: "Critical RCE in SQLite 3.50 — affects all platforms".to_string(),
                source_type: "hackernews".to_string(),
                score: 0.95,
                signal_type: Some("security_alert".to_string()),
                url: Some("https://example.com/sqlite-cve".to_string()),
                item_id: Some(1),
                signal_priority: Some("critical".to_string()),
                description: Some(
                    "Patch SQLite immediately — your projects depend on it".to_string(),
                ),
                matched_deps: vec!["sqlite".to_string(), "rusqlite".to_string()],
            },
            BriefingItem {
                title: "Tauri 3.0 drops macOS 11 support — migration guide".to_string(),
                source_type: "github".to_string(),
                score: 0.82,
                signal_type: Some("breaking_change".to_string()),
                url: Some("https://example.com/tauri3".to_string()),
                item_id: Some(2),
                signal_priority: Some("alert".to_string()),
                description: Some("Review migration guide before upgrading".to_string()),
                matched_deps: vec!["tauri".to_string()],
            },
            BriefingItem {
                title: "Show HN: Rust testing framework 10x faster than cargo test".to_string(),
                source_type: "hackernews".to_string(),
                score: 0.71,
                signal_type: Some("tool_discovery".to_string()),
                url: None,
                item_id: Some(3),
                signal_priority: Some("advisory".to_string()),
                description: Some("Evaluate for your Rust workflow".to_string()),
                matched_deps: vec![],
            },
        ],
        total_relevant: 3,
        ongoing_topics: vec!["WebAssembly".to_string(), "AI agents".to_string()],
        knowledge_gaps: vec![KnowledgeGap {
            topic: "React".to_string(),
            days_since_last: 12,
        }],
        escalating_chains: vec![ChainSummary {
            name: "SQLite security chain (3 events)".to_string(),
            phase: "escalating".to_string(),
            link_count: 3,
            action: "Review security implications for SQLite in your projects".to_string(),
            confidence: 0.87,
        }],
    };

    crate::briefing_window::show_briefing(&app, &preview);

    info!(target: "4da::settings", "Briefing preview triggered");
    Ok(serde_json::json!({ "message": "Briefing preview shown" }))
}

#[cfg(test)]
mod tests {
    use crate::monitoring::{BatchedNotification, MonitoringState};
    use std::sync::atomic::Ordering;

    // ---- MonitoringState construction & defaults ----

    #[test]
    fn test_monitoring_state_default() {
        let state = MonitoringState::new();
        assert!(!state.is_enabled());
        assert_eq!(state.get_interval(), 1800); // 30 minutes default
        assert!(!state.is_checking.load(Ordering::Relaxed));
        assert_eq!(state.last_check.load(Ordering::Relaxed), 0);
        assert_eq!(state.last_relevant_count.load(Ordering::Relaxed), 0);
        assert_eq!(state.total_checks.load(Ordering::Relaxed), 0);
    }

    // ---- MonitoringState enable/disable ----

    #[test]
    fn test_monitoring_state_enable_disable() {
        let state = MonitoringState::new();
        assert!(!state.is_enabled());

        state.set_enabled(true);
        assert!(state.is_enabled());

        state.set_enabled(false);
        assert!(!state.is_enabled());
    }

    // ---- MonitoringState interval ----

    #[test]
    fn test_monitoring_state_set_interval() {
        let state = MonitoringState::new();
        state.set_interval(3600);
        assert_eq!(state.get_interval(), 3600);
    }

    #[test]
    fn test_monitoring_state_interval_clamped_low() {
        let state = MonitoringState::new();
        // set_interval clamps to [60, 86400]
        state.set_interval(10);
        assert_eq!(state.get_interval(), 60);
    }

    #[test]
    fn test_monitoring_state_interval_clamped_high() {
        let state = MonitoringState::new();
        state.set_interval(100_000);
        assert_eq!(state.get_interval(), 86400);
    }

    // ---- BatchedNotification construction ----

    #[test]
    fn test_batched_notification_construction() {
        let notification = BatchedNotification {
            title: "New Rust RFC".to_string(),
            source_type: "hackernews".to_string(),
            score: 0.85,
            signal_priority: Some("alert".to_string()),
        };
        assert_eq!(notification.title, "New Rust RFC");
        assert_eq!(notification.source_type, "hackernews");
        assert!(notification.score > 0.8);
        assert_eq!(notification.signal_priority, Some("alert".to_string()));
    }

    #[test]
    fn test_batched_notification_no_priority() {
        let notification = BatchedNotification {
            title: "Minor update".to_string(),
            source_type: "rss".to_string(),
            score: 0.3,
            signal_priority: None,
        };
        assert!(notification.signal_priority.is_none());
    }

    // ---- MonitoringState batched_items ----

    #[test]
    fn test_monitoring_state_batched_items() {
        let state = MonitoringState::new();
        {
            let mut items = state.batched_items.lock();
            assert!(items.is_empty());
            items.push(BatchedNotification {
                title: "Test".to_string(),
                source_type: "test".to_string(),
                score: 0.5,
                signal_priority: None,
            });
        }
        let items = state.batched_items.lock();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Test");
    }

    // ---- Notification threshold validation logic ----

    #[test]
    fn test_notification_threshold_valid_values() {
        let valid = ["critical_only", "high_and_above", "all"];
        assert!(valid.contains(&"critical_only"));
        assert!(valid.contains(&"high_and_above"));
        assert!(valid.contains(&"all"));
        assert!(!valid.contains(&"invalid"));
        assert!(!valid.contains(&""));
    }

    // ---- Minutes clamping logic (from set_monitoring_interval) ----

    #[test]
    fn test_minutes_clamping() {
        // Mirrors the clamping logic in set_monitoring_interval
        let clamp = |m: u64| m.clamp(1, 1440);
        assert_eq!(clamp(0), 1);
        assert_eq!(clamp(1), 1);
        assert_eq!(clamp(30), 30);
        assert_eq!(clamp(1440), 1440);
        assert_eq!(clamp(9999), 1440);
    }
}
