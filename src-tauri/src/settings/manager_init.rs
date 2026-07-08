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
        Self::new_inner(data_dir, true, true)
    }

    /// Test-only constructor that skips keychain hydration so tests
    /// are not polluted by real keys stored on the dev machine.
    #[cfg(test)]
    pub fn new_without_keychain(data_dir: &std::path::Path) -> Self {
        Self::new_inner(data_dir, false, false)
    }

    /// Test-only constructor for exercising the reverse-trial auto-start in a
    /// hermetic license state: it does NOT hydrate the platform keychain (so a
    /// real license_key on the dev/CI machine can't leak in and suppress the
    /// trial), yet still considers the trial. Without this, the real `new()`
    /// loads the operator's ~285-char license key on the self-hosted CI runner,
    /// `license_key.is_empty()` is false, the trial is (correctly) skipped, and
    /// the test wrongly fails. Production behavior is unchanged — only the
    /// keychain-vs-trial coupling is decoupled for testability.
    #[cfg(test)]
    pub fn new_for_reverse_trial_test(data_dir: &std::path::Path) -> Self {
        Self::new_inner(data_dir, false, true)
    }

    fn new_inner(
        data_dir: &std::path::Path,
        hydrate_keychain: bool,
        consider_reverse_trial: bool,
    ) -> Self {
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

        // When a corrupt settings.json is recovered (from backup or
        // defaults), the recovered value lives only in memory until the
        // next save. The startup health check re-reads the raw file and
        // caches its verdict for the process lifetime, so without healing
        // the disk it reports "settings.json is invalid JSON" forever
        // while the app runs fine — a pure blame-magnet. Heal the file on
        // disk immediately so disk matches memory and health reads clean.
        //
        // Resilient load: a single missing / renamed / retyped field in a large
        // config must NEVER discard the whole file. That all-or-nothing parse is
        // exactly how paid users silently lost their license tier every time a
        // new build added a settings field their settings.json didn't have — the
        // parse failed, the config reset to defaults (tier "free"), and the good
        // file was healed over. Every config struct now carries #[serde(default)]
        // so additive schema changes parse cleanly; parse_settings_preserving is
        // the field-by-field backstop for the rarer removed/retyped field.
        let mut healed_from_corruption = false;
        let mut settings = if settings_path.exists() {
            match fs::read_to_string(&settings_path) {
                Ok(content) => match serde_json::from_str::<Settings>(&content) {
                    Ok(s) => s,
                    Err(strict_err) => {
                        healed_from_corruption = true;
                        // Preserve the exact bytes we could not parse so nothing
                        // is ever destroyed and the failure stays diagnosable.
                        snapshot_unparseable_settings(&settings_path, &content);
                        // Recover field-by-field: keep every value that still
                        // deserializes (critically, the `license` block) and
                        // reset only the offending field(s) to their default.
                        match parse_settings_preserving(&content) {
                            Some(recovered) => {
                                warn!(target: "4da::settings", error = %strict_err, "settings.json did not match current schema — recovered field-by-field (license and every valid field preserved)");
                                recovered
                            }
                            None => {
                                // Not even valid JSON — fall back to the .bak,
                                // run through the same preserving recovery.
                                let bak_path = settings_path.with_extension("json.bak");
                                match fs::read_to_string(&bak_path)
                                    .ok()
                                    .and_then(|c| parse_settings_preserving(&c))
                                {
                                    Some(restored) => {
                                        warn!(target: "4da::settings", "settings.json unparseable — restored from backup");
                                        restored
                                    }
                                    None => {
                                        warn!(target: "4da::settings", "settings.json unparseable and no valid backup — using defaults");
                                        Settings::default()
                                    }
                                }
                            }
                        }
                    }
                },
                Err(e) => {
                    warn!(target: "4da::settings", error = %e, "Failed to read settings.json — using defaults");
                    Settings::default()
                }
            }
        } else {
            info!(target: "4da::settings", "No settings file found, using defaults");
            Settings::default()
        };

        // Heal a corrupt-then-recovered settings.json on disk (atomic
        // write via tmp + rename) so the recovery is durable and the
        // health check stops reporting already-fixed corruption.
        if healed_from_corruption {
            if let Ok(json) = serde_json::to_string_pretty(&settings) {
                let tmp_path = settings_path.with_extension("json.heal-tmp");
                if fs::write(&tmp_path, &json).is_ok()
                    && fs::rename(&tmp_path, &settings_path).is_ok()
                {
                    info!(target: "4da::settings", "Healed corrupt settings.json on disk from recovery");
                } else {
                    let _ = fs::remove_file(&tmp_path);
                    warn!(target: "4da::settings", "Could not heal settings.json on disk (will retry on next save)");
                }
            }
        }

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

        // --- License self-heal: re-derive tier from the signed key ---
        // Universal backstop for the recurring "Signal dropped to Free" bug.
        // If a cryptographically valid license key survives ANYWHERE — in the
        // settings we just loaded, in the keychain we just hydrated, or in the
        // durable license_backup.json — restore the paid tier it proves, even if
        // the tier field itself was lost (e.g. a settings.json schema-drift reset
        // wiped the license block). This closes the long-standing asymmetry where
        // startup validation could only DOWNGRADE a paid tier with no key, but
        // nothing re-DERIVED the correct tier from a key that was still present.
        // Security: only a VERIFIED key grants a paid tier — a hand-edited tier
        // string still cannot (validate_license_on_startup downgrades that).
        if crate::settings::reconcile_license_from_proof(&mut settings.license, data_dir) {
            info!(
                target: "4da::license",
                tier = %settings.license.tier,
                "License tier self-healed from signed key at startup"
            );
            if let Some(parent) = settings_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(&settings) {
                let tmp_path = settings_path.with_extension("json.tmp");
                if fs::write(&tmp_path, &json).is_ok() {
                    let _ = atomic_replace(&tmp_path, &settings_path);
                }
            }
            // Keep all three license stores in agreement — write the backup into
            // the same data_dir we loaded from (not the global get_db_path).
            crate::settings::save_license_backup_to(
                data_dir,
                &settings.license.license_key,
                &settings.license.tier,
                settings.license.activated_at.as_deref().unwrap_or(""),
            );
        }

        // --- Reverse trial: auto-start the 14-day Signal trial on first launch ---
        // Every brand-new install experiences the full product (Preemption, Blind
        // Spots, Signal Chains, …) for 14 days, then converts or drops to Free.
        // Fires exactly once: once `trial_started_at` is set it never re-triggers,
        // and a real license (paid tier or key) opts out. Gated on
        // `consider_reverse_trial` (true for the production `new()`, false for
        // `new_without_keychain`) — kept separate from `hydrate_keychain` so a
        // hermetic test can exercise the trial without the real keychain's
        // license leaking in. Production `new()` passes both true, so behavior
        // is identical to gating on `hydrate_keychain`.
        if consider_reverse_trial
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

/// Parse `settings.json` without ever discarding the whole config because a
/// single field is missing, newly-removed, or type-changed by a different build.
///
/// The strict parse is the fast path (done by the caller). This recovery path is
/// only reached when that fails on a file that is still valid JSON: it starts
/// from `Settings::default()` and greedily re-applies each top-level field that
/// keeps the struct deserializable, so every good field — above all the
/// `license` block — survives, and only the offending field(s) fall back to
/// their default. Returns `None` only when the content is not a JSON object at
/// all (the caller then tries the `.bak`, then defaults).
///
/// Granularity note: recovery is per top-level field. Per-field
/// `#[serde(default)]` fills *missing* sub-fields, but a wrong-TYPED sub-field
/// inside a nested object still fails that object and drops the whole sub-struct
/// to its default. The license is the one block we cannot lose that way, so it
/// gets a second line of defence: even if its block is dropped here, the caller
/// then runs [`reconcile_license_from_proof`], which restores tier + key from the
/// separate `license_backup.json` / keychain / validation cache.
fn parse_settings_preserving(content: &str) -> Option<Settings> {
    let parsed: serde_json::Value = serde_json::from_str(content).ok()?;
    let parsed_obj = parsed.as_object()?;

    // Accumulator starts as the serialized defaults (always deserializable).
    let mut acc = serde_json::to_value(Settings::default()).ok()?;
    let acc_obj = acc.as_object_mut()?;

    for (key, value) in parsed_obj {
        let mut trial = acc_obj.clone();
        trial.insert(key.clone(), value.clone());
        if serde_json::from_value::<Settings>(serde_json::Value::Object(trial.clone())).is_ok() {
            *acc_obj = trial;
        } else {
            warn!(
                target: "4da::settings",
                field = %key,
                "Dropping unparseable settings field during recovery (all other fields, incl. license, preserved)"
            );
        }
    }

    serde_json::from_value(acc).ok()
}

/// Snapshot the exact bytes of a `settings.json` we could not parse, before any
/// heal overwrites it — so a paid user's data is never silently destroyed and
/// the schema mismatch stays diagnosable after the fact. Best-effort; a single
/// `.corrupt` snapshot is kept (overwritten each time) to avoid clutter.
fn snapshot_unparseable_settings(settings_path: &std::path::Path, content: &str) {
    let snap = settings_path.with_extension("json.corrupt");
    match fs::write(&snap, content) {
        Ok(()) => warn!(
            target: "4da::settings",
            snapshot = %snap.display(),
            "Snapshotted unparseable settings.json before recovery"
        ),
        Err(e) => {
            warn!(target: "4da::settings", error = %e, "Failed to snapshot unparseable settings.json")
        }
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
mod settings_durability_tests {
    use super::*;

    fn signal_settings() -> Settings {
        let mut s = Settings::default();
        s.license.tier = "signal".to_string();
        s.license.license_key = "TESTKEY-DUMMY".to_string();
        s.license.activated_at = Some("2026-04-22T00:00:00Z".to_string());
        s
    }

    /// The exact regression behind ~20 "Signal dropped to Free" incidents: a
    /// settings.json that fails a strict schema parse (here a sibling field with
    /// the wrong type) must NEVER discard a paid license. Before the fix the
    /// whole file reset to defaults (tier "free") and healed over the good file.
    #[test]
    fn preserving_parse_keeps_license_when_sibling_field_is_bad() {
        let mut v = serde_json::to_value(signal_settings()).unwrap();
        // Poison an unrelated field's TYPE so strict parse fails.
        v.as_object_mut().unwrap().insert(
            "embedding_threshold".to_string(),
            serde_json::json!("not-a-number"),
        );
        let content = serde_json::to_string_pretty(&v).unwrap();

        // Precondition: the crafted file really does fail strict parse.
        assert!(
            serde_json::from_str::<Settings>(&content).is_err(),
            "test precondition: crafted settings must fail strict parse"
        );

        let recovered =
            parse_settings_preserving(&content).expect("recovery should succeed on valid JSON");
        assert_eq!(
            recovered.license.tier, "signal",
            "license tier must survive"
        );
        assert_eq!(recovered.license.license_key, "TESTKEY-DUMMY");
        // Only the poisoned field falls back to its default.
        assert_eq!(recovered.embedding_threshold, 0.50);
    }

    #[test]
    fn preserving_parse_returns_none_for_non_json() {
        assert!(parse_settings_preserving("this is not json {{{").is_none());
    }

    /// With container `#[serde(default)]`, a missing or unknown field — the
    /// dominant cause of the drops (a new build adds a field the file lacks) —
    /// no longer fails the strict parse at all.
    #[test]
    fn additive_or_missing_field_no_longer_breaks_strict_parse() {
        let json =
            r#"{ "license": { "tier": "signal", "license_key": "K" }, "some_future_field": 123 }"#;
        let s: Settings =
            serde_json::from_str(json).expect("unknown + missing fields must parse cleanly");
        assert_eq!(s.license.tier, "signal");
        assert_eq!(s.license.license_key, "K");
    }

    /// Full load path through `SettingsManager::new`: a schema-breaking
    /// settings.json still yields the paid tier, and the healed on-disk file
    /// parses strictly afterward.
    #[test]
    fn full_load_recovers_paid_tier_from_broken_settings() {
        let tmp = std::env::temp_dir().join("4da_test_license_durability");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let mut v = serde_json::to_value(signal_settings()).unwrap();
        v.as_object_mut()
            .unwrap()
            .insert("embedding_threshold".to_string(), serde_json::json!("nope"));
        std::fs::write(
            tmp.join("settings.json"),
            serde_json::to_string_pretty(&v).unwrap(),
        )
        .unwrap();

        let manager = SettingsManager::new_without_keychain(&tmp);
        assert_eq!(
            manager.get().license.tier,
            "signal",
            "signal must survive a schema-breaking settings.json"
        );
        assert_eq!(manager.get().license.license_key, "TESTKEY-DUMMY");

        // The healed file on disk must now strict-parse and still be signal.
        let healed: Settings =
            serde_json::from_str(&std::fs::read_to_string(tmp.join("settings.json")).unwrap())
                .expect("healed settings.json must strict-parse");
        assert_eq!(healed.license.tier, "signal");

        let _ = std::fs::remove_dir_all(&tmp);
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
