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
