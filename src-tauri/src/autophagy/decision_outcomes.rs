//! Decision window outcome analysis.
//!
//! Analyzes historical decision window outcomes (acted vs expired)
//! to learn which window types and dependencies users respond to.
//! Results feed back into scoring to improve future window urgency.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{Result, ResultExt};

// ============================================================================
// Types
// ============================================================================

/// Aggregated outcome data for a (window_type, dependency) pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DecisionWindowOutcome {
    pub window_type: String,
    pub dependency: Option<String>,
    pub windows_acted: i64,
    pub windows_expired: i64,
    pub response_rate: f64,
    pub avg_lead_time_hours: f64,
    pub confidence: f64,
    pub sample_size: i64,
}

// ============================================================================
// Analysis
// ============================================================================

/// Analyze decision window outcomes from the last 30 days.
///
/// Groups resolved windows (acted, expired, closed) by (window_type, dependency)
/// and computes response rates, average lead times, and confidence scores.
pub(crate) fn analyze_decision_window_outcomes(conn: &Connection) -> Vec<DecisionWindowOutcome> {
    let mut stmt = match conn.prepare(
        "SELECT window_type, dependency, status, lead_time_hours
         FROM decision_windows
         WHERE status IN ('acted', 'expired', 'closed')
           AND COALESCE(acted_at, closed_at, opened_at) > datetime('now', '-30 days')",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Decision outcome query failed");
            return vec![];
        }
    };

    // Accumulate per-(window_type, dependency) stats
    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<f64>>(3)?,
        ))
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Decision outcome row iteration failed");
            return vec![];
        }
    };

    // Key: (window_type, dependency) -> (acted, expired, lead_time_sum, lead_time_count)
    let mut stats: std::collections::HashMap<(String, Option<String>), (i64, i64, f64, i64)> =
        std::collections::HashMap::new();

    for row in rows.flatten() {
        let (window_type, dependency, status, lead_time_hours) = row;
        let key = (window_type, dependency);
        let entry = stats.entry(key).or_insert((0, 0, 0.0, 0));

        match status.as_str() {
            "acted" => {
                entry.0 += 1;
                if let Some(lt) = lead_time_hours {
                    entry.2 += lt;
                    entry.3 += 1;
                }
            }
            "expired" | "closed" => {
                entry.1 += 1;
            }
            _ => {}
        }
    }

    if stats.is_empty() {
        debug!(target: "4da::autophagy", "No decision window outcomes to analyze");
        return vec![];
    }

    let mut outcomes = Vec::new();

    for ((window_type, dependency), (acted, expired, lead_time_sum, lead_time_count)) in stats {
        let sample_size = acted + expired;
        if sample_size == 0 {
            continue;
        }

        let response_rate = acted as f64 / sample_size as f64;
        let avg_lead_time_hours = if lead_time_count > 0 {
            lead_time_sum / lead_time_count as f64
        } else {
            0.0
        };
        let confidence = (sample_size as f64 / 20.0).sqrt().min(1.0);

        outcomes.push(DecisionWindowOutcome {
            window_type,
            dependency,
            windows_acted: acted,
            windows_expired: expired,
            response_rate,
            avg_lead_time_hours,
            confidence,
            sample_size,
        });
    }

    info!(
        target: "4da::autophagy",
        outcomes = outcomes.len(),
        "Decision window outcome analysis complete"
    );

    outcomes
}

// ============================================================================
// Storage
// ============================================================================

/// Store decision window outcomes to `digested_intelligence`.
///
/// Uses digest_type = 'decision_outcome' and subject = '{window_type}' or
/// '{window_type}_{dependency}'. Previous entries for the same subject are
/// superseded (DELETE + INSERT) to keep only the latest analysis.
pub(crate) fn store_decision_outcomes(
    conn: &Connection,
    outcomes: &[DecisionWindowOutcome],
) -> Result<()> {
    for outcome in outcomes {
        let subject = match &outcome.dependency {
            Some(dep) => format!("{}_{}", outcome.window_type, dep),
            None => outcome.window_type.clone(),
        };

        let data = serde_json::to_string(outcome)
            .with_context(|| format!("Failed to serialize outcome for {}", subject))?;

        // Supersede previous entries: delete old, insert new
        conn.execute(
            "DELETE FROM digested_intelligence
             WHERE digest_type = 'decision_outcome' AND subject = ?1",
            params![subject],
        )
        .with_context(|| format!("Failed to supersede decision outcome for {}", subject))?;

        conn.execute(
            "INSERT INTO digested_intelligence (digest_type, subject, data, confidence, sample_size)
             VALUES ('decision_outcome', ?1, ?2, ?3, ?4)",
            params![
                subject,
                data,
                outcome.confidence,
                outcome.sample_size
            ],
        )
        .with_context(|| format!("Failed to insert decision outcome for {}", subject))?;
    }

    debug!(
        target: "4da::autophagy",
        count = outcomes.len(),
        "Stored decision window outcomes"
    );
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE decision_windows (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                window_type TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT DEFAULT '',
                urgency REAL DEFAULT 0.5,
                relevance REAL DEFAULT 0.5,
                source_item_ids TEXT DEFAULT '[]',
                signal_chain_id INTEGER,
                dependency TEXT,
                status TEXT DEFAULT 'open',
                opened_at TEXT DEFAULT (datetime('now')),
                expires_at TEXT,
                acted_at TEXT,
                closed_at TEXT,
                outcome TEXT,
                lead_time_hours REAL,
                streets_engine TEXT
            );
            CREATE TABLE digested_intelligence (
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
        .expect("create tables");
        conn
    }

    /// Insert a resolved decision window into the test DB.
    fn insert_window(
        conn: &Connection,
        window_type: &str,
        dependency: Option<&str>,
        status: &str,
        lead_time_hours: Option<f64>,
    ) {
        let acted_at = if status == "acted" {
            Some("datetime('now')".to_string())
        } else {
            None
        };
        let closed_at = if status != "acted" {
            Some("datetime('now')".to_string())
        } else {
            None
        };

        // Use raw SQL with datetime expressions for timestamps
        conn.execute(
            "INSERT INTO decision_windows
                (window_type, title, dependency, status, lead_time_hours,
                 acted_at, closed_at, opened_at)
             VALUES (?1, ?2, ?3, ?4, ?5,
                     CASE WHEN ?4 = 'acted' THEN datetime('now') ELSE NULL END,
                     CASE WHEN ?4 != 'acted' THEN datetime('now') ELSE NULL END,
                     datetime('now'))",
            params![
                window_type,
                format!("Test: {} {}", window_type, dependency.unwrap_or("general")),
                dependency,
                status,
                lead_time_hours,
            ],
        )
        .expect("insert test window");
    }

    #[test]
    fn test_empty_decision_windows_table() {
        let conn = setup_test_db();
        let outcomes = analyze_decision_window_outcomes(&conn);
        assert!(
            outcomes.is_empty(),
            "Empty decision_windows table should produce no outcomes"
        );
    }

    #[test]
    fn test_mixed_acted_expired_response_rate() {
        let conn = setup_test_db();

        // 3 acted, 2 expired for security_patch + lodash
        insert_window(&conn, "security_patch", Some("lodash"), "acted", Some(12.0));
        insert_window(&conn, "security_patch", Some("lodash"), "acted", Some(24.0));
        insert_window(&conn, "security_patch", Some("lodash"), "acted", Some(6.0));
        insert_window(&conn, "security_patch", Some("lodash"), "expired", None);
        insert_window(&conn, "security_patch", Some("lodash"), "expired", None);

        let outcomes = analyze_decision_window_outcomes(&conn);
        assert_eq!(outcomes.len(), 1, "Should have 1 outcome group");

        let outcome = &outcomes[0];
        assert_eq!(outcome.window_type, "security_patch");
        assert_eq!(outcome.dependency.as_deref(), Some("lodash"));
        assert_eq!(outcome.windows_acted, 3);
        assert_eq!(outcome.windows_expired, 2);
        assert_eq!(outcome.sample_size, 5);
        // response_rate = 3/5 = 0.6
        assert!(
            (outcome.response_rate - 0.6).abs() < 1e-6,
            "Expected response_rate 0.6, got {}",
            outcome.response_rate
        );
        // avg_lead_time = (12 + 24 + 6) / 3 = 14.0
        assert!(
            (outcome.avg_lead_time_hours - 14.0).abs() < 1e-6,
            "Expected avg_lead_time 14.0, got {}",
            outcome.avg_lead_time_hours
        );
    }

    #[test]
    fn test_confidence_scaling() {
        let conn = setup_test_db();

        // 1 sample -> confidence = sqrt(1/20) = 0.2236
        insert_window(&conn, "adoption", None, "acted", Some(48.0));
        let outcomes = analyze_decision_window_outcomes(&conn);
        assert_eq!(outcomes.len(), 1);
        let conf_1 = outcomes[0].confidence;
        assert!(
            (conf_1 - (1.0_f64 / 20.0).sqrt()).abs() < 1e-6,
            "1-sample confidence should be sqrt(1/20), got {}",
            conf_1
        );

        // Reset and test 20 samples -> confidence = sqrt(20/20) = 1.0
        let conn2 = setup_test_db();
        for _ in 0..20 {
            insert_window(&conn2, "adoption", None, "acted", Some(24.0));
        }
        let outcomes2 = analyze_decision_window_outcomes(&conn2);
        assert_eq!(outcomes2.len(), 1);
        assert!(
            (outcomes2[0].confidence - 1.0).abs() < 1e-6,
            "20-sample confidence should be 1.0, got {}",
            outcomes2[0].confidence
        );

        // 100 samples -> confidence = sqrt(100/20) = sqrt(5) > 1.0, clamped to 1.0
        let conn3 = setup_test_db();
        for i in 0..100 {
            let status = if i % 2 == 0 { "acted" } else { "expired" };
            insert_window(&conn3, "migration", Some("react"), status, Some(36.0));
        }
        let outcomes3 = analyze_decision_window_outcomes(&conn3);
        assert_eq!(outcomes3.len(), 1);
        assert!(
            (outcomes3[0].confidence - 1.0).abs() < 1e-6,
            "100-sample confidence should be clamped to 1.0, got {}",
            outcomes3[0].confidence
        );
    }

    #[test]
    fn test_store_supersession() {
        let conn = setup_test_db();

        // First round: 2 acted, 0 expired
        insert_window(&conn, "security_patch", Some("lodash"), "acted", Some(12.0));
        insert_window(&conn, "security_patch", Some("lodash"), "acted", Some(24.0));
        let outcomes_v1 = analyze_decision_window_outcomes(&conn);
        store_decision_outcomes(&conn, &outcomes_v1).expect("store v1");

        // Verify first storage
        let count_v1: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM digested_intelligence
                 WHERE digest_type = 'decision_outcome' AND subject = 'security_patch_lodash'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count_v1, 1, "Should have exactly 1 entry after first store");

        // Second round: add more windows and re-analyze
        insert_window(&conn, "security_patch", Some("lodash"), "expired", None);
        let outcomes_v2 = analyze_decision_window_outcomes(&conn);
        store_decision_outcomes(&conn, &outcomes_v2).expect("store v2");

        // Should still be exactly 1 (old one deleted, new one inserted)
        let count_v2: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM digested_intelligence
                 WHERE digest_type = 'decision_outcome' AND subject = 'security_patch_lodash'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count_v2, 1,
            "Should have exactly 1 entry after supersession (DELETE+INSERT)"
        );

        // Verify the latest data reflects updated stats (2 acted + 1 expired)
        let data_str: String = conn
            .query_row(
                "SELECT data FROM digested_intelligence
                 WHERE digest_type = 'decision_outcome' AND subject = 'security_patch_lodash'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let stored: DecisionWindowOutcome =
            serde_json::from_str(&data_str).expect("deserialize outcome");
        assert_eq!(stored.windows_acted, 2);
        assert_eq!(stored.windows_expired, 1);
        assert_eq!(stored.sample_size, 3);
    }
}
