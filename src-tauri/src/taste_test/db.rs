//! Persistence layer for taste test results using existing SQLite database.

use rusqlite::{params, Connection};
use serde_json;

use super::{
    PersonaWeight, TasteProfile, TasteProfileSummary, TasteResponse, PERSONA_DESCRIPTIONS,
    PERSONA_NAMES,
};
use crate::autophagy::CalibrationDelta;

// ============================================================================
// Schema
// ============================================================================

/// Create taste test tables if they don't exist.
/// Called lazily on first use (same pattern as coach_nudges).
pub fn ensure_taste_test_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS taste_test_results (
            id INTEGER PRIMARY KEY,
            completed_at TEXT NOT NULL DEFAULT (datetime('now')),
            items_shown INTEGER NOT NULL,
            confidence REAL NOT NULL,
            persona_weights TEXT NOT NULL,
            dominant_persona INTEGER NOT NULL,
            version INTEGER NOT NULL DEFAULT 1
        );
        CREATE TABLE IF NOT EXISTS taste_test_responses (
            id INTEGER PRIMARY KEY,
            test_id INTEGER NOT NULL REFERENCES taste_test_results(id),
            item_slot INTEGER NOT NULL,
            response TEXT NOT NULL,
            response_time_ms INTEGER
        );",
    )
    .map_err(|e| format!("Failed to create taste test tables: {e}"))?;
    Ok(())
}

// ============================================================================
// CRUD
// ============================================================================

/// Save a completed taste test result and its individual responses.
pub fn save_taste_result(
    conn: &Connection,
    profile: &TasteProfile,
    responses: &[(usize, TasteResponse)],
    latencies: &[Option<u64>],
) -> Result<i64, String> {
    ensure_taste_test_tables(conn)?;

    let weights_json = serde_json::to_string(&profile.persona_weights.to_vec())
        .map_err(|e| format!("Failed to serialize weights: {e}"))?;

    conn.execute(
        "INSERT INTO taste_test_results (items_shown, confidence, persona_weights, dominant_persona)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            profile.items_shown,
            profile.confidence,
            weights_json,
            profile.dominant_persona as i64,
        ],
    ).map_err(|e| format!("Failed to save taste test result: {e}"))?;

    let test_id = conn.last_insert_rowid();

    for (i, (slot, response)) in responses.iter().enumerate() {
        let response_str = match response {
            TasteResponse::Interested => "interested",
            TasteResponse::NotInterested => "not_interested",
            TasteResponse::StrongInterest => "strong_interest",
        };
        let latency_ms = latencies.get(i).copied().flatten();
        conn.execute(
            "INSERT INTO taste_test_responses (test_id, item_slot, response, response_time_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![test_id, *slot as i64, response_str, latency_ms],
        )
        .map_err(|e| format!("Failed to save taste test response: {e}"))?;
    }

    Ok(test_id)
}

/// Load the most recent taste test result.
pub fn load_latest_taste_result(conn: &Connection) -> Option<TasteProfileSummary> {
    if ensure_taste_test_tables(conn).is_err() {
        return None;
    }

    let mut stmt = conn
        .prepare(
            "SELECT items_shown, confidence, persona_weights, dominant_persona
             FROM taste_test_results
             ORDER BY id DESC LIMIT 1",
        )
        .ok()?;

    stmt.query_row([], |row| {
        let items_shown: u32 = row.get(0)?;
        let confidence: f64 = row.get(1)?;
        let weights_json: String = row.get(2)?;
        let dominant: usize = row.get::<_, i64>(3)? as usize;

        let weights: Vec<f64> = serde_json::from_str(&weights_json).unwrap_or_default();

        // Build summary from stored weights
        let persona_weights: Vec<PersonaWeight> = weights
            .iter()
            .enumerate()
            .filter(|(_, &w)| w > 0.05)
            .map(|(i, &w)| PersonaWeight {
                name: PERSONA_NAMES.get(i).unwrap_or(&"Unknown").to_string(),
                weight: w,
            })
            .collect();

        // Reconstruct top interests via blending
        let mut weight_arr = [0.0f64; 9];
        for (i, &w) in weights.iter().enumerate().take(9) {
            weight_arr[i] = w;
        }
        let blended = crate::taste_test::blending::blend_profile(&weight_arr, 0.10);
        let top_interests: Vec<String> = blended
            .interests
            .into_iter()
            .take(10)
            .map(|(t, _)| t)
            .collect();

        Ok(TasteProfileSummary {
            dominant_persona_name: PERSONA_NAMES
                .get(dominant)
                .unwrap_or(&"Unknown")
                .to_string(),
            dominant_persona_description: PERSONA_DESCRIPTIONS
                .get(dominant)
                .unwrap_or(&"")
                .to_string(),
            confidence,
            items_shown,
            persona_weights,
            top_interests,
        })
    })
    .ok()
}

/// Check if any taste test has been completed.
pub fn is_calibrated(conn: &Connection) -> bool {
    if ensure_taste_test_tables(conn).is_err() {
        return false;
    }

    conn.query_row("SELECT COUNT(*) FROM taste_test_results", [], |row| {
        row.get::<_, i64>(0)
    })
    .unwrap_or(0)
        > 0
}

/// Apply taste test results to the existing context tables.
///
/// Writes to the SAME tables that build_scoring_context() reads:
/// - explicit_interests
/// - exclusions
/// - digested_intelligence (via store_calibrations)
pub fn apply_taste_to_context(conn: &Connection, profile: &TasteProfile) -> Result<(), String> {
    // Add inferred interests
    for (topic, weight) in &profile.inferred_interests {
        conn.execute(
            "INSERT OR REPLACE INTO explicit_interests (topic, weight, source)
             VALUES (?1, ?2, ?3)",
            params![topic, weight, "inferred"],
        )
        .map_err(|e| format!("Failed to add interest '{topic}': {e}"))?;
    }

    // Add exclusions
    for topic in &profile.inferred_exclusions {
        conn.execute(
            "INSERT OR IGNORE INTO exclusions (topic) VALUES (?1)",
            params![topic],
        )
        .map_err(|e| format!("Failed to add exclusion '{topic}': {e}"))?;
    }

    // Store calibration deltas via autophagy
    let deltas: Vec<CalibrationDelta> = profile
        .calibration_deltas
        .iter()
        .map(|(topic, delta)| CalibrationDelta {
            topic: topic.clone(),
            scored_avg: 0.0,
            engaged_avg: 0.0,
            delta: *delta,
            sample_size: 1,
            confidence: profile.confidence as f32,
        })
        .collect();

    if !deltas.is_empty() {
        crate::autophagy::store_calibrations(conn, &deltas)?;
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // Create required tables
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS explicit_interests (
                id INTEGER PRIMARY KEY,
                topic TEXT NOT NULL UNIQUE,
                weight REAL NOT NULL DEFAULT 1.0,
                source TEXT NOT NULL DEFAULT 'explicit',
                embedding BLOB
            );
            CREATE TABLE IF NOT EXISTS exclusions (
                id INTEGER PRIMARY KEY,
                topic TEXT NOT NULL UNIQUE
            );
            CREATE TABLE IF NOT EXISTS digested_intelligence (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                digest_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                data TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 0.5,
                sample_size INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT,
                superseded_by INTEGER
            );",
        )
        .unwrap();
        ensure_taste_test_tables(&conn).unwrap();
        conn
    }

    fn make_test_profile() -> TasteProfile {
        TasteProfile {
            persona_weights: [0.5, 0.2, 0.1, 0.05, 0.05, 0.03, 0.03, 0.02, 0.02],
            dominant_persona: 0,
            confidence: 0.75,
            items_shown: 10,
            inferred_interests: vec![
                ("Rust".to_string(), 1.0),
                ("systems programming".to_string(), 0.9),
            ],
            inferred_exclusions: vec![],
            calibration_deltas: vec![("Machine Learning".to_string(), 0.05)],
        }
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let conn = setup_test_db();
        let profile = make_test_profile();
        let responses = vec![
            (0, TasteResponse::Interested),
            (1, TasteResponse::NotInterested),
        ];

        let latencies = vec![Some(1200), Some(800)];
        let test_id = save_taste_result(&conn, &profile, &responses, &latencies).unwrap();
        assert!(test_id > 0);

        let loaded = load_latest_taste_result(&conn);
        assert!(loaded.is_some());
        let summary = loaded.unwrap();
        assert_eq!(summary.items_shown, 10);
        assert!((summary.confidence - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_is_calibrated_false_initially() {
        let conn = setup_test_db();
        assert!(!is_calibrated(&conn));
    }

    #[test]
    fn test_is_calibrated_true_after_save() {
        let conn = setup_test_db();
        let profile = make_test_profile();
        save_taste_result(&conn, &profile, &[], &[]).unwrap();
        assert!(is_calibrated(&conn));
    }

    #[test]
    fn test_apply_taste_writes_interests() {
        let conn = setup_test_db();
        let profile = make_test_profile();
        apply_taste_to_context(&conn, &profile).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM explicit_interests WHERE source = 'inferred'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(count > 0, "Should have written inferred interests");
    }
}
