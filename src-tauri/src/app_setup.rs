// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Application setup and startup initialization.
//!
//! Contains the Tauri `setup()` callback body and ACE startup logic,
//! extracted from lib.rs for maintainability.

use tauri::{Emitter, Listener, Manager};
use tracing::{debug, error, info, warn};

use crate::state::{
    get_ace_engine, get_context_dirs, get_database, get_monitoring_state, get_settings_manager,
    set_relevance_threshold,
};
use crate::{
    ace_commands, analysis, channel_render, events, monitoring, ollama, open_db_connection,
    signal_terminal, source_fetching::fill_cache_background, standing_queries, void_engine,
};

/// Pre-Tauri initialization: logging, threshold, database, context engine, source registry.
///
/// Must be called before `tauri::Builder` is constructed.
pub(crate) fn initialize_pre_tauri() {
    use crate::state::{
        get_context_dir, get_context_engine, get_relevance_threshold, get_source_registry,
    };

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!(target: "4da::startup", "========================================");
    info!(target: "4da::startup", "4DA Home - Personalized Intelligence");
    info!(target: "4da::startup", "All signal. No feed.");
    info!(target: "4da::startup", "========================================");

    // Clean up temp files from any crashed previous session
    if let Some(data_dir) = dirs::data_local_dir() {
        let temp_dir = data_dir.join("4da").join("temp");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
            info!(target: "4da::startup", "Cleaned stale temp directory");
        }
    }

    // Verify data directory is writable before attempting database operations
    {
        use crate::state::get_db_path;
        let db_path = get_db_path();
        if let Some(data_dir) = db_path.parent() {
            // Ensure directory exists
            if let Err(e) = std::fs::create_dir_all(data_dir) {
                error!(target: "4da::startup", path = ?data_dir, error = %e,
                    "FATAL: Cannot create data directory. Check file permissions.");
                // Show a native dialog on platforms that support it
                #[cfg(not(target_os = "linux"))]
                {
                    let msg = format!(
                        "4DA cannot create its data directory:\n{}\n\nError: {}\n\nPlease check file permissions and try again.",
                        data_dir.display(), e
                    );
                    // Use rfd (if available) or just eprintln for now
                    eprintln!("FATAL: {msg}");
                }
                #[cfg(target_os = "linux")]
                {
                    eprintln!(
                        "FATAL: 4DA cannot create data directory: {}\nError: {}\nPlease check permissions.",
                        data_dir.display(), e
                    );
                }
                std::process::exit(1);
            }
            // Test writability by creating and removing a temp file
            let test_file = data_dir.join(".4da_write_test");
            match std::fs::write(&test_file, b"test") {
                Ok(()) => {
                    let _ = std::fs::remove_file(&test_file);
                }
                Err(e) => {
                    error!(target: "4da::startup", path = ?data_dir, error = %e,
                        "FATAL: Data directory exists but is not writable.");
                    eprintln!(
                        "FATAL: 4DA data directory is not writable: {}\nError: {}\n\
                        Please check permissions or set FOURDA_DB_PATH to a writable location.",
                        data_dir.display(),
                        e
                    );
                    std::process::exit(1);
                }
            }
            info!(target: "4da::startup", path = ?data_dir, "Data directory verified writable");
        }
    }

    info!(target: "4da::startup", context_dir = ?get_context_dir(), "Context directory");
    info!(target: "4da::startup", model = "all-MiniLM-L6-v2", dimensions = 384, "Embedding model");

    // Initialize relevance threshold from ACE storage or default
    if let Ok(ace) = get_ace_engine() {
        if let Some(stored) = ace.get_stored_threshold() {
            set_relevance_threshold(stored);
            info!(target: "4da::startup", threshold = get_relevance_threshold(), "Loaded stored relevance threshold");
        } else {
            set_relevance_threshold(0.35);
            info!(target: "4da::startup", threshold = get_relevance_threshold(), "Relevance threshold (default)");
        }
    } else {
        set_relevance_threshold(0.35);
        info!(target: "4da::startup", threshold = get_relevance_threshold(), "Relevance threshold (default, ACE unavailable)");
    }

    // Initialize database early
    match get_database() {
        Ok(db) => {
            let ctx_count = db.context_count().unwrap_or(0);
            let item_count = db.total_item_count().unwrap_or(0);
            info!(target: "4da::startup", context_chunks = ctx_count, source_items = item_count, "Database ready");
        }
        Err(e) => {
            error!(target: "4da::startup", error = %e, "Database initialization failed");
        }
    }

    // Initialize context engine
    match get_context_engine() {
        Ok(engine) => {
            let interest_count = engine.interest_count().unwrap_or(0);
            let exclusion_count = engine.exclusion_count().unwrap_or(0);
            if let Ok(identity) = engine.get_static_identity() {
                let role_str = identity.role.as_deref().unwrap_or("Not set");
                info!(target: "4da::startup",
                    interests = interest_count,
                    exclusions = exclusion_count,
                    role = role_str,
                    "Context Engine ready"
                );
                if !identity.tech_stack.is_empty() {
                    debug!(target: "4da::startup", tech_stack = %identity.tech_stack.join(", "), "Tech Stack");
                }
                if !identity.domains.is_empty() {
                    debug!(target: "4da::startup", domains = %identity.domains.join(", "), "Domains");
                }
            }
        }
        Err(e) => {
            error!(target: "4da::startup", error = %e, "Context Engine initialization failed");
        }
    }

    // Initialize source registry
    let registry = get_source_registry();
    let (source_count, source_names) = {
        let reg = registry.lock();
        let count = reg.count();
        let names: Vec<String> = reg.sources().iter().map(|s| s.name().to_string()).collect();
        (count, names)
    };
    info!(target: "4da::startup", count = source_count, sources = %source_names.join(", "), "Sources registered");

    // Ensure plugins directory exists for Source Plugin API
    crate::plugins::loader::ensure_plugins_dir();

    // Run startup health self-check (fast, offline, infallible)
    let _startup_issues = crate::startup_health::run_startup_health_check();
}

/// Tauri `setup()` callback body.
///
/// Handles tray, monitoring, event listeners, background tasks, and ACE initialization.
pub(crate) fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Record app start time for diagnostics uptime tracking
    crate::diagnostics::record_start_time();

    // Start Signal Terminal HTTP server (requires Tokio runtime from Tauri)
    signal_terminal::start_signal_terminal();

    // Set up system tray (non-fatal: app works without tray)
    let tray = match monitoring::setup_tray(app.handle()) {
        Ok(tray) => {
            crate::capabilities::report_restored(crate::capabilities::Capability::SystemTray);
            Some(tray)
        }
        Err(e) => {
            warn!("System tray setup failed, continuing without tray: {e}");
            crate::capabilities::report_unavailable(
                crate::capabilities::Capability::SystemTray,
                "System tray not supported on this desktop environment",
                "The app works normally without a tray icon. Use the main window instead.",
            );
            None
        }
    };

    // Store tray handle for later updates
    app.manage(parking_lot::Mutex::new(tray));

    // Load monitoring settings from persistence
    let monitoring_state = get_monitoring_state().clone();
    {
        let settings = get_settings_manager().lock();
        let config = settings.get_monitoring_config();
        monitoring_state.set_enabled(config.enabled);
        monitoring_state.set_interval(config.interval_minutes * 60);
        info!(target: "4da::monitor", enabled = config.enabled, interval_mins = config.interval_minutes, "Loaded monitoring settings");
    }

    // First run: persist launch_at_startup as false (opt-in, not opt-out).
    // Users can enable in Settings > Monitoring. We don't auto-enroll in autostart
    // without explicit consent — that's a user-hostile pattern.
    {
        let should_persist = {
            let settings = get_settings_manager().lock();
            settings.get().monitoring.launch_at_startup.is_none()
        };
        if should_persist {
            let mut settings = get_settings_manager().lock();
            let m = settings.get().monitoring.clone();
            let _ = settings.set_monitoring_config(crate::settings::MonitoringConfig {
                launch_at_startup: Some(false),
                ..m
            });
            info!(target: "4da::startup", "First run: launch_at_startup defaulted to false (opt-in)");
        }
    }

    // Validate license integrity (reset tier if no key present)
    crate::settings::validate_license_on_startup();

    // Start background scheduler
    let app_handle = app.handle().clone();
    monitoring::start_scheduler(app_handle.clone(), monitoring_state.clone());

    // Start team sync scheduler (if configured)
    #[cfg(feature = "team-sync")]
    {
        let team_state = std::sync::Arc::new(crate::team_sync_scheduler::TeamSyncState::default());
        let settings = get_settings_manager().lock();
        if let Some(ref relay_cfg) = settings.get().team_relay {
            team_state.configure(relay_cfg);
            // Load team key from DB if available
            if let Ok(conn) = crate::state::open_db_connection() {
                if let Some(ref tid) = relay_cfg.team_id {
                    if let Ok(key_bytes) = conn.query_row(
                        "SELECT team_symmetric_key_enc FROM team_crypto WHERE team_id = ?1",
                        rusqlite::params![tid],
                        |row| row.get::<_, Vec<u8>>(0),
                    ) {
                        if key_bytes.len() == 32 {
                            let mut key = [0u8; 32];
                            key.copy_from_slice(&key_bytes);
                            *team_state.team_key.lock() = Some(key);
                        }
                    }
                }
            }
            info!(target: "4da::team_sync", enabled = relay_cfg.enabled, "Team sync config loaded");
        }
        drop(settings);
        crate::team_sync_scheduler::start_sync_scheduler(app_handle.clone(), team_state);
    }

    // Start enterprise retention enforcement scheduler (daily, fire-and-forget)
    #[cfg(feature = "enterprise")]
    crate::organization::start_retention_scheduler();

    // Listen for tray events
    let app_handle_analyze = app_handle.clone();
    app.listen("tray-analyze", move |_| {
        info!(target: "4da::tray", "Manual analysis triggered from tray");
        let _ = app_handle_analyze.emit("start-analysis-from-tray", ());
    });

    // Handle deep-link URLs (4da://activate?key=...)
    let deep_link_handle = app_handle.clone();
    app.listen("deep-link://new-url", move |event| {
        if let Some(urls) = event
            .payload()
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
        {
            // Payload is a JSON array of URL strings
            if let Ok(url_list) = serde_json::from_str::<Vec<String>>(&format!("[{urls}]")) {
                for url in url_list {
                    info!(target: "4da::deeplink", url = %url, "Deep-link received");
                    let _ = deep_link_handle.emit("deep-link-activate", url);
                }
            }
        }
    });

    let _app_handle_toggle = app_handle.clone();
    app.listen("tray-toggle-monitoring", move |_| {
        let state = get_monitoring_state();
        let new_enabled = !state.is_enabled();
        state.set_enabled(new_enabled);
        info!(target: "4da::monitor", enabled = new_enabled, "Monitoring toggled");
        // monitoring-toggled event available for future UI wiring
    });

    // Listen for scheduled analysis events
    // Uses cache-first approach: fetch to fill cache, then analyze cached items
    let app_handle_scheduled = app_handle.clone();
    app.listen("scheduled-analysis", move |_| {
        info!(target: "4da::monitor", "Scheduled analysis starting (cache-first)");
        let handle = app_handle_scheduled.clone();
        tauri::async_runtime::spawn(async move {
            run_scheduled_analysis(handle).await;
        });
    });

    // Pre-warm the custom notification window (hidden, ready for instant show)
    if let Err(e) = crate::notification_window::init_notification_window(app.handle()) {
        warn!(target: "4da::notify", error = %e, "Notification window pre-warm failed (will retry on first notification)");
    }

    // Pre-warm the briefing window (hidden, center-screen, ready for morning briefing)
    if let Err(e) = crate::briefing_window::init_briefing_window(app.handle()) {
        warn!(target: "4da::briefing", error = %e, "Briefing window pre-warm failed (will retry on first briefing)");
    }

    // Listen for notification-ready from the notification frontend
    app.listen("notification-ready", move |_| {
        crate::notification_window::mark_ready();
        info!(target: "4da::notify", "Notification window JS listener registered");
    });

    // Listen for briefing-ready from the briefing frontend
    app.listen("briefing-ready", move |_| {
        crate::briefing_window::mark_ready();
        info!(target: "4da::briefing", "Briefing window JS listener registered");
    });

    // Listen for notification-hidden from the notification frontend
    let app_handle_notif = app_handle.clone();
    app.listen("notification-hidden", move |_| {
        crate::notification_window::hide_notification(&app_handle_notif);
    });

    // Listen for notification-clicked from the notification frontend
    let app_handle_click = app_handle.clone();
    app.listen("notification-clicked", move |event| {
        let handle = app_handle_click.clone();
        // Extract optional item_id from the event payload
        let item_id = serde_json::from_str::<serde_json::Value>(event.payload())
            .ok()
            .and_then(|v| v.get("item_id")?.as_i64());
        tauri::async_runtime::spawn(async move {
            crate::notification_window::notification_clicked(handle, item_id).await;
        });
    });

    // Listen for briefing-hidden from the briefing frontend
    let app_handle_briefing_hide = app_handle.clone();
    app.listen("briefing-hidden", move |_| {
        crate::briefing_window::hide_briefing(&app_handle_briefing_hide);
    });

    // Listen for briefing-item-clicked from the briefing frontend
    let app_handle_briefing_click = app_handle.clone();
    app.listen("briefing-item-clicked", move |event| {
        let handle = app_handle_briefing_click.clone();
        let item_id = serde_json::from_str::<serde_json::Value>(event.payload())
            .ok()
            .and_then(|v| v.get("item_id")?.as_i64());
        tauri::async_runtime::spawn(async move {
            crate::briefing_window::briefing_item_clicked(handle, item_id).await;
        });
    });

    info!(target: "4da::tray", "System tray and monitoring initialized");

    // Ensure Ollama models are available and warm on startup
    {
        let settings = get_settings_manager().lock();
        let llm = &settings.get().llm;
        if llm.provider == "ollama" && !llm.model.is_empty() {
            let model = llm.model.clone();
            let base_url = llm
                .base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            let warm_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                ollama::ensure_models_available(&model, &base_url, &warm_handle).await;
            });
        }
    }

    // Validate license key against Keygen API (fire-and-forget, non-blocking)
    {
        let license_key = {
            let settings = get_settings_manager().lock();
            settings.get().license.license_key.clone()
        };
        if !license_key.is_empty() && !license_key.starts_with("4DA-") {
            // Only validate Keygen API keys at startup.
            // Self-signed 4DA- keys are verified locally on-demand (get_license_tier / validate_license).
            let current_tier = {
                let settings = get_settings_manager().lock();
                settings.get().license.tier.clone()
            };
            tauri::async_runtime::spawn(async move {
                info!(target: "4da::license", "Startup license validation (Keygen)");
                let result =
                    crate::settings::validate_license_key_keygen(&license_key, &current_tier).await;

                // Re-read FRESH tier from settings after async completes.
                // The user may have changed their license during the network call.
                let manager = get_settings_manager();
                let mut guard = manager.lock();
                let fresh_tier = guard.get().license.tier.clone();

                if result.tier == fresh_tier {
                    info!(target: "4da::license",
                        tier = %result.tier,
                        cached = result.cached,
                        detail = %result.detail,
                        "Startup license validation complete — tier matches"
                    );
                } else if result.tier != "free" && fresh_tier == "free" {
                    // Keygen says paid but local says free — upgrade (key was validated)
                    info!(target: "4da::license",
                        old_tier = %fresh_tier,
                        new_tier = %result.tier,
                        detail = %result.detail,
                        "Tier upgraded after startup Keygen validation"
                    );
                    guard.get_mut().license.tier = result.tier;
                    if let Err(e) = guard.save() {
                        warn!("Failed to save settings: {e}");
                    }
                } else {
                    // Keygen says free or different paid tier — log but DON'T downgrade.
                    // The save-time invariant will correct if needed; don't race with user actions.
                    info!(target: "4da::license",
                        fresh_tier = %fresh_tier,
                        keygen_tier = %result.tier,
                        detail = %result.detail,
                        "Startup Keygen result differs from current tier — not overwriting"
                    );
                }
            });
        }
    }

    // Refresh model registry (fire-and-forget, <=1x/24h)
    tauri::async_runtime::spawn(async {
        if let Err(e) = crate::model_registry::refresh_registry().await {
            debug!(target: "4da::registry", error = %e, "Model registry refresh failed (using cached/bundled)");
        }
    });

    // Emit initial void signal (shows current state to heartbeat)
    if let Ok(db) = get_database() {
        let mon = get_monitoring_state();
        let signal = void_engine::compute_signal(db, mon);
        void_engine::emit_if_changed(&app_handle, signal);
    }

    // Staleness timer: update void signal once per minute
    // This is the ONLY timer in the void engine - everything else is change-driven
    let app_handle_staleness = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Ok(db) = get_database() {
                let mon = get_monitoring_state();
                let signal = void_engine::tick_staleness(db, mon);
                void_engine::emit_if_changed(&app_handle_staleness, signal);
            }
        }
    });

    // Initialize ACE with configured directories (runs async in background)
    initialize_ace_on_startup(app.handle().clone());

    Ok(())
}

/// Handle the `.run()` event callback for hide-to-tray and shutdown cleanup.
pub(crate) fn handle_run_event(app_handle: &tauri::AppHandle, event: tauri::RunEvent) {
    // Hide-to-tray: intercept window close when enabled
    if let tauri::RunEvent::WindowEvent {
        event: tauri::WindowEvent::CloseRequested { api, .. },
        ..
    } = &event
    {
        let close_to_tray = {
            let settings = get_settings_manager().lock();
            let user_pref = settings.get().monitoring.close_to_tray;
            // On Linux with GNOME/Pantheon/Unity (no system tray support),
            // default to false to prevent the window from becoming unreachable.
            // Users who install a tray extension can explicitly enable this.
            #[cfg(target_os = "linux")]
            let default_value = {
                let desktop = std::env::var("XDG_CURRENT_DESKTOP")
                    .unwrap_or_default()
                    .to_uppercase();
                !desktop.contains("GNOME")
                    && !desktop.contains("PANTHEON")
                    && !desktop.contains("UNITY")
            };
            #[cfg(not(target_os = "linux"))]
            let default_value = true;
            user_pref.unwrap_or(default_value)
        };
        // Safety: if tray setup failed, never hide to tray (window becomes unreachable)
        let tray_available = app_handle
            .try_state::<parking_lot::Mutex<Option<tauri::tray::TrayIcon<tauri::Wry>>>>()
            .map(|state| state.lock().is_some())
            .unwrap_or(false);
        let close_to_tray = close_to_tray && tray_available;
        if close_to_tray {
            api.prevent_close();
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.hide();
                info!(target: "4da::tray", "Window hidden to tray (close_to_tray enabled)");
            }
        }
    }
    if let tauri::RunEvent::Exit = event {
        info!(target: "4da::shutdown", "Application shutting down - cleaning up...");
        // Disable monitoring to stop scheduler
        let state = get_monitoring_state();
        state.set_enabled(false);
        // Clean up temp extraction directory (cross-platform)
        if let Some(data_dir) = dirs::data_local_dir() {
            let temp_dir = data_dir.join("4da").join("temp");
            if temp_dir.exists() {
                let _ = std::fs::remove_dir_all(&temp_dir);
                info!(target: "4da::shutdown", "Cleaned up temp directory");
            }
        }
        info!(target: "4da::shutdown", "Cleanup complete");
    }
}

// ============================================================================
// Scheduled Analysis
// ============================================================================

/// Execute a scheduled analysis cycle (cache-first approach).
async fn run_scheduled_analysis(handle: tauri::AppHandle) {
    // Step 1: Fill cache with deep fetch (background, no UI blocking)
    info!(target: "4da::monitor", "Step 1: Filling cache with deep fetch...");
    if let Err(e) = fill_cache_background(&handle).await {
        warn!(target: "4da::monitor", error = %e, "Cache fill failed, continuing with existing cache");
    }

    // Step 2: Analyze cached content (INSTANT)
    info!(target: "4da::monitor", "Step 2: Analyzing cached content...");
    match analysis::analyze_cached_content_impl(&handle).await {
        Ok(results) => {
            let relevant_count = results.iter().filter(|r| r.relevant).count();

            // Build signal summary for notifications
            let signal_summary = build_signal_summary(&results);

            // Extract notification info before moving signal_summary
            let notification_info = signal_summary
                .as_ref()
                .map(|s| (s.critical_count, s.high_count));

            let state = get_monitoring_state();
            monitoring::complete_scheduled_check(
                &handle,
                state,
                relevant_count,
                results.len(),
                signal_summary,
            );

            // Pulse heartbeat for notification events
            match notification_info {
                Some((critical, _)) if critical > 0 => {
                    events::void_signal_notification(&handle, true, critical);
                }
                Some((_, high)) if high > 0 => {
                    events::void_signal_notification(&handle, false, high);
                }
                _ if relevant_count > 0 => {
                    events::void_signal_notification(&handle, false, relevant_count);
                }
                _ => {}
            }

            // Emit results to frontend if window is visible
            events::void_signal_analysis_complete(&handle, &results);
            let _ = handle.emit("analysis-complete", results);

            // Auto-render stale channels after each monitoring cycle
            tauri::async_runtime::spawn(async move {
                if let Err(e) = channel_render::auto_render_stale_channels().await {
                    warn!(target: "4da::channels", error = %e, "Channel auto-render failed");
                }
            });

            // Evaluate standing queries for Signal users
            if crate::settings::is_signal() {
                let standing_handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    if let Ok(conn) = crate::open_db_connection() {
                        let alerts = standing_queries::evaluate_standing_queries(&conn);
                        if !alerts.is_empty() {
                            let total_new: i64 = alerts.iter().map(|a| a.new_matches).sum();
                            let _ = standing_handle.emit("standing-query-matches", &alerts);
                            if total_new > 0 {
                                events::void_signal_notification(
                                    &standing_handle,
                                    false,
                                    total_new as usize,
                                );
                            }
                        }
                    }
                });
            }
        }
        Err(e) => {
            error!(target: "4da::monitor", error = %e, "Scheduled analysis failed");
            events::void_signal_error(&handle);
            let state = get_monitoring_state();
            state
                .is_checking
                .store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Build a signal summary from analysis results for notifications.
fn build_signal_summary(results: &[crate::SourceRelevance]) -> Option<monitoring::SignalSummary> {
    let critical_count = results
        .iter()
        .filter(|r| r.signal_priority.as_deref() == Some("critical"))
        .count();
    let high_count = results
        .iter()
        .filter(|r| r.signal_priority.as_deref() == Some("alert"))
        .count();
    let top_signal = results
        .iter()
        .filter(|r| r.signal_type.is_some())
        .max_by(|a, b| {
            let pa = match a.signal_priority.as_deref() {
                Some("critical") => 4u8,
                Some("alert") => 3,
                Some("advisory") => 2,
                _ => 1,
            };
            let pb = match b.signal_priority.as_deref() {
                Some("critical") => 4u8,
                Some("alert") => 3,
                Some("advisory") => 2,
                _ => 1,
            };
            pa.cmp(&pb).then_with(|| {
                a.top_score
                    .partial_cmp(&b.top_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        })
        .and_then(|r| Some((r.signal_type.clone()?, r.signal_action.clone()?)));
    let top_item_id = results
        .iter()
        .filter(|r| r.signal_type.is_some())
        .max_by(|a, b| {
            let pa = match a.signal_priority.as_deref() {
                Some("critical") => 4u8,
                Some("high") => 3,
                Some("medium") => 2,
                _ => 1,
            };
            let pb = match b.signal_priority.as_deref() {
                Some("critical") => 4u8,
                Some("high") => 3,
                Some("medium") => 2,
                _ => 1,
            };
            pa.cmp(&pb).then_with(|| {
                a.top_score
                    .partial_cmp(&b.top_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        })
        .map(|r| r.id as i64);
    if critical_count > 0 || high_count > 0 {
        Some(monitoring::SignalSummary {
            critical_count,
            high_count,
            top_signal,
            top_item_id,
        })
    } else {
        None
    }
}

// ============================================================================
// ACE Startup Initialization
// ============================================================================

/// Initialize ACE on startup with automatic context discovery.
///
/// This is the core of ACE AUTONOMY -- the system discovers context without manual configuration.
fn initialize_ace_on_startup(app_handle: tauri::AppHandle) {
    // Check if auto-discovery is needed (first run with no context dirs)
    let needs_discovery = {
        let settings = get_settings_manager().lock();
        settings.needs_auto_discovery()
    };

    if needs_discovery {
        info!(target: "4da::startup", "First run detected - running AUTONOMOUS context discovery");
        let _ = app_handle.emit(
            "ace-discovery-started",
            "Discovering your development context...",
        );

        // Phase 1: Discover common dev directories
        let discovered_dirs = crate::settings::discover_dev_directories();

        if discovered_dirs.is_empty() {
            warn!(target: "4da::startup", "No dev directories found. User will need to configure manually");
            // Mark as completed so we don't keep trying
            let mut settings = get_settings_manager().lock();
            let _ = settings.mark_auto_discovery_completed();
        } else {
            // Phase 2: Deep scan for actual project directories
            info!(target: "4da::startup", dirs = discovered_dirs.len(), "Scanning directories for projects");
            let project_dirs = crate::settings::find_project_directories(&discovered_dirs, 3);

            // Use discovered dev directories (or project dirs if we want more granular)
            // For now, use the top-level dev dirs to allow ACE scanner to find all projects
            let dirs_to_add = if project_dirs.len() > 50 {
                // Too many projects - use parent directories instead
                debug!(target: "4da::startup", projects = project_dirs.len(), "Too many projects, using parent directories");
                discovered_dirs
            } else if !project_dirs.is_empty() {
                debug!(target: "4da::startup", projects = project_dirs.len(), "Found projects");
                project_dirs
            } else {
                debug!(target: "4da::startup", "No projects found, using discovered directories");
                discovered_dirs
            };

            // Save discovered directories to settings
            {
                let mut settings = get_settings_manager().lock();
                if let Err(e) = settings.add_context_dirs(dirs_to_add.clone()) {
                    error!(target: "4da::startup", error = %e, "Failed to save discovered directories");
                }
                let _ = settings.mark_auto_discovery_completed();
            }

            let _ = app_handle.emit(
                "ace-discovery-complete",
                serde_json::json!({
                    "directories_found": dirs_to_add.len(),
                    "directories": dirs_to_add
                }),
            );
        }
    }

    // Now get all context directories (either pre-configured or just discovered)
    let context_dirs = get_context_dirs();

    if context_dirs.is_empty() {
        warn!(target: "4da::startup", "No context directories available, ACE will wait for configuration");
        crate::capabilities::report_degraded(
            crate::capabilities::Capability::AceContext,
            "No project directories configured",
            "Add project directories in Settings for personalized scoring",
        );
        return;
    }

    info!(target: "4da::startup", dirs = context_dirs.len(), "Initializing ACE");

    // Spawn async task for ACE initialization
    tauri::async_runtime::spawn(async move {
        // Small delay to let the app fully initialize
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let paths: Vec<String> = context_dirs
            .iter()
            .map(|p| p.display().to_string())
            .collect();

        // Run full scan - this builds the context profile AUTONOMOUSLY
        info!(target: "4da::startup", "Running AUTONOMOUS ACE context scan");
        match ace_commands::ace_full_scan(paths.clone()).await {
            Ok(result) => {
                info!(target: "4da::startup", result = %result, "ACE context scan complete");
                // Pulse the heartbeat to show context was discovered
                events::void_signal_context_change(&app_handle, 0.6);
            }
            Err(e) => {
                error!(target: "4da::startup", error = %e, "ACE scan failed");
            }
        }

        // AUTO-SEED: Populate interests from ACE-detected tech if interests are empty
        // This provides immediate value without requiring manual configuration
        if let Err(e) = ace_commands::auto_seed_interests_from_ace().await {
            warn!(target: "4da::startup", error = %e, "Auto-seeding interests failed (non-fatal)");
        }

        // CONTENT INTEGRITY: Auto-verify and clean personalized content data.
        // Removes non-display-worthy tech from tech_stack (e.g. ORMs like drizzle
        // that were incorrectly seeded) and detects phantom tech. Runs every startup.
        if let Ok(conn) = open_db_connection() {
            let report = crate::content_integrity::verify_content_integrity(&conn, true);
            if !report.passed {
                info!(
                    target: "4da::startup",
                    filtered = report.filtered_tech.len(),
                    phantoms = report.phantom_tech.len(),
                    corrected = report.auto_corrected,
                    "Content integrity auto-corrected issues"
                );
            }
        }

        // PASIFA: Index README files from discovered projects for semantic search
        // This makes discovered context contribute to embedding-based relevance
        debug!(target: "4da::startup", "Indexing README files from discovered projects");
        let indexed_count = ace_commands::index_discovered_readmes(&context_dirs).await;
        if indexed_count > 0 {
            info!(target: "4da::startup", count = indexed_count, "Indexed README files for semantic search");
            let _ = app_handle.emit(
                "ace-readme-indexed",
                serde_json::json!({
                    "count": indexed_count
                }),
            );
        }

        // Start file watcher for continuous context updates
        debug!(target: "4da::startup", "Starting ACE FileWatcher for continuous monitoring");
        match ace_commands::ace_start_watcher(paths).await {
            Ok(result) => {
                info!(target: "4da::startup", result = %result, "ACE FileWatcher started");
                // ace-watcher-started event available for future UI wiring
            }
            Err(e) => {
                warn!(target: "4da::startup", error = %e, "ACE FileWatcher failed");
            }
        }

        info!(target: "4da::startup", "ACE AUTONOMOUS initialization complete - context is now being built");
    });
}
