//! Team-tier licence verification for the relay.
//!
//! `POST /teams` is a bootstrap endpoint: it is the call that *issues* a client's
//! first JWT, so it cannot itself be gated on one. Before this module existed the
//! only gate was `if body.license_key_hash.is_empty()` — a client-supplied string
//! that the relay never checked against anything, so any non-empty value created a
//! team and minted an admin token.
//!
//! The gate is now a signature check against the same Ed25519 authority the desktop
//! app already trusts (`src-tauri/src/settings/license/verify.rs`). The client sends
//! the licence key itself; the relay verifies the signature, the tier and the expiry,
//! and derives the stored hash server-side. A client-supplied hash is never trusted,
//! because a hash proves nothing — only the signature does.

use base64::Engine as _;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

use crate::error::RelayError;

/// Ed25519 public key of the licence authority, hex-encoded.
///
/// Identical to `LICENSE_PUBLIC_KEY_HEX` in the desktop app — one authority signs
/// both. Overridable via `RELAY_LICENSE_PUBLIC_KEY` for staging relays that issue
/// against a different keypair.
const DEFAULT_LICENSE_PUBLIC_KEY_HEX: &str =
    "084dc1b1b9549bf0ddff11db9186cb623ceb9d72831fbf2e6f01db160388f9d6";

/// Tiers entitled to create a team on the managed relay.
const TEAM_TIERS: &[&str] = &["team", "enterprise"];

/// Licence keys are ~300-400 chars. Reject obvious junk before doing any crypto.
const MAX_LICENSE_KEY_LEN: usize = 1024;

/// The signed payload embedded in a licence key.
///
/// Mirrors the desktop `LicensePayload`. `#[serde(default)]` on `features` keeps
/// older keys parseable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicensePayload {
    pub tier: String,
    pub email: String,
    pub expires_at: String,
    pub issued_at: String,
    #[serde(default)]
    pub features: Vec<String>,
}

/// A licence that passed signature, tier and expiry checks.
#[derive(Debug, Clone)]
pub struct VerifiedLicense {
    pub payload: LicensePayload,
    /// SHA-256 of the whitespace-stripped key, hex-encoded. Derived here, never
    /// taken from the request.
    pub key_hash: String,
}

static VERIFYING_KEY: OnceLock<Result<VerifyingKey, String>> = OnceLock::new();

fn parse_verifying_key(hex_str: &str) -> Result<VerifyingKey, String> {
    let bytes = hex::decode(hex_str.trim())
        .map_err(|e| format!("licence public key is not valid hex: {e}"))?;
    let len = bytes.len();
    let bytes: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| format!("licence public key must be 32 bytes, got {len}"))?;
    VerifyingKey::from_bytes(&bytes).map_err(|e| format!("licence public key is invalid: {e}"))
}

/// Resolve the licence authority key once per process.
///
/// A malformed `RELAY_LICENSE_PUBLIC_KEY` is a hard error rather than a silent
/// fallback to the default: an operator who set it meant to point at a different
/// authority, and quietly accepting production keys instead would be worse than
/// refusing to create teams.
fn verifying_key() -> Result<&'static VerifyingKey, RelayError> {
    let resolved = VERIFYING_KEY.get_or_init(|| {
        let configured = std::env::var("RELAY_LICENSE_PUBLIC_KEY")
            .unwrap_or_else(|_| DEFAULT_LICENSE_PUBLIC_KEY_HEX.to_string());
        parse_verifying_key(&configured)
    });

    resolved
        .as_ref()
        .map_err(|e| RelayError::Internal(format!("licence verification unavailable: {e}")))
}

/// Force the verifying key, for tests that sign with their own keypair.
///
/// Every test in the crate shares one process, so the first caller wins and the
/// rest are no-ops — which is why all tests sign with the same static keypair.
#[cfg(test)]
pub fn set_verifying_key_for_tests(key: VerifyingKey) {
    let _ = VERIFYING_KEY.set(Ok(key));
}

/// SHA-256 of a canonicalised key, hex-encoded.
fn hash_key(canonical_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Verify a `4DA-` licence key and confirm it entitles the holder to a team.
///
/// Checks, in order and all fail-closed:
/// 1. length and `4DA-` prefix
/// 2. Ed25519 signature over the raw payload bytes
/// 3. payload parses as JSON
/// 4. `expires_at` is present, parseable and in the future
/// 5. `tier` is one of [`TEAM_TIERS`]
///
/// Note (4) is deliberately stricter than the desktop verifier, which uses
/// `if let Ok(expires)` and so treats an unparseable date as "not expired". On a
/// server that grants durable access, an unreadable expiry is a rejection.
pub fn verify_team_license(key: &str) -> Result<VerifiedLicense, RelayError> {
    let canonical: String = key.chars().filter(|c| !c.is_whitespace()).collect();

    if canonical.is_empty() {
        return Err(RelayError::BadRequest(
            "License key required to create a team".to_string(),
        ));
    }
    if canonical.len() > MAX_LICENSE_KEY_LEN {
        return Err(RelayError::Auth(
            "Invalid license: key too long".to_string(),
        ));
    }

    let body = canonical
        .strip_prefix("4DA-")
        .ok_or_else(|| RelayError::Auth("Invalid license format".to_string()))?;

    let (payload_b64, sig_b64) = body
        .split_once('.')
        .ok_or_else(|| RelayError::Auth("Invalid license format".to_string()))?;

    let engine = base64::engine::general_purpose::STANDARD;
    let payload_bytes = engine
        .decode(payload_b64)
        .map_err(|_| RelayError::Auth("Invalid license encoding".to_string()))?;
    let sig_bytes = engine
        .decode(sig_b64)
        .map_err(|_| RelayError::Auth("Invalid license encoding".to_string()))?;

    let sig_bytes: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| RelayError::Auth("Invalid license signature".to_string()))?;

    verifying_key()?
        .verify(&payload_bytes, &Signature::from_bytes(&sig_bytes))
        .map_err(|_| RelayError::Auth("Invalid license: signature check failed".to_string()))?;

    let payload: LicensePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| RelayError::Auth("Invalid license payload".to_string()))?;

    let expires = chrono::DateTime::parse_from_rfc3339(&payload.expires_at)
        .map_err(|_| RelayError::Auth("Invalid license: unreadable expiry".to_string()))?;
    if chrono::Utc::now() > expires {
        return Err(RelayError::Auth("License has expired".to_string()));
    }

    let tier = payload.tier.trim().to_ascii_lowercase();
    if !TEAM_TIERS.contains(&tier.as_str()) {
        return Err(RelayError::Auth(format!(
            "License tier '{}' does not include team sync",
            payload.tier
        )));
    }

    let key_hash = hash_key(&canonical);
    Ok(VerifiedLicense { payload, key_hash })
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    /// The keypair every test in this crate signs with. Installed into the
    /// `OnceLock` on first use so the relay verifies against it instead of the
    /// production authority.
    pub fn test_signing_key() -> &'static SigningKey {
        static KEY: OnceLock<SigningKey> = OnceLock::new();
        KEY.get_or_init(|| SigningKey::from_bytes(&[7u8; 32]))
    }

    pub fn install_test_key() {
        set_verifying_key_for_tests(test_signing_key().verifying_key());
    }

    /// Mint a licence key the way the licensing service does.
    pub fn mint(tier: &str, expires_at: &str) -> String {
        let payload = serde_json::json!({
            "tier": tier,
            "email": "team@example.com",
            "expires_at": expires_at,
            "issued_at": "2026-01-01T00:00:00Z",
            "features": ["team_sync"],
        });
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        let sig = test_signing_key().sign(&payload_bytes);
        let engine = base64::engine::general_purpose::STANDARD;
        format!(
            "4DA-{}.{}",
            engine.encode(&payload_bytes),
            engine.encode(sig.to_bytes())
        )
    }

    /// A valid team licence far from expiry — the happy path for HTTP tests.
    pub fn valid_team_license() -> String {
        install_test_key();
        mint(
            "team",
            &(chrono::Utc::now() + chrono::Duration::days(365)).to_rfc3339(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    fn far_future() -> String {
        (chrono::Utc::now() + chrono::Duration::days(365)).to_rfc3339()
    }

    #[test]
    fn accepts_a_signed_unexpired_team_license() {
        install_test_key();
        let verified = verify_team_license(&mint("team", &far_future())).unwrap();
        assert_eq!(verified.payload.tier, "team");
        assert_eq!(verified.key_hash.len(), 64);
    }

    #[test]
    fn accepts_enterprise_tier() {
        install_test_key();
        assert!(verify_team_license(&mint("enterprise", &far_future())).is_ok());
    }

    #[test]
    fn tier_check_is_case_insensitive() {
        install_test_key();
        assert!(verify_team_license(&mint("Team", &far_future())).is_ok());
    }

    /// The exact hole this module closes: before, any non-empty string worked.
    #[test]
    fn rejects_an_arbitrary_non_empty_string() {
        install_test_key();
        let long = "a".repeat(2000);
        for junk in ["testhash123", "x", "4DA-", "4DA-abc.def", long.as_str()] {
            assert!(
                verify_team_license(junk).is_err(),
                "unsigned junk was accepted: {junk}"
            );
        }
    }

    #[test]
    fn rejects_a_forged_signature() {
        install_test_key();
        let key = mint("team", &far_future());
        let (head, _) = key.rsplit_once('.').unwrap();
        let engine = base64::engine::general_purpose::STANDARD;
        let forged = format!("{head}.{}", engine.encode([0u8; 64]));
        assert!(verify_team_license(&forged).is_err());
    }

    /// A payload whose tier was swapped is still a forgery, because the tier is
    /// inside the signed bytes.
    #[test]
    fn rejects_a_free_tier_license() {
        install_test_key();
        let err = verify_team_license(&mint("free", &far_future())).unwrap_err();
        assert!(format!("{err:?}").contains("does not include team sync"));
    }

    #[test]
    fn rejects_an_expired_license() {
        install_test_key();
        let past = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        assert!(verify_team_license(&mint("team", &past)).is_err());
    }

    /// Unlike the desktop verifier, an unreadable expiry is refused rather than
    /// treated as "not expired".
    #[test]
    fn rejects_an_unparseable_expiry() {
        install_test_key();
        assert!(verify_team_license(&mint("team", "not-a-date")).is_err());
    }

    #[test]
    fn whitespace_in_a_pasted_key_is_tolerated_and_hashes_identically() {
        install_test_key();
        let key = mint("team", &far_future());
        let mangled = format!("{}\n {}", &key[..20], &key[20..]);
        let a = verify_team_license(&key).unwrap();
        let b = verify_team_license(&mangled).unwrap();
        assert_eq!(a.key_hash, b.key_hash);
    }

    #[test]
    fn default_public_key_is_a_well_formed_ed25519_key() {
        // Guards against a typo silently pointing the relay at no valid authority.
        assert!(parse_verifying_key(DEFAULT_LICENSE_PUBLIC_KEY_HEX).is_ok());
    }

    #[test]
    fn malformed_configured_key_is_an_error_not_a_fallback() {
        assert!(parse_verifying_key("nonsense").is_err());
        assert!(parse_verifying_key("aabb").is_err());
    }
}
