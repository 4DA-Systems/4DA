//! Monitoring Tauri commands for background analysis scheduling.
//!
//! Extracted from lib.rs. Provides commands for checking monitoring status,
//! enabling/disabling monitoring, and setting intervals.

use tauri::AppHandle;
use tracing::info;

use crate::{get_monitoring_state, get_settings_manager, monitoring, settings};

// ============================================================================
// Monitoring Commands (Phase 3)
// ============================================================================

/// Get monitoring status
#[tauri::command]
pub async fn get_monitoring_status() -> Result<serde_json::Value, String> {
    let state = get_monitoring_state();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let last_check = state.last_check.load(std::sync::atomic::Ordering::Relaxed);
    let secs_since_check = if last_check > 0 { now - last_check } else { 0 };

    Ok(serde_json::json!({
        "enabled": state.is_enabled(),
        "interval_secs": state.get_interval(),
        "interval_mins": state.get_interval() / 60,
        "is_checking": state.is_checking.load(std::sync::atomic::Ordering::Relaxed),
        "last_check": last_check,
        "secs_since_check": secs_since_check,
        "last_relevant_count": state.last_relevant_count.load(std::sync::atomic::Ordering::Relaxed),
        "total_checks": state.total_checks.load(std::sync::atomic::Ordering::Relaxed)
    }))
}

/// Enable or disable monitoring
#[tauri::command]
pub async fn set_monitoring_enabled(enabled: bool) -> Result<serde_json::Value, String> {
    let state = get_monitoring_state();
    state.set_enabled(enabled);

    if enabled {
        // Set last_check to 0 to trigger immediate check
        state
            .last_check
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    // Persist to settings
    {
        let mut settings = get_settings_manager().lock();
        let interval = state.get_interval() / 60;
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            enabled,
            interval_minutes: interval,
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
pub async fn set_monitoring_interval(minutes: u64) -> Result<serde_json::Value, String> {
    let state = get_monitoring_state();
    let secs = minutes * 60;
    state.set_interval(secs);

    // Persist to settings
    {
        let mut settings = get_settings_manager().lock();
        let _ = settings.set_monitoring_config(settings::MonitoringConfig {
            enabled: state.is_enabled(),
            interval_minutes: minutes,
        });
    }

    info!(target: "4da::monitor", interval_mins = minutes, "Interval persisted");

    Ok(serde_json::json!({
        "interval_mins": minutes,
        "interval_secs": secs
    }))
}

/// Test notification delivery
#[tauri::command]
pub async fn trigger_notification_test(app: AppHandle) -> Result<serde_json::Value, String> {
    monitoring::send_notification(&app, 3, 30);
    Ok(serde_json::json!({
        "success": true,
        "message": "Test notification sent"
    }))
}
