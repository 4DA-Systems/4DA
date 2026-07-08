// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Periodic runtime license re-validation and startup validation.
//!
//! Ensures license integrity at startup and at regular intervals,
//! catching settings.json manipulation and recovering lost keys
//! from the keychain/backup/cache fallback chain.

use std::sync::atomic::Ordering;
use tracing::{info, warn};

use super::gating::{is_paid_tier, is_trial_active};
use super::keygen::{has_license_key_available, save_license_backup};
use super::{
    LicenseConfig, ACTIVATION_GRACE_PERIOD_DAYS, LAST_LICENSE_CHECK,
    LICENSE_REVALIDATION_INTERVAL_SECS, TIER_DOWNGRADED,
};

/// Check if the user activated within the grace period.
fn is_within_activation_grace(license: &LicenseConfig) -> bool {
    if let Some(ref activated) = license.activated_at {
        if let Ok(activated_date) = chrono::DateTime::parse_from_rfc3339(activated) {
            let elapsed = chrono::Utc::now().signed_duration_since(activated_date);
            if elapsed.num_days() < ACTIVATION_GRACE_PERIOD_DAYS {
                return true;
            }
        }
    }
    false
}

/// Periodically re-run license integrity checks at runtime.
///
/// If the tier claims paid access but no license key is present (checked
/// in memory, keychain, and validation cache), the tier is reset to "free".
/// Uses relaxed atomic ordering since a rare double-check is harmless.
pub(crate) fn maybe_revalidate_license() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let last = LAST_LICENSE_CHECK.load(Ordering::Relaxed);

    if now.saturating_sub(last) < LICENSE_REVALIDATION_INTERVAL_SECS {
        return;
    }

    // Mark as checked *before* doing the work to avoid redundant checks
    // from concurrent callers during the same window.
    LAST_LICENSE_CHECK.store(now, Ordering::Relaxed);

    let manager = crate::get_settings_manager();
    let mut guard = manager.lock();
    let mut license = guard.get().license.clone();

    // Dev unlock: preserve tier in debug builds with flag set.
    if cfg!(debug_assertions) && license.dev_unlock_all {
        return;
    }

    if is_paid_tier(license.tier.as_str())
        && !is_trial_active(&license)
        && !has_license_key_available(&mut license)
    {
        if is_within_activation_grace(&license) {
            warn!(
                "Runtime re-validation: tier '{}' with no license key — within grace period, preserving tier",
                license.tier
            );
        } else {
            warn!(
                "Runtime re-validation: tier '{}' with no license key (checked memory, keychain, and cache) — resetting to free",
                license.tier
            );
            guard.get_mut().license.tier = "free".to_string();
            TIER_DOWNGRADED.store(true, Ordering::Relaxed);
            if let Err(e) = guard.save() {
                warn!(
                    "Failed to persist license reset during re-validation: {}",
                    e
                );
            }
        }
    } else if !license.license_key.is_empty() && guard.get().license.license_key.is_empty() {
        // Re-hydration happened (from keychain) — persist key to BOTH in-memory
        // settings AND disk for resilience against future keychain failures.
        info!(
            target: "4da::license",
            "Re-hydrated license key during periodic check — persisting to disk"
        );
        guard.get_mut().license.license_key = license.license_key.clone();
        if let Err(e) = guard.save() {
            warn!(
                target: "4da::license",
                error = %e,
                "Failed to persist re-hydrated license key to disk during periodic check"
            );
        }
        save_license_backup(
            &license.license_key,
            &license.tier,
            license.activated_at.as_deref().unwrap_or(""),
        );
    }
}

/// Validate license integrity on startup.
/// If tier claims "signal"/"team"/"enterprise" but no valid license key exists
/// (checked in memory, keychain, and validation cache), reset tier to "free".
/// Also initializes the periodic re-validation timestamp.
///
/// Dev bypass: in debug builds with `dev_unlock_all: true`, the tier is
/// preserved without needing a license key. Release builds ignore this flag.
pub fn validate_license_on_startup() {
    let manager = crate::get_settings_manager();
    let mut guard = manager.lock();
    let mut license = guard.get().license.clone();

    // Dev unlock: skip validation entirely in debug builds with the flag set.
    // This keeps the tier set to whatever the user chose in settings.json.
    if cfg!(debug_assertions) && license.dev_unlock_all {
        info!(
            target: "4da::license",
            tier = %license.tier,
            "Dev unlock active — skipping license validation, tier preserved"
        );
        return;
    }

    // If tier is paid but no license key is set, check grace period before downgrading
    if is_paid_tier(license.tier.as_str())
        && !is_trial_active(&license)
        && !has_license_key_available(&mut license)
    {
        if is_within_activation_grace(&license) {
            warn!(
                "License tier is '{}' but no license key found — within activation grace period, preserving tier",
                license.tier
            );
        } else {
            warn!(
                "License tier is '{}' but no license key found (checked memory, keychain, and cache) — resetting to free",
                license.tier
            );
            guard.get_mut().license.tier = "free".to_string();
            TIER_DOWNGRADED.store(true, Ordering::Relaxed);
            if let Err(e) = guard.save() {
                warn!("Failed to reset license tier: {}", e);
            }
        }
    } else if !license.license_key.is_empty() && guard.get().license.license_key.is_empty() {
        // Re-hydration happened (from keychain) — persist key to BOTH in-memory
        // settings AND disk so we don't depend on keychain again next startup.
        info!(
            target: "4da::license",
            "Re-hydrated license key into in-memory settings at startup — persisting to disk"
        );
        guard.get_mut().license.license_key = license.license_key.clone();
        if let Err(e) = guard.save() {
            warn!(
                target: "4da::license",
                error = %e,
                "Failed to persist re-hydrated license key to disk"
            );
        }
        save_license_backup(
            &license.license_key,
            &license.tier,
            license.activated_at.as_deref().unwrap_or(""),
        );
    } else if !license.license_key.is_empty() {
        // Key is present and valid — ensure backup file exists
        save_license_backup(
            &license.license_key,
            &license.tier,
            license.activated_at.as_deref().unwrap_or(""),
        );
    }

    // Record the startup validation timestamp so periodic re-checks
    // start counting from now rather than epoch-0.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    LAST_LICENSE_CHECK.store(now, Ordering::Relaxed);
}

/// Normalize a license tier string, folding legacy names into the current set.
fn normalize_tier(tier: &str) -> String {
    match tier {
        "signal" | "team" | "enterprise" => tier.to_string(),
        // Legacy: "pro", "community", "cohort" all map to "signal".
        "pro" | "community" | "cohort" => "signal".to_string(),
        other => other.to_string(),
    }
}

/// Re-derive the license tier from cryptographic proof, healing a tier that was
/// lost (e.g. a settings.json schema-drift reset wiped the license block).
///
/// This is the universal backstop for the recurring "Signal dropped to Free"
/// bug. Recovery order:
///
/// 1. If `license.license_key` is a valid self-signed `4DA-` key, trust the tier
///    embedded in its verified payload — the same proof `activate_license`
///    requires. If the key is present and the tier already matches, no-op.
/// 2. Otherwise, if the current tier is not paid, fall back to the durable
///    `license_backup.json`: restore key + tier + `activated_at` from it when it
///    holds a verifiable `4DA-` key (or a trusted Keygen-format key that was
///    validated at activation time).
///
/// Only ever *raises* to a paid tier when a valid key proves it — it never
/// grants paid access from a bare tier string, so a hand-edited `settings.json`
/// still can't unlock Signal (that path stays governed by the downgrade check in
/// [`validate_license_on_startup`]). Returns `true` if it changed `license`.
///
/// `data_dir` is the directory being loaded — the backup is read from there, not
/// from the global `get_db_path()`, so this stays hermetic under test (a temp
/// `data_dir` never sees the dev machine's real `license_backup.json`) and never
/// depends on global path-init ordering.
pub fn reconcile_license_from_proof(
    license: &mut LicenseConfig,
    data_dir: &std::path::Path,
) -> bool {
    use super::gating::is_paid_tier;
    use super::keygen::load_license_backup_from;
    use super::verify::verify_license_key;

    // 1. In-memory / settings key is the primary proof.
    if license.license_key.starts_with("4DA-") {
        if let Ok(payload) = verify_license_key(&license.license_key) {
            let effective = normalize_tier(&payload.tier);
            if is_paid_tier(&effective) && license.tier != effective {
                license.tier = effective;
                if license.activated_at.is_none() {
                    license.activated_at = Some(payload.issued_at);
                }
                license.trial_started_at = None;
                return true;
            }
            // Key present and tier already correct — nothing to heal.
            return false;
        }
        // Key present but does not verify: leave the downgrade path in
        // validate_license_on_startup to handle a genuinely invalid key.
    }

    // 2. No usable in-memory key and the tier is not paid: consult the backup.
    if !is_paid_tier(&license.tier) {
        if let Some(backup) = load_license_backup_from(data_dir) {
            if backup.license_key.is_empty() {
                return false;
            }
            let restore_tier = if backup.license_key.starts_with("4DA-") {
                // Self-signed: must verify before we trust its tier.
                verify_license_key(&backup.license_key)
                    .ok()
                    .map(|p| normalize_tier(&p.tier))
            } else {
                // Keygen-format backup: validated at activation, trusted here
                // (matches has_license_key_available's treatment of Keygen keys).
                Some(normalize_tier(&backup.tier))
            };
            if let Some(tier) = restore_tier {
                if is_paid_tier(&tier) {
                    license.license_key = backup.license_key;
                    license.tier = tier;
                    license.activated_at = Some(backup.activated_at);
                    license.trial_started_at = None;
                    return true;
                }
            }
        }
    }

    false
}

/// Check and clear the tier-downgraded flag. Returns true once per downgrade event.
/// Called by `get_license_tier` to include a one-shot notification in the response.
pub fn take_downgrade_flag() -> bool {
    TIER_DOWNGRADED.swap(false, Ordering::Relaxed)
}

/// Get the timestamp of the last successful online license validation.
/// Returns None if no cache exists or the cache is unreadable.
pub fn get_last_validated_at() -> Option<String> {
    super::keygen::load_validation_cache().map(|c| c.validated_at)
}

#[cfg(test)]
mod reconcile_tests {
    use super::*;

    fn lic(tier: &str, key: &str) -> LicenseConfig {
        LicenseConfig {
            tier: tier.to_string(),
            license_key: key.to_string(),
            activated_at: None,
            trial_started_at: None,
            dev_unlock_all: false,
        }
    }

    #[test]
    fn normalize_folds_legacy_names_to_signal() {
        assert_eq!(normalize_tier("pro"), "signal");
        assert_eq!(normalize_tier("community"), "signal");
        assert_eq!(normalize_tier("cohort"), "signal");
        assert_eq!(normalize_tier("signal"), "signal");
        assert_eq!(normalize_tier("team"), "team");
        assert_eq!(normalize_tier("free"), "free");
    }

    /// A data dir guaranteed to contain no license_backup.json — keeps the
    /// no-backup paths hermetic regardless of the machine's real license files.
    fn empty_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("4da_reconcile_{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reconcile_is_noop_when_already_paid_with_keygen_key() {
        // Keygen-format key + already-paid tier: step 1 skipped (not a 4DA- key),
        // step 2 skipped (tier is paid), no backup read. No change.
        let dir = empty_dir("noop");
        let mut l = lic("signal", "BE3529-KEYGEN");
        assert!(!reconcile_license_from_proof(&mut l, &dir));
        assert_eq!(l.tier, "signal");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reconcile_does_not_touch_paid_tier_with_unverifiable_4da_key() {
        // An invalid-signature 4DA- key must not let this function downgrade a
        // paid tier — that stays the job of validate_license_on_startup. verify
        // fails, and step 2 is skipped because the tier is already paid.
        let dir = empty_dir("invalid4da");
        let mut l = lic("signal", "4DA-invalidpayload.invalidsig");
        assert!(!reconcile_license_from_proof(&mut l, &dir));
        assert_eq!(l.tier, "signal");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reconcile_stays_free_when_no_key_and_no_backup() {
        // The common free user: no key, no backup file. Must NOT be flipped to a
        // paid tier. (This is the case that a global-path backup read would have
        // broken by picking up the dev machine's real signal backup.)
        let dir = empty_dir("free");
        let mut l = lic("free", "");
        assert!(!reconcile_license_from_proof(&mut l, &dir));
        assert_eq!(l.tier, "free");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reconcile_restores_paid_tier_from_keygen_backup_when_settings_lost_it() {
        // Settings block lost (tier free, no key) but a durable Keygen-format
        // backup survives: restore key + tier from it. Fully hermetic — the
        // backup is written into and read from the same temp data_dir.
        let dir = empty_dir("restore_keygen");
        super::super::keygen::save_license_backup_to(
            &dir,
            "BE3529-741BAF-DEADBEEF",
            "signal",
            "2026-04-22T00:00:00Z",
        );
        let mut l = lic("free", "");
        assert!(reconcile_license_from_proof(&mut l, &dir));
        assert_eq!(l.tier, "signal");
        assert_eq!(l.license_key, "BE3529-741BAF-DEADBEEF");
        assert_eq!(l.activated_at.as_deref(), Some("2026-04-22T00:00:00Z"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reconcile_ignores_free_tier_backup() {
        // A backup that itself records a free tier must never grant paid access.
        let dir = empty_dir("free_backup");
        super::super::keygen::save_license_backup_to(&dir, "SOME-KEY", "free", "");
        let mut l = lic("free", "");
        assert!(!reconcile_license_from_proof(&mut l, &dir));
        assert_eq!(l.tier, "free");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
