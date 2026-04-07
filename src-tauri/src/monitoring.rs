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
#[allow(dead_code)] // Reason: signal_priority field only read in tests; other fields used in production
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
    /// Last accuracy recording timestamp (unix seconds)
    pub last_accuracy_check: AtomicU64,
    /// Items below notification threshold, batched for next briefing
    pub batched_items: parking_lot::Mutex<Vec<BatchedNotification>>,
    /// Last morning briefing date (YYYY-MM-DD) to avoid firing twice in one day
    pub last_morning_briefing_date: parking_lot::Mutex<Option<String>>,
    /// Last CVE scan timestamp (unix seconds)
    pub last_cve_scan: AtomicU64,
    /// Last dependency health check timestamp (unix seconds)
    pub last_dep_health_check: AtomicU64,
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
            last_accuracy_check: AtomicU64::new(0),
            batched_items: parking_lot::Mutex::new(Vec::new()),
            last_morning_briefing_date: parking_lot::Mutex::new(None),
            last_cve_scan: AtomicU64::new(0),
            last_dep_health_check: AtomicU64::new(0),
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
//
// Tuning rationale (2026-03-25):
// - Health check (5 min): lightweight probe, catches issues early — keep tight
// - Anomaly detection (1 hr): rare events, hourly is sufficient detection window
// - CVE scan (1 hr): advisory databases update hourly at best, 30 min was wasteful
// - DB maintenance (4 hr): WAL checkpoint + PRAGMA optimize — no need for hourly
// - Behavior decay (24 hr): daily granularity matches engagement patterns
// - Accuracy recording (7 days): weekly snapshot, sufficient for trend analysis
const HEALTH_CHECK_INTERVAL: u64 = 300; // 5 minutes
const ANOMALY_CHECK_INTERVAL: u64 = 3600; // 1 hour
const BEHAVIOR_DECAY_INTERVAL: u64 = 86400; // 24 hours (daily)
const ACCURACY_RECORD_INTERVAL: u64 = 604800; // 7 days
const CVE_SCAN_INTERVAL: u64 = 3600; // 1 hour (was 30 min — advisory DBs update hourly)
const DB_MAINTENANCE_INTERVAL: u64 = 3600; // 1 hour — WAL checkpoint + optimize (was 4h, too infrequent for startup spike recovery)
const DEP_HEALTH_INTERVAL: u64 = 21600; // 6 hours — dependency health check (Layer 5)

/// Sovereign Cold Boot — adaptive grace period.
///
/// On cold boot, the scheduler runs ZERO maintenance jobs for this many
/// seconds. The first minute (or two) belongs to the user: instant briefing,
/// instant window, instant interaction. Maintenance catches up afterward.
///
/// The default is 90 seconds for the conservative cold-boot case. The
/// scheduler tightens this to ~30s if it detects the app was launched by
/// user action (vs autostart) — see `boot_context::adapt_grace_secs`.
const COLD_BOOT_GRACE_SECS_DEFAULT: u64 = 90;

/// Helper: update an atomic AND persist the new timestamp to the
/// `scheduler_state` table. Best-effort persistence — failures are logged
/// but never crash the scheduler.
#[inline]
fn mark_job_complete(atomic: &AtomicU64, now: u64, job_name: &'static str) {
    atomic.store(now, Ordering::Relaxed);
    crate::scheduler_state::persist_run(job_name, now);
}

/// Start the background monitoring scheduler
pub fn start_scheduler<R: Runtime>(app: AppHandle<R>, state: Arc<MonitoringState>) {
    info!(target: "4da::monitor", "Starting background scheduler");
    info!(target: "4da::monitor",
        health_interval_min = HEALTH_CHECK_INTERVAL / 60,
        anomaly_interval_hr = ANOMALY_CHECK_INTERVAL / 3600,
        decay_interval_hr = BEHAVIOR_DECAY_INTERVAL / 3600,
        cold_boot_grace_s = COLD_BOOT_GRACE_SECS_DEFAULT,
        "Background job intervals configured"
    );

    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60)); // Check every minute
        let mut last_wake_time = std::time::Instant::now();
        let scheduler_started_at = std::time::Instant::now();

        loop {
            interval.tick().await;

            // ================================================================
            // Sovereign Cold Boot — grace period
            // ================================================================
            // For the first COLD_BOOT_GRACE_SECS_DEFAULT seconds after the
            // scheduler starts, run NO maintenance jobs at all. This guarantees
            // the user owns the CPU/IO/network for the first minute of every
            // cold boot, regardless of whether persisted timestamps think any
            // jobs are due. This is the second half of the stampede fix
            // (the first half is `scheduler_state::hydrate_from_db`).
            let cold_boot_elapsed = scheduler_started_at.elapsed().as_secs();
            if cold_boot_elapsed < COLD_BOOT_GRACE_SECS_DEFAULT {
                tracing::debug!(
                    target: "4da::monitor",
                    elapsed_s = cold_boot_elapsed,
                    grace_s = COLD_BOOT_GRACE_SECS_DEFAULT,
                    "Cold-boot grace period — deferring all maintenance"
                );
                continue;
            }

            // Power-aware scheduling: detect sleep/wake and stagger deferred jobs
            let elapsed_since_last = last_wake_time.elapsed();
            let likely_woke_from_sleep = elapsed_since_last > Duration::from_secs(120);
            if likely_woke_from_sleep {
                // System likely slept — stagger jobs to avoid CPU spike on wake
                info!(target: "4da::monitor", elapsed_secs = elapsed_since_last.as_secs(), "Detected wake from sleep — staggering deferred jobs");
                tokio::time::sleep(Duration::from_secs(10)).await;

                // Report network capability as potentially stale after sleep
                crate::capabilities::report_degraded(
                    crate::capabilities::Capability::SourceFetching,
                    "Network state uncertain after sleep/wake",
                    "Re-checking connectivity on next source fetch",
                );
            }
            last_wake_time = std::time::Instant::now();

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
                mark_job_complete(
                    &state.last_health_check,
                    now,
                    crate::scheduler_state::jobs::HEALTH_CHECK,
                );
                match crate::run_background_health_check().await {
                    Ok(result) => {
                        info!(target: "4da::monitor", result = %result, "Health check completed");
                    }
                    Err(e) => {
                        warn!(target: "4da::monitor", error = %e, "Health check failed");
                    }
                }
            }

            // Active Work Topic Embedding — every 5 min (piggyback on health check)
            // Embeds recent file-content topics that lack embeddings, so the scoring
            // pipeline gets semantic (not just keyword) work signals.
            if now - last_health < 2 {
                if let Ok(ace) = crate::get_ace_engine() {
                    if let Some(emb_service) = ace.embedding_service() {
                        let topics_to_embed: Vec<String> = {
                            let conn = ace.conn.lock();
                            conn.prepare(
                                "SELECT topic FROM active_topics
                                 WHERE source = 'file_content'
                                 AND last_seen > datetime('now', '-4 hours')
                                 AND embedding IS NULL
                                 ORDER BY weight DESC LIMIT 10",
                            )
                            .ok()
                            .and_then(|mut stmt| {
                                stmt.query_map([], |row| row.get::<_, String>(0))
                                    .ok()
                                    .map(|rows| rows.flatten().collect())
                            })
                            .unwrap_or_default()
                        };
                        if !topics_to_embed.is_empty() {
                            match emb_service.lock().embed_batch(&topics_to_embed) {
                                Ok(embeddings) => {
                                    let conn = ace.conn.lock();
                                    let mut embedded = 0usize;
                                    for (topic, emb) in
                                        topics_to_embed.iter().zip(embeddings.iter())
                                    {
                                        let blob =
                                            crate::ace::topic_embeddings::embedding_to_blob(emb);
                                        if conn.execute(
                                            "UPDATE active_topics SET embedding = ?1 WHERE topic = ?2",
                                            rusqlite::params![blob, topic],
                                        ).is_ok() {
                                            embedded += 1;
                                        }
                                    }
                                    if embedded > 0 {
                                        info!(target: "4da::monitor", count = embedded, "Embedded active work topics");
                                    }
                                }
                                Err(e) => {
                                    tracing::debug!(target: "4da::monitor", error = %e, "Active work embedding skipped");
                                }
                            }
                        }
                    }
                }
            }

            // Database maintenance — hourly WAL checkpoint + PRAGMA optimize
            // Prevents WAL bloat (139MB+ observed) and keeps query planner current
            let last_health_for_db = state.last_health_check.load(Ordering::Relaxed);
            if now - last_health_for_db < 2 && now > DB_MAINTENANCE_INTERVAL {
                // Piggyback on health check tick (runs every 5 min, but we only maintain hourly).
                // Sovereign Cold Boot — function-local statics LAST_MAINTENANCE and
                // LAST_VACUUM are hydrated from the persisted scheduler_state table on
                // first use via an atomic init flag. Without this, every cold boot would
                // re-fire VACUUM and DB maintenance because the static atomics start at 0.
                static LAST_MAINTENANCE: AtomicU64 = AtomicU64::new(0);
                static LAST_MAINTENANCE_HYDRATED: AtomicBool = AtomicBool::new(false);
                if !LAST_MAINTENANCE_HYDRATED.swap(true, Ordering::SeqCst) {
                    let persisted = crate::scheduler_state::get_persisted_timestamp(
                        crate::scheduler_state::jobs::DB_MAINTENANCE,
                    );
                    if persisted > 0 {
                        LAST_MAINTENANCE.store(persisted, Ordering::Relaxed);
                    }
                }

                let last_maint = LAST_MAINTENANCE.load(Ordering::Relaxed);
                if now - last_maint >= DB_MAINTENANCE_INTERVAL {
                    LAST_MAINTENANCE.store(now, Ordering::Relaxed);
                    crate::scheduler_state::persist_run(
                        crate::scheduler_state::jobs::DB_MAINTENANCE,
                        now,
                    );
                    if let Ok(db) = crate::get_database() {
                        match db.run_scheduled_maintenance() {
                            Ok(()) => {
                                info!(target: "4da::monitor", "Hourly DB maintenance completed (WAL checkpoint + optimize)");
                            }
                            Err(e) => {
                                warn!(target: "4da::monitor", error = %e, "DB maintenance failed");
                            }
                        }
                    }

                    // Prune stale embedding cache entries (>30 days old)
                    if let Ok(ace) = crate::get_ace_engine() {
                        if let Some(emb_service) = ace.embedding_service() {
                            if let Err(e) = emb_service.lock().prune_stale_embeddings() {
                                warn!(target: "4da::monitor", error = %e, "Embedding cache pruning failed");
                            }
                        }
                    }

                    // Weekly VACUUM to reclaim disk space after deletions
                    static LAST_VACUUM: AtomicU64 = AtomicU64::new(0);
                    static LAST_VACUUM_HYDRATED: AtomicBool = AtomicBool::new(false);
                    if !LAST_VACUUM_HYDRATED.swap(true, Ordering::SeqCst) {
                        let persisted = crate::scheduler_state::get_persisted_timestamp(
                            crate::scheduler_state::jobs::VACUUM,
                        );
                        if persisted > 0 {
                            LAST_VACUUM.store(persisted, Ordering::Relaxed);
                        }
                    }

                    let last_vac = LAST_VACUUM.load(Ordering::Relaxed);
                    const VACUUM_INTERVAL: u64 = 7 * 24 * 3600; // Weekly
                    if now - last_vac >= VACUUM_INTERVAL {
                        LAST_VACUUM.store(now, Ordering::Relaxed);
                        crate::scheduler_state::persist_run(
                            crate::scheduler_state::jobs::VACUUM,
                            now,
                        );
                        if let Ok(db) = crate::get_database() {
                            match db.conn.lock().execute_batch("VACUUM;") {
                                Ok(()) => {
                                    info!(target: "4da::monitor", "Weekly VACUUM completed — disk space reclaimed");
                                }
                                Err(e) => {
                                    warn!(target: "4da::monitor", error = %e, "Weekly VACUUM failed");
                                }
                            }
                        }
                    }
                }
            }

            // Anomaly detection - every hour + anomaly bridge (Fix 5)
            let last_anomaly = state.last_anomaly_check.load(Ordering::Relaxed);
            if now - last_anomaly >= ANOMALY_CHECK_INTERVAL {
                mark_job_complete(
                    &state.last_anomaly_check,
                    now,
                    crate::scheduler_state::jobs::ANOMALY_DETECTION,
                );
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

            // CVE scan — Developer Immune System (every 30 minutes)
            let last_cve = state.last_cve_scan.load(Ordering::Relaxed);
            if now - last_cve >= CVE_SCAN_INTERVAL {
                mark_job_complete(
                    &state.last_cve_scan,
                    now,
                    crate::scheduler_state::jobs::CVE_SCAN,
                );
                let cve_app = app.clone();
                tokio::spawn(async move {
                    crate::monitoring_jobs::run_cve_scan(&cve_app).await;
                });
            }

            // Dependency health check — every 6 hours (Layer 5)
            // Classifies dependency health from local DB data and creates
            // proactive decision windows for stale or vulnerable packages.
            let last_dep_health = state.last_dep_health_check.load(Ordering::Relaxed);
            if now - last_dep_health >= DEP_HEALTH_INTERVAL {
                mark_job_complete(
                    &state.last_dep_health_check,
                    now,
                    crate::scheduler_state::jobs::DEP_HEALTH,
                );
                match crate::run_dependency_health_check() {
                    Ok(health) => {
                        let actionable = health
                            .iter()
                            .filter(|h| {
                                !matches!(
                                    h.health_status,
                                    crate::dependency_health::HealthStatus::Healthy
                                        | crate::dependency_health::HealthStatus::Unknown
                                )
                            })
                            .count();
                        info!(
                            target: "4da::monitor",
                            total = health.len(),
                            actionable,
                            "Dependency health check completed"
                        );
                    }
                    Err(e) => {
                        warn!(target: "4da::monitor", error = %e, "Dependency health check failed");
                    }
                }
            }

            // Proactive chain prediction notifications — hourly
            // Sends OS notifications for chains in Escalating or Peak phase.
            crate::monitoring_jobs::maybe_notify_escalating_chains(&app);

            // Mini-autophagy: trigger early cycle when sufficient feedback accumulated
            // but no autophagy has run yet. Shortens time-to-first-calibration from days to hours.
            {
                if let Ok(db_conn) = crate::open_db_connection() {
                    let feedback_count: i64 = db_conn
                        .query_row("SELECT COUNT(*) FROM feedback", [], |r| r.get(0))
                        .unwrap_or(0);
                    let cycle_count: i64 = db_conn
                        .query_row("SELECT COUNT(*) FROM autophagy_cycles", [], |r| r.get(0))
                        .unwrap_or(0);

                    if feedback_count >= 20 && cycle_count == 0 {
                        info!(target: "4da::monitor", feedback_count, "Mini-autophagy: enough feedback for first cycle");
                        let max_age_days = {
                            let sm = crate::get_settings_manager().lock();
                            sm.get().monitoring.cleanup_max_age_days.unwrap_or(30)
                        };
                        match crate::autophagy::run_autophagy_cycle(&db_conn, max_age_days as i64) {
                            Ok(cycle) => {
                                info!(
                                    target: "4da::monitor",
                                    items_analyzed = cycle.items_analyzed,
                                    calibrations = cycle.calibrations_produced,
                                    "Mini-autophagy cycle completed (first-time)"
                                );
                                if let Err(e) = app.emit("autophagy-cycle-complete", &cycle) {
                                    tracing::warn!(
                                        "Failed to emit 'autophagy-cycle-complete': {e}"
                                    );
                                }
                            }
                            Err(e) => {
                                warn!(target: "4da::monitor", error = %e, "Mini-autophagy cycle failed");
                            }
                        }
                    }
                }
            }

            // Behavior decay - daily
            let last_decay = state.last_decay.load(Ordering::Relaxed);
            if now - last_decay >= BEHAVIOR_DECAY_INTERVAL {
                mark_job_complete(
                    &state.last_decay,
                    now,
                    crate::scheduler_state::jobs::BEHAVIOR_DECAY,
                );
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

                        // Bridge accuracy feedback from ACE behavior data into calibration.
                        // Uses a fresh connection for ACE data to avoid lock ordering violation
                        // (holding both ace_conn and daily_conn simultaneously could deadlock).
                        if let Ok(ace_read_conn) = crate::state::open_db_connection() {
                            match crate::autophagy::bridge_accuracy_feedback(
                                &ace_read_conn,
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

                        // ── Threshold auto-tuning ──────────────────────────────
                        // After autophagy + accuracy bridging, use calibration deltas
                        // to nudge the global relevance threshold.
                        //
                        // Guards: requires 50+ feedback signals and 2+ autophagy cycles
                        // to avoid premature adjustments. Clamps to [0.15, 0.65].
                        // Step size capped at ±0.03 per cycle to prevent oscillation.
                        {
                            let cycle_count: i64 = daily_conn
                                .query_row("SELECT COUNT(*) FROM autophagy_cycles", [], |r| {
                                    r.get(0)
                                })
                                .unwrap_or(0);
                            let feedback_count: i64 = daily_conn
                                .query_row("SELECT COUNT(*) FROM feedback", [], |r| r.get(0))
                                .unwrap_or(0);

                            if cycle_count >= 2 && feedback_count >= 50 {
                                let deltas = crate::autophagy::load_calibration_deltas(&daily_conn);
                                if !deltas.is_empty() {
                                    // Weighted mean of deltas: positive = under-scoring (lower threshold),
                                    // negative = over-scoring (raise threshold)
                                    let total_weight: f32 = deltas.len() as f32;
                                    let weighted_sum: f32 = deltas.values().sum();
                                    let mean_delta = weighted_sum / total_weight;

                                    // Scale: each 0.1 mean delta shifts threshold by 0.01
                                    let adjustment = (mean_delta * 0.1).clamp(-0.03, 0.03);

                                    if adjustment.abs() > 0.001 {
                                        let current = crate::get_relevance_threshold();
                                        let new_threshold =
                                            (current + adjustment).clamp(0.15, 0.65);

                                        if (new_threshold - current).abs() > 0.001 {
                                            crate::set_relevance_threshold(new_threshold);
                                            if let Ok(ace) = crate::state::get_ace_engine() {
                                                ace.store_threshold(new_threshold);
                                            }
                                            info!(
                                                target: "4da::monitor",
                                                old = format!("{:.3}", current),
                                                new = format!("{:.3}", new_threshold),
                                                mean_delta = format!("{:.4}", mean_delta),
                                                deltas = deltas.len(),
                                                feedback_count,
                                                "Relevance threshold auto-tuned"
                                            );
                                        }
                                    }
                                }
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

                        // Daily deep maintenance: clean superseded intelligence, temporal data, sun_runs
                        match db.run_maintenance(max_age_days as i64) {
                            Ok(result) => {
                                let total_deleted = result.deleted_intelligence
                                    + result.deleted_windows
                                    + result.deleted_cycles
                                    + result.deleted_necessity;
                                if total_deleted > 0 {
                                    info!(
                                        target: "4da::monitor",
                                        intelligence = result.deleted_intelligence,
                                        windows = result.deleted_windows,
                                        cycles = result.deleted_cycles,
                                        necessity = result.deleted_necessity,
                                        "Daily deep maintenance completed"
                                    );
                                }
                            }
                            Err(e) => {
                                warn!(target: "4da::monitor", error = %e, "Daily deep maintenance failed");
                            }
                        }

                        // Emit data health warning if DB is getting large
                        if let Ok(stats) = db.get_db_stats() {
                            let size_mb = stats.db_size_bytes as f64 / (1024.0 * 1024.0);
                            if size_mb > 500.0 || stats.source_items > 100_000 {
                                let _ = app.emit("data-health-warning", serde_json::json!({
                                    "size_mb": (size_mb * 10.0).round() / 10.0,
                                    "items": stats.source_items,
                                    "message": format!("Database is {:.0}MB with {} items — consider running a deep clean", size_mb, stats.source_items),
                                }));
                            }
                        }
                    }
                }
            }

            // Weekly accuracy + timeline recording
            let last_accuracy = state.last_accuracy_check.load(Ordering::Relaxed);
            if now - last_accuracy >= ACCURACY_RECORD_INTERVAL {
                mark_job_complete(
                    &state.last_accuracy_check,
                    now,
                    crate::scheduler_state::jobs::ACCURACY_RECORD,
                );
                if let Ok(db) = crate::get_database() {
                    let conn = db.conn.lock();
                    match crate::accuracy::record_weekly_accuracy(&conn) {
                        Ok(()) => info!(target: "4da::monitor", "Weekly accuracy recorded"),
                        Err(e) => {
                            warn!(target: "4da::monitor", error = %e, "Failed to record weekly accuracy");
                        }
                    }
                    match crate::temporal_graph::record_weekly_timeline(&conn) {
                        Ok(()) => {
                            info!(target: "4da::monitor", "Weekly timeline snapshot recorded");
                        }
                        Err(e) => {
                            warn!(target: "4da::monitor", error = %e, "Failed to record weekly timeline snapshot");
                        }
                    }
                }
            }

            // Digest scheduler (Fix 2) -- check on every tick
            crate::monitoring_jobs::maybe_generate_digest(&app).await;

            // Smart batching (Improvement E) -- save mini-digest when threshold reached
            crate::monitoring_jobs::maybe_save_mini_digest(&state);

            // Morning briefing notification — fires once per day at the configured time
            if let Some(briefing) = crate::monitoring_notifications::check_morning_briefing(&state)
            {
                info!(target: "4da::monitor", items = briefing.total_relevant, "Morning briefing triggered");
                crate::monitoring_notifications::send_morning_briefing_notification(
                    &app, &briefing,
                );
                // Emit event to frontend so it can show an in-app briefing card
                if let Err(e) = app.emit(
                    "morning-briefing-ready",
                    serde_json::json!({
                        "title": briefing.title,
                        "total_relevant": briefing.total_relevant,
                        "items": briefing.items.iter().map(|i| serde_json::json!({
                            "title": i.title,
                            "source_type": i.source_type,
                            "score": i.score,
                            "signal_type": i.signal_type,
                        })).collect::<Vec<_>>(),
                    }),
                ) {
                    tracing::warn!("Failed to emit 'morning-briefing-ready': {e}");
                }

                // Async LLM synthesis — narrative intelligence brief
                {
                    let app_synth = app.clone();
                    let briefing_synth = briefing.clone();
                    tauri::async_runtime::spawn(async move {
                        match crate::monitoring_briefing::synthesize_morning_briefing(
                            &briefing_synth,
                        )
                        .await
                        {
                            Ok(synthesis) => {
                                info!(target: "4da::briefing", "Morning brief synthesis ready");
                                let _ =
                                    app_synth.emit_to("briefing", "briefing-synthesis", &synthesis);
                                let _ = app_synth.emit(
                                    "morning-briefing-synthesis",
                                    serde_json::json!({ "synthesis": synthesis }),
                                );
                            }
                            Err(e) => {
                                info!(target: "4da::briefing", reason = %e, "Synthesis skipped");
                            }
                        }
                    });
                }

                // AWE behavioral wisdom synthesis — personalized daily insight
                {
                    let app_wisdom = app.clone();
                    tauri::async_runtime::spawn(async move {
                        match crate::awe_synthesis::build_behavioral_context() {
                            Ok(ctx) => {
                                // Write context file for AWE CLI
                                let _ = crate::awe_synthesis::write_context_file(&ctx);
                                // Synthesize wisdom via LLM
                                match crate::awe_synthesis::synthesize_daily_wisdom(&ctx).await {
                                    Ok(wisdom) => {
                                        info!(target: "4da::awe_synthesis", "Daily wisdom synthesis ready");
                                        let _ = app_wisdom.emit(
                                            "awe-wisdom-synthesis",
                                            serde_json::json!({ "wisdom": wisdom }),
                                        );
                                    }
                                    Err(e) => {
                                        info!(target: "4da::awe_synthesis", reason = %e, "Wisdom synthesis skipped");
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(target: "4da::awe_synthesis", error = %e, "Failed to build behavioral context");
                            }
                        }
                    });
                }

                // AWE daily jobs — piggyback on morning briefing trigger (once per day)
                // 1. Sync AWE wisdom into context for PASIFA scoring
                tauri::async_runtime::spawn(async {
                    match crate::context_commands::sync_awe_wisdom().await {
                        Ok(msg) => {
                            info!(target: "4da::awe", msg = %msg, "Daily AWE wisdom sync complete")
                        }
                        Err(e) => {
                            warn!(target: "4da::awe", error = %e, "Daily AWE wisdom sync failed (non-fatal)")
                        }
                    }
                });
                // 2. Auto-infer feedback from git history
                tauri::async_runtime::spawn(async {
                    match crate::awe_commands::run_awe_auto_feedback().await {
                        Ok(msg) => {
                            info!(target: "4da::awe", msg = %msg, "Daily AWE auto-feedback complete")
                        }
                        Err(e) => {
                            warn!(target: "4da::awe", error = %e, "Daily AWE auto-feedback failed (non-fatal)")
                        }
                    }
                });
            }

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

            // Check if enough time has passed since last check.
            // Add 0-60s jitter to prevent predictable fetch timing (privacy hardening).
            let last = state.last_check.load(Ordering::Relaxed);
            let interval_secs = state.get_interval();
            let jitter = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
                % 60) as u64;

            if now - last < interval_secs + jitter {
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
#[allow(dead_code)] // Reason: dynamic tray menu updates not yet wired into monitoring loop
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
