//! License verification, feature gating, and trial management.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{info, warn};

use super::keystore;
use super::LicenseConfig;

// ============================================================================
// Keygen API Validation Constants
// ============================================================================

const KEYGEN_ACCOUNT_ID: &str = "runyourempirehq";

/// Base URL template for Keygen validation.
/// Full URL: `https://api.keygen.sh/v1/accounts/{ACCOUNT_ID}/licenses/actions/validate-key`
#[allow(dead_code)] // Reason: Keygen validation URL reserved for license activation flow
const KEYGEN_VALIDATE_URL: &str = "https://api.keygen.sh/v1/licenses/actions/validate-key";

/// Hours before a cached validation result is considered stale.
/// 7 days provides resilience for offline periods and intermittent keychain failures.
/// The cache is refreshed on every successful online validation, so active users
/// always have a fresh cache. The 7-day window only matters when offline.
const VALIDATION_CACHE_HOURS: u64 = 168;

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
        .checked_sub(std::time::Duration::from_secs(60))
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

// ============================================================================
// Periodic Runtime Re-validation
// ============================================================================

/// Epoch-seconds timestamp of the last license integrity check.
/// Initialized to 0 so the first call always triggers validation.
static LAST_LICENSE_CHECK: AtomicU64 = AtomicU64::new(0);

/// Flag set when a paid tier is downgraded to free during re-validation.
/// Read and cleared by `get_license_tier` to notify the frontend once.
static TIER_DOWNGRADED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Re-validate license integrity every 6 hours to prevent settings.json
/// manipulation granting paid features between restarts.
const LICENSE_REVALIDATION_INTERVAL_SECS: u64 = 21_600; // 6 hours

/// Reset the re-validation timestamp to force the next check to trigger.
/// Only available in tests.
#[cfg(test)]
pub(crate) fn reset_license_check_timestamp() {
    LAST_LICENSE_CHECK.store(0, Ordering::Relaxed);
}

/// Check if a license key is available — in memory, keychain, or validation cache.
///
/// Three-layer fallback chain:
/// 1. **In-memory** (loaded from settings.json at startup — fastest, most reliable since
///    the key is now always persisted to disk)
/// 2. **Keychain** (platform credential store — re-hydrates in-memory if found)
/// 3. **Validation cache** (Keygen result — 7-day TTL, prevents downgrade during offline)
///
/// Returns true and re-hydrates `license` if ANY layer has the key.
fn has_license_key_available(license: &mut LicenseConfig) -> bool {
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

    // Tertiary: check if we have a valid Keygen validation cache for a paid tier.
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

/// Periodically re-run license integrity checks at runtime.
///
/// If the tier claims paid access but no license key is present (checked
/// in memory, keychain, and validation cache), the tier is reset to "free".
/// Uses relaxed atomic ordering since a rare double-check is harmless.
fn maybe_revalidate_license() {
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

    if is_paid_tier(license.tier.as_str())
        && !is_trial_active(&license)
        && !has_license_key_available(&mut license)
    {
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
    }
}

// ============================================================================
// Feature Tier Gating
// ============================================================================

/// Signal-gated features list.
///
/// Registry / inventory of every Tauri command that requires Signal tier.
/// The enforcement itself happens via `require_signal_feature("name")` at the
/// top of each command — the name passed in is only used for error messaging
/// and auditing; tier checking is independent of this list. Keeping the list
/// accurate lets the frontend query gating status up-front (via
/// `is_signal_feature_available`) and lets the license audit compare intent
/// vs enforcement.
///
/// When adding a gate to a new command, append its name here.
/// See `docs/strategy/LICENSE-GATING-AUDIT-2026-04-15.md` for the full audit.
pub const SIGNAL_FEATURES: &[&str] = &[
    // Intelligence panels (original)
    "get_attention_report",
    "get_knowledge_gaps",
    "get_signal_chains",
    "get_signal_chains_predicted",
    "get_project_health",
    // Developer DNA un-gated (AD-026): free tier viral sharing of DNA cards
    // natural_language_query removed — BYOK: runs on user's API key at zero cost (AD-025)
    "get_semantic_shifts",
    "get_decision_signals",
    "synthesize_search",
    "standing_queries",
    // Additional panels added by LICENSE-GATING-AUDIT-2026-04-15
    "get_blind_spots",
    "get_preemption_alerts",
    "resolve_signal_chain",
    "get_decision_health_report",
    // Cross-project intelligence
    "get_tech_convergence",
    "get_project_health_comparison",
    "get_cross_project_dependencies",
    // Accuracy / intelligence reporting
    "get_accuracy_report",
    "get_intelligence_report",
    // Trust ledger analytics
    "get_domain_precision_report",
    "get_false_positive_analysis",
];

/// Check if the current user has Signal (or Team/Enterprise) tier access.
/// Returns true for "signal", "team", "enterprise", legacy "pro", or an active trial.
/// Triggers periodic re-validation to catch settings.json manipulation.
pub fn is_signal() -> bool {
    maybe_revalidate_license();
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let license = &guard.get().license;
    is_paid_tier(license.tier.as_str()) || is_trial_active(license)
}

/// Validate license integrity on startup.
/// If tier claims "signal"/"team"/"enterprise" but no valid license key exists
/// (checked in memory, keychain, and validation cache), reset tier to "free".
/// Also initializes the periodic re-validation timestamp.
pub fn validate_license_on_startup() {
    let manager = crate::get_settings_manager();
    let mut guard = manager.lock();
    let mut license = guard.get().license.clone();

    // If tier is paid but no license key is set, reset to free
    if is_paid_tier(license.tier.as_str())
        && !is_trial_active(&license)
        && !has_license_key_available(&mut license)
    {
        warn!(
            "License tier is '{}' but no license key found (checked memory, keychain, and cache) — resetting to free",
            license.tier
        );
        guard.get_mut().license.tier = "free".to_string();
        TIER_DOWNGRADED.store(true, Ordering::Relaxed);
        if let Err(e) = guard.save() {
            warn!("Failed to reset license tier: {}", e);
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
    load_validation_cache().map(|c| c.validated_at)
}

/// Check if a tier string represents a paid tier.
/// Accepts legacy "pro" for backwards compatibility with existing settings.json files.
fn is_paid_tier(tier: &str) -> bool {
    matches!(tier, "signal" | "team" | "enterprise" | "pro")
}

/// Check if a feature is available for the given tier, including trial period
pub fn is_signal_feature_available(feature: &str, license: &LicenseConfig) -> bool {
    if is_paid_tier(license.tier.as_str()) {
        return true;
    }
    if is_trial_active(license) {
        return true;
    }
    !SIGNAL_FEATURES.contains(&feature)
}

/// Gate a Signal feature — returns Ok(()) if allowed, Err if not
/// Call at the top of any Signal-gated Tauri command.
/// Triggers periodic re-validation to catch settings.json manipulation.
pub fn require_signal_feature(feature: &str) -> Result<()> {
    maybe_revalidate_license();
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let license = &guard.get().license;
    if is_signal_feature_available(feature, license) {
        Ok(())
    } else {
        Err(format!("{feature} requires 4DA Signal — upgrade or start a free trial").into())
    }
}

/// Trial duration in days. Reverse trial: auto-starts on first launch,
/// giving users enough time for compound intelligence effects to demonstrate value.
const TRIAL_DURATION_DAYS: i64 = 14;

/// Check if the free trial is still active (14 days from trial_started_at)
pub fn is_trial_active(license: &LicenseConfig) -> bool {
    if is_paid_tier(license.tier.as_str()) {
        return false; // Not on trial, has a real license
    }
    match &license.trial_started_at {
        Some(started) => {
            if let Ok(start_date) = chrono::DateTime::parse_from_rfc3339(started) {
                let elapsed = chrono::Utc::now().signed_duration_since(start_date);
                elapsed.num_days() < TRIAL_DURATION_DAYS
            } else {
                false
            }
        }
        None => false, // Trial not started yet
    }
}

/// Get trial status info
pub fn get_trial_status(license: &LicenseConfig) -> TrialStatus {
    if is_paid_tier(license.tier.as_str()) {
        return TrialStatus {
            active: false,
            days_remaining: 0,
            started_at: None,
            has_license: true,
        };
    }
    match &license.trial_started_at {
        Some(started) => {
            if let Ok(start_date) = chrono::DateTime::parse_from_rfc3339(started) {
                let elapsed = chrono::Utc::now().signed_duration_since(start_date);
                let remaining = TRIAL_DURATION_DAYS - elapsed.num_days();
                TrialStatus {
                    active: remaining > 0,
                    days_remaining: remaining.max(0) as i32,
                    started_at: Some(started.clone()),
                    has_license: false,
                }
            } else {
                TrialStatus {
                    active: false,
                    days_remaining: 0,
                    started_at: Some(started.clone()),
                    has_license: false,
                }
            }
        }
        None => TrialStatus {
            active: false,
            days_remaining: 0,
            started_at: None,
            has_license: false,
        },
    }
}

/// Trial status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialStatus {
    pub active: bool,
    pub days_remaining: i32,
    pub started_at: Option<String>,
    pub has_license: bool,
}

// ============================================================================
// Keygen API Validation (online license verification)
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

/// Load the validation cache from disk. Returns `None` if missing or unparseable.
fn load_validation_cache() -> Option<KeygenValidationCache> {
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
fn is_cache_valid(cache: &KeygenValidationCache, current_key: &str) -> bool {
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

// ============================================================================
// License Key Verification (ed25519)
// ============================================================================

/// Ed25519 public key for license verification (hex-encoded)
/// The private key is held server-side for license generation.
const LICENSE_PUBLIC_KEY_HEX: &str =
    "084dc1b1b9549bf0ddff11db9186cb623ceb9d72831fbf2e6f01db160388f9d6";

/// License payload embedded in a signed license key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicensePayload {
    pub tier: String,
    pub email: String,
    pub expires_at: String,
    pub issued_at: String,
    #[serde(default)]
    pub features: Vec<String>,
}

/// Verify and decode a license key.
/// Format: `4DA-{base64(json_payload)}.{base64(ed25519_signature)}`
pub fn verify_license_key(key: &str) -> Result<LicensePayload> {
    // Strip ALL whitespace — users copying keys from emails often get line breaks
    // or spaces injected in the middle of the base64. Valid keys never contain spaces.
    let key: String = key.chars().filter(|c| !c.is_whitespace()).collect();

    // Sanity check: license keys are ~300-400 chars; reject obvious junk early
    if key.len() > 1024 {
        return Err("Invalid license: key too long".into());
    }

    // Must start with 4DA- prefix
    let body = key
        .strip_prefix("4DA-")
        .ok_or("Invalid license format: must start with 4DA-")?;

    // Split payload and signature
    let parts: Vec<&str> = body.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err("Invalid license format: missing signature".into());
    }

    let payload_b64 = parts[0];
    let sig_b64 = parts[1];

    // Decode payload
    let payload_bytes =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, payload_b64)
            .map_err(|e| format!("Invalid payload encoding: {e}"))?;

    // Decode signature
    let sig_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, sig_b64)
        .map_err(|e| format!("Invalid signature encoding: {e}"))?;

    // Decode public key
    let pub_key_bytes =
        hex::decode(LICENSE_PUBLIC_KEY_HEX).map_err(|e| format!("Invalid public key: {e}"))?;

    if pub_key_bytes.len() != 32 {
        return Err("Invalid public key length".into());
    }

    if sig_bytes.len() != 64 {
        return Err("Invalid signature length".into());
    }

    // Verify ed25519 signature
    use ed25519_dalek::{Signature, VerifyingKey};

    let verifying_key = VerifyingKey::from_bytes(
        pub_key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid public key bytes")?,
    )
    .map_err(|e| format!("Invalid public key: {e}"))?;

    let signature = Signature::from_bytes(
        sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid signature bytes")?,
    );

    use ed25519_dalek::Verifier;
    verifying_key
        .verify(&payload_bytes, &signature)
        .map_err(|_| "Invalid license: signature verification failed".to_string())?;

    // Parse payload JSON
    let payload: LicensePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|e| format!("Invalid license payload: {e}"))?;

    // Check expiration
    if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(&payload.expires_at) {
        if chrono::Utc::now() > expires {
            return Err("License has expired".into());
        }
    }

    Ok(payload)
}
