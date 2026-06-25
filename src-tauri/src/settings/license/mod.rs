// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! License verification, feature gating, and trial management.

mod gating;
mod keygen;
mod revalidation;
mod verify;

// ============================================================================
// Re-exports — preserve the original public API surface
// ============================================================================

pub use gating::{
    get_trial_status, is_signal, is_signal_feature_available, is_trial_active,
    require_signal_feature, TrialStatus, SIGNAL_FEATURES,
};
pub use keygen::{
    save_license_backup, validate_license_key_keygen, validate_license_key_keygen_fresh,
    KeygenValidationCache, KeygenValidationResult,
};
pub use revalidation::{get_last_validated_at, take_downgrade_flag, validate_license_on_startup};
pub use verify::{verify_license_key, LicensePayload};

// Rate limiting is pub from here
pub use self::rate_limit::{check_activation_rate_limit, clear_activation_rate_limit};

// ============================================================================
// Shared constants used across submodules
// ============================================================================

use super::keystore;
use super::LicenseConfig;
use std::sync::atomic::AtomicU64;

const KEYGEN_ACCOUNT_ID: &str = "runyourempirehq";

/// Base URL template for Keygen validation.
/// Full URL: `https://api.keygen.sh/v1/accounts/{ACCOUNT_ID}/licenses/actions/validate-key`
// REMOVE BY 2026-08-01
#[allow(dead_code)] // Const: Keygen API URL for license validation
const KEYGEN_VALIDATE_URL: &str = "https://api.keygen.sh/v1/licenses/actions/validate-key";

/// Hours before a cached validation result is considered stale.
/// 90 days provides resilience for offline periods and intermittent keychain failures.
const VALIDATION_CACHE_HOURS: u64 = 2160; // 90 days

/// Grace period: if `activated_at` is set, do not downgrade the tier for this
/// many days even if the license key is temporarily unavailable.
const ACTIVATION_GRACE_PERIOD_DAYS: i64 = 30;

/// Re-validate license integrity every 6 hours to prevent settings.json
/// manipulation granting paid features between restarts.
const LICENSE_REVALIDATION_INTERVAL_SECS: u64 = 21_600; // 6 hours

/// Epoch-seconds timestamp of the last license integrity check.
/// Initialized to 0 so the first call always triggers validation.
static LAST_LICENSE_CHECK: AtomicU64 = AtomicU64::new(0);

/// Flag set when a paid tier is downgraded to free during re-validation.
/// Read and cleared by `get_license_tier` to notify the frontend once.
static TIER_DOWNGRADED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// ============================================================================
// Rate Limiting (activation attempts)
// ============================================================================

mod rate_limit {
    use crate::error::Result;

    /// Maximum license activation attempts per minute
    const MAX_ACTIVATION_ATTEMPTS_PER_MINUTE: u32 = 5;

    /// Track activation attempts for rate limiting
    static ACTIVATION_ATTEMPTS: std::sync::LazyLock<parking_lot::Mutex<Vec<std::time::Instant>>> =
        std::sync::LazyLock::new(|| parking_lot::Mutex::new(Vec::new()));

    /// Check and enforce rate limiting on license activation
    pub fn check_activation_rate_limit() -> Result<()> {
        let mut attempts = ACTIVATION_ATTEMPTS.lock();
        let now = std::time::Instant::now();
        let one_minute_ago = now
            .checked_sub(std::time::Duration::from_mins(1))
            .unwrap_or(now);

        // Remove attempts older than 1 minute
        attempts.retain(|t| *t > one_minute_ago);

        if attempts.len() >= MAX_ACTIVATION_ATTEMPTS_PER_MINUTE as usize {
            return Err(
                "Too many activation attempts. Please wait a minute before trying again.".into(),
            );
        }

        attempts.push(now);
        Ok(())
    }

    /// Clear the activation rate limiter. Called after successful activation
    /// so users who typo'd several times aren't blocked from retrying.
    pub fn clear_activation_rate_limit() {
        ACTIVATION_ATTEMPTS.lock().clear();
    }
}
