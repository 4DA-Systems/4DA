// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Cryptographic verification for SSO tokens and assertions.
//!
//! Provides:
//! - OIDC JWT signature verification with JWKS caching
//! - SAML XML-DSig RSA-SHA256 signature verification
//! - X.509 certificate PEM parsing

use base64::Engine as _;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use parking_lot::Mutex;
use rsa::pkcs1v15::VerifyingKey;
use rsa::signature::Verifier;
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::LazyLock;
use tracing::{info, warn};
use x509_cert::der::Decode;
use x509_cert::der::Encode;

use crate::sso_xml;

// ============================================================================
// JWKS Cache (for OIDC)
// ============================================================================

struct JwksCache {
    keys: HashMap<String, (DecodingKey, Algorithm)>,
    fetched_at: std::time::Instant,
    issuer: String,
}

static JWKS_CACHE: LazyLock<Mutex<Option<JwksCache>>> = LazyLock::new(|| Mutex::new(None));

const JWKS_CACHE_DURATION: std::time::Duration = std::time::Duration::from_secs(3600);

/// Fetch JWKS from the IdP's discovery endpoint and cache the keys.
pub(crate) async fn fetch_and_cache_jwks(issuer: &str) -> crate::error::Result<()> {
    let client = &crate::http_client::HTTP_CLIENT;

    // 1. Fetch OIDC discovery document
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        issuer.trim_end_matches('/')
    );
    let discovery: serde_json::Value = client
        .get(&discovery_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch OIDC discovery from {discovery_url}: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Invalid OIDC discovery response: {e}"))?;

    let jwks_uri = discovery["jwks_uri"]
        .as_str()
        .ok_or("OIDC discovery missing jwks_uri")?;

    // 2. Fetch JWKS
    let jwks: serde_json::Value = client
        .get(jwks_uri)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch JWKS from {jwks_uri}: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Invalid JWKS response: {e}"))?;

    // 3. Parse keys
    let mut keys = HashMap::new();
    if let Some(jwk_keys) = jwks["keys"].as_array() {
        for key in jwk_keys {
            let kid = match key["kid"].as_str() {
                Some(k) => k.to_string(),
                None => continue,
            };
            let kty = key["kty"].as_str().unwrap_or("");
            let alg_str = key["alg"].as_str().unwrap_or("RS256");

            let algorithm = match alg_str {
                "RS256" => Algorithm::RS256,
                "RS384" => Algorithm::RS384,
                "RS512" => Algorithm::RS512,
                "ES256" => Algorithm::ES256,
                "ES384" => Algorithm::ES384,
                _ => {
                    warn!(target: "4da::sso", kid = %kid, alg = %alg_str, "Skipping unsupported key algorithm");
                    continue;
                }
            };

            let decoding_key = match kty {
                "RSA" => {
                    let n = match key["n"].as_str() {
                        Some(v) => v,
                        None => continue,
                    };
                    let e = match key["e"].as_str() {
                        Some(v) => v,
                        None => continue,
                    };
                    match DecodingKey::from_rsa_components(n, e) {
                        Ok(dk) => dk,
                        Err(e) => {
                            warn!(target: "4da::sso", kid = %kid, error = %e, "Failed to parse RSA key");
                            continue;
                        }
                    }
                }
                "EC" => {
                    let x = match key["x"].as_str() {
                        Some(v) => v,
                        None => continue,
                    };
                    let y = match key["y"].as_str() {
                        Some(v) => v,
                        None => continue,
                    };
                    match DecodingKey::from_ec_components(x, y) {
                        Ok(dk) => dk,
                        Err(e) => {
                            warn!(target: "4da::sso", kid = %kid, error = %e, "Failed to parse EC key");
                            continue;
                        }
                    }
                }
                _ => {
                    warn!(target: "4da::sso", kid = %kid, kty = %kty, "Skipping unsupported key type");
                    continue;
                }
            };

            keys.insert(kid, (decoding_key, algorithm));
        }
    }

    if keys.is_empty() {
        return Err("No usable keys found in JWKS".into());
    }

    info!(target: "4da::sso", key_count = keys.len(), "JWKS cached successfully");

    let mut cache = JWKS_CACHE.lock();
    *cache = Some(JwksCache {
        keys,
        fetched_at: std::time::Instant::now(),
        issuer: issuer.to_string(),
    });

    Ok(())
}

/// Look up a cached JWKS key by kid. Returns None if cache is expired or key not found.
fn get_cached_jwk(kid: &str, issuer: &str) -> Option<(DecodingKey, Algorithm)> {
    let cache = JWKS_CACHE.lock();
    let c = cache.as_ref()?;

    if c.issuer != issuer {
        return None;
    }

    if c.fetched_at.elapsed() > JWKS_CACHE_DURATION {
        return None;
    }

    c.keys.get(kid).cloned()
}

// ============================================================================
// OIDC JWT Verification
// ============================================================================

/// Verify an OIDC ID token JWT against the IdP's JWKS keys.
///
/// Returns the validated claims as a JSON value.
pub(crate) async fn verify_oidc_token(
    id_token: &str,
    issuer: &str,
    client_id: &str,
    expected_nonce: Option<&str>,
) -> crate::error::Result<serde_json::Value> {
    // 1. Decode header to get kid and alg
    let header =
        jsonwebtoken::decode_header(id_token).map_err(|e| format!("Invalid JWT header: {e}"))?;

    let kid = header
        .kid
        .as_deref()
        .ok_or("JWT missing kid (key ID) in header")?;

    // 2. Look up key in cache, fetch if needed
    let (decoding_key, algorithm) = match get_cached_jwk(kid, issuer) {
        Some(key) => key,
        None => {
            // Fetch and retry
            fetch_and_cache_jwks(issuer).await?;
            get_cached_jwk(kid, issuer).ok_or_else(|| {
                format!("Unknown signing key (kid: {kid}). Key not found in IdP's JWKS.")
            })?
        }
    };

    // 3. Build validation
    let mut validation = Validation::new(algorithm);
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[client_id]);
    validation.validate_exp = true;

    // 4. Decode and verify
    let token_data =
        jsonwebtoken::decode::<serde_json::Value>(id_token, &decoding_key, &validation).map_err(
            |e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    "SSO token has expired. Please sign in again.".to_string()
                }
                jsonwebtoken::errors::ErrorKind::InvalidAudience => {
                    "SSO token was issued for a different application.".to_string()
                }
                jsonwebtoken::errors::ErrorKind::InvalidIssuer => {
                    "SSO token came from an unexpected identity provider.".to_string()
                }
                jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                    "SSO token signature is invalid. This could indicate tampering.".to_string()
                }
                _ => format!("SSO token validation failed: {e}"),
            },
        )?;

    // 5. Validate nonce if expected
    if let Some(expected) = expected_nonce {
        let actual = token_data.claims["nonce"]
            .as_str()
            .ok_or("SSO token missing nonce claim")?;
        if actual != expected {
            return Err("SSO nonce mismatch -- possible replay attack.".into());
        }
    }

    info!(target: "4da::sso", "OIDC JWT signature verified successfully");
    Ok(token_data.claims)
}

// ============================================================================
// SAML Signature Verification
// ============================================================================

/// Cached SAML certificate (PEM hash -> parsed RSA public key).
static SAML_CERT_CACHE: LazyLock<Mutex<Option<(String, rsa::RsaPublicKey)>>> =
    LazyLock::new(|| Mutex::new(None));

/// Parse a PEM-encoded X.509 certificate and extract the RSA public key.
fn parse_certificate_pem(cert_pem: &str) -> crate::error::Result<rsa::RsaPublicKey> {
    use sha2::Digest;

    // Compute cache key (SHA-256 of PEM text)
    let pem_hash = {
        let mut hasher = Sha256::new();
        hasher.update(cert_pem.as_bytes());
        hex::encode(hasher.finalize())
    };

    // Check cache
    {
        let cache = SAML_CERT_CACHE.lock();
        if let Some((ref hash, ref key)) = *cache {
            if *hash == pem_hash {
                return Ok(key.clone());
            }
        }
    }

    // Parse PEM -> DER
    let pem_body = cert_pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<String>();
    let der_bytes = base64::engine::general_purpose::STANDARD
        .decode(&pem_body)
        .map_err(|e| format!("Invalid base64 in certificate PEM: {e}"))?;

    // Parse X.509 certificate to extract SubjectPublicKeyInfo
    let cert = x509_cert::Certificate::from_der(&der_bytes)
        .map_err(|e| format!("Failed to parse X.509 certificate: {e}"))?;

    let spki_der = cert
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .map_err(|e| format!("Failed to encode SPKI: {e}"))?;

    let public_key = {
        use pkcs8::DecodePublicKey;
        rsa::RsaPublicKey::from_public_key_der(&spki_der)
    }
    .map_err(|e| format!("Failed to extract RSA public key from certificate: {e}"))?;

    // Cache the parsed key
    {
        let mut cache = SAML_CERT_CACHE.lock();
        *cache = Some((pem_hash, public_key.clone()));
    }

    Ok(public_key)
}

/// Verify the XML digital signature of a SAML assertion.
///
/// Implements RSA-SHA256 signature verification using:
/// 1. Extract and canonicalize the SignedInfo block
/// 2. Verify the SignatureValue against the canonicalized SignedInfo
/// 3. Extract the DigestValue and verify the assertion body digest
///
/// This uses a simplified exclusive C14N implementation that covers
/// the majority of real-world SAML IdPs (Okta, Azure AD, Google, OneLogin).
pub(crate) fn verify_saml_signature(xml: &str, certificate_pem: &str) -> crate::error::Result<()> {
    // 1. Parse certificate
    let public_key = parse_certificate_pem(certificate_pem)?;
    let verifying_key = VerifyingKey::<Sha256>::new(public_key);

    // 2. Extract SignedInfo and SignatureValue
    let signed_info =
        sso_xml::extract_signed_info(xml).ok_or("SAML response missing ds:SignedInfo element")?;
    let sig_b64 = sso_xml::extract_signature_value(xml)
        .ok_or("SAML response missing ds:SignatureValue element")?;

    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(&sig_b64)
        .map_err(|e| format!("Invalid base64 in SignatureValue: {e}"))?;

    // 3. Canonicalize SignedInfo
    let canonical_signed_info = sso_xml::canonicalize_xml(&signed_info);

    // 4. Verify RSA-SHA256 signature over canonicalized SignedInfo
    let signature = rsa::pkcs1v15::Signature::try_from(signature_bytes.as_slice())
        .map_err(|e| format!("Invalid RSA signature format: {e}"))?;

    verifying_key
        .verify(canonical_signed_info.as_bytes(), &signature)
        .map_err(|_| "SAML signature verification failed: invalid signature")?;

    // 5. Verify digest (assertion body hash)
    if let Some(expected_digest_b64) = sso_xml::extract_digest_value(xml) {
        let expected_digest = base64::engine::general_purpose::STANDARD
            .decode(&expected_digest_b64)
            .map_err(|e| format!("Invalid base64 in DigestValue: {e}"))?;

        // Remove Signature element and canonicalize the assertion body
        let assertion_body = sso_xml::remove_signature_element(xml);
        let canonical_body = sso_xml::canonicalize_xml(&assertion_body);

        // Compute SHA-256 digest
        use sha2::Digest;
        let mut hasher = Sha256::new();
        hasher.update(canonical_body.as_bytes());
        let actual_digest = hasher.finalize();

        if actual_digest.as_slice() != expected_digest.as_slice() {
            return Err(
                "SAML digest verification failed: assertion body has been tampered with".into(),
            );
        }
    }

    info!(target: "4da::sso", "SAML assertion signature verified successfully");
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate an ephemeral RSA-2048 key pair for test use.
    fn generate_test_rsa_keys() -> (jsonwebtoken::EncodingKey, DecodingKey) {
        use rsa::pkcs1::EncodeRsaPrivateKey;
        use rsa::pkcs1::EncodeRsaPublicKey;

        let mut rng = rand::thread_rng();
        let private_key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let public_key = private_key.to_public_key();

        let private_pem = private_key
            .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
            .unwrap();
        let public_der = public_key.to_pkcs1_der().unwrap();

        let encoding = jsonwebtoken::EncodingKey::from_rsa_pem(private_pem.as_bytes()).unwrap();
        let decoding = DecodingKey::from_rsa_der(public_der.as_bytes());

        (encoding, decoding)
    }

    #[test]
    fn test_jwks_cache_empty_returns_none() {
        assert!(get_cached_jwk("nonexistent", "https://example.com").is_none());
    }

    #[test]
    fn test_parse_pem_invalid_base64() {
        let result = parse_certificate_pem(
            "-----BEGIN CERTIFICATE-----\ninvalid!!\n-----END CERTIFICATE-----",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_jwt_rs256() {
        let (encoding_key, decoding_key) = generate_test_rsa_keys();

        let claims = serde_json::json!({
            "sub": "user-123",
            "email": "test@example.com",
            "name": "Test User",
            "iss": "https://test-idp.example.com",
            "aud": "test-client-id",
            "exp": 4102444800_u64,
            "iat": 1700000000_u64,
            "nonce": "test-nonce-123",
        });

        let mut header = jsonwebtoken::Header::new(Algorithm::RS256);
        header.kid = Some("test-key-1".to_string());

        let token = jsonwebtoken::encode(&header, &claims, &encoding_key).unwrap();

        // Set up JWKS cache with the test public key
        {
            let mut cache = JWKS_CACHE.lock();
            let mut keys = HashMap::new();
            keys.insert("test-key-1".to_string(), (decoding_key, Algorithm::RS256));
            *cache = Some(JwksCache {
                keys,
                fetched_at: std::time::Instant::now(),
                issuer: "https://test-idp.example.com".to_string(),
            });
        }

        // Verify the token by calling the underlying validation
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&["https://test-idp.example.com"]);
        validation.set_audience(&["test-client-id"]);
        validation.validate_exp = true;

        let cached = get_cached_jwk("test-key-1", "https://test-idp.example.com").unwrap();
        let result = jsonwebtoken::decode::<serde_json::Value>(&token, &cached.0, &validation);
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.claims["email"], "test@example.com");
    }

    #[test]
    fn test_verify_jwt_tampered_rejected() {
        let (encoding_key, decoding_key) = generate_test_rsa_keys();

        let claims = serde_json::json!({
            "sub": "user-123",
            "email": "test@example.com",
            "iss": "https://test-idp.example.com",
            "aud": "test-client-id",
            "exp": 4102444800_u64,
        });

        let mut header = jsonwebtoken::Header::new(Algorithm::RS256);
        header.kid = Some("test-key-1".to_string());

        let token = jsonwebtoken::encode(&header, &claims, &encoding_key).unwrap();

        // Tamper with the payload
        let parts: Vec<&str> = token.split('.').collect();
        let tampered_payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            r#"{"sub":"hacker","email":"evil@example.com","iss":"https://test-idp.example.com","aud":"test-client-id","exp":4102444800}"#
                .as_bytes(),
        );
        let tampered_token = format!("{}.{}.{}", parts[0], tampered_payload, parts[2]);

        {
            let mut cache = JWKS_CACHE.lock();
            let mut keys = HashMap::new();
            keys.insert("test-key-1".to_string(), (decoding_key, Algorithm::RS256));
            *cache = Some(JwksCache {
                keys,
                fetched_at: std::time::Instant::now(),
                issuer: "https://test-idp.example.com".to_string(),
            });
        }

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&["https://test-idp.example.com"]);
        validation.set_audience(&["test-client-id"]);

        let cached = get_cached_jwk("test-key-1", "https://test-idp.example.com").unwrap();
        let result =
            jsonwebtoken::decode::<serde_json::Value>(&tampered_token, &cached.0, &validation);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_jwt_expired_rejected() {
        let (encoding_key, decoding_key) = generate_test_rsa_keys();

        let claims = serde_json::json!({
            "sub": "user-123",
            "email": "test@example.com",
            "iss": "https://test-idp.example.com",
            "aud": "test-client-id",
            "exp": 946684800_u64,  // 2000-01-01 (expired)
        });

        let mut header = jsonwebtoken::Header::new(Algorithm::RS256);
        header.kid = Some("test-key-1".to_string());

        let token = jsonwebtoken::encode(&header, &claims, &encoding_key).unwrap();

        {
            let mut cache = JWKS_CACHE.lock();
            let mut keys = HashMap::new();
            keys.insert("test-key-1".to_string(), (decoding_key, Algorithm::RS256));
            *cache = Some(JwksCache {
                keys,
                fetched_at: std::time::Instant::now(),
                issuer: "https://test-idp.example.com".to_string(),
            });
        }

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&["https://test-idp.example.com"]);
        validation.set_audience(&["test-client-id"]);

        let cached = get_cached_jwk("test-key-1", "https://test-idp.example.com").unwrap();
        let result = jsonwebtoken::decode::<serde_json::Value>(&token, &cached.0, &validation);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_jwt_wrong_audience_rejected() {
        let (encoding_key, decoding_key) = generate_test_rsa_keys();

        let claims = serde_json::json!({
            "sub": "user-123",
            "email": "test@example.com",
            "iss": "https://test-idp.example.com",
            "aud": "wrong-client-id",
            "exp": 4102444800_u64,
        });

        let mut header = jsonwebtoken::Header::new(Algorithm::RS256);
        header.kid = Some("test-key-1".to_string());

        let token = jsonwebtoken::encode(&header, &claims, &encoding_key).unwrap();

        {
            let mut cache = JWKS_CACHE.lock();
            let mut keys = HashMap::new();
            keys.insert("test-key-1".to_string(), (decoding_key, Algorithm::RS256));
            *cache = Some(JwksCache {
                keys,
                fetched_at: std::time::Instant::now(),
                issuer: "https://test-idp.example.com".to_string(),
            });
        }

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&["https://test-idp.example.com"]);
        validation.set_audience(&["correct-client-id"]);

        let cached = get_cached_jwk("test-key-1", "https://test-idp.example.com").unwrap();
        let result = jsonwebtoken::decode::<serde_json::Value>(&token, &cached.0, &validation);
        assert!(result.is_err());
    }
}
