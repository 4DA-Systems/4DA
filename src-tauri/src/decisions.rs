// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Decision Memory — Developer Decision Intelligence for 4DA
//!
//! Records, retrieves, and checks alignment of developer decisions.
//! Decisions persist across sessions and inform signal classification,
//! technology radar, and AI agent context.

use crate::error::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tracing::info;
use ts_rs::TS;

#[path = "decisions_commands.rs"]
mod decisions_commands;
pub use decisions_commands::*;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum DecisionType {
    TechChoice,
    Architecture,
    Workflow,
    Pattern,
    Dependency,
}

impl DecisionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DecisionType::TechChoice => "tech_choice",
            DecisionType::Architecture => "architecture",
            DecisionType::Workflow => "workflow",
            DecisionType::Pattern => "pattern",
            DecisionType::Dependency => "dependency",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "tech_choice" => DecisionType::TechChoice,
            "architecture" => DecisionType::Architecture,
            "workflow" => DecisionType::Workflow,
            "pattern" => DecisionType::Pattern,
            "dependency" => DecisionType::Dependency,
            _ => DecisionType::TechChoice,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum DecisionStatus {
    Active,
    Superseded,
    Reconsidering,
}

impl DecisionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DecisionStatus::Active => "active",
            DecisionStatus::Superseded => "superseded",
            DecisionStatus::Reconsidering => "reconsidering",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "active" => DecisionStatus::Active,
            "superseded" => DecisionStatus::Superseded,
            "reconsidering" => DecisionStatus::Reconsidering,
            _ => DecisionStatus::Active,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DeveloperDecision {
    pub id: i64,
    pub decision_type: DecisionType,
    pub subject: String,
    pub decision: String,
    pub rationale: Option<String>,
    pub alternatives_rejected: Vec<String>,
    pub context_tags: Vec<String>,
    pub confidence: f64,
    pub status: DecisionStatus,
    pub superseded_by: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================================
// Core Functions
// ============================================================================

/// Record a new developer decision.
#[allow(clippy::too_many_arguments)]
pub fn record_decision(
    conn: &Connection,
    decision_type: &DecisionType,
    subject: &str,
    decision: &str,
    rationale: Option<&str>,
    alternatives_rejected: &[String],
    context_tags: &[String],
    confidence: f64,
) -> Result<i64> {
    let alts_json =
        serde_json::to_string(alternatives_rejected).unwrap_or_else(|_| "[]".to_string());
    let tags_json = serde_json::to_string(context_tags).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO developer_decisions (decision_type, subject, decision, rationale, alternatives_rejected, context_tags, confidence, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active')",
        params![
            decision_type.as_str(),
            subject,
            decision,
            rationale,
            alts_json,
            tags_json,
            confidence,
        ],
    )?;

    let id = conn.last_insert_rowid();
    info!(target: "4da::decisions", id = id, subject = subject, "Decision recorded");
    Ok(id)
}

/// Get a single decision by ID.
///
/// Test-only read-back helper: the UI lists decisions (`get_decisions`) and the
/// MCP server queries the DB itself, so nothing in production fetches one by id.
#[cfg(test)]
pub fn get_decision(conn: &Connection, id: i64) -> Result<Option<DeveloperDecision>> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row(
            "SELECT id, decision_type, subject, decision, rationale, alternatives_rejected, context_tags, confidence, status, superseded_by, created_at, updated_at
             FROM developer_decisions WHERE id = ?1",
            params![id],
            |row| Ok(row_to_decision(row)),
        )
        .optional()?)
}

/// List decisions with optional type and status filter.
pub fn list_decisions(
    conn: &Connection,
    decision_type: Option<&DecisionType>,
    status: Option<&DecisionStatus>,
    limit: usize,
) -> Result<Vec<DeveloperDecision>> {
    let mut sql = String::from(
        "SELECT id, decision_type, subject, decision, rationale, alternatives_rejected, context_tags, confidence, status, superseded_by, created_at, updated_at
         FROM developer_decisions WHERE 1=1",
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(dt) = decision_type {
        sql.push_str(" AND decision_type = ?");
        param_values.push(Box::new(dt.as_str().to_string()));
    }
    if let Some(st) = status {
        sql.push_str(" AND status = ?");
        param_values.push(Box::new(st.as_str().to_string()));
    }
    sql.push_str(" ORDER BY updated_at DESC LIMIT ?");
    param_values.push(Box::new(limit as i64));

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values
        .iter()
        .map(std::convert::AsRef::as_ref)
        .collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_ref.as_slice(), |row| Ok(row_to_decision(row)))?;

    let mut decisions = Vec::new();
    for row in rows {
        decisions.push(row?);
    }
    Ok(decisions)
}

/// Update a decision's fields.
pub fn update_decision(
    conn: &Connection,
    id: i64,
    decision: Option<&str>,
    rationale: Option<&str>,
    status: Option<&DecisionStatus>,
    confidence: Option<f64>,
) -> Result<()> {
    let mut sets = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(d) = decision {
        sets.push("decision = ?");
        param_values.push(Box::new(d.to_string()));
    }
    if let Some(r) = rationale {
        sets.push("rationale = ?");
        param_values.push(Box::new(r.to_string()));
    }
    if let Some(s) = status {
        sets.push("status = ?");
        param_values.push(Box::new(s.as_str().to_string()));
    }
    if let Some(c) = confidence {
        sets.push("confidence = ?");
        param_values.push(Box::new(c));
    }

    if sets.is_empty() {
        return Ok(());
    }

    sets.push("updated_at = datetime('now')");

    let sql = format!(
        "UPDATE developer_decisions SET {} WHERE id = ?",
        sets.join(", ")
    );
    param_values.push(Box::new(id));

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values
        .iter()
        .map(std::convert::AsRef::as_ref)
        .collect();

    conn.execute(&sql, params_ref.as_slice())?;

    info!(target: "4da::decisions", id = id, "Decision updated");
    Ok(())
}

// ============================================================================
// Helpers
// ============================================================================

fn row_to_decision(row: &rusqlite::Row) -> DeveloperDecision {
    let alts_str: String = row.get::<_, String>(5).unwrap_or_else(|_| "[]".to_string());
    let tags_str: String = row.get::<_, String>(6).unwrap_or_else(|_| "[]".to_string());

    DeveloperDecision {
        id: row.get(0).unwrap_or(0),
        decision_type: DecisionType::from_str(&row.get::<_, String>(1).unwrap_or_default()),
        subject: row.get(2).unwrap_or_default(),
        decision: row.get(3).unwrap_or_default(),
        rationale: row.get(4).ok(),
        alternatives_rejected: serde_json::from_str(&alts_str).unwrap_or_default(),
        context_tags: serde_json::from_str(&tags_str).unwrap_or_default(),
        confidence: row.get(7).unwrap_or(0.8),
        status: DecisionStatus::from_str(&row.get::<_, String>(8).unwrap_or_default()),
        superseded_by: row.get(9).ok().flatten(),
        created_at: row.get(10).unwrap_or_default(),
        updated_at: row.get(11).unwrap_or_default(),
    }
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
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (superseded_by) REFERENCES developer_decisions(id)
            );
            CREATE TABLE IF NOT EXISTS tech_stack (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                technology TEXT NOT NULL UNIQUE
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_record_and_get_decision() {
        let conn = setup_test_db();
        let id = record_decision(
            &conn,
            &DecisionType::TechChoice,
            "sqlite",
            "Use SQLite for local storage",
            Some("Local-first principle"),
            &["postgresql".to_string()],
            &["database".to_string(), "storage".to_string()],
            0.9,
        )
        .unwrap();

        let decision = get_decision(&conn, id).unwrap().unwrap();
        assert_eq!(decision.subject, "sqlite");
        assert_eq!(decision.decision, "Use SQLite for local storage");
        assert_eq!(decision.alternatives_rejected, vec!["postgresql"]);
        assert_eq!(decision.confidence, 0.9);
        assert_eq!(decision.status, DecisionStatus::Active);
    }

    #[test]
    fn test_list_decisions_with_filter() {
        let conn = setup_test_db();
        record_decision(
            &conn,
            &DecisionType::TechChoice,
            "rust",
            "Use Rust",
            None,
            &[],
            &[],
            0.9,
        )
        .unwrap();
        record_decision(
            &conn,
            &DecisionType::Architecture,
            "modular",
            "Modular arch",
            None,
            &[],
            &[],
            0.8,
        )
        .unwrap();

        let all = list_decisions(&conn, None, None, 50).unwrap();
        assert_eq!(all.len(), 2);

        let tech_only = list_decisions(&conn, Some(&DecisionType::TechChoice), None, 50).unwrap();
        assert_eq!(tech_only.len(), 1);
        assert_eq!(tech_only[0].subject, "rust");
    }

    #[test]
    fn test_seed_decisions_from_profile() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO tech_stack (technology) VALUES ('rust')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO tech_stack (technology) VALUES ('typescript')",
            [],
        )
        .unwrap();

        let seeded = seed_decisions_from_profile(&conn).unwrap();
        assert_eq!(seeded, 2);

        // Should not re-seed
        let seeded2 = seed_decisions_from_profile(&conn).unwrap();
        assert_eq!(seeded2, 0);

        let decisions = list_decisions(&conn, None, None, 50).unwrap();
        assert_eq!(decisions.len(), 2);
        assert!(decisions.iter().all(|d| d.confidence == 0.6));
    }

    #[test]
    fn test_update_decision() {
        let conn = setup_test_db();
        let id = record_decision(
            &conn,
            &DecisionType::TechChoice,
            "react",
            "Use React",
            None,
            &[],
            &[],
            0.8,
        )
        .unwrap();

        update_decision(
            &conn,
            id,
            Some("Use React 19"),
            Some("New features needed"),
            Some(&DecisionStatus::Reconsidering),
            Some(0.6),
        )
        .unwrap();

        let updated = get_decision(&conn, id).unwrap().unwrap();
        assert_eq!(updated.decision, "Use React 19");
        assert_eq!(updated.rationale, Some("New features needed".to_string()));
        assert_eq!(updated.status, DecisionStatus::Reconsidering);
        assert_eq!(updated.confidence, 0.6);
    }
}
