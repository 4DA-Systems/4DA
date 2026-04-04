//! Settings type definitions for 4DA
//!
//! All struct and enum definitions, Default impls, serde helpers,
//! and simple impl blocks for settings types.

use crate::community_intelligence::CommunityIntelligenceConfig;
use crate::digest::DigestConfig;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

// ============================================================================
// Settings Types
// ============================================================================

/// LLM Provider configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct LLMProvider {
    /// Provider type: "anthropic", "openai", "ollama", "openai-compatible"
    pub provider: String,
    /// API key for the selected provider (empty for Ollama)
    pub api_key: String,
    /// Model to use (e.g., "claude-haiku-4-5-20251001", "gpt-4o-mini")
    pub model: String,
    /// Base URL (for Ollama or custom endpoints)
    pub base_url: Option<String>,
    /// OpenAI API key specifically for embeddings (used when provider is not OpenAI)
    #[serde(default)]
    pub openai_api_key: String,
    /// Embedding model for semantic search. Default: "nomic-embed-text".
    /// "nomic-embed-text-v2-moe" recommended for multilingual content.
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
}

fn default_embedding_model() -> String {
    "nomic-embed-text".to_string()
}

impl std::fmt::Debug for LLMProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LLMProvider")
            .field("provider", &self.provider)
            .field(
                "api_key",
                &if self.api_key.is_empty() {
                    "(empty)"
                } else {
                    "[REDACTED]"
                },
            )
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field(
                "openai_api_key",
                &if self.openai_api_key.is_empty() {
                    "(empty)"
                } else {
                    "[REDACTED]"
                },
            )
            .field("embedding_model", &self.embedding_model)
            .finish()
    }
}

impl Drop for LLMProvider {
    fn drop(&mut self) {
        self.api_key.zeroize();
        self.openai_api_key.zeroize();
    }
}

impl Default for LLMProvider {
    fn default() -> Self {
        Self {
            provider: "none".to_string(),
            api_key: String::new(),
            model: String::new(),
            base_url: None,
            openai_api_key: String::new(),
            embedding_model: default_embedding_model(),
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
    /// Whether morning briefing notifications are enabled (default: true)
    #[serde(default)]
    pub morning_briefing: Option<bool>,
    /// Time for morning briefing in HH:MM format (default: "08:00")
    #[serde(default)]
    pub briefing_time: Option<String>,
    /// Last morning briefing date (YYYY-MM-DD) to avoid firing twice in one day.
    /// Persisted so restart doesn't re-trigger today's briefing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_briefing_date: Option<String>,
    /// Whether 4DA launches automatically at system startup (default: false)
    #[serde(default)]
    pub launch_at_startup: Option<bool>,
    /// Notification style: "custom" (GAME-powered popup) or "native" (OS toast).
    /// Default: "custom".
    #[serde(default = "default_notification_style")]
    pub notification_style: String,
}

fn default_notification_threshold() -> String {
    "high_and_above".to_string()
}

fn default_notification_style() -> String {
    "custom".to_string()
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
            morning_briefing: None, // Defaults to true via unwrap_or(true)
            briefing_time: None,    // Defaults to "08:00"
            last_briefing_date: None,
            launch_at_startup: None,
            notification_style: default_notification_style(),
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
#[derive(Clone, Serialize, Deserialize)]
pub struct LicenseConfig {
    /// Tier: "free", "signal", "team", or "enterprise" (legacy "pro" also accepted)
    pub tier: String,
    /// License key (empty for free tier)
    pub license_key: String,
    /// ISO timestamp when license was activated
    pub activated_at: Option<String>,
    /// ISO timestamp when the free trial started (set on first launch)
    #[serde(default)]
    pub trial_started_at: Option<String>,
}

impl std::fmt::Debug for LicenseConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LicenseConfig")
            .field("tier", &self.tier)
            .field(
                "license_key",
                &if self.license_key.is_empty() {
                    "(empty)"
                } else {
                    "[REDACTED]"
                },
            )
            .field("activated_at", &self.activated_at)
            .field("trial_started_at", &self.trial_started_at)
            .finish()
    }
}

impl Drop for LicenseConfig {
    fn drop(&mut self) {
        self.license_key.zeroize();
    }
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
pub(crate) fn default_rate_budgets() -> std::collections::HashMap<String, RateBudgetConfig> {
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

/// Build the default source resilience map (empty -- all sources use built-in defaults)
pub(crate) fn default_source_resilience(
) -> std::collections::HashMap<String, SourceResilienceConfig> {
    std::collections::HashMap::new()
}

/// Dedicated translation provider configuration.
/// Separate from the main LLM to allow cheaper, purpose-built translation APIs.
#[derive(Clone, Serialize, Deserialize)]
pub struct TranslationConfig {
    /// Translation provider: "auto" | "deepl" | "google" | "azure" | "ollama" | "llm"
    /// "auto" tries dedicated API first, falls back to LLM
    pub provider: String,
    /// API key for the dedicated translation provider
    pub api_key: String,
    /// Auto-translate feed content titles at ingest time
    pub auto_translate: bool,
    /// Also translate descriptions (not just titles) — more API usage
    pub translate_descriptions: bool,
}

impl std::fmt::Debug for TranslationConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TranslationConfig")
            .field("provider", &self.provider)
            .field(
                "api_key",
                &if self.api_key.is_empty() {
                    "(empty)"
                } else {
                    "[REDACTED]"
                },
            )
            .field("auto_translate", &self.auto_translate)
            .field("translate_descriptions", &self.translate_descriptions)
            .finish()
    }
}

impl Drop for TranslationConfig {
    fn drop(&mut self) {
        self.api_key.zeroize();
    }
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            provider: "auto".to_string(),
            api_key: String::new(),
            auto_translate: true,
            translate_descriptions: false,
        }
    }
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

/// LLM rate-limiting configuration (daily token + cost caps)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmLimitsConfig {
    /// Daily token limit for LLM calls (0 = unlimited)
    pub daily_token_limit: u64,
    /// Daily cost limit in USD cents for LLM calls (0 = unlimited)
    pub daily_cost_limit_cents: u64,
}

impl Default for LlmLimitsConfig {
    fn default() -> Self {
        Self {
            daily_token_limit: 500_000, // generous default — protects against runaway usage
            daily_cost_limit_cents: 200, // $2.00/day default limit
        }
    }
}

/// Network privacy settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Proxy URL (e.g., "socks5://127.0.0.1:9050" for Tor, "http://proxy:8080")
    #[serde(default)]
    pub proxy_url: Option<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self { proxy_url: None }
    }
}

/// Privacy settings for controlling data sent to cloud providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    /// What content level to send to cloud LLM providers.
    /// "full" = title + 2000 chars content (default)
    /// "titles_only" = only titles, no content (95% less data to provider)
    #[serde(default = "default_llm_content_level")]
    pub llm_content_level: String,
}

fn default_llm_content_level() -> String {
    "full".to_string()
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            llm_content_level: default_llm_content_level(),
        }
    }
}

/// Main settings structure
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    /// LLM provider configuration
    pub llm: LLMProvider,
    /// Re-ranking configuration
    pub rerank: RerankConfig,
    /// LLM rate-limiting configuration (daily token + cost caps)
    #[serde(default)]
    pub llm_limits: LlmLimitsConfig,
    /// Usage statistics -- kept for backwards-compatible deserialization, but
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
    /// Dedicated translation provider configuration (DeepL, Google, Azure)
    #[serde(default)]
    pub translation: TranslationConfig,
    /// Community intelligence configuration (opt-in anonymous pattern sharing)
    #[serde(default)]
    pub community_intelligence: Option<CommunityIntelligenceConfig>,
    /// Team relay configuration (encrypted metadata sync between team members)
    #[serde(default)]
    pub team_relay: Option<crate::team_sync_types::TeamRelayConfig>,
    /// Network privacy settings (proxy/SOCKS5 support)
    #[serde(default)]
    pub network: NetworkConfig,
    /// Privacy settings for controlling data sent to cloud providers
    #[serde(default)]
    pub privacy: PrivacyConfig,
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("llm", &self.llm)
            .field("rerank", &self.rerank)
            .field("llm_limits", &self.llm_limits)
            .field("context_dirs", &self.context_dirs)
            .field("embedding_threshold", &self.embedding_threshold)
            .field("monitoring", &self.monitoring)
            .field("auto_discovery_completed", &self.auto_discovery_completed)
            .field("onboarding_complete", &self.onboarding_complete)
            .field("rss_feeds", &format!("[{} feeds]", self.rss_feeds.len()))
            .field(
                "twitter_handles",
                &format!("[{} handles]", self.twitter_handles.len()),
            )
            .field(
                "x_api_key",
                &if self.x_api_key.is_empty() {
                    "(empty)"
                } else {
                    "[REDACTED]"
                },
            )
            .field(
                "youtube_channels",
                &format!("[{} channels]", self.youtube_channels.len()),
            )
            .field("github_languages", &self.github_languages)
            .field("license", &self.license)
            .field("locale", &self.locale)
            .field("translation", &self.translation)
            .field(
                "community_intelligence",
                &self.community_intelligence.is_some(),
            )
            .field("team_relay", &self.team_relay.is_some())
            .field("network", &self.network)
            .field("privacy", &self.privacy)
            .finish_non_exhaustive()
    }
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

        // monitoring.interval_minutes must be in 1..=1440 (1 min to 24 hours)
        {
            let old = self.monitoring.interval_minutes;
            self.monitoring.interval_minutes = old.clamp(1, 1440);
            if self.monitoring.interval_minutes != old {
                tracing::warn!(target: "4da::settings", field = "monitoring.interval_minutes", old, new = self.monitoring.interval_minutes, "Clamped invalid value");
            }
        }

        // rerank.max_items_per_batch upper bound (prevent memory exhaustion)
        {
            let old = self.rerank.max_items_per_batch;
            self.rerank.max_items_per_batch = old.clamp(1, 200);
            if self.rerank.max_items_per_batch != old {
                tracing::warn!(target: "4da::settings", field = "rerank.max_items_per_batch", old, new = self.rerank.max_items_per_batch, "Clamped invalid value");
            }
        }

        // rate_budgets: clamp all requests_per_minute to 1..=120
        for (source, budget) in self.rate_budgets.iter_mut() {
            let old = budget.requests_per_minute;
            budget.requests_per_minute = old.clamp(1, 120);
            if budget.requests_per_minute != old {
                tracing::warn!(target: "4da::settings", field = "rate_budget.requests_per_minute", source = %source, old, new = budget.requests_per_minute, "Clamped invalid value");
            }
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

        // llm.base_url must be a valid URL with http(s) scheme and a host.
        // Only localhost/127.0.0.1 are permitted over plain HTTP; all remote
        // servers must use HTTPS.
        if let Some(ref raw) = self.llm.base_url {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                self.llm.base_url = None;
            } else {
                match url::Url::parse(trimmed) {
                    Ok(parsed) => {
                        let scheme = parsed.scheme();
                        let host = parsed.host_str().unwrap_or("");

                        if scheme != "http" && scheme != "https" {
                            tracing::warn!(
                                target: "4da::settings",
                                field = "llm.base_url",
                                value = trimmed,
                                "Rejected base_url with invalid scheme (must be http or https)"
                            );
                            self.llm.base_url = None;
                        } else if host.is_empty() {
                            tracing::warn!(
                                target: "4da::settings",
                                field = "llm.base_url",
                                value = trimmed,
                                "Rejected base_url with empty host"
                            );
                            self.llm.base_url = None;
                        } else if scheme == "http"
                            && host != "localhost"
                            && host != "127.0.0.1"
                            && host != "::1"
                            && host != "[::1]"
                        {
                            tracing::warn!(
                                target: "4da::settings",
                                field = "llm.base_url",
                                value = trimmed,
                                "Rejected non-HTTPS base_url for remote server (only localhost allowed over HTTP)"
                            );
                            self.llm.base_url = None;
                        } else if scheme == "http" {
                            tracing::warn!(
                                target: "4da::settings",
                                field = "llm.base_url",
                                value = trimmed,
                                "base_url uses HTTP — acceptable for local Ollama but not for remote servers"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "4da::settings",
                            field = "llm.base_url",
                            value = trimmed,
                            error = %e,
                            "Rejected base_url: failed to parse as URL"
                        );
                        self.llm.base_url = None;
                    }
                }
            }
        }
    }
}

impl Drop for Settings {
    fn drop(&mut self) {
        // x_api_key lives on Settings directly (not in a sub-struct with its own Drop)
        self.x_api_key.zeroize();
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            llm: LLMProvider::default(),
            rerank: RerankConfig::default(),
            llm_limits: LlmLimitsConfig::default(),
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
            translation: TranslationConfig::default(),
            community_intelligence: None,
            team_relay: None,
            network: NetworkConfig::default(),
            privacy: PrivacyConfig::default(),
        }
    }
}
