// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! SettingsManager::new() — constructor with disk loading, backup recovery,
//! locale detection, token-limit bumping, tier migration, and keychain migration.

use super::super::helpers::detect_system_locale;
use super::super::keystore;
use super::super::types::*;
use super::{atomic_replace, SettingsManager};
use std::fs;
use tracing::{info, warn};

impl SettingsManager {
    /// Create a new settings manager, loading from disk if available.
    /// Hydrates API keys from the platform keychain.
    pub fn new(data_dir: &std::path::Path) -> Self {
        Self::new_inner(data_dir, true)
    }

    /// Test-only constructor that skips keychain hydration so tests
    /// are not polluted by real keys stored on the dev machine.
    #[cfg(test)]
    pub fn new_without_keychain(data_dir: &std::path::Path) -> Self {
        Self::new_inner(data_dir, false)
    }

    fn new_inner(data_dir: &std::path::Path, hydrate_keychain: bool) -> Self {
        let settings_path = data_dir.join("settings.json");
        let usage_path = data_dir.join("usage.json");

        // Reject symlinks in data path to prevent symlink attacks
        if settings_path.exists() {
            let meta = fs::symlink_metadata(&settings_path);
            if let Ok(m) = meta {
                if m.file_type().is_symlink() {
                    warn!(
                        target: "4da::security",
                        path = %settings_path.display(),
                        "Rejected symlink in data directory — using defaults"
                    );
                    // Log to security audit trail
                    if let Ok(db) = crate::get_database() {
                        db.log_security_event(
                            "symlink_blocked",
                            &settings_path.display().to_string(),
                            "critical",
                        );
                    }
                    return Self {
                        settings: Settings::default(),
                        usage: UsageStats::default(),
                        settings_path,
                        usage_path,
                    };
                }
            }
        }

        let mut settings = if settings_path.exists() {
            let load_result = fs::read_to_string(&settings_path)
                .ok()
                .and_then(|content| serde_json::from_str::<Settings>(&content).ok());

            match load_result {
                Some(s) => s,
                None => {
                    // Primary settings corrupted or unreadable — try backup
                    let bak_path = settings_path.with_extension("json.bak");
                    let bak_result = if bak_path.exists() {
                        fs::read_to_string(&bak_path)
                            .ok()
                            .and_then(|content| serde_json::from_str::<Settings>(&content).ok())
                    } else {
                        None
                    };

                    match bak_result {
                        Some(restored) => {
                            warn!(target: "4da::settings", "settings.json corrupted — restored from backup");
                            restored
                        }
                        None => {
                            warn!(target: "4da::settings", "settings.json corrupted and no valid backup — using defaults");
                            Settings::default()
                        }
                    }
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

        // Bump token limits from old defaults to accommodate translation workload.
        // Users who explicitly set lower limits won't be affected (only exact old defaults bumped).
        if settings.llm_limits.daily_token_limit == 500_000 {
            info!(target: "4da::settings", "Bumping daily token limit 500k → 2M (translation workload)");
            settings.llm_limits.daily_token_limit = 2_000_000;
        }
        if settings.llm_limits.daily_cost_limit_cents == 200 {
            settings.llm_limits.daily_cost_limit_cents = 500;
        }
        if settings.rerank.daily_token_limit == 500_000 {
            settings.rerank.daily_token_limit = 2_000_000;
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
                    let _ = atomic_replace(&tmp_path, &settings_path);
                }
            }
        }

        // Migrate the retired built-in local LLM (see migrate_retired_llm_provider).
        if migrate_retired_llm_provider(&mut settings) {
            info!(target: "4da::settings", "Migrated retired provider 'builtin' -> 'none' (built-in LLM was removed)");
            // Persist the migration so it only logs once (atomic write)
            if let Some(parent) = settings_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(&settings) {
                let tmp_path = settings_path.with_extension("json.tmp");
                if fs::write(&tmp_path, &json).is_ok() {
                    let _ = atomic_replace(&tmp_path, &settings_path);
                }
            }
        }

        // --- Mirror keys to platform keychain (secondary store) ---
        // Keys always stay on disk (the authoritative source). The keychain
        // is a best-effort mirror for OS-level credential integration.
        let has_plaintext_keys = !settings.llm.api_key.is_empty()
            || !settings.llm.openai_api_key.is_empty()
            || !settings.x_api_key.is_empty()
            || !settings.license.license_key.is_empty()
            || !settings.translation.api_key.is_empty();

        if has_plaintext_keys {
            match keystore::migrate_from_plaintext(&settings) {
                Ok(report) => {
                    if !report.migrated.is_empty() {
                        info!(
                            target: "4da::keystore",
                            mirrored = report.migrated.len(),
                            failed = report.failed.len(),
                            "Mirrored keys to platform keychain (disk remains authoritative)"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        target: "4da::keystore",
                        error = %e,
                        "Keychain mirroring failed — keys safe on disk"
                    );
                }
            }
        }

        // --- Hydrate keys from keychain into in-memory settings ---
        // Exponential backoff: the credential store can be briefly locked during
        // dev-mode hot-reloads (old process still releasing handles). A single
        // 150ms retry was insufficient — observed failures up to ~1s after restart.
        if hydrate_keychain {
            let hydrated = Self::hydrate_from_keychain(&mut settings);
            if hydrated == 0 && !has_plaintext_keys {
                let needs_key = !matches!(
                    settings.llm.provider.as_str(),
                    "none" | "ollama" | "local" | ""
                );
                if needs_key {
                    let backoff_ms = [200, 500, 1000, 2000];
                    for (attempt, delay) in backoff_ms.iter().enumerate() {
                        std::thread::sleep(std::time::Duration::from_millis(*delay));
                        let retried = Self::hydrate_from_keychain(&mut settings);
                        if retried > 0 {
                            info!(
                                target: "4da::keystore",
                                keys_recovered = retried,
                                attempt = attempt + 2,
                                delay_ms = delay,
                                "Keychain hydration succeeded on retry"
                            );
                            break;
                        }
                        if attempt == backoff_ms.len() - 1 {
                            warn!(
                                target: "4da::keystore",
                                provider = %settings.llm.provider,
                                total_attempts = backoff_ms.len() + 1,
                                "Keychain hydration exhausted all retries — ensure_keys_hydrated() will retry on first use"
                            );
                        }
                    }
                }
            }
        }

        // --- Reverse trial: auto-start the 14-day Signal trial on first launch ---
        // Every brand-new install experiences the full product (Preemption, Blind
        // Spots, Signal Chains, …) for 14 days, then converts or drops to Free.
        // Fires exactly once: once `trial_started_at` is set it never re-triggers,
        // and a real license (paid tier or key) opts out. Gated on `hydrate_keychain`
        // so the test constructor (`new_without_keychain`) never silently grants a trial.
        if hydrate_keychain
            && settings.license.trial_started_at.is_none()
            && settings.license.license_key.is_empty()
            && settings.license.tier == "free"
        {
            let now = chrono::Utc::now().to_rfc3339();
            info!(target: "4da::license", "First launch — auto-starting 14-day Signal trial");
            settings.license.trial_started_at = Some(now);
            // Persist immediately so the trial window is stable across restarts
            // (mirrors the tier-migration persist pattern above).
            if let Some(parent) = settings_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(&settings) {
                let tmp_path = settings_path.with_extension("json.tmp");
                if fs::write(&tmp_path, &json).is_ok() {
                    let _ = atomic_replace(&tmp_path, &settings_path);
                }
            }
        }

        Self {
            settings,
            usage,
            settings_path,
            usage_path,
        }
    }

    /// Read keychain secrets into the in-memory settings struct.
    /// Returns the count of keys successfully hydrated.
    fn hydrate_from_keychain(settings: &mut super::super::types::Settings) -> u32 {
        let mut count = 0u32;
        match keystore::get_secret("llm_api_key") {
            Ok(Some(key)) if !key.is_empty() => {
                info!(target: "4da::keystore", "Hydrated llm_api_key from keychain");
                settings.llm.api_key = key;
                count += 1;
            }
            Ok(Some(_)) => {
                info!(target: "4da::keystore", "llm_api_key in keychain but empty");
            }
            Ok(None) => {
                info!(target: "4da::keystore", "llm_api_key not found in keychain");
            }
            Err(e) => {
                warn!(target: "4da::keystore", error = %e, "Failed to read llm_api_key from keychain");
            }
        }
        if let Ok(Some(key)) = keystore::get_secret("openai_api_key") {
            if !key.is_empty() {
                settings.llm.openai_api_key = key;
                count += 1;
            }
        }
        if let Ok(Some(key)) = keystore::get_secret("x_api_key") {
            if !key.is_empty() {
                settings.x_api_key = super::super::types::SensitiveString::new(key);
                count += 1;
            }
        }
        if let Ok(Some(key)) = keystore::get_secret("license_key") {
            if !key.is_empty() {
                settings.license.license_key = key;
                count += 1;
            }
        }
        if let Ok(Some(key)) = keystore::get_secret("translation_api_key") {
            if !key.is_empty() {
                settings.translation.api_key = key;
                count += 1;
            }
        }
        info!(
            target: "4da::keystore",
            keys_hydrated = count,
            provider = %settings.llm.provider,
            has_llm_key = !settings.llm.api_key.is_empty(),
            "Keychain hydration complete"
        );
        count
    }
}

/// Migrate a retired LLM provider value to a usable state.
///
/// The built-in local LLM (provider `"builtin"`) was removed; any value persisted by a
/// pre-removal build can no longer run, so it resets to `"none"` (clearing the model) —
/// the app then degrades honestly to BYOK/Ollama rather than pointing at a deleted
/// sidecar. Returns `true` when it changed the settings (the caller persists on `true`).
fn migrate_retired_llm_provider(settings: &mut Settings) -> bool {
    if settings.llm.provider == "builtin" {
        settings.llm.provider = "none".to_string();
        settings.llm.model = String::new();
        true
    } else {
        false
    }
}

#[cfg(test)]
mod retired_provider_migration_tests {
    use super::*;

    fn settings_with(provider: &str, model: &str) -> Settings {
        let mut s = Settings::default();
        s.llm.provider = provider.to_string();
        s.llm.model = model.to_string();
        s
    }

    #[test]
    fn builtin_migrates_to_none_and_clears_model() {
        let mut s = settings_with("builtin", "qwen3-14b-q4km");
        assert!(
            migrate_retired_llm_provider(&mut s),
            "a persisted 'builtin' provider must be migrated"
        );
        assert_eq!(s.llm.provider, "none");
        assert_eq!(s.llm.model, "");
    }

    #[test]
    fn live_providers_are_left_untouched() {
        for provider in ["none", "ollama", "anthropic", "openai", "openai-compatible"] {
            let mut s = settings_with(provider, "some-model");
            assert!(
                !migrate_retired_llm_provider(&mut s),
                "provider '{provider}' must not be migrated"
            );
            assert_eq!(s.llm.provider, provider);
            assert_eq!(s.llm.model, "some-model");
        }
    }
}
