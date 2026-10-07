// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Platform-native credential storage for secrets.
//!
//! Uses the `keyring` crate to store secrets in:
//! - Windows: Credential Manager
//! - macOS: Keychain
//! - Linux: Secret Service (GNOME Keyring / KWallet)
//!
//! Falls back gracefully when no keychain is available (headless Linux, WSL, CI):
//! every write reports whether it was verifiably persisted, and callers keep a
//! file fallback when it was not.
//!
//! ## Scopes (audit 2026-10-07)
//!
//! Secrets live under one of two credential-store services:
//!
//! - **Machine scope** — [`MACHINE_SCOPE_KEYS`]: the receipt signing key, the
//!   licence time floor, the database encryption key, the trial stamp and the
//!   licence-cache MAC key. These describe the machine/install, not a data
//!   profile, and always use the global service `com.4da.app`.
//! - **Profile scope** — everything else (BYOK keys, licence key, translation
//!   key, webhook secrets, team keys). In the default profile they use
//!   `com.4da.app` too (unchanged, no migration). When `FOURDA_DATA_DIR`
//!   selects another profile they use `com.4da.app.p.<12 hex of
//!   sha256(canonical data dir)>` and never fall back to the global service —
//!   a throwaway profile must not read, overwrite or scrub the real profile's
//!   credentials.
//!
//! Every `#[cfg(test)]` build routes ALL secrets to the test service
//! (`com.4da.app.test`, optionally narrowed per test thread) — never to the
//! production service. Tests that overwrote real credentials are the reason
//! this exists (see `3d325a97e`).

use crate::error::Result;
use std::path::{Path, PathBuf};
use tracing::warn;

/// The global credential-store service (machine scope + default profile).
const SERVICE_NAME: &str = "com.4da.app";

#[cfg(test)]
pub(crate) const TEST_SERVICE_NAME: &str = "com.4da.app.test";

/// Keychain name of the machine-scope trial start stamp (RFC 3339).
pub(crate) const TRIAL_STAMP_KEY: &str = "license_trial_started_at";

/// Keychain name of the machine-scope licence-cache MAC key (32 bytes, hex).
pub(crate) const LICENSE_CACHE_MAC_KEY: &str = "license_cache_mac_key";

/// Secrets that belong to the machine, not to a data profile.
const MACHINE_SCOPE_KEYS: &[&str] = &[
    "engine_receipt_signing_key",
    "license_time_floor",
    "4da_db_encryption_key",
    TRIAL_STAMP_KEY,
    LICENSE_CACHE_MAC_KEY,
];

// ============================================================================
// Service resolution
// ============================================================================

/// Is this secret machine-scoped (always the global service)?
pub(crate) fn is_machine_scope(key_name: &str) -> bool {
    MACHINE_SCOPE_KEYS.contains(&key_name)
}

/// First 12 hex chars of sha256 of the canonicalised profile directory.
pub(crate) fn profile_tag(data_dir: &Path) -> String {
    use sha2::Digest;
    let canonical = std::fs::canonicalize(data_dir).unwrap_or_else(|_| data_dir.to_path_buf());
    let digest = sha2::Sha256::digest(canonical.to_string_lossy().as_bytes());
    hex::encode(digest)[..12].to_string()
}

/// Pure service-name resolution: `base` for machine-scope secrets or the
/// default profile, `base.p.<tag>` for a profile secret in a non-default profile.
pub(crate) fn service_for_with(base: &str, key_name: &str, profile_dir: Option<&Path>) -> String {
    match profile_dir {
        Some(dir) if !is_machine_scope(key_name) => format!("{base}.p.{}", profile_tag(dir)),
        _ => base.to_string(),
    }
}

/// The non-default profile directory selected by `FOURDA_DATA_DIR`, if any.
fn profile_dir_override() -> Option<PathBuf> {
    let raw = std::env::var("FOURDA_DATA_DIR").ok()?;
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

#[cfg(not(test))]
fn base_service() -> String {
    SERVICE_NAME.to_string()
}

#[cfg(test)]
fn base_service() -> String {
    test_support::current_base()
}

/// The credential-store service a secret resolves to in this process.
pub(crate) fn service_for(key_name: &str) -> String {
    service_for_with(&base_service(), key_name, profile_dir_override().as_deref())
}

/// Human-readable description of where profile secrets live (for logs/UI).
pub(crate) fn profile_service_name() -> String {
    service_for("llm_api_key")
}

// ============================================================================
// Public API (scope-aware)
// ============================================================================

/// Store a secret in the platform keychain.
///
/// Returns `Ok(true)` only when the secret was persisted AND read back through a
/// fresh entry handle with an identical value. Returns `Ok(false)` when the
/// keychain was unavailable, the write failed or the round-trip did not match —
/// the caller keeps its file fallback and must surface that posture.
pub fn store_secret(key_name: &str, value: &str) -> Result<bool> {
    #[cfg(test)]
    if test_support::writes_fail() {
        return Ok(false);
    }
    store_secret_with_service(&service_for(key_name), key_name, value)
}

/// Retrieve a secret from the platform keychain.
///
/// Returns `Ok(None)` if the key does not exist or the keychain is unavailable.
pub fn get_secret(key_name: &str) -> Result<Option<String>> {
    #[cfg(test)]
    if test_support::reads_fail() {
        return Ok(None);
    }
    get_secret_with_service(&service_for(key_name), key_name)
}

/// Delete a secret from the platform keychain.
///
/// Returns Ok(()) even if the key does not exist or the keychain is unavailable.
pub fn delete_secret(key_name: &str) -> Result<()> {
    delete_secret_with_service(&service_for(key_name), key_name)
}

/// Check whether a secret exists in the platform keychain.
///
/// Returns `false` if the keychain is unavailable.
pub fn has_secret(key_name: &str) -> bool {
    matches!(get_secret(key_name), Ok(Some(_)))
}

/// Read the secret back through a FRESH entry handle and compare.
///
/// Returns `true` only if the read-back matches exactly. This catches platforms
/// where `set_password` reports success but nothing is persisted (the keyring
/// crate without a native backend did exactly that — `b6d3a5a29`).
pub fn verify_round_trip(key_name: &str, expected: &str) -> bool {
    #[cfg(test)]
    if test_support::reads_fail() || test_support::writes_fail() {
        return false;
    }
    verify_round_trip_with_service(&service_for(key_name), key_name, expected)
}

// ============================================================================
// Service-explicit primitives
// ============================================================================

pub(crate) fn store_secret_with_service(
    service: &str,
    key_name: &str,
    value: &str,
) -> Result<bool> {
    let entry = match keyring::Entry::new(service, key_name) {
        Ok(entry) => entry,
        Err(e) => {
            warn!(target: "4da::keystore", key = key_name, error = %e, "Keychain unavailable — secret not stored in the credential store");
            return Ok(false);
        }
    };
    if let Err(e) = entry.set_password(value) {
        warn!(target: "4da::keystore", key = key_name, error = %e, "Failed to write to keychain — secret not stored in the credential store");
        return Ok(false);
    }
    if verify_round_trip_with_service(service, key_name, value) {
        Ok(true)
    } else {
        warn!(target: "4da::keystore", key = key_name, "Keychain write reported success but round-trip verification failed");
        Ok(false)
    }
}

pub(crate) fn get_secret_with_service(service: &str, key_name: &str) -> Result<Option<String>> {
    match keyring::Entry::new(service, key_name) {
        Ok(entry) => match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => {
                warn!(target: "4da::keystore", key = key_name, error = %e, "Failed to retrieve secret from keychain");
                Ok(None)
            }
        },
        Err(e) => {
            warn!(target: "4da::keystore", key = key_name, error = %e, "Keychain unavailable — cannot retrieve secret");
            Ok(None)
        }
    }
}

pub(crate) fn delete_secret_with_service(service: &str, key_name: &str) -> Result<()> {
    match keyring::Entry::new(service, key_name) {
        Ok(entry) => {
            if let Err(e) = entry.delete_credential() {
                if !matches!(e, keyring::Error::NoEntry) {
                    warn!(target: "4da::keystore", key = key_name, error = %e, "Failed to delete secret from keychain");
                }
            }
        }
        Err(e) => {
            warn!(target: "4da::keystore", key = key_name, error = %e, "Keychain unavailable — cannot delete secret");
        }
    }
    Ok(())
}

fn verify_round_trip_with_service(service: &str, key_name: &str, expected: &str) -> bool {
    if expected.is_empty() {
        return false;
    }
    match keyring::Entry::new(service, key_name) {
        Ok(entry) => matches!(entry.get_password(), Ok(ref val) if val == expected),
        Err(_) => false,
    }
}

// ============================================================================
// Test support — per-thread service isolation + fault injection
// ============================================================================

/// Test-only hooks. Each test runs on its own thread, so these thread-locals
/// isolate a test's credential-store traffic from every other test.
#[cfg(test)]
pub(crate) mod test_support {
    use std::cell::{Cell, RefCell};

    thread_local! {
        static BASE: RefCell<Option<String>> = const { RefCell::new(None) };
        static FAIL_WRITES: Cell<bool> = const { Cell::new(false) };
        static FAIL_READS: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn current_base() -> String {
        BASE.with(|b| b.borrow().clone())
            .unwrap_or_else(|| super::TEST_SERVICE_NAME.to_string())
    }

    pub(crate) fn writes_fail() -> bool {
        FAIL_WRITES.with(Cell::get)
    }

    pub(crate) fn reads_fail() -> bool {
        FAIL_READS.with(Cell::get)
    }

    /// Simulate an unavailable/failing credential store for writes.
    pub(crate) fn set_writes_fail(on: bool) {
        FAIL_WRITES.with(|c| c.set(on));
    }

    /// Simulate a failed hydration (every read returns nothing).
    pub(crate) fn set_reads_fail(on: bool) {
        FAIL_READS.with(|c| c.set(on));
    }

    /// Route this thread's secrets to a unique `com.4da.app.test.<tag>` service
    /// for the guard's lifetime. Always a child of the test service — a test
    /// can never reach the production service through this.
    pub(crate) struct IsolatedService {
        pub(crate) base: String,
    }

    impl IsolatedService {
        pub(crate) fn new(tag: &str) -> Self {
            let base = format!(
                "{}.{tag}.{}",
                super::TEST_SERVICE_NAME,
                uuid::Uuid::new_v4()
            );
            BASE.with(|b| *b.borrow_mut() = Some(base.clone()));
            Self { base }
        }

        /// Read a secret directly from this isolated service (bypasses faults).
        pub(crate) fn read(&self, key: &str) -> Option<String> {
            super::get_secret_with_service(&super::service_for(key), key)
                .ok()
                .flatten()
        }
    }

    impl Drop for IsolatedService {
        fn drop(&mut self) {
            set_writes_fail(false);
            set_reads_fail(false);
            for key in [
                "llm_api_key",
                "openai_api_key",
                "x_api_key",
                "license_key",
                "translation_api_key",
                super::TRIAL_STAMP_KEY,
                super::LICENSE_CACHE_MAC_KEY,
                "license_time_floor",
            ] {
                let _ = super::delete_secret(key);
            }
            BASE.with(|b| *b.borrow_mut() = None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::IsolatedService;
    use super::*;

    #[test]
    fn test_store_retrieve_delete_round_trip() {
        let test_key = "4da_test_round_trip";
        let test_value = "test-secret-value-12345";

        let store_result = store_secret_with_service(TEST_SERVICE_NAME, test_key, test_value);
        assert!(store_result.is_ok());

        let get_result = get_secret_with_service(TEST_SERVICE_NAME, test_key);
        assert!(get_result.is_ok());
        if let Ok(Some(retrieved)) = &get_result {
            assert_eq!(retrieved, test_value);
        }

        let delete_result = delete_secret_with_service(TEST_SERVICE_NAME, test_key);
        assert!(delete_result.is_ok());

        let after_delete = get_secret_with_service(TEST_SERVICE_NAME, test_key);
        assert!(after_delete.is_ok());
    }

    #[test]
    fn test_has_secret_nonexistent() {
        assert!(!has_secret("4da_test_nonexistent_key_xyz"));
    }

    #[test]
    fn test_get_secret_nonexistent() {
        let result = get_secret("4da_test_nonexistent_key_abc");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent_key_is_ok() {
        let result = delete_secret("4da_test_delete_nonexistent_zzz");
        assert!(result.is_ok());
    }

    #[test]
    fn test_store_secret_verifies_round_trip() {
        let key = "4da_test_store_verify";
        let val = "store-verify-value";
        let result = store_secret_with_service(TEST_SERVICE_NAME, key, val);
        assert!(result.is_ok());
        if result.unwrap() {
            let readback = get_secret_with_service(TEST_SERVICE_NAME, key);
            assert!(matches!(readback, Ok(Some(ref v)) if v == val));
            let _ = delete_secret_with_service(TEST_SERVICE_NAME, key);
        }
    }

    #[test]
    fn tests_never_resolve_to_the_production_service() {
        for key in ["llm_api_key", "license_key", "engine_receipt_signing_key"] {
            let svc = service_for(key);
            assert!(
                svc.starts_with(TEST_SERVICE_NAME),
                "{key} resolved to {svc} under cfg(test)"
            );
        }
    }

    #[test]
    fn default_profile_uses_the_global_service_unchanged() {
        assert_eq!(
            service_for_with(SERVICE_NAME, "llm_api_key", None),
            SERVICE_NAME
        );
        assert_eq!(
            service_for_with(SERVICE_NAME, "license_key", None),
            SERVICE_NAME
        );
    }

    #[test]
    fn profile_secrets_are_scoped_and_machine_secrets_stay_global() {
        let dir = std::env::temp_dir();
        let scoped = service_for_with(SERVICE_NAME, "llm_api_key", Some(&dir));
        assert!(scoped.starts_with("com.4da.app.p."), "{scoped}");
        assert_eq!(scoped.len(), "com.4da.app.p.".len() + 12);
        for key in ["license_key", "translation_api_key", "webhook_secret__abc"] {
            assert_eq!(service_for_with(SERVICE_NAME, key, Some(&dir)), scoped);
        }
        for key in MACHINE_SCOPE_KEYS {
            assert_eq!(
                service_for_with(SERVICE_NAME, key, Some(&dir)),
                SERVICE_NAME,
                "{key} must stay machine-scoped"
            );
        }
    }

    #[test]
    fn different_profiles_get_different_services() {
        let a = std::env::temp_dir().join("4da_profile_a");
        let b = std::env::temp_dir().join("4da_profile_b");
        assert_ne!(
            service_for_with(SERVICE_NAME, "llm_api_key", Some(&a)),
            service_for_with(SERVICE_NAME, "llm_api_key", Some(&b))
        );
    }

    /// A scoped profile cannot read the global profile's llm_api_key — there is
    /// no fallback to the global service for profile secrets.
    #[test]
    fn scoped_profile_cannot_read_a_global_llm_api_key() {
        let iso = IsolatedService::new("scope");
        let global = iso.base.clone();
        let stored = store_secret_with_service(&global, "llm_api_key", "global-only-test-key")
            .unwrap_or(false);
        if !stored {
            return; // no usable credential store on this host
        }
        let profile = std::env::temp_dir().join("4da_scoped_profile_probe");
        let scoped = service_for_with(&global, "llm_api_key", Some(&profile));
        assert_ne!(scoped, global);
        let got = get_secret_with_service(&scoped, "llm_api_key").unwrap_or(None);
        assert!(got.is_none(), "scoped profile must not see the global key");
        let _ = delete_secret_with_service(&global, "llm_api_key");
    }

    /// The receipt signing key resolves identically in the default and a
    /// scoped profile — machine scope.
    #[test]
    fn receipt_key_resolves_identically_in_both_scopes() {
        let profile = std::env::temp_dir().join("4da_scoped_profile_receipt");
        let base = TEST_SERVICE_NAME;
        assert_eq!(
            service_for_with(base, "engine_receipt_signing_key", None),
            service_for_with(base, "engine_receipt_signing_key", Some(&profile))
        );
    }

    #[test]
    fn fault_injection_reports_failed_writes_and_reads() {
        let _iso = IsolatedService::new("faults");
        test_support::set_writes_fail(true);
        assert!(!store_secret("llm_api_key", "x").unwrap_or(true));
        test_support::set_writes_fail(false);
        test_support::set_reads_fail(true);
        assert!(get_secret("llm_api_key").unwrap_or(None).is_none());
    }
}
