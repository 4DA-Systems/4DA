// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Source Health Diagnostics for Blind Spots
//!
//! Queries the `feed_health` table to produce a per-adapter status summary
//! that explains WHY blind spots exist — adapter disabled, failing,
//! rate-limited, circuit-open, or simply stale.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::Result;

// ============================================================================
// Types
// ============================================================================

/// Per-adapter health status derived from the `feed_health` table.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AdapterStatus {
    pub source_type: String,
    pub feed_origin: String,
    /// One of: "healthy", "failing", "circuit_open", "stale"
    pub status: String,
    pub consecutive_failures: i64,
    pub last_success_at: Option<String>,
    pub last_failure_at: Option<String>,
    pub last_error: Option<String>,
}

/// Aggregate summary of all source adapter health.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct SourceHealthSummary {
    pub adapters: Vec<AdapterStatus>,
    pub total_active: usize,
    pub total_failing: usize,
    pub total_disabled: usize,
}

// ============================================================================
// Status Classification
// ============================================================================

/// Classify an adapter's status from its feed_health row data.
///
/// Priority order:
/// 1. `circuit_open` if the circuit breaker is tripped
/// 2. `failing` if consecutive_failures > 0 (not yet tripped)
/// 3. `stale` if last_success_at is >7 days ago
/// 4. `healthy` otherwise
pub fn classify_adapter_status(
    circuit_open: bool,
    consecutive_failures: i64,
    last_success_at: Option<&str>,
) -> &'static str {
    if circuit_open {
        return "circuit_open";
    }
    if consecutive_failures > 0 {
        return "failing";
    }
    if let Some(ts) = last_success_at {
        if is_stale(ts) {
            return "stale";
        }
    }
    "healthy"
}

/// Returns true if the given ISO timestamp is more than 7 days ago.
fn is_stale(iso_timestamp: &str) -> bool {
    let now = chrono::Utc::now().naive_utc();
    if let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(iso_timestamp, "%Y-%m-%d %H:%M:%S") {
        let diff = now - parsed;
        return diff.num_days() > 7;
    }
    // If we can't parse, treat as stale (unknown = conservative)
    true
}

// ============================================================================
// Query
// ============================================================================

/// Query all `feed_health` rows and produce a `SourceHealthSummary`.
pub fn get_source_health_summary(conn: &Connection) -> Result<SourceHealthSummary> {
    let mut stmt = conn.prepare(
        "SELECT feed_origin, source_type, consecutive_failures, \
                last_success_at, last_failure_at, last_error, circuit_open \
         FROM feed_health ORDER BY source_type, feed_origin",
    )?;

    let adapters: Vec<AdapterStatus> = stmt
        .query_map([], |row| {
            let feed_origin: String = row.get(0)?;
            let source_type: String = row.get(1)?;
            let consecutive_failures: i64 = row.get(2)?;
            let last_success_at: Option<String> = row.get(3)?;
            let last_failure_at: Option<String> = row.get(4)?;
            let last_error: Option<String> = row.get(5)?;
            let circuit_open_int: i64 = row.get(6)?;
            let circuit_open = circuit_open_int != 0;

            let status = classify_adapter_status(
                circuit_open,
                consecutive_failures,
                last_success_at.as_deref(),
            )
            .to_string();

            Ok(AdapterStatus {
                source_type,
                feed_origin,
                status,
                consecutive_failures,
                last_success_at,
                last_failure_at,
                last_error,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let total_active = adapters.iter().filter(|a| a.status == "healthy").count();
    let total_failing = adapters
        .iter()
        .filter(|a| a.status == "failing" || a.status == "circuit_open")
        .count();
    let total_disabled = adapters.iter().filter(|a| a.status == "stale").count();

    Ok(SourceHealthSummary {
        adapters,
        total_active,
        total_failing,
        total_disabled,
    })
}

// ============================================================================
// Tauri Command
// ============================================================================

#[tauri::command]
pub fn get_source_health() -> std::result::Result<SourceHealthSummary, String> {
    let conn = crate::open_db_connection().map_err(|e| e.to_string())?;
    get_source_health_summary(&conn).map_err(|e| e.to_string())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    /// Create an in-memory DB with the feed_health table.
    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE feed_health (
                feed_origin TEXT NOT NULL,
                source_type TEXT NOT NULL,
                consecutive_failures INTEGER NOT NULL DEFAULT 0,
                total_successes INTEGER NOT NULL DEFAULT 0,
                total_failures INTEGER NOT NULL DEFAULT 0,
                last_success_at TEXT,
                last_failure_at TEXT,
                last_error TEXT,
                circuit_open INTEGER NOT NULL DEFAULT 0,
                circuit_opened_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (feed_origin, source_type)
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_classify_status_priority() {
        // circuit_open takes highest priority
        assert_eq!(
            classify_adapter_status(true, 5, Some("2020-01-01 00:00:00")),
            "circuit_open"
        );
        // failing takes priority over stale
        assert_eq!(
            classify_adapter_status(false, 3, Some("2020-01-01 00:00:00")),
            "failing"
        );
        // stale when no failures but last success >7 days ago
        assert_eq!(
            classify_adapter_status(false, 0, Some("2020-01-01 00:00:00")),
            "stale"
        );
        // healthy when no failures and recent success
        let recent = chrono::Utc::now()
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        assert_eq!(classify_adapter_status(false, 0, Some(&recent)), "healthy");
        // healthy when no failures and no success timestamp (fresh entry)
        assert_eq!(classify_adapter_status(false, 0, None), "healthy");
    }

    #[test]
    fn test_empty_feed_health_table() {
        let conn = setup_test_db();
        let summary = get_source_health_summary(&conn).unwrap();
        assert!(summary.adapters.is_empty());
        assert_eq!(summary.total_active, 0);
        assert_eq!(summary.total_failing, 0);
        assert_eq!(summary.total_disabled, 0);
    }

    #[test]
    fn test_mixed_adapter_statuses() {
        let conn = setup_test_db();
        let now = chrono::Utc::now()
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        // Healthy adapter
        conn.execute(
            "INSERT INTO feed_health (feed_origin, source_type, consecutive_failures, last_success_at, circuit_open) \
             VALUES (?1, ?2, 0, ?3, 0)",
            params!["https://hn.com", "hackernews", &now],
        )
        .unwrap();

        // Failing adapter
        conn.execute(
            "INSERT INTO feed_health (feed_origin, source_type, consecutive_failures, last_failure_at, last_error, circuit_open) \
             VALUES (?1, ?2, 3, ?3, ?4, 0)",
            params![
                "https://reddit.com/r/rust",
                "reddit",
                &now,
                "rate limited"
            ],
        )
        .unwrap();

        // Circuit-open adapter
        conn.execute(
            "INSERT INTO feed_health (feed_origin, source_type, consecutive_failures, last_failure_at, last_error, circuit_open) \
             VALUES (?1, ?2, 5, ?3, ?4, 1)",
            params![
                "https://bad-rss.com/feed",
                "rss",
                &now,
                "connection refused"
            ],
        )
        .unwrap();

        // Stale adapter (last success >7 days ago)
        conn.execute(
            "INSERT INTO feed_health (feed_origin, source_type, consecutive_failures, last_success_at, circuit_open) \
             VALUES (?1, ?2, 0, ?3, 0)",
            params!["UCtest123", "youtube", "2020-01-01 00:00:00"],
        )
        .unwrap();

        let summary = get_source_health_summary(&conn).unwrap();
        assert_eq!(summary.adapters.len(), 4);
        assert_eq!(summary.total_active, 1, "one healthy");
        assert_eq!(summary.total_failing, 2, "one failing + one circuit_open");
        assert_eq!(summary.total_disabled, 1, "one stale");

        // Verify individual statuses
        let hn = summary
            .adapters
            .iter()
            .find(|a| a.source_type == "hackernews")
            .unwrap();
        assert_eq!(hn.status, "healthy");

        let reddit = summary
            .adapters
            .iter()
            .find(|a| a.source_type == "reddit")
            .unwrap();
        assert_eq!(reddit.status, "failing");
        assert_eq!(reddit.last_error.as_deref(), Some("rate limited"));

        let rss = summary
            .adapters
            .iter()
            .find(|a| a.source_type == "rss")
            .unwrap();
        assert_eq!(rss.status, "circuit_open");
        assert_eq!(rss.consecutive_failures, 5);

        let yt = summary
            .adapters
            .iter()
            .find(|a| a.source_type == "youtube")
            .unwrap();
        assert_eq!(yt.status, "stale");
    }

    #[test]
    fn test_stale_boundary() {
        // Exactly 7 days ago should NOT be stale (>7 required)
        let seven_days_ago = (chrono::Utc::now().naive_utc() - chrono::Duration::days(7))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        assert_eq!(
            classify_adapter_status(false, 0, Some(&seven_days_ago)),
            "healthy",
            "exactly 7 days should be healthy, stale requires >7"
        );

        // 8 days ago should be stale
        let eight_days_ago = (chrono::Utc::now().naive_utc() - chrono::Duration::days(8))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        assert_eq!(
            classify_adapter_status(false, 0, Some(&eight_days_ago)),
            "stale",
            "8 days old should be stale"
        );
    }
}
