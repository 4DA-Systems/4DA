// Copyright (c) 2025-2026 4DA Systems. All rights reserved.
// Licensed under the Business Source License 1.1 (BSL-1.1). See LICENSE file.

use tauri::{Emitter, Listener, Manager};
use tracing::{debug, error, info, warn};

pub mod error;
mod types;

// Re-exports from types (preserves `use crate::TypeName` interface)
pub use types::{
    AnalysisState, AnalysisStatus, ContextFile, ContextSettings, EnhancedRelevance, LLMJudgment,
    RelevanceMatch, ScoreBreakdown, SourceRelevance,
};
pub(crate) use types::{GenericSourceItem, ANALYSIS_TIMEOUT_SECS};

mod commands;
mod embeddings;
mod events;
mod state;
mod utils;

// Re-export from embeddings
pub(crate) use embeddings::embed_texts;

// Re-exports from events
pub(crate) use events::{
    emit_progress, void_signal_analysis_complete, void_signal_cache_filled, void_signal_error,
    void_signal_fetching,
};

// Re-exports from utils (preserves `use crate::fn_name` interface)
pub(crate) use utils::{
    build_embedding_text, check_exclusions, chunk_text, cosine_similarity_with_norm,
    decode_html_entities, extract_topics, scrape_article_content, truncate_utf8, vector_norm,
};

// Re-exports from commands (pub background jobs called by monitoring scheduler)
pub use commands::{
    run_background_anomaly_detection, run_background_behavior_decay, run_background_health_check,
};

// Re-exports from state (preserves `use crate::accessor` interface)
pub(crate) use state::{
    get_ace_engine, get_ace_engine_mut, get_analysis_abort, get_analysis_state, get_context_dir,
    get_context_dirs, get_context_engine, get_database, get_monitoring_state,
    get_relevance_threshold, get_settings_manager, get_source_registry, invalidate_context_engine,
    open_db_connection, set_relevance_threshold, SUPPORTED_EXTENSIONS,
};

mod ace;
mod ace_commands;
mod analysis;
mod anomaly;
mod attention;
mod competing_tech;
mod content_commands;
mod content_dna;
mod content_quality;
mod context_commands;
mod context_engine;
mod db;
mod developer_dna;
mod digest;
mod digest_commands;
mod document_index;
mod domain_profile;
pub mod extractors;
mod handoff;
mod health;
mod health_commands;
mod job_queue;
mod job_queue_commands;
mod knowledge_decay;
mod llm;
mod monitoring;
mod monitoring_commands;
mod novelty;
mod ollama;
mod predictive;
mod project_health;
pub mod query;
mod reverse_relevance;
mod scoring;
pub(crate) mod scoring_config;
mod semantic_diff;
mod settings;
mod settings_commands;
mod signal_chains;
mod signals;
mod source_config;
mod source_fetching;
mod sources;
mod temporal;
mod tts;
mod void_commands;
mod void_engine;

use source_fetching::fill_cache_background;

// ============================================================================
// App Entry
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
    let source_count = registry.lock().count();
    let source_names: Vec<String> = registry
        .lock()
        .sources()
        .iter()
        .map(|s| s.name().to_string())
        .collect();
    info!(target: "4da::startup", count = source_count, sources = %source_names.join(", "), "Sources registered");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            // Context
            context_commands::get_context_files,
            context_commands::clear_context,
            context_commands::index_context,
            context_commands::index_project_readmes,
            context_commands::set_context_dirs,
            // Analysis
            analysis::run_deep_initial_scan,
            analysis::run_cached_analysis,
            analysis::get_analysis_status,
            analysis::cancel_analysis,
            // Settings
            settings_commands::get_settings,
            settings_commands::set_llm_provider,
            settings_commands::mark_onboarding_complete,
            settings_commands::set_rerank_config,
            settings_commands::test_llm_connection,
            settings_commands::check_ollama_status,
            settings_commands::pull_ollama_model,
            settings_commands::get_user_context,
            settings_commands::set_user_role,
            settings_commands::add_tech_stack,
            settings_commands::remove_tech_stack,
            settings_commands::add_interest,
            settings_commands::remove_interest,
            settings_commands::add_exclusion,
            settings_commands::remove_exclusion,
            settings_commands::record_interaction,
            settings_commands::get_context_stats,
            // Monitoring
            monitoring_commands::get_monitoring_status,
            monitoring_commands::set_monitoring_enabled,
            monitoring_commands::set_monitoring_interval,
            monitoring_commands::set_notification_threshold,
            monitoring_commands::trigger_notification_test,
            // ACE (frontend-used subset)
            ace_commands::ace_get_detected_tech,
            ace_commands::ace_get_active_topics,
            ace_commands::ace_full_scan,
            ace_commands::ace_auto_discover,
            ace_commands::ace_record_interaction,
            ace_commands::ace_get_topic_affinities,
            ace_commands::ace_get_anti_topics,
            ace_commands::ace_find_similar_topics,
            ace_commands::ace_embedding_status,
            ace_commands::ace_save_watcher_state,
            ace_commands::ace_get_rate_limit_status,
            ace_commands::ace_get_suggested_interests,
            ace_commands::ace_get_unresolved_anomalies,
            ace_commands::ace_detect_anomalies,
            ace_commands::ace_resolve_anomaly,
            ace_commands::ace_get_accuracy_metrics,
            ace_commands::ace_record_accuracy_feedback,
            ace_commands::ace_get_single_affinity,
            // Source config
            source_config::get_rss_feeds,
            source_config::set_rss_feeds,
            source_config::get_twitter_handles,
            source_config::set_twitter_handles,
            source_config::get_x_api_key,
            source_config::set_x_api_key,
            source_config::get_youtube_channels,
            source_config::set_youtube_channels,
            source_config::get_github_languages,
            source_config::set_github_languages,
            // Digest & Briefing
            digest_commands::get_digest_config,
            digest_commands::set_digest_config,
            digest_commands::generate_ai_briefing,
            digest_commands::get_latest_briefing,
            // Content
            commands::get_sources,
            commands::mcp_score_autopsy,
            commands::export_results,
            document_index::get_indexed_documents,
            document_index::get_document_content,
            document_index::search_documents,
            document_index::get_indexed_stats,
            document_index::natural_language_query,
            // Void Engine
            void_commands::get_void_signal,
            // Intelligence panels
            attention::get_attention_report,
            knowledge_decay::get_knowledge_gaps,
            signal_chains::get_signal_chains,
            signal_chains::resolve_signal_chain,
            project_health::get_project_health,
            developer_dna::get_developer_dna,
            developer_dna::export_developer_dna_markdown,
            // Audio & Handoff
            tts::generate_audio_briefing,
            tts::get_audio_briefing_status,
            handoff::generate_context_packet,
            // Predictive
            predictive::get_predicted_context,
            // Content (article reader, AI summaries, saved items)
            content_commands::get_item_content,
            content_commands::get_item_summary,
            content_commands::generate_item_summary,
            content_commands::get_saved_items,
            content_commands::remove_saved_item,
            // Source Health
            health_commands::get_source_health_status
        ])
        .setup(|app| {
            // Set up system tray
            let tray = monitoring::setup_tray(app.handle()).expect("Failed to set up system tray");

            // Store tray handle for later updates
            app.manage(std::sync::Mutex::new(Some(tray)));

            // Load monitoring settings from persistence
            let monitoring_state = get_monitoring_state().clone();
            {
                let settings = get_settings_manager().lock();
                let config = settings.get_monitoring_config();
                monitoring_state.set_enabled(config.enabled);
                monitoring_state.set_interval(config.interval_minutes * 60);
                info!(target: "4da::monitor", enabled = config.enabled, interval_mins = config.interval_minutes, "Loaded monitoring settings");
            }

            // Start background scheduler
            let app_handle = app.handle().clone();
            monitoring::start_scheduler(app_handle.clone(), monitoring_state.clone());

            // Listen for tray events
            let app_handle_analyze = app_handle.clone();
            app.listen("tray-analyze", move |_| {
                info!(target: "4da::tray", "Manual analysis triggered from tray");
                let _ = app_handle_analyze.emit("start-analysis-from-tray", ());
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
                            let signal_summary = {
                                let critical_count = results.iter()
                                    .filter(|r| r.signal_priority.as_deref() == Some("critical"))
                                    .count();
                                let high_count = results.iter()
                                    .filter(|r| r.signal_priority.as_deref() == Some("high"))
                                    .count();
                                let top_signal = results.iter()
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
                                            a.top_score.partial_cmp(&b.top_score)
                                                .unwrap_or(std::cmp::Ordering::Equal)
                                        })
                                    })
                                    .and_then(|r| {
                                        Some((
                                            r.signal_type.clone()?,
                                            r.signal_action.clone()?,
                                        ))
                                    });
                                if critical_count > 0 || high_count > 0 {
                                    Some(monitoring::SignalSummary {
                                        critical_count,
                                        high_count,
                                        top_signal,
                                    })
                                } else {
                                    None
                                }
                            };

                            let state = get_monitoring_state();
                            monitoring::complete_scheduled_check(
                                &handle,
                                state,
                                relevant_count,
                                results.len(),
                                signal_summary,
                            );
                            // Emit results to frontend if window is visible
                            void_signal_analysis_complete(&handle, &results);
                            let _ = handle.emit("analysis-complete", results);
                        }
                        Err(e) => {
                            error!(target: "4da::monitor", error = %e, "Scheduled analysis failed");
                            void_signal_error(&handle);
                            let state = get_monitoring_state();
                            state
                                .is_checking
                                .store(false, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                });
            });

            info!(target: "4da::tray", "System tray and monitoring initialized");

            // Ensure Ollama models are available and warm on startup
            {
                let settings = get_settings_manager().lock();
                let llm = &settings.get().llm;
                if llm.provider == "ollama" && !llm.model.is_empty() {
                    let model = llm.model.clone();
                    let base_url = llm.base_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string());
                    let warm_handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        ollama::ensure_models_available(&model, &base_url, &warm_handle).await;
                    });
                }
            }

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
                        let signal = void_engine::compute_signal(db, mon);
                        void_engine::emit_if_changed(&app_handle_staleness, signal);
                    }
                }
            });

            // Initialize ACE with configured directories (runs async in background)
            initialize_ace_on_startup(app.handle().clone());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                info!(target: "4da::shutdown", "Application shutting down - cleaning up...");
                // Disable monitoring to stop scheduler
                let state = get_monitoring_state();
                state.set_enabled(false);
                // Clean up temp extraction directory
                if let Ok(data_dir) = std::env::var("APPDATA") {
                    let temp_dir = std::path::PathBuf::from(data_dir).join("4da").join("temp");
                    if temp_dir.exists() {
                        let _ = std::fs::remove_dir_all(&temp_dir);
                        info!(target: "4da::shutdown", "Cleaned up temp directory");
                    }
                }
                info!(target: "4da::shutdown", "Cleanup complete");
            }
        });
}

// ============================================================================
// Startup Initialization
// ============================================================================

/// Initialize ACE on startup with automatic context discovery
/// This is the core of ACE AUTONOMY - the system discovers context without manual configuration
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
