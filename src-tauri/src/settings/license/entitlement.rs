// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Signal entitlement for out-of-process readers (the `4da` CLI).
//!
//! The app decides Signal in [`super::is_signal`] against its in-memory
//! settings, after startup has (1) hydrated the licence key from the keychain,
//! (2) re-derived the tier from a signed key ([`super::reconcile_license_from_proof`]),
//! (3) merged the trial stamp across settings.json, the keychain and
//! license_clock.json, and (4) downgraded a paid tier with no usable key.
//! A reader that only looked at `settings.json`'s `tier` would trust a field
//! anyone can type. This module replays those same checks against the data
//! directory WITHOUT writing settings.json (the app may be running and owns
//! that file); the one write it can cause is the same idempotent
//! earliest-wins grace anchor the app records on its next check.

use std::path::Path;

use super::gating::{is_paid_tier, is_trial_active};
use super::keygen::has_license_key_available;
use super::revalidation::is_within_activation_grace;
use super::LicenseConfig;

/// Does the install whose data lives in `data_dir` hold Signal (paid tier with
/// a usable key, an activation grace window, or an active trial)?
///
/// Paths the licence helpers resolve on their own (validation cache, backup,
/// clock floor) follow the process-wide data dir (`FOURDA_DB_PATH`), so a
/// caller must point that at the same database it reads.
pub fn signal_entitlement_read_only(data_dir: &Path) -> bool {
    evaluate(
        read_license_block(data_dir),
        data_dir,
        true,
        has_license_key_available,
    )
}

/// The `license` block of `data_dir/settings.json`; the free default when the
/// file or block is missing or unreadable (the app reads that as free too).
fn read_license_block(data_dir: &Path) -> LicenseConfig {
    std::fs::read_to_string(data_dir.join("settings.json"))
        .ok()
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        .and_then(|v| v.get("license").cloned())
        .and_then(|l| serde_json::from_value::<LicenseConfig>(l).ok())
        .unwrap_or_default()
}

/// The decision itself, on an owned copy. `use_keychain` is false and
/// `key_available` is a stub only in tests: the real key check reads the
/// keychain and the process-wide licence files, which tests running in
/// parallel share (keygen.rs tests its four layers on their own).
fn evaluate(
    mut license: LicenseConfig,
    data_dir: &Path,
    use_keychain: bool,
    key_available: impl FnOnce(&mut LicenseConfig) -> bool,
) -> bool {
    // Same debug-only escape hatch as `is_signal`; release builds ignore it.
    if cfg!(debug_assertions) && license.dev_unlock_all {
        return true;
    }
    // The keychain is authoritative for the key: settings.json may hold none.
    if use_keychain && license.license_key.is_empty() {
        if let Ok(Some(key)) = super::super::keystore::get_secret("license_key") {
            license.license_key = key;
        }
    }
    // A signed key (here or in license_backup.json) proves the tier.
    super::reconcile_license_from_proof(&mut license, data_dir);
    // The trial is judged from the merged machine-scope stamp, exactly as the
    // app's startup reconciles it — never from settings.json alone.
    if license.license_key.is_empty() && license.tier == "free" {
        license.trial_started_at = super::trial::peek_trial_stamp(
            data_dir,
            license.trial_started_at.as_deref(),
            use_keychain,
        );
    }
    if is_trial_active(&license) {
        return true;
    }
    // A paid tier counts only with a usable key, or inside the anchored
    // activation grace — the condition under which the app keeps it.
    is_paid_tier(&license.tier)
        && (key_available(&mut license) || is_within_activation_grace(&license))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("4da_entitle_{tag}_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// The production key check minus the stores tests share: only the key in
    /// hand, verified the way `key_is_usable` verifies a self-signed key.
    fn key_in_hand(l: &mut LicenseConfig) -> bool {
        l.license_key.starts_with("4DA-")
            && super::super::verify_license_key(&l.license_key).is_ok()
    }

    fn eval(l: LicenseConfig, dir: &Path) -> bool {
        evaluate(l, dir, false, key_in_hand)
    }

    #[test]
    fn a_paid_tier_with_a_usable_key_is_signal() {
        let dir = temp_dir("paid");
        assert!(evaluate(license("signal", "k", None), &dir, false, |_| {
            true
        }));
    }

    fn license(tier: &str, key: &str, trial: Option<String>) -> LicenseConfig {
        LicenseConfig {
            tier: tier.to_string(),
            license_key: key.to_string(),
            activated_at: None,
            trial_started_at: trial,
            dev_unlock_all: false,
        }
    }

    #[test]
    fn free_tier_without_trial_is_not_signal() {
        let dir = temp_dir("free");
        assert!(!eval(license("free", "", None), &dir));
    }

    #[test]
    fn active_trial_is_signal_and_expired_trial_is_not() {
        let dir = temp_dir("trial");
        let fresh = Some(chrono::Utc::now().to_rfc3339());
        assert!(eval(license("free", "", fresh), &dir));
        let old = Some((chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339());
        assert!(!eval(license("free", "", old), &dir));
    }

    #[test]
    fn a_hand_typed_paid_tier_is_not_signal() {
        // The tamper the CLI must not trust: tier "signal" with no key, an
        // unsigned key string, or a pasted 4DA- string that does not verify.
        let dir = temp_dir("tamper");
        for key in ["", "x", "4DA-not-a-real-signed-key"] {
            assert!(
                !eval(license("signal", key, None), &dir),
                "tier signal with key {key:?} must not be entitled"
            );
        }
    }

    #[test]
    fn dev_unlock_is_debug_only() {
        let dir = temp_dir("dev");
        let mut l = license("free", "", None);
        l.dev_unlock_all = true;
        assert_eq!(eval(l, &dir), cfg!(debug_assertions));
    }

    #[test]
    fn license_block_is_read_from_settings_json() {
        let dir = temp_dir("read");
        assert_eq!(
            read_license_block(&dir).tier,
            "free",
            "missing file reads as free"
        );
        std::fs::write(
            dir.join("settings.json"),
            r#"{"license":{"tier":"signal","trial_started_at":"2026-01-01T00:00:00Z"},"other":1}"#,
        )
        .expect("write settings");
        let l = read_license_block(&dir);
        assert_eq!(l.tier, "signal");
        assert_eq!(l.trial_started_at.as_deref(), Some("2026-01-01T00:00:00Z"));
        std::fs::write(dir.join("settings.json"), "{ not json").expect("write settings");
        assert_eq!(read_license_block(&dir).tier, "free");
    }
}
