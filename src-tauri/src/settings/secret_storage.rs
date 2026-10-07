// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Keychain-authoritative secret storage for `settings.json` (audit 2026-10-07).
//!
//! ## Why the keychain can be authoritative again
//!
//! `a003b9064` (2026-05-29) made settings.json the authoritative store, citing
//! "temporary keychain inaccessibility". The actual key-loss causes were
//! already fixed by then:
//! - `b6d3a5a29`: the keyring crate had no Windows/macOS backend, so
//!   `set_password` returned Ok while storing nothing. Fixed with the native
//!   backends plus a fresh-handle round-trip (`keystore::store_secret`).
//! - `3d325a97e` / `91ed48597`: tests overwrote the real `com.4da.app`
//!   credentials. Tests now use `com.4da.app.test` exclusively.
//! - Dev hot-restart read locking: retried hydration (manager_init) and
//!   `ensure_keys_hydrated` before every save.
//!
//! ## Rules this module enforces
//!
//! 1. A secret is removed from the on-disk JSON ONLY after the keychain holds
//!    it, proven by a read through a fresh handle (`store_secret == Ok(true)`).
//! 2. The keychain is never overwritten or deleted with an empty value or
//!    because a read failed — empty in memory simply writes nothing.
//! 3. When the keychain cannot hold a secret, the file keeps it (owner-only
//!    ACL) and the posture says so — the UI never claims secure storage then.
//! 4. `settings.json.bak` is built from the same scrubbed JSON (never
//!    `fs::copy` of the old file), and every file is hardened BEFORE it is
//!    renamed into place, so no window exists where a secret sits in a file
//!    with inherited permissions.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;
use tracing::{info, warn};

use super::keystore;
use super::types::{SensitiveString, Settings};

/// Names of every secret held in `Settings`, in a fixed order.
pub(crate) const SECRET_NAMES: [&str; 5] = [
    "llm_api_key",
    "openai_api_key",
    "x_api_key",
    "license_key",
    "translation_api_key",
];

/// Where a present secret is persisted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretPosture {
    /// In the OS credential store (verified); absent from settings.json.
    Keychain,
    /// The credential store could not hold it; kept in settings.json, which is
    /// restricted to the current user.
    FileFallback,
}

/// Per-secret posture for the secrets that are currently present.
pub type PostureMap = BTreeMap<&'static str, SecretPosture>;

/// Read one secret from `Settings` by name.
pub(crate) fn secret_value<'a>(s: &'a Settings, name: &str) -> &'a str {
    match name {
        "llm_api_key" => s.llm.api_key.as_str(),
        "openai_api_key" => s.llm.openai_api_key.as_str(),
        "x_api_key" => s.x_api_key.as_str(),
        "license_key" => s.license.license_key.as_str(),
        "translation_api_key" => s.translation.api_key.as_str(),
        _ => "",
    }
}

/// Write one secret into `Settings` by name.
pub(crate) fn set_secret_value(s: &mut Settings, name: &str, value: String) {
    match name {
        "llm_api_key" => s.llm.api_key = value,
        "openai_api_key" => s.llm.openai_api_key = value,
        "x_api_key" => s.x_api_key = SensitiveString::new(value),
        "license_key" => s.license.license_key = value,
        "translation_api_key" => s.translation.api_key = value,
        _ => {}
    }
}

/// Mirror every present secret to the keychain and report each one's posture.
///
/// Skips the write when the keychain already holds the identical value. Never
/// writes or deletes for an empty value (rule 2).
pub(crate) fn mirror_present_secrets(s: &Settings) -> PostureMap {
    let mut posture = PostureMap::new();
    for name in SECRET_NAMES {
        let value = secret_value(s, name);
        if value.is_empty() {
            continue;
        }
        let already = matches!(keystore::get_secret(name), Ok(Some(ref v)) if v == value)
            && keystore::verify_round_trip(name, value);
        let held = already || matches!(keystore::store_secret(name, value), Ok(true));
        let p = if held {
            SecretPosture::Keychain
        } else {
            SecretPosture::FileFallback
        };
        posture.insert(name, p);
    }
    posture
}

/// One-time startup migration, all-or-nothing.
///
/// For every secret present in plaintext: mirror it to the keychain and read it
/// back through a fresh handle. Returns `Some(posture)` (all `Keychain`) only
/// when EVERY secret verified; otherwise `None` — the caller then scrubs
/// nothing and the whole set stays in the file.
pub(crate) fn migrate_plaintext_all_or_nothing(s: &Settings) -> Option<PostureMap> {
    let posture = mirror_present_secrets(s);
    if posture.is_empty() {
        return None;
    }
    if posture.values().all(|p| *p == SecretPosture::Keychain) {
        Some(posture)
    } else {
        let failed: Vec<&str> = posture
            .iter()
            .filter(|(_, p)| **p == SecretPosture::FileFallback)
            .map(|(n, _)| *n)
            .collect();
        warn!(target: "4da::keystore", failed = ?failed, "Keychain migration incomplete — keeping every secret in settings.json (owner-only); nothing scrubbed");
        None
    }
}

/// The JSON that may be written to disk: secrets held by the keychain are
/// removed, and the team-relay JWT is never persisted.
pub(crate) fn scrubbed_for_disk(s: &Settings, posture: &PostureMap) -> Settings {
    let mut disk = s.clone();
    for (name, p) in posture {
        if *p == SecretPosture::Keychain {
            set_secret_value(&mut disk, name, String::new());
        }
    }
    if let Some(ref mut relay) = disk.team_relay {
        relay.auth_token = None;
    }
    disk
}

/// Atomically write `settings.json` and a matching `settings.json.bak` from
/// the same (already scrubbed) JSON. Each file is hardened to owner-only
/// BEFORE it is renamed into place.
pub(crate) fn write_settings_files(settings_path: &Path, json: &str) -> std::io::Result<()> {
    if serde_json::from_str::<serde_json::Value>(json).is_err() {
        return Err(std::io::Error::other(
            "settings serialization produced invalid JSON",
        ));
    }
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bak_path = settings_path.with_extension("json.bak");
    write_hardened_then_rename(&bak_path, &bak_path.with_extension("bak.tmp"), json)?;
    write_hardened_then_rename(
        settings_path,
        &settings_path.with_extension("json.tmp"),
        json,
    )
}

/// Write `content` to `tmp`, harden it, then move it over `target`.
fn write_hardened_then_rename(target: &Path, tmp: &Path, content: &str) -> std::io::Result<()> {
    let _ = fs::remove_file(tmp);
    fs::write(tmp, content)?;
    harden_owner_only(tmp);
    if let Err(e) = super::manager::atomic_replace(tmp, target) {
        let _ = fs::remove_file(tmp);
        return Err(e);
    }
    // Belt and braces: re-apply on the final path (a rename keeps the DACL on
    // the same volume, but a cross-volume move would not).
    harden_owner_only(target);
    Ok(())
}

/// Restrict a file to the current user only. Best-effort; logs on failure.
pub(crate) fn harden_owner_only(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
            warn!(target: "4da::settings", error = %e, "Could not restrict file permissions");
        }
    }
    #[cfg(windows)]
    {
        harden_windows(path);
    }
}

#[cfg(windows)]
fn harden_windows(path: &Path) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Some(sid) = owner_sid() else {
        warn!(target: "4da::settings", "Could not resolve the current user's SID — file ACL not hardened");
        return;
    };
    let path_str = path.to_string_lossy();
    let grant = format!("*{sid}:(F)");
    let out = std::process::Command::new("icacls")
        .args([
            path_str.as_ref(),
            "/inheritance:r",
            "/grant:r",
            grant.as_str(),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match out {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            warn!(target: "4da::settings", code = ?o.status.code(), "icacls did not harden the settings file")
        }
        Err(e) => {
            warn!(target: "4da::settings", error = %e, "icacls unavailable — settings file ACL not hardened")
        }
    }
}

/// SID of the user this process runs as (from the process token via
/// `whoami /user`), cached. A SID is unambiguous where `%USERNAME%` is not
/// (domain vs local accounts, renamed accounts, unset env).
#[cfg(windows)]
fn owner_sid() -> Option<&'static str> {
    use std::os::windows::process::CommandExt;
    static SID: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    SID.get_or_init(|| {
        let out = std::process::Command::new("whoami")
            .args(["/user", "/fo", "csv", "/nh"])
            .creation_flags(0x0800_0000)
            .output()
            .ok()?;
        parse_whoami_sid(&String::from_utf8_lossy(&out.stdout))
    })
    .as_deref()
}

/// Extract the SID from `whoami /user /fo csv /nh` output.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn parse_whoami_sid(output: &str) -> Option<String> {
    output
        .split(',')
        .map(|f| f.trim().trim_matches('"'))
        .find(|f| f.starts_with("S-1-") && f.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .map(str::to_string)
}

/// Frontend-facing summary of where secrets are stored.
#[derive(Debug, Clone, Serialize)]
pub struct SecretStorageStatus {
    /// `keychain` (every present secret is in the credential store),
    /// `file_fallback` (at least one is kept in settings.json), or `none`.
    pub mode: &'static str,
    /// Per-secret posture for the secrets currently present.
    pub secrets: BTreeMap<&'static str, SecretPosture>,
}

/// Summarise a posture map for the UI.
pub(crate) fn status_of(posture: &PostureMap) -> SecretStorageStatus {
    let mode = if posture.is_empty() {
        "none"
    } else if posture.values().any(|p| *p == SecretPosture::FileFallback) {
        "file_fallback"
    } else {
        "keychain"
    };
    SecretStorageStatus {
        mode,
        secrets: posture.clone(),
    }
}

/// Posture of the secrets present in `s` when the keychain is not used at all
/// (hermetic test constructors): everything present is in the file.
pub(crate) fn file_only_posture(s: &Settings) -> PostureMap {
    SECRET_NAMES
        .into_iter()
        .filter(|n| !secret_value(s, n).is_empty())
        .map(|n| (n, SecretPosture::FileFallback))
        .collect()
}

/// Log the outcome of the startup migration (no secret values, names only).
pub(crate) fn log_migration(posture: &PostureMap) {
    info!(
        target: "4da::keystore",
        secrets = ?posture.keys().collect::<Vec<_>>(),
        service = %keystore::profile_service_name(),
        "Secrets verified in the OS credential store — removed from settings.json and its backup"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sid_from_whoami_csv() {
        let out = "\"desktop-x\\\\administrator\",\"S-1-5-21-111-222-333-500\"\r\n";
        assert_eq!(
            parse_whoami_sid(out).as_deref(),
            Some("S-1-5-21-111-222-333-500")
        );
        assert_eq!(parse_whoami_sid("garbage"), None);
        assert_eq!(parse_whoami_sid("\"S-1-5;rm\""), None);
    }

    #[test]
    fn scrub_removes_only_keychain_held_secrets() {
        let mut s = Settings::default();
        s.llm.api_key = "sk-held".into();
        s.license.license_key = "lic-fallback".into();
        let mut posture = PostureMap::new();
        posture.insert("llm_api_key", SecretPosture::Keychain);
        posture.insert("license_key", SecretPosture::FileFallback);
        let disk = scrubbed_for_disk(&s, &posture);
        assert!(disk.llm.api_key.is_empty());
        assert_eq!(disk.license.license_key, "lic-fallback");
        assert_eq!(status_of(&posture).mode, "file_fallback");
    }

    #[test]
    fn status_reports_keychain_only_when_every_secret_is_held() {
        let mut posture = PostureMap::new();
        assert_eq!(status_of(&posture).mode, "none");
        posture.insert("llm_api_key", SecretPosture::Keychain);
        assert_eq!(status_of(&posture).mode, "keychain");
    }
}
