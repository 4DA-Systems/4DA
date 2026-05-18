// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.

//! Briefing Seals — compound temporal memory for the intelligence briefing.
//!
//! Instead of reprocessing historical content each briefing cycle, previous
//! days are "sealed" into summary nodes. Today's briefing builds on sealed
//! summaries + new content, enabling compound understanding without cost.
//!
//! Hierarchy: daily → weekly (7 daily) → monthly (4 weekly).

use rusqlite::{params, Connection};
use tracing::{debug, info};

// ============================================================================
// Constants
// ============================================================================

const MAX_DAILY_SEAL_TOKENS: usize = 500;
const MAX_WEEKLY_SEAL_TOKENS: usize = 200;
const MAX_MONTHLY_SEAL_TOKENS: usize = 100;
const WEEKLY_ROLLUP_COUNT: usize = 7;
const MONTHLY_ROLLUP_COUNT: usize = 4;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct BriefingSeal {
    pub seal_id: String,
    pub seal_date: String,
    pub seal_level: SealLevel,
    pub parent_seal_id: Option<String>,
    pub summary_text: String,
    pub item_count: i64,
    pub top_topics: Vec<String>,
    pub token_count: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealLevel {
    Daily = 0,
    Weekly = 1,
    Monthly = 2,
}

impl SealLevel {
    fn from_i64(v: i64) -> Self {
        match v {
            1 => Self::Weekly,
            2 => Self::Monthly,
            _ => Self::Daily,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContinuitySignal {
    pub signal_type: ContinuityType,
    pub topic: String,
    pub days_running: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuityType {
    DevelopingStory,
    EmergingSignal,
    Faded,
}

// ============================================================================
// Seal Creation
// ============================================================================

pub fn create_daily_seal(
    conn: &Connection,
    date: &str,
    summary_text: &str,
    item_count: i64,
    top_topics: &[String],
) -> Option<String> {
    let seal_id = format!("daily-{date}");
    let now = now_unix();
    let topics_json = serde_json::to_string(top_topics).unwrap_or_else(|_| "[]".to_string());
    let token_estimate = summary_text.split_whitespace().count() as i64;

    if let Err(e) = conn.execute(
        "INSERT OR REPLACE INTO briefing_seals (seal_id, seal_date, seal_level, parent_seal_id, summary_text, item_count, top_topics, token_count, created_at)
         VALUES (?1, ?2, 0, NULL, ?3, ?4, ?5, ?6, ?7)",
        params![seal_id, date, summary_text, item_count, topics_json, token_estimate, now],
    ) {
        debug!(target: "4da::seals", error = %e, "Failed to create daily seal");
        return None;
    }

    info!(target: "4da::seals", date = date, items = item_count, topics = topics_json, "Daily seal created");
    Some(seal_id)
}

pub fn create_rollup_seal(
    conn: &Connection,
    level: SealLevel,
    date: &str,
    summary_text: &str,
    child_seal_ids: &[String],
) -> Option<String> {
    let prefix = match level {
        SealLevel::Weekly => "weekly",
        SealLevel::Monthly => "monthly",
        SealLevel::Daily => return None,
    };
    let seal_id = format!("{prefix}-{date}");
    let now = now_unix();

    // Aggregate topics from child seals
    let mut all_topics: Vec<String> = Vec::new();
    let mut total_items: i64 = 0;
    for child_id in child_seal_ids {
        if let Ok(seal) = get_seal(conn, child_id) {
            all_topics.extend(seal.top_topics);
            total_items += seal.item_count;
        }
    }
    // Deduplicate and keep top topics by frequency
    let topic_counts = count_topics(&all_topics);
    let top_topics: Vec<String> = topic_counts.into_iter().take(10).map(|(t, _)| t).collect();
    let topics_json = serde_json::to_string(&top_topics).unwrap_or_else(|_| "[]".to_string());
    let token_estimate = summary_text.split_whitespace().count() as i64;

    if let Err(e) = conn.execute(
        "INSERT OR REPLACE INTO briefing_seals (seal_id, seal_date, seal_level, parent_seal_id, summary_text, item_count, top_topics, token_count, created_at)
         VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8)",
        params![seal_id, date, level as i64, summary_text, total_items, topics_json, token_estimate, now],
    ) {
        debug!(target: "4da::seals", error = %e, "Failed to create rollup seal");
        return None;
    }

    // Link child seals to parent
    for child_id in child_seal_ids {
        let _ = conn.execute(
            "UPDATE briefing_seals SET parent_seal_id = ?1 WHERE seal_id = ?2",
            params![seal_id, child_id],
        );
    }

    info!(target: "4da::seals", level = prefix, date = date, children = child_seal_ids.len(), "Rollup seal created");
    Some(seal_id)
}

// ============================================================================
// Rollup Detection
// ============================================================================

pub fn pending_weekly_rollup(conn: &Connection) -> Vec<Vec<String>> {
    let daily_seals = get_unrolled_daily_seals(conn);
    if daily_seals.len() < WEEKLY_ROLLUP_COUNT {
        return Vec::new();
    }
    daily_seals
        .chunks(WEEKLY_ROLLUP_COUNT)
        .filter(|chunk| chunk.len() == WEEKLY_ROLLUP_COUNT)
        .map(|chunk| chunk.iter().map(|s| s.seal_id.clone()).collect())
        .collect()
}

pub fn pending_monthly_rollup(conn: &Connection) -> Vec<Vec<String>> {
    let weekly_seals = get_unrolled_weekly_seals(conn);
    if weekly_seals.len() < MONTHLY_ROLLUP_COUNT {
        return Vec::new();
    }
    weekly_seals
        .chunks(MONTHLY_ROLLUP_COUNT)
        .filter(|chunk| chunk.len() == MONTHLY_ROLLUP_COUNT)
        .map(|chunk| chunk.iter().map(|s| s.seal_id.clone()).collect())
        .collect()
}

// ============================================================================
// Briefing Context Injection
// ============================================================================

pub fn build_seal_context(conn: &Connection) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Yesterday's daily seal
    if let Some(seal) = get_most_recent_seal(conn, SealLevel::Daily) {
        let truncated = truncate_to_tokens(&seal.summary_text, MAX_DAILY_SEAL_TOKENS);
        parts.push(format!(
            "Yesterday's briefing summary ({}):\n{}",
            seal.seal_date, truncated
        ));
    }

    // Most recent weekly seal
    if let Some(seal) = get_most_recent_seal(conn, SealLevel::Weekly) {
        let truncated = truncate_to_tokens(&seal.summary_text, MAX_WEEKLY_SEAL_TOKENS);
        parts.push(format!(
            "This week's summary ({}):\n{}",
            seal.seal_date, truncated
        ));
    }

    // Most recent monthly seal
    if let Some(seal) = get_most_recent_seal(conn, SealLevel::Monthly) {
        let truncated = truncate_to_tokens(&seal.summary_text, MAX_MONTHLY_SEAL_TOKENS);
        parts.push(format!(
            "Last month's summary ({}):\n{}",
            seal.seal_date, truncated
        ));
    }

    if parts.is_empty() {
        return String::new();
    }

    format!(
        "\n\nPrevious briefing context (for continuity — reference trends, developing stories, and patterns):\n{}",
        parts.join("\n\n")
    )
}

// ============================================================================
// Continuity Detection
// ============================================================================

pub fn detect_continuity(conn: &Connection, today_topics: &[String]) -> Vec<ContinuitySignal> {
    let mut signals = Vec::new();

    let recent_seals = get_recent_daily_seals(conn, 7);
    if recent_seals.is_empty() {
        return signals;
    }

    let yesterday_topics: std::collections::HashSet<String> = recent_seals
        .first()
        .map(|s| s.top_topics.iter().cloned().collect())
        .unwrap_or_default();

    let all_recent_topics: std::collections::HashSet<String> = recent_seals
        .iter()
        .flat_map(|s| s.top_topics.iter().cloned())
        .collect();

    let today_set: std::collections::HashSet<String> =
        today_topics.iter().map(|t| t.to_lowercase()).collect();

    // Developing story: topic in both today and yesterday
    for topic in &today_set {
        if yesterday_topics.contains(topic) {
            let days = recent_seals
                .iter()
                .take_while(|s| s.top_topics.iter().any(|t| t == topic))
                .count() as i64
                + 1;
            signals.push(ContinuitySignal {
                signal_type: ContinuityType::DevelopingStory,
                topic: topic.clone(),
                days_running: days,
            });
        }
    }

    // Emerging signal: new topic not in any recent seal
    for topic in &today_set {
        if !all_recent_topics.contains(topic) && topic.len() >= 3 {
            signals.push(ContinuitySignal {
                signal_type: ContinuityType::EmergingSignal,
                topic: topic.clone(),
                days_running: 1,
            });
        }
    }

    // Faded: topic in yesterday but not today
    for topic in &yesterday_topics {
        if !today_set.contains(topic) {
            signals.push(ContinuitySignal {
                signal_type: ContinuityType::Faded,
                topic: topic.clone(),
                days_running: 0,
            });
        }
    }

    signals
}

// ============================================================================
// Queries
// ============================================================================

fn get_seal(conn: &Connection, seal_id: &str) -> Result<BriefingSeal, rusqlite::Error> {
    conn.query_row(
        "SELECT seal_id, seal_date, seal_level, parent_seal_id, summary_text, item_count, top_topics, token_count, created_at
         FROM briefing_seals WHERE seal_id = ?1",
        params![seal_id],
        |row| row_to_seal(row),
    )
}

fn get_most_recent_seal(conn: &Connection, level: SealLevel) -> Option<BriefingSeal> {
    let mut stmt = conn
        .prepare(
            "SELECT seal_id, seal_date, seal_level, parent_seal_id, summary_text, item_count, top_topics, token_count, created_at
             FROM briefing_seals
             WHERE seal_level = ?1
             ORDER BY seal_date DESC
             LIMIT 1",
        )
        .ok()?;

    stmt.query_row(params![level as i64], |row| row_to_seal(row))
        .ok()
}

fn get_recent_daily_seals(conn: &Connection, limit: usize) -> Vec<BriefingSeal> {
    let mut stmt = match conn.prepare(
        "SELECT seal_id, seal_date, seal_level, parent_seal_id, summary_text, item_count, top_topics, token_count, created_at
         FROM briefing_seals
         WHERE seal_level = 0
         ORDER BY seal_date DESC
         LIMIT ?1",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map(params![limit as i64], |row| row_to_seal(row))
        .ok()
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
}

fn get_unrolled_daily_seals(conn: &Connection) -> Vec<BriefingSeal> {
    let mut stmt = match conn.prepare(
        "SELECT seal_id, seal_date, seal_level, parent_seal_id, summary_text, item_count, top_topics, token_count, created_at
         FROM briefing_seals
         WHERE seal_level = 0 AND parent_seal_id IS NULL
         ORDER BY seal_date ASC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map([], |row| row_to_seal(row))
        .ok()
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
}

fn get_unrolled_weekly_seals(conn: &Connection) -> Vec<BriefingSeal> {
    let mut stmt = match conn.prepare(
        "SELECT seal_id, seal_date, seal_level, parent_seal_id, summary_text, item_count, top_topics, token_count, created_at
         FROM briefing_seals
         WHERE seal_level = 1 AND parent_seal_id IS NULL
         ORDER BY seal_date ASC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map([], |row| row_to_seal(row))
        .ok()
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
}

pub fn get_seal_count(conn: &Connection) -> (i64, i64, i64) {
    let count = |level: i64| -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM briefing_seals WHERE seal_level = ?1",
            params![level],
            |row| row.get(0),
        )
        .unwrap_or(0)
    };
    (count(0), count(1), count(2))
}

// ============================================================================
// Helpers
// ============================================================================

fn row_to_seal(row: &rusqlite::Row) -> Result<BriefingSeal, rusqlite::Error> {
    let topics_json: String = row.get(6)?;
    let top_topics: Vec<String> = serde_json::from_str(&topics_json).unwrap_or_default();

    Ok(BriefingSeal {
        seal_id: row.get(0)?,
        seal_date: row.get(1)?,
        seal_level: SealLevel::from_i64(row.get(2)?),
        parent_seal_id: row.get(3)?,
        summary_text: row.get(4)?,
        item_count: row.get(5)?,
        top_topics,
        token_count: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn truncate_to_tokens(text: &str, max_tokens: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= max_tokens {
        return text.to_string();
    }
    words[..max_tokens].join(" ") + "..."
}

fn count_topics(topics: &[String]) -> Vec<(String, usize)> {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for t in topics {
        *counts.entry(t.to_lowercase()).or_default() += 1;
    }
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE briefing_seals (
                seal_id TEXT PRIMARY KEY,
                seal_date TEXT NOT NULL,
                seal_level INTEGER NOT NULL DEFAULT 0,
                parent_seal_id TEXT,
                summary_text TEXT NOT NULL,
                item_count INTEGER NOT NULL,
                top_topics TEXT NOT NULL DEFAULT '[]',
                token_count INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_create_daily_seal() {
        let conn = setup_db();
        let topics = vec!["rust".to_string(), "tauri".to_string()];
        let id = create_daily_seal(
            &conn,
            "2026-05-18",
            "Rust 1.80 released, Tauri 2.1 beta",
            5,
            &topics,
        );
        assert_eq!(id, Some("daily-2026-05-18".to_string()));

        let seal = get_seal(&conn, "daily-2026-05-18").unwrap();
        assert_eq!(seal.item_count, 5);
        assert_eq!(seal.top_topics, topics);
        assert_eq!(seal.seal_level, SealLevel::Daily);
    }

    #[test]
    fn test_weekly_rollup_detection() {
        let conn = setup_db();
        for i in 1..=7 {
            create_daily_seal(
                &conn,
                &format!("2026-05-{:02}", i),
                &format!("Day {i} summary"),
                3,
                &["rust".to_string()],
            );
        }
        let pending = pending_weekly_rollup(&conn);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].len(), 7);
    }

    #[test]
    fn test_no_rollup_below_threshold() {
        let conn = setup_db();
        for i in 1..=5 {
            create_daily_seal(
                &conn,
                &format!("2026-05-{:02}", i),
                &format!("Day {i} summary"),
                2,
                &["rust".to_string()],
            );
        }
        let pending = pending_weekly_rollup(&conn);
        assert!(pending.is_empty());
    }

    #[test]
    fn test_create_rollup_seal() {
        let conn = setup_db();
        let child_ids: Vec<String> = (1..=7)
            .map(|i| {
                let date = format!("2026-05-{:02}", i);
                create_daily_seal(
                    &conn,
                    &date,
                    &format!("Day {i}"),
                    3,
                    &["rust".to_string(), "wasm".to_string()],
                )
                .unwrap()
            })
            .collect();

        let weekly_id = create_rollup_seal(
            &conn,
            SealLevel::Weekly,
            "2026-05-07",
            "Week summary: Rust and WebAssembly dominated",
            &child_ids,
        );
        assert!(weekly_id.is_some());

        // Children should now be linked to parent
        let child = get_seal(&conn, &child_ids[0]).unwrap();
        assert_eq!(child.parent_seal_id.as_deref(), Some("weekly-2026-05-07"));

        // No more pending rollups
        let pending = pending_weekly_rollup(&conn);
        assert!(pending.is_empty());
    }

    #[test]
    fn test_build_seal_context_empty() {
        let conn = setup_db();
        let ctx = build_seal_context(&conn);
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_build_seal_context_with_data() {
        let conn = setup_db();
        create_daily_seal(
            &conn,
            "2026-05-17",
            "Rust dominated yesterday",
            5,
            &["rust".to_string()],
        );
        let ctx = build_seal_context(&conn);
        assert!(ctx.contains("Yesterday's briefing summary"));
        assert!(ctx.contains("Rust dominated yesterday"));
    }

    #[test]
    fn test_continuity_developing_story() {
        let conn = setup_db();
        create_daily_seal(
            &conn,
            "2026-05-17",
            "Rust release",
            3,
            &["rust".to_string(), "tauri".to_string()],
        );
        create_daily_seal(
            &conn,
            "2026-05-16",
            "Rust progress",
            3,
            &["rust".to_string()],
        );

        let signals = detect_continuity(&conn, &["rust".to_string(), "wasm".to_string()]);
        let developing: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == ContinuityType::DevelopingStory)
            .collect();
        let emerging: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == ContinuityType::EmergingSignal)
            .collect();

        assert_eq!(developing.len(), 1);
        assert_eq!(developing[0].topic, "rust");
        assert!(developing[0].days_running >= 2);

        assert_eq!(emerging.len(), 1);
        assert_eq!(emerging[0].topic, "wasm");
    }

    #[test]
    fn test_continuity_faded() {
        let conn = setup_db();
        create_daily_seal(
            &conn,
            "2026-05-17",
            "AI news",
            3,
            &["ai".to_string(), "rust".to_string()],
        );

        let signals = detect_continuity(&conn, &["rust".to_string()]);
        let faded: Vec<_> = signals
            .iter()
            .filter(|s| s.signal_type == ContinuityType::Faded)
            .collect();
        assert_eq!(faded.len(), 1);
        assert_eq!(faded[0].topic, "ai");
    }

    #[test]
    fn test_truncate_to_tokens() {
        let text = "word ".repeat(100);
        let truncated = truncate_to_tokens(&text, 10);
        assert!(truncated.ends_with("..."));
        assert!(truncated.split_whitespace().count() <= 11);
    }

    #[test]
    fn test_seal_count() {
        let conn = setup_db();
        create_daily_seal(&conn, "2026-05-17", "Day", 1, &[]);
        create_daily_seal(&conn, "2026-05-16", "Day", 1, &[]);
        let (daily, weekly, monthly) = get_seal_count(&conn);
        assert_eq!(daily, 2);
        assert_eq!(weekly, 0);
        assert_eq!(monthly, 0);
    }
}
