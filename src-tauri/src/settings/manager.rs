//! SettingsManager — loading, saving, and accessing settings
//!
//! Handles disk I/O, keychain migration, locale detection,
//! usage tracking, and all SettingsManager methods.

use super::helpers::detect_system_locale;
use super::keystore;
use super::types::*;
use crate::error::Result;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

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

        // Auto-detect system locale if still at defaults (first run for non-US users)
        if settings.locale.country == "US"
            && settings.locale.language == "en"
            && settings.locale.currency == "USD"
        {
            let detected = detect_system_locale();
            if detected.country != "US" || detected.language != "en" {
                info!(target: "4da::settings", country = %detected.country, language = %detected.language, currency = %detected.currency, "Auto-detected system locale");
                settings.locale = detected;
            }
        }

        // Validate settings, clamping any out-of-range values
        settings.validate();

        // Migrate legacy tier names: "pro" -> "signal"
        if settings.license.tier == "pro" {
            info!(target: "4da::settings", "Migrated legacy tier 'pro' -> 'signal'");
            settings.license.tier = "signal".to_string();
            // Persist the migration so it only logs once (atomic write)
            if let Some(parent) = settings_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(&settings) {
                let tmp_path = settings_path.with_extension("json.tmp");
                if fs::write(&tmp_path, &json).is_ok() {
                    let _ = fs::rename(&tmp_path, &settings_path);
                }
            }
        }

        // --- Keychain migration: move plaintext keys to platform keychain ---
        let has_plaintext_keys = !settings.llm.api_key.is_empty()
            || !settings.llm.openai_api_key.is_empty()
            || !settings.x_api_key.is_empty()
            || !settings.license.license_key.is_empty();

        if has_plaintext_keys {
            match keystore::migrate_from_plaintext(&settings) {
                Ok(report) => {
                    if !report.migrated.is_empty() {
                        // Clear plaintext keys from the JSON file (keep in-memory copy)
                        let mut clean_settings = settings.clone();
                        clean_settings.llm.api_key = String::new();
                        clean_settings.llm.openai_api_key = String::new();
                        clean_settings.x_api_key = String::new();
                        clean_settings.license.license_key = String::new();

                        if let Some(parent) = settings_path.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        if let Ok(json) = serde_json::to_string_pretty(&clean_settings) {
                            let tmp_path = settings_path.with_extension("json.tmp");
                            if fs::write(&tmp_path, &json).is_ok() {
                                let _ = fs::rename(&tmp_path, &settings_path);
                            }
                        }
                        info!(
                            target: "4da::keystore",
                            count = report.migrated.len(),
                            "Cleared plaintext keys from settings.json after keychain migration"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        target: "4da::keystore",
                        error = %e,
                        "Keychain migration failed -- keys remain in plaintext"
                    );
                }
            }
        }

        // --- Hydrate keys from keychain into in-memory settings ---
        if let Ok(Some(key)) = keystore::get_secret("llm_api_key") {
            if !key.is_empty() {
                settings.llm.api_key = key;
            }
        }
        if let Ok(Some(key)) = keystore::get_secret("openai_api_key") {
            if !key.is_empty() {
                settings.llm.openai_api_key = key;
            }
        }
        if let Ok(Some(key)) = keystore::get_secret("x_api_key") {
            if !key.is_empty() {
                settings.x_api_key = key;
            }
        }
        if let Ok(Some(key)) = keystore::get_secret("license_key") {
            if !key.is_empty() {
                settings.license.license_key = key;
            }
        }

        Self {
            settings,
            usage,
            settings_path,
            usage_path,
        }
    }

    /// Save settings to disk (excludes usage -- that's saved separately).
    ///
    /// API keys are stripped from the on-disk copy -- they live in the
    /// platform keychain. The in-memory `self.settings` remains intact.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.settings_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Clone settings and clear secret fields that are safely in the keychain.
        // Only strip a key from disk if it's verified in the keychain -- otherwise
        // keep the plaintext version so the app still works without a keychain.
        let mut disk_settings = self.settings.clone();
        if keystore::has_secret("llm_api_key") {
            disk_settings.llm.api_key = String::new();
        }
        if keystore::has_secret("openai_api_key") {
            disk_settings.llm.openai_api_key = String::new();
        }
        if keystore::has_secret("x_api_key") {
            disk_settings.x_api_key = String::new();
        }
        if keystore::has_secret("license_key") {
            disk_settings.license.license_key = String::new();
        }

        let json = serde_json::to_string_pretty(&disk_settings)?;

        // Atomic write: write to temp file then rename, so a crash mid-write
        // won't corrupt the original settings.json. fs::rename is atomic on
        // the same volume.
        let tmp_path = self.settings_path.with_extension("json.tmp");
        fs::write(&tmp_path, &json)?;
        fs::rename(&tmp_path, &self.settings_path)?;

        // Restrict file permissions to owner-only
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.settings_path, fs::Permissions::from_mode(0o600));
        }

        Ok(())
    }

    /// Save usage stats to disk (atomic: temp file → rename)
    fn save_usage(&self) -> Result<()> {
        if let Some(parent) = self.usage_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&self.usage)?;
        let tmp_path = self.usage_path.with_extension("json.tmp");
        fs::write(&tmp_path, &json)?;
        fs::rename(&tmp_path, &self.usage_path)?;
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

    /// Get the path to the settings file (for tests / diagnostics)
    pub fn get_settings_path(&self) -> &std::path::Path {
        &self.settings_path
    }

    /// Update LLM provider settings.
    ///
    /// Keys are persisted to the platform keychain (if available) and stripped
    /// from the on-disk JSON. The in-memory `settings.llm` retains the keys.
    pub fn set_llm_provider(&mut self, provider: LLMProvider) -> Result<()> {
        if !provider.api_key.is_empty() {
            let _ = keystore::store_secret("llm_api_key", &provider.api_key);
        }
        if !provider.openai_api_key.is_empty() {
            let _ = keystore::store_secret("openai_api_key", &provider.openai_api_key);
        }
        self.settings.llm = provider;
        self.save()
    }

    /// Update re-rank configuration
    pub fn set_rerank_config(&mut self, config: RerankConfig) -> Result<()> {
        self.settings.rerank = config;
        self.save()
    }

    /// Update LLM rate-limiting configuration
    pub fn set_llm_limits(&mut self, config: LlmLimitsConfig) -> Result<()> {
        self.settings.llm_limits = config;
        self.save()
    }

    /// Update monitoring configuration
    pub fn set_monitoring_config(&mut self, config: MonitoringConfig) -> Result<()> {
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
    pub fn mark_auto_discovery_completed(&mut self) -> Result<()> {
        self.settings.auto_discovery_completed = true;
        self.save()
    }

    /// Mark onboarding as completed
    pub fn mark_onboarding_complete(&mut self) -> Result<()> {
        self.settings.onboarding_complete = true;
        self.save()
    }

    /// Add discovered directories to context_dirs
    pub fn add_context_dirs(&mut self, dirs: Vec<String>) -> Result<()> {
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
    pub fn add_rss_feed(&mut self, url: String) -> Result<()> {
        if !self.settings.rss_feeds.contains(&url) {
            self.settings.rss_feeds.push(url);
            self.save()?;
        }
        Ok(())
    }

    /// Remove an RSS feed URL
    pub fn remove_rss_feed(&mut self, url: &str) -> Result<()> {
        self.settings.rss_feeds.retain(|f| f != url);
        self.save()
    }

    /// Set all RSS feed URLs (replacing existing)
    pub fn set_rss_feeds(&mut self, feeds: Vec<String>) -> Result<()> {
        self.settings.rss_feeds = feeds;
        self.save()
    }

    /// Get configured Twitter handles
    pub fn get_twitter_handles(&self) -> Vec<String> {
        self.settings.twitter_handles.clone()
    }

    /// Add a Twitter handle
    pub fn add_twitter_handle(&mut self, handle: String) -> Result<()> {
        if !self.settings.twitter_handles.contains(&handle) {
            self.settings.twitter_handles.push(handle);
            self.save()?;
        }
        Ok(())
    }

    /// Remove a Twitter handle
    pub fn remove_twitter_handle(&mut self, handle: &str) -> Result<()> {
        self.settings.twitter_handles.retain(|h| h != handle);
        self.save()
    }

    /// Set all Twitter handles (replacing existing)
    pub fn set_twitter_handles(&mut self, handles: Vec<String>) -> Result<()> {
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
    pub fn set_nitter_instance(&mut self, instance: String) -> Result<()> {
        self.settings.nitter_instance = Some(instance);
        self.save()
    }

    /// Get X API Bearer Token
    pub fn get_x_api_key(&self) -> String {
        self.settings.x_api_key.clone()
    }

    /// Set X API Bearer Token
    pub fn set_x_api_key(&mut self, key: String) -> Result<()> {
        if !key.is_empty() {
            let _ = keystore::store_secret("x_api_key", &key);
        }
        self.settings.x_api_key = key;
        self.save()
    }

    /// Get YouTube channel IDs
    pub fn get_youtube_channels(&self) -> Vec<String> {
        self.settings.youtube_channels.clone()
    }

    /// Add a YouTube channel ID
    pub fn add_youtube_channel(&mut self, channel_id: String) -> Result<()> {
        if !self.settings.youtube_channels.contains(&channel_id) {
            self.settings.youtube_channels.push(channel_id);
            self.save()?;
        }
        Ok(())
    }

    /// Remove a YouTube channel ID
    pub fn remove_youtube_channel(&mut self, channel_id: &str) -> Result<()> {
        self.settings.youtube_channels.retain(|c| c != channel_id);
        self.save()
    }

    /// Set all YouTube channel IDs (replacing existing)
    pub fn set_youtube_channels(&mut self, channels: Vec<String>) -> Result<()> {
        self.settings.youtube_channels = channels;
        self.save()
    }

    /// Get GitHub languages to track
    pub fn get_github_languages(&self) -> Vec<String> {
        self.settings.github_languages.clone()
    }

    /// Set GitHub languages to track (replacing existing)
    pub fn set_github_languages(&mut self, languages: Vec<String>) -> Result<()> {
        self.settings.github_languages = languages;
        self.save()
    }
}
