// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! SettingsManager::new() — constructor with disk loading, backup recovery,
//! locale detection, token-limit bumping, tier migration, and keychain migration.

use super::super::helpers::{detect_locale_once, detect_system_locale};
use super::super::keystore;
use super::super::secret_storage::{self, PostureMap, SecretPosture};
use super::super::types::*;
use super::SettingsManager;
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
                        keychain_enabled: hydrate_keychain,
                        secret_posture: PostureMap::new(),
                        persist_blocked: false,
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
        // Set when the file exists but could not be READ: nothing below may
        // write settings.json or its .bak during this run (see the field).
        let mut persist_blocked = false;
        let mut settings = if settings_path.exists() {
            match read_settings_text(&settings_path) {
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
                                match read_settings_text(&bak_path)
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
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    info!(target: "4da::settings", "settings.json vanished before it was read, using defaults");
                    Settings::default()
                }
                Err(e) => {
                    // The user's file is there but unreadable this moment (a
                    // sharing violation, a permission error, broken UTF-16).
                    // Defaults written now would replace it AND its backup,
                    // so read the backup, write nothing this run, and say so
                    // (startup health check, every refused save).
                    persist_blocked = true;
                    let backup = read_settings_text(&settings_path.with_extension("json.bak"))
                        .ok()
                        .and_then(|c| parse_settings_preserving(&c));
                    let notice = unreadable_notice_text(&settings_path, &e, backup.is_some());
                    warn!(target: "4da::settings", "{notice}");
                    let _ = UNREADABLE_NOTICE.set((settings_path.clone(), notice));
                    backup.unwrap_or_default()
                }
            }
        } else {
            info!(target: "4da::settings", "No settings file found, using defaults");
            Settings::default()
        };

        // Heal a corrupt-then-recovered settings.json on disk (atomic
        // write via tmp + rename) so the recovery is durable and the
        // health check stops reporting already-fixed corruption.
        // Nothing is scrubbed here (empty posture): the keychain migration
        // below has not run yet, so this writes back exactly what was loaded.
        let can_persist = !persist_blocked;
        if healed_from_corruption && can_persist {
            if persist_with_posture(&settings_path, &settings, &PostureMap::new()) {
                info!(target: "4da::settings", "Healed corrupt settings.json on disk from recovery");
            } else {
                warn!(target: "4da::settings", "Could not heal settings.json on disk (will retry on next save)");
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

        // Auto-detect the system locale ONCE per install (see detect_locale_once);
        // persist the flag so detection never runs on a later launch.
        if detect_locale_once(&mut settings, detect_system_locale) && can_persist {
            persist_with_posture(&settings_path, &settings, &PostureMap::new());
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
            if can_persist {
                persist_with_posture(&settings_path, &settings, &PostureMap::new());
            }
        }

        // Migrate the retired built-in local LLM (see migrate_retired_llm_provider).
        if migrate_retired_llm_provider(&mut settings) {
            info!(target: "4da::settings", "Migrated retired provider 'builtin' -> 'none' (built-in LLM was removed)");
            // Persist the migration so it only logs once (atomic write)
            if can_persist {
                persist_with_posture(&settings_path, &settings, &PostureMap::new());
            }
        }

        // --- Keychain-authoritative secrets: one-time migration + hydration ---
        // The plaintext->keychain migration rewrites settings.json, so it waits
        // for a start that could read the file.
        let mut secret_posture = Self::establish_secret_posture(
            &mut settings,
            &settings_path,
            hydrate_keychain,
            can_persist,
        );

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
            if can_persist {
                secret_posture = Self::persist_startup(&settings_path, &settings, hydrate_keychain);
                // Keep all three license stores in agreement — write the backup into
                // the same data_dir we loaded from (not the global get_db_path).
                crate::settings::save_license_backup_to(
                    data_dir,
                    &settings.license.license_key,
                    &settings.license.tier,
                    settings.license.activated_at.as_deref().unwrap_or(""),
                );
            }
        }

        // --- Reverse trial: auto-start the 14-day Signal trial on first launch ---
        // Every brand-new install experiences the full product (Preemption, Blind
        // Spots, Signal Chains, …) for 14 days, then converts or drops to Free.
        // A real license (paid tier or key) opts out. The start stamp is
        // machine-scoped (audit 2026-10-07): it is merged to the EARLIEST value
        // across settings.json, the keychain and license_clock.json, so deleting
        // settings.json cannot restart the trial and a far-future stamp is
        // clamped to now. Gated on `consider_reverse_trial` (true for `new()`,
        // false for `new_without_keychain`); the keychain copy is consulted only
        // when `hydrate_keychain` is on, so the hermetic trial test stays local.
        if consider_reverse_trial
            && settings.license.license_key.is_empty()
            && settings.license.tier == "free"
        {
            let stamp = crate::settings::reconcile_trial_stamp(
                data_dir,
                settings.license.trial_started_at.as_deref(),
                hydrate_keychain,
            );
            if settings.license.trial_started_at.as_deref() != Some(stamp.as_str()) {
                if settings.license.trial_started_at.is_none() {
                    info!(target: "4da::license", "Trial stamp established (first launch, or restored from the machine-scope stores)");
                } else {
                    warn!(target: "4da::license", "Trial stamp in settings.json disagreed with the machine-scope stores — using the earliest");
                }
                settings.license.trial_started_at = Some(stamp);
                // Persist immediately so the trial window is stable across restarts.
                if can_persist {
                    secret_posture =
                        Self::persist_startup(&settings_path, &settings, hydrate_keychain);
                }
            }
        }

        crate::llm_egress::publish_content_level(&settings.privacy.llm_content_level);

        Self {
            settings,
            usage,
            settings_path,
            usage_path,
            keychain_enabled: hydrate_keychain,
            secret_posture,
            persist_blocked,
        }
    }

    /// Run the one-time plaintext → keychain migration, then hydrate empty
    /// secrets from the keychain, and report every present secret's posture.
    fn establish_secret_posture(
        settings: &mut Settings,
        settings_path: &std::path::Path,
        use_keychain: bool,
        can_persist: bool,
    ) -> PostureMap {
        if !use_keychain {
            return secret_storage::file_only_posture(settings);
        }
        let had_plaintext = secret_storage::SECRET_NAMES
            .iter()
            .any(|n| !secret_storage::secret_value(settings, n).is_empty());
        let mut posture = if had_plaintext && !can_persist {
            secret_storage::file_only_posture(settings)
        } else if had_plaintext {
            Self::migrate_plaintext(settings, settings_path)
        } else {
            PostureMap::new()
        };
        Self::hydrate_with_retry(settings, had_plaintext);
        // Anything present now that the migration did not classify came from
        // the keychain itself.
        for name in secret_storage::SECRET_NAMES {
            if !secret_storage::secret_value(settings, name).is_empty() {
                posture.entry(name).or_insert(SecretPosture::Keychain);
            }
        }
        posture
    }

    /// All-or-nothing: scrub settings.json (and rewrite its .bak) only if every
    /// plaintext secret verified in the keychain through a fresh handle.
    fn migrate_plaintext(settings: &Settings, settings_path: &std::path::Path) -> PostureMap {
        match secret_storage::migrate_plaintext_all_or_nothing(settings) {
            Some(posture) => {
                if persist_with_posture(settings_path, settings, &posture) {
                    secret_storage::log_migration(&posture);
                    posture
                } else {
                    warn!(target: "4da::keystore", "Secrets are in the keychain but settings.json could not be rewritten — keeping the file copy for now");
                    secret_storage::file_only_posture(settings)
                }
            }
            None => secret_storage::file_only_posture(settings),
        }
    }

    /// Hydrate with exponential backoff: the credential store can be briefly
    /// locked during dev-mode hot-reloads (old process still releasing handles).
    /// A single 150ms retry was insufficient — observed failures up to ~1s.
    fn hydrate_with_retry(settings: &mut Settings, had_plaintext: bool) {
        let hydrated = Self::hydrate_from_keychain(settings);
        let needs_key = !matches!(
            settings.llm.provider.as_str(),
            "none" | "ollama" | "local" | ""
        );
        if hydrated > 0 || had_plaintext || !needs_key {
            return;
        }
        let backoff_ms = [200, 500, 1000, 2000];
        for (attempt, delay) in backoff_ms.iter().enumerate() {
            std::thread::sleep(std::time::Duration::from_millis(*delay));
            let retried = Self::hydrate_from_keychain(settings);
            if retried > 0 {
                info!(target: "4da::keystore", keys_recovered = retried, attempt = attempt + 2, delay_ms = delay, "Keychain hydration succeeded on retry");
                return;
            }
        }
        warn!(target: "4da::keystore", provider = %settings.llm.provider, total_attempts = backoff_ms.len() + 1, "Keychain hydration exhausted all retries — ensure_keys_hydrated() will retry on first use");
    }

    /// Fill EMPTY secrets from the keychain. A value already in memory (loaded
    /// from the file) is never replaced by an older keychain copy.
    /// Returns the count of keys hydrated.
    fn hydrate_from_keychain(settings: &mut Settings) -> u32 {
        let mut count = 0u32;
        for name in secret_storage::SECRET_NAMES {
            if !secret_storage::secret_value(settings, name).is_empty() {
                continue;
            }
            if let Ok(Some(key)) = keystore::get_secret(name) {
                if !key.is_empty() {
                    secret_storage::set_secret_value(settings, name, key);
                    count += 1;
                }
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

    /// Persist during construction after the migration: mirror present secrets
    /// (when the keychain is in use), scrub the verified ones, write both files.
    fn persist_startup(
        settings_path: &std::path::Path,
        settings: &Settings,
        use_keychain: bool,
    ) -> PostureMap {
        let posture = if use_keychain {
            secret_storage::mirror_present_secrets(settings)
        } else {
            secret_storage::file_only_posture(settings)
        };
        persist_with_posture(settings_path, settings, &posture);
        posture
    }
}

/// Read a settings file as text, accepting the encodings Windows tools write.
///
/// PowerShell 5's `Out-File` / `>` write UTF-16LE with a byte-order mark and
/// its `-Encoding UTF8` writes a UTF-8 BOM; Notepad and older editors save in
/// the ANSI code page (Windows-1252). A plain `read_to_string` refused UTF-16
/// and ANSI (an error, so the app fell back to defaults) and handed serde the
/// BOM (a parse failure). All of them are the user's own settings. Only an I/O
/// failure (a sharing violation, a permission error) or broken UTF-16 is an
/// error, and the caller must not write over the file then.
pub(crate) fn read_settings_text(path: &std::path::Path) -> std::io::Result<String> {
    decode_settings_bytes(&fs::read(path)?)
}

fn decode_settings_bytes(bytes: &[u8]) -> std::io::Result<String> {
    let invalid =
        |what: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, what.to_string());
    let utf16 = |body: &[u8], little: bool| -> std::io::Result<String> {
        if body.len() % 2 != 0 {
            return Err(invalid("settings.json is UTF-16 with an odd byte count"));
        }
        let units = body.chunks_exact(2).map(|p| {
            if little {
                u16::from_le_bytes([p[0], p[1]])
            } else {
                u16::from_be_bytes([p[0], p[1]])
            }
        });
        char::decode_utf16(units)
            .collect::<Result<String, _>>()
            .map_err(|_| invalid("settings.json is not valid UTF-16"))
    };
    match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => Ok(utf8_or_windows_1252(rest)),
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        _ => Ok(utf8_or_windows_1252(bytes)),
    }
}

/// UTF-8 when the bytes are UTF-8, otherwise Windows-1252 (every byte maps
/// to a character, so this cannot fail).
fn utf8_or_windows_1252(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    warn!(target: "4da::settings", "settings.json is not UTF-8: read it as Windows-1252 (ANSI); it is saved back as UTF-8");
    // 0x80..=0x9F differ from Latin-1; the five bytes Windows leaves
    // undefined keep their C1 code point, as Windows itself decodes them.
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{81}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{8D}',
        '\u{017D}', '\u{8F}', '\u{90}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}',
        '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}',
        '\u{9D}', '\u{017E}', '\u{0178}',
    ];
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => HIGH[usize::from(b - 0x80)],
            _ => char::from(b),
        })
        .collect()
}

/// Set once per run when settings.json existed but could not be read: what
/// happened, what 4DA is running on instead, and what the user can do. Shown
/// by the startup health check and returned by every refused save.
static UNREADABLE_NOTICE: std::sync::OnceLock<(std::path::PathBuf, String)> =
    std::sync::OnceLock::new();

/// The unreadable-settings notice for this run, if `settings_path` is the
/// file that could not be read.
pub(crate) fn settings_unreadable_notice(settings_path: &std::path::Path) -> Option<&'static str> {
    UNREADABLE_NOTICE
        .get()
        .filter(|(path, _)| path == settings_path)
        .map(|(_, notice)| notice.as_str())
}

fn unreadable_notice_text(
    path: &std::path::Path,
    error: &std::io::Error,
    used_backup: bool,
) -> String {
    let bak = path.with_extension("json.bak");
    let running_on = if used_backup {
        format!("its backup copy, {}", bak.display())
    } else {
        "default settings (the backup could not be read either)".to_string()
    };
    format!(
        "4DA could not read {} when it started ({error}). It is running on {running_on}, and it \
         will not save setting changes until it can read that file, so your file is never \
         overwritten. Close any program that has the file open (an editor, a sync or backup \
         tool), or replace it with the backup copy, then restart 4DA.",
        path.display()
    )
}

/// Write `settings` (scrubbed per `posture`) to settings.json and its .bak.
/// Returns false (and logs) on failure.
fn persist_with_posture(
    settings_path: &std::path::Path,
    settings: &Settings,
    posture: &PostureMap,
) -> bool {
    let disk = secret_storage::scrubbed_for_disk(settings, posture);
    let written = serde_json::to_string_pretty(&disk)
        .map_err(std::io::Error::other)
        .and_then(|json| secret_storage::write_settings_files(settings_path, &json));
    match written {
        Ok(()) => true,
        Err(e) => {
            warn!(target: "4da::settings", error = %e, "Failed to persist settings.json");
            false
        }
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
    let written = fs::write(&snap, content);
    // Unparseable bytes may still contain secrets — owner-only like settings.json.
    if written.is_ok() {
        secret_storage::harden_owner_only(&snap);
    }
    match written {
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

    fn signal_json() -> String {
        serde_json::to_string_pretty(&signal_settings()).unwrap()
    }

    /// PowerShell 5 writes UTF-16LE with a BOM. That is the user's real file:
    /// it must load, not be replaced by defaults (audit O3).
    #[test]
    fn utf16_settings_file_loads_instead_of_resetting() {
        let tmp = tempfile::tempdir().unwrap();
        let mut bytes = vec![0xFF, 0xFE];
        for unit in signal_json().encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        std::fs::write(tmp.path().join("settings.json"), &bytes).unwrap();

        let manager = SettingsManager::new_without_keychain(tmp.path());
        assert_eq!(manager.get().license.tier, "signal");
        assert_eq!(manager.get().license.license_key, "TESTKEY-DUMMY");
    }

    #[test]
    fn utf8_bom_settings_file_loads() {
        let tmp = tempfile::tempdir().unwrap();
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(signal_json().as_bytes());
        std::fs::write(tmp.path().join("settings.json"), &bytes).unwrap();
        let manager = SettingsManager::new_without_keychain(tmp.path());
        assert_eq!(manager.get().license.tier, "signal");
    }

    /// A settings.json that cannot be read (here: a directory in its place,
    /// standing in for a sharing violation) is replaced in memory by its
    /// backup; neither file is written during that run, not by startup and not
    /// by a later save (audit O3: the defaults used to be persisted over the
    /// file AND the backup). The refusal says what happened and what to do.
    #[test]
    fn unreadable_settings_uses_backup_and_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("settings.json");
        let bak = tmp.path().join("settings.json.bak");
        std::fs::create_dir(&main).unwrap();
        std::fs::write(&bak, signal_json()).unwrap();

        let mut manager = SettingsManager::new_without_keychain(tmp.path());
        assert_eq!(
            manager.get().license.tier,
            "signal",
            "loaded from the backup"
        );
        let refusal = manager.save().expect_err("save must refuse this session");
        let notice = settings_unreadable_notice(&main).expect("the run records a notice");
        assert!(
            notice.contains("settings.json") && notice.contains("backup"),
            "{notice}"
        );
        assert!(refusal.to_string().contains("will not save"), "{refusal}");

        assert!(main.is_dir(), "settings.json untouched");
        assert_eq!(
            std::fs::read_to_string(&bak).unwrap(),
            signal_json(),
            "settings.json.bak untouched"
        );
    }

    /// A file saved in the ANSI code page (Windows-1252) is the user's own
    /// settings: it loads, accents intact, instead of blocking every save.
    #[test]
    fn windows_1252_settings_file_loads() {
        let tmp = tempfile::tempdir().unwrap();
        let mut json = signal_json().into_bytes();
        json.truncate(json.len() - 1); // drop the closing brace
        json.extend_from_slice(b", \"twitter_handles\": [\"caf\xE9\x80\"]}");
        std::fs::write(tmp.path().join("settings.json"), &json).unwrap();

        let mut manager = SettingsManager::new_without_keychain(tmp.path());
        assert_eq!(manager.get().license.tier, "signal");
        assert_eq!(
            manager.get().twitter_handles,
            vec!["caf\u{e9}\u{20ac}".to_string()]
        );
        assert!(manager.save().is_ok(), "an ANSI file never blocks saving");
    }

    #[test]
    fn decode_reads_ansi_and_rejects_broken_utf16() {
        assert_eq!(decode_settings_bytes(&[0x63, 0xE9]).unwrap(), "c\u{e9}");
        assert!(
            decode_settings_bytes(&[0xFF, 0xFE, 0x00]).is_err(),
            "odd UTF-16"
        );
        assert_eq!(decode_settings_bytes(b"{}").unwrap(), "{}");
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
