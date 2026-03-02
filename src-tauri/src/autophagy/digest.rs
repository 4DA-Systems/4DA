//! Autophagy cycle orchestrator — runs all four analyzers and records metrics.
//!
//! This is the main entry point for an autophagy cycle. It:
//! 1. Counts items approaching pruning
//! 2. Runs calibration, topic decay, source autopsy, and anti-pattern detection
//! 3. Stores all intelligence to `digested_intelligence`
//! 4. Records the cycle in `autophagy_cycles`
//!
//! Each analyzer is fault-tolerant: failures produce warnings but don't abort the cycle.

use rusqlite::{params, Connection};
use std::time::Instant;
use tracing::{info, warn};

/// Run a full autophagy cycle: analyze content approaching pruning, extract intelligence,
/// then record cycle metrics.
///
/// `max_age_days` controls the pruning window: items between (max_age_days - 7) and
/// max_age_days old are analyzed for calibration. All recent items are analyzed for
/// topic decay, source quality, and anti-patterns.
///
/// Returns the cycle result with counts of all intelligence produced.
/// Pruning itself is NOT performed here; it happens via `db.cleanup_old_items()`.
pub(crate) fn run_autophagy_cycle(
    conn: &Connection,
    max_age_days: i64,
) -> Result<super::AutophagyCycleResult, String> {
    let start = Instant::now();
    info!(target: "4da::autophagy", max_age_days, "Starting autophagy cycle");

    // Get DB size before (page_count * page_size)
    let db_size_before: i64 = conn
        .query_row(
            "SELECT page_count * page_size FROM pragma_page_count, pragma_page_size",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // Count items in the pruning analysis window
    let window_end_days = max_age_days.saturating_sub(7);
    let items_analyzed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM source_items
             WHERE last_seen < datetime('now', ?1)",
            params![format!("-{} days", window_end_days)],
            |r| r.get(0),
        )
        .unwrap_or(0);

    info!(
        target: "4da::autophagy",
        items_analyzed,
        db_size_before_kb = db_size_before / 1024,
        "Autophagy pre-analysis stats"
    );

    // Run all 5 analyzers (each returns empty vec on failure, never panics)
    let calibrations = super::calibration::analyze_calibration(conn, max_age_days);
    let topic_calibrations =
        super::calibration_analysis::analyze_topic_calibration(conn, max_age_days);
    let decay_profiles = super::topic_decay::analyze_topic_decay(conn);
    let source_autopsies = super::source_autopsy::analyze_sources(conn, max_age_days);
    let anti_patterns = super::anti_patterns::detect_anti_patterns(conn, 0.35);

    // Store source-level calibration results
    let calibrations_produced = (calibrations.len() + topic_calibrations.len()) as i64;
    if !calibrations.is_empty() {
        if let Err(e) = super::calibration::store_calibrations(conn, &calibrations) {
            warn!(target: "4da::autophagy", error = %e, "Failed to store calibrations");
        }
    }
    // Store topic-level calibration results
    if !topic_calibrations.is_empty() {
        if let Err(e) = super::calibration::store_calibrations(conn, &topic_calibrations) {
            warn!(target: "4da::autophagy", error = %e, "Failed to store topic calibrations");
        }
    }

    // Store decay profiles
    let topic_decay_rates_updated = decay_profiles.len() as i64;
    if !decay_profiles.is_empty() {
        if let Err(e) = super::topic_decay::store_decay_profiles(conn, &decay_profiles) {
            warn!(target: "4da::autophagy", error = %e, "Failed to store decay profiles");
        }
    }

    // Store source autopsies
    let source_autopsies_produced = source_autopsies.len() as i64;
    if !source_autopsies.is_empty() {
        if let Err(e) = super::source_autopsy::store_source_autopsies(conn, &source_autopsies) {
            warn!(target: "4da::autophagy", error = %e, "Failed to store source autopsies");
        }
    }

    // Store anti-patterns
    let anti_patterns_detected = anti_patterns.len() as i64;
    if !anti_patterns.is_empty() {
        if let Err(e) = super::anti_patterns::store_anti_patterns(conn, &anti_patterns) {
            warn!(target: "4da::autophagy", error = %e, "Failed to store anti-patterns");
        }
    }

    // Bridge ACE behavior data into calibration system (topic-level accuracy feedback).
    // This analyzes implicit user signals (save, click, dismiss) from the ACE database
    // and produces per-topic calibration deltas that the scoring pipeline uses.
    let mut ace_calibrations_bridged: i64 = 0;
    if let Ok(ace) = crate::get_ace_engine() {
        let ace_conn = ace.get_conn().lock();
        match super::calibration_analysis::bridge_accuracy_feedback(&ace_conn, conn, max_age_days) {
            Ok(count) => {
                ace_calibrations_bridged = count as i64;
                info!(
                    target: "4da::autophagy",
                    count,
                    "ACE accuracy feedback bridged to calibration"
                );
            }
            Err(e) => {
                warn!(target: "4da::autophagy", error = %e, "ACE feedback bridge failed");
            }
        }
    }

    let total_calibrations = calibrations_produced + ace_calibrations_bridged;
    let duration_ms = start.elapsed().as_millis() as i64;

    // Record the cycle in autophagy_cycles table
    if let Err(e) = conn.execute(
        "INSERT INTO autophagy_cycles
            (items_analyzed, items_pruned, calibrations_produced,
             topic_decay_rates_updated, source_autopsies_produced,
             anti_patterns_detected, db_size_before_bytes, db_size_after_bytes, duration_ms)
         VALUES (?1, 0, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
        params![
            items_analyzed,
            total_calibrations,
            topic_decay_rates_updated,
            source_autopsies_produced,
            anti_patterns_detected,
            db_size_before,
            duration_ms,
        ],
    ) {
        warn!(target: "4da::autophagy", error = %e, "Failed to record autophagy cycle");
    }

    info!(
        target: "4da::autophagy",
        items_analyzed,
        calibrations_produced = total_calibrations,
        ace_calibrations_bridged,
        topic_decay_rates_updated,
        source_autopsies_produced,
        anti_patterns_detected,
        duration_ms,
        "Autophagy cycle complete"
    );

    Ok(super::AutophagyCycleResult {
        items_analyzed,
        items_pruned: 0, // Pruning happens separately via db.cleanup_old_items()
        calibrations_produced: total_calibrations,
        topic_decay_rates_updated,
        source_autopsies_produced,
        anti_patterns_detected,
        duration_ms,
    })
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
            "CREATE TABLE source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL DEFAULT '',
                url TEXT,
                title TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL DEFAULT '',
                content_hash TEXT NOT NULL DEFAULT '',
                embedding BLOB,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_seen TEXT NOT NULL DEFAULT (datetime('now')),
                summary TEXT,
                embedding_status TEXT DEFAULT 'pending',
                embed_text TEXT
            );
            CREATE TABLE feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER NOT NULL,
                relevant INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (source_item_id) REFERENCES source_items(id)
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
            );
            CREATE TABLE autophagy_cycles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                items_analyzed INTEGER NOT NULL DEFAULT 0,
                items_pruned INTEGER NOT NULL DEFAULT 0,
                calibrations_produced INTEGER NOT NULL DEFAULT 0,
                topic_decay_rates_updated INTEGER NOT NULL DEFAULT 0,
                source_autopsies_produced INTEGER NOT NULL DEFAULT 0,
                anti_patterns_detected INTEGER NOT NULL DEFAULT 0,
                db_size_before_bytes INTEGER NOT NULL DEFAULT 0,
                db_size_after_bytes INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .expect("create tables");
        conn
    }

    #[test]
    fn test_run_cycle_empty_db() {
        let conn = setup_test_db();
        let result = run_autophagy_cycle(&conn, 30).expect("cycle should succeed");

        assert_eq!(result.items_analyzed, 0);
        assert_eq!(result.items_pruned, 0);
        assert_eq!(result.calibrations_produced, 0);
        assert_eq!(result.topic_decay_rates_updated, 0);
        assert_eq!(result.source_autopsies_produced, 0);
        assert_eq!(result.anti_patterns_detected, 0);
        assert!(result.duration_ms >= 0);

        // Verify cycle was recorded
        let cycle_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM autophagy_cycles", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cycle_count, 1);
    }

    #[test]
    fn test_run_cycle_with_data() {
        let conn = setup_test_db();

        // Insert items (recent, not in pruning window)
        for i in 0..20 {
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, last_seen)
                 VALUES ('hackernews', ?1, ?2, datetime('now', '-2 days'))",
                params![format!("hn_{}", i), format!("Story {}", i)],
            )
            .unwrap();
        }

        // Add feedback for source autopsy and anti-pattern analysis
        for i in 1..=3 {
            conn.execute(
                "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 1)",
                params![i],
            )
            .unwrap();
        }

        let result = run_autophagy_cycle(&conn, 30).expect("cycle should succeed");

        // Items are only 2 days old, so nothing in the pruning window (23-30 days)
        assert_eq!(result.items_analyzed, 0);
        // But source autopsy should find 1 source type (hackernews)
        assert_eq!(result.source_autopsies_produced, 1);
        assert!(result.duration_ms >= 0);

        // Verify intelligence was stored
        let digest_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM digested_intelligence", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(digest_count >= 1);
    }

    #[test]
    fn test_cycle_records_to_autophagy_cycles() {
        let conn = setup_test_db();

        // Run two cycles
        run_autophagy_cycle(&conn, 30).expect("cycle 1");
        run_autophagy_cycle(&conn, 30).expect("cycle 2");

        let cycle_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM autophagy_cycles", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cycle_count, 2);
    }
}
