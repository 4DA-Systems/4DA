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

/// Get the path to the license validation cache file.
/// Uses the runtime data directory (same location as settings.json and 4da.db)
/// so it works in both dev and production builds.
///
/// NOTE: Derives path from `get_db_path()` rather than the SettingsManager to
/// avoid a deadlock — this function is called from paths that already hold the
/// settings lock (validate_license_on_startup, maybe_revalidate_license).
fn cache_path() -> std::path::PathBuf {
    let db_path = crate::state::get_db_path();
    db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("data"))
        .join("license_cache.json")
}

/// SHA-256 hash a license key to a hex string (for cache comparison).
fn hash_key(key: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

// ============================================================================
// Cache I/O
// ============================================================================

/// Load the validation cache from disk. Returns `None` if missing or unparseable.
pub(crate) fn load_validation_cache() -> Option<KeygenValidationCache> {
    let path = cache_path();
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

/// Persist the validation cache to disk.
fn save_validation_cache(cache: &KeygenValidationCache) {
    let path = cache_path();
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

fn backup_path() -> std::path::PathBuf {
    let db_path = crate::state::get_db_path();
    db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("data"))
        .join("license_backup.json")
}

pub fn save_license_backup(key: &str, tier: &str, activated_at: &str) {
    if key.is_empty() {
        return;
    }
    let backup = LicenseBackup {
        license_key: key.to_string(),
        tier: tier.to_string(),
        activated_at: activated_at.to_string(),
        backup_created_at: chrono::Utc::now().to_rfc3339(),
    };
    let path = backup_path();
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

    // Fast path: in-memory key is present (loaded from settings.json at startup)
    if !license.license_key.is_empty() {
        return true;
    }

    // Fallback: check keychain directly and re-hydrate if found.
    // This covers the transition period for users who activated before the
    // disk-persistence fix — their settings.json may still have an empty key.
    if let Ok(Some(key)) = keystore::get_secret("license_key") {
        if !key.is_empty() {
            info!(
                target: "4da::license",
                "Re-hydrated license key from keychain (was missing from in-memory settings)"
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
            } else {
                info!(
                    target: "4da::license",
                    "Re-hydrated license key from backup file (Keygen format — trusted)"
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
