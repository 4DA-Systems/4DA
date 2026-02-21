//! License verification, feature gating, and trial management.

use serde::{Deserialize, Serialize};

use super::LicenseConfig;

// ============================================================================
// Feature Tier Gating
// ============================================================================

/// Pro-gated features list
pub const PRO_FEATURES: &[&str] = &[
    "generate_ai_briefing",
    "get_latest_briefing",
    "generate_audio_briefing",
    "get_attention_report",
    "get_knowledge_gaps",
    "get_signal_chains",
    "get_project_health",
    "get_developer_dna",
    "export_developer_dna_markdown",
    "export_developer_dna_svg",
    "get_predicted_context",
    "generate_context_packet",
    "natural_language_query",
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
// License Key Verification (ed25519)
// ============================================================================

/// Ed25519 public key for license verification (hex-encoded)
/// The private key is held server-side for license generation.
const LICENSE_PUBLIC_KEY_HEX: &str =
    "a1b2c3d4e5f6071829304050607080901a2b3c4d5e6f0a1b2c3d4e5f6070809";

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
