// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

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
pub mod state;
mod utils;

// Re-export from embeddings
pub(crate) use embeddings::embed_texts;

// Re-exports from events
pub(crate) use events::{
    emit_progress, void_signal_analysis_complete, void_signal_cache_filled, void_signal_error,
    void_signal_fetch_progress, void_signal_fetching, void_signal_notification,
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

// Re-exports from state (preserves `use crate::accessor` interface)
pub(crate) use state::{
    get_ace_engine, get_ace_engine_mut, get_analysis_abort, get_analysis_state, get_context_dir,
    get_context_dirs, get_context_engine, get_database, get_llm_token_usage, get_monitoring_state,
    get_relevance_threshold, get_settings_manager, get_source_registry, invalidate_context_engine,
    open_db_connection, register_sqlite_vec_extension, set_relevance_threshold,
    SUPPORTED_EXTENSIONS,
};

mod accuracy;
mod ace;
mod ace_commands;
mod agent_brief;
mod agent_memory;
mod ai_costs;
mod analysis;
mod analysis_narration;
mod analysis_rerank;
mod anomaly;
mod attention;
mod autophagy;
mod autophagy_commands;
mod autophagy_pulse;
mod calibration_commands;
mod calibration_probes;
mod channel_changelog;
mod channel_commands;
mod channel_provenance;
mod channel_render;
pub mod channels;
mod community_intelligence;
mod competing_tech;
mod content_commands;
mod content_dna;
mod content_quality;
mod context_commands;
mod context_engine;
mod data_export;
pub mod db;
mod decision_advantage;
mod decision_advantage_commands;
mod decision_signals;
mod decisions;
#[cfg(feature = "experimental")]
mod delegation;
mod dependency_commands;
#[cfg(not(feature = "experimental"))]
#[allow(dead_code)] // Feature-gated: stub active only when "experimental" is disabled
mod delegation {
    use crate::error::Result;
    use serde::{Deserialize, Serialize};
    use ts_rs::TS;

    #[derive(Debug, Clone, Serialize, Deserialize, TS)]
    #[ts(export, export_to = "bindings/")]
    pub struct DelegationScore {
        pub subject: String,
        pub overall_score: f64,
        pub factors: DelegationFactors,
        pub recommendation: DelegationRec,
        pub caveats: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, TS)]
    #[ts(export, export_to = "bindings/")]
    pub struct DelegationFactors {
        pub pattern_stability: f64,
        pub security_sensitivity: f64,
        pub codebase_complexity: f64,
        pub decision_density: f64,
        pub ai_track_record: f64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
    #[serde(rename_all = "snake_case")]
    #[ts(export, export_to = "bindings/")]
    pub enum DelegationRec {
        FullyDelegate,
        DelegateWithReview,
        CollaborateRealtime,
        HumanOnly,
    }

    #[tauri::command]
    pub async fn get_delegation_score(_subject: String) -> Result<DelegationScore> {
        Err("Delegation scoring is an experimental feature".into())
    }

    #[tauri::command]
    pub async fn get_all_delegation_scores() -> Result<Vec<DelegationScore>> {
        Err("Delegation scoring is an experimental feature".into())
    }
}
mod developer_dna;
mod diagnostics;
mod digest;
mod digest_commands;
mod digest_config;
mod digest_email;
mod domain_profile;
mod domain_profile_data;
pub mod extractors;
mod free_briefing;
#[cfg(feature = "experimental")]
mod game_achievements;
#[cfg(not(feature = "experimental"))]
mod game_achievements {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "lowercase")]
    pub enum AchievementTier {
        Bronze,
        Silver,
        Gold,
    }
}
#[cfg(feature = "experimental")]
mod game_commands;
mod http_client;
#[cfg(not(feature = "experimental"))]
#[allow(dead_code)] // Feature-gated: stub active only when "experimental" is disabled
mod game_commands {
    use crate::error::Result;
    use tauri::AppHandle;

    #[tauri::command]
    pub fn get_game_state() -> Result<serde_json::Value> {
        Ok(
            serde_json::json!({"counters": [], "achievements": [], "streak": 0, "last_active": null}),
        )
    }

    #[tauri::command]
    pub fn get_achievements() -> Result<serde_json::Value> {
        Ok(serde_json::json!([]))
    }

    #[tauri::command]
    pub fn check_daily_streak(_app: AppHandle) -> Result<serde_json::Value> {
        Ok(serde_json::json!([]))
    }
}
#[cfg(feature = "experimental")]
mod game_engine;
#[cfg(not(feature = "experimental"))]
mod game_engine {
    use crate::db::Database;
    use crate::game_achievements::AchievementTier;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AchievementUnlocked {
        pub id: String,
        pub name: String,
        pub description: String,
        pub icon: String,
        pub tier: AchievementTier,
        pub celebration_intensity: f64,
        pub unlocked_at: String,
    }

    pub fn create_tables(_conn: &rusqlite::Connection) -> rusqlite::Result<()> {
        Ok(())
    }

    pub fn increment_counter(
        _db: &Database,
        _counter_type: &str,
        _amount: u64,
    ) -> Vec<AchievementUnlocked> {
        Vec::new()
    }

    #[allow(dead_code)] // Feature-gated: stub active only when "experimental" is disabled
    pub fn check_daily_streak(_db: &Database) -> Vec<AchievementUnlocked> {
        Vec::new()
    }

    #[allow(dead_code)] // Feature-gated: stub active only when "experimental" is disabled
    pub fn get_game_state(_db: &Database) -> serde_json::Value {
        serde_json::json!({"counters": [], "achievements": [], "streak": 0, "last_active": null})
    }

    #[allow(dead_code)] // Feature-gated: stub active only when "experimental" is disabled
    pub fn get_achievements(_db: &Database) -> Vec<AchievementUnlocked> {
        Vec::new()
    }
}
mod health;
mod health_commands;
mod indexed_documents_commands;
mod intelligence_history;
mod job_queue;
mod knowledge_decay;
mod llm;
mod llm_judge;
mod llm_stream;
mod local_audit;
pub mod model_registry;
mod monitoring;
mod monitoring_briefing;
mod monitoring_commands;
mod monitoring_jobs;
mod monitoring_notifications;
mod natural_language_search;
mod novelty;
mod ollama;
mod plugin_commands;
pub mod plugins;
mod probes_corpus;
mod probes_engine;
mod project_health;
mod project_health_dimensions;
pub mod query;
mod scoring;
pub(crate) mod scoring_config;
mod search_synthesis;
mod semantic_diff;
pub mod settings;
mod settings_commands;
mod signal_chains;
mod signal_terminal;
mod signal_terminal_pages;
mod signals;
mod source_config;
mod source_fetching;
pub mod sources;
mod standing_queries;
mod standing_queries_evaluation;
mod standing_queries_suggestions;
mod startup_health;
mod suns;
mod suns_commands;
mod tech_convergence;
mod tech_radar;
mod tech_radar_commands;
mod tech_radar_compute;
mod temporal;
mod temporal_graph;
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
mod toolkit_http {
    use crate::error::Result;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HttpProbeRequest {
        pub method: String,
        pub url: String,
        pub headers: Vec<(String, String)>,
        pub body: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HttpProbeResponse {
        pub status: u16,
        pub status_text: String,
        pub headers: Vec<(String, String)>,
        pub body: String,
        pub duration_ms: u64,
        pub size_bytes: usize,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HttpHistoryEntry {
        pub id: i64,
        pub method: String,
        pub url: String,
        pub status: u16,
        pub duration_ms: u64,
        pub created_at: String,
    }

    #[tauri::command]
    pub async fn toolkit_http_request(_request: HttpProbeRequest) -> Result<HttpProbeResponse> {
        Err(crate::error::FourDaError::Config(
            "HTTP toolkit is an experimental feature".into(),
        ))
    }

    #[tauri::command]
    pub async fn toolkit_get_http_history(_limit: Option<u32>) -> Result<Vec<HttpHistoryEntry>> {
        Ok(vec![])
    }
}
// Team sync — encrypted metadata relay (AD-023)
// Gated: 17 commands with zero frontend callers. Enable with --features team-sync.
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
mod team_sync_types {
    //! Minimal stub — only TeamRelayConfig is needed by settings deserialization.
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct TeamRelayConfig {
        pub enabled: bool,
        pub relay_url: Option<String>,
        pub auth_token: Option<String>,
        pub team_id: Option<String>,
        pub client_id: Option<String>,
        pub display_name: Option<String>,
        pub role: Option<String>,
        pub sync_interval_secs: Option<u64>,
    }
}
#[cfg(feature = "team-sync")]
mod team_intelligence;
#[cfg(feature = "team-sync")]
mod team_monitoring;
#[cfg(feature = "team-sync")]
mod team_notifications;

// Stubs when team-sync is disabled (commands register but return errors)
#[cfg(not(feature = "team-sync"))]
mod team_sync_commands {
    use crate::error::Result;

    #[tauri::command]
    pub async fn get_team_sync_status() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn get_team_members() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn share_dna_with_team() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn share_signal_with_team() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn propose_team_decision() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn vote_on_decision() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn get_team_decisions() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn get_decision_detail() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn resolve_decision() -> Result<()> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn join_team_via_invite() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn create_team() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn create_team_invite() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn share_source_with_team() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn get_team_sources() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn upvote_team_source() -> Result<()> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn remove_team_source() -> Result<()> {
        Err("Team sync requires --features team-sync".into())
    }
}
#[cfg(not(feature = "team-sync"))]
mod team_intelligence {
    use crate::error::Result;

    #[tauri::command]
    pub async fn get_team_profile_cmd() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn get_team_blind_spots_cmd() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn get_bus_factor_report_cmd() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn get_team_signal_summary_cmd() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
}
#[cfg(not(feature = "team-sync"))]
mod team_monitoring {
    use crate::error::Result;

    #[tauri::command]
    pub async fn get_team_signals_cmd() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn resolve_team_signal_cmd() -> Result<()> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn get_alert_policy_cmd() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn set_alert_policy_cmd() -> Result<()> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn get_monitoring_summary_cmd() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
}
#[cfg(not(feature = "team-sync"))]
mod team_notifications {
    use crate::error::Result;

    #[tauri::command]
    pub async fn get_team_notifications() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn get_notification_summary() -> Result<serde_json::Value> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn mark_notification_read() -> Result<()> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn mark_all_notifications_read() -> Result<()> {
        Err("Team sync requires --features team-sync".into())
    }
    #[tauri::command]
    pub async fn dismiss_notification() -> Result<()> {
        Err("Team sync requires --features team-sync".into())
    }
}

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
mod audit {
    use crate::error::Result;

    /// Bundled audit logging parameters (used by team-sync without enterprise).
    #[cfg(feature = "team-sync")]
    pub struct AuditLogParams<'a> {
        pub conn: &'a rusqlite::Connection,
        pub team_id: &'a str,
        pub actor_id: &'a str,
        pub actor_display_name: &'a str,
        pub action: &'a str,
        pub resource_type: &'a str,
        pub resource_id: Option<&'a str>,
        pub details: Option<&'a serde_json::Value>,
    }

    /// No-op audit logging when enterprise feature is disabled.
    #[allow(unused_variables)]
    pub fn log_team_audit(
        conn: &rusqlite::Connection,
        action: &str,
        resource_type: &str,
        resource_id: Option<&str>,
        details: Option<&serde_json::Value>,
    ) {
        // Enterprise audit logging disabled — no-op
    }

    /// No-op direct audit logging when enterprise feature is disabled.
    #[cfg(feature = "team-sync")]
    #[allow(unused_variables)]
    pub fn log_audit(_params: &AuditLogParams<'_>) {
        // Enterprise audit logging disabled — no-op
    }

    #[tauri::command]
    pub async fn get_audit_log() -> Result<serde_json::Value> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn get_audit_summary_cmd() -> Result<serde_json::Value> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn export_audit_csv_cmd() -> Result<String> {
        Err("Enterprise features require --features enterprise".into())
    }
}
#[cfg(not(feature = "enterprise"))]
mod webhooks {
    use crate::error::Result;

    #[tauri::command]
    pub async fn register_webhook_cmd() -> Result<serde_json::Value> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn list_webhooks_cmd() -> Result<serde_json::Value> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn delete_webhook_cmd() -> Result<()> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn test_webhook_cmd() -> Result<bool> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn get_webhook_deliveries_cmd() -> Result<serde_json::Value> {
        Err("Enterprise features require --features enterprise".into())
    }
}
#[cfg(not(feature = "enterprise"))]
mod organization {
    use crate::error::Result;

    #[tauri::command]
    pub async fn get_organization_cmd() -> Result<serde_json::Value> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn get_org_teams_cmd() -> Result<serde_json::Value> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn get_retention_policies_cmd() -> Result<serde_json::Value> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn set_retention_policy_cmd() -> Result<()> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn get_cross_team_signals_cmd() -> Result<serde_json::Value> {
        Err("Enterprise features require --features enterprise".into())
    }
}
#[cfg(not(feature = "enterprise"))]
mod enterprise_analytics {
    use crate::error::Result;

    #[tauri::command]
    pub async fn get_org_analytics_cmd() -> Result<serde_json::Value> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn export_org_analytics_cmd() -> Result<String> {
        Err("Enterprise features require --features enterprise".into())
    }
}
#[cfg(not(feature = "enterprise"))]
mod sso {
    use crate::error::Result;

    #[tauri::command]
    pub async fn get_sso_config() -> Result<serde_json::Value> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn set_sso_config() -> Result<()> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn initiate_sso_login() -> Result<String> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn get_sso_session() -> Result<serde_json::Value> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn validate_sso_callback(
        _assertion: String,
        _state: Option<String>,
    ) -> Result<serde_json::Value> {
        Err("Enterprise features require --features enterprise".into())
    }
    #[tauri::command]
    pub async fn logout_sso() -> Result<()> {
        Err("Enterprise features require --features enterprise".into())
    }
}

mod telemetry;
mod toolkit_intelligence;
mod translation_commands;
#[cfg(test)]
mod translation_commands_tests;
mod translation_pipeline;
use source_fetching::fill_cache_background;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Must be set BEFORE any WebKitGTK initialization
    #[cfg(target_os = "linux")]
    {
        // Detect NVIDIA GPU and apply WebKitGTK workaround for blank screen
        // This is the #1 reported Tauri Linux issue (tauri-apps/tauri#9304)
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            if let Ok(output) = std::process::Command::new("lspci").output() {
                let lspci = String::from_utf8_lossy(&output.stdout);
                if lspci.contains("NVIDIA") {
                    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
                }
            }
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
    let (source_count, source_names) = {
        let reg = registry.lock();
        let count = reg.count();
        let names: Vec<String> = reg.sources().iter().map(|s| s.name().to_string()).collect();
        (count, names)
    };
    info!(target: "4da::startup", count = source_count, sources = %source_names.join(", "), "Sources registered");

    // Ensure plugins directory exists for Source Plugin API
    plugins::loader::ensure_plugins_dir();

    // Run startup health self-check (fast, offline, infallible)
    let _startup_issues = startup_health::run_startup_health_check();

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
        .invoke_handler(tauri::generate_handler![
            // Context
            context_commands::get_context_files,
            context_commands::clear_context,
            context_commands::index_context,
            context_commands::index_project_readmes,
            context_commands::sync_awe_wisdom,
            context_commands::get_awe_summary,
            context_commands::run_awe_transmute,
            context_commands::run_awe_quick_check,
            context_commands::run_awe_consequence_scan,
            context_commands::run_awe_feedback,
            context_commands::run_awe_recall,
            context_commands::run_awe_calibration,
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
            settings_commands::test_llm_connection,
            settings_commands::check_ollama_status,
            settings_commands::pull_ollama_model,
            settings_commands::list_provider_models,
            settings_commands::detect_local_servers,
            settings_commands::get_llm_key_for_mcp,
            settings_commands::detect_environment,
            settings_commands::import_env_key,
            settings_commands::validate_api_key,
            calibration_commands::run_calibration,
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
            settings_commands::get_locale,
            settings_commands::set_locale,
            settings_commands::get_pro_value_report,
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
            monitoring_commands::set_close_to_tray,
            monitoring_commands::set_launch_at_startup,
            monitoring_commands::get_launch_at_startup,
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
            knowledge_decay::get_knowledge_gaps,
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
            // Decision Intelligence
            decisions::get_decisions,
            decisions::record_developer_decision,
            decisions::update_developer_decision,
            decisions::remove_tech_decision,
            // Decision Advantage
            decision_advantage_commands::get_decision_windows,
            decision_advantage_commands::act_on_decision_window,
            decision_advantage_commands::close_decision_window,
            decision_advantage_commands::get_compound_advantage,
            decision_advantage_commands::get_advantage_history,
            // Agent Memory
            // Tech Radar
            tech_radar::get_tech_radar,
            tech_radar::get_radar_entry,
            tech_radar_commands::get_radar_entry_detail,
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
            // Playbook (STREETS Course)
            playbook_commands::get_playbook_modules,
            playbook_commands::get_playbook_content,
            playbook_commands::get_playbook_progress,
            playbook_commands::mark_lesson_complete,
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
            startup_health::get_startup_health,
            // Scoring Validation (persona-based precision testing)
            scoring::validation::runner::run_scoring_validation,
            // Feedback → Autophagy bridge
            ace_commands::record_item_feedback,
            // Autophagy (intelligent content metabolism)
            autophagy_commands::get_autophagy_status,
            autophagy_commands::get_autophagy_history,
            autophagy_pulse::get_intelligence_pulse,
            autophagy_commands::trigger_autophagy_cycle,
            // Translation Pipeline
            translation_commands::get_translation_status,
            translation_commands::trigger_translation,
            translation_commands::get_all_translations,
            translation_commands::save_translation_override,
            translation_commands::get_translation_overrides,
            translation_commands::delete_translation_override,
            // GAME Engine
            game_commands::get_game_state,
            game_commands::get_achievements,
            game_commands::check_daily_streak,
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
        ])
        .setup(|app| {
            // Record app start time for diagnostics uptime tracking
            diagnostics::record_start_time();

            // Start Signal Terminal HTTP server (requires Tokio runtime from Tauri)
            signal_terminal::start_signal_terminal();

            // Set up system tray (non-fatal: app works without tray)
            let tray = match monitoring::setup_tray(app.handle()) {
                Ok(tray) => Some(tray),
                Err(e) => {
                    warn!("System tray setup failed, continuing without tray: {e}");
                    None
                }
            };

            // Store tray handle for later updates
            app.manage(std::sync::Mutex::new(tray));

            // Load monitoring settings from persistence
            let monitoring_state = get_monitoring_state().clone();
            {
                let settings = get_settings_manager().lock();
                let config = settings.get_monitoring_config();
                monitoring_state.set_enabled(config.enabled);
                monitoring_state.set_interval(config.interval_minutes * 60);
                info!(target: "4da::monitor", enabled = config.enabled, interval_mins = config.interval_minutes, "Loaded monitoring settings");
            }

            // Validate license integrity (reset tier if no key present)
            crate::settings::validate_license_on_startup();

            // Start background scheduler
            let app_handle = app.handle().clone();
            monitoring::start_scheduler(app_handle.clone(), monitoring_state.clone());

            // Start team sync scheduler (if configured)
            #[cfg(feature = "team-sync")]
            {
                let team_state = std::sync::Arc::new(team_sync_scheduler::TeamSyncState::default());
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
                team_sync_scheduler::start_sync_scheduler(app_handle.clone(), team_state);
            }

            // Start enterprise retention enforcement scheduler (daily, fire-and-forget)
            #[cfg(feature = "enterprise")]
            organization::start_retention_scheduler();

            // Listen for tray events
            let app_handle_analyze = app_handle.clone();
            app.listen("tray-analyze", move |_| {
                info!(target: "4da::tray", "Manual analysis triggered from tray");
                let _ = app_handle_analyze.emit("start-analysis-from-tray", ());
            });

            // Handle deep-link URLs (4da://activate?key=...)
            let deep_link_handle = app_handle.clone();
            app.listen("deep-link://new-url", move |event| {
                if let Some(urls) = event.payload().strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
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

                            // Extract notification info before moving signal_summary
                            let notification_info = signal_summary.as_ref().map(|s| (s.critical_count, s.high_count));

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
                                    void_signal_notification(&handle, true, critical);
                                }
                                Some((_, high)) if high > 0 => {
                                    void_signal_notification(&handle, false, high);
                                }
                                _ if relevant_count > 0 => {
                                    void_signal_notification(&handle, false, relevant_count);
                                }
                                _ => {}
                            }

                            // Emit results to frontend if window is visible
                            void_signal_analysis_complete(&handle, &results);
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
                                                void_signal_notification(&standing_handle, false, total_new as usize);
                                            }
                                        }
                                    }
                                });
                            }
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

            // Validate license key against Keygen API (fire-and-forget, non-blocking)
            {
                let license_key = {
                    let settings = get_settings_manager().lock();
                    settings.get().license.license_key.clone()
                };
                if !license_key.is_empty() {
                    let current_tier = {
                        let settings = get_settings_manager().lock();
                        settings.get().license.tier.clone()
                    };
                    tauri::async_runtime::spawn(async move {
                        info!(target: "4da::license", "Startup license validation (Keygen)");
                        let result = crate::settings::validate_license_key_keygen(
                            &license_key,
                            &current_tier,
                        )
                        .await;
                        if result.tier != current_tier {
                            let manager = get_settings_manager();
                            let mut guard = manager.lock();
                            let settings = guard.get_mut();
                            info!(target: "4da::license",
                                old_tier = %current_tier,
                                new_tier = %result.tier,
                                detail = %result.detail,
                                "Tier updated after startup Keygen validation"
                            );
                            settings.license.tier = result.tier;
                            if let Err(e) = guard.save() {
                                warn!("Failed to save settings: {e}");
                            }
                        } else {
                            info!(target: "4da::license",
                                tier = %result.tier,
                                cached = result.cached,
                                detail = %result.detail,
                                "Startup license validation complete"
                            );
                        }
                    });
                }
            }

            // Refresh model registry (fire-and-forget, ≤1x/24h)
            tauri::async_runtime::spawn(async {
                if let Err(e) = model_registry::refresh_registry().await {
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
        })
        .build(tauri::generate_context!())
        .expect("Failed to build Tauri application. Check tauri.conf.json and system permissions.")
        .run(|app_handle, event| {
            // Hide-to-tray: intercept window close when enabled
            if let tauri::RunEvent::WindowEvent { event: tauri::WindowEvent::CloseRequested { api, .. }, .. } = &event {
                let close_to_tray = {
                    let settings = get_settings_manager().lock();
                    let user_pref = settings.get().monitoring.close_to_tray;
                    // On Linux with GNOME/Pantheon/Unity (no system tray support),
                    // default to false to prevent the window from becoming unreachable.
                    // Users who install a tray extension can explicitly enable this.
                    #[cfg(target_os = "linux")]
                    let default_value = {
                        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();
                        !desktop.contains("GNOME") && !desktop.contains("PANTHEON") && !desktop.contains("UNITY")
                    };
                    #[cfg(not(target_os = "linux"))]
                    let default_value = true;
                    user_pref.unwrap_or(default_value)
                };
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
            let report = content_integrity::verify_content_integrity(&conn, true);
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
