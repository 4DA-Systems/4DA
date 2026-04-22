// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! License, trial, context engine, and STREETS membership commands.

use tracing::{debug, info, warn};

use crate::context_engine::{InteractionType, InterestSource};
use crate::error::Result;
use tauri::AppHandle;

use crate::error::FourDaError;
use crate::{embed_texts, get_context_engine, get_settings_manager, invalidate_context_engine};

use super::validate_input_length;

/// Get current license tier and feature availability
#[tauri::command]
pub async fn get_license_tier() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let guard = manager.lock();
    let license = &guard.get().license;

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

/// Activate a license key — tries Keygen API first, falls back to ed25519 self-signed
#[tauri::command]
pub async fn activate_license(license_key: String) -> Result<serde_json::Value> {
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
        info!(target: "4da::license", "Validating Keygen key (format: {}...)", &license_key[..6.min(license_key.len())]);
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
    let manager = get_settings_manager();
    let guard = manager.lock();
    let settings = guard.get();
    let status = crate::settings::get_trial_status(&settings.license);

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

    Ok(serde_json::json!({
        "success": true,
        "days_remaining": 45,
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
/// Calls the Vercel API to look up the key, then auto-activates if found.
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
                200 => {
                    // Extract and validate license key
                    let license_key = body["license_key"].as_str().unwrap_or("").to_string();
                    if license_key.is_empty() {
                        return Ok(serde_json::json!({ "success": false, "reason": "not_found" }));
                    }
                    // Validate key format — must be 4DA- or Keygen format
                    if !license_key.starts_with("4DA-") && !license_key.contains('-') {
                        warn!(target: "4da::license", "Recovery returned invalid key format");
                        return Ok(
                            serde_json::json!({ "success": false, "reason": "invalid_key", "detail": "Server returned invalid license key format" }),
                        );
                    }

                    // Extract tier — default to "free" (not "signal") if missing
                    let tier = body["tier"].as_str().unwrap_or("free").to_string();

                    // Auto-activate: store in keychain + settings
                    let effective_tier = match tier.as_str() {
                        "signal" | "team" | "enterprise" => tier.clone(),
                        "pro" | "community" | "cohort" => "signal".to_string(),
                        _ => tier.clone(),
                    };

                    let manager = get_settings_manager();
                    let mut guard = manager.lock();
                    let settings = guard.get_mut();

                    let _ = crate::settings::keystore::store_secret("license_key", &license_key);
                    settings.license.license_key = license_key.clone();
                    settings.license.tier = effective_tier.clone();
                    let activated_at = chrono::Utc::now().to_rfc3339();
                    settings.license.activated_at = Some(activated_at.clone());
                    settings.license.trial_started_at = None;
                    guard.save()?;

                    crate::settings::save_license_backup(
                        &license_key,
                        &effective_tier,
                        &activated_at,
                    );

                    info!(target: "4da::license", tier = %effective_tier, "License recovered and activated via email lookup");

                    Ok(serde_json::json!({
                        "success": true,
                        "license_key": license_key,
                        "tier": effective_tier,
                        "expires_at": body["expires_at"],
                        "status": body["status"],
                    }))
                }
                404 => Ok(serde_json::json!({ "success": false, "reason": "not_found" })),
                410 => Ok(serde_json::json!({
                    "success": false,
                    "reason": "expired",
                    "detail": body["expired_at"].as_str().unwrap_or(""),
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

// ============================================================================
// Context Engine Commands
// ============================================================================

/// Get the user's static identity (interests, exclusions, role, etc.)
#[tauri::command]
pub async fn get_user_context() -> Result<serde_json::Value> {
    let engine = get_context_engine()?;

    let identity = engine
        .get_static_identity()
        .map_err(|e| format!("Failed to get identity: {e}"))?;

    let interest_count = engine.interest_count().unwrap_or(0);
    let exclusion_count = engine.exclusion_count().unwrap_or(0);

    Ok(serde_json::json!({
        "role": identity.role,
        "tech_stack": identity.tech_stack,
        "domains": identity.domains,
        "interests": identity.interests.iter().map(|i| serde_json::json!({
            "id": i.id,
            "topic": i.topic,
            "weight": i.weight,
            "source": i.source,
            "has_embedding": i.embedding.is_some()
        })).collect::<Vec<_>>(),
        "exclusions": identity.exclusions,
        "stats": {
            "interest_count": interest_count,
            "exclusion_count": exclusion_count
        }
    }))
}

/// Set the user's role
#[tauri::command]
pub async fn set_user_role(app: AppHandle, role: Option<String>) -> Result<serde_json::Value> {
    if let Some(ref r) = role {
        validate_input_length(r, "Role", 100)?;
    }
    let engine = get_context_engine()?;
    engine
        .set_role(role.as_deref())
        .map_err(|e| format!("Failed to set role: {e}"))?;

    info!(target: "4da::context", role = ?role, "Role updated");

    // GAME: track context setup + profile updates
    if role.is_some() {
        if let Ok(db) = crate::get_database() {
            for a in crate::achievement_engine::increment_counter(db, "context", 1) {
                crate::events::emit_achievement_unlocked(&app, &a);
            }
            for a in crate::achievement_engine::increment_counter(db, "profile_updates", 1) {
                crate::events::emit_achievement_unlocked(&app, &a);
            }
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "role": role
    }))
}

/// Set the user's experience level
#[tauri::command]
pub async fn set_experience_level(
    app: AppHandle,
    level: Option<String>,
) -> Result<serde_json::Value> {
    if let Some(ref l) = level {
        validate_input_length(l, "Experience level", 50)?;
    }
    let engine = get_context_engine()?;
    engine
        .set_experience_level(level.as_deref())
        .map_err(|e| format!("Failed to set experience level: {e}"))?;

    info!(target: "4da::context", level = ?level, "Experience level updated");

    // GAME: track profile updates
    if level.is_some() {
        if let Ok(db) = crate::get_database() {
            for a in crate::achievement_engine::increment_counter(db, "profile_updates", 1) {
                crate::events::emit_achievement_unlocked(&app, &a);
            }
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "experience_level": level
    }))
}

/// Add a technology to the user's tech stack
#[tauri::command]
pub async fn add_tech_stack(app: AppHandle, technology: String) -> Result<serde_json::Value> {
    validate_input_length(&technology, "Technology", 100)?;
    let engine = get_context_engine()?;
    engine
        .add_technology(&technology)
        .map_err(|e| format!("Failed to add technology: {e}"))?;

    debug!(target: "4da::context", technology = %technology, "Added technology");

    // GAME: track context setup + profile updates
    if let Ok(db) = crate::get_database() {
        for a in crate::achievement_engine::increment_counter(db, "context", 1) {
            crate::events::emit_achievement_unlocked(&app, &a);
        }
        for a in crate::achievement_engine::increment_counter(db, "profile_updates", 1) {
            crate::events::emit_achievement_unlocked(&app, &a);
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "technology": technology
    }))
}

/// Remove a technology from the user's tech stack
#[tauri::command]
pub async fn remove_tech_stack(technology: String) -> Result<serde_json::Value> {
    let engine = get_context_engine()?;
    engine
        .remove_technology(&technology)
        .map_err(|e| format!("Failed to remove technology: {e}"))?;

    debug!(target: "4da::context", technology = %technology, "Removed technology");

    Ok(serde_json::json!({
        "success": true
    }))
}
/// Add an explicit interest (with embedding generation)
#[tauri::command]
pub async fn add_interest(
    app: AppHandle,
    topic: String,
    weight: Option<f32>,
) -> Result<serde_json::Value> {
    validate_input_length(&topic, "Interest topic", 200)?;
    let engine = get_context_engine()?;
    let weight = weight.unwrap_or(1.0);

    // Generate embedding for the topic
    let embedding = embed_texts(std::slice::from_ref(&topic)).await?;
    let emb = embedding.first().map(std::vec::Vec::as_slice);

    let id = engine
        .add_interest(&topic, weight, emb, InterestSource::Explicit)
        .map_err(|e| format!("Failed to add interest: {e}"))?;

    info!(target: "4da::context", topic = %topic, weight = weight, has_embedding = emb.is_some(), "Added interest");
    invalidate_context_engine();

    // GAME: track context setup + profile updates
    if let Ok(db) = crate::get_database() {
        for a in crate::achievement_engine::increment_counter(db, "context", 1) {
            crate::events::emit_achievement_unlocked(&app, &a);
        }
        for a in crate::achievement_engine::increment_counter(db, "profile_updates", 1) {
            crate::events::emit_achievement_unlocked(&app, &a);
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "id": id,
        "topic": topic,
        "weight": weight,
        "has_embedding": emb.is_some()
    }))
}

/// Remove an interest
#[tauri::command]
pub async fn remove_interest(topic: String) -> Result<serde_json::Value> {
    let engine = get_context_engine()?;
    engine
        .remove_interest(&topic)
        .map_err(|e| format!("Failed to remove interest: {e}"))?;

    info!(target: "4da::context", topic = %topic, "Removed interest");
    invalidate_context_engine();

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Add an exclusion (topic to never show)
#[tauri::command]
pub async fn add_exclusion(topic: String) -> Result<serde_json::Value> {
    validate_input_length(&topic, "Exclusion topic", 200)?;
    let engine = get_context_engine()?;
    engine
        .add_exclusion(&topic)
        .map_err(|e| format!("Failed to add exclusion: {e}"))?;

    info!(target: "4da::context", topic = %topic, "Added exclusion");
    invalidate_context_engine();

    Ok(serde_json::json!({
        "success": true,
        "topic": topic
    }))
}

/// Remove an exclusion
#[tauri::command]
pub async fn remove_exclusion(topic: String) -> Result<serde_json::Value> {
    let engine = get_context_engine()?;
    engine
        .remove_exclusion(&topic)
        .map_err(|e| format!("Failed to remove exclusion: {e}"))?;

    info!(target: "4da::context", topic = %topic, "Removed exclusion");
    invalidate_context_engine();

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Record a user interaction (click, save, dismiss)
#[tauri::command]
pub async fn record_interaction(
    app: AppHandle,
    source_item_id: i64,
    action: String,
) -> Result<serde_json::Value> {
    let engine = get_context_engine()?;

    let action_type = match action.to_lowercase().as_str() {
        "click" => InteractionType::Click,
        "save" => InteractionType::Save,
        "dismiss" => InteractionType::Dismiss,
        "ignore" => InteractionType::Ignore,
        _ => return Err(format!("Unknown action type: {action}").into()),
    };

    engine
        .record_interaction(source_item_id, action_type)
        .map_err(|e| format!("Failed to record interaction: {e}"))?;

    debug!(target: "4da::context", action = %action, item_id = source_item_id, "Recorded interaction");

    // GAME: track saves
    if action.to_lowercase() == "save" {
        if let Ok(db) = crate::get_database() {
            for a in crate::achievement_engine::increment_counter(db, "saves", 1) {
                crate::events::emit_achievement_unlocked(&app, &a);
            }
        }
    }

    Ok(serde_json::json!({
        "success": true
    }))
}

/// Snooze an item for a number of days. The item will reappear after the
/// snooze period expires. Recorded as a distinct interaction type so the
/// trust model treats it as "deferred" rather than "rejected."
#[tauri::command]
pub async fn snooze_item(
    source_item_id: i64,
    days: u32,
) -> Result<serde_json::Value> {
    let days = days.clamp(1, 30);
    let conn = crate::state::open_db_connection()?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS snoozed_items (
            source_item_id INTEGER PRIMARY KEY,
            snooze_until TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )
    .map_err(|e| format!("Failed to create snoozed_items table: {e}"))?;

    conn.execute(
        "INSERT OR REPLACE INTO snoozed_items (source_item_id, snooze_until)
         VALUES (?1, datetime('now', ?2))",
        rusqlite::params![source_item_id, format!("+{days} days")],
    )
    .map_err(|e| format!("Failed to snooze item: {e}"))?;

    if let Ok(engine) = get_context_engine() {
        let _ = engine.record_interaction(source_item_id, InteractionType::Ignore);
    }

    debug!(target: "4da::context", item_id = source_item_id, days = days, "Snoozed item");

    Ok(serde_json::json!({
        "success": true,
        "snooze_days": days
    }))
}

/// Get locale configuration
#[tauri::command]
pub async fn get_locale() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let guard = manager.lock();
    let locale = &guard.get().locale;
    Ok(serde_json::json!({
        "country": locale.country,
        "language": locale.language,
        "currency": locale.currency
    }))
}

/// Update locale configuration
#[tauri::command]
pub async fn set_locale(country: String, language: String, currency: String) -> Result<()> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();
    guard.get_mut().locale = crate::settings::LocaleConfig {
        country,
        language,
        currency,
    };
    guard
        .save()
        .map_err(|e| FourDaError::Config(format!("Failed to save locale: {e}")))?;
    info!(target: "4da::settings", "Locale updated");
    Ok(())
}

/// Get context engine statistics
#[tauri::command]
pub async fn get_context_stats() -> Result<serde_json::Value> {
    let engine = get_context_engine()?;

    let interest_count = engine.interest_count().unwrap_or(0);
    let exclusion_count = engine.exclusion_count().unwrap_or(0);

    let identity = engine
        .get_static_identity()
        .map_err(|e| format!("Failed to get identity: {e}"))?;

    Ok(serde_json::json!({
        "interests": interest_count,
        "exclusions": exclusion_count,
        "tech_stack": identity.tech_stack.len(),
        "domains": identity.domains.len(),
        "has_role": identity.role.is_some()
    }))
}

/// Get a Signal value report summarizing pipeline impact from real analysis data.
#[tauri::command]
pub async fn get_pro_value_report() -> Result<serde_json::Value> {
    let conn = crate::state::open_db_connection()?;

    let total_scored: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(total_scored), 0) FROM accuracy_history",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_relevant: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(total_relevant), 0) FROM accuracy_history",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let noise_rejected = (total_scored - total_relevant).max(0) as u32;
    let time_saved = crate::accuracy::estimate_time_saved(noise_rejected);

    let knowledge_gaps: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digested_intelligence WHERE digest_type = 'knowledge_gap'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let decisions: i64 = conn
        .query_row("SELECT COUNT(*) FROM decision_windows", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    Ok(serde_json::json!({
        "total_saved_hours": time_saved,
        "total_signals_processed": total_scored,
        "signals_detected": total_relevant,
        "knowledge_gaps_caught": knowledge_gaps,
        "decisions_recorded": decisions,
        "estimated_hours_saved": time_saved,
        "top_discoveries": [],
        "period_days": 30
    }))
}
