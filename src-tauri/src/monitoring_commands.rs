//! Monitoring Tauri commands for background analysis scheduling.
//!
//! Extracted from lib.rs. Provides commands for checking monitoring status,
//! enabling/disabling monitoring, and setting intervals.

use tauri::AppHandle;
use tracing::info;

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
    let (notification_threshold, close_to_tray) = {
        let settings = get_settings_manager().lock();
        let m = &settings.get().monitoring;
        (
            m.notification_threshold.clone(),
            m.close_to_tray.unwrap_or(true),
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
        "close_to_tray": close_to_tray
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
        let threshold = settings.get().monitoring.notification_threshold.clone();
        let cleanup = settings.get().monitoring.cleanup_max_age_days;
        let close_to_tray = settings.get().monitoring.close_to_tray;
        let auto_briefing = settings.get().monitoring.auto_briefing_on_critical;
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            enabled,
            interval_minutes: interval,
            notification_threshold: threshold,
            cleanup_max_age_days: cleanup,
            close_to_tray,
            auto_briefing_on_critical: auto_briefing,
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
        let is_pro = matches!(license.tier.as_str(), "pro" | "team")
            || settings::is_trial_active(license);

        if is_pro {
            if minutes < 5 {
                return Err("Minimum monitoring interval is 5 minutes.".into());
            }
        } else if minutes < 30 {
            return Err(
                "Free tier minimum monitoring interval is 30 minutes. Upgrade to Pro for intervals as low as 5 minutes."
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
        let threshold = settings.get().monitoring.notification_threshold.clone();
        let cleanup = settings.get().monitoring.cleanup_max_age_days;
        let close_to_tray = settings.get().monitoring.close_to_tray;
        let auto_briefing = settings.get().monitoring.auto_briefing_on_critical;
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            enabled: state.is_enabled(),
            interval_minutes: minutes,
            notification_threshold: threshold,
            cleanup_max_age_days: cleanup,
            close_to_tray,
            auto_briefing_on_critical: auto_briefing,
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
        let cleanup = settings.get().monitoring.cleanup_max_age_days;
        let close_to_tray = settings.get().monitoring.close_to_tray;
        let auto_briefing = settings.get().monitoring.auto_briefing_on_critical;
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            enabled: state.is_enabled(),
            interval_minutes: state.get_interval() / 60,
            notification_threshold: threshold.clone(),
            cleanup_max_age_days: cleanup,
            close_to_tray,
            auto_briefing_on_critical: auto_briefing,
        });
    }

    info!(target: "4da::monitor", threshold = %threshold, "Notification threshold updated");

    Ok(serde_json::json!({
        "threshold": threshold,
        "message": format!("Notification threshold set to {}", threshold)
    }))
}

/// Test notification delivery
#[tauri::command]
pub async fn trigger_notification_test(app: AppHandle) -> Result<serde_json::Value> {
    monitoring::send_notification(&app, 3, 30);
    Ok(serde_json::json!({
        "success": true,
        "message": "Test notification sent"
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
            signal_priority: Some("high".to_string()),
        };
        assert_eq!(notification.title, "New Rust RFC");
        assert_eq!(notification.source_type, "hackernews");
        assert!(notification.score > 0.8);
        assert_eq!(notification.signal_priority, Some("high".to_string()));
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
