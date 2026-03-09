//! Agent Session Brief — Tailored context briefs for AI agents connecting to 4DA
//!
//! Generates a structured summary of active decisions, ecosystem changes,
//! concerns, and open signals so that any AI agent can quickly understand
//! the developer's current technology landscape.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tracing::info;
use ts_rs::TS;

use crate::error::{Result, ResultExt};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AgentSessionBrief {
    pub generated_at: String,
    pub version: String,
    pub agent_type: Option<String>,
    pub active_decisions: Vec<DecisionSummary>,
    pub ecosystem_changes: Vec<EcosystemChange>,
    pub active_concerns: Vec<ActiveConcern>,
    pub open_signals: Vec<SignalSummary>,
    pub recent_briefing: Option<String>,
    pub agent_memories: Vec<crate::agent_memory::AgentMemoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DecisionSummary {
    pub id: i64,
    pub subject: String,
    pub decision: String,
    pub decision_type: String,
    pub confidence: f64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct EcosystemChange {
    pub change_type: String,
    pub subject: String,
    pub summary: String,
    pub relevance: f32,
    pub since: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ActiveConcern {
    pub subject: String,
    pub concern_type: String,
    pub summary: String,
    pub priority: String,
    pub related_decision_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct SignalSummary {
    pub title: String,
    pub source_type: String,
    pub signal_type: String,
    pub priority: String,
    pub url: Option<String>,
}

// ============================================================================
// Core Function
// ============================================================================

/// Generate a tailored session brief for an AI agent.
///
/// Aggregates active decisions, recent ecosystem changes (from signal-classified
/// source items), cross-referenced concerns, open signals, the latest AI
/// briefing text, and recent agent memories.
pub fn generate_brief(
    conn: &Connection,
    agent_type: Option<&str>,
    since: Option<&str>,
) -> Result<AgentSessionBrief> {
    let since_ts = since.unwrap_or("(last 24h)");
    let generated_at = chrono::Utc::now().to_rfc3339();

    info!(
        target: "4da::agent_brief",
        agent_type = ?agent_type,
        since = since_ts,
        "Generating agent session brief"
    );

    let active_decisions = query_active_decisions(conn)?;
    let ecosystem_changes = query_ecosystem_changes(conn, since)?;
    let active_concerns = build_active_concerns(conn, &active_decisions, since)?;
    let open_signals = query_open_signals(conn, since)?;
    let recent_briefing = crate::digest_commands::get_latest_briefing_text();

    let since_for_memories = since.unwrap_or("2000-01-01T00:00:00Z");
    let agent_memories =
        crate::agent_memory::get_memories_since(conn, since_for_memories, agent_type, 50)?;

    let brief = AgentSessionBrief {
        generated_at,
        version: "1.0.0".to_string(),
        agent_type: agent_type.map(|s| s.to_string()),
        active_decisions,
        ecosystem_changes,
        active_concerns,
        open_signals,
        recent_briefing,
        agent_memories,
    };

    info!(
        target: "4da::agent_brief",
        decisions = brief.active_decisions.len(),
        changes = brief.ecosystem_changes.len(),
        concerns = brief.active_concerns.len(),
        signals = brief.open_signals.len(),
        memories = brief.agent_memories.len(),
        "Agent brief generated"
    );

    Ok(brief)
}

// ============================================================================
// Query Helpers
// ============================================================================

/// Build a time-filtered SQL WHERE clause for source_items queries.
/// Returns (full_sql, needs_param) — when `since` is `Some`, the caller must
/// bind the timestamp as `?1`.
fn time_filtered_sql(select: &str, since: Option<&str>, limit: u32) -> (String, bool) {
    if since.is_some() {
        (
            format!(
                "{} FROM source_items WHERE created_at > ?1 ORDER BY created_at DESC LIMIT {}",
                select, limit
            ),
            true,
        )
    } else {
        (
            format!(
                "{} FROM source_items WHERE created_at > datetime('now', '-24 hours') ORDER BY created_at DESC LIMIT {}",
                select, limit
            ),
            false,
        )
    }
}

/// Execute a time-filtered query, applying `map_fn` to each row.
fn query_since<T, F>(
    conn: &Connection,
    select: &str,
    since: Option<&str>,
    limit: u32,
    map_fn: F,
) -> Result<Vec<T>>
where
    F: Fn(&rusqlite::Row) -> rusqlite::Result<Option<T>>,
{
    let (sql, needs_param) = time_filtered_sql(select, since, limit);
    let mut stmt = conn.prepare(&sql).context("Prepare error")?;

    let rows = if needs_param {
        stmt.query_map(params![since.unwrap_or("")], &map_fn)
    } else {
        stmt.query_map([], &map_fn)
    }
    .context("Query error")?;

    let mut results = Vec::new();
    for row in rows {
        if let Ok(Some(item)) = row {
            results.push(item);
        }
    }
    Ok(results)
}

/// Query active developer decisions, sorted by confidence DESC, limit 20.
fn query_active_decisions(conn: &Connection) -> Result<Vec<DecisionSummary>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, subject, decision, decision_type, confidence, status
             FROM developer_decisions
             WHERE status = 'active'
             ORDER BY confidence DESC
             LIMIT 20",
        )
        .context("Failed to prepare decisions query")?;

    let rows = stmt
        .query_map([], |row| {
            Ok(DecisionSummary {
                id: row.get(0)?,
                subject: row.get(1)?,
                decision: row.get(2)?,
                decision_type: row.get(3)?,
                confidence: row.get(4)?,
                status: row.get(5)?,
            })
        })
        .context("Failed to query decisions")?;

    let mut decisions = Vec::new();
    for row in rows {
        decisions.push(row.context("Decision row error")?);
    }
    Ok(decisions)
}

/// Classify text into an ecosystem change type and relevance score.
fn classify_change(title: &str, content: &str) -> Option<(String, f32)> {
    let text = format!("{} {}", title, content).to_lowercase();
    if text.contains("cve")
        || text.contains("vulnerability")
        || text.contains("security flaw")
        || text.contains("exploit")
    {
        Some(("security_alert".to_string(), 0.9))
    } else if text.contains("breaking change")
        || text.contains("deprecated")
        || text.contains("end of life")
        || text.contains("migration guide")
    {
        Some(("breaking_change".to_string(), 0.8))
    } else if text.contains("trending")
        || text.contains("adoption")
        || text.contains("state of")
        || text.contains("survey")
    {
        Some(("trend_shift".to_string(), 0.6))
    } else if text.contains("new release")
        || text.contains("just released")
        || text.contains("announcing")
        || text.contains("introducing")
    {
        Some(("new_version".to_string(), 0.7))
    } else {
        None
    }
}

/// Classify text into a signal type and priority.
fn classify_signal(title: &str, content: &str) -> Option<(String, String)> {
    let text = format!("{} {}", title, content).to_lowercase();
    if text.contains("cve") || text.contains("vulnerability") || text.contains("exploit") {
        Some(("security_alert".to_string(), "high".to_string()))
    } else if text.contains("breaking change") || text.contains("deprecated") {
        Some(("breaking_change".to_string(), "high".to_string()))
    } else if text.contains("trending") || text.contains("adoption") {
        Some(("trend_shift".to_string(), "medium".to_string()))
    } else if text.contains("new release") || text.contains("announcing") {
        Some(("new_version".to_string(), "medium".to_string()))
    } else {
        None
    }
}

/// Query source_items since the given timestamp that match signal patterns.
///
/// Maps signal types to ecosystem change categories:
///   SecurityAlert  -> "security_alert"
///   BreakingChange -> "breaking_change"
///   TechTrend      -> "trend_shift"
///   ToolDiscovery  -> "new_version"
fn query_ecosystem_changes(conn: &Connection, since: Option<&str>) -> Result<Vec<EcosystemChange>> {
    query_since(
        conn,
        "SELECT title, source_type, content, created_at",
        since,
        100,
        |row| {
            let title: String = row.get(0)?;
            let _source_type: String = row.get(1)?;
            let content: String = row.get(2)?;
            let created_at: String = row.get(3)?;

            Ok(
                classify_change(&title, &content).map(|(change_type, relevance)| EcosystemChange {
                    change_type,
                    subject: title,
                    summary: content.chars().take(200).collect(),
                    relevance,
                    since: created_at,
                }),
            )
        },
    )
}

/// Cross-reference open signals with active decisions.
///
/// If a source item's title or content mentions a decision's subject,
/// generate an `ActiveConcern` linking the two.
fn build_active_concerns(
    conn: &Connection,
    decisions: &[DecisionSummary],
    since: Option<&str>,
) -> Result<Vec<ActiveConcern>> {
    if decisions.is_empty() {
        return Ok(Vec::new());
    }

    struct ItemRow {
        title: String,
        content: String,
        source_type: String,
    }

    let items: Vec<ItemRow> = query_since(
        conn,
        "SELECT title, content, source_type",
        since,
        200,
        |row| {
            Ok(Some(ItemRow {
                title: row.get(0)?,
                content: row.get(1)?,
                source_type: row.get(2)?,
            }))
        },
    )?;

    let mut concerns = Vec::new();

    for item in &items {
        let text_lower = format!("{} {}", item.title, item.content).to_lowercase();

        for decision in decisions {
            let subject_lower = decision.subject.to_lowercase();
            if !text_lower.contains(&subject_lower) {
                continue;
            }

            let (concern_type, priority) = if text_lower.contains("cve")
                || text_lower.contains("vulnerability")
                || text_lower.contains("security")
            {
                ("security", "high")
            } else if text_lower.contains("breaking") || text_lower.contains("deprecated") {
                ("breaking_change", "high")
            } else if text_lower.contains("reconsider") || text_lower.contains("alternative") {
                ("decision_reconsider", "medium")
            } else {
                ("blind_spot", "low")
            };

            concerns.push(ActiveConcern {
                subject: item.title.clone(),
                concern_type: concern_type.to_string(),
                summary: format!(
                    "{} ({}) may affect decision: {}",
                    item.title, item.source_type, decision.subject
                ),
                priority: priority.to_string(),
                related_decision_id: Some(decision.id),
            });
        }
    }

    // Deduplicate by subject — keep the highest priority
    concerns.sort_by(|a, b| {
        let ord = |p: &str| -> u8 {
            match p {
                "critical" => 4,
                "high" => 3,
                "medium" => 2,
                _ => 1,
            }
        };
        ord(&b.priority).cmp(&ord(&a.priority))
    });
    concerns.dedup_by(|a, b| a.subject == b.subject);

    Ok(concerns)
}

/// Query recent source items that match signal patterns as open signals.
fn query_open_signals(conn: &Connection, since: Option<&str>) -> Result<Vec<SignalSummary>> {
    query_since(
        conn,
        "SELECT title, source_type, url, content",
        since,
        50,
        |row| {
            let title: String = row.get(0)?;
            let source_type: String = row.get(1)?;
            let url: Option<String> = row.get(2)?;
            let content: String = row.get(3)?;

            Ok(
                classify_signal(&title, &content).map(|(signal_type, priority)| SignalSummary {
                    title,
                    source_type,
                    signal_type,
                    priority,
                    url,
                }),
            )
        },
    )
}

// ============================================================================
// Tauri Command
// ============================================================================

#[tauri::command]
pub async fn generate_agent_brief(
    agent_type: Option<String>,
    since: Option<String>,
) -> Result<AgentSessionBrief> {
    let conn = crate::open_db_connection()?;
    let brief = generate_brief(&conn, agent_type.as_deref(), since.as_deref())?;
    Ok(brief)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        crate::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS developer_decisions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                decision_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                decision TEXT NOT NULL,
                rationale TEXT,
                alternatives_rejected TEXT DEFAULT '[]',
                context_tags TEXT DEFAULT '[]',
                confidence REAL NOT NULL DEFAULT 0.8,
                status TEXT NOT NULL DEFAULT 'active',
                superseded_by INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                url TEXT,
                title TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                content_hash TEXT NOT NULL,
                embedding BLOB NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_seen TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(source_type, source_id)
            );
            CREATE TABLE IF NOT EXISTS agent_memory (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                agent_type TEXT NOT NULL,
                memory_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                content TEXT NOT NULL,
                context_tags TEXT DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT,
                promoted_to_decision_id INTEGER
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_generate_brief_empty_db() {
        let conn = setup_test_db();
        let brief = generate_brief(&conn, None, None).unwrap();

        assert_eq!(brief.version, "1.0.0");
        assert!(brief.active_decisions.is_empty());
        assert!(brief.ecosystem_changes.is_empty());
        assert!(brief.active_concerns.is_empty());
        assert!(brief.open_signals.is_empty());
        assert!(brief.agent_memories.is_empty());
        assert!(brief.agent_type.is_none());
    }

    #[test]
    fn test_generate_brief_with_data() {
        let conn = setup_test_db();

        // Insert decisions
        conn.execute(
            "INSERT INTO developer_decisions (decision_type, subject, decision, confidence, status)
             VALUES ('tech_choice', 'sqlite', 'Use SQLite for local storage', 0.9, 'active')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO developer_decisions (decision_type, subject, decision, confidence, status)
             VALUES ('architecture', 'tauri', 'Use Tauri 2.0 for desktop app', 0.85, 'active')",
            [],
        )
        .unwrap();

        // Insert source items — one with a security signal about sqlite
        let empty_embedding = vec![0u8; 1536];
        conn.execute(
            "INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, embedding)
             VALUES ('hackernews', 'hn-001', 'https://example.com/cve',
                     'CVE-2026-9999: SQLite vulnerability found',
                     'A critical security flaw exploit was discovered in SQLite allowing remote access',
                     'hash1', ?1)",
            params![empty_embedding],
        ).unwrap();

        // Insert a non-signal item
        conn.execute(
            "INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, embedding)
             VALUES ('reddit', 'rd-001', 'https://reddit.com/r/rust',
                     'My weekend Rust project',
                     'Just a fun project I built this weekend', 'hash2', ?1)",
            params![empty_embedding],
        ).unwrap();

        // Insert agent memory
        conn.execute(
            "INSERT INTO agent_memory (session_id, agent_type, memory_type, subject, content, context_tags)
             VALUES ('sess-1', 'claude_code', 'discovery', 'sqlite optimization',
                     'WAL mode is better', '[\"database\"]')",
            [],
        ).unwrap();

        let brief = generate_brief(&conn, Some("claude_code"), None).unwrap();

        // Should have 2 active decisions
        assert_eq!(brief.active_decisions.len(), 2);
        assert_eq!(brief.active_decisions[0].subject, "sqlite");
        assert_eq!(brief.active_decisions[0].confidence, 0.9);
        assert_eq!(brief.active_decisions[1].subject, "tauri");

        // Should find the security-related ecosystem change
        assert!(
            !brief.ecosystem_changes.is_empty(),
            "Should detect security alert"
        );
        assert_eq!(brief.ecosystem_changes[0].change_type, "security_alert");

        // Should create an active concern linking CVE to sqlite decision
        assert!(
            !brief.active_concerns.is_empty(),
            "Should cross-reference CVE with decision"
        );
        let concern = &brief.active_concerns[0];
        assert_eq!(concern.concern_type, "security");
        assert_eq!(concern.priority, "high");
        assert!(concern.related_decision_id.is_some());

        // Should find the signal
        assert!(
            !brief.open_signals.is_empty(),
            "Should classify CVE as open signal"
        );
        assert_eq!(brief.open_signals[0].signal_type, "security_alert");

        // Agent type filter
        assert_eq!(brief.agent_type, Some("claude_code".to_string()));

        // Agent memories (filtered by agent_type)
        assert_eq!(brief.agent_memories.len(), 1);
        assert_eq!(brief.agent_memories[0].subject, "sqlite optimization");
    }
}
