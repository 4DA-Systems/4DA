//! Agent Memory — Cross-Agent Shared Memory for 4DA
//!
//! Enables AI agents to store and recall memories across sessions.
//! Memories can be promoted to developer decisions for permanent tracking.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tracing::info;
use ts_rs::TS;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum MemoryType {
    Discovery,
    Decision,
    Context,
    Warning,
    Preference,
}

impl MemoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryType::Discovery => "discovery",
            MemoryType::Decision => "decision",
            MemoryType::Context => "context",
            MemoryType::Warning => "warning",
            MemoryType::Preference => "preference",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "discovery" => MemoryType::Discovery,
            "decision" => MemoryType::Decision,
            "context" => MemoryType::Context,
            "warning" => MemoryType::Warning,
            "preference" => MemoryType::Preference,
            _ => MemoryType::Context,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AgentMemoryEntry {
    pub id: i64,
    pub session_id: String,
    pub agent_type: String,
    pub memory_type: MemoryType,
    pub subject: String,
    pub content: String,
    pub context_tags: Vec<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub promoted_to_decision_id: Option<i64>,
}

// ============================================================================
// Core Functions
// ============================================================================

/// Store a new agent memory entry.
#[allow(clippy::too_many_arguments)]
pub fn store_memory(
    conn: &Connection,
    session_id: &str,
    agent_type: &str,
    memory_type: &MemoryType,
    subject: &str,
    content: &str,
    context_tags: &[String],
    expires_at: Option<&str>,
) -> Result<i64, String> {
    let tags_json = serde_json::to_string(context_tags).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO agent_memory (session_id, agent_type, memory_type, subject, content, context_tags, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            session_id,
            agent_type,
            memory_type.as_str(),
            subject,
            content,
            tags_json,
            expires_at,
        ],
    )
    .map_err(|e| format!("Failed to store memory: {}", e))?;

    let id = conn.last_insert_rowid();
    info!(target: "4da::agent_memory", id = id, agent = agent_type, subject = subject, "Memory stored");
    Ok(id)
}

/// Recall memories by subject and optional agent type filter.
pub fn recall_memories(
    conn: &Connection,
    subject: &str,
    agent_type: Option<&str>,
    limit: usize,
) -> Result<Vec<AgentMemoryEntry>, String> {
    let pattern = format!("%{}%", subject.to_lowercase());
    let mut sql = String::from(
        "SELECT id, session_id, agent_type, memory_type, subject, content, context_tags, created_at, expires_at, promoted_to_decision_id
         FROM agent_memory
         WHERE (LOWER(subject) LIKE ?1 OR LOWER(context_tags) LIKE ?1)
         AND (expires_at IS NULL OR expires_at > datetime('now'))",
    );

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(pattern)];

    if let Some(at) = agent_type {
        sql.push_str(" AND agent_type = ?");
        param_values.push(Box::new(at.to_string()));
    }

    sql.push_str(" ORDER BY created_at DESC LIMIT ?");
    param_values.push(Box::new(limit as i64));

    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare: {}", e))?;
    let rows = stmt
        .query_map(params_ref.as_slice(), |row| Ok(row_to_memory(row)))
        .map_err(|e| format!("Failed to recall memories: {}", e))?;

    let mut memories = Vec::new();
    for row in rows {
        memories.push(row.map_err(|e| format!("Row error: {}", e))?);
    }
    Ok(memories)
}

/// Get memories since a timestamp, optionally filtered by agent type.
pub fn get_memories_since(
    conn: &Connection,
    since: &str,
    agent_type: Option<&str>,
    limit: usize,
) -> Result<Vec<AgentMemoryEntry>, String> {
    let mut sql = String::from(
        "SELECT id, session_id, agent_type, memory_type, subject, content, context_tags, created_at, expires_at, promoted_to_decision_id
         FROM agent_memory WHERE created_at > ?1",
    );

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(since.to_string())];

    if let Some(at) = agent_type {
        sql.push_str(" AND agent_type = ?");
        param_values.push(Box::new(at.to_string()));
    }

    sql.push_str(" ORDER BY created_at DESC LIMIT ?");
    param_values.push(Box::new(limit as i64));

    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare: {}", e))?;
    let rows = stmt
        .query_map(params_ref.as_slice(), |row| Ok(row_to_memory(row)))
        .map_err(|e| format!("Failed to get memories since: {}", e))?;

    let mut memories = Vec::new();
    for row in rows {
        memories.push(row.map_err(|e| format!("Row error: {}", e))?);
    }
    Ok(memories)
}

/// Promote an agent memory to a developer decision.
pub fn promote_to_decision(conn: &Connection, memory_id: i64) -> Result<i64, String> {
    // Get the memory
    let memory = conn
        .query_row(
            "SELECT id, subject, content, context_tags FROM agent_memory WHERE id = ?1",
            params![memory_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("Failed to get memory: {}", e))?
        .ok_or_else(|| format!("Memory {} not found", memory_id))?;

    let (_, subject, content, tags_str) = memory;
    let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

    // Create a decision from this memory
    let decision_id = crate::decisions::record_decision(
        conn,
        &crate::decisions::DecisionType::TechChoice,
        &subject,
        &content,
        Some("Promoted from agent memory"),
        &[],
        &tags,
        0.7,
    )
    .map_err(|e| e.to_string())?;

    // Mark memory as promoted
    conn.execute(
        "UPDATE agent_memory SET promoted_to_decision_id = ?1 WHERE id = ?2",
        params![decision_id, memory_id],
    )
    .map_err(|e| format!("Failed to mark memory as promoted: {}", e))?;

    info!(target: "4da::agent_memory", memory_id = memory_id, decision_id = decision_id, "Memory promoted to decision");
    Ok(decision_id)
}

/// Clean up expired memories.
pub fn cleanup_expired(conn: &Connection) -> Result<usize, String> {
    let deleted = conn
        .execute(
            "DELETE FROM agent_memory WHERE expires_at IS NOT NULL AND expires_at <= datetime('now')",
            [],
        )
        .map_err(|e| format!("Failed to cleanup expired: {}", e))?;

    if deleted > 0 {
        info!(target: "4da::agent_memory", deleted = deleted, "Expired memories cleaned up");
    }
    Ok(deleted)
}

// ============================================================================
// Helpers
// ============================================================================

fn row_to_memory(row: &rusqlite::Row) -> AgentMemoryEntry {
    let tags_str: String = row.get::<_, String>(6).unwrap_or_else(|_| "[]".to_string());

    AgentMemoryEntry {
        id: row.get(0).unwrap_or(0),
        session_id: row.get(1).unwrap_or_default(),
        agent_type: row.get(2).unwrap_or_default(),
        memory_type: MemoryType::from_str(&row.get::<_, String>(3).unwrap_or_default()),
        subject: row.get(4).unwrap_or_default(),
        content: row.get(5).unwrap_or_default(),
        context_tags: serde_json::from_str(&tags_str).unwrap_or_default(),
        created_at: row.get(7).unwrap_or_default(),
        expires_at: row.get(8).ok().flatten(),
        promoted_to_decision_id: row.get(9).ok().flatten(),
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn store_agent_memory(
    session_id: String,
    agent_type: String,
    memory_type: String,
    subject: String,
    content: String,
    context_tags: Option<Vec<String>>,
    expires_at: Option<String>,
) -> Result<i64, String> {
    let conn = crate::open_db_connection()?;
    store_memory(
        &conn,
        &session_id,
        &agent_type,
        &MemoryType::from_str(&memory_type),
        &subject,
        &content,
        &context_tags.unwrap_or_default(),
        expires_at.as_deref(),
    )
}

#[tauri::command]
pub async fn recall_agent_memories(
    subject: String,
    agent_type: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<AgentMemoryEntry>, String> {
    let conn = crate::open_db_connection()?;
    recall_memories(&conn, &subject, agent_type.as_deref(), limit.unwrap_or(20))
}

#[tauri::command]
pub async fn promote_memory_to_decision(memory_id: i64) -> Result<i64, String> {
    let conn = crate::open_db_connection()?;
    promote_to_decision(&conn, memory_id)
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
            "CREATE TABLE IF NOT EXISTS agent_memory (
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
            );
            CREATE TABLE IF NOT EXISTS developer_decisions (
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
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_store_and_recall() {
        let conn = setup_test_db();
        let id = store_memory(
            &conn,
            "session-1",
            "claude_code",
            &MemoryType::Discovery,
            "sqlite optimization",
            "WAL mode improves concurrent read performance significantly",
            &["database".to_string(), "performance".to_string()],
            None,
        )
        .unwrap();

        assert!(id > 0);

        let memories = recall_memories(&conn, "sqlite", None, 10).unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].subject, "sqlite optimization");
        assert_eq!(memories[0].agent_type, "claude_code");
    }

    #[test]
    fn test_cross_agent_recall() {
        let conn = setup_test_db();
        store_memory(
            &conn,
            "s1",
            "claude_code",
            &MemoryType::Context,
            "auth approach",
            "Using JWT with refresh tokens",
            &["auth".to_string()],
            None,
        )
        .unwrap();

        // Different agent can recall
        let memories = recall_memories(&conn, "auth", None, 10).unwrap();
        assert_eq!(memories.len(), 1);

        // Filter by agent type
        let claude_only = recall_memories(&conn, "auth", Some("claude_code"), 10).unwrap();
        assert_eq!(claude_only.len(), 1);

        let cursor_only = recall_memories(&conn, "auth", Some("cursor"), 10).unwrap();
        assert_eq!(cursor_only.len(), 0);
    }

    #[test]
    fn test_cleanup_expired() {
        let conn = setup_test_db();
        // Insert an already-expired memory
        conn.execute(
            "INSERT INTO agent_memory (session_id, agent_type, memory_type, subject, content, expires_at)
             VALUES ('s1', 'test', 'context', 'old', 'expired content', datetime('now', '-1 hour'))",
            [],
        ).unwrap();
        // Insert a non-expired memory
        store_memory(
            &conn,
            "s1",
            "test",
            &MemoryType::Context,
            "fresh",
            "still valid",
            &[],
            None,
        )
        .unwrap();

        let deleted = cleanup_expired(&conn).unwrap();
        assert_eq!(deleted, 1);

        let remaining = recall_memories(&conn, "fresh", None, 10).unwrap();
        assert_eq!(remaining.len(), 1);
    }
}
