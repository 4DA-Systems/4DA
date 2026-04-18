// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Compound Advantage Score — running metric of pipeline value.
//!
//! Weights:
//! - Window response rate:     0.30  (acted / (acted + expired))
//! - Average lead time:        0.20  (hours of advance warning, capped at 168h)
//! - Calibration accuracy:     0.25  (from autophagy digested_intelligence)
//! - Knowledge gap closure:    0.15  (knowledge windows acted / 5, capped at 1.0)
//! - Items surfaced:           0.10  (log-scaled, ln(items) / ln(1000))

use rusqlite::{params, Connection};
use tracing::{debug, warn};

use super::CompoundAdvantageScore;

// ============================================================================
// Weight constants
// ============================================================================

const W_RESPONSE_RATE: f32 = 0.30;
const W_LEAD_TIME: f32 = 0.20;
const W_CALIBRATION: f32 = 0.25;
const W_KNOWLEDGE: f32 = 0.15;
const W_SURFACED: f32 = 0.10;

/// Maximum lead time (hours) that yields a perfect 1.0 sub-score.
const MAX_LEAD_TIME_HOURS: f32 = 168.0; // 1 week

/// Knowledge gap closure normalization target (5 gaps = 1.0 sub-score).
const KNOWLEDGE_NORMALIZATION: f32 = 5.0;

/// Items surfaced log-scale denominator (ln(1000) ~ 6.9).
const SURFACED_LOG_BASE: f32 = 6.9078; // ln(1000)

// ============================================================================
// Public API
// ============================================================================

/// Compute the compound advantage score for the given period.
///
/// Queries window metrics, calibration data, and item counts, then computes
/// a weighted composite score (0-100). Stores the result in `advantage_score`
/// and computes a trend against the previous period's score.
pub(crate) fn compute_compound_score(conn: &Connection, period: &str) -> CompoundAdvantageScore {
    let period_days = match period {
        "daily" => 1,
        "weekly" => 7,
        "monthly" => 30,
        _ => 7,
    };
    let since = format!("-{period_days} days");

    // ---- Window metrics ----

    let windows_opened = query_count(
        conn,
        "SELECT COUNT(*) FROM decision_windows WHERE opened_at > datetime('now', ?1)",
        &since,
    );

    let windows_acted = query_count(
        conn,
        "SELECT COUNT(*) FROM decision_windows WHERE status = 'acted' AND acted_at > datetime('now', ?1)",
        &since,
    );

    let windows_expired = query_count(
        conn,
        "SELECT COUNT(*) FROM decision_windows WHERE status = 'expired' AND closed_at > datetime('now', ?1)",
        &since,
    );

    // Response rate: acted / (acted + expired). Default 0.5 when no data.
    let response_rate = {
        let total = windows_acted + windows_expired;
        if total > 0 {
            windows_acted as f32 / total as f32
        } else {
            0.5
        }
    };

    // ---- Average lead time ----

    let avg_lead_time: f32 = conn
        .query_row(
            "SELECT COALESCE(AVG(lead_time_hours), 0.0) FROM decision_windows \
             WHERE status = 'acted' AND acted_at > datetime('now', ?1)",
            params![&since],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    // Higher lead time = more advantage (capped at MAX_LEAD_TIME_HOURS)
    let lead_time_score = (avg_lead_time / MAX_LEAD_TIME_HOURS).min(1.0);

    // ---- Calibration accuracy from autophagy ----

    let calibration_accuracy: f32 = conn
        .query_row(
            "SELECT COALESCE(AVG(confidence), 0.0) FROM digested_intelligence \
             WHERE digest_type = 'calibration' AND superseded_by IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    // ---- Knowledge gaps closed ----

    let knowledge_gaps_closed = query_count(
        conn,
        "SELECT COUNT(*) FROM decision_windows WHERE window_type = 'knowledge' AND status = 'acted' AND acted_at > datetime('now', ?1)",
        &since,
    );

    let kg_score = (knowledge_gaps_closed as f32 / KNOWLEDGE_NORMALIZATION).min(1.0);

    // ---- Items surfaced (log-scaled) ----

    let items_surfaced = query_count(
        conn,
        "SELECT COUNT(*) FROM source_items WHERE created_at > datetime('now', ?1)",
        &since,
    );

    let surfaced_score = if items_surfaced > 0 {
        ((items_surfaced as f32).ln() / SURFACED_LOG_BASE).min(1.0)
    } else {
        0.0
    };

    // ---- Weighted composite (0-100) ----

    let score = (response_rate * W_RESPONSE_RATE
        + lead_time_score * W_LEAD_TIME
        + calibration_accuracy * W_CALIBRATION
        + kg_score * W_KNOWLEDGE
        + surfaced_score * W_SURFACED)
        * 100.0;

    // ---- Trend vs previous period ----

    let prev_score: f32 = conn
        .query_row(
            "SELECT COALESCE(score, 0.0) FROM advantage_score \
             WHERE period = ?1 ORDER BY computed_at DESC LIMIT 1",
            params![period],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    let trend = if prev_score > 0.0 {
        ((score - prev_score) / prev_score).clamp(-1.0, 1.0)
    } else if score > 0.0 {
        1.0
    } else {
        0.0
    };

    // ---- Persist the computed score ----

    if let Err(e) = conn.execute(
        "INSERT INTO advantage_score \
         (period, score, items_surfaced, avg_lead_time_hours, windows_opened, \
          windows_acted, windows_expired, knowledge_gaps_closed, calibration_accuracy) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            period,
            score,
            items_surfaced,
            avg_lead_time,
            windows_opened,
            windows_acted,
            windows_expired,
            knowledge_gaps_closed,
            calibration_accuracy,
        ],
    ) {
        warn!(target: "4da::decision_advantage", error = %e, "Failed to persist advantage score");
    }

    debug!(
        target: "4da::decision_advantage",
        score,
        period,
        response_rate,
        lead_time_score,
        calibration_accuracy,
        kg_score,
        surfaced_score,
        trend,
        "Compound advantage score computed"
    );

    CompoundAdvantageScore {
        score,
        period: period.to_string(),
        items_surfaced,
        avg_lead_time_hours: avg_lead_time,
        windows_opened,
        windows_acted,
        windows_expired,
        knowledge_gaps_closed,
        calibration_accuracy,
        trend,
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Execute a COUNT(*) query with a single text parameter. Returns 0 on error.
fn query_count(conn: &Connection, sql: &str, param: &str) -> i64 {
    conn.query_row(sql, params![param], |r| r.get(0))
        .unwrap_or(0)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA: &str = "
        CREATE TABLE source_items (id INTEGER PRIMARY KEY AUTOINCREMENT, source_type TEXT DEFAULT 'test', source_id TEXT DEFAULT '', url TEXT, title TEXT DEFAULT '', content TEXT DEFAULT '', content_hash TEXT DEFAULT '', created_at TEXT DEFAULT (datetime('now')), last_seen TEXT DEFAULT (datetime('now')));
        CREATE TABLE decision_windows (id INTEGER PRIMARY KEY AUTOINCREMENT, window_type TEXT NOT NULL, title TEXT NOT NULL, description TEXT DEFAULT '', urgency REAL DEFAULT 0.5, relevance REAL DEFAULT 0.5, source_item_ids TEXT DEFAULT '[]', signal_chain_id INTEGER, dependency TEXT, status TEXT DEFAULT 'open', opened_at TEXT DEFAULT (datetime('now')), expires_at TEXT, acted_at TEXT, closed_at TEXT, outcome TEXT, lead_time_hours REAL, streets_engine TEXT);
        CREATE TABLE advantage_score (id INTEGER PRIMARY KEY AUTOINCREMENT, period TEXT NOT NULL, score REAL DEFAULT 0.0, items_surfaced INTEGER DEFAULT 0, avg_lead_time_hours REAL DEFAULT 0.0, windows_opened INTEGER DEFAULT 0, windows_acted INTEGER DEFAULT 0, windows_expired INTEGER DEFAULT 0, knowledge_gaps_closed INTEGER DEFAULT 0, calibration_accuracy REAL DEFAULT 0.0, computed_at TEXT DEFAULT (datetime('now')));
        CREATE TABLE digested_intelligence (id INTEGER PRIMARY KEY AUTOINCREMENT, digest_type TEXT NOT NULL, subject TEXT NOT NULL, data TEXT NOT NULL, confidence REAL DEFAULT 0.5, sample_size INTEGER DEFAULT 0, created_at TEXT DEFAULT (datetime('now')), expires_at TEXT, superseded_by INTEGER);
    ";

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(SCHEMA).unwrap();
        c
    }

    #[test]
    fn test_empty_database_returns_baseline_score() {
        let score = compute_compound_score(&db(), "weekly");
        // response_rate=0.5 default, all others 0 => 0.5*0.30*100 = 15.0
        assert!(
            (score.score - 15.0).abs() < 0.1,
            "baseline ~15.0, got {}",
            score.score
        );
        assert_eq!(score.period, "weekly");
        assert_eq!(score.windows_opened, 0);
        assert!(
            (score.trend - 1.0).abs() < f32::EPSILON,
            "first score trend +1.0"
        );
    }

    #[test]
    fn test_score_increases_with_acted_windows() {
        let conn = db();
        conn.execute("INSERT INTO decision_windows (window_type, title, status, acted_at, lead_time_hours, opened_at) VALUES ('security_patch', 'Fix', 'acted', datetime('now'), 48.0, datetime('now', '-2 days'))", []).unwrap();
        conn.execute("INSERT INTO decision_windows (window_type, title, status, acted_at, lead_time_hours, opened_at) VALUES ('migration', 'Upgrade', 'acted', datetime('now'), 72.0, datetime('now', '-3 days'))", []).unwrap();
        for i in 0..10 {
            conn.execute(
                "INSERT INTO source_items (source_type, title, content) VALUES ('test', ?1, 'c')",
                params![format!("Item {i}")],
            )
            .unwrap();
        }
        let score = compute_compound_score(&conn, "weekly");
        assert!(
            score.score > 15.0,
            "acted windows boost score, got {}",
            score.score
        );
        assert_eq!(score.windows_acted, 2);
        assert!((score.avg_lead_time_hours - 60.0).abs() < 0.1);
    }

    #[test]
    fn test_score_persisted_and_trend_calculated() {
        let conn = db();
        let first = compute_compound_score(&conn, "weekly");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM advantage_score WHERE period='weekly'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        conn.execute("INSERT INTO decision_windows (window_type, title, status, acted_at, lead_time_hours, opened_at) VALUES ('security_patch', 'Fix2', 'acted', datetime('now'), 100.0, datetime('now', '-4 days'))", []).unwrap();
        let second = compute_compound_score(&conn, "weekly");
        assert!(second.score != first.score || second.trend != 0.0);
    }
}
