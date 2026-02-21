#![allow(dead_code)]
//! Settings module for 4DA
//!
//! Manages user configuration including API keys (BYOK), preferences,
//! and usage limits. Settings are stored in the app data directory.

use crate::digest::DigestConfig;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

/// Maximum directories to discover to prevent OOM
const MAX_DISCOVERED_DIRECTORIES: usize = 1000;

// ============================================================================
// Settings Types
// ============================================================================

/// LLM Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMProvider {
    /// Provider type: "anthropic", "openai", "ollama"
    pub provider: String,
    /// API key for the selected provider (empty for Ollama)
    pub api_key: String,
    /// Model to use (e.g., "claude-3-haiku-20240307", "gpt-4o-mini")
    pub model: String,
    /// Base URL (for Ollama or custom endpoints)
    pub base_url: Option<String>,
    /// OpenAI API key specifically for embeddings (used when provider is not OpenAI)
    #[serde(default)]
    pub openai_api_key: String,
}

impl Default for LLMProvider {
    fn default() -> Self {
        Self {
            provider: "none".to_string(),
            api_key: String::new(),
            model: String::new(),
            base_url: None,
            openai_api_key: String::new(),
        }
    }
}

/// Cost tracking for LLM usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    /// Total tokens used today
    pub tokens_today: u64,
    /// Estimated cost today (USD cents)
    pub cost_today_cents: u64,
    /// Date of current stats (YYYY-MM-DD)
    pub stats_date: String,
    /// Total tokens used all time
    pub tokens_total: u64,
    /// Total items re-ranked
    pub items_reranked: u64,
}

impl Default for UsageStats {
    fn default() -> Self {
        Self {
            tokens_today: 0,
            cost_today_cents: 0,
            stats_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            tokens_total: 0,
            items_reranked: 0,
        }
    }
}

/// Re-ranking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankConfig {
    /// Whether LLM re-ranking is enabled
    pub enabled: bool,
    /// Maximum items to send to LLM per analysis (cost control)
    pub max_items_per_batch: usize,
    /// Minimum embedding score to consider for re-ranking
    pub min_embedding_score: f32,
    /// Daily token limit (0 = unlimited)
    pub daily_token_limit: u64,
    /// Daily cost limit in cents (0 = unlimited)
    pub daily_cost_limit_cents: u64,
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_items_per_batch: 48,
            min_embedding_score: 0.20,
            daily_token_limit: 500_000, // generous for Ollama (free) and cloud
            daily_cost_limit_cents: 100, // $1.00/day default limit
        }
    }
}

/// Monitoring configuration (persisted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Whether continuous monitoring is enabled
    pub enabled: bool,
    /// Interval between checks in minutes
    pub interval_minutes: u64,
    /// Notification quality threshold: "critical_only", "high_and_above" (default), "all"
    #[serde(default = "default_notification_threshold")]
    pub notification_threshold: String,
    /// Max age in days for source_items before cleanup (default: 30)
    #[serde(default)]
    pub cleanup_max_age_days: Option<u32>,
    /// When true, closing the window hides to tray instead of quitting
    #[serde(default)]
    pub close_to_tray: Option<bool>,
    /// When true, auto-generate briefing when critical signals are detected
    #[serde(default)]
    pub auto_briefing_on_critical: Option<bool>,
}

fn default_notification_threshold() -> String {
    "high_and_above".to_string()
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,        // Autonomous by default - no manual enabling needed
            interval_minutes: 10, // Check every 10 minutes
            notification_threshold: default_notification_threshold(),
            cleanup_max_age_days: None, // Uses 30 days default in monitoring.rs
            close_to_tray: None,        // Defaults to true via unwrap_or(true)
            auto_briefing_on_critical: None,
        }
    }
}

/// Predictive context switching configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictiveConfig {
    pub enabled: bool,
    pub prefetch_window_minutes: u32,
}

impl Default for PredictiveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prefetch_window_minutes: 30,
        }
    }
}

/// Serendipity engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerendipityConfig {
    pub enabled: bool,
    pub budget_percent: u8,
}

impl Default for SerendipityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            budget_percent: 8,
        }
    }
}

/// Audio briefing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioBriefingConfig {
    pub enabled: bool,
    pub tts_model: String,
    pub max_duration_seconds: u32,
}

impl Default for AudioBriefingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tts_model: "auto".to_string(),
            max_duration_seconds: 180,
        }
    }
}

/// Project health radar configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthRadarConfig {
    pub enabled: bool,
    pub check_interval_hours: u32,
}

impl Default for HealthRadarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval_hours: 24,
        }
    }
}

/// Attention tracking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionConfig {
    pub enabled: bool,
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// License tier configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseConfig {
    /// Tier: "free", "pro", or "team"
    pub tier: String,
    /// License key (empty for free tier)
    pub license_key: String,
    /// ISO timestamp when license was activated
    pub activated_at: Option<String>,
    /// ISO timestamp when the free trial started (set on first launch)
    #[serde(default)]
    pub trial_started_at: Option<String>,
}

impl Default for LicenseConfig {
    fn default() -> Self {
        Self {
            tier: "free".to_string(),
            license_key: String::new(),
            activated_at: None,
            trial_started_at: None,
        }
    }
}

/// Per-source circuit breaker / resilience configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceResilienceConfig {
    /// Maximum consecutive failures before the circuit breaker opens
    #[serde(default = "default_max_failures")]
    pub max_failures: u32,
    /// Cooldown period in seconds before retrying after circuit opens
    #[serde(default = "default_cooldown_seconds")]
    pub cooldown_seconds: u64,
}

fn default_max_failures() -> u32 {
    5
}

fn default_cooldown_seconds() -> u64 {
    600
}

impl Default for SourceResilienceConfig {
    fn default() -> Self {
        Self {
            max_failures: default_max_failures(),
            cooldown_seconds: default_cooldown_seconds(),
        }
    }
}

/// Per-source rate budget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateBudgetConfig {
    /// Maximum requests allowed per minute for this source
    pub requests_per_minute: u32,
}

impl Default for RateBudgetConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 30,
        }
    }
}

/// Build the default rate budget map for known sources
fn default_rate_budgets() -> std::collections::HashMap<String, RateBudgetConfig> {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "hackernews".to_string(),
        RateBudgetConfig {
            requests_per_minute: 30,
        },
    );
    map.insert(
        "reddit".to_string(),
        RateBudgetConfig {
            requests_per_minute: 10,
        },
    );
    map.insert(
        "github".to_string(),
        RateBudgetConfig {
            requests_per_minute: 25,
        },
    );
    map.insert(
        "twitter".to_string(),
        RateBudgetConfig {
            requests_per_minute: 15,
        },
    );
    map.insert(
        "arxiv".to_string(),
        RateBudgetConfig {
            requests_per_minute: 30,
        },
    );
    map.insert(
        "rss".to_string(),
        RateBudgetConfig {
            requests_per_minute: 30,
        },
    );
    map.insert(
        "youtube".to_string(),
        RateBudgetConfig {
            requests_per_minute: 30,
        },
    );
    map.insert(
        "lobsters".to_string(),
        RateBudgetConfig {
            requests_per_minute: 30,
        },
    );
    map.insert(
        "devto".to_string(),
        RateBudgetConfig {
            requests_per_minute: 30,
        },
    );
    map.insert(
        "producthunt".to_string(),
        RateBudgetConfig {
            requests_per_minute: 30,
        },
    );
    map
}

/// Build the default source resilience map (empty — all sources use built-in defaults)
fn default_source_resilience() -> std::collections::HashMap<String, SourceResilienceConfig> {
    std::collections::HashMap::new()
}

/// Locale configuration for regional content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleConfig {
    /// ISO 3166-1 alpha-2 country code (e.g., "US", "GB", "DE")
    pub country: String,
    /// BCP 47 language tag (e.g., "en", "de", "fr")
    pub language: String,
    /// ISO 4217 currency code (e.g., "USD", "EUR", "GBP")
    pub currency: String,
}

impl Default for LocaleConfig {
    fn default() -> Self {
        Self {
            country: "US".to_string(),
            language: "en".to_string(),
            currency: "USD".to_string(),
        }
    }
}

/// Detect system locale from OS environment
pub fn detect_system_locale() -> LocaleConfig {
    // Try LANG/LC_ALL env vars on Unix, or system locale on Windows
    let lang = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default();

    // Parse "en_US.UTF-8" -> country=US, language=en
    if let Some((language, rest)) = lang.split_once('_') {
        let country = rest.split('.').next().unwrap_or("US").to_uppercase();
        let language = language.to_lowercase();
        let currency = country_to_currency(&country);
        return LocaleConfig {
            country,
            language,
            currency,
        };
    }

    LocaleConfig::default()
}

fn country_to_currency(country: &str) -> String {
    match country {
        "US" => "USD",
        "GB" => "GBP",
        "DE" | "FR" | "NL" | "IT" | "ES" | "AT" | "BE" | "FI" | "IE" | "PT" => "EUR",
        "CA" => "CAD",
        "AU" => "AUD",
        "JP" => "JPY",
        "IN" => "INR",
        "BR" => "BRL",
        "CH" => "CHF",
        "SE" => "SEK",
        "NO" => "NOK",
        "DK" => "DKK",
        "NZ" => "NZD",
        "KR" => "KRW",
        "CN" => "CNY",
        "SG" => "SGD",
        "MX" => "MXN",
        _ => "USD",
    }
    .to_string()
}

/// Main settings structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// LLM provider configuration
    pub llm: LLMProvider,
    /// Re-ranking configuration
    pub rerank: RerankConfig,
    /// Usage statistics — kept for backwards-compatible deserialization, but
    /// runtime usage is stored in a separate `usage.json` file.
    #[serde(default, skip_serializing)]
    pub usage: UsageStats,
    /// Context directories to watch
    pub context_dirs: Vec<String>,
    /// Embedding relevance threshold (Stage 1)
    pub embedding_threshold: f32,
    /// Monitoring configuration
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    /// Whether first-run auto-discovery has been completed
    #[serde(default)]
    pub auto_discovery_completed: bool,
    /// Whether onboarding wizard has been completed
    #[serde(default)]
    pub onboarding_complete: bool,
    /// Email digest configuration
    #[serde(default)]
    pub digest: DigestConfig,
    /// RSS feed URLs to monitor
    #[serde(default)]
    pub rss_feeds: Vec<String>,
    /// Twitter handles to monitor
    #[serde(default)]
    pub twitter_handles: Vec<String>,
    /// Nitter instance to use for Twitter RSS (deprecated, kept for compat)
    #[serde(default)]
    pub nitter_instance: Option<String>,
    /// X API Bearer Token (BYOK)
    #[serde(default)]
    pub x_api_key: String,
    /// YouTube channel IDs to monitor (free, no API key needed)
    #[serde(default)]
    pub youtube_channels: Vec<String>,
    /// GitHub programming languages to track trending repos
    #[serde(default)]
    pub github_languages: Vec<String>,
    /// Predictive context switching
    #[serde(default)]
    pub predictive: PredictiveConfig,
    /// Serendipity engine (anti-bubble)
    #[serde(default)]
    pub serendipity: SerendipityConfig,
    /// Audio briefing
    #[serde(default)]
    pub audio_briefing: AudioBriefingConfig,
    /// Project health radar
    #[serde(default)]
    pub health_radar: HealthRadarConfig,
    /// Attention tracking
    #[serde(default)]
    pub attention: AttentionConfig,
    /// License tier configuration
    #[serde(default)]
    pub license: LicenseConfig,
    /// Per-source circuit breaker configuration overrides
    #[serde(default = "default_source_resilience")]
    pub source_resilience: std::collections::HashMap<String, SourceResilienceConfig>,
    /// Per-source rate budget configuration
    #[serde(default = "default_rate_budgets")]
    pub rate_budgets: std::collections::HashMap<String, RateBudgetConfig>,
    /// Locale configuration for regional content
    #[serde(default)]
    pub locale: LocaleConfig,
}

impl Settings {
    /// Validate settings values, clamping invalid ranges to safe defaults.
    /// Logs warnings for any clamped values but never crashes.
    pub fn validate(&mut self) {
        // rerank.max_items_per_batch must be > 0
        if self.rerank.max_items_per_batch == 0 {
            tracing::warn!(target: "4da::settings", field = "rerank.max_items_per_batch", old = 0, new = 1, "Clamped invalid value");
            self.rerank.max_items_per_batch = 1;
        }

        // rerank.min_embedding_score must be in 0.0..=1.0
        if self.rerank.min_embedding_score < 0.0 || self.rerank.min_embedding_score > 1.0 {
            let old = self.rerank.min_embedding_score;
            self.rerank.min_embedding_score = old.clamp(0.0, 1.0);
            tracing::warn!(target: "4da::settings", field = "rerank.min_embedding_score", old, new = self.rerank.min_embedding_score, "Clamped invalid value");
        }

        // embedding_threshold must be in 0.0..=1.0
        if self.embedding_threshold < 0.0 || self.embedding_threshold > 1.0 {
            let old = self.embedding_threshold;
            self.embedding_threshold = old.clamp(0.0, 1.0);
            tracing::warn!(target: "4da::settings", field = "embedding_threshold", old, new = self.embedding_threshold, "Clamped invalid value");
        }

        // monitoring.interval_minutes must be > 0
        if self.monitoring.interval_minutes == 0 {
            tracing::warn!(target: "4da::settings", field = "monitoring.interval_minutes", old = 0, new = 1, "Clamped invalid value");
            self.monitoring.interval_minutes = 1;
        }

        // context_dirs paths must be non-empty strings
        let before = self.context_dirs.len();
        self.context_dirs.retain(|d| !d.trim().is_empty());
        if self.context_dirs.len() < before {
            tracing::warn!(target: "4da::settings", removed = before - self.context_dirs.len(), "Removed empty context_dirs entries");
        }

        // serendipity.budget_percent should be 0-100
        if self.serendipity.budget_percent > 100 {
            tracing::warn!(target: "4da::settings", field = "serendipity.budget_percent", old = self.serendipity.budget_percent, new = 100, "Clamped invalid value");
            self.serendipity.budget_percent = 100;
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            llm: LLMProvider::default(),
            rerank: RerankConfig::default(),
            usage: UsageStats::default(),
            context_dirs: vec![],
            embedding_threshold: 0.50,
            monitoring: MonitoringConfig::default(),
            auto_discovery_completed: false,
            onboarding_complete: false,
            digest: DigestConfig::default(),
            rss_feeds: vec![],
            twitter_handles: vec![],
            nitter_instance: None,
            x_api_key: String::new(),
            youtube_channels: vec![],
            github_languages: vec![],
            predictive: PredictiveConfig::default(),
            serendipity: SerendipityConfig::default(),
            audio_briefing: AudioBriefingConfig::default(),
            health_radar: HealthRadarConfig::default(),
            attention: AttentionConfig::default(),
            license: LicenseConfig::default(),
            source_resilience: default_source_resilience(),
            rate_budgets: default_rate_budgets(),
            locale: LocaleConfig::default(),
        }
    }
}

// ============================================================================
// Settings Manager
// ============================================================================

/// Manages loading, saving, and accessing settings
pub struct SettingsManager {
    settings: Settings,
    usage: UsageStats,
    settings_path: PathBuf,
    usage_path: PathBuf,
}

impl SettingsManager {
    /// Create a new settings manager, loading from disk if available
    pub fn new(data_dir: &std::path::Path) -> Self {
        let settings_path = data_dir.join("settings.json");
        let usage_path = data_dir.join("usage.json");

        let mut settings = if settings_path.exists() {
            match fs::read_to_string(&settings_path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                    warn!(target: "4da::settings", error = %e, "Failed to parse settings");
                    Settings::default()
                }),
                Err(e) => {
                    warn!(target: "4da::settings", error = %e, "Failed to read settings");
                    Settings::default()
                }
            }
        } else {
            info!(target: "4da::settings", "No settings file found, using defaults");
            Settings::default()
        };

        // Load usage from separate file, falling back to settings.usage for migration
        let usage = if usage_path.exists() {
            match fs::read_to_string(&usage_path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                    warn!(target: "4da::settings", error = %e, "Failed to parse usage.json");
                    UsageStats::default()
                }),
                Err(e) => {
                    warn!(target: "4da::settings", error = %e, "Failed to read usage.json");
                    UsageStats::default()
                }
            }
        } else if settings.usage.tokens_total > 0 {
            // Migrate: usage was in settings.json, move it out
            info!(target: "4da::settings", "Migrating usage stats from settings.json to usage.json");
            let migrated = settings.usage.clone();
            settings.usage = UsageStats::default();
            migrated
        } else {
            UsageStats::default()
        };

        // Validate settings, clamping any out-of-range values
        settings.validate();

        Self {
            settings,
            usage,
            settings_path,
            usage_path,
        }
    }

    /// Save settings to disk (excludes usage — that's saved separately)
    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.settings_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let json = serde_json::to_string_pretty(&self.settings).map_err(|e| e.to_string())?;
        fs::write(&self.settings_path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Save usage stats to disk
    fn save_usage(&self) -> Result<(), String> {
        if let Some(parent) = self.usage_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let json = serde_json::to_string_pretty(&self.usage).map_err(|e| e.to_string())?;
        fs::write(&self.usage_path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Get current settings
    pub fn get(&self) -> &Settings {
        &self.settings
    }

    /// Get mutable settings
    pub fn get_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    /// Update LLM provider settings
    pub fn set_llm_provider(&mut self, provider: LLMProvider) -> Result<(), String> {
        self.settings.llm = provider;
        self.save()
    }

    /// Update re-rank configuration
    pub fn set_rerank_config(&mut self, config: RerankConfig) -> Result<(), String> {
        self.settings.rerank = config;
        self.save()
    }

    /// Update monitoring configuration
    pub fn set_monitoring_config(&mut self, config: MonitoringConfig) -> Result<(), String> {
        self.settings.monitoring = config;
        self.save()
    }

    /// Get monitoring configuration
    pub fn get_monitoring_config(&self) -> &MonitoringConfig {
        &self.settings.monitoring
    }

    /// Check if LLM re-ranking is configured and enabled
    pub fn is_rerank_enabled(&self) -> bool {
        self.settings.rerank.enabled
            && self.settings.llm.provider != "none"
            && (self.settings.llm.provider == "ollama" || !self.settings.llm.api_key.is_empty())
    }

    /// Get usage stats
    pub fn get_usage(&self) -> &UsageStats {
        &self.usage
    }

    /// Check if within daily limits
    pub fn within_daily_limits(&mut self) -> bool {
        // Reset stats if new day
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        if self.usage.stats_date != today {
            self.usage.tokens_today = 0;
            self.usage.cost_today_cents = 0;
            self.usage.stats_date = today;
            let _ = self.save_usage();
        }

        let token_ok = self.settings.rerank.daily_token_limit == 0
            || self.usage.tokens_today < self.settings.rerank.daily_token_limit;

        let cost_ok = self.settings.rerank.daily_cost_limit_cents == 0
            || self.usage.cost_today_cents < self.settings.rerank.daily_cost_limit_cents;

        token_ok && cost_ok
    }

    /// Record token usage (called after LLM/embedding API calls)
    pub fn record_usage(&mut self, tokens: u64, cost_cents: u64) {
        self.usage.tokens_today += tokens;
        self.usage.cost_today_cents += cost_cents;
        self.usage.tokens_total += tokens;
        self.usage.items_reranked += 1;
        let _ = self.save_usage();
    }

    /// Get usage summary
    pub fn usage_summary(&self) -> String {
        format!(
            "Today: {} tokens (~${:.3}) | Total: {} tokens | {} items re-ranked",
            self.usage.tokens_today,
            self.usage.cost_today_cents as f64 / 100.0,
            self.usage.tokens_total,
            self.usage.items_reranked
        )
    }

    /// Check if auto-discovery has been completed
    pub fn needs_auto_discovery(&self) -> bool {
        !self.settings.auto_discovery_completed && self.settings.context_dirs.is_empty()
    }

    /// Mark auto-discovery as completed
    pub fn mark_auto_discovery_completed(&mut self) -> Result<(), String> {
        self.settings.auto_discovery_completed = true;
        self.save()
    }

    /// Mark onboarding as completed
    pub fn mark_onboarding_complete(&mut self) -> Result<(), String> {
        self.settings.onboarding_complete = true;
        self.save()
    }

    /// Add discovered directories to context_dirs
    pub fn add_context_dirs(&mut self, dirs: Vec<String>) -> Result<(), String> {
        for dir in dirs {
            if !self.settings.context_dirs.contains(&dir) {
                self.settings.context_dirs.push(dir);
            }
        }
        self.save()
    }

    /// Get RSS feed URLs
    pub fn get_rss_feeds(&self) -> Vec<String> {
        self.settings.rss_feeds.clone()
    }

    /// Add an RSS feed URL
    pub fn add_rss_feed(&mut self, url: String) -> Result<(), String> {
        if !self.settings.rss_feeds.contains(&url) {
            self.settings.rss_feeds.push(url);
            self.save()?;
        }
        Ok(())
    }

    /// Remove an RSS feed URL
    pub fn remove_rss_feed(&mut self, url: &str) -> Result<(), String> {
        self.settings.rss_feeds.retain(|f| f != url);
        self.save()
    }

    /// Set all RSS feed URLs (replacing existing)
    pub fn set_rss_feeds(&mut self, feeds: Vec<String>) -> Result<(), String> {
        self.settings.rss_feeds = feeds;
        self.save()
    }

    /// Get configured Twitter handles
    pub fn get_twitter_handles(&self) -> Vec<String> {
        self.settings.twitter_handles.clone()
    }

    /// Add a Twitter handle
    pub fn add_twitter_handle(&mut self, handle: String) -> Result<(), String> {
        if !self.settings.twitter_handles.contains(&handle) {
            self.settings.twitter_handles.push(handle);
            self.save()?;
        }
        Ok(())
    }

    /// Remove a Twitter handle
    pub fn remove_twitter_handle(&mut self, handle: &str) -> Result<(), String> {
        self.settings.twitter_handles.retain(|h| h != handle);
        self.save()
    }

    /// Set all Twitter handles (replacing existing)
    pub fn set_twitter_handles(&mut self, handles: Vec<String>) -> Result<(), String> {
        self.settings.twitter_handles = handles;
        self.save()
    }

    /// Get configured Nitter instance
    pub fn get_nitter_instance(&self) -> String {
        self.settings
            .nitter_instance
            .clone()
            .unwrap_or_else(|| "nitter.net".to_string())
    }

    /// Set Nitter instance
    pub fn set_nitter_instance(&mut self, instance: String) -> Result<(), String> {
        self.settings.nitter_instance = Some(instance);
        self.save()
    }

    /// Get X API Bearer Token
    pub fn get_x_api_key(&self) -> String {
        self.settings.x_api_key.clone()
    }

    /// Set X API Bearer Token
    pub fn set_x_api_key(&mut self, key: String) -> Result<(), String> {
        self.settings.x_api_key = key;
        self.save()
    }

    /// Get YouTube channel IDs
    pub fn get_youtube_channels(&self) -> Vec<String> {
        self.settings.youtube_channels.clone()
    }

    /// Add a YouTube channel ID
    pub fn add_youtube_channel(&mut self, channel_id: String) -> Result<(), String> {
        if !self.settings.youtube_channels.contains(&channel_id) {
            self.settings.youtube_channels.push(channel_id);
            self.save()?;
        }
        Ok(())
    }

    /// Remove a YouTube channel ID
    pub fn remove_youtube_channel(&mut self, channel_id: &str) -> Result<(), String> {
        self.settings.youtube_channels.retain(|c| c != channel_id);
        self.save()
    }

    /// Set all YouTube channel IDs (replacing existing)
    pub fn set_youtube_channels(&mut self, channels: Vec<String>) -> Result<(), String> {
        self.settings.youtube_channels = channels;
        self.save()
    }

    /// Get GitHub languages to track
    pub fn get_github_languages(&self) -> Vec<String> {
        self.settings.github_languages.clone()
    }

    /// Set GitHub languages to track (replacing existing)
    pub fn set_github_languages(&mut self, languages: Vec<String>) -> Result<(), String> {
        self.settings.github_languages = languages;
        self.save()
    }
}

// ============================================================================
// Feature Tier Gating
// ============================================================================

/// Pro-gated features list
pub const PRO_FEATURES: &[&str] = &[
    "generate_ai_briefing",
    "get_latest_briefing",
    "generate_audio_briefing",
    "get_attention_report",
    "get_knowledge_gaps",
    "get_signal_chains",
    "get_project_health",
    "get_developer_dna",
    "export_developer_dna_markdown",
    "export_developer_dna_svg",
    "get_predicted_context",
    "generate_context_packet",
    "natural_language_query",
];

/// Check if a feature is available for the given tier, including trial period
pub fn is_pro_feature_available(feature: &str, license: &LicenseConfig) -> bool {
    match license.tier.as_str() {
        "pro" | "team" => true,
        _ => {
            if is_trial_active(license) {
                return true;
            }
            !PRO_FEATURES.contains(&feature)
        }
    }
}

/// Gate a Pro feature — returns Ok(()) if allowed, Err if not
/// Call at the top of any Pro-gated Tauri command
pub fn require_pro_feature(feature: &str) -> Result<(), String> {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let license = &guard.get().license;
    if is_pro_feature_available(feature, license) {
        Ok(())
    } else {
        Err(format!(
            "{} requires 4DA Pro — upgrade or start a free trial",
            feature
        ))
    }
}

/// Check if the free trial is still active (30 days from trial_started_at)
pub fn is_trial_active(license: &LicenseConfig) -> bool {
    if license.tier == "pro" || license.tier == "team" {
        return false; // Not on trial, has a real license
    }
    match &license.trial_started_at {
        Some(started) => {
            if let Ok(start_date) = chrono::DateTime::parse_from_rfc3339(started) {
                let elapsed = chrono::Utc::now().signed_duration_since(start_date);
                elapsed.num_days() < 30
            } else {
                false
            }
        }
        None => false, // Trial not started yet
    }
}

/// Get trial status info
pub fn get_trial_status(license: &LicenseConfig) -> TrialStatus {
    if license.tier == "pro" || license.tier == "team" {
        return TrialStatus {
            active: false,
            days_remaining: 0,
            started_at: None,
            has_license: true,
        };
    }
    match &license.trial_started_at {
        Some(started) => {
            if let Ok(start_date) = chrono::DateTime::parse_from_rfc3339(started) {
                let elapsed = chrono::Utc::now().signed_duration_since(start_date);
                let remaining = 30 - elapsed.num_days();
                TrialStatus {
                    active: remaining > 0,
                    days_remaining: remaining.max(0) as i32,
                    started_at: Some(started.clone()),
                    has_license: false,
                }
            } else {
                TrialStatus {
                    active: false,
                    days_remaining: 0,
                    started_at: Some(started.clone()),
                    has_license: false,
                }
            }
        }
        None => TrialStatus {
            active: false,
            days_remaining: 0,
            started_at: None,
            has_license: false,
        },
    }
}

/// Trial status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialStatus {
    pub active: bool,
    pub days_remaining: i32,
    pub started_at: Option<String>,
    pub has_license: bool,
}

// ============================================================================
// License Key Verification (ed25519)
// ============================================================================

/// Ed25519 public key for license verification (hex-encoded)
/// The private key is held server-side for license generation.
const LICENSE_PUBLIC_KEY_HEX: &str =
    "a1b2c3d4e5f6071829304050607080901a2b3c4d5e6f0a1b2c3d4e5f6070809";

/// License payload embedded in a signed license key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicensePayload {
    pub tier: String,
    pub email: String,
    pub expires_at: String,
    pub issued_at: String,
    #[serde(default)]
    pub features: Vec<String>,
}

/// Verify and decode a license key.
/// Format: `4DA-{base64(json_payload)}.{base64(ed25519_signature)}`
pub fn verify_license_key(key: &str) -> Result<LicensePayload, String> {
    let key = key.trim();

    // Must start with 4DA- prefix
    let body = key
        .strip_prefix("4DA-")
        .ok_or("Invalid license format: must start with 4DA-")?;

    // Split payload and signature
    let parts: Vec<&str> = body.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err("Invalid license format: missing signature".to_string());
    }

    let payload_b64 = parts[0];
    let sig_b64 = parts[1];

    // Decode payload
    let payload_bytes =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, payload_b64)
            .map_err(|e| format!("Invalid payload encoding: {e}"))?;

    // Decode signature
    let sig_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, sig_b64)
        .map_err(|e| format!("Invalid signature encoding: {e}"))?;

    // Decode public key
    let pub_key_bytes =
        hex::decode(LICENSE_PUBLIC_KEY_HEX).map_err(|e| format!("Invalid public key: {e}"))?;

    if pub_key_bytes.len() != 32 {
        return Err("Invalid public key length".to_string());
    }

    if sig_bytes.len() != 64 {
        return Err("Invalid signature length".to_string());
    }

    // Verify ed25519 signature
    use ed25519_dalek::{Signature, VerifyingKey};

    let verifying_key = VerifyingKey::from_bytes(
        pub_key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid public key bytes")?,
    )
    .map_err(|e| format!("Invalid public key: {e}"))?;

    let signature = Signature::from_bytes(
        sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid signature bytes")?,
    );

    use ed25519_dalek::Verifier;
    verifying_key
        .verify(&payload_bytes, &signature)
        .map_err(|_| "Invalid license: signature verification failed".to_string())?;

    // Parse payload JSON
    let payload: LicensePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|e| format!("Invalid license payload: {e}"))?;

    // Check expiration
    if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(&payload.expires_at) {
        if chrono::Utc::now() > expires {
            return Err("License has expired".to_string());
        }
    }

    Ok(payload)
}

// ============================================================================
// Autonomous Directory Discovery
// ============================================================================

/// Discovers directories that define the user's context on their system.
/// This is the core of ACE autonomy - finding ALL relevant context, not just code.
/// Context can come from ANY directory: projects, documents, notes, research, etc.
pub fn discover_dev_directories() -> Vec<String> {
    let mut discovered: Vec<String> = Vec::new();

    // Get home directory
    let home = dirs::home_dir();

    if let Some(home_path) = home {
        // Context-relevant directories (not just dev!)
        let context_dirs = [
            // Development
            "Projects",
            "projects",
            "Development",
            "development",
            "dev",
            "Dev",
            "code",
            "Code",
            "src",
            "work",
            "Work",
            "repos",
            "Repos",
            "github",
            "GitHub",
            "git",
            "workspace",
            "Workspace",
            // Documents & Notes (context!)
            "Documents",
            "documents",
            "Notes",
            "notes",
            "Obsidian",
            "Research",
            "research",
            "Writing",
            "writing",
            // Learning & Reference
            "Books",
            "books",
            "Articles",
            "articles",
            "Papers",
            "papers",
            // Creative/Work
            "Design",
            "design",
            "Creative",
            "creative",
        ];

        for dir_name in context_dirs {
            let dir_path = home_path.join(dir_name);
            if dir_path.exists() && dir_path.is_dir() {
                info!(target: "4da::discovery", path = %dir_path.display(), "Found context directory");
                discovered.push(dir_path.display().to_string());
            }
        }

        // Also check for common WSL mount points (for Windows users in WSL)
        #[cfg(target_os = "linux")]
        {
            let wsl_mounts = ["/mnt/c", "/mnt/d", "/mnt/e"];
            // Context indicators (code AND content)
            let context_markers = [
                "package.json",
                "Cargo.toml",
                "pyproject.toml",
                "go.mod",
                ".git",
                ".obsidian",
                "README.md",
                "index.md",
            ];
            let skip_dirs = [
                "$RECYCLE.BIN",
                "System Volume Information",
                "Windows",
                "Program Files",
                "Program Files (x86)",
                "ProgramData",
                "Recovery",
                "Users",
            ];

            for mount in wsl_mounts {
                let mount_path = PathBuf::from(mount);
                if !mount_path.exists() {
                    continue;
                }

                // Check common locations on mounted drives (including context dirs, not just dev)
                for subdir in [
                    "Users",
                    "projects",
                    "code",
                    "dev",
                    "Documents",
                    "Notes",
                    "Research",
                    "Work",
                ] {
                    let check_path = mount_path.join(subdir);
                    if check_path.exists() && check_path.is_dir() {
                        // Don't add entire Users folder, look for specific user folders
                        if subdir == "Users" {
                            // Try to find user's folder
                            if let Ok(entries) = fs::read_dir(&check_path) {
                                for entry in entries.flatten() {
                                    let user_path = entry.path();
                                    if user_path.is_dir() {
                                        for dev_dir in
                                            ["Projects", "code", "dev", "repos", "source"]
                                        {
                                            let dev_path = user_path.join(dev_dir);
                                            if dev_path.exists() && dev_path.is_dir() {
                                                info!(target: "4da::discovery", path = %dev_path.display(), "Found WSL dev directory");
                                                discovered.push(dev_path.display().to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            info!(target: "4da::discovery", path = %check_path.display(), "Found WSL dev directory");
                            discovered.push(check_path.display().to_string());
                        }
                    }
                }

                // CRITICAL: Also scan root of mounts for project directories
                // This finds projects like /mnt/d/4DA that aren't in standard folders
                info!(target: "4da::discovery", mount = mount, "Scanning mount root for projects");
                if let Ok(entries) = fs::read_dir(&mount_path) {
                    for entry in entries.flatten() {
                        let entry_path = entry.path();
                        let entry_name =
                            entry_path.file_name().unwrap_or_default().to_string_lossy();

                        // Skip system directories
                        if skip_dirs.iter().any(|s| entry_name.eq_ignore_ascii_case(s)) {
                            continue;
                        }

                        // Skip hidden directories and files
                        if entry_name.starts_with('.') || entry_name.starts_with('$') {
                            continue;
                        }

                        if entry_path.is_dir() {
                            // Check if this directory has context markers (code or content)
                            for marker in context_markers {
                                if entry_path.join(marker).exists() {
                                    info!(target: "4da::discovery", path = %entry_path.display(), "Found context directory at mount root");
                                    discovered.push(entry_path.display().to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Deduplicate
    discovered.sort();
    discovered.dedup();

    info!(target: "4da::discovery", count = discovered.len(), "Total directories discovered");
    discovered
}

/// Deep scan for context-defining directories
/// Returns directories containing projects OR significant context (notes, docs, etc.)
pub fn find_project_directories(base_dirs: &[String], max_depth: usize) -> Vec<String> {
    let mut project_dirs: Vec<String> = Vec::new();
    // Context indicators: code manifests AND content markers
    let manifest_files = [
        // Code projects
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "requirements.txt",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "Gemfile",
        "composer.json",
        ".git",
        // Obsidian/notes vaults
        ".obsidian",
        // Documentation
        "README.md",
        "index.md",
    ];

    let skip_dirs = [
        "node_modules",
        "target",
        ".git",
        "dist",
        "build",
        "__pycache__",
        ".next",
        "vendor",
        ".cargo",
    ];

    fn scan_recursive(
        path: &std::path::Path,
        depth: usize,
        max_depth: usize,
        manifests: &[&str],
        skip: &[&str],
        results: &mut Vec<String>,
        max_results: usize,
    ) {
        // Bound check: stop if we've hit the limit
        if results.len() >= max_results || depth > max_depth || !path.is_dir() {
            return;
        }

        // Check if this directory has a manifest
        for manifest in manifests {
            if path.join(manifest).exists() {
                results.push(path.display().to_string());
                return; // Don't recurse deeper once we find a project
            }
        }

        // Recurse into subdirectories
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                // Check bound before each recursion
                if results.len() >= max_results {
                    return;
                }
                let entry_path = entry.path();
                if entry_path.is_dir() && !entry_path.is_symlink() {
                    let name = entry_path.file_name().unwrap_or_default().to_string_lossy();
                    if !skip.contains(&name.as_ref()) {
                        scan_recursive(
                            &entry_path,
                            depth + 1,
                            max_depth,
                            manifests,
                            skip,
                            results,
                            max_results,
                        );
                    }
                }
            }
        }
    }

    for base in base_dirs {
        // Stop if we've hit the limit
        if project_dirs.len() >= MAX_DISCOVERED_DIRECTORIES {
            break;
        }
        let base_path = PathBuf::from(base);
        scan_recursive(
            &base_path,
            0,
            max_depth,
            &manifest_files,
            &skip_dirs,
            &mut project_dirs,
            MAX_DISCOVERED_DIRECTORIES,
        );
    }

    project_dirs.sort();
    project_dirs.dedup();

    info!(target: "4da::discovery", count = project_dirs.len(), "Found project directories");
    project_dirs
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert!(settings.rerank.enabled);
        assert_eq!(settings.embedding_threshold, 0.50);
        assert_eq!(settings.rerank.max_items_per_batch, 48);
    }

    #[test]
    fn test_llm_provider_default() {
        let provider = LLMProvider::default();
        assert_eq!(provider.provider, "none");
        assert!(provider.api_key.is_empty());
    }
}
