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

/// Process-lifetime holder for the single-instance lock. Set once in
/// `initialize_pre_tauri` and kept alive until process exit so the Drop impl
/// on `InstanceLock` fires during normal shutdown.
static INSTANCE_LOCK: once_cell::sync::OnceCell<
    parking_lot::Mutex<Option<crate::single_instance::InstanceLock>>,
> = once_cell::sync::OnceCell::new();

/// Crash-loop status captured during pre-Tauri init. Read by `setup_app`
/// after the `AppHandle` exists so we can emit frontend events.
static STARTUP_CRASH_STATUS: once_cell::sync::OnceCell<crate::startup_watchdog::CrashLoopStatus> =
    once_cell::sync::OnceCell::new();

/// Pre-Tauri correctness gate: acquire the single-instance lock and detect
/// crash loops. Split out of `initialize_pre_tauri` to keep that function
/// under file-size budget.
///
/// Single-instance: belt-and-braces with `tauri_plugin_single_instance` —
/// the plugin's callback handles "focus existing window" UX but runs only
/// after the Tauri builder is set up. This file lock rejects duplicate
/// launches BEFORE any DB open, preventing SQLite WAL corruption from two
/// processes racing on the same data directory. IO errors are non-fatal;
/// AlreadyRunning exits the process with a friendly message.
///
/// Crash-loop: reads the previous session's exit state (via `.running`
/// marker inspected in `begin_startup_watch`) and a persisted crash history.
/// If the recent crash rate breaches the Critical threshold, sets the
/// safe-mode flag and arms the `startup-crash-loop-critical` event for
/// later emission from `setup_app` once the AppHandle exists.
fn acquire_instance_and_detect_crash_loop() {
    use crate::state::get_db_path;

    let Some(dir) = get_db_path().parent().map(std::path::Path::to_path_buf) else {
        warn!(target: "4da::startup", "Could not resolve data dir for pre-Tauri correctness gates");
        return;
    };

    // Single-instance lock.
    match crate::single_instance::acquire_instance_lock(&dir) {
        Ok(lock) => {
            let cell = INSTANCE_LOCK.get_or_init(|| parking_lot::Mutex::new(None));
            *cell.lock() = Some(lock);
            info!(target: "4da::startup", "Single-instance lock acquired");
        }
        Err(crate::single_instance::InstanceError::AlreadyRunning(pid)) => {
            error!(target: "4da::startup", running_pid = pid,
                "Another 4DA instance is already running — this process will exit");
            eprintln!(
                "\n4DA is already running (pid {pid}).\n\
                 Check your system tray for the existing window, or wait a few seconds\n\
                 if you just closed it (lock cleanup takes a moment)."
            );
            std::process::exit(0);
        }
        Err(e) => {
            warn!(target: "4da::startup", error = %e,
                "Single-instance lock I/O error — continuing anyway (non-fatal)");
        }
    }

    // Crash-loop detection.
    let prev_crashed = crate::startup_watchdog::previous_session_crashed();
    let status = crate::startup_watchdog::check_crash_loop(&dir, prev_crashed);
    let _ = STARTUP_CRASH_STATUS.set(status);
}

/// Process-lifetime holder for the non-blocking file-log worker. Dropping it
/// flushes pending records and stops the worker thread. We stash it in a
/// `OnceCell` so `initialize_pre_tauri` can return without the guard going
/// out of scope (which would silently disable file logging).
static LOG_FILE_GUARD: once_cell::sync::OnceCell<tracing_appender::non_blocking::WorkerGuard> =
    once_cell::sync::OnceCell::new();

/// Pre-Tauri initialization: logging, threshold, database, context engine, source registry.
///
/// Must be called before `tauri::Builder` is constructed.
pub(crate) fn initialize_pre_tauri() {
    use crate::state::{
        get_context_dir, get_context_engine, get_relevance_threshold, get_source_registry,
    };
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{fmt, EnvFilter};

    // Initialize runtime paths BEFORE tracing so the file appender can use
    // data_dir. RuntimePaths::init emits one `info!` that predates subscriber
    // install — that one line is lost, but the paths are restamped in the
    // banner below.
    crate::runtime_paths::RuntimePaths::init();

    // Daily-rotated file appender under data_dir/logs. The WorkerGuard is
    // stashed in LOG_FILE_GUARD so records from background tasks flush during
    // shutdown.
    let log_dir = crate::log_retention::log_dir();
    let file_appender =
        tracing_appender::rolling::daily(&log_dir, crate::log_retention::LOG_FILE_STEM);
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);
    // OnceCell::set only errors if already set (cannot happen: runs exactly once).
    let _ = LOG_FILE_GUARD.set(file_guard);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // stderr layer: keeps developer-visible output (coloured in TTY).
    let stderr_layer = fmt::layer().with_writer(std::io::stderr);
    // file layer: ANSI off so log files stay grep-friendly.
    let file_layer = fmt::layer().with_ansi(false).with_writer(file_writer);

    // try_init returns Err if a subscriber is already installed (tests, repeat
    // runs via hot-reload). Ignore silently — file logging will be missing in
    // that edge case but the app still starts.
    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(stderr_layer)
        .with(file_layer)
        .try_init();

    // Prune log files older than the retention window (best-effort).
    crate::log_retention::cleanup_old_logs();

    // Install crash guard BEFORE any secrets are loaded — zeroizes sensitive
    // memory in the panic hook so crash dumps don't leak API keys.
    crate::crash_guard::install();

    // Sovereign Cold Boot — start the startup watchdog timer immediately.
    // This records the startup clock used by phase budget enforcement and
    // inspects crash-trail markers from the previous session.
    crate::startup_watchdog::begin_startup_watch();

    // Pre-Tauri correctness gates: single-instance lock + crash-loop detection.
    // Both are infallible-from-caller's-perspective (AlreadyRunning exits the
    // process immediately; everything else either succeeds or degrades
    // gracefully). See `acquire_instance_and_detect_crash_loop` for detail.
    acquire_instance_and_detect_crash_loop();

    // Verify binary integrity (code signature, size sanity, permissions).
    // Runs after crash guard so any panics are handled. Logs only — never blocks.
    crate::integrity::verify_integrity();

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

    // Sovereign Cold Boot — verify sqlite-vec ONCE per process here, before any
    // other code opens a connection. This eliminates ~200 redundant verification
    // log lines that previously appeared on every cold boot (one per call to
    // open_db_connection across 83 files / 224 callsites).
    crate::verify_sqlite_vec_once();

    // Initialize relevance threshold from ACE storage or default
    if let Ok(ace) = get_ace_engine() {
        if let Some(stored) = ace.get_stored_threshold() {
            set_relevance_threshold(stored);
            info!(target: "4da::startup", threshold = get_relevance_threshold(), "Loaded stored relevance threshold");
        } else {
            set_relevance_threshold(0.40);
            info!(target: "4da::startup", threshold = get_relevance_threshold(), "Relevance threshold (default)");
        }
    } else {
        set_relevance_threshold(0.40);
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
///
/// # Sovereign Cold Boot — phased startup
///
/// Even though `setup_app` is structurally one function, we instrument it
/// with explicit phase markers so the watchdog can enforce time budgets and
/// regressions show up as warnings in the cold-boot logs:
///
/// - **Phase 0** (first-light, target <500ms): tray, monitoring config,
///   sqlite-vec verify, scheduler hydration, briefing snapshot fetch
/// - **Phase 1** (essential services, target <2s): scheduler start,
///   immediate briefing trigger, ACE initialization
/// - **Phase 2** (background warmup, no budget): AWE sync, model registry,
///   re-embedding, CVE refresh — all fire-and-forget
///
/// Each phase logs `phase=N elapsed_ms=X` so you can grep for them in cold-boot
/// traces. If a phase exceeds its budget, the watchdog writes a `.stalled`
/// marker so the next launch can surface the regression.
pub(crate) fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let setup_began = std::time::Instant::now();
    info!(target: "4da::startup", "setup_app: phase 0 (essential services) begin");

    // Record app start time for diagnostics uptime tracking
    crate::diagnostics::record_start_time();

    // Surface crash-loop status to the frontend. The actual detection ran
    // in `initialize_pre_tauri`; here we only emit the event now that we
    // have an AppHandle. The frontend listens for:
    //   - startup-crash-loop-warning: show a dismissible banner
    //   - startup-crash-loop-critical: show full recovery UI; safe mode
    //     is already engaged on the backend (see startup_watchdog::is_safe_mode).
    if let Some(status) = STARTUP_CRASH_STATUS.get() {
        match *status {
            crate::startup_watchdog::CrashLoopStatus::Warning(count) => {
                let _ = app.emit(
                    "startup-crash-loop-warning",
                    serde_json::json!({
                        "recent_crashes": count,
                        "window_minutes": 5,
                    }),
                );
                info!(target: "4da::startup", count, "Emitted startup-crash-loop-warning");
            }
            crate::startup_watchdog::CrashLoopStatus::Critical(count) => {
                let _ = app.emit(
                    "startup-crash-loop-critical",
                    serde_json::json!({
                        "recent_crashes": count,
                        "window_seconds": 60,
                        "safe_mode": true,
                    }),
                );
                warn!(target: "4da::startup", count,
                    "Emitted startup-crash-loop-critical — running in safe mode");
            }
            crate::startup_watchdog::CrashLoopStatus::Normal => {}
        }
    }

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
            // Notify frontend so it can hide tray-related UI elements
            let _ = app.emit(
                "tray-unavailable",
                serde_json::json!({
                    "reason": "System tray not supported on this desktop environment",
                    "desktop": std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
                }),
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
            if let Err(e) = settings.set_monitoring_config(crate::settings::MonitoringConfig {
                launch_at_startup: Some(false),
                ..m
            }) {
                warn!(target: "4da::startup", error = %e, "Failed to persist default monitoring config");
            }
            info!(target: "4da::startup", "First run: launch_at_startup defaulted to false (opt-in)");
        }
    }

    // Validate license integrity (reset tier if no key present)
    crate::settings::validate_license_on_startup();

    // Check if embedding model has changed — trigger background re-embed if needed
    if let Ok(db) = get_database() {
        let needs_reembed = {
            let conn = db.conn.lock();
            crate::reembed::check_embedding_model_changed(&conn)
        };
        if needs_reembed {
            info!(target: "4da::startup", "Embedding model changed — launching background re-embed");
            tauri::async_runtime::spawn(async {
                crate::reembed::reembed_all_items().await;
            });
        }
    }

    // Sovereign Cold Boot — hydrate persisted scheduler timestamps BEFORE the
    // scheduler starts ticking. This is the fix for the cold-boot stampede:
    // jobs whose interval has not actually elapsed since last run will be
    // skipped on the first tick instead of all firing simultaneously.
    crate::scheduler_state::hydrate_from_db(&monitoring_state);

    // Sovereign Cold Boot — detect WHY this process was launched and adapt
    // the cold-boot grace period accordingly:
    //   - cold power-on / autostart: 90s grace (shared CPU with desktop boot)
    //   - user clicked icon:         30s grace (responsiveness expected)
    //   - process restart:           0s grace (persisted state already prevents stampede)
    {
        use tauri_plugin_autostart::ManagerExt;
        let autostart_enabled = app.handle().autolaunch().is_enabled().unwrap_or(false);
        crate::boot_context::detect_and_cache(autostart_enabled);
    }

    // Start background scheduler
    let app_handle = app.handle().clone();
    monitoring::start_scheduler(app_handle.clone(), monitoring_state.clone());

    // Sovereign Cold Boot — Phase 0 (essential services) complete.
    // From here on, all spawned tasks are background warmup that the user
    // doesn't need to wait for. The window can show, the briefing snapshot
    // can render, and Phase 1 work catches up silently.
    let phase0_ms = setup_began.elapsed().as_millis();
    info!(
        target: "4da::startup",
        phase = 0,
        elapsed_ms = phase0_ms,
        "setup_app: phase 0 (essential services) complete"
    );
    crate::startup_watchdog::mark_phase1_complete();

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
                    if crate::utils::validate_deep_link_url(&url) {
                        info!(target: "4da::deeplink", url = %url, "Deep-link received");
                        let _ = deep_link_handle.emit("deep-link-activate", url);
                    } else {
                        warn!(target: "4da::security", url = %url, "Rejected invalid deep-link URL");
                        if let Ok(db) = crate::get_database() {
                            db.log_security_event("deeplink_blocked", &url, "warning");
                        }
                    }
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

    // AWE: sync wisdom into context on startup (non-blocking, best-effort)
    // This ensures PASIFA scoring is informed by decision history from the first analysis.
    tauri::async_runtime::spawn(async {
        match crate::context_commands::sync_awe_wisdom().await {
            Ok(msg) => info!(target: "4da::awe", msg = %msg, "AWE wisdom synced on startup"),
            Err(e) => {
                warn!(target: "4da::awe", error = %e, "AWE startup sync failed (non-fatal — AWE may not be installed)")
            }
        }
    });

    // Intelligence Reconciliation Phase 13 — Auto-seed on first launch.
    // Runs the git decision miner against configured context_dirs to populate
    // AWE's Wisdom Graph with personal priors. Non-blocking, best-effort.
    // The curated corpus (Phase 8) is already compiled into the binary and
    // available via load_corpus() without a startup step.
    tauri::async_runtime::spawn(async {
        let repos = crate::get_context_dirs();
        if repos.is_empty() {
            info!(target: "4da::startup", "Git decision miner: no context dirs configured — skipping auto-seed");
            return;
        }
        let (decisions, summary) = crate::git_decision_miner::mine_many(&repos, 5, 200);
        if summary.decisions_found > 0 {
            info!(
                target: "4da::startup",
                repos = summary.repos_scanned,
                decisions = summary.decisions_found,
                confirmed = summary.confirmed,
                "Git decision miner: auto-seed complete"
            );
            let jsonl_path = std::env::temp_dir().join("awe_git_seeded.jsonl");
            let lines: Vec<String> = decisions
                .iter()
                .filter_map(|d| serde_json::to_string(d).ok())
                .collect();
            let _ = std::fs::write(&jsonl_path, lines.join("\n"));
        } else {
            info!(target: "4da::startup", "Git decision miner: no decisions found in {} repos", summary.repos_scanned);
        }
    });

    // One-time startup data cleanup: purge bloated tables that accumulate dead rows.
    // Non-blocking, non-fatal — runs in background to avoid slowing startup.
    tauri::async_runtime::spawn(async {
        if let Ok(conn) = crate::open_db_connection() {
            let mut total_deleted: usize = 0;

            // Purge stale worktree paths from project_dependencies.
            // Claude Code worktrees create ephemeral copies at .claude/worktrees/agent-*
            // which get scanned into project_dependencies but are deleted after the
            // agent completes. These stale paths cause the Preemption page to show
            // "AFFECTED PROJECTS: D:\4DA\.claude\worktrees\agent-a0249898\..." noise.
            match conn.execute(
                "DELETE FROM project_dependencies WHERE project_path LIKE '%worktrees%agent-%'",
                [],
            ) {
                Ok(n) => { total_deleted += n; if n > 0 { info!(target: "4da::startup", deleted = n, "Startup cleanup: stale worktree project dependencies"); } }
                Err(e) => { warn!(target: "4da::startup", error = %e, "Startup cleanup: worktree deps failed"); }
            }

            // Purge superseded intelligence older than 7 days (98.7% are useless dead rows)
            match conn.execute(
                "DELETE FROM digested_intelligence WHERE superseded_by IS NOT NULL AND created_at < datetime('now', '-7 days')",
                [],
            ) {
                Ok(n) => { total_deleted += n; if n > 0 { info!(target: "4da::startup", deleted = n, "Startup cleanup: superseded intelligence"); } }
                Err(e) => { warn!(target: "4da::startup", error = %e, "Startup cleanup: digested_intelligence failed"); }
            }

            // Purge temporal_events older than 30 days
            match conn.execute(
                "DELETE FROM temporal_events WHERE created_at < datetime('now', '-30 days')",
                [],
            ) {
                Ok(n) => {
                    total_deleted += n;
                    if n > 0 {
                        info!(target: "4da::startup", deleted = n, "Startup cleanup: old temporal_events");
                    }
                }
                Err(e) => {
                    warn!(target: "4da::startup", error = %e, "Startup cleanup: temporal_events failed");
                }
            }

            // Purge file_signals older than 7 days
            match conn.execute(
                "DELETE FROM file_signals WHERE timestamp < datetime('now', '-7 days')",
                [],
            ) {
                Ok(n) => {
                    total_deleted += n;
                    if n > 0 {
                        info!(target: "4da::startup", deleted = n, "Startup cleanup: old file_signals");
                    }
                }
                Err(e) => {
                    warn!(target: "4da::startup", error = %e, "Startup cleanup: file_signals failed");
                }
            }

            // Keep only the most recent 500 sun_runs
            match conn.execute(
                "DELETE FROM sun_runs WHERE id NOT IN (SELECT id FROM sun_runs ORDER BY created_at DESC LIMIT 500)",
                [],
            ) {
                Ok(n) => { total_deleted += n; if n > 0 { info!(target: "4da::startup", deleted = n, "Startup cleanup: excess sun_runs"); } }
                Err(e) => { warn!(target: "4da::startup", error = %e, "Startup cleanup: sun_runs failed"); }
            }

            if total_deleted > 0 {
                info!(target: "4da::startup", total_deleted, "Startup data cleanup complete");
                // Checkpoint WAL to reclaim space. TRUNCATE for substantial deletes,
                // PASSIVE (non-blocking) for small cleanups. Previous threshold of 1000
                // was too high — even 100 deleted rows can produce meaningful WAL bloat.
                if total_deleted > 100 {
                    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
                    info!(target: "4da::startup", "WAL TRUNCATE checkpoint after startup cleanup");
                } else {
                    let _ = conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);");
                }
            }
        }
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

    // ========================================================================
    // Immediate Morning Briefing Check
    // ========================================================================
    // The scheduler ticks every 60 seconds, but we don't want the user to
    // wait up to a minute for their morning briefing on cold boot / autostart.
    // Fire an immediate check 3 seconds after startup (gives the briefing
    // window time to pre-warm its JS listener). This runs the same logic as
    // the scheduler tick: check → notify → synthesize → AWE daily jobs.
    {
        let briefing_handle = app_handle.clone();
        let briefing_state = get_monitoring_state().clone();
        tauri::async_runtime::spawn(async move {
            // Short delay: let briefing window JS register its listener
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            if let Some(briefing) =
                crate::monitoring_notifications::check_morning_briefing(&briefing_state)
            {
                info!(target: "4da::startup",
                    items = briefing.total_relevant,
                    "Immediate morning briefing (cold boot catch-up)"
                );

                // Sovereign Cold Boot — persist this briefing immediately so the
                // NEXT cold boot has it pre-baked. The snapshot will be re-saved
                // once the LLM synthesis completes (below) so the next-boot view
                // includes the narrative paragraph.
                crate::briefing_snapshot::save_snapshot(&briefing);

                crate::monitoring_notifications::send_morning_briefing_notification(
                    &briefing_handle,
                    &briefing,
                );
                // Emit to frontend for in-app briefing card
                let _ = briefing_handle.emit(
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
                );

                // Async LLM synthesis — narrative intelligence brief
                {
                    let app_synth = briefing_handle.clone();
                    let briefing_synth = briefing.clone();
                    tauri::async_runtime::spawn(async move {
                        match crate::monitoring_briefing::synthesize_morning_briefing(
                            &briefing_synth,
                        )
                        .await
                        {
                            Ok(synthesis) => {
                                info!(target: "4da::briefing", "Startup brief synthesis ready");
                                let _ =
                                    app_synth.emit_to("briefing", "briefing-synthesis", &synthesis);
                                let _ = app_synth.emit(
                                    "morning-briefing-synthesis",
                                    serde_json::json!({ "synthesis": synthesis }),
                                );

                                // Re-save the snapshot now that we have the synthesis text,
                                // so the next cold boot loads the briefing WITH narrative.
                                let mut enriched = briefing_synth.clone();
                                enriched.synthesis = Some(synthesis);
                                crate::briefing_snapshot::save_snapshot(&enriched);
                            }
                            Err(e) => {
                                info!(target: "4da::briefing", reason = %e, "Synthesis skipped");
                            }
                        }
                    });
                }

                // AWE behavioral wisdom synthesis
                {
                    let app_wisdom = briefing_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        match crate::awe_synthesis::build_behavioral_context() {
                            Ok(ctx) => {
                                if let Err(e) = crate::awe_synthesis::write_context_file(&ctx) {
                                    warn!(target: "4da::awe", error = %e, "Failed to write AWE context file");
                                }
                                match crate::awe_synthesis::synthesize_daily_wisdom(&ctx).await {
                                    Ok(wisdom) => {
                                        info!(target: "4da::awe_synthesis", "Startup wisdom synthesis ready");
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
                                warn!(target: "4da::awe_synthesis", error = %e, "Behavioral context failed");
                            }
                        }
                    });
                }

                // AWE daily jobs — wisdom sync + auto-feedback
                tauri::async_runtime::spawn(async {
                    if let Err(e) = crate::context_commands::sync_awe_wisdom().await {
                        warn!(target: "4da::awe", error = %e, "Startup AWE wisdom sync failed");
                    }
                });
                tauri::async_runtime::spawn(async {
                    if let Err(e) = crate::awe_commands::run_awe_auto_feedback().await {
                        warn!(target: "4da::awe", error = %e, "Startup AWE auto-feedback failed");
                    }
                });
            }
        });
    }

    // ========================================================================
    // Frontend Readiness Gate
    // ========================================================================
    // The main window starts hidden (visible: false in tauri.conf.json).
    //
    // Problem: In dev mode, the Vite dev server may not be ready when the
    // webview first navigates to devUrl, causing a "can't reach this page"
    // error. The React SplashScreen never mounts, so there's no recovery.
    //
    // Solution (3-layer defense):
    //  1. Window stays hidden until we KNOW the frontend loaded
    //  2. Frontend emits "frontend-ready" on mount → Rust shows window
    //  3. Dev-mode watchdog polls the dev server, force-reloads webview
    //     once it responds, then shows window as fallback
    //  4. Production: show window immediately (frontend is bundled)
    //
    // This guarantees the user never sees a broken error page on startup.
    // ========================================================================

    #[cfg(not(debug_assertions))]
    {
        // Production: frontend is bundled in the binary — always available.
        // Show the window immediately; the SplashScreen provides visual
        // feedback while backend initialization completes.
        if let Some(w) = app_handle.get_webview_window("main") {
            let _ = w.show();
            let _ = w.set_focus();
        }
        info!(target: "4da::startup", "Main window shown (production mode)");
        // Sovereign Cold Boot — Phase 0 (first-light) is complete as soon
        // as the window is visible. Logs elapsed time and triggers the
        // stalled-marker write if we exceeded the budget.
        crate::startup_watchdog::mark_phase0_complete();
    }

    #[cfg(debug_assertions)]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let shown = Arc::new(AtomicBool::new(false));

        // Layer 1: Frontend signals readiness via event (happy path)
        {
            let show_handle = app_handle.clone();
            let shown_event = shown.clone();
            app.listen("frontend-ready", move |_| {
                if !shown_event.swap(true, Ordering::SeqCst) {
                    if let Some(w) = show_handle.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                    info!(target: "4da::startup", "Main window shown (frontend-ready signal)");
                    // Sovereign Cold Boot — Phase 0 complete on first window-visible.
                    crate::startup_watchdog::mark_phase0_complete();
                }
            });
        }

        // Layer 2: Dev server poll + webview navigate (recovery path)
        //
        // Why navigate() instead of eval("reload")?
        // WebView2 error pages (edge://network-error/) block JS execution.
        // Tauri's navigate() operates at the engine level — works regardless
        // of what the webview is currently displaying.
        //
        // Wave 7 enhancement: this loop now KEEPS RUNNING after the initial
        // navigation, polling the dev server periodically. If the user sees
        // a stale error page because the dev server was briefly unreachable
        // when the webview first tried to load, we re-navigate as soon as
        // the dev server responds. The previous behavior was to give up
        // after 30s, which left the user staring at a broken page forever.
        {
            let ready_handle = app_handle.clone();
            let shown_poll = shown.clone();
            tauri::async_runtime::spawn(async move {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(2))
                    .no_proxy()
                    .build()
                    .unwrap_or_default();

                let dev_url =
                    url::Url::parse("http://localhost:4444/").expect("hardcoded dev URL is valid");

                let mut window_shown_locally = false;
                let mut last_navigate_attempt: Option<std::time::Instant> = None;

                // Phase A: aggressive poll every 500ms for the first 30s.
                // Most cold boots resolve here (dev server is up within ~5s).
                for attempt in 1..=60u32 {
                    if shown_poll.load(Ordering::SeqCst) {
                        // Frontend-ready fired. The window is visible AND React
                        // mounted successfully — we're done with phase A.
                        window_shown_locally = true;
                        break;
                    }
                    if let Ok(r) = client.get(dev_url.as_str()).send().await {
                        if r.status().is_success() {
                            info!(target: "4da::startup", attempt, "Dev server ready — navigating webview");
                            if let Some(w) = ready_handle.get_webview_window("main") {
                                let _ = w.navigate(dev_url.clone());
                                last_navigate_attempt = Some(std::time::Instant::now());
                            }
                            // Give the navigation up to 2s to trigger frontend-ready.
                            // main.tsx emits frontend-ready before React mounts
                            // (~300-500ms), so 2s is generous.
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            break;
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }

                // Phase A fallback: show window regardless (better than invisible forever)
                if !shown_poll.swap(true, Ordering::SeqCst) {
                    if let Some(w) = ready_handle.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                    warn!(target: "4da::startup", "Main window shown (dev server poll fallback)");
                    crate::startup_watchdog::mark_phase0_complete();
                    window_shown_locally = true;
                }

                // Phase B: persistent recovery loop (Wave 7).
                // The window is visible but `frontend-ready` may not have fired
                // — webview could be on an error page. Poll the dev server every
                // 3s for the next 5 minutes. Whenever we successfully reach the
                // dev server AND the frontend still hasn't signaled ready, force
                // a navigate. This rescues users from the "stale error page"
                // scenario in screenshot 1900.
                if window_shown_locally {
                    let frontend_ready_listener = ready_handle.clone();
                    let frontend_did_ready = Arc::new(AtomicBool::new(false));
                    let did_ready_clone = frontend_did_ready.clone();
                    frontend_ready_listener.listen("frontend-ready", move |_| {
                        did_ready_clone.store(true, Ordering::SeqCst);
                    });

                    let recovery_began = std::time::Instant::now();
                    let recovery_max = std::time::Duration::from_secs(300); // 5 min ceiling
                    let mut consecutive_navigates = 0_u32;
                    while recovery_began.elapsed() < recovery_max {
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

                        if frontend_did_ready.load(Ordering::SeqCst) {
                            debug!(target: "4da::startup", "Recovery loop: frontend-ready fired, exiting");
                            break;
                        }

                        // Frontend still hasn't signaled ready. Probe the dev server.
                        let server_up = client
                            .get(dev_url.as_str())
                            .send()
                            .await
                            .map(|r| r.status().is_success())
                            .unwrap_or(false);

                        if server_up {
                            // Avoid hammering the webview: only re-navigate if the
                            // last attempt was at least 5 seconds ago.
                            let should_navigate = match last_navigate_attempt {
                                Some(t) => t.elapsed() >= std::time::Duration::from_secs(5),
                                None => true,
                            };
                            if should_navigate {
                                consecutive_navigates += 1;
                                warn!(
                                    target: "4da::startup",
                                    consecutive_navigates,
                                    "Recovery: dev server reachable but frontend not ready — re-navigating webview"
                                );
                                if let Some(w) = ready_handle.get_webview_window("main") {
                                    let _ = w.navigate(dev_url.clone());
                                    last_navigate_attempt = Some(std::time::Instant::now());
                                }
                                // After 3 consecutive failed re-navigates, log a
                                // hard error so it shows up in diagnostics.
                                if consecutive_navigates >= 3 {
                                    warn!(
                                        target: "4da::startup",
                                        "Recovery: 3 consecutive re-navigates failed — frontend may be broken"
                                    );
                                }
                            }
                        }
                    }
                    debug!(target: "4da::startup", "Recovery loop exited");
                }
            });
        }
    }

    // Sovereign Cold Boot — start the steady-state heartbeat writer.
    // Writes data/.healthy every 60s so the frontend can detect a frozen
    // backend via a future IPC command. Also write one immediately so the
    // first heartbeat check doesn't have to wait a minute.
    crate::startup_watchdog::write_heartbeat();
    crate::startup_watchdog::start_heartbeat();

    // ========================================================================
    // Calibration Fitter Scheduler (Intelligence Mesh Phase 5b.3)
    // ========================================================================
    // The Filter (calibration_fitter) is authored to be idempotent and
    // cheap — pure local SQL + bucketing, no LLM or network. Running it
    // daily keeps the per-model calibration curves current without user
    // effort. Manual trigger (`fit_calibration_curves_now` Tauri command)
    // remains available from the UI for users who want to force a refit.
    //
    // Boot grace: 5 minutes. Long enough that cold-boot noise (DB
    // migrations, ACE scanning, briefing synthesis) doesn't race with
    // the first fit, short enough that users who open + close the app
    // quickly still get a fit within their session.
    //
    // Cadence: 24h between ticks. Calibration curves at 50-sample
    // minimum don't meaningfully move at sub-daily resolution; hourly
    // ticks would waste cycles for no user-visible benefit.
    //
    // Gating: if no (model, task) pair has enough unprocessed samples,
    // the fitter reports "skipped" per pair and writes no curves — the
    // scheduler logs the zero-curve outcome at debug and moves on.
    // There's no separate "is it worth running?" pre-check because
    // fit_calibration_curves_now already self-gates cheaply.
    {
        let app_for_cal = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            // Boot grace: let the rest of startup finish before spinning
            // up the first fit. Prevents lock contention on DB during the
            // first 5 minutes when other tasks are aggressively reading.
            tokio::time::sleep(std::time::Duration::from_secs(5 * 60)).await;

            let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 3600));
            // First tick: fires immediately after the sleep (tokio default).
            // Subsequent ticks: aligned 24h apart. If the app is suspended
            // mid-tick, Tokio catches up on wake — we don't over-fit
            // because each sample only processes once (processed_at flip).
            loop {
                interval.tick().await;
                match crate::calibration_commands::fit_calibration_curves_now().await {
                    Ok(report) => {
                        if report.curves_produced > 0 {
                            info!(
                                target: "4da::calibration",
                                candidates = report.total_candidates,
                                curves_produced = report.curves_produced,
                                "Scheduled calibration fit produced curves"
                            );
                            // Notify UI that curves refreshed so any
                            // open receipts panel can reload.
                            if let Err(e) = tauri::Emitter::emit(
                                &app_for_cal,
                                "calibration-curves-updated",
                                &report,
                            ) {
                                debug!(
                                    target: "4da::calibration",
                                    error = %e,
                                    "Failed to emit calibration-curves-updated (non-fatal)"
                                );
                            }
                        } else {
                            debug!(
                                target: "4da::calibration",
                                candidates = report.total_candidates,
                                "Scheduled calibration fit produced no curves"
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            target: "4da::calibration",
                            error = %e,
                            "Scheduled calibration fit failed (non-fatal)"
                        );
                    }
                }
            }
        });
    }

    // Sovereign Cold Boot — setup_app fully done. All remaining work is
    // either background tasks or scheduler ticks. The user should already
    // have a window with the cached briefing visible at this point.
    let total_ms = setup_began.elapsed().as_millis();
    info!(
        target: "4da::startup",
        phase = "setup_app",
        elapsed_ms = total_ms,
        "setup_app: complete (background tasks continuing in spawned futures)"
    );

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

        // Sovereign Cold Boot — remove the startup-watchdog markers so the
        // next launch knows this was a clean shutdown (no crash recovery toast).
        crate::startup_watchdog::mark_clean_shutdown();

        // Sovereign Cold Boot — capture the latest in-memory briefing to disk
        // so the NEXT cold boot can render it as the first paint. This is the
        // shutdown half of the briefing snapshot system; the steady-state half
        // persists snapshots whenever a new briefing fires.
        //
        // Best-effort: a missing/empty snapshot just means the next boot shows
        // its normal first-run state.
        {
            let analysis_state = crate::get_analysis_state().lock();
            if let Some(ref results) = analysis_state.results {
                use crate::monitoring_briefing::{BriefingItem, BriefingNotification};
                let user_lang = crate::i18n::get_user_language();
                let items: Vec<BriefingItem> = results
                    .iter()
                    .filter(|r| r.relevant && !r.excluded)
                    .filter(|r| r.detected_lang == user_lang)
                    .filter(|r| r.top_score >= 0.15)
                    .take(8)
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
                    .collect();
                if !items.is_empty() {
                    let total_relevant =
                        results.iter().filter(|r| r.relevant && !r.excluded).count();
                    let briefing = BriefingNotification {
                        title: format!("Brief — {} signals", total_relevant),
                        items,
                        total_relevant,
                        ongoing_topics: vec![],
                        knowledge_gaps: vec![],
                        escalating_chains: vec![],
                        wisdom_signals: vec![],
                        synthesis: None,
                        wisdom_synthesis: None,
                        preemption_alerts: vec![],
                        blind_spot_score: None,
                        labels: None,
                    };
                    drop(analysis_state); // release lock before disk I/O
                    crate::briefing_snapshot::save_snapshot(&briefing);
                    info!(target: "4da::shutdown", "Briefing snapshot persisted for next cold boot");
                }
            }
        }

        // Checkpoint WAL before exit — prevents large WAL persisting to next
        // cold boot. TRUNCATE resets the WAL file to zero length. Without this,
        // users face 100-275MB WAL recovery on every startup.
        if let Ok(db) = get_database() {
            match db
                .conn
                .lock()
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            {
                Ok(()) => info!(target: "4da::shutdown", "WAL checkpoint complete"),
                Err(e) => warn!(target: "4da::shutdown", error = %e, "WAL checkpoint failed"),
            }
        }

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
    let (needs_discovery, onboarding_done) = {
        let settings = get_settings_manager().lock();
        (
            settings.needs_auto_discovery(),
            settings.get().onboarding_complete,
        )
    };

    // Privacy: do NOT auto-scan directories before the user completes onboarding.
    // The user should explicitly add project directories during onboarding.
    // Auto-discovery only runs for returning users who have no context dirs configured.
    if needs_discovery && !onboarding_done {
        info!(target: "4da::startup", "First run — deferring ACE discovery until onboarding completes");
    } else if needs_discovery {
        info!(target: "4da::startup", "Post-onboarding run with no context dirs — running discovery");
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
            if let Err(e) = settings.mark_auto_discovery_completed() {
                warn!(target: "4da::ace", error = %e, "Failed to mark auto-discovery completed");
            }
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
                if let Err(e) = settings.mark_auto_discovery_completed() {
                    warn!(target: "4da::ace", error = %e, "Failed to mark auto-discovery completed");
                }
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
