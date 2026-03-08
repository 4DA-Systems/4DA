//! License verification, feature gating, and trial management.

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::LicenseConfig;

// ============================================================================
// Keygen API Validation Constants
// ============================================================================

const KEYGEN_ACCOUNT_ID: &str = "runyourempirehq";

/// Base URL template for Keygen validation.
/// Full URL: `https://api.keygen.sh/v1/accounts/{ACCOUNT_ID}/licenses/actions/validate-key`
#[allow(dead_code)]
const KEYGEN_VALIDATE_URL: &str = "https://api.keygen.sh/v1/licenses/actions/validate-key";

/// Hours before a cached validation result is considered stale
const VALIDATION_CACHE_HOURS: u64 = 24;

// ============================================================================
// Feature Tier Gating
// ============================================================================

/// Pro-gated features list
pub const PRO_FEATURES: &[&str] = &[
    "generate_ai_briefing",
    "get_latest_briefing",
    "get_attention_report",
    "get_knowledge_gaps",
    "get_signal_chains",
    "get_project_health",
    "get_developer_dna",
    "export_developer_dna_markdown",
    "export_developer_dna_svg",
    "natural_language_query",
    "get_semantic_shifts",
    "generate_weekly_digest",
    "get_decision_signals",
];

/// Check if a feature is available for the given tier, including trial period
pub fn is_pro_feature_available(feature: &str, license: &LicenseConfig) -> bool {
    match license.tier.as_str() {
        "pro" | "team" => true,
        _ => {
            if is_trial_active(license) {
                return true;
            }
            !PRO_FEATURES.contains(&feature)
        }
    }
}

/// Gate a Pro feature — returns Ok(()) if allowed, Err if not
/// Call at the top of any Pro-gated Tauri command
pub fn require_pro_feature(feature: &str) -> Result<(), String> {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let license = &guard.get().license;
    if is_pro_feature_available(feature, license) {
        Ok(())
    } else {
        Err(format!(
            "{} requires 4DA Pro — upgrade or start a free trial",
            feature
        ))
    }
}

/// Check if the free trial is still active (30 days from trial_started_at)
pub fn is_trial_active(license: &LicenseConfig) -> bool {
    if license.tier == "pro" || license.tier == "team" {
        return false; // Not on trial, has a real license
    }
    match &license.trial_started_at {
        Some(started) => {
            if let Ok(start_date) = chrono::DateTime::parse_from_rfc3339(started) {
                let elapsed = chrono::Utc::now().signed_duration_since(start_date);
                elapsed.num_days() < 30
            } else {
                false
            }
        }
        None => false, // Trial not started yet
    }
}

/// Get trial status info
pub fn get_trial_status(license: &LicenseConfig) -> TrialStatus {
    if license.tier == "pro" || license.tier == "team" {
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
                let remaining = 30 - elapsed.num_days();
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
fn cache_path() -> std::path::PathBuf {
    let mut base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.push("data");
    base.push("license_cache.json");
    base
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
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Persist the validation cache to disk.
fn save_validation_cache(cache: &KeygenValidationCache) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(cache) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                warn!(target: "4da::license", error = %e, "Failed to write license cache");
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
        "https://api.keygen.sh/v1/accounts/{}/licenses/actions/validate-key",
        KEYGEN_ACCOUNT_ID
    );

    let client = reqwest::Client::new();
    let response = client
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
                        detail: format!("Network error reading response: {}", e),
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
                detail: format!("Network error: {}", e),
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
                detail: format!("Invalid response from Keygen (HTTP {})", status),
                code: "PARSE_ERROR".to_string(),
            };
        }
    };

    // Keygen returns { "meta": { "valid": true/false, "code": "..." }, "data": { ... } }
    let valid = json
        .pointer("/meta/valid")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let validation_code = json
        .pointer("/meta/code")
        .and_then(|v| v.as_str())
        .unwrap_or("UNKNOWN")
        .to_string();

    if valid {
        // Extract tier from license metadata if available
        let tier = json
            .pointer("/data/attributes/metadata/tier")
            .and_then(|v| v.as_str())
            .unwrap_or("pro")
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
            detail: format!("Valid ({})", validation_code),
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

        KeygenValidationResult {
            online: true,
            tier: "free".to_string(),
            cached: false,
            detail: format!("Invalid key ({})", validation_code),
            code: validation_code,
        }
    }
}

// ============================================================================
// STREETS Feature Gating
// ============================================================================

/// STREETS Community-gated features
pub const STREETS_COMMUNITY_FEATURES: &[&str] = &[
    "streets_community",
    "coach_create_session",
    "coach_send_message",
    "coach_recommend_engines",
    "coach_generate_strategy",
    "coach_launch_review",
    "coach_progress_check_in",
    "streets_premium_templates",
];

/// STREETS Cohort-gated features (includes Community features)
pub const STREETS_COHORT_FEATURES: &[&str] = &[
    "streets_cohort",
    "video_curriculum_access",
    "strategy_deep_dive",
];

/// Check if a STREETS feature is available
pub fn is_streets_feature_available(feature: &str, license: &LicenseConfig) -> bool {
    // Pro/team tiers get everything
    match license.tier.as_str() {
        "pro" | "team" => return true,
        _ => {}
    }
    // Check license key features
    if !license.license_key.is_empty() {
        if let Ok(payload) = verify_license_key(&license.license_key) {
            // Cohort includes community features
            if payload.features.contains(&"streets_cohort".to_string()) {
                return STREETS_COMMUNITY_FEATURES.contains(&feature)
                    || STREETS_COHORT_FEATURES.contains(&feature);
            }
            if payload.features.contains(&"streets_community".to_string()) {
                return STREETS_COMMUNITY_FEATURES.contains(&feature);
            }
            // Direct feature check
            return payload.features.contains(&feature.to_string());
        }
    }
    false
}

/// Gate a STREETS feature — returns Ok(()) if allowed, Err if not
pub fn require_streets_feature(feature: &str) -> Result<(), String> {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let license = &guard.get().license;
    if is_streets_feature_available(feature, license) {
        Ok(())
    } else {
        let tier_needed = if STREETS_COHORT_FEATURES.contains(&feature) {
            "STREETS Cohort"
        } else {
            "STREETS Community"
        };
        Err(format!(
            "{} requires {} membership — upgrade at streets.4da.ai",
            feature, tier_needed
        ))
    }
}

/// Get the user's current STREETS tier
pub fn get_streets_tier(license: &LicenseConfig) -> &'static str {
    match license.tier.as_str() {
        "pro" | "team" => "cohort", // Pro/team get everything
        _ => {
            if !license.license_key.is_empty() {
                if let Ok(payload) = verify_license_key(&license.license_key) {
                    if payload.features.contains(&"streets_cohort".to_string()) {
                        return "cohort";
                    }
                    if payload.features.contains(&"streets_community".to_string()) {
                        return "community";
                    }
                }
            }
            "playbook" // Free tier — all modules, no coaching
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
pub fn verify_license_key(key: &str) -> Result<LicensePayload, String> {
    let key = key.trim();

    // Sanity check: license keys are ~300-400 chars; reject obvious junk early
    if key.len() > 1024 {
        return Err("Invalid license: key too long".to_string());
    }

    // Must start with 4DA- prefix
    let body = key
        .strip_prefix("4DA-")
        .ok_or("Invalid license format: must start with 4DA-")?;

    // Split payload and signature
    let parts: Vec<&str> = body.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err("Invalid license format: missing signature".to_string());
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
        return Err("Invalid public key length".to_string());
    }

    if sig_bytes.len() != 64 {
        return Err("Invalid signature length".to_string());
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
            return Err("License has expired".to_string());
        }
    }

    Ok(payload)
}
