// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! SettingsManager — loading, saving, and accessing settings
//!
//! Handles disk I/O, keychain migration, locale detection,
//! usage tracking, and all SettingsManager methods.

use super::keystore;
use super::secret_storage;
use super::types::*;
use crate::error::Result;
use std::fs;
use std::path::PathBuf;

// ============================================================================
// Atomic file helpers
// ============================================================================

/// Atomic file replacement. On Unix, fs::rename is atomic on the same volume.
/// On Windows, we need a different approach since rename can fail if target exists.
pub(crate) fn atomic_replace(
    tmp: &std::path::Path,
    target: &std::path::Path,
) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        // Try direct rename first (works if target doesn't exist)
        if std::fs::rename(tmp, target).is_ok() {
            return Ok(());
        }
        // Target exists — use a swap file for crash safety. Never the
        // `.json.bak` name: that is the recovery backup and must survive.
        let backup = target.with_extension("swap");
        // Step 1: Rename existing to the swap file
        if target.exists() {
            let _ = std::fs::rename(target, &backup);
        }
        // Step 2: Rename new file into place
        match std::fs::rename(tmp, target) {
            Ok(()) => {
                // Success — clean up backup
                let _ = std::fs::remove_file(&backup);
                Ok(())
            }
            Err(e) => {
                // Failed — restore from backup
                if backup.exists() && !target.exists() {
                    let _ = std::fs::rename(&backup, target);
                }
                Err(e)
            }
        }
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(tmp, target)
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
    /// False only for hermetic test constructors: the keychain is never read
    /// or written and every secret stays in the file.
    keychain_enabled: bool,
    /// Where each present secret is stored (see `secret_storage`).
    secret_posture: secret_storage::PostureMap,
}

// Constructor (new) lives in manager_init.rs — separate impl block.
#[path = "manager_init.rs"]
mod manager_init;

impl SettingsManager {
    /// Save settings to disk (excludes usage -- that's saved separately).
    ///
    /// Keychain-authoritative (audit 2026-10-07, see `secret_storage`): each
    /// present secret is mirrored to the OS credential store and removed from
    /// the written JSON only once a fresh-handle read proves the store holds
    /// it. A secret the store cannot hold stays in the (owner-only) file and
    /// its posture becomes `FileFallback`. Empty secrets never touch the store.
    pub fn save(&mut self) -> Result<()> {
        // Every change to the privacy level is followed by a save, so this keeps
        // the lock-free mirror current (see `llm_egress`).
        crate::llm_egress::publish_content_level(&self.settings.privacy.llm_content_level);

        self.ensure_keys_hydrated();
        self.secret_posture = if self.keychain_enabled {
            secret_storage::mirror_present_secrets(&self.settings)
        } else {
            secret_storage::file_only_posture(&self.settings)
        };

        let mut disk_settings =
            secret_storage::scrubbed_for_disk(&self.settings, &self.secret_posture);
        Self::enforce_license_tier_invariant(&self.settings, &mut disk_settings);

        let json = serde_json::to_string_pretty(&disk_settings)?;
        secret_storage::write_settings_files(&self.settings_path, &json)?;
        Ok(())
    }

    /// License tier invariant: if a valid self-signed key is present, the tier
    /// written to disk MUST match the key's embedded tier.
    fn enforce_license_tier_invariant(settings: &Settings, disk_settings: &mut Settings) {
        if !settings.license.license_key.starts_with("4DA-") {
            return;
        }
        let Ok(payload) = crate::settings::verify_license_key(&settings.license.license_key) else {
            return;
        };
        let expected_tier = match payload.tier.as_str() {
            "signal" | "team" | "enterprise" => payload.tier.clone(),
            "pro" | "community" | "cohort" => "signal".to_string(),
            _ => payload.tier.clone(),
        };
        let expired = chrono::DateTime::parse_from_rfc3339(&payload.expires_at)
            .map(|exp| exp.with_timezone(&chrono::Utc) < crate::settings::license_effective_now())
            .unwrap_or(false);
        if !expired && disk_settings.license.tier != expected_tier {
            tracing::warn!(
                target: "4da::license",
                attempted_tier = %disk_settings.license.tier,
                correct_tier = %expected_tier,
                "Save-time invariant: correcting tier before write"
            );
            disk_settings.license.tier = expected_tier;
        }
    }

    /// Where each present secret is stored, for the settings payload / UI.
    pub fn secret_storage_status(&self) -> secret_storage::SecretStorageStatus {
        secret_storage::status_of(&self.secret_posture)
    }

    /// Save usage stats to disk (atomic: temp file → rename)
    fn save_usage(&self) -> Result<()> {
        if let Some(parent) = self.usage_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&self.usage)?;
        let tmp_path = self.usage_path.with_extension("json.tmp");
        fs::write(&tmp_path, &json)?;
        atomic_replace(&tmp_path, &self.usage_path)?;
        Ok(())
    }

    /// Ensure all keychain-managed secrets are present in memory.
    ///
    /// Called at the start of every `save()` and before any code path that
    /// gates on `api_key.is_empty()`. If a key is empty in memory but
    /// present in the keychain, we pull it back. This is the permanent fix
    /// for the dev-mode hydration race: even if startup hydration fails,
    /// every consumer re-checks before giving up. Never writes to the store.
    pub fn ensure_keys_hydrated(&mut self) {
        if !self.keychain_enabled {
            return;
        }
        for name in secret_storage::SECRET_NAMES {
            if !secret_storage::secret_value(&self.settings, name).is_empty() {
                continue;
            }
            if let Ok(Some(val)) = keystore::get_secret(name) {
                if !val.is_empty() {
                    tracing::info!(
                        target: "4da::keystore",
                        key = name,
                        "Recovered key from keychain — was empty in memory"
                    );
                    secret_storage::set_secret_value(&mut self.settings, name, val);
                }
            }
        }
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

    /// Get the data directory (parent of settings.json).
    /// Used by the license cache to resolve paths at runtime rather than
    /// relying on compile-time CARGO_MANIFEST_DIR.
    pub fn data_dir(&self) -> &std::path::Path {
        self.settings_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
    }

    /// Update LLM provider settings.
    ///
    /// Keys are persisted by `save()` (keychain first, file fallback).
    pub fn set_llm_provider(&mut self, mut provider: LLMProvider) -> Result<()> {
        // Trim keys before storage: a trailing newline/space from a paste is
        // stored verbatim and later rejected by the provider as an invalid
        // key, which looks like a "saved but broken" key to the user.
        provider.api_key = provider.api_key.trim().to_string();
        provider.openai_api_key = provider.openai_api_key.trim().to_string();
        // BYOK = informed consent, recorded HERE at configuration time. The BYOK
        // setup UI shows the disclosure of what gets sent to the provider, so
        // saving a cloud provider with a key records that acceptance. We never
        // flip this flag silently at call time (see llm.rs) — recording consent at
        // the moment data is sent would defeat its purpose.
        let is_cloud = !matches!(provider.provider.as_str(), "ollama" | "none" | "local");
        if is_cloud && !provider.api_key.is_empty() {
            self.settings.privacy.cloud_llm_disclosure_accepted = true;
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
            && crate::llm_gate::compute_has_llm(
                &self.settings.llm.provider,
                &self.settings.llm.api_key,
            )
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

    /// Get X API Bearer Token
    pub fn get_x_api_key(&self) -> String {
        self.settings.x_api_key.as_str().to_string()
    }

    /// Set X API Bearer Token
    pub fn set_x_api_key(&mut self, key: String) -> Result<()> {
        self.settings.x_api_key = SensitiveString::new(key);
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

    /// Get disabled default RSS feeds
    pub fn get_disabled_default_rss_feeds(&self) -> Vec<String> {
        self.settings.disabled_default_rss_feeds.clone()
    }

    /// Set disabled default RSS feeds
    pub fn set_disabled_default_rss_feeds(&mut self, feeds: Vec<String>) -> Result<()> {
        self.settings.disabled_default_rss_feeds = feeds;
        self.save()
    }

    /// Get project paths excluded from the user's stack grounding
    pub fn get_excluded_project_paths(&self) -> Vec<String> {
        self.settings.excluded_project_paths.clone()
    }

    /// Set project paths excluded from the user's stack grounding
    pub fn set_excluded_project_paths(&mut self, paths: Vec<String>) -> Result<()> {
        self.settings.excluded_project_paths = paths;
        self.save()
    }

    /// Get disabled default YouTube channels
    pub fn get_disabled_default_youtube_channels(&self) -> Vec<String> {
        self.settings.disabled_default_youtube_channels.clone()
    }

    /// Set disabled default YouTube channels
    pub fn set_disabled_default_youtube_channels(&mut self, channels: Vec<String>) -> Result<()> {
        self.settings.disabled_default_youtube_channels = channels;
        self.save()
    }

    /// Get disabled default Twitter handles
    pub fn get_disabled_default_twitter_handles(&self) -> Vec<String> {
        self.settings.disabled_default_twitter_handles.clone()
    }

    /// Set disabled default Twitter handles
    pub fn set_disabled_default_twitter_handles(&mut self, handles: Vec<String>) -> Result<()> {
        self.settings.disabled_default_twitter_handles = handles;
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
