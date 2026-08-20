// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! License and trial Tauri commands.

use std::sync::Mutex;

use tracing::{info, warn};

use crate::error::Result;

use crate::get_settings_manager;

use super::validate_input_length;

/// A deep link that launched the app COLD (`fourda://activate?key=...` in
/// argv). The `deep-link://new-url` event only covers links arriving while the
/// app runs; a launch URL exists before any listener attaches, so app_setup
/// parks it here and the frontend collects it once its listeners are mounted.
static PENDING_DEEP_LINK: Mutex<Option<String>> = Mutex::new(None);

/// Called from app_setup with an ALREADY-VALIDATED URL (validate_deep_link_url).
pub(crate) fn set_pending_deep_link(url: String) {
    if let Ok(mut slot) = PENDING_DEEP_LINK.lock() {
        *slot = Some(url);
    }
}

/// One-shot: the frontend calls this on mount to collect a launch deep link.
/// Consuming (take) so a webview reload cannot re-activate the same link.
#[tauri::command]
pub async fn take_pending_deep_link() -> Result<Option<String>> {
    Ok(PENDING_DEEP_LINK
        .lock()
        .ok()
        .and_then(|mut slot| slot.take()))
}

/// Get current license tier and feature availability
#[tauri::command]
pub async fn get_license_tier() -> Result<serde_json::Value> {
    let license = {
        let manager = get_settings_manager();
        let guard = manager.lock();
        guard.get().license.clone()
    };

    let dev_unlock = cfg!(debug_assertions) && license.dev_unlock_all;

    // Extract expiry from license key payload if present.
    // Self-signed keys (4DA-...) embed expiry in the payload.
    // Keygen keys (BE3529-...) don't — trust the stored tier and use cached validation.
    let (expires_at, days_remaining, expired) = if license.license_key.is_empty() {
        (None, 0, false)
    } else if license.license_key.starts_with("4DA-") {
        match crate::settings::verify_license_key(&license.license_key) {
            Ok(payload) => {
                if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&payload.expires_at) {
                    let now: chrono::DateTime<chrono::Utc> = chrono::Utc::now();
                    let days = (exp.with_timezone(&chrono::Utc) - now).num_days();
                    (Some(payload.expires_at), days.max(0) as i32, days < 0)
                } else {
                    (Some(payload.expires_at), 0, false)
                }
            }
            Err(_) if dev_unlock => (None, 365, false),
            Err(_) => (None, 0, true),
        }
    } else {
        (None, 0, false)
    };

    // One-shot flag: true if tier was downgraded since last check
    let was_downgraded = crate::settings::take_downgrade_flag();

    let last_validated_at = crate::settings::get_last_validated_at();

    Ok(serde_json::json!({
        "tier": license.tier,
        "activated_at": license.activated_at,
        "has_key": !license.license_key.is_empty(),
        "signal_features": crate::settings::SIGNAL_FEATURES,
        "expires_at": expires_at,
        "days_remaining": days_remaining,
        "expired": expired,
        "was_downgraded": was_downgraded,
        "last_validated_at": last_validated_at,
    }))
}

/// Should a DEEP-LINK activation be refused because it would replace an existing,
/// valid licence belonging to a DIFFERENT account?
///
/// A `fourda://activate?key=...` link is remotely triggerable — any web page the
/// user visits can fire it — so it must never silently swap a paying user's
/// licence for another. Returns true (refuse) only when a valid, non-expired
/// `4DA-` licence is already present AND the incoming key does not carry the SAME
/// account's email. First activation (no valid current key), a same-account
/// renewal (matching email), and manual paste (which never sets `from_deep_link`)
/// are all allowed unchanged.
///
/// `incoming_email` is the email embedded in the key being activated — `Some` for
/// a `4DA-` key, `None` for a Keygen key (which carries no local email and so can
/// never prove it is the same account).
fn deep_link_replacement_blocked(current_key: &str, incoming_email: Option<&str>) -> bool {
    // Only a live, valid 4DA- licence is worth protecting. An absent, invalid or
    // expired current key is not something a deep link needs consent to replace.
    let current = match crate::settings::verify_license_key(current_key) {
        Ok(payload) => payload,
        Err(_) => return false,
    };
    match incoming_email {
        Some(e) if e.eq_ignore_ascii_case(&current.email) => false, // same account — renewal
        _ => true, // different email, or a Keygen key with no email — refuse the swap
    }
}

/// Activate a license key — tries Keygen API first, falls back to ed25519 self-signed.
///
/// `from_deep_link` is set ONLY by the `fourda://activate` handler; manual paste in
/// Settings leaves it None. When true, `deep_link_replacement_blocked` guards
/// against a website silently replacing a valid licence for a different account.
#[tauri::command]
pub async fn activate_license(
    license_key: String,
    from_deep_link: Option<bool>,
) -> Result<serde_json::Value> {
    crate::settings::check_activation_rate_limit()?;
    // Strip whitespace — users copying keys from emails often get line breaks injected
    let license_key: String = license_key.chars().filter(|c| !c.is_whitespace()).collect();
    if license_key.is_empty() {
        return Err("License key cannot be empty".into());
    }

    // Strategy: try Keygen API validation first (for Keygen-format keys like BE3529-...),
    // then fall back to local ed25519 verification (for self-signed 4DA- keys).
    let effective_tier: String;
    let email: Option<String>;
    let expires_at: Option<String>;

    if license_key.starts_with("4DA-") {
        // Self-signed ed25519 key
        let payload = crate::settings::verify_license_key(&license_key)?;
        effective_tier = match payload.tier.as_str() {
            "signal" | "team" | "enterprise" => payload.tier.clone(),
            // Legacy: "pro", "community", "cohort" all map to "signal"
            "pro" | "community" | "cohort" => "signal".to_string(),
            _ => payload.tier.clone(),
        };
        email = Some(payload.email);
        expires_at = Some(payload.expires_at);
    } else {
        // Keygen API key (e.g., BE3529-741BAF-...)
        // Char-based prefix: `[..6.min(len)]` clamps length but not the char
        // boundary, so a mis-pasted key with multi-byte UTF-8 in its first
        // 6 bytes panicked this command.
        let key_prefix: String = license_key.chars().take(6).collect();
        info!(target: "4da::license", "Validating Keygen key (format: {key_prefix}...)");
        let result = crate::settings::validate_license_key_keygen_fresh(&license_key, "free").await;
        info!(target: "4da::license", tier = %result.tier, online = result.online, cached = result.cached, code = %result.code, detail = %result.detail, "Keygen validation result");

        if result.tier == "free" {
            return Ok(serde_json::json!({
                "success": false,
                "reason": result.detail,
            }));
        }
        effective_tier = result.tier;
        email = None;
        expires_at = None;
    }

    // Deep-link consent guard (see deep_link_replacement_blocked). A remotely
    // triggerable link must not silently replace a valid licence for a different
    // account. Read the current key in its own short-lived lock, released before
    // the write below.
    if from_deep_link.unwrap_or(false) {
        let current_key = {
            let manager = get_settings_manager();
            let guard = manager.lock();
            guard.get().license.license_key.clone()
        };
        if deep_link_replacement_blocked(&current_key, email.as_deref()) {
            warn!(
                target: "4da::license",
                "Deep-link activation refused — it would replace a valid licence for a different account"
            );
            return Ok(serde_json::json!({
                "success": false,
                "reason": "different_account",
                "detail": "This link is for a different 4DA account. To switch licences, open Settings \u{2192} License and paste the key.",
            }));
        }
    }

    let manager = get_settings_manager();
    let mut guard = manager.lock();

    if !license_key.is_empty() {
        let _ = crate::settings::keystore::store_secret("license_key", &license_key);
        if !crate::settings::keystore::has_secret("license_key") {
            warn!(
                target: "4da::license",
                "Keychain write appeared to succeed but key not found on read-back. \
                 License key will be persisted to settings.json as fallback."
            );
        }
    }
    let activated_at = chrono::Utc::now().to_rfc3339();
    {
        let settings = guard.get_mut();
        settings.license.license_key = license_key.clone();
        settings.license.tier = effective_tier.clone();
        settings.license.activated_at = Some(activated_at.clone());
        settings.license.trial_started_at = None;
    }
    guard.save()?;

    crate::settings::save_license_backup(&license_key, &effective_tier, &activated_at);

    info!(target: "4da::license", "License activated — tier: {}", effective_tier);
    crate::settings::clear_activation_rate_limit();

    // Audit: license activated (fire-and-forget, only logs if team relay is configured)
    if let Ok(conn) = crate::state::open_db_connection() {
        crate::audit::log_team_audit(
            &conn,
            "license.activated",
            "license",
            None,
            Some(&serde_json::json!({ "tier": effective_tier })),
        );
    }

    Ok(serde_json::json!({
        "success": true,
        "tier": effective_tier,
        "email": email,
        "expires_at": expires_at,
    }))
}

/// Get trial status
#[tauri::command]
pub async fn get_trial_status() -> Result<serde_json::Value> {
    let license = {
        let manager = get_settings_manager();
        let guard = manager.lock();
        guard.get().license.clone()
    };
    let status = crate::settings::get_trial_status(&license);

    Ok(serde_json::json!({
        "active": status.active,
        "days_remaining": status.days_remaining,
        "started_at": status.started_at,
        "has_license": status.has_license,
    }))
}

/// Start a free trial
#[tauri::command]
pub async fn start_trial() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();
    let settings = guard.get_mut();

    if !settings.license.license_key.is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "reason": "Already have a license key",
        }));
    }

    if settings.license.trial_started_at.is_some() {
        let status = crate::settings::get_trial_status(&settings.license);
        return Ok(serde_json::json!({
            "success": false,
            "reason": "Trial already started",
            "active": status.active,
            "days_remaining": status.days_remaining,
        }));
    }

    settings.license.trial_started_at = Some(chrono::Utc::now().to_rfc3339());
    guard.save()?;

    info!(target: "4da::license", "Free trial started");

    // Report the ACTUAL trial length, computed from the same source gating
    // enforces (get_trial_status -> TRIAL_DURATION_DAYS). This used to hardcode
    // 45 while gating.rs expires the trial at 14 days — a response that promised
    // three times the access the app actually grants.
    let status = crate::settings::get_trial_status(&guard.get().license);
    Ok(serde_json::json!({
        "success": true,
        "days_remaining": status.days_remaining,
    }))
}

/// Validate the current license key.
/// Self-signed 4DA- keys are verified locally (ed25519 signature).
/// Keygen API keys are validated online.
/// Returns the validation result and updates the tier in settings if needed.
#[tauri::command]
pub async fn validate_license() -> Result<serde_json::Value> {
    // Read current license info (release lock before async work)
    let (license_key, current_tier) = {
        let manager = get_settings_manager();
        let guard = manager.lock();
        let license = &guard.get().license;
        (license.license_key.clone(), license.tier.clone())
    };

    if license_key.is_empty() {
        return Ok(serde_json::json!({
            "validated": false,
            "tier": "free",
            "detail": "No license key configured",
        }));
    }

    // Route by key format: self-signed keys are verified locally,
    // Keygen keys are validated via the Keygen API.
    if license_key.starts_with("4DA-") {
        // Self-signed ed25519 key — verify locally, NEVER send to Keygen
        match crate::settings::verify_license_key(&license_key) {
            Ok(payload) => {
                let effective_tier = match payload.tier.as_str() {
                    "signal" | "team" | "enterprise" => payload.tier.clone(),
                    "pro" | "community" | "cohort" => "signal".to_string(),
                    _ => payload.tier.clone(),
                };

                // Check if key has expired
                let expired =
                    if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&payload.expires_at) {
                        exp.with_timezone(&chrono::Utc) < chrono::Utc::now()
                    } else {
                        false
                    };

                if expired {
                    // Key expired — downgrade
                    if current_tier != "free" {
                        let manager = get_settings_manager();
                        let mut guard = manager.lock();
                        guard.get_mut().license.tier = "free".to_string();
                        if let Err(e) = guard.save() {
                            warn!("Failed to save settings after expired license: {e}");
                        }
                    }
                    return Ok(serde_json::json!({
                        "validated": false,
                        "tier": "free",
                        "cached": false,
                        "detail": "License key has expired",
                    }));
                }

                // Valid key — ensure tier is correct
                if effective_tier != current_tier {
                    let manager = get_settings_manager();
                    let mut guard = manager.lock();
                    info!(target: "4da::license", old_tier = %current_tier, new_tier = %effective_tier, "Tier corrected after local validation");
                    guard.get_mut().license.tier = effective_tier.clone();
                    if let Err(e) = guard.save() {
                        warn!("Failed to save settings after license validation: {e}");
                    }
                }

                Ok(serde_json::json!({
                    "validated": true,
                    "tier": effective_tier,
                    "cached": false,
                    "detail": "Valid (local signature verified)",
                }))
            }
            Err(e) => {
                warn!(target: "4da::license", error = %e, "Self-signed license key verification failed");
                Ok(serde_json::json!({
                    "validated": false,
                    "tier": current_tier, // Don't downgrade on verification error — preserve existing tier
                    "cached": false,
                    "detail": format!("Verification failed: {e}"),
                }))
            }
        }
    } else {
        // Keygen API key — validate online
        let result =
            crate::settings::validate_license_key_keygen(&license_key, &current_tier).await;

        // Update tier in settings if it changed
        if result.tier != current_tier {
            let manager = get_settings_manager();
            let mut guard = manager.lock();
            let settings = guard.get_mut();
            info!(target: "4da::license", old_tier = %current_tier, new_tier = %result.tier, "Tier updated after Keygen validation");
            settings.license.tier = result.tier.clone();
            if let Err(e) = guard.save() {
                warn!("Failed to save settings after license update: {e}");
            }
        }

        Ok(serde_json::json!({
            "validated": result.online || result.cached,
            "tier": result.tier,
            "cached": result.cached,
            "detail": result.detail,
        }))
    }
}

/// Recover a license key by purchase email.
///
/// The server **never returns the key to this call** and therefore never
/// auto-activates. Because the email is caller-supplied and unverifiable, the
/// endpoint mails the key to the address on file and answers `202 Accepted`
/// identically whether or not that address holds a licence — otherwise anyone
/// could retrieve any customer's offline-verifiable key just by knowing their
/// email address (fixed 2026-08-14, see site/functions/api/streets/activate.js).
///
/// So the success outcome here is `reason: "emailed"`, not an activation: the
/// user completes recovery by opening the email and using the key (or its
/// `fourda://activate` deep link). This command NEVER auto-activates a key handed
/// back over HTTP — an older server returned the key in a 200 body and this used
/// to store it into settings/keychain/backup with no signature or online check,
/// which a user who repointed `4da.ai` at their own server could abuse to grant
/// any tier. A 200 is now treated as "check your email" and nothing is stored;
/// activation only ever happens through `activate_license`, which verifies.
#[tauri::command]
pub async fn recover_license_by_email(email: String) -> Result<serde_json::Value> {
    validate_input_length(&email, "Email", 254)?;
    crate::settings::check_activation_rate_limit()?;

    let response = crate::http_client::HTTP_CLIENT
        .get("https://4da.ai/api/streets/activate")
        .query(&[("email", email.as_str())])
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = match resp.json().await {
                Ok(v) => v,
                Err(e) => {
                    warn!(target: "4da::license", error = %e, "Failed to parse recovery response JSON");
                    return Ok(
                        serde_json::json!({ "success": false, "reason": "network_error", "detail": "Invalid response from server" }),
                    );
                }
            };

            match status {
                // The live server never returns a key to this call — recovery
                // MAILS the key and answers 202. This arm existed for an older
                // server that returned the key in the body, and it auto-activated
                // it into settings + keychain + backup WITHOUT any signature or
                // online validation (unlike `activate_license`). That is a real
                // hole: a user who repoints 4da.ai at a server they control (hosts
                // file + a self-installed root CA) could hand themselves any tier.
                // We no longer trust a key handed back by this endpoint. If a
                // legacy server ever returns 200, treat it as "check your email"
                // and let the user activate through the verified path.
                200 => Ok(serde_json::json!({
                    "success": false,
                    "reason": "emailed",
                    "detail": "Check your email for your licence key, then activate it in Settings.",
                })),
                // The normal, secure outcome: the server accepted the request and
                // mailed the key to the address on file. It deliberately tells us
                // nothing about whether that address is a customer, so we must not
                // infer or display anything beyond "check your email".
                202 => Ok(serde_json::json!({
                    "success": false,
                    "reason": "emailed",
                    "detail": body["message"].as_str().unwrap_or(""),
                })),
                400 => Ok(serde_json::json!({
                    "success": false,
                    "reason": body["reason"].as_str().unwrap_or("invalid_email"),
                })),
                404 => Ok(serde_json::json!({ "success": false, "reason": "not_found" })),
                410 => Ok(serde_json::json!({
                    "success": false,
                    "reason": "expired",
                    "detail": body["expired_at"].as_str().unwrap_or(""),
                })),
                // Recovery mail not provisioned server-side — an honest, actionable
                // failure rather than silently pretending an email was sent.
                503 => Ok(serde_json::json!({
                    "success": false,
                    "reason": body["reason"].as_str().unwrap_or("recovery_email_unavailable"),
                    "detail": body["error"].as_str().unwrap_or(""),
                })),
                _ => Ok(serde_json::json!({
                    "success": false,
                    "reason": "network_error",
                    "detail": format!("Unexpected status: {}", status),
                })),
            }
        }
        Err(e) => {
            warn!(target: "4da::license", error = %e, "License recovery API unreachable");
            Ok(serde_json::json!({
                "success": false,
                "reason": "network_error",
                "detail": format!("Network error: {}", e),
            }))
        }
    }
}

#[cfg(test)]
mod deep_link_guard_tests {
    use super::deep_link_replacement_blocked;

    // A real signed 4DA- key (same fixture as license_tests.rs). Embedded email:
    // e2e-test-1771748424165@4da.test; expires 2027-02-22 (unexpired), so it
    // verifies — a genuinely valid current licence worth protecting.
    const VALID_KEY: &str = "4DA-eyJ0aWVyIjoiY29tbXVuaXR5IiwiZW1haWwiOiJlMmUtdGVzdC0xNzcxNzQ4NDI0MTY1QDRkYS50ZXN0IiwiZXhwaXJlc19hdCI6IjIwMjctMDItMjJUMDg6MjA6MjQuNzM5WiIsImlzc3VlZF9hdCI6IjIwMjYtMDItMjJUMDg6MjA6MjQuNzM5WiIsImZlYXR1cmVzIjpbInN0cmVldHNfY29tbXVuaXR5Il19.1T/4tSaET1tC1z/fuEEGwecSqBd8fIrplHdFxnUW9J0ZIOfWRmKhnJvTIt1i+Q7U3+OkrLBpwl4f8hngo0t6Bg==";
    const OWNER_EMAIL: &str = "e2e-test-1771748424165@4da.test";

    #[test]
    fn first_activation_is_never_blocked() {
        // No current key — the common legitimate case (buyer clicks the email link).
        assert!(!deep_link_replacement_blocked(
            "",
            Some("anyone@example.com")
        ));
    }

    #[test]
    fn an_invalid_or_expired_current_key_is_not_protected() {
        // Nothing valid to protect → a deep link may replace it.
        assert!(!deep_link_replacement_blocked(
            "4DA-bad.bad",
            Some("x@y.co")
        ));
        assert!(!deep_link_replacement_blocked(
            "BE3529-KEYGEN",
            Some("x@y.co")
        ));
    }

    #[test]
    fn same_account_renewal_is_allowed() {
        // A renewal link carries a fresh key for the SAME email — must replace freely.
        assert!(!deep_link_replacement_blocked(VALID_KEY, Some(OWNER_EMAIL)));
        assert!(!deep_link_replacement_blocked(
            VALID_KEY,
            Some(&OWNER_EMAIL.to_uppercase())
        ));
    }

    #[test]
    fn a_different_account_key_is_refused() {
        // THE ATTACK: a website fires fourda://activate?key=<attacker's own valid
        // key>. Different email → refuse the silent swap.
        assert!(deep_link_replacement_blocked(
            VALID_KEY,
            Some("attacker@evil.com")
        ));
    }

    #[test]
    fn a_keygen_key_cannot_replace_a_valid_4da_licence_via_deep_link() {
        // No embedded email to match → cannot prove same account → refuse.
        assert!(deep_link_replacement_blocked(VALID_KEY, None));
    }
}
