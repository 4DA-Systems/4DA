//! Decision Memory — Developer Decision Intelligence for 4DA
//!
//! Records, retrieves, and checks alignment of developer decisions.
//! Decisions persist across sessions and inform signal classification,
//! technology radar, and AI agent context.

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

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AlignmentResult {
    pub aligned: bool,
    pub relevant_decisions: Vec<DeveloperDecision>,
    pub conflicts: Vec<AlignmentConflict>,
    pub confidence: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AlignmentConflict {
    pub decision_id: i64,
    pub decision_subject: String,
    pub conflict_reason: String,
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
) -> Result<i64, String> {
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
    )
    .map_err(|e| format!("Failed to record decision: {}", e))?;

    let id = conn.last_insert_rowid();
    info!(target: "4da::decisions", id = id, subject = subject, "Decision recorded");
    Ok(id)
}

/// Get a single decision by ID.
#[allow(dead_code)]
pub fn get_decision(conn: &Connection, id: i64) -> Result<Option<DeveloperDecision>, String> {
    conn.query_row(
        "SELECT id, decision_type, subject, decision, rationale, alternatives_rejected, context_tags, confidence, status, superseded_by, created_at, updated_at
         FROM developer_decisions WHERE id = ?1",
        params![id],
        |row| Ok(row_to_decision(row)),
    )
    .optional()
    .map_err(|e| format!("Failed to get decision: {}", e))
}

/// List decisions with optional type and status filter.
pub fn list_decisions(
    conn: &Connection,
    decision_type: Option<&DecisionType>,
    status: Option<&DecisionStatus>,
    limit: usize,
) -> Result<Vec<DeveloperDecision>, String> {
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

    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare: {}", e))?;
    let rows = stmt
        .query_map(params_ref.as_slice(), |row| Ok(row_to_decision(row)))
        .map_err(|e| format!("Failed to list decisions: {}", e))?;

    let mut decisions = Vec::new();
    for row in rows {
        decisions.push(row.map_err(|e| format!("Row error: {}", e))?);
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
) -> Result<(), String> {
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

    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    conn.execute(&sql, params_ref.as_slice())
        .map_err(|e| format!("Failed to update decision: {}", e))?;

    info!(target: "4da::decisions", id = id, "Decision updated");
    Ok(())
}

/// Supersede an old decision with a new one.
#[allow(dead_code)]
pub fn supersede_decision(conn: &Connection, old_id: i64, new_id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE developer_decisions SET status = 'superseded', superseded_by = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![new_id, old_id],
    )
    .map_err(|e| format!("Failed to supersede decision: {}", e))?;

    info!(target: "4da::decisions", old = old_id, new = new_id, "Decision superseded");
    Ok(())
}

/// Find decisions by subject (fuzzy match on subject + context_tags).
#[allow(dead_code)]
pub fn find_decisions_by_subject(
    conn: &Connection,
    subject: &str,
    limit: usize,
) -> Result<Vec<DeveloperDecision>, String> {
    let pattern = format!("%{}%", subject.to_lowercase());
    let mut stmt = conn
        .prepare(
            "SELECT id, decision_type, subject, decision, rationale, alternatives_rejected, context_tags, confidence, status, superseded_by, created_at, updated_at
             FROM developer_decisions
             WHERE status = 'active'
             AND (LOWER(subject) LIKE ?1 OR LOWER(context_tags) LIKE ?1)
             ORDER BY confidence DESC
             LIMIT ?2",
        )
        .map_err(|e| format!("Failed to prepare: {}", e))?;

    let rows = stmt
        .query_map(params![pattern, limit as i64], |row| {
            Ok(row_to_decision(row))
        })
        .map_err(|e| format!("Failed to find decisions: {}", e))?;

    let mut decisions = Vec::new();
    for row in rows {
        decisions.push(row.map_err(|e| format!("Row error: {}", e))?);
    }
    Ok(decisions)
}

/// Check alignment of a technology/pattern against active decisions.
/// This is the critical function that AI agents call before suggesting changes.
#[allow(dead_code)]
pub fn check_alignment(
    conn: &Connection,
    technology: &str,
    pattern: Option<&str>,
) -> Result<AlignmentResult, String> {
    let search = technology.to_lowercase();
    let pattern_search = pattern.map(|p| p.to_lowercase());

    // Find all active decisions that relate to this technology
    let mut stmt = conn
        .prepare(
            "SELECT id, decision_type, subject, decision, rationale, alternatives_rejected, context_tags, confidence, status, superseded_by, created_at, updated_at
             FROM developer_decisions
             WHERE status = 'active'
             AND (LOWER(subject) LIKE ?1 OR LOWER(context_tags) LIKE ?1 OR LOWER(alternatives_rejected) LIKE ?1)",
        )
        .map_err(|e| format!("Failed to prepare: {}", e))?;

    let like_pattern = format!("%{}%", search);
    let rows = stmt
        .query_map(params![like_pattern], |row| Ok(row_to_decision(row)))
        .map_err(|e| format!("Failed to check alignment: {}", e))?;

    let mut relevant_decisions = Vec::new();
    let mut conflicts = Vec::new();

    for row in rows {
        let decision = row.map_err(|e| format!("Row error: {}", e))?;

        // Check if this technology was explicitly rejected
        let rejected = decision
            .alternatives_rejected
            .iter()
            .any(|alt| alt.to_lowercase().contains(&search));

        if rejected {
            conflicts.push(AlignmentConflict {
                decision_id: decision.id,
                decision_subject: decision.subject.clone(),
                conflict_reason: format!(
                    "'{}' was rejected in favor of '{}' (rationale: {})",
                    technology,
                    decision.decision,
                    decision.rationale.as_deref().unwrap_or("none")
                ),
            });
        }

        relevant_decisions.push(decision);
    }

    // Also check pattern alignment if provided
    if let Some(pat) = &pattern_search {
        let pat_like = format!("%{}%", pat);
        let mut stmt2 = conn
            .prepare(
                "SELECT id, decision_type, subject, decision, rationale, alternatives_rejected, context_tags, confidence, status, superseded_by, created_at, updated_at
                 FROM developer_decisions
                 WHERE status = 'active'
                 AND decision_type IN ('architecture', 'pattern')
                 AND (LOWER(subject) LIKE ?1 OR LOWER(decision) LIKE ?1)",
            )
            .map_err(|e| format!("Failed to prepare pattern query: {}", e))?;

        let rows2 = stmt2
            .query_map(params![pat_like], |row| Ok(row_to_decision(row)))
            .map_err(|e| format!("Failed to check pattern alignment: {}", e))?;

        for row in rows2 {
            let decision = row.map_err(|e| format!("Row error: {}", e))?;
            if !relevant_decisions.iter().any(|d| d.id == decision.id) {
                relevant_decisions.push(decision);
            }
        }
    }

    let aligned = conflicts.is_empty();
    let confidence = if relevant_decisions.is_empty() {
        0.5 // No data: neutral
    } else if aligned {
        relevant_decisions
            .iter()
            .map(|d| d.confidence)
            .fold(0.0_f64, f64::max)
    } else {
        // Confidence of the conflicting decision
        conflicts
            .iter()
            .filter_map(|c| relevant_decisions.iter().find(|d| d.id == c.decision_id))
            .map(|d| d.confidence)
            .fold(0.0_f64, f64::max)
    };

    Ok(AlignmentResult {
        aligned,
        relevant_decisions,
        conflicts,
        confidence,
    })
}

/// Auto-seed decisions from tech_stack on first run.
/// Creates TechChoice decisions with confidence=0.6 for each tech_stack entry.
pub fn seed_decisions_from_profile(conn: &Connection) -> Result<usize, String> {
    // Only seed if table is empty
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM developer_decisions", [], |row| {
            row.get(0)
        })
        .map_err(|e| format!("Failed to count decisions: {}", e))?;

    if count > 0 {
        return Ok(0);
    }

    // Get tech_stack entries
    let mut stmt = conn
        .prepare("SELECT technology FROM tech_stack")
        .map_err(|e| format!("Failed to read tech_stack: {}", e))?;

    let techs: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to query tech_stack: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    if techs.is_empty() {
        return Ok(0);
    }

    let mut seeded = 0;
    for tech in &techs {
        record_decision(
            conn,
            &DecisionType::TechChoice,
            tech,
            &format!("Using {} as part of primary tech stack", tech),
            Some("Inferred from project setup"),
            &[],
            &[tech.to_lowercase()],
            0.6,
        )?;
        seeded += 1;
    }

    info!(target: "4da::decisions", count = seeded, "Auto-seeded decisions from tech_stack");
    Ok(seeded)
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
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn get_decisions(
    decision_type: Option<String>,
    status: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<DeveloperDecision>, String> {
    let conn = crate::open_db_connection()?;
    let dt = decision_type.map(|s| DecisionType::from_str(&s));
    let st = status.map(|s| DecisionStatus::from_str(&s));
    list_decisions(&conn, dt.as_ref(), st.as_ref(), limit.unwrap_or(50))
}

#[tauri::command]
pub async fn record_developer_decision(
    decision_type: String,
    subject: String,
    decision: String,
    rationale: Option<String>,
    alternatives_rejected: Option<Vec<String>>,
    context_tags: Option<Vec<String>>,
    confidence: Option<f64>,
) -> Result<i64, String> {
    let conn = crate::open_db_connection()?;
    record_decision(
        &conn,
        &DecisionType::from_str(&decision_type),
        &subject,
        &decision,
        rationale.as_deref(),
        &alternatives_rejected.unwrap_or_default(),
        &context_tags.unwrap_or_default(),
        confidence.unwrap_or(0.8),
    )
}

#[tauri::command]
pub async fn update_developer_decision(
    id: i64,
    decision: Option<String>,
    rationale: Option<String>,
    status: Option<String>,
    confidence: Option<f64>,
) -> Result<(), String> {
    let conn = crate::open_db_connection()?;
    let st = status.map(|s| DecisionStatus::from_str(&s));
    update_decision(
        &conn,
        id,
        decision.as_deref(),
        rationale.as_deref(),
        st.as_ref(),
        confidence,
    )
}

/// Remove a technology from tech_stack, explicit_interests, and supersede its decision.
/// Used when ACE scanner detects a technology the user doesn't actually use.
/// Cleans all three locations where a falsely-detected tech can persist.
#[tauri::command]
pub async fn remove_tech_decision(technology: String) -> Result<(), String> {
    let conn = crate::open_db_connection()?;

    // 1. Remove from tech_stack (primary tech storage)
    conn.execute(
        "DELETE FROM tech_stack WHERE technology = ?1",
        params![technology],
    )
    .map_err(|e| format!("Failed to remove technology from tech_stack: {}", e))?;

    // 2. Remove from explicit_interests (may have been auto-seeded by ACE)
    conn.execute(
        "DELETE FROM explicit_interests WHERE topic = ?1",
        params![technology],
    )
    .map_err(|e| format!("Failed to remove from interests: {}", e))?;

    // 3. Supersede any matching active tech_choice decision
    conn.execute(
        "UPDATE developer_decisions SET status = 'superseded', updated_at = datetime('now') WHERE subject = ?1 AND decision_type = 'tech_choice' AND status = 'active'",
        params![technology],
    )
    .map_err(|e| format!("Failed to supersede tech decision: {}", e))?;

    info!(target: "4da::decisions", technology = %technology, "Technology fully removed: tech_stack + interests + decision superseded");
    Ok(())
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
    fn test_check_alignment_conflict() {
        let conn = setup_test_db();
        record_decision(
            &conn,
            &DecisionType::TechChoice,
            "sqlite",
            "Use SQLite for local storage",
            Some("Local-first principle"),
            &["postgresql".to_string(), "mysql".to_string()],
            &["database".to_string()],
            0.9,
        )
        .unwrap();

        // Querying rejected tech should show conflict
        let result = check_alignment(&conn, "postgresql", None).unwrap();
        assert!(!result.aligned);
        assert_eq!(result.conflicts.len(), 1);
        assert!(result.conflicts[0].conflict_reason.contains("rejected"));

        // Querying chosen tech should be aligned
        let result2 = check_alignment(&conn, "sqlite", None).unwrap();
        assert!(result2.aligned);
        assert!(!result2.relevant_decisions.is_empty());
    }

    #[test]
    fn test_supersede_decision() {
        let conn = setup_test_db();
        let old_id = record_decision(
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
        let new_id = record_decision(
            &conn,
            &DecisionType::TechChoice,
            "svelte",
            "Use Svelte",
            None,
            &["react".to_string()],
            &[],
            0.9,
        )
        .unwrap();

        supersede_decision(&conn, old_id, new_id).unwrap();

        let old = get_decision(&conn, old_id).unwrap().unwrap();
        assert_eq!(old.status, DecisionStatus::Superseded);
        assert_eq!(old.superseded_by, Some(new_id));
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
    fn test_find_decisions_by_subject() {
        let conn = setup_test_db();
        record_decision(
            &conn,
            &DecisionType::TechChoice,
            "sqlite",
            "Use SQLite",
            None,
            &[],
            &["database".to_string()],
            0.9,
        )
        .unwrap();
        record_decision(
            &conn,
            &DecisionType::TechChoice,
            "rust",
            "Use Rust",
            None,
            &[],
            &["language".to_string()],
            0.9,
        )
        .unwrap();

        let found = find_decisions_by_subject(&conn, "sqlite", 10).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].subject, "sqlite");

        let found2 = find_decisions_by_subject(&conn, "database", 10).unwrap();
        assert_eq!(found2.len(), 1); // matches via context_tags
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
