// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

#![cfg_attr(test, allow(clippy::unwrap_used))]
// format_push_string: write!() is no clearer than push_str(&format!()) in digest/export code
#![allow(clippy::format_push_string)]
// cast_possible_wrap: usize→i64 casts are safe on our 64-bit targets (Windows/macOS/Linux)
#![allow(clippy::cast_possible_wrap)]
// ── Pre-existing style lints (381 occurrences) — suppress to unblock CI gate ──
// These are valid code-quality improvements to address incrementally, not correctness bugs.
// manual_let_else: Rust 1.65+ let-else syntax — will adopt incrementally
#![allow(clippy::manual_let_else)]
// match_same_arms: identical match arms — many are intentional for readability
#![allow(clippy::match_same_arms)]
// needless_pass_by_value: pass-by-value vs pass-by-ref — not a correctness issue
#![allow(clippy::needless_pass_by_value)]
// unnecessary_wraps: Result<T> return on fns that never fail — many are Tauri command signatures
#![allow(clippy::unnecessary_wraps)]
// unused_self: &self methods that don't use self — often for API consistency
#![allow(clippy::unused_self)]
// uninlined_format_args: format!("{}", x) vs format!("{x}") — cosmetic
#![allow(clippy::uninlined_format_args)]
// redundant_closure: |x| f(x) instead of f — some are clearer with the closure
#![allow(clippy::redundant_closure)]
#![allow(clippy::redundant_closure_for_method_calls)]
// unused_async: async fn with no await — many are Tauri command requirements
#![allow(clippy::unused_async)]
// Additional pre-existing style lints
#![allow(clippy::redundant_else)]
#![allow(clippy::inefficient_to_string)]
#![allow(clippy::nonminimal_bool)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::implicit_clone)]
#![allow(clippy::expect_used)]
#![allow(clippy::single_match_else)]
#![allow(clippy::if_not_else)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::ref_option)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::needless_continue)]
#![allow(clippy::unnested_or_patterns)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::use_debug)]
#![allow(clippy::needless_return)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::manual_filter)]
#![allow(clippy::manual_find_map)]
#![allow(clippy::doc_link_with_quotes)]
#![allow(clippy::large_types_passed_by_value)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::manual_strip)]
#![allow(clippy::float_cmp)]
#![allow(clippy::only_used_in_recursion)]
#![allow(clippy::borrow_as_ptr)]
#![allow(clippy::manual_clamp)]
#![allow(clippy::iter_filter_is_ok)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::struct_field_names)]
#![allow(clippy::manual_map)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::doc_lazy_continuation)]
// manual_char_is_ascii: only in Clippy nightly — suppress the unknown-lint error
#![allow(unknown_lints)]
#![allow(clippy::use_debug)]
#![allow(clippy::unnecessary_debug_formatting)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::duplicated_attributes)]
#![allow(clippy::unnecessary_map_or)]
#![allow(clippy::self_only_used_in_recursion)]
#![allow(clippy::ref_as_ptr)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::manual_pattern_char_comparison)]
#![allow(clippy::manual_flatten)]
#![allow(clippy::comparison_chain)]
#![allow(clippy::cloned_ref_to_slice_refs)]

use tauri::Manager;

pub mod error;
mod types;

// Re-exports from types (preserves `use crate::TypeName` interface)
pub use types::{
    AnalysisState, AnalysisStatus, ContextFile, ContextSettings, EnhancedRelevance, LLMJudgment,
    RelevanceMatch, ScoreBreakdown, SourceRelevance,
};
pub(crate) use types::{GenericSourceItem, ANALYSIS_TIMEOUT_SECS};

mod commands;
pub(crate) mod embedding_calibration;
mod embeddings;
mod events;
pub(crate) mod reembed;
pub mod state;
mod utils;

// Re-export from embeddings
pub(crate) use embeddings::embed_texts;

// Re-exports from events
pub(crate) use events::{
    emit_progress, void_signal_analysis_complete, void_signal_cache_filled, void_signal_error,
    void_signal_fetch_progress, void_signal_fetching,
};

// Re-exports from utils (preserves `use crate::fn_name` interface)
pub(crate) use utils::{
    build_embedding_text, check_exclusions, chunk_text, cosine_similarity_with_norm,
    decode_html_entities, detect_trend_topics, extract_topics, scrape_article_content,
    truncate_utf8, vector_norm,
};

// Re-exports from commands (pub background jobs called by monitoring scheduler)
pub use commands::{
    run_background_anomaly_detection, run_background_anomaly_detection_with_results,
    run_background_behavior_decay, run_background_health_check,
};

// Re-export dependency health background job (Layer 5 — 6-hour scheduler interval)
pub use dependency_health::run_dependency_health_check;

// Re-exports from state (preserves `use crate::accessor` interface)
pub(crate) use state::{
    get_ace_engine, get_ace_engine_mut, get_analysis_abort, get_analysis_state, get_context_dir,
    get_context_dirs, get_context_engine, get_database, get_llm_token_usage, get_monitoring_state,
    get_relevance_threshold, get_settings_manager, get_source_registry, invalidate_context_engine,
    open_db_connection, register_sqlite_vec_extension, set_relevance_threshold, try_get_database,
    verify_sqlite_vec_once, SUPPORTED_EXTENSIONS,
};

mod accuracy;
mod ace;
mod ace_commands;
mod adversarial;
mod agent_brief;
mod agent_memory;
mod ai_costs;
mod alert_triage;
mod analysis;
mod analysis_narration;
mod analysis_rerank;
mod anomaly;
mod app_setup;
mod attention;
mod autophagy;
// Wave 4 — adapt cold-boot grace period to launch context
mod autophagy_commands;
mod autophagy_pulse;
mod blind_spots;
mod boot_context;
mod briefing_snapshot;
mod briefing_window;
mod calibration;
mod calibration_commands;
mod calibration_fitter;
mod calibration_probes;
mod calibration_samples;
mod calibration_store;
pub mod capabilities;
mod channel_changelog;
mod channel_commands;
mod channel_provenance;
mod channel_render;
pub mod channels;
/// Intelligence Reconciliation Phase 11 — Commitment Contracts.
mod commitment_contracts;
mod community_intelligence;
mod competing_tech;
mod concept_graph;
mod content_analysis;
mod content_commands;
mod content_dna;
mod content_enrichment;
mod content_quality;
mod content_sophistication;
mod context_commands;
mod context_engine;
mod crash_guard;
mod data_export;
pub mod db;
mod decision_advantage;
mod decision_advantage_commands;
mod decision_signals;
mod decisions;
#[cfg(feature = "experimental")]
mod delegation;
#[cfg(not(feature = "experimental"))]
#[allow(dead_code)] // Feature-gated: stub active only when "experimental" is disabled
#[path = "delegation_stub.rs"]
mod delegation;
mod dependency_commands;
mod dependency_health;
mod developer_dna;
mod diagnostics;
mod digest;
mod digest_commands;
mod digest_config;
mod digest_email;
mod domain_profile;
mod domain_profile_data;
mod entity_extraction;
// Silent-Failure Defense Layer 1 — typed wrappers for external boundaries.
// See docs/strategy/SILENT-FAILURE-DEFENSE.md. Skeleton only in this commit.
#[cfg(feature = "experimental")]
mod achievement_commands;
#[cfg(not(feature = "experimental"))]
#[allow(dead_code)] // Feature-gated: stub active only when "experimental" is disabled
#[path = "achievement_commands_stub.rs"]
mod achievement_commands;
#[cfg(feature = "experimental")]
mod achievement_definitions;
#[cfg(not(feature = "experimental"))]
#[path = "achievement_definitions_stub.rs"]
mod achievement_definitions;
#[cfg(feature = "experimental")]
mod achievement_engine;
#[cfg(not(feature = "experimental"))]
#[path = "achievement_engine_stub.rs"]
mod achievement_engine;
mod briefing_dedupe;
mod briefing_groundedness;
#[cfg(test)]
mod briefing_pipeline_tests;
mod briefing_quality;
/// Intelligence Reconciliation (2026-04-16) — the canonical EvidenceItem type
/// that every intelligence surface emits after Phases 3-5. Read
/// `docs/strategy/INTELLIGENCE-RECONCILIATION.md` before touching intelligence.
mod evidence;
mod external;
pub mod extractors;
mod free_briefing;
/// Intelligence Reconciliation Phase 7 — Cold Start Layer 1.
mod git_decision_miner;
mod health;
mod health_commands;
mod http_client;
mod indexed_documents_commands;
mod integrity;
mod intelligence_core;
mod intelligence_history;
mod intelligence_metrics;
mod intelligence_packs;
mod ipc_guard;
mod ipc_rate_limit;
mod job_queue;
mod knowledge_decay;
pub(crate) mod language_detect;
mod llm;
pub(crate) mod llm_capability;
mod llm_judge;
mod llm_judgments;
mod llm_stream;
mod local_audit;
mod log_retention;
pub mod model_registry;
mod monitoring;
mod monitoring_briefing;
mod monitoring_commands;
mod monitoring_jobs;
mod monitoring_notifications;
mod natural_language_search;
mod notification_window;
mod novelty;
mod ollama;
mod osv;
mod plugin_commands;
pub mod plugins;
mod preemption;
mod probes_corpus;
mod probes_engine;
mod project_health;
mod project_health_dimensions;
mod prompt_safety;
pub mod provenance;
pub mod query;
mod reconciler;
pub(crate) mod runtime_paths;
mod scheduler_state;
mod scoring;
pub(crate) mod scoring_config;
mod search_synthesis;
/// Intelligence Reconciliation Phase 8 — Cold Start Layer 2 (curated corpus).
mod seed_corpus;
mod semantic_diff;
pub mod settings;
mod settings_commands;
mod signal_chains;
mod signal_terminal;
mod signal_terminal_events;
mod signal_terminal_pages;
mod signals;
mod source_config;
mod source_fetch_commands;
mod source_fetching;
pub(crate) mod source_tiers;
pub mod sources;
mod standing_queries;
mod standing_queries_evaluation;
mod standing_queries_suggestions;
mod startup_health;
// Wave 5 — universal startup watchdog with crash trail and heartbeat
mod startup_watchdog;
// Single-instance file lock (belt-and-braces with tauri_plugin_single_instance)
mod single_instance;
mod suns;
mod suns_commands;
mod tech_convergence;
mod tech_radar;
mod tech_radar_commands;
mod tech_radar_compute;
mod temporal;
mod temporal_graph;
mod topic_clustering;
mod url_validation;
mod void_commands;
mod void_engine;
#[allow(dead_code)]
mod waitlist;
mod weekly_digest;

mod stack_commands;
mod stack_health;
pub mod stacks;

pub mod taste_test;
mod taste_test_commands;

mod content_integrity;
mod content_personalization;
mod first_run_audit;
pub(crate) mod i18n;
mod playbook_commands;
mod sovereign_developer_profile;
mod sovereign_facts;
mod sovereign_profile;
mod streets_commands;
mod streets_engine;
mod streets_localization;
mod streets_suggestion;
mod template_data;
mod templates;
mod toolkit;
mod toolkit_export;
#[cfg(feature = "experimental")]
mod toolkit_http;
#[cfg(not(feature = "experimental"))]
#[allow(dead_code)] // Feature-gated: stub active only when "experimental" is disabled
#[path = "toolkit_http_stub.rs"]
mod toolkit_http;
// Team sync — encrypted metadata relay (AD-023)
// Gated: 17 commands with zero frontend callers. Enable with --features team-sync.
#[cfg(feature = "team-sync")]
mod team_intelligence;
#[cfg(feature = "team-sync")]
mod team_monitoring;
#[cfg(feature = "team-sync")]
mod team_notifications;
#[cfg(feature = "team-sync")]
mod team_sync;
#[cfg(feature = "team-sync")]
mod team_sync_commands;
#[cfg(feature = "team-sync")]
mod team_sync_crypto;
#[cfg(feature = "team-sync")]
mod team_sync_scheduler;
#[cfg(feature = "team-sync")]
mod team_sync_types;
#[cfg(not(feature = "team-sync"))]
#[path = "team_sync_types_stub.rs"]
mod team_sync_types;

// Stubs when team-sync is disabled (commands register but return errors)
#[cfg(not(feature = "team-sync"))]
#[path = "team_intelligence_stub.rs"]
mod team_intelligence;
#[cfg(not(feature = "team-sync"))]
#[path = "team_monitoring_stub.rs"]
mod team_monitoring;
#[cfg(not(feature = "team-sync"))]
#[path = "team_notifications_stub.rs"]
mod team_notifications;
#[cfg(not(feature = "team-sync"))]
#[path = "team_sync_commands_stub.rs"]
mod team_sync_commands;

// Enterprise: audit log, webhooks, organizations, analytics
// Gated: 15 commands with zero frontend callers. Enable with --features enterprise.
#[cfg(feature = "enterprise")]
mod audit;
#[cfg(feature = "enterprise")]
mod enterprise_analytics;
#[cfg(feature = "enterprise")]
mod organization;
#[cfg(feature = "enterprise")]
mod sso;
#[cfg(feature = "enterprise")]
mod sso_crypto;
#[cfg(feature = "enterprise")]
mod sso_xml;
#[cfg(feature = "enterprise")]
mod webhooks;

// Stubs when enterprise is disabled
#[cfg(not(feature = "enterprise"))]
#[path = "audit_stub.rs"]
mod audit;
#[cfg(not(feature = "enterprise"))]
#[path = "enterprise_analytics_stub.rs"]
mod enterprise_analytics;
#[cfg(not(feature = "enterprise"))]
#[path = "organization_stub.rs"]
mod organization;
#[cfg(not(feature = "enterprise"))]
#[path = "sso_stub.rs"]
mod sso;
#[cfg(not(feature = "enterprise"))]
#[path = "webhooks_stub.rs"]
mod webhooks;

mod content_translation;
mod content_translation_commands;
mod telemetry;
mod toolkit_intelligence;
mod translation_commands;
#[cfg(test)]
mod translation_commands_tests;
mod translation_pipeline;
mod translation_providers;
mod trust_ledger;

/// Shared test utilities — compiled unconditionally so integration tests
/// and benchmarks can access them via `fourda_lib::test_utils`.
#[doc(hidden)]
pub mod test_utils;

#[cfg(all(test, feature = "enterprise"))]
mod enterprise_analytics_tests;
#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod hardening_error_path_tests;
#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
#[cfg(all(test, feature = "enterprise"))]
mod organization_tests;
#[cfg(test)]
mod privacy_tests;
#[cfg(test)]
mod privacy_tests_exports;
#[cfg(test)]
mod startup_health_tests;
#[cfg(test)]
mod utils_edge_tests;

// ============================================================================
// App Entry
// ============================================================================

/// Check for required Linux shared libraries.
/// Returns list of missing library descriptions.
#[cfg(target_os = "linux")]
fn check_linux_dependencies() -> Vec<String> {
    let mut missing = Vec::new();

    // Check via ldconfig (most reliable for runtime detection)
    let check_lib = |lib_pattern: &str| -> bool {
        std::process::Command::new("ldconfig")
            .args(["-p"])
            .output()
            .map(|out| {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout.contains(lib_pattern)
            })
            .unwrap_or(false)
    };

    if !check_lib("libwebkit2gtk-4.1") {
        missing.push("libwebkit2gtk-4.1 (WebView rendering engine)".to_string());
    }
    if !check_lib("libgtk-3") {
        missing.push("libgtk-3 (GTK UI framework)".to_string());
    }

    missing
}

/// Detect NVIDIA GPU using multiple methods (lspci may not be available).
#[cfg(target_os = "linux")]
fn detect_nvidia_gpu() -> bool {
    // Method 1: lspci (most common, may not be installed)
    if let Ok(output) = std::process::Command::new("lspci").output() {
        let lspci = String::from_utf8_lossy(&output.stdout);
        if lspci.contains("NVIDIA") {
            return true;
        }
    }

    // Method 2: Check /proc/driver/nvidia (present if NVIDIA kernel module loaded)
    if std::path::Path::new("/proc/driver/nvidia").exists() {
        return true;
    }

    // Method 3: Check for nvidia kernel modules via /sys
    if let Ok(modules) = std::fs::read_to_string("/proc/modules") {
        if modules.contains("nvidia") {
            return true;
        }
    }

    // Method 4: Check glxinfo environment (if mesa reports nvidia)
    if let Ok(renderer) = std::env::var("__GLX_VENDOR_LIBRARY_NAME") {
        if renderer.to_lowercase().contains("nvidia") {
            return true;
        }
    }

    false
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Must be set BEFORE any WebKitGTK initialization
    #[cfg(target_os = "linux")]
    {
        // DMABUF renderer can cause blank screens on NVIDIA (and some AMD) with WebKitGTK.
        // Disable it proactively unless user explicitly opts in.
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            let has_nvidia = detect_nvidia_gpu();
            let on_wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
                || std::env::var("XDG_SESSION_TYPE")
                    .map(|v| v == "wayland")
                    .unwrap_or(false);

            if has_nvidia {
                std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
                tracing::info!(target: "4da::startup", "NVIDIA GPU detected — disabled DMABUF renderer");
            } else if on_wayland {
                // Wayland + DMABUF can also cause issues with some mesa versions.
                // Be conservative: disable unless we're sure it's safe.
                std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
                tracing::info!(target: "4da::startup", "Wayland session detected — disabled DMABUF renderer (safety)");
            }
        }
    }

    // Pre-flight: verify critical Linux dependencies before Tauri tries to use them.
    // Without these, Tauri crashes with cryptic errors. Better to fail clearly.
    #[cfg(target_os = "linux")]
    {
        let missing_libs = check_linux_dependencies();
        if !missing_libs.is_empty() {
            eprintln!("\n\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}");
            eprintln!("\u{2551}  4DA: Missing required system libraries              \u{2551}");
            eprintln!("\u{255a}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255d}\n");
            for lib in &missing_libs {
                eprintln!("  \u{2717} {lib}");
            }
            eprintln!("\nInstall with:");
            eprintln!("  Ubuntu/Debian:  sudo apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev libappindicator3-dev");
            eprintln!("  Fedora/RHEL:    sudo dnf install webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel");
            eprintln!("  Arch Linux:     sudo pacman -S webkit2gtk-4.1 gtk3 libappindicator-gtk3");
            eprintln!();
            std::process::exit(1);
        }
    }

    // Enable WebView2 remote debugging in dev mode (port 9222)
    // Allows Playwright to connect via CDP for functional testing
    #[cfg(all(debug_assertions, target_os = "windows"))]
    {
        if std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_err() {
            std::env::set_var(
                "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
                "--remote-debugging-port=9222",
            );
        }
    }

    // Set Windows AUMID so OS notifications show "4DA" instead of inheriting parent process name.
    // Two-part fix: (1) set process AUMID for taskbar grouping, (2) register in HKCU so the
    // Windows toast notification subsystem resolves "com.4da.app" → DisplayName "4DA".
    #[cfg(target_os = "windows")]
    {
        #[allow(unsafe_code)]
        {
            use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
            let aumid: Vec<u16> = "com.4da.app\0".encode_utf16().collect();
            unsafe {
                SetCurrentProcessExplicitAppUserModelID(aumid.as_ptr());
            }
        }

        // Register notification identity in HKCU (no admin required, works in dev mode)
        if let Ok(hkcu) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .create_subkey("Software\\Classes\\AppUserModelId\\com.4da.app")
        {
            let (key, _) = hkcu;
            let _ = key.set_value("DisplayName", &"4DA");
            let _ = key.set_value("ShowInSettings", &1u32);
        }
    }

    // Pre-Tauri initialization (logging, threshold, DB, context, registry)
    app_setup::initialize_pre_tauri();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Focus the existing window when a second instance is launched
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(victauri_plugin::init())
        .invoke_handler(tauri::generate_handler![
            // Context
            context_commands::get_context_files,
            context_commands::clear_context,
            context_commands::index_context,
            context_commands::index_project_readmes,
            // Intelligence Reconciliation Phase 7 — Git decision miner.
            git_decision_miner::mine_git_decisions,
            // Intelligence Reconciliation Phase 8 — Curated corpus.
            seed_corpus::get_seed_corpus_stats,
            // Intelligence Reconciliation Phase 11 — Commitment Contracts.
            commitment_contracts::create_commitment_contract,
            commitment_contracts::get_commitment_contracts,
            commitment_contracts::dismiss_commitment_contract,
            commitment_contracts::check_refutations,
            context_commands::set_context_dirs,
            context_commands::get_context_dirs,
            context_commands::generate_cli_briefing,
            // Analysis
            analysis::run_deep_initial_scan,
            analysis::run_cached_analysis,
            analysis::get_analysis_status,
            analysis::get_scoring_stats,
            analysis::cancel_analysis,
            // Settings
            settings_commands::get_settings,
            settings_commands::get_llm_usage,
            settings_commands::set_llm_provider,
            settings_commands::mark_onboarding_complete,
            settings_commands::set_rerank_config,
            settings_commands::set_llm_limits,
            settings_commands::test_llm_connection,
            settings_commands::check_ollama_status,
            settings_commands::pull_ollama_model,
            settings_commands::cancel_ollama_pull,
            settings_commands::check_synthesis_capability,
            settings_commands::list_provider_models,
            settings_commands::detect_local_servers,
            settings_commands::get_llm_key_for_mcp,
            settings_commands::detect_environment,
            settings_commands::import_env_key,
            settings_commands::validate_api_key,
            settings_commands::get_privacy_config,
            settings_commands::set_privacy_config,
            calibration_commands::run_calibration,
            calibration_commands::fit_calibration_curves_now,
            calibration_commands::get_calibration_curve_status,
            // Taste Test Calibration
            taste_test_commands::taste_test_start,
            taste_test_commands::taste_test_respond,
            taste_test_commands::taste_test_finalize,
            taste_test_commands::taste_test_is_calibrated,
            taste_test_commands::taste_test_get_profile,
            settings_commands::get_license_tier,
            settings_commands::activate_license,
            settings_commands::get_trial_status,
            settings_commands::start_trial,
            settings_commands::validate_license,
            settings_commands::recover_license_by_email,
            settings_commands::get_locale,
            settings_commands::set_locale,
            settings_commands::get_pro_value_report,
            settings_commands::get_user_context,
            settings_commands::set_user_role,
            settings_commands::set_experience_level,
            settings_commands::add_tech_stack,
            settings_commands::remove_tech_stack,
            settings_commands::add_interest,
            settings_commands::remove_interest,
            settings_commands::add_exclusion,
            settings_commands::remove_exclusion,
            settings_commands::record_interaction,
            settings_commands::snooze_item,
            settings_commands::watch_item,
            settings_commands::unwatch_item,
            settings_commands::get_watched_items,
            settings_commands::set_blind_spot_sensitivity,
            settings_commands::get_blind_spot_sensitivity,
            settings_commands::get_context_stats,
            // Embedding model management
            reembed::get_embedding_model_info,
            // Monitoring
            monitoring_commands::get_monitoring_status,
            monitoring_commands::set_monitoring_enabled,
            monitoring_commands::set_monitoring_interval,
            monitoring_commands::set_notification_threshold,
            monitoring_commands::trigger_notification_test,
            monitoring_commands::set_close_to_tray,
            monitoring_commands::set_launch_at_startup,
            monitoring_commands::get_launch_at_startup,
            monitoring_commands::set_notification_style,
            monitoring_commands::trigger_notification_preview,
            // Morning briefing configuration
            monitoring_commands::set_morning_briefing_enabled,
            monitoring_commands::get_morning_briefing_config,
            monitoring_commands::set_briefing_time,
            monitoring_commands::trigger_briefing_preview,
            // Notification window
            notification_window::notification_clicked,
            // Briefing window
            briefing_window::briefing_item_clicked,
            briefing_window::briefing_open_url,
            briefing_window::trigger_morning_briefing,
            // Briefing snapshot — Sovereign Cold Boot instant-paint
            briefing_snapshot::get_briefing_snapshot,
            // ACE (frontend-used subset)
            ace_commands::ace_get_detected_tech,
            ace_commands::ace_get_active_topics,
            ace_commands::ace_full_scan,
            ace_commands::ace_auto_discover,
            ace_commands::ace_get_scan_summary,
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
            ace_commands::get_engagement_summary,
            ace_commands::ace_get_single_affinity,
            // Source config
            source_config::get_rss_feeds,
            source_config::set_rss_feeds,
            source_config::get_twitter_handles,
            source_config::set_twitter_handles,
            source_config::set_x_api_key,
            source_config::has_x_api_key,
            source_config::get_youtube_channels,
            source_config::set_youtube_channels,
            source_config::get_github_languages,
            source_config::set_github_languages,
            source_config::get_default_rss_feeds,
            source_config::get_default_youtube_channels,
            source_config::get_default_twitter_handles,
            source_config::get_disabled_default_rss_feeds,
            source_config::set_disabled_default_rss_feeds,
            source_config::get_disabled_default_youtube_channels,
            source_config::set_disabled_default_youtube_channels,
            source_config::get_disabled_default_twitter_handles,
            source_config::set_disabled_default_twitter_handles,
            // Source validation & immediate fetch
            source_config::validate_rss_feed,
            source_config::validate_youtube_channel,
            source_fetch_commands::fetch_single_feed,
            source_fetch_commands::fetch_single_youtube_channel,
            source_fetch_commands::reset_feed_health,
            source_fetch_commands::get_feed_health_status,
            // Digest & Briefing
            digest_config::get_digest_config,
            digest_config::set_digest_config,
            digest_commands::generate_ai_briefing,
            digest_commands::get_latest_briefing,
            digest_email::test_digest_email,
            digest_email::set_digest_email_config,
            free_briefing::generate_free_briefing,
            // Content
            commands::get_sources,
            commands::mcp_score_autopsy,
            commands::export_results,
            // Void Engine
            void_commands::get_void_signal,
            // Intelligence panels
            attention::get_attention_report,
            blind_spots::get_blind_spots,
            knowledge_decay::get_knowledge_gaps,
            preemption::get_preemption_alerts,
            signal_chains::get_signal_chains,
            signal_chains::get_signal_chains_predicted,
            signal_chains::resolve_signal_chain,
            semantic_diff::get_semantic_shifts,
            project_health::get_project_health,
            developer_dna::get_developer_dna,
            developer_dna::export_developer_dna_markdown,
            developer_dna::export_developer_dna_svg,
            developer_dna::export_developer_dna_card,
            // Content (article reader, AI summaries, saved items)
            content_commands::get_item_content,
            content_commands::get_item_summary,
            content_commands::generate_item_summary,
            content_commands::get_saved_items,
            content_commands::remove_saved_item,
            // Source Health
            health_commands::get_source_health_status,
            health_commands::get_source_quality,
            health_commands::reset_source_circuit_breaker,
            // Decision Intelligence
            decisions::get_decisions,
            decisions::record_developer_decision,
            decisions::update_developer_decision,
            decisions::remove_tech_decision,
            // Decision Advantage
            decision_advantage_commands::get_decision_windows,
            decision_advantage_commands::act_on_decision_window,
            decision_advantage_commands::close_decision_window,
            decision_advantage_commands::get_advantage_history,
            // Agent Memory
            // Tech Radar
            tech_radar::get_tech_radar,
            tech_radar::get_radar_entry,
            tech_radar_commands::get_radar_entry_detail,
            tech_radar_commands::generate_tech_narratives,
            tech_radar_commands::get_radar_snapshots,
            tech_radar_commands::get_radar_at_snapshot,
            // Agent Memory
            agent_memory::store_agent_memory,
            agent_memory::recall_agent_memories,
            agent_memory::promote_memory_to_decision,
            // Agent Brief
            agent_brief::generate_agent_brief,
            // Delegation Scoring
            delegation::get_delegation_score,
            delegation::get_all_delegation_scores,
            // Toolkit
            toolkit::toolkit_list_ports,
            toolkit::toolkit_kill_process,
            toolkit::toolkit_env_snapshot,
            toolkit_http::toolkit_http_request,
            toolkit_http::toolkit_get_http_history,
            // Stack Intelligence
            stack_commands::get_stack_profiles,
            stack_commands::get_selected_stacks,
            stack_commands::set_selected_stacks,
            stack_commands::detect_stack_profiles,
            stack_commands::get_composed_stack,
            // Stack Health Engine
            stack_health::get_stack_health,
            stack_health::get_missed_intelligence,
            // Playbook (STREETS Playbook)
            playbook_commands::get_playbook_modules,
            playbook_commands::get_playbook_content,
            playbook_commands::get_playbook_progress,
            playbook_commands::mark_lesson_complete,
            playbook_commands::translate_playbook_module,
            playbook_commands::get_lesson_translation_status,
            // Content Personalization (Sovereign Content Engine)
            content_personalization::commands::get_personalized_lesson,
            content_personalization::commands::get_personalized_lessons_batch,
            content_personalization::commands::get_personalization_context_summary,
            content_personalization::commands::prune_personalization_cache,
            content_personalization::commands::hydrate_lesson_with_llm,
            // Content Integrity Verification
            content_integrity::check_content_integrity,
            content_integrity::audit_content_integrity,
            // First-Run Simulation Audit
            first_run_audit::run_first_run_simulation,
            // STREETS Command Execution
            streets_commands::parse_lesson_commands,
            streets_commands::execute_streets_command,
            streets_commands::execute_lesson_commands,
            // STREETS Contextual Suggestion
            streets_suggestion::get_streets_suggestion,
            // Sovereign Developer Profile (unified view)
            sovereign_developer_profile::get_sovereign_developer_profile,
            sovereign_developer_profile::export_sovereign_profile_markdown,
            sovereign_developer_profile::export_sovereign_profile_json,
            // Sovereign Profile
            sovereign_profile::get_sovereign_profile,
            sovereign_profile::get_sovereign_profile_completeness,
            sovereign_profile::generate_sovereign_stack_document,
            sovereign_profile::save_sovereign_fact,
            sovereign_profile::get_execution_log,
            // STREETS Localization
            streets_localization::get_regional_data,
            streets_localization::format_currency,
            streets_localization::calculate_electricity_cost,
            // Toolkit Intelligence
            toolkit_intelligence::toolkit_test_feed,
            toolkit_intelligence::toolkit_score_sandbox,
            toolkit_export::toolkit_generate_export_pack,
            // Templates (STREETS Community)
            templates::get_templates,
            templates::get_template_content,
            // Diagnostics
            commands::get_diagnostics,
            commands::get_scoring_history,
            startup_health::get_startup_health,
            startup_health::get_diagnostic_report,
            // Capability Health (Graceful Degradation Framework)
            capabilities::get_capability_states,
            capabilities::get_capability_summary,
            // Scoring Validation (persona-based precision testing)
            scoring::validation::runner::run_scoring_validation,
            // Feedback -> Autophagy bridge
            ace_commands::record_item_feedback,
            // Autophagy (intelligent content metabolism)
            autophagy_commands::get_autophagy_status,
            autophagy_commands::get_autophagy_history,
            autophagy_pulse::get_intelligence_pulse,
            autophagy_commands::trigger_autophagy_cycle,
            // Data Health
            autophagy_commands::get_data_health,
            autophagy_commands::run_deep_clean,
            autophagy_commands::set_cleanup_retention,
            // Translation Pipeline
            translation_commands::get_translation_status,
            translation_commands::trigger_translation,
            translation_commands::get_all_translations,
            translation_commands::save_translation_override,
            translation_commands::get_translation_overrides,
            translation_commands::delete_translation_override,
            // Content Translation (real-time feed/briefing translation)
            content_translation_commands::translate_content,
            content_translation_commands::translate_content_batch,
            content_translation_commands::get_content_translation_settings,
            content_translation_commands::get_translation_cache_stats,
            content_translation_commands::purge_translation_cache,
            content_translation_commands::get_translation_config,
            content_translation_commands::set_translation_config,
            // Achievements
            achievement_commands::get_achievement_state,
            achievement_commands::get_achievements,
            achievement_commands::check_daily_streak,
            // Information Channels
            channel_commands::list_channels,
            channel_commands::get_channel,
            channel_commands::get_channel_content,
            channel_commands::render_channel_now,
            channel_commands::get_channel_provenance,
            channel_commands::get_channel_changelog,
            channel_commands::get_channel_sources,
            channel_commands::refresh_channel_sources,
            channel_commands::auto_render_all_channels,
            channel_commands::create_custom_channel,
            channel_commands::preview_channel_sources,
            channel_commands::delete_channel,
            // Natural Language Search (Signal)
            natural_language_search::natural_language_query,
            // Search Synthesis — LLM briefings (Signal)
            search_synthesis::synthesize_search,
            // Weekly Intelligence Digest (free — BYOK)
            weekly_digest::generate_weekly_digest,
            weekly_digest::get_latest_digest,
            // Decision Impact Tracking (Signal)
            decision_signals::get_decision_signals,
            decision_signals::get_decision_health_report,
            // Standing Queries (Signal)
            standing_queries::create_standing_query,
            standing_queries::list_standing_queries,
            standing_queries::delete_standing_query,
            standing_queries::get_standing_query_matches,
            standing_queries::get_standing_query_suggestions,
            // Indexed Documents
            indexed_documents_commands::get_indexed_documents,
            indexed_documents_commands::get_indexed_stats,
            indexed_documents_commands::search_documents,
            indexed_documents_commands::get_document_content,
            // STREETS Health
            suns_commands::get_street_health,
            // Intelligence History
            intelligence_history::get_intelligence_growth,
            intelligence_history::get_session_diff,
            // Intelligence Packs
            intelligence_packs::list_intelligence_packs,
            intelligence_packs::activate_intelligence_pack,
            intelligence_packs::deactivate_intelligence_pack,
            intelligence_packs::suggest_intelligence_packs,
            // Community Intelligence
            community_intelligence::get_community_status,
            community_intelligence::set_community_intelligence_enabled,
            community_intelligence::set_community_frequency,
            // Local Telemetry (privacy-first, never leaves machine)
            telemetry::track_event,
            telemetry::get_usage_analytics,
            telemetry::clear_telemetry,
            telemetry::get_error_telemetry,
            telemetry::get_error_summary_cmd,
            telemetry::clear_error_telemetry,
            telemetry::get_security_audit_log,
            // Team Sync (AD-023)
            team_sync_commands::get_team_sync_status,
            team_sync_commands::get_team_members,
            team_sync_commands::share_dna_with_team,
            team_sync_commands::share_signal_with_team,
            team_sync_commands::propose_team_decision,
            team_sync_commands::vote_on_decision,
            team_sync_commands::get_team_decisions,
            team_sync_commands::get_decision_detail,
            team_sync_commands::resolve_decision,
            team_sync_commands::join_team_via_invite,
            team_sync_commands::create_team,
            team_sync_commands::create_team_invite,
            // Team Shared Sources (AD-023)
            team_sync_commands::share_source_with_team,
            team_sync_commands::get_team_sources,
            team_sync_commands::upvote_team_source,
            team_sync_commands::remove_team_source,
            // Team Intelligence (AD-023)
            team_intelligence::get_team_profile_cmd,
            team_intelligence::get_team_blind_spots_cmd,
            team_intelligence::get_bus_factor_report_cmd,
            team_intelligence::get_team_signal_summary_cmd,
            // Team Monitoring
            team_monitoring::get_team_signals_cmd,
            team_monitoring::resolve_team_signal_cmd,
            team_monitoring::get_alert_policy_cmd,
            team_monitoring::set_alert_policy_cmd,
            team_monitoring::get_monitoring_summary_cmd,
            // Team Notifications
            team_notifications::get_team_notifications,
            team_notifications::get_notification_summary,
            team_notifications::mark_notification_read,
            team_notifications::mark_all_notifications_read,
            team_notifications::dismiss_notification,
            // Data Export (GDPR compliance — all tiers)
            data_export::export_all_data,
            data_export::export_section,
            data_export::list_exports,
            data_export::delete_export,
            data_export::factory_reset,
            // Enterprise: Audit Log
            audit::get_audit_log,
            audit::get_audit_summary_cmd,
            audit::export_audit_csv_cmd,
            // Enterprise: Webhooks
            webhooks::register_webhook_cmd,
            webhooks::list_webhooks_cmd,
            webhooks::delete_webhook_cmd,
            webhooks::test_webhook_cmd,
            webhooks::get_webhook_deliveries_cmd,
            // Enterprise: Organizations
            organization::get_organization_cmd,
            organization::get_org_teams_cmd,
            organization::get_retention_policies_cmd,
            organization::set_retention_policy_cmd,
            organization::get_cross_team_signals_cmd,
            // Enterprise: Analytics
            enterprise_analytics::get_org_analytics_cmd,
            enterprise_analytics::export_org_analytics_cmd,
            // Enterprise: SSO/SAML/OIDC
            sso::get_sso_config,
            sso::set_sso_config,
            sso::initiate_sso_login,
            sso::get_sso_session,
            sso::validate_sso_callback,
            sso::logout_sso,
            // LLM Capability Tier Detection
            llm_capability::get_llm_capability_tier,
            llm_capability::probe_llm_capability,
            // Model Registry
            model_registry::get_model_registry,
            model_registry::refresh_model_registry,
            // Dependency Intelligence
            dependency_commands::get_dependency_overview,
            dependency_commands::get_project_deps,
            dependency_commands::get_dependency_alerts,
            dependency_commands::resolve_dependency_alert,
            dependency_commands::check_dependency_upgrades,
            dependency_commands::get_license_overview,
            // OSV Mirror (Tier 1 Intelligence)
            osv::osv_sync_now,
            osv::osv_get_matches,
            osv::osv_get_sync_status,
            // Intelligence Metrics (Phase 9)
            intelligence_metrics::get_intelligence_metrics,
            // Accuracy Tracking (Phase 4.1)
            accuracy::get_accuracy_report,
            accuracy::get_intelligence_report,
            // Temporal Graph (Phase 4.5)
            temporal_graph::get_temporal_snapshot,
            temporal_graph::get_adoption_curves,
            temporal_graph::get_knowledge_decay_report,
            // Tech Convergence (Phase 6.3)
            tech_convergence::get_tech_convergence,
            tech_convergence::get_project_health_comparison,
            tech_convergence::get_cross_project_dependencies,
            // AI Cost Tracking (Phase 8.2)
            ai_costs::get_ai_usage_summary,
            ai_costs::get_ai_cost_estimate,
            ai_costs::get_ai_cost_recommendation,
            // Source Plugin API (Phase 7)
            plugin_commands::list_plugins,
            plugin_commands::fetch_plugin_items,
            plugin_commands::fetch_all_plugins,
            // Waitlist
            waitlist::save_waitlist_signup,
            waitlist::get_waitlist_signups,
            // Trust Ledger (intelligence quality measurement)
            trust_ledger::get_trust_dashboard,
            trust_ledger::record_intelligence_feedback,
            trust_ledger::get_domain_precision_report,
            trust_ledger::get_false_positive_analysis,
            // Alert Triage (persistent security triage actions)
            alert_triage::triage_alert,
            alert_triage::get_triage_states,
            alert_triage::clear_expired_triage,
            // Diagnostic log reader (persistent log files)
            log_retention::get_recent_logs,
        ])
        .setup(app_setup::setup_app)
        .build(tauri::generate_context!())
        .expect("Failed to build Tauri application. Check tauri.conf.json and system permissions.")
        .run(app_setup::handle_run_event);
}
