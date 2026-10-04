// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Silent licence renewal for Signal subscriptions.
//!
//! A subscription key carries its own expiry (monthly ~35 days, annual ~1 year
//! + 7 days) and verifies offline. Before this module, the only way a renewed
//! key reached the app was an email the customer had to click — a monthly
//! subscriber who ignored it fell back to Free while still being billed.
//!
//! Now, when the held key is within [`RENEW_WINDOW_DAYS`] of expiry (or expired
//! no more than [`MAX_LAPSE_DAYS`] ago), the app sends that key — and nothing
//! else — to `4da.ai/api/license/renew` (NETWORK.md §2k, Terms §4.3). The server
//! confirms the live subscription in Stripe and answers with a key valid to the
//! end of the paid period. Properties this module guarantees:
//!
//! - **Key-only, rarely.** No call at all for free users, lifetime keys or
//!   Keygen keys; for subscribers, at most every 12h and only near expiry —
//!   roughly one call per billing period.
//! - **Never downgrades.** Any network error, outage or refusal leaves the
//!   current key untouched; it simply runs to its own expiry.
//! - **Never accepts a worse key.** A returned key is installed only if it
//!   verifies against the embedded public key, names the same email and
//!   expires later than the key it replaces.

use std::time::Duration;

use chrono::{DateTime, Datelike, Utc};
use tracing::{info, warn};

use super::verify::{verify_license_key_at, LicensePayload};

const RENEW_URL: &str = "https://4da.ai/api/license/renew";

/// Start asking this many days before the held key expires.
pub(crate) const RENEW_WINDOW_DAYS: i64 = 10;
/// Keep asking for this long after expiry (an app that was offline over the
/// renewal date recovers on its own). Mirrors the server's MAX_LAPSE_DAYS.
pub(crate) const MAX_LAPSE_DAYS: i64 = 60;

const FIRST_CHECK_DELAY: Duration = Duration::from_mins(2);
const CHECK_INTERVAL: Duration = Duration::from_hours(12);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// What the renewal endpoint told us.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RenewOutcome {
    /// A newer key — still to be checked by [`accept_renewed_key`].
    Renewed(String),
    /// The key we hold is already the newest.
    Current,
    /// Lifetime keys never renew.
    Lifetime,
    /// No entitling subscription (cancelled, refunded, lapsed) or the server
    /// would not accept our key. The current key runs to its own expiry.
    NotEntitled(String),
    /// Network / server trouble. Try again next cycle; keep the current key.
    Retry(String),
}

/// Signature-valid payload of a `4DA-` key, ignoring its expiry (an expired
/// subscription key is still a valid renewal credential within the lapse window).
fn signature_valid_payload(key: &str) -> Option<LicensePayload> {
    verify_license_key_at(key, DateTime::<Utc>::UNIX_EPOCH).ok()
}

fn parse_expiry(payload: &LicensePayload) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&payload.expires_at)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// Is this key a subscription key that is due for renewal at `now`?
pub(crate) fn renewal_due(payload: &LicensePayload, now: DateTime<Utc>) -> bool {
    let Some(expiry) = parse_expiry(payload) else {
        return false;
    };
    if expiry.year() >= 2099 {
        return false; // lifetime
    }
    let remaining = expiry - now;
    remaining <= chrono::Duration::days(RENEW_WINDOW_DAYS)
        && remaining >= -chrono::Duration::days(MAX_LAPSE_DAYS)
}

/// Map the endpoint's HTTP status + JSON body to an outcome.
pub(crate) fn classify_response(status: u16, body: &str) -> RenewOutcome {
    let json: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
    let state = json.get("status").and_then(|v| v.as_str()).unwrap_or("");
    match (status, state) {
        (200, "renewed") => match json.get("license_key").and_then(|v| v.as_str()) {
            Some(key) if !key.is_empty() => RenewOutcome::Renewed(key.to_string()),
            _ => RenewOutcome::Retry("renewed response carried no key".to_string()),
        },
        (200, "current") => RenewOutcome::Current,
        (200, "lifetime") => RenewOutcome::Lifetime,
        (400 | 401 | 403, _) => {
            let reason = json
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or(if state.is_empty() { "forbidden" } else { state });
            RenewOutcome::NotEntitled(reason.to_string())
        }
        _ => RenewOutcome::Retry(format!("HTTP {status} {state}").trim().to_string()),
    }
}

/// Decide whether a key the server returned may replace the one we hold.
/// `verify` checks signature AND expiry (production: `verify_license_key`).
pub(crate) fn accept_renewed_key(
    current: &LicensePayload,
    new_key: &str,
    verify: impl Fn(&str) -> crate::error::Result<LicensePayload>,
) -> Result<LicensePayload, String> {
    let renewed = verify(new_key).map_err(|e| format!("renewed key failed verification: {e}"))?;
    if !renewed.email.eq_ignore_ascii_case(&current.email) {
        return Err("renewed key names a different account".to_string());
    }
    match (parse_expiry(&renewed), parse_expiry(current)) {
        (Some(new_exp), Some(old_exp)) if new_exp > old_exp => Ok(renewed),
        (Some(_), None) => Ok(renewed),
        _ => Err("renewed key does not extend access".to_string()),
    }
}

/// The `4DA-` key this install holds (settings first, then keychain), with its
/// signature-verified payload. `None` for free / Keygen / unreadable keys.
fn held_subscription_key() -> Option<(String, LicensePayload)> {
    let in_settings = {
        let manager = crate::get_settings_manager();
        let guard = manager.lock();
        guard.get().license.license_key.clone()
    };
    let key = if in_settings.is_empty() {
        crate::settings::keystore::get_secret("license_key")
            .ok()
            .flatten()?
    } else {
        in_settings
    };
    if !key.starts_with("4DA-") {
        return None;
    }
    let payload = signature_valid_payload(&key)?;
    Some((key, payload))
}

/// Persist a renewed key exactly where activation does: keychain, settings,
/// backup. Restores the paid tier if startup validation had already dropped it
/// because the old key expired while the app was offline.
fn install_renewed_key(key: &str, payload: &LicensePayload) {
    let tier = match payload.tier.as_str() {
        "pro" | "community" | "cohort" => "signal".to_string(),
        other => other.to_string(),
    };
    if let Err(e) = crate::settings::keystore::store_secret("license_key", key) {
        warn!(target: "4da::license", error = %e, "Keychain write failed for renewed key; settings.json keeps it");
    }
    let manager = crate::get_settings_manager();
    let mut guard = manager.lock();
    let activated_at = guard
        .get()
        .license
        .activated_at
        .clone()
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    {
        let settings = guard.get_mut();
        settings.license.license_key = key.to_string();
        settings.license.tier = tier.clone();
        settings.license.activated_at = Some(activated_at.clone());
    }
    if let Err(e) = guard.save() {
        warn!(target: "4da::license", error = %e, "Failed to persist renewed licence key");
    }
    drop(guard);
    super::save_license_backup(key, &tier, &activated_at);
}

async fn post_renewal(key: &str) -> RenewOutcome {
    let response = crate::http_client::HTTP_CLIENT
        .post(RENEW_URL)
        .json(&serde_json::json!({ "key": key }))
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await;
    match response {
        Ok(resp) => {
            let status = resp.status().as_u16();
            match resp.text().await {
                Ok(body) => classify_response(status, &body),
                Err(e) => RenewOutcome::Retry(format!("could not read response: {e}")),
            }
        }
        Err(e) => RenewOutcome::Retry(format!("network: {e}")),
    }
}

/// One renewal pass. Returns `None` when no call was made (nothing due).
pub(crate) async fn renew_once() -> Option<RenewOutcome> {
    let (key, payload) = held_subscription_key()?;
    if !renewal_due(&payload, super::license_effective_now()) {
        return None;
    }
    let outcome = post_renewal(&key).await;
    match &outcome {
        RenewOutcome::Renewed(new_key) => {
            match accept_renewed_key(&payload, new_key, super::verify_license_key) {
                Ok(renewed) => {
                    install_renewed_key(new_key, &renewed);
                    info!(target: "4da::license", expires_at = %renewed.expires_at, "Licence renewed silently");
                }
                Err(e) => {
                    warn!(target: "4da::license", reason = %e, "Renewed licence key rejected; keeping current key")
                }
            }
        }
        RenewOutcome::Current | RenewOutcome::Lifetime => {}
        RenewOutcome::NotEntitled(reason) => info!(
            target: "4da::license",
            reason = %reason,
            "Licence not renewed; the current key stays valid until its own expiry"
        ),
        RenewOutcome::Retry(reason) => {
            warn!(target: "4da::license", reason = %reason, "Licence renewal deferred; will retry")
        }
    }
    Some(outcome)
}

/// Start the background renewal loop: first check shortly after launch, then
/// every 12 hours for as long as the app runs.
pub fn spawn_license_renewal_task() {
    tauri::async_runtime::spawn(async {
        tokio::time::sleep(FIRST_CHECK_DELAY).await;
        loop {
            let _ = renew_once().await;
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(email: &str, expires_at: &str) -> LicensePayload {
        LicensePayload {
            tier: "signal".to_string(),
            email: email.to_string(),
            expires_at: expires_at.to_string(),
            issued_at: "2026-09-01T00:00:00Z".to_string(),
            features: vec!["signal".to_string()],
        }
    }

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_default()
    }

    #[test]
    fn due_only_inside_the_window_and_lapse_bound() {
        let now = at("2026-10-04T00:00:00Z");
        assert!(
            !renewal_due(&payload("a@b.co", "2026-10-20T00:00:00Z"), now),
            "16 days left: not yet"
        );
        assert!(
            renewal_due(&payload("a@b.co", "2026-10-13T00:00:00Z"), now),
            "9 days left: due"
        );
        assert!(
            renewal_due(&payload("a@b.co", "2026-09-20T00:00:00Z"), now),
            "expired 14 days: still recoverable"
        );
        assert!(
            !renewal_due(&payload("a@b.co", "2026-07-01T00:00:00Z"), now),
            "expired 95 days: lapsed"
        );
        assert!(!renewal_due(&payload("a@b.co", "garbage"), now));
    }

    #[test]
    fn lifetime_keys_never_call_out() {
        let now = at("2099-10-01T00:00:00Z");
        assert!(!renewal_due(
            &payload("a@b.co", "2099-10-04T13:32:22.574Z"),
            now
        ));
    }

    #[test]
    fn classifies_every_server_answer() {
        assert_eq!(
            classify_response(
                200,
                r#"{"status":"renewed","license_key":"4DA-x.y","expires_at":"z"}"#
            ),
            RenewOutcome::Renewed("4DA-x.y".to_string())
        );
        assert_eq!(
            classify_response(200, r#"{"status":"current"}"#),
            RenewOutcome::Current
        );
        assert_eq!(
            classify_response(200, r#"{"status":"lifetime"}"#),
            RenewOutcome::Lifetime
        );
        assert_eq!(
            classify_response(403, r#"{"status":"not_entitled"}"#),
            RenewOutcome::NotEntitled("not_entitled".to_string())
        );
        assert_eq!(
            classify_response(401, r#"{"status":"error","reason":"invalid_signature"}"#),
            RenewOutcome::NotEntitled("invalid_signature".to_string())
        );
        assert!(matches!(
            classify_response(503, r#"{"retryable":true}"#),
            RenewOutcome::Retry(_)
        ));
        assert!(matches!(
            classify_response(429, "{}"),
            RenewOutcome::Retry(_)
        ));
        assert!(matches!(
            classify_response(200, "not json"),
            RenewOutcome::Retry(_)
        ));
        assert!(matches!(
            classify_response(200, r#"{"status":"renewed"}"#),
            RenewOutcome::Retry(_)
        ));
    }

    #[test]
    fn accepts_only_a_later_key_for_the_same_account() {
        let current = payload("Buyer@Example.com", "2026-11-08T00:00:00Z");
        let later = |_: &str| Ok(payload("buyer@example.com", "2026-12-15T00:00:00Z"));
        assert!(
            accept_renewed_key(&current, "k", later).is_ok(),
            "same account (case-insensitive), later expiry"
        );

        let other = |_: &str| Ok(payload("someone@else.com", "2027-12-15T00:00:00Z"));
        assert!(
            accept_renewed_key(&current, "k", other).is_err(),
            "different account refused"
        );

        let older = |_: &str| Ok(payload("buyer@example.com", "2026-11-01T00:00:00Z"));
        assert!(
            accept_renewed_key(&current, "k", older).is_err(),
            "shorter key refused"
        );

        let forged = |_: &str| -> crate::error::Result<LicensePayload> {
            Err("signature verification failed".into())
        };
        assert!(
            accept_renewed_key(&current, "k", forged).is_err(),
            "unverifiable key refused"
        );
    }
}
