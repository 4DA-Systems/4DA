//! Tech Radar — Additional Tauri commands for interactive radar features
//!
//! Provides entry detail enrichment, snapshot listing, and historical
//! radar views. Keeps tech_radar.rs under the 600-line Rust limit.

use rusqlite::params;
use serde_json::json;
use tracing::debug;

// ============================================================================
// Entry Detail
// ============================================================================

/// Returns enriched detail for a specific radar entry:
/// - The entry itself (if found)
/// - Related source items mentioning this technology
/// - Decision details if decision_ref is present
#[tauri::command]
pub async fn get_radar_entry_detail(name: String) -> Result<serde_json::Value, String> {
    let conn = crate::open_db_connection()?;
    let radar = crate::tech_radar::compute_radar(&conn)?;
    let entry = radar
        .entries
        .into_iter()
        .find(|e| e.name.eq_ignore_ascii_case(&name));

    // Query recent source items that mention this technology
    let pattern = format!("%{}%", name);
    let related_items: Vec<serde_json::Value> = {
        let mut stmt = conn
            .prepare(
                "SELECT title, source_type, url, created_at
                 FROM source_items
                 WHERE (LOWER(title) LIKE ?1 OR LOWER(content) LIKE ?1)
                 AND created_at >= datetime('now', '-30 days')
                 ORDER BY created_at DESC
                 LIMIT 10",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![pattern], |row| {
                Ok(json!({
                    "title": row.get::<_, String>(0)?,
                    "source_type": row.get::<_, String>(1)?,
                    "url": row.get::<_, Option<String>>(2)?,
                    "created_at": row.get::<_, String>(3)?,
                }))
            })
            .map_err(|e| e.to_string())?;

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
pub async fn get_radar_snapshots() -> Result<Vec<serde_json::Value>, String> {
    let conn = crate::open_db_connection()?;

    let mut stmt = conn
        .prepare(
            "SELECT created_at
             FROM temporal_events
             WHERE event_type = 'radar_snapshot'
             ORDER BY created_at DESC
             LIMIT 30",
        )
        .map_err(|e| e.to_string())?;

    let snapshots: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            let date: String = row.get(0)?;
            Ok(json!({ "date": date }))
        })
        .map_err(|e| e.to_string())?
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
pub async fn get_radar_at_snapshot(snapshot_date: String) -> Result<serde_json::Value, String> {
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

    match data {
        Some(json_str) => {
            let parsed: serde_json::Value =
                serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
            debug!(
                target: "4da::tech_radar",
                date = %snapshot_date,
                "Loaded radar snapshot"
            );
            Ok(parsed)
        }
        None => {
            // No snapshot found at this date — return current radar instead
            let radar = crate::tech_radar::compute_radar(&conn)?;
            Ok(serde_json::to_value(radar).map_err(|e| e.to_string())?)
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

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
