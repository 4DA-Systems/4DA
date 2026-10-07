// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! End-to-end tests for keychain-authoritative secret storage (audit 2026-10-07).
//!
//! Every test runs against an isolated child of the TEST credential-store
//! service (`com.4da.app.test.<tag>.<uuid>`) via `IsolatedService` — never the
//! production `com.4da.app` service. Tests that need a working credential store
//! skip (with a note) on hosts that have none, e.g. headless Linux CI.

use super::keystore::{self, test_support};
use super::secret_storage::{self, SecretPosture};
use super::{Settings, SettingsManager};

const TEST_KEY: &str = "test-llm-key-AUDIT1007-scrub-me-not-a-real-key";
const TEST_LICENSE: &str = "TEST-LICENSE-AUDIT1007-not-a-real-licence";

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("4da_secret_{tag}_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// Ollama provider: no hydration retry sleeps, but the key fields still exist.
fn write_settings(dir: &std::path::Path, api_key: &str, license_key: &str) {
    let mut s = Settings::default();
    s.llm.provider = "ollama".into();
    s.llm.api_key = api_key.into();
    s.license.license_key = license_key.into();
    let json = serde_json::to_string_pretty(&s).expect("serialize");
    std::fs::write(dir.join("settings.json"), json).expect("write settings");
}

fn read(dir: &std::path::Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap_or_default()
}

/// Does this host have a credential store that verifiably holds a value?
fn credential_store_usable() -> bool {
    let ok = matches!(
        keystore::store_secret("4da_probe_usable", "probe"),
        Ok(true)
    );
    let _ = keystore::delete_secret("4da_probe_usable");
    if !ok {
        eprintln!("skipping: no usable credential store on this host");
    }
    ok
}

#[test]
fn startup_migration_scrubs_only_after_verified_round_trip() {
    let iso = test_support::IsolatedService::new("migrate");
    if !credential_store_usable() {
        return;
    }
    let dir = temp_dir("migrate");
    write_settings(&dir, TEST_KEY, TEST_LICENSE);

    let manager = SettingsManager::new(&dir);

    assert_eq!(iso.read("llm_api_key").as_deref(), Some(TEST_KEY));
    assert_eq!(iso.read("license_key").as_deref(), Some(TEST_LICENSE));
    let disk = read(&dir, "settings.json");
    let bak = read(&dir, "settings.json.bak");
    assert!(
        !disk.contains(TEST_KEY) && !disk.contains(TEST_LICENSE),
        "settings.json still holds a secret"
    );
    assert!(
        !bak.is_empty(),
        ".bak must be rewritten from the scrubbed JSON"
    );
    assert!(
        !bak.contains(TEST_KEY) && !bak.contains(TEST_LICENSE),
        ".bak still holds a secret"
    );
    // Memory keeps working values; posture is truthful.
    assert_eq!(manager.get().llm.api_key, TEST_KEY);
    let status = manager.secret_storage_status();
    assert_eq!(status.mode, "keychain");
    assert_eq!(
        status.secrets.get("llm_api_key"),
        Some(&SecretPosture::Keychain)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_keychain_write_leaves_plaintext_and_reports_file_fallback() {
    let _iso = test_support::IsolatedService::new("failwrite");
    let dir = temp_dir("failwrite");
    write_settings(&dir, TEST_KEY, "");
    test_support::set_writes_fail(true);

    let mut manager = SettingsManager::new(&dir);
    assert!(
        read(&dir, "settings.json").contains(TEST_KEY),
        "nothing may be scrubbed"
    );
    assert_eq!(manager.secret_storage_status().mode, "file_fallback");

    manager.save().expect("save");
    assert!(
        read(&dir, "settings.json").contains(TEST_KEY),
        "save must keep the fallback copy"
    );
    assert_eq!(
        manager.secret_storage_status().secrets.get("llm_api_key"),
        Some(&SecretPosture::FileFallback)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn all_or_nothing_migration_scrubs_nothing_when_one_secret_fails() {
    let _iso = test_support::IsolatedService::new("partial");
    let dir = temp_dir("partial");
    write_settings(&dir, TEST_KEY, TEST_LICENSE);
    let mut s = Settings::default();
    s.llm.api_key = TEST_KEY.into();
    s.license.license_key = TEST_LICENSE.into();
    test_support::set_writes_fail(true);
    assert!(secret_storage::migrate_plaintext_all_or_nothing(&s).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_hydration_then_save_never_deletes_or_overwrites_the_keychain() {
    let iso = test_support::IsolatedService::new("hydrate");
    if !credential_store_usable() {
        return;
    }
    assert!(matches!(
        keystore::store_secret("llm_api_key", TEST_KEY),
        Ok(true)
    ));
    let dir = temp_dir("hydrate");
    write_settings(&dir, "", ""); // already-scrubbed file

    test_support::set_reads_fail(true);
    let mut manager = SettingsManager::new(&dir);
    assert!(
        manager.get().llm.api_key.is_empty(),
        "precondition: hydration failed"
    );
    manager.save().expect("save with an empty in-memory key");
    test_support::set_reads_fail(false);

    assert_eq!(
        iso.read("llm_api_key").as_deref(),
        Some(TEST_KEY),
        "an empty value or a failed read must never delete or overwrite the stored key"
    );
    // The next consumer recovers it.
    manager.ensure_keys_hydrated();
    assert_eq!(manager.get().llm.api_key, TEST_KEY);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn backup_never_contains_a_keychain_held_secret_across_saves() {
    let _iso = test_support::IsolatedService::new("bak");
    if !credential_store_usable() {
        return;
    }
    let dir = temp_dir("bak");
    write_settings(&dir, TEST_KEY, "");
    let mut manager = SettingsManager::new(&dir);
    for _ in 0..3 {
        manager.save().expect("save");
        assert!(!read(&dir, "settings.json.bak").contains(TEST_KEY));
        assert!(!read(&dir, "settings.json").contains(TEST_KEY));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn corrupt_settings_recovers_from_backup_with_keys_from_the_keychain() {
    let _iso = test_support::IsolatedService::new("corrupt");
    if !credential_store_usable() {
        return;
    }
    let dir = temp_dir("corrupt");
    write_settings(&dir, TEST_KEY, "");
    drop(SettingsManager::new(&dir)); // migrate: keychain + scrubbed .bak
    std::fs::write(dir.join("settings.json"), "{{{ not json").expect("corrupt");

    let manager = SettingsManager::new(&dir);
    assert_eq!(manager.get().llm.provider, "ollama", "restored from .bak");
    assert_eq!(
        manager.get().llm.api_key,
        TEST_KEY,
        "key resolved from the keychain"
    );
    assert!(!read(&dir, "settings.json").contains(TEST_KEY));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Files are hardened before they are renamed into place: no inherited ACEs,
/// and only the owner SID is granted.
#[cfg(windows)]
#[test]
fn written_settings_files_have_owner_only_acl() {
    let dir = temp_dir("acl");
    let path = dir.join("settings.json");
    secret_storage::write_settings_files(&path, "{}").expect("write");
    for p in [path.clone(), dir.join("settings.json.bak")] {
        let out = std::process::Command::new("icacls")
            .arg(&p)
            .output()
            .expect("icacls");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            !text.contains("(I)"),
            "inherited ACE on {}: {text}",
            p.display()
        );
        assert!(
            !text.contains("Everyone") && !text.contains("Users:"),
            "{text}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
