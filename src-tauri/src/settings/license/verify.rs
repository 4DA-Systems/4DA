// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Ed25519 license key verification.
//!
//! Verifies self-signed `4DA-` prefixed license keys using an embedded
//! ed25519 public key. The private key is held server-side for generation.

use crate::error::Result;
use serde::{Deserialize, Serialize};

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
