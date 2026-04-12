//! Encrypted keystore for API keys using platform-native credential storage.
//!
//! Uses the `keyring` crate to store secrets in:
//! - Windows: Credential Manager
//! - macOS: Keychain
//! - Linux: Secret Service (GNOME Keyring / KWallet)
//!
//! Falls back gracefully when no keychain is available (headless Linux, WSL, CI).

use crate::error::Result;
use tracing::{info, warn};

const SERVICE_NAME: &str = "com.4da.app";

/// Key names for secrets stored in the platform keychain.
const KEY_NAMES: &[&str] = &[
    "llm_api_key",
    "openai_api_key",
    "x_api_key",
    "license_key",
    "translation_api_key",
];

/// Report of a plaintext-to-keychain migration run.
#[derive(Debug, Clone)]
pub struct MigrationReport {
    /// Key names that were successfully migrated to the keychain.
    pub migrated: Vec<String>,
    /// Key names that failed to migrate (keychain error).
    pub failed: Vec<String>,
    /// Key names that were skipped (empty or already absent).
    pub skipped: Vec<String>,
}

/// Store a secret in the platform keychain.
///
/// If the keychain is unavailable, logs a warning and returns Ok(())
/// so the caller can continue operating with in-memory keys.
pub fn store_secret(key_name: &str, value: &str) -> Result<()> {
    match keyring::Entry::new(SERVICE_NAME, key_name) {
        Ok(entry) => {
            if let Err(e) = entry.set_password(value) {
                warn!(
                    target: "4da::keystore",
                    key = key_name,
                    error = %e,
                    "Failed to store secret in keychain — continuing with in-memory key"
                );
            }
        }
        Err(e) => {
            warn!(
                target: "4da::keystore",
                key = key_name,
                error = %e,
                "Keychain unavailable — secret not persisted"
            );
        }
    }
    Ok(())
}

/// Retrieve a secret from the platform keychain.
///
/// Returns `Ok(None)` if the key does not exist or if the keychain is unavailable.
pub fn get_secret(key_name: &str) -> Result<Option<String>> {
    match keyring::Entry::new(SERVICE_NAME, key_name) {
        Ok(entry) => match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => {
                warn!(
                    target: "4da::keystore",
                    key = key_name,
                    error = %e,
                    "Failed to retrieve secret from keychain"
                );
                Ok(None)
            }
        },
        Err(e) => {
            warn!(
                target: "4da::keystore",
                key = key_name,
                error = %e,
                "Keychain unavailable — cannot retrieve secret"
            );
            Ok(None)
        }
    }
}

/// Delete a secret from the platform keychain.
///
/// Returns Ok(()) even if the key does not exist or the keychain is unavailable.
pub fn delete_secret(key_name: &str) -> Result<()> {
    match keyring::Entry::new(SERVICE_NAME, key_name) {
        Ok(entry) => {
            if let Err(e) = entry.delete_credential() {
                // NoEntry is fine — the key was already absent
                if !matches!(e, keyring::Error::NoEntry) {
                    warn!(
                        target: "4da::keystore",
                        key = key_name,
                        error = %e,
                        "Failed to delete secret from keychain"
                    );
                }
            }
        }
        Err(e) => {
            warn!(
                target: "4da::keystore",
                key = key_name,
                error = %e,
                "Keychain unavailable — cannot delete secret"
            );
        }
    }
    Ok(())
}

/// Check whether a secret exists in the platform keychain.
///
/// Returns `false` if the keychain is unavailable.
pub fn has_secret(key_name: &str) -> bool {
    match keyring::Entry::new(SERVICE_NAME, key_name) {
        Ok(entry) => entry.get_password().is_ok(),
        Err(_) => false,
    }
}

/// Migrate API keys from plaintext settings to the platform keychain.
///
/// For each known key field in `Settings`:
/// - If the value is non-empty, store it in the keychain.
/// - Track what was migrated, what failed, and what was skipped.
///
/// The caller is responsible for clearing the plaintext fields and re-saving
/// the settings file after a successful migration.
pub fn migrate_from_plaintext(settings: &super::Settings) -> Result<MigrationReport> {
    let mut report = MigrationReport {
        migrated: Vec::new(),
        failed: Vec::new(),
        skipped: Vec::new(),
    };

    let key_values: Vec<(&str, &str)> = vec![
        ("llm_api_key", &settings.llm.api_key),
        ("openai_api_key", &settings.llm.openai_api_key),
        ("x_api_key", &settings.x_api_key),
        ("license_key", &settings.license.license_key),
        ("translation_api_key", &settings.translation.api_key),
    ];

    for (key_name, value) in key_values {
        if value.is_empty() {
            report.skipped.push(key_name.to_string());
            continue;
        }

        match keyring::Entry::new(SERVICE_NAME, key_name) {
            Ok(entry) => match entry.set_password(value) {
                Ok(()) => {
                    info!(
                        target: "4da::keystore",
                        key = key_name,
                        "Migrated secret to platform keychain"
                    );
                    report.migrated.push(key_name.to_string());
                }
                Err(e) => {
                    warn!(
                        target: "4da::keystore",
                        key = key_name,
                        error = %e,
                        "Failed to migrate secret to keychain"
                    );
                    report.failed.push(key_name.to_string());
                }
            },
            Err(e) => {
                warn!(
                    target: "4da::keystore",
                    key = key_name,
                    error = %e,
                    "Keychain unavailable — cannot migrate secret"
                );
                report.failed.push(key_name.to_string());
            }
        }
    }

    if !report.migrated.is_empty() {
        info!(
            target: "4da::keystore",
            migrated = ?report.migrated,
            failed = ?report.failed,
            skipped = ?report.skipped,
            "Keychain migration complete"
        );
    }

    Ok(report)
}

/// Return the list of known key names for iteration / diagnostics.
pub fn known_key_names() -> &'static [&'static str] {
    KEY_NAMES
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that store/retrieve/delete round-trip works or falls back gracefully.
    ///
    /// On CI or headless systems the keychain may not be available, so we accept
    /// both success and graceful fallback (no panic).
    #[test]
    fn test_store_retrieve_delete_round_trip() {
        let test_key = "4da_test_round_trip";
        let test_value = "test-secret-value-12345";

        // Store — should not panic regardless of keychain availability
        let store_result = store_secret(test_key, test_value);
        assert!(store_result.is_ok());

        // Retrieve
        let get_result = get_secret(test_key);
        assert!(get_result.is_ok());
        if let Ok(Some(retrieved)) = &get_result {
            // Keychain was available — verify the value
            assert_eq!(retrieved, test_value);
        }
        // If Ok(None), keychain was unavailable — that's acceptable

        // Delete — should not panic
        let delete_result = delete_secret(test_key);
        assert!(delete_result.is_ok());

        // After delete, get should return None
        let after_delete = get_secret(test_key);
        assert!(after_delete.is_ok());
        // On systems with keychain, this should be None now
        // On systems without keychain, it was already None
    }

    #[test]
    fn test_has_secret_nonexistent() {
        // A key that was never stored should return false
        let result = has_secret("4da_test_nonexistent_key_xyz");
        // Can be false (no keychain) or false (key not found) — both correct
        assert!(!result);
    }

    #[test]
    fn test_get_secret_nonexistent() {
        let result = get_secret("4da_test_nonexistent_key_abc");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_migrate_from_plaintext_empty_settings() {
        let settings = super::super::Settings::default();
        let report = migrate_from_plaintext(&settings);
        assert!(report.is_ok());

        let report = report.unwrap();
        // All keys should be skipped since default settings have empty values
        assert!(report.migrated.is_empty());
        assert_eq!(report.skipped.len(), 5);
        assert!(report.skipped.contains(&"llm_api_key".to_string()));
        assert!(report.skipped.contains(&"openai_api_key".to_string()));
        assert!(report.skipped.contains(&"x_api_key".to_string()));
        assert!(report.skipped.contains(&"license_key".to_string()));
        assert!(report.skipped.contains(&"translation_api_key".to_string()));
    }

    #[test]
    fn test_migrate_from_plaintext_with_keys() {
        let mut settings = super::super::Settings::default();
        settings.llm.api_key = "sk-test-anthropic".to_string();
        settings.x_api_key = "bearer-test-x".to_string();
        // openai_api_key and license_key left empty

        let report = migrate_from_plaintext(&settings);
        assert!(report.is_ok());

        let report = report.unwrap();
        // Two keys had values — they were either migrated or failed (no panic either way)
        let attempted = report.migrated.len() + report.failed.len();
        assert_eq!(attempted, 2);
        assert_eq!(report.skipped.len(), 3);
        assert!(report.skipped.contains(&"openai_api_key".to_string()));
        assert!(report.skipped.contains(&"license_key".to_string()));
        assert!(report.skipped.contains(&"translation_api_key".to_string()));
    }

    #[test]
    fn test_known_key_names() {
        let names = known_key_names();
        assert_eq!(names.len(), 5);
        assert!(names.contains(&"llm_api_key"));
        assert!(names.contains(&"openai_api_key"));
        assert!(names.contains(&"x_api_key"));
        assert!(names.contains(&"license_key"));
        assert!(names.contains(&"translation_api_key"));
    }

    #[test]
    fn test_delete_nonexistent_key_is_ok() {
        // Deleting a key that doesn't exist should not error
        let result = delete_secret("4da_test_delete_nonexistent_zzz");
        assert!(result.is_ok());
    }
}
