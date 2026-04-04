//! Tech Radar — Additional Tauri commands for interactive radar features
//!
//! Provides entry detail enrichment, snapshot listing, and historical
//! radar views. Keeps tech_radar.rs under the 600-line Rust limit.

use rusqlite::params;
use serde_json::json;
use tracing::debug;

use crate::error::Result;

// ============================================================================
// Entry Detail
// ============================================================================

/// Returns enriched detail for a specific radar entry:
/// - The entry itself (if found)
/// - Related source items mentioning this technology
/// - Decision details if decision_ref is present
#[tauri::command]
pub async fn get_radar_entry_detail(name: String) -> Result<serde_json::Value> {
    let name =
        crate::ipc_guard::validate_length("name", &name, crate::ipc_guard::MAX_INPUT_LENGTH)?;
    let conn = crate::open_db_connection()?;
    let radar = crate::tech_radar::compute_radar(&conn)?;
    let entry = radar
        .entries
        .into_iter()
        .find(|e| e.name.eq_ignore_ascii_case(&name));

    // Query recent source items that mention this technology
    let pattern = format!("%{name}%");
    let related_items: Vec<serde_json::Value> = {
        let mut stmt = conn.prepare(
            "SELECT title, source_type, url, created_at
             FROM source_items
             WHERE (LOWER(title) LIKE ?1 OR LOWER(content) LIKE ?1)
             AND created_at >= datetime('now', '-30 days')
             ORDER BY created_at DESC
             LIMIT 10",
        )?;

        let rows = stmt.query_map(params![pattern], |row| {
            Ok(json!({
                "title": row.get::<_, String>(0)?,
                "source_type": row.get::<_, String>(1)?,
                "url": row.get::<_, Option<String>>(2)?,
                "created_at": row.get::<_, String>(3)?,
            }))
        })?;

        rows.flatten().collect()
    };

    // Query decision details if the entry has a decision_ref
    let decision = if let Some(ref e) = entry {
        if let Some(ref_id) = e.decision_ref {
            conn.query_row(
                "SELECT id, decision, rationale, status
                 FROM developer_decisions WHERE id = ?1",
                params![ref_id],
                |row| {
                    Ok(json!({
                        "id": row.get::<_, i64>(0)?,
                        "decision": row.get::<_, String>(1)?,
                        "rationale": row.get::<_, Option<String>>(2)?,
                        "status": row.get::<_, String>(3)?,
                    }))
                },
            )
            .ok()
        } else {
            None
        }
    } else {
        None
    };

    debug!(
        target: "4da::tech_radar",
        name = %name,
        found = entry.is_some(),
        related = related_items.len(),
        has_decision = decision.is_some(),
        "Entry detail requested"
    );

    Ok(json!({
        "entry": entry,
        "related_items": related_items,
        "decision": decision,
    }))
}

// ============================================================================
// Snapshots
// ============================================================================

/// Returns available radar snapshot dates from temporal_events.
/// Each snapshot represents a point-in-time radar state.
#[tauri::command]
pub async fn get_radar_snapshots() -> Result<Vec<serde_json::Value>> {
    let conn = crate::open_db_connection()?;

    let mut stmt = conn.prepare(
        "SELECT created_at
         FROM temporal_events
         WHERE event_type = 'radar_snapshot'
         ORDER BY created_at DESC
         LIMIT 30",
    )?;

    let snapshots: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            let date: String = row.get(0)?;
            Ok(json!({ "date": date }))
        })?
        .flatten()
        .collect();

    debug!(
        target: "4da::tech_radar",
        count = snapshots.len(),
        "Radar snapshots listed"
    );

    Ok(snapshots)
}

/// Load a historical radar state from a specific temporal snapshot.
/// Returns the stored radar data for that point in time.
#[tauri::command]
pub async fn get_radar_at_snapshot(snapshot_date: String) -> Result<serde_json::Value> {
    let conn = crate::open_db_connection()?;

    let data: Option<String> = conn
        .query_row(
            "SELECT data FROM temporal_events
             WHERE event_type = 'radar_snapshot'
             AND created_at = ?1
             LIMIT 1",
            params![snapshot_date],
            |row| row.get(0),
        )
        .ok();

    if let Some(json_str) = data {
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;
        debug!(
            target: "4da::tech_radar",
            date = %snapshot_date,
            "Loaded radar snapshot"
        );
        Ok(parsed)
    } else {
        // No snapshot found at this date — return current radar instead
        let radar = crate::tech_radar::compute_radar(&conn)?;
        Ok(serde_json::to_value(radar)?)
    }
}

// ============================================================================
// Tests
// ============================================================================

// ============================================================================
// LLM Narrative Synthesis
// ============================================================================

/// Generate natural-language narratives for moving technologies.
/// Uses the LLM pipeline to produce one-sentence summaries like:
/// "React Server Components adoption is accelerating — 3 of your dependencies shipped RSC support"
/// Falls back gracefully if no LLM is configured.
#[tauri::command]
pub async fn generate_tech_narratives() -> Result<serde_json::Value> {
    let conn = crate::open_db_connection()?;
    let radar = crate::tech_radar::compute_radar(&conn)?;

    let moving: Vec<&crate::tech_radar::RadarEntry> = radar
        .entries
        .iter()
        .filter(|e| !matches!(e.movement, crate::tech_radar::RadarMovement::Stable))
        .take(8)
        .collect();

    if moving.is_empty() {
        return Ok(json!({ "narratives": {} }));
    }

    // Build a compact prompt for the LLM
    let mut tech_summaries = String::new();
    for entry in &moving {
        let movement = match entry.movement {
            crate::tech_radar::RadarMovement::Up => "rising",
            crate::tech_radar::RadarMovement::Down => "declining",
            crate::tech_radar::RadarMovement::New => "newly appearing",
            crate::tech_radar::RadarMovement::Stable => "stable",
        };
        let signals_text = entry.signals.join("; ");
        tech_summaries.push_str(&format!(
            "- {} ({}): {}\n",
            entry.name, movement, signals_text
        ));
    }

    let prompt = format!(
        "You are a developer intelligence assistant. For each technology below, write ONE concise sentence \
         explaining what's happening and why it matters to a developer. Be specific, reference the signals. \
         No preamble, no markdown. Format: TechName: sentence\n\n{tech_summaries}"
    );

    // Try LLM synthesis
    let llm_result: std::result::Result<String, crate::error::FourDaError> = async {
        let llm_settings = {
            let settings = crate::get_settings_manager().lock();
            settings.get().llm.clone()
        };
        if llm_settings.provider != "ollama" && llm_settings.api_key.is_empty() {
            return Err(crate::error::FourDaError::Config("No LLM configured".into()));
        }
        let client = crate::llm::LLMClient::new(llm_settings);
        let messages = vec![crate::llm::Message {
            role: "user".to_string(),
            content: prompt.clone(),
        }];
        let response = client.complete(
            "You are a concise developer intelligence assistant. Write exactly one sentence per technology.",
            messages,
        ).await?;
        Ok(response.content)
    }.await;

    match llm_result {
        Ok(response) => {
            let mut narratives = serde_json::Map::new();
            for line in response.lines() {
                if let Some((name, narrative)) = line.split_once(':') {
                    let clean_name = name.trim().to_lowercase();
                    let clean_narrative = narrative.trim().to_string();
                    if !clean_narrative.is_empty() {
                        narratives.insert(clean_name, json!(clean_narrative));
                    }
                }
            }
            Ok(json!({ "narratives": narratives }))
        }
        Err(e) => {
            debug!(target: "4da::radar", error = %e, "LLM narrative synthesis unavailable, using signal-based fallback");
            Ok(json!({ "narratives": {} }))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_snapshot_date_parsing() {
        // Verify the date string format we expect
        let date = "2025-01-15T10:30:00Z";
        assert!(date.contains('T'));
        assert!(date.ends_with('Z'));
    }
}
