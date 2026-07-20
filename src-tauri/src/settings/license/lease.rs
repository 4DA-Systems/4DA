// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Lease-model license client.
//!
//! The user activates once with a stable `4DA-LIC-...` refresh credential. This
//! module exchanges it at `/api/license/refresh` for a short-lived, signed
//! `4DA-...` entitlement token (stored in `license.license_key`, verified OFFLINE
//! by [`super::verify::verify_license_key`]). A background task re-runs the
//! exchange on startup and on a timer, so:
//!   - revocation (a cancel/refund reflected LIVE in Stripe) propagates within the
//!     refresh interval for online users, and
//!   - offline users keep working until the current token's embedded expiry.
//!
//! Design invariant: a network or server error NEVER downgrades a paying user —
//! it returns [`RefreshOutcome::KeepCurrent`]. Only a definitive, authenticated
//! "not entitled" from the server downgrades.

use std::sync::atomic::Ordering;
use tracing::{info, warn};

use super::TIER_DOWNGRADED;

const REFRESH_URL: &str = "https://4da.ai/api/license/refresh";

/// Keychain secret name for the durable refresh credential (survives a
/// settings.json license-block wipe — the recurring "Signal dropped to Free" bug).
const REFRESH_KEY_SECRET: &str = "refresh_key";

/// A refresh credential is `4DA-LIC-<base32>` — distinct from a signed entitlement
/// token (`4DA-<b64>.<b64>`, contains a `.`) and from a Keygen key.
pub fn is_refresh_credential(key: &str) -> bool {
    key.starts_with("4DA-LIC-") && !key.contains('.')
}

/// Result of a refresh attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshOutcome {
    /// A fresh entitlement token was minted.
    Renewed {
        token: String,
        tier: String,
        expires_at: String,
    },
    /// The credential is authoritatively NOT entitled (cancelled / refunded /
    /// unknown) — downgrade to free.
    Revoked { reason: String },
    /// Transient (network / server / malformed) — keep the current token, retry later.
    KeepCurrent { reason: String },
}

/// Pure classifier for a refresh response — unit-testable without the network.
/// `server_error` is true for HTTP 5xx (never a revocation).
fn classify_response(server_error: bool, json: &serde_json::Value) -> RefreshOutcome {
    let valid = json
        .get("valid")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if valid {
        let token = json
            .get("token")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let tier = json
            .get("tier")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let expires_at = json
            .get("expires_at")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        // A "valid" response that is missing the token/tier is malformed — treat as
        // transient, never as an entitlement and never as a revocation.
        if token.starts_with("4DA-") && token.contains('.') && !tier.is_empty() {
            return RefreshOutcome::Renewed {
                token,
                tier,
                expires_at,
            };
        }
        return RefreshOutcome::KeepCurrent {
            reason: "malformed_valid_response".into(),
        };
    }

    let retryable = json
        .get("retryable")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let reason = json
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("denied")
        .to_string();
    // 5xx or an explicit retryable flag => transient, keep the current token.
    if server_error || retryable {
        RefreshOutcome::KeepCurrent { reason }
    } else {
        RefreshOutcome::Revoked { reason }
    }
}

/// Exchange a refresh credential for a fresh entitlement token.
pub async fn refresh_entitlement(refresh_key: &str) -> RefreshOutcome {
    let body = serde_json::json!({ "key": refresh_key });
    let resp = crate::http_client::HTTP_CLIENT
        .post(REFRESH_URL)
        .json(&body)
        .send()
        .await;

    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::license", error = %e, "Lease refresh unreachable — keeping current token");
            return RefreshOutcome::KeepCurrent {
                reason: format!("network: {e}"),
            };
        }
    };
    let server_error = resp.status().is_server_error();
    let json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return RefreshOutcome::KeepCurrent {
                reason: format!("bad_response: {e}"),
            }
        }
    };
    classify_response(server_error, &json)
}

/// Read the durable refresh credential: settings first, then keychain (which
/// survives a settings.json license wipe). Returns the key and whether it needed
/// rehydration from the keychain.
fn load_refresh_credential() -> Option<(String, bool)> {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    if let Some(k) = guard.get().license.refresh_key.as_ref() {
        if !k.is_empty() {
            return Some((k.clone(), false));
        }
    }
    drop(guard);
    // Fall back to the keychain copy stored at activation.
    match crate::settings::keystore::get_secret(REFRESH_KEY_SECRET) {
        Ok(Some(k)) if !k.is_empty() => Some((k, true)),
        _ => None,
    }
}

/// Persist a freshly-minted entitlement token + tier. Slides `activated_at`
/// forward so the activation grace window always covers an actively-refreshing
/// user. Optionally rehydrates the refresh credential into settings + keychain.
fn apply_renewed(token: &str, tier: &str, refresh_key: &str, rehydrate: bool) {
    let activated_at = chrono::Utc::now().to_rfc3339();
    {
        let manager = crate::get_settings_manager();
        let mut guard = manager.lock();
        let s = guard.get_mut();
        s.license.license_key = token.to_string();
        s.license.tier = tier.to_string();
        s.license.activated_at = Some(activated_at.clone());
        s.license.trial_started_at = None;
        if rehydrate {
            s.license.refresh_key = Some(refresh_key.to_string());
        }
        if let Err(e) = guard.save() {
            warn!(target: "4da::license", error = %e, "Failed to persist renewed lease token");
        }
    }
    if rehydrate {
        let _ = crate::settings::keystore::store_secret(REFRESH_KEY_SECRET, refresh_key);
    }
    crate::settings::save_license_backup(token, tier, &activated_at);
    info!(target: "4da::license", tier = %tier, "Lease renewed");
}

/// Downgrade to free after an authoritative revocation. Keeps the refresh
/// credential so a re-subscribe on the same account re-ups automatically.
fn apply_revoked(reason: &str) {
    let manager = crate::get_settings_manager();
    let mut guard = manager.lock();
    let cur = guard.get().license.tier.clone();
    if cur != "free" {
        let s = guard.get_mut();
        s.license.tier = "free".to_string();
        s.license.license_key = String::new();
        if let Err(e) = guard.save() {
            warn!(target: "4da::license", error = %e, "Failed to persist lease revoke");
        }
        TIER_DOWNGRADED.store(true, Ordering::Relaxed);
        warn!(target: "4da::license", reason = %reason, "Lease revoked — downgraded to free");
    }
}

/// Refresh the lease if this install holds a refresh credential. Safe to call on
/// startup and on a periodic timer; a no-op for non-lease licenses.
pub async fn maybe_refresh_lease() {
    let Some((refresh_key, rehydrate)) = load_refresh_credential() else {
        return; // not a lease license
    };

    match refresh_entitlement(&refresh_key).await {
        RefreshOutcome::Renewed {
            token,
            tier,
            expires_at,
        } => {
            let _ = expires_at; // embedded in the token; kept for logging/telemetry
            apply_renewed(&token, &tier, &refresh_key, rehydrate);
        }
        RefreshOutcome::Revoked { reason } => apply_revoked(&reason),
        RefreshOutcome::KeepCurrent { reason } => {
            info!(target: "4da::license", reason = %reason, "Lease refresh deferred — keeping current token");
        }
    }
}

/// Store the refresh credential in the keychain (called at activation).
pub fn store_refresh_credential(key: &str) {
    let _ = crate::settings::keystore::store_secret(REFRESH_KEY_SECRET, key);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_refresh_credentials() {
        assert!(is_refresh_credential("4DA-LIC-AAAABBBBCCCCDDDD"));
        // A signed entitlement token is NOT a refresh credential.
        assert!(!is_refresh_credential("4DA-eyJ0aWVy.c2ln"));
        // A legacy signed key / keygen key are not refresh credentials.
        assert!(!is_refresh_credential("4DA-payload.sig"));
        assert!(!is_refresh_credential("BE3529-741BAF"));
        assert!(!is_refresh_credential(""));
    }

    #[test]
    fn valid_response_renews() {
        let r = classify_response(
            false,
            &json!({ "valid": true, "token": "4DA-abc.def", "tier": "signal", "expires_at": "2026-08-20T00:00:00Z" }),
        );
        assert_eq!(
            r,
            RefreshOutcome::Renewed {
                token: "4DA-abc.def".into(),
                tier: "signal".into(),
                expires_at: "2026-08-20T00:00:00Z".into()
            }
        );
    }

    #[test]
    fn valid_but_malformed_token_keeps_current_not_renew() {
        // "valid" but the token isn't a signed 4DA-...  token => never trust it.
        let r = classify_response(
            false,
            &json!({ "valid": true, "token": "garbage", "tier": "signal" }),
        );
        assert!(matches!(r, RefreshOutcome::KeepCurrent { .. }));
    }

    #[test]
    fn definitive_denial_revokes() {
        let r = classify_response(
            false,
            &json!({ "valid": false, "reason": "no_active_entitlement" }),
        );
        assert_eq!(
            r,
            RefreshOutcome::Revoked {
                reason: "no_active_entitlement".into()
            }
        );
        let r2 = classify_response(false, &json!({ "valid": false, "reason": "not_found" }));
        assert_eq!(
            r2,
            RefreshOutcome::Revoked {
                reason: "not_found".into()
            }
        );
    }

    #[test]
    fn server_error_never_revokes() {
        // A 5xx must NOT lock out a paying user, even with valid:false.
        let r = classify_response(
            true,
            &json!({ "valid": false, "reason": "temporary_error" }),
        );
        assert!(matches!(r, RefreshOutcome::KeepCurrent { .. }));
    }

    #[test]
    fn explicit_retryable_keeps_current() {
        let r = classify_response(
            false,
            &json!({ "valid": false, "reason": "temporary_error", "retryable": true }),
        );
        assert!(matches!(r, RefreshOutcome::KeepCurrent { .. }));
    }
}
