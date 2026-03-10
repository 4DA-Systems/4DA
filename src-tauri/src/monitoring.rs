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
use tracing::{info, warn};

use crate::error::Result;

// Re-export notification types and functions so existing `monitoring::X` paths still work
pub use crate::monitoring_notifications::{
    complete_scheduled_check, drain_batched_notifications, send_notification, SignalSummary,
};

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
        // Clamp to [60, 86400] seconds (1 min to 24 hours) for safety
        let clamped = secs.clamp(60, 86400);
        self.interval_secs.store(clamped, Ordering::Relaxed);
    }
}

// ============================================================================
// System Tray
// ============================================================================

/// Set up the system tray with menu
pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> Result<TrayIcon<R>> {
    // Create menu items
    let show_item = MenuItem::with_id(app, "show", "Show 4DA Home", true, None::<&str>)?;
    let analyze_item = MenuItem::with_id(app, "analyze", "Analyze Now", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let monitoring_item = MenuItem::with_id(
        app,
        "toggle_monitoring",
        "Start Monitoring",
        true,
        None::<&str>,
    )?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

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
    )?;

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
                    if let Err(e) = app.emit("tray-analyze", ()) {
                        tracing::warn!("Failed to emit 'tray-analyze': {e}");
                    }
                }
                "toggle_monitoring" => {
                    // Emit event to toggle monitoring
                    if let Err(e) = app.emit("tray-toggle-monitoring", ()) {
                        tracing::warn!("Failed to emit 'tray-toggle-monitoring': {e}");
                    }
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
        .build(app)?;

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

            // Anomaly detection - every hour + anomaly bridge (Fix 5)
            let last_anomaly = state.last_anomaly_check.load(Ordering::Relaxed);
            if now - last_anomaly >= ANOMALY_CHECK_INTERVAL {
                state.last_anomaly_check.store(now, Ordering::Relaxed);
                match crate::run_background_anomaly_detection_with_results().await {
                    Ok(anomalies) => {
                        info!(target: "4da::monitor", found = anomalies.len(), "Anomaly detection completed");
                        // Fix 5: Process anomalies (notifications, auto-remediation, auto-briefing)
                        crate::monitoring_jobs::process_anomalies(&app, &anomalies).await;
                    }
                    Err(e) => {
                        warn!(target: "4da::monitor", error = %e, "Anomaly detection failed");
                    }
                }
            }

            // Decision window detection — runs with anomaly check (hourly).
            // Expire stale windows, detect new ones from recent content, compute advantage.
            if now - last_anomaly < 2 {
                // Only run when anomaly check just fired (within the same tick)
                if let Ok(conn) = crate::open_db_connection() {
                    let expired = crate::decision_advantage::expire_stale_windows(&conn);
                    let detected = crate::decision_advantage::detect_decision_windows(&conn);
                    if expired > 0 || !detected.is_empty() {
                        info!(
                            target: "4da::monitor",
                            expired, detected = detected.len(),
                            "Decision windows updated"
                        );
                        if let Err(e) = app.emit("decision-windows-updated", &detected) {
                            tracing::warn!("Failed to emit 'decision-windows-updated': {e}");
                        }
                    }
                    // Compute compound advantage score (weekly period)
                    let _ = crate::decision_advantage::compute_compound_score(&conn, "weekly");
                }
            }

            // Proactive chain prediction notifications — hourly
            // Sends OS notifications for chains in Escalating or Peak phase.
            crate::monitoring_jobs::maybe_notify_escalating_chains(&app);

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

                // Translation sync check - daily
                // Flags untranslated strings for the user's language as a sun alert.
                // Does NOT auto-translate -- just informs the user.
                {
                    let user_lang = crate::i18n::get_user_language();
                    if user_lang != "en" {
                        match crate::translation_pipeline::get_untranslated_keys(&user_lang) {
                            Ok(untranslated) if !untranslated.is_empty() => {
                                let msg = format!(
                                    "{} UI strings are not yet translated to {}. \
                                     Use Settings > Language to trigger translation.",
                                    untranslated.len(),
                                    user_lang
                                );
                                crate::suns::store_sun_alert("i18n_sync", "translation_gap", &msg);
                                info!(
                                    target: "4da::monitor",
                                    lang = %user_lang,
                                    missing = untranslated.len(),
                                    "Translation gap detected"
                                );
                            }
                            Ok(_) => {
                                info!(target: "4da::monitor", lang = %user_lang, "All strings translated");
                            }
                            Err(e) => {
                                warn!(target: "4da::monitor", error = %e, "Translation sync check failed");
                            }
                        }
                    }
                }

                // Open a shared connection for daily maintenance tasks
                if let Ok(daily_conn) = crate::open_db_connection() {
                    // Agent memory cleanup - runs with behavior decay (daily)
                    if let Err(e) = crate::agent_memory::cleanup_expired(&daily_conn) {
                        warn!(target: "4da::monitor", error = %e, "Agent memory cleanup failed");
                    }

                    let max_age_days = {
                        let sm = crate::get_settings_manager().lock();
                        sm.get().monitoring.cleanup_max_age_days.unwrap_or(30)
                    };

                    // Intelligence Metabolism: Autophagy-powered cleanup.
                    // Instead of blind DELETE, first extract meta-intelligence (calibration
                    // deltas, topic decay, source autopsies, anti-patterns) then prune.
                    {
                        match crate::autophagy::run_autophagy_cycle(
                            &daily_conn,
                            max_age_days as i64,
                        ) {
                            Ok(cycle) => {
                                info!(
                                    target: "4da::monitor",
                                    items_analyzed = cycle.items_analyzed,
                                    calibrations = cycle.calibrations_produced,
                                    anti_patterns = cycle.anti_patterns_detected,
                                    duration_ms = cycle.duration_ms,
                                    "Autophagy cycle completed"
                                );
                                // Emit event so frontend can refresh insights
                                if let Err(e) = app.emit("autophagy-cycle-complete", &cycle) {
                                    tracing::warn!(
                                        "Failed to emit 'autophagy-cycle-complete': {e}"
                                    );
                                }

                                // GAME: track calibrations produced
                                if cycle.calibrations_produced > 0 {
                                    if let Ok(db) = crate::get_database() {
                                        let _unlocked = crate::game_engine::increment_counter(
                                            db,
                                            "calibrations",
                                            cycle.calibrations_produced as u64,
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(target: "4da::monitor", error = %e, "Autophagy cycle failed");
                            }
                        }

                        // Bridge accuracy feedback from ACE behavior data into calibration
                        if let Ok(ace) = crate::state::get_ace_engine() {
                            let ace_conn = ace.get_conn().lock();
                            match crate::autophagy::bridge_accuracy_feedback(
                                &ace_conn,
                                &daily_conn,
                                max_age_days as i64,
                            ) {
                                Ok(count) if count > 0 => {
                                    info!(
                                        target: "4da::monitor",
                                        count,
                                        "Accuracy feedback bridged to calibration"
                                    );
                                }
                                Err(e) => {
                                    warn!(target: "4da::monitor", error = %e, "Accuracy feedback bridge failed");
                                }
                                _ => {}
                            }
                        }
                    }
                    // Still run the actual prune after autophagy extracted intelligence
                    if let Ok(db) = crate::get_database() {
                        match db.cleanup_old_items(max_age_days) {
                            Ok(deleted) if deleted > 0 => {
                                info!(target: "4da::monitor", deleted, max_age_days, "Pruned old source items (post-autophagy)");
                                if let Err(e) = db.vacuum_if_needed(deleted, 1000) {
                                    warn!(target: "4da::monitor", error = %e, "VACUUM after cleanup failed");
                                }
                            }
                            Ok(_) => {}
                            Err(e) => {
                                warn!(target: "4da::monitor", error = %e, "Database cleanup failed");
                            }
                        }
                    }
                }
            }

            // Digest scheduler (Fix 2) -- check on every tick
            crate::monitoring_jobs::maybe_generate_digest(&app).await;

            // Smart batching (Improvement E) -- save mini-digest when threshold reached
            crate::monitoring_jobs::maybe_save_mini_digest(&state);

            // Suns: tick all enabled suns
            {
                let mut registry = crate::suns_commands::get_sun_registry();
                let results = registry.tick();
                for (sun_id, result) in &results {
                    if result.success {
                        tracing::debug!(target: "4da::suns", sun = %sun_id, msg = %result.message, "Sun completed");
                    } else {
                        warn!(target: "4da::suns", sun = %sun_id, msg = %result.message, "Sun failed");
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
                crate::scoring::run_background_analysis(&bg_app, &bg_state).await;
            });
        }
    });
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
) -> Result<()> {
    let show_item = MenuItem::with_id(app, "show", "Show 4DA Home", true, None::<&str>)?;
    let analyze_item = MenuItem::with_id(app, "analyze", "Analyze Now", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;

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
    )?;

    let separator2 = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

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
    )?;

    tray.set_menu(Some(menu))?;

    // Update tooltip
    let tooltip = if monitoring_enabled {
        "4DA Home - Monitoring active"
    } else {
        "4DA Home - All signal. No feed."
    };
    tray.set_tooltip(Some(tooltip))?;

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
