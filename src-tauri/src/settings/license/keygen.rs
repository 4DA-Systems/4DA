// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Keygen API validation, caching, and license backup file management.
//!
//! Handles online license verification against the Keygen API,
//! caches results for offline resilience, and maintains a backup
//! file as a fourth recovery layer for license persistence.

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::{LicenseConfig, KEYGEN_ACCOUNT_ID, VALIDATION_CACHE_HOURS};

// ============================================================================
// Keygen API Validation Types
// ============================================================================

/// Cached result of a Keygen API validation call.
/// Stored as JSON in `data/license_cache.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeygenValidationCache {
    /// ISO-8601 timestamp of the last successful validation
    pub validated_at: String,
    /// Tier returned by the validation (e.g. "pro", "free")
    pub tier: String,
    /// SHA-256 hash of the license key (detect key changes without storing the key)
    pub key_hash: String,
}

/// Result returned by `validate_license_key_keygen`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeygenValidationResult {
    /// Whether validation reached the API successfully
    pub online: bool,
    /// The resolved tier after validation
    pub tier: String,
    /// Whether a cached result was used
    pub cached: bool,
    /// Human-readable detail message
    pub detail: String,
    /// Raw Keygen validation code (e.g., "VALID", "NO_MACHINES", "NOT_FOUND")
    #[serde(default)]
    pub code: String,
}

// ============================================================================
// Cache Path + Hashing
// ============================================================================

/// Validation cache file location for a given data directory.
///
/// NOTE: post-startup callers derive `data_dir` from `get_db_path().parent()`
/// rather than the SettingsManager to avoid a deadlock — those paths already
/// hold the settings lock (validate_license_on_startup, maybe_revalidate_license).
/// Construction-time callers pass the explicit `data_dir` for hermeticity — see
/// the backup helpers below.
fn cache_path_in(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("license_cache.json")
}

/// SHA-256 hash a license key to a hex string (for cache comparison).
pub(crate) fn hash_key(key: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

// ============================================================================
// Cache I/O
// ============================================================================

/// Load the validation cache from an explicit data directory.
pub(crate) fn load_validation_cache_from(
    data_dir: &std::path::Path,
) -> Option<KeygenValidationCache> {
    let path = cache_path_in(data_dir);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(target: "4da::license", error = %e, "Failed to read license cache");
            }
            return None;
        }
    };
    match serde_json::from_str(&content) {
        Ok(cache) => Some(cache),
        Err(e) => {
            warn!(target: "4da::license", error = %e, "Failed to parse license cache — will be regenerated");
            None
        }
    }
}

/// Load the validation cache from disk. Returns `None` if missing or unparseable.
pub(crate) fn load_validation_cache() -> Option<KeygenValidationCache> {
    let db_path = crate::state::get_db_path();
    load_validation_cache_from(
        db_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("data")),
    )
}

/// Persist the validation cache into an explicit data directory.
pub(crate) fn save_validation_cache_to(data_dir: &std::path::Path, cache: &KeygenValidationCache) {
    let path = cache_path_in(data_dir);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(cache) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, &json) {
                warn!(target: "4da::license", error = %e, "Failed to write license cache");
            } else {
                // Restrict to owner-only on Unix (matches settings.json handling)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
                }
            }
        }
        Err(e) => {
            warn!(target: "4da::license", error = %e, "Failed to serialize license cache");
        }
    }
}

/// Persist the validation cache to disk.
fn save_validation_cache(cache: &KeygenValidationCache) {
    let db_path = crate::state::get_db_path();
    save_validation_cache_to(
        db_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("data")),
        cache,
    );
}

/// Check if the cached validation is still fresh (< VALIDATION_CACHE_HOURS old)
/// and matches the current license key.
pub(crate) fn is_cache_valid(cache: &KeygenValidationCache, current_key: &str) -> bool {
    // Key must match
    if cache.key_hash != hash_key(current_key) {
        return false;
    }
    // Must not be stale
    if let Ok(validated) = chrono::DateTime::parse_from_rfc3339(&cache.validated_at) {
        let age = chrono::Utc::now().signed_duration_since(validated);
        return age.num_hours() < VALIDATION_CACHE_HOURS as i64;
    }
    false
}

// ============================================================================
// License Backup File (4th recovery layer)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LicenseBackup {
    pub(crate) license_key: String,
    pub(crate) tier: String,
    pub(crate) activated_at: String,
    backup_created_at: String,
}

/// Backup file location for a given data directory.
fn backup_path_in(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("license_backup.json")
}

/// Global backup path, derived from the runtime DB path. Used by callers that
/// run after startup (activation, runtime re-validation) where `get_db_path()`
/// is authoritative. Construction-time callers must use the `*_in`/`*_from`
/// variants with the explicit `data_dir` instead — `get_db_path()` is a global
/// and would read a different directory than the one being loaded, breaking
/// test hermeticity.
fn backup_path() -> std::path::PathBuf {
    let db_path = crate::state::get_db_path();
    backup_path_in(
        db_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("data")),
    )
}

/// Save a license backup into an explicit data directory.
pub fn save_license_backup_to(
    data_dir: &std::path::Path,
    key: &str,
    tier: &str,
    activated_at: &str,
) {
    if key.is_empty() {
        return;
    }
    let backup = LicenseBackup {
        license_key: key.to_string(),
        tier: tier.to_string(),
        activated_at: activated_at.to_string(),
        backup_created_at: chrono::Utc::now().to_rfc3339(),
    };
    let path = backup_path_in(data_dir);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(&backup) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, &json) {
                warn!(target: "4da::license", error = %e, "Failed to write license backup");
            } else {
                info!(target: "4da::license", "License backup saved");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
                }
            }
        }
        Err(e) => {
            warn!(target: "4da::license", error = %e, "Failed to serialize license backup");
        }
    }
}

pub fn save_license_backup(key: &str, tier: &str, activated_at: &str) {
    let db_path = crate::state::get_db_path();
    let data_dir = db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("data"));
    save_license_backup_to(data_dir, key, tier, activated_at);
}

/// Load a license backup from an explicit data directory.
pub(crate) fn load_license_backup_from(data_dir: &std::path::Path) -> Option<LicenseBackup> {
    let path = backup_path_in(data_dir);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(target: "4da::license", error = %e, "Failed to read license backup");
            }
            return None;
        }
    };
    match serde_json::from_str(&content) {
        Ok(backup) => Some(backup),
        Err(e) => {
            warn!(target: "4da::license", error = %e, "Failed to parse license backup");
            None
        }
    }
}

pub(crate) fn load_license_backup() -> Option<LicenseBackup> {
    let path = backup_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(target: "4da::license", error = %e, "Failed to read license backup");
            }
            return None;
        }
    };
    match serde_json::from_str(&content) {
        Ok(backup) => Some(backup),
        Err(e) => {
            warn!(target: "4da::license", error = %e, "Failed to parse license backup");
            None
        }
    }
}

// ============================================================================
// Keygen API Validation (online license verification)
// ============================================================================

/// Validate a license key against the Keygen API.
///
/// **Offline-tolerant:** on network failure the current tier from settings
/// is preserved (no downgrade). Invalid keys resolve to `"free"`.
/// Results are cached for `VALIDATION_CACHE_HOURS` hours.
pub async fn validate_license_key_keygen(
    license_key: &str,
    current_tier: &str,
) -> KeygenValidationResult {
    validate_license_key_keygen_inner(license_key, current_tier, false).await
}

/// Force-validate without using cache. Used during explicit activation.
pub async fn validate_license_key_keygen_fresh(
    license_key: &str,
    current_tier: &str,
) -> KeygenValidationResult {
    validate_license_key_keygen_inner(license_key, current_tier, true).await
}

async fn validate_license_key_keygen_inner(
    license_key: &str,
    current_tier: &str,
    skip_cache: bool,
) -> KeygenValidationResult {
    // Safety guard: self-signed 4DA- keys must NEVER be sent to the Keygen API.
    // They are verified locally via ed25519. Sending them to Keygen returns a
    // rejection that gets cached as tier "free", corrupting the license state.
    if license_key.starts_with("4DA-") {
        tracing::warn!(
            target: "4da::license",
            "BUG GUARD: validate_license_key_keygen called with self-signed key — returning current tier"
        );
        return KeygenValidationResult {
            online: false,
            cached: false,
            tier: current_tier.to_string(),
            code: "self_signed".to_string(),
            detail: "Self-signed key — use local verification".to_string(),
        };
    }

    if license_key.trim().is_empty() {
        return KeygenValidationResult {
            online: false,
            tier: "free".to_string(),
            cached: false,
            detail: "No license key provided".to_string(),
            code: String::new(),
        };
    }

    // Check cache first (unless explicitly skipped, e.g. during activation)
    if !skip_cache {
        if let Some(cache) = load_validation_cache() {
            if is_cache_valid(&cache, license_key) {
                info!(target: "4da::license", tier = %cache.tier, "Using cached Keygen validation");
                return KeygenValidationResult {
                    online: false,
                    tier: cache.tier.clone(),
                    cached: true,
                    detail: format!("Cached validation from {}", cache.validated_at),
                    code: "CACHED".to_string(),
                };
            }
        }
    }

    // Simple key-only validation (no fingerprint scope).
    // Device-level licensing can be added later for Team tier if needed.
    let body = serde_json::json!({
        "meta": {
            "key": license_key
        }
    });

    let url = format!(
        "https://api.keygen.sh/v1/accounts/{KEYGEN_ACCOUNT_ID}/licenses/actions/validate-key"
    );

    let response = crate::http_client::HTTP_CLIENT
        .post(&url)
        .header("Content-Type", "application/vnd.api+json")
        .header("Accept", "application/vnd.api+json")
        .json(&body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            match resp.text().await {
                Ok(text) => parse_keygen_response(status.as_u16(), &text, license_key),
                Err(e) => {
                    warn!(target: "4da::license", error = %e, "Failed to read Keygen response body");
                    KeygenValidationResult {
                        online: false,
                        tier: current_tier.to_string(),
                        cached: false,
                        detail: format!("Network error reading response: {e}"),
                        code: "NETWORK_ERROR".to_string(),
                    }
                }
            }
        }
        Err(e) => {
            warn!(target: "4da::license", error = %e, "Keygen API unreachable, keeping current tier");
            KeygenValidationResult {
                online: false,
                tier: current_tier.to_string(),
                cached: false,
                detail: format!("Network error: {e}"),
                code: "NETWORK_ERROR".to_string(),
            }
        }
    }
}

/// Parse the JSON response from the Keygen validation endpoint and update cache.
fn parse_keygen_response(status: u16, body: &str, license_key: &str) -> KeygenValidationResult {
    let json: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            warn!(target: "4da::license", error = %e, status, "Failed to parse Keygen response");
            return KeygenValidationResult {
                online: true,
                tier: "free".to_string(),
                cached: false,
                detail: format!("Invalid response from Keygen (HTTP {status})"),
                code: "PARSE_ERROR".to_string(),
            };
        }
    };

    // Keygen returns { "meta": { "valid": true/false, "code": "..." }, "data": { ... } }
    let valid = json
        .pointer("/meta/valid")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let validation_code = json
        .pointer("/meta/code")
        .and_then(|v| v.as_str())
        .unwrap_or("UNKNOWN")
        .to_string();

    if valid {
        // Extract tier from license metadata — MUST be present for valid keys.
        // Never default to a paid tier on missing metadata: that would silently
        // upgrade free users or cache a wrong tier. If metadata is absent, treat
        // the response as valid but don't upgrade — preserve whatever tier the
        // caller already has by returning "free" (callers compare and preserve).
        let tier = json
            .pointer("/data/attributes/metadata/tier")
            .and_then(|v| v.as_str())
            .unwrap_or("free")
            .to_string();

        info!(target: "4da::license", tier = %tier, code = %validation_code, "Keygen validation succeeded");

        // Cache the successful result
        let cache = KeygenValidationCache {
            validated_at: chrono::Utc::now().to_rfc3339(),
            tier: tier.clone(),
            key_hash: hash_key(license_key),
        };
        save_validation_cache(&cache);

        KeygenValidationResult {
            online: true,
            tier,
            cached: false,
            detail: format!("Valid ({validation_code})"),
            code: validation_code,
        }
    } else {
        info!(target: "4da::license", code = %validation_code, "Keygen validation failed");

        // Don't cache NO_MACHINES / NO_MACHINE — these are fixable by machine activation
        let is_machine_issue = validation_code == "NO_MACHINES"
            || validation_code == "NO_MACHINE"
            || validation_code == "FINGERPRINT_SCOPE_REQUIRED";

        if !is_machine_issue {
            let cache = KeygenValidationCache {
                validated_at: chrono::Utc::now().to_rfc3339(),
                tier: "free".to_string(),
                key_hash: hash_key(license_key),
            };
            save_validation_cache(&cache);
        }

        // Map Keygen error codes to human-readable messages
        let detail = match validation_code.as_str() {
            "NO_MACHINES" | "NO_MACHINE" => {
                "This license key requires device activation. Please contact support or check your email for activation instructions.".to_string()
            }
            "FINGERPRINT_SCOPE_REQUIRED" => {
                "This license key requires device registration. Please contact support.".to_string()
            }
            "SUSPENDED" => "This license has been suspended. Please contact support.".to_string(),
            "EXPIRED" => {
                "This license has expired. Renew at 4da.ai/signal to get a new key.".to_string()
            }
            "NOT_FOUND" => "License key not recognized. Please check and try again.".to_string(),
            _ => format!("License validation failed ({validation_code})"),
        };

        KeygenValidationResult {
            online: true,
            tier: "free".to_string(),
            cached: false,
            detail,
            code: validation_code,
        }
    }
}

// Re-export verify_license_key used only by has_license_key_available in revalidation
// (kept as a cross-module dependency — verify.rs owns the function)
pub(crate) use super::verify::verify_license_key as verify_license_key_ed25519;

/// Is `key` a USABLE licence key for the fast path of `has_license_key_available`?
///
/// - A self-signed `4DA-` key is usable only if it verifies (Ed25519 signature +
///   embedded expiry — `verify_license_key` checks both). This is what makes an
///   expired or tampered `4DA-` key fail the availability check and downgrade.
/// - A Keygen-format key carries no local signature, so PRESENCE PROVES NOTHING.
///   It is usable only if the validation cache vouches for THIS key: matching key
///   hash, a paid tier, and still inside the freshness window.
///
/// The second rule closes a fail-open. This function used to `return true` for any
/// non-empty string that did not start with `4DA-`, and because it is the FAST
/// PATH it short-circuited before the validation cache (Layer 4) was ever reached.
/// Writing `{"tier":"signal","license_key":"x"}` into settings.json therefore
/// granted Signal permanently — no signature, no network call, no expiry. The
/// comment on `has_license_key_available` asserted that the cache established
/// validity for these keys; the cache was never consulted. Fixing it here fixes
/// the keychain path too, which shares this helper.
fn key_is_usable(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    if key.starts_with("4DA-") {
        return verify_license_key_ed25519(key).is_ok();
    }
    cache_vouches_for_key(load_validation_cache().as_ref(), key)
}

/// Does the validation cache prove that THIS key validated online as a paid tier,
/// recently enough to still be trusted?
///
/// Split out from `key_is_usable` so it is testable without touching disk.
/// `is_cache_valid` already enforces the key-hash match and the freshness window;
/// the tier check is what stops a cache recording a `free` result from vouching
/// for anything.
pub(crate) fn cache_vouches_for_key(cache: Option<&KeygenValidationCache>, key: &str) -> bool {
    use crate::settings::license::gating::is_paid_tier;
    match cache {
        Some(c) => is_paid_tier(&c.tier) && is_cache_valid(c, key),
        None => false,
    }
}

/// Helper to check if a license key is available — four-layer fallback chain.
///
/// 1. **In-memory** (loaded from settings.json at startup)
/// 2. **Keychain** (platform credential store)
/// 3. **Backup file** (license_backup.json — survives settings corruption/reset)
/// 4. **Validation cache** (Keygen result — 90-day TTL, prevents offline downgrade)
///
/// Returns true and re-hydrates `license` if ANY layer has the key.
pub(crate) fn has_license_key_available(license: &mut LicenseConfig) -> bool {
    use super::keystore;
    use crate::settings::license::gating::is_paid_tier;

    // Fast path: in-memory key is present (loaded from settings.json at startup).
    //
    // "Present" is NOT "usable". A self-signed `4DA-` key must VERIFY — signature
    // AND embedded expiry — before it counts, because this is the check the
    // downgrade path in revalidation.rs gates on. Two holes this closes:
    //   * settings.json tamper: pasting `tier: "signal"` + any non-empty string
    //     used to grant Signal; a bare string now fails verification.
    //   * expiry enforcement: an EXPIRED `4DA-` key used to keep granting Signal
    //     forever, because the only automatic downgrade fires on an ABSENT key,
    //     and `validate_license` (which does check expiry) is a command the
    //     frontend never calls on its own. A cancelled monthly subscriber kept
    //     the tier ~indefinitely past their key's ~35-day expiry.
    // Keygen-format keys (no `4DA-` prefix) carry no local signature; their
    // validity is established by the online validation cache (Layer 4 below), so
    // for them "present and non-empty" remains the right fast-path answer.
    if key_is_usable(&license.license_key) {
        return true;
    }

    // Fallback: check keychain directly and re-hydrate if found. Covers users
    // who activated before the disk-persistence fix (settings.json key empty)
    // AND the case above where the in-memory `4DA-` key was present but not
    // usable — the keychain copy is checked on its own merits, never trusted for
    // being merely non-empty.
    if let Ok(Some(key)) = keystore::get_secret("license_key") {
        if key_is_usable(&key) {
            info!(
                target: "4da::license",
                "Re-hydrated usable license key from keychain"
            );
            license.license_key = key;
            return true;
        }
    }

    // Layer 3: backup file (separate from settings.json — survives settings corruption/reset)
    if let Some(backup) = load_license_backup() {
        if !backup.license_key.is_empty() {
            if backup.license_key.starts_with("4DA-") {
                if verify_license_key_ed25519(&backup.license_key).is_ok() {
                    info!(
                        target: "4da::license",
                        "Re-hydrated license key from backup file (ed25519 verified)"
                    );
                    license.license_key = backup.license_key;
                    license.tier = backup.tier;
                    license.activated_at = Some(backup.activated_at);
                    return true;
                }
            } else if cache_vouches_for_key(load_validation_cache().as_ref(), &backup.license_key) {
                // A Keygen key recovered from the backup file gets the same
                // treatment as one from settings.json: the cache must vouch for it.
                // This file used to be trusted on presence alone, which made it a
                // second write-a-file route to a permanent paid tier.
                info!(
                    target: "4da::license",
                    "Re-hydrated license key from backup file (Keygen format, cache-vouched)"
                );
                license.license_key = backup.license_key;
                license.tier = backup.tier;
                license.activated_at = Some(backup.activated_at);
                return true;
            }
        }
    }

    // Layer 4: check if we have a valid Keygen validation cache for a paid tier.
    // If the key was validated online recently, don't downgrade just because
    // both disk and keychain are temporarily unavailable.
    if let Some(cache) = load_validation_cache() {
        if is_paid_tier(&cache.tier) {
            if let Ok(validated) = chrono::DateTime::parse_from_rfc3339(&cache.validated_at) {
                let age = chrono::Utc::now().signed_duration_since(validated);
                if age.num_hours() < VALIDATION_CACHE_HOURS as i64 {
                    info!(
                        target: "4da::license",
                        tier = %cache.tier,
                        validated_at = %cache.validated_at,
                        "License key missing but valid Keygen cache exists — preserving tier"
                    );
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod key_usable_tests {
    use super::key_is_usable;

    #[test]
    fn empty_key_is_not_usable() {
        assert!(!key_is_usable(""));
    }

    #[test]
    fn a_hand_pasted_4da_string_is_rejected() {
        // The settings.json tamper: `license_key: "4DA-anything"` used to pass the
        // fast path on non-emptiness alone and grant Signal. It must now fail the
        // Ed25519 verification and be treated as no usable key.
        assert!(!key_is_usable("4DA-not-a-real-signed-key"));
        assert!(!key_is_usable("4DA-eyJ0aWVyIjoic2lnbmFsIn0.bm90YXNpZw"));
    }

    #[test]
    fn a_keygen_format_key_is_not_usable_on_presence_alone() {
        // THE fail-open this file's own comment claimed was closed. A non-`4DA-`
        // key carries no local signature, so presence proves nothing. With no
        // validation cache vouching for it — the state of any test environment,
        // and of any machine where someone hand-edited settings.json — it must
        // not be usable.
        assert!(!key_is_usable("BE3529-741BAF-DEADBEEF"));
        assert!(!key_is_usable("x"));
        assert!(!key_is_usable("totally-made-up"));
    }

    #[test]
    fn cache_must_vouch_for_the_same_key_a_paid_tier_and_be_fresh() {
        use super::{cache_vouches_for_key, hash_key, KeygenValidationCache};
        let key = "BE3529-741BAF-DEADBEEF";
        let now = chrono::Utc::now().to_rfc3339();
        let mk = |tier: &str, hash: String, at: String| KeygenValidationCache {
            validated_at: at,
            tier: tier.to_string(),
            key_hash: hash,
        };

        // Positive control — without this, the fix could be satisfied by
        // always returning false, which would strand every real customer.
        let good = mk("signal", hash_key(key), now.clone());
        assert!(cache_vouches_for_key(Some(&good), key));

        // No cache at all.
        assert!(!cache_vouches_for_key(None, key));

        // A cache for a DIFFERENT key must not vouch for this one.
        let other = mk("signal", hash_key("BE3529-OTHER-KEY"), now.clone());
        assert!(!cache_vouches_for_key(Some(&other), key));

        // A cache recording a FREE result proves the key is not paid.
        let free = mk("free", hash_key(key), now);
        assert!(!cache_vouches_for_key(Some(&free), key));

        // A stale cache must not vouch indefinitely.
        let stale_at = (chrono::Utc::now() - chrono::Duration::days(3650)).to_rfc3339();
        let stale = mk("signal", hash_key(key), stale_at);
        assert!(!cache_vouches_for_key(Some(&stale), key));

        // A malformed timestamp must fail closed, not parse to "now".
        let bad_time = mk("signal", hash_key(key), "not-a-timestamp".to_string());
        assert!(!cache_vouches_for_key(Some(&bad_time), key));
    }

    // The valid-signature-but-EXPIRED case (a cancelled monthly key past its ~35d
    // expiry) is not unit-testable here: minting a key that verifies requires the
    // server-side private seed, which the app deliberately does not hold. Expiry
    // rejection is exercised by verify.rs's own expiry check, which key_is_usable
    // routes every `4DA-` key through.
}
