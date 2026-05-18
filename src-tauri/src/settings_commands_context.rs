// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Context engine, interaction, and user-identity Tauri commands.

use tracing::{debug, info};

use crate::context_engine::{InteractionType, InterestSource};
use crate::error::{FourDaError, Result};
use tauri::AppHandle;

use crate::{embed_texts, get_context_engine, get_settings_manager, invalidate_context_engine};

use super::validate_input_length;

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
    dismiss_reason: Option<String>,
    dismiss_category: Option<String>,
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
        .record_interaction(
            source_item_id,
            action_type,
            dismiss_reason.as_deref(),
            dismiss_category.as_deref(),
        )
        .map_err(|e| format!("Failed to record interaction: {e}"))?;

    debug!(target: "4da::context", action = %action, item_id = source_item_id, "Recorded interaction");

    // Feed the stability detector — this is what makes preferences compound
    if let Ok(conn) = crate::state::open_db_connection() {
        match action.to_lowercase().as_str() {
            "click" => crate::engagement_telemetry::on_click(&conn, source_item_id),
            "save" => crate::engagement_telemetry::on_save(&conn, source_item_id),
            "dismiss" => crate::engagement_telemetry::on_dismiss(
                &conn,
                source_item_id,
                dismiss_category.as_deref(),
            ),
            _ => {}
        }

        // Record temporal event for compound context
        let _ = crate::temporal::record_event(
            &conn,
            "user_interaction",
            &action,
            &serde_json::json!({
                "source_item_id": source_item_id,
                "dismiss_reason": dismiss_reason,
                "dismiss_category": dismiss_category,
            }),
            Some(source_item_id),
            Some(&(chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339()),
        );
    }

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
pub async fn snooze_item(source_item_id: i64, days: u32) -> Result<serde_json::Value> {
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
        let _ = engine.record_interaction(source_item_id, InteractionType::Ignore, None, None);
    }

    debug!(target: "4da::context", item_id = source_item_id, days = days, "Snoozed item");

    Ok(serde_json::json!({
        "success": true,
        "snooze_days": days
    }))
}

/// Watch an item — resurface when new signals arrive for the same topic
#[tauri::command]
pub async fn watch_item(
    source_item_id: i64,
    topic: String,
    title: String,
) -> Result<serde_json::Value> {
    validate_input_length(&topic, "Watch topic", 200)?;
    validate_input_length(&title, "Watch title", 500)?;
    let conn = crate::state::open_db_connection()?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS watched_items (
            source_item_id INTEGER PRIMARY KEY,
            topic TEXT NOT NULL,
            title TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )
    .map_err(|e| format!("Failed to create watched_items table: {e}"))?;

    conn.execute(
        "INSERT OR REPLACE INTO watched_items (source_item_id, topic, title)
         VALUES (?1, ?2, ?3)",
        rusqlite::params![source_item_id, topic, title],
    )
    .map_err(|e| format!("Failed to watch item: {e}"))?;

    debug!(target: "4da::context", item_id = source_item_id, topic = %topic, "Watching item");

    Ok(serde_json::json!({
        "success": true,
        "topic": topic
    }))
}

/// Remove an item from watched list
#[tauri::command]
pub async fn unwatch_item(source_item_id: i64) -> Result<serde_json::Value> {
    let conn = crate::state::open_db_connection()?;

    conn.execute(
        "DELETE FROM watched_items WHERE source_item_id = ?1",
        rusqlite::params![source_item_id],
    )
    .map_err(|e| format!("Failed to unwatch item: {e}"))?;

    Ok(serde_json::json!({ "success": true }))
}

/// Get all currently watched items
#[tauri::command]
pub async fn get_watched_items() -> Result<serde_json::Value> {
    let conn = crate::state::open_db_connection()?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS watched_items (
            source_item_id INTEGER PRIMARY KEY,
            topic TEXT NOT NULL,
            title TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )
    .map_err(|e| format!("Failed to create watched_items table: {e}"))?;

    let mut stmt = conn
        .prepare("SELECT source_item_id, topic, title, created_at FROM watched_items ORDER BY created_at DESC")
        .map_err(|e| format!("Failed to query watched items: {e}"))?;

    let items: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "source_item_id": row.get::<_, i64>(0)?,
                "topic": row.get::<_, String>(1)?,
                "title": row.get::<_, String>(2)?,
                "created_at": row.get::<_, String>(3)?,
            }))
        })
        .map_err(|e| format!("Failed to read watched items: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(serde_json::json!({ "watched_items": items }))
}

/// Set blind spot detection sensitivity
#[tauri::command]
pub async fn set_blind_spot_sensitivity(sensitivity: String) -> Result<serde_json::Value> {
    let valid = ["aggressive", "normal", "relaxed"];
    if !valid.contains(&sensitivity.as_str()) {
        return Err(format!(
            "Invalid sensitivity: {sensitivity}. Must be one of: {}",
            valid.join(", ")
        )
        .into());
    }
    let manager = get_settings_manager();
    let mut guard = manager.lock();
    guard.get_mut().blind_spot_sensitivity = sensitivity.clone();
    guard.save()?;
    Ok(serde_json::json!({ "success": true, "sensitivity": sensitivity }))
}

/// Get blind spot detection sensitivity
#[tauri::command]
pub async fn get_blind_spot_sensitivity() -> Result<serde_json::Value> {
    let manager = get_settings_manager();
    let guard = manager.lock();
    let sensitivity = guard.get().blind_spot_sensitivity.clone();
    Ok(serde_json::json!({ "sensitivity": sensitivity }))
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
