// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Commitment Contracts — Intelligence Reconciliation Phase 11 (2026-04-17).
//!
//! When a user accepts a Decision Brief, they set a **refutation
//! condition** — one sentence describing what would convince them the
//! decision was wrong. 4DA stores the contract and a background
//! watcher scans incoming source items for matches.
//!
//! When a match fires, the contract is flipped to `triggered` status
//! and a `Refutation` `EvidenceItem` surfaces in the Evidence lens.
//!
//! This is AWE holding itself accountable to the user — the
//! philosophical signature of the product.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::info;
use ts_rs::TS;

use crate::error::Result;
use crate::evidence::{
    Action as EvidenceAction, Confidence, EvidenceCitation, EvidenceItem, EvidenceKind, LensHints,
    Urgency,
};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct CommitmentContract {
    pub id: i64,
    pub decision_statement: String,
    pub refutation_condition: String,
    pub subject: String,
    pub status: ContractStatus,
    pub created_at: String,
    pub triggered_at: Option<String>,
    pub trigger_item_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "bindings/")]
pub enum ContractStatus {
    Active,
    Triggered,
    Dismissed,
}

// ============================================================================
// Storage
// ============================================================================

pub fn create_contract(
    conn: &rusqlite::Connection,
    decision_statement: &str,
    refutation_condition: &str,
    subject: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO commitment_contracts (decision_statement, refutation_condition, subject)
         VALUES (?1, ?2, ?3)",
        params![decision_statement, refutation_condition, subject],
    )?;
    let id = conn.last_insert_rowid();
    info!(
        target: "4da::contracts",
        id,
        subject,
        "commitment contract created"
    );
    Ok(id)
}

pub fn list_active_contracts(conn: &rusqlite::Connection) -> Result<Vec<CommitmentContract>> {
    let mut stmt = conn.prepare(
        "SELECT id, decision_statement, refutation_condition, subject, status, created_at,
                triggered_at, trigger_item_id
         FROM commitment_contracts
         WHERE status = 'active'
         ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(CommitmentContract {
            id: row.get(0)?,
            decision_statement: row.get(1)?,
            refutation_condition: row.get(2)?,
            subject: row.get(3)?,
            status: ContractStatus::Active,
            created_at: row.get(5)?,
            triggered_at: row.get(6)?,
            trigger_item_id: row.get(7)?,
        })
    })?;
    Ok(rows.flatten().collect())
}

pub fn list_all_contracts(conn: &rusqlite::Connection) -> Result<Vec<CommitmentContract>> {
    let mut stmt = conn.prepare(
        "SELECT id, decision_statement, refutation_condition, subject, status, created_at,
                triggered_at, trigger_item_id
         FROM commitment_contracts
         ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let status_str: String = row.get(4)?;
        let status = match status_str.as_str() {
            "triggered" => ContractStatus::Triggered,
            "dismissed" => ContractStatus::Dismissed,
            _ => ContractStatus::Active,
        };
        Ok(CommitmentContract {
            id: row.get(0)?,
            decision_statement: row.get(1)?,
            refutation_condition: row.get(2)?,
            subject: row.get(3)?,
            status,
            created_at: row.get(5)?,
            triggered_at: row.get(6)?,
            trigger_item_id: row.get(7)?,
        })
    })?;
    Ok(rows.flatten().collect())
}

pub fn trigger_contract(
    conn: &rusqlite::Connection,
    contract_id: i64,
    trigger_item_id: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE commitment_contracts
         SET status = 'triggered', triggered_at = datetime('now'), trigger_item_id = ?2
         WHERE id = ?1 AND status = 'active'",
        params![contract_id, trigger_item_id],
    )?;
    info!(
        target: "4da::contracts",
        contract_id,
        trigger_item_id,
        "commitment contract triggered"
    );
    Ok(())
}

pub fn dismiss_contract(conn: &rusqlite::Connection, contract_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE commitment_contracts SET status = 'dismissed' WHERE id = ?1",
        params![contract_id],
    )?;
    Ok(())
}

// ============================================================================
// Watcher — scan source items for refutation matches
// ============================================================================

/// Check all active contracts against recent source items. Returns newly
/// triggered contract IDs so callers can surface Refutation EvidenceItems.
///
/// The match is intentionally simple: case-insensitive substring of the
/// refutation condition's keywords against the source item title + content.
/// Phase 13 (hardening) can upgrade this to semantic similarity via
/// embeddings; for now the goal is a working round-trip, not a perfect
/// classifier.
pub fn scan_for_refutations(conn: &rusqlite::Connection, hours: u32) -> Result<Vec<i64>> {
    let contracts = list_active_contracts(conn)?;
    if contracts.is_empty() {
        return Ok(Vec::new());
    }

    let mut triggered_ids: Vec<i64> = Vec::new();

    for contract in &contracts {
        let keywords = extract_keywords(&contract.refutation_condition);
        if keywords.is_empty() {
            continue;
        }

        // Build a LIKE clause for each keyword (AND'd — all must match).
        // rusqlite positional params are 1-indexed.
        let mut conditions: Vec<String> = Vec::new();
        let mut bind_values: Vec<String> = Vec::new();
        for kw in &keywords {
            let idx = bind_values.len() + 1; // 1-based
            conditions.push(format!(
                "(LOWER(title) LIKE ?{idx} OR LOWER(COALESCE(content,'')) LIKE ?{idx})"
            ));
            bind_values.push(format!("%{kw}%"));
        }

        let sql = format!(
            "SELECT id FROM source_items
             WHERE created_at > datetime('now', '-{hours} hours')
             AND ({})
             LIMIT 1",
            conditions.join(" AND "),
        );

        let mut stmt = conn.prepare(&sql)?;
        let params_vec: Vec<&dyn rusqlite::ToSql> =
            bind_values.iter().map(|v| v as &dyn rusqlite::ToSql).collect();

        if let Ok(item_id) = stmt.query_row(params_vec.as_slice(), |row| row.get::<_, i64>(0)) {
            trigger_contract(conn, contract.id, item_id)?;
            triggered_ids.push(contract.id);
        }
    }

    Ok(triggered_ids)
}

/// Extract lowercase keywords from a refutation condition.
/// Drops stopwords and too-short tokens.
fn extract_keywords(condition: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "if", "the", "a", "an", "is", "are", "was", "were", "in", "on", "at",
        "to", "for", "of", "and", "or", "not", "it", "my", "our", "they",
        "this", "that", "there", "than", "then", "with", "from", "by", "has",
        "have", "had", "will", "would", "could", "should", "be", "been",
        "being", "do", "does", "did", "any", "more", "over", "go", "goes",
    ];
    condition
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | '.' | '!' | '?' | ';' | ':'))
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 3 && !STOP.contains(&w.as_str()))
        .take(5) // cap keywords to keep the SQL scannable
        .collect()
}

// ============================================================================
// EvidenceItem conversion
// ============================================================================

impl CommitmentContract {
    pub fn to_evidence_item(&self) -> EvidenceItem {
        let title = format!("Refutation: {}", self.decision_statement)
            .chars()
            .take(120)
            .collect::<String>()
            .trim_end_matches('.')
            .to_string();

        let explanation = format!(
            "Your refutation condition was met: \"{}\"",
            self.refutation_condition
        );

        let citation = EvidenceCitation {
            source: "commitment-contract".to_string(),
            title: format!("Contract #{}", self.id),
            url: None,
            freshness_days: 0.0,
            relevance_note: format!(
                "condition: {}",
                self.refutation_condition.chars().take(200).collect::<String>()
            ),
        };

        EvidenceItem {
            id: format!("cc_refute_{}", self.id),
            kind: EvidenceKind::Refutation,
            title,
            explanation,
            confidence: Confidence::heuristic(0.7),
            urgency: Urgency::High,
            reversibility: None,
            evidence: vec![citation],
            affected_projects: Vec::new(),
            affected_deps: if self.subject.is_empty() {
                Vec::new()
            } else {
                vec![self.subject.clone()]
            },
            suggested_actions: vec![
                EvidenceAction {
                    action_id: "investigate".to_string(),
                    label: "Re-examine".to_string(),
                    description: "Revisit your original decision.".to_string(),
                },
                EvidenceAction {
                    action_id: "dismiss".to_string(),
                    label: "Dismiss".to_string(),
                    description: "The condition was met but the decision still holds.".to_string(),
                },
            ],
            precedents: Vec::new(),
            refutation_condition: Some(self.refutation_condition.clone()),
            lens_hints: LensHints::evidence_only(),
            created_at: chrono::Utc::now().timestamp_millis(),
            expires_at: None,
        }
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn create_commitment_contract(
    decision_statement: String,
    refutation_condition: String,
    subject: String,
) -> std::result::Result<i64, String> {
    let conn = crate::open_db_connection().map_err(|e| e.to_string())?;
    create_contract(&conn, &decision_statement, &refutation_condition, &subject)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_commitment_contracts() -> std::result::Result<String, String> {
    let conn = crate::open_db_connection().map_err(|e| e.to_string())?;
    let contracts = list_all_contracts(&conn).map_err(|e| e.to_string())?;
    serde_json::to_string(&contracts).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dismiss_commitment_contract(contract_id: i64) -> std::result::Result<(), String> {
    let conn = crate::open_db_connection().map_err(|e| e.to_string())?;
    dismiss_contract(&conn, contract_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_refutations(hours: u32) -> std::result::Result<String, String> {
    let conn = crate::open_db_connection().map_err(|e| e.to_string())?;
    let triggered = scan_for_refutations(&conn, hours).map_err(|e| e.to_string())?;
    serde_json::to_string(&triggered).map_err(|e| e.to_string())
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
            "CREATE TABLE commitment_contracts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                decision_statement TEXT NOT NULL,
                refutation_condition TEXT NOT NULL,
                subject TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                triggered_at TEXT,
                trigger_item_id INTEGER
             );
             CREATE TABLE source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                content TEXT,
                source_type TEXT NOT NULL DEFAULT 'test',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
             );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn create_and_list_contract() {
        let conn = setup_db();
        let id = create_contract(
            &conn,
            "Adopted Turborepo",
            "If build times exceed 45 seconds",
            "turborepo",
        )
        .unwrap();
        assert!(id > 0);
        let active = list_active_contracts(&conn).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].decision_statement, "Adopted Turborepo");
        assert_eq!(active[0].subject, "turborepo");
    }

    #[test]
    fn trigger_contract_flips_status() {
        let conn = setup_db();
        let id = create_contract(&conn, "test", "condition", "").unwrap();
        trigger_contract(&conn, id, 42).unwrap();
        let all = list_all_contracts(&conn).unwrap();
        assert_eq!(all[0].status, ContractStatus::Triggered);
        assert_eq!(all[0].trigger_item_id, Some(42));
    }

    #[test]
    fn trigger_only_affects_active() {
        let conn = setup_db();
        let id = create_contract(&conn, "test", "condition", "").unwrap();
        dismiss_contract(&conn, id).unwrap();
        trigger_contract(&conn, id, 42).unwrap();
        let all = list_all_contracts(&conn).unwrap();
        assert_eq!(all[0].status, ContractStatus::Dismissed);
    }

    #[test]
    fn dismiss_contract_works() {
        let conn = setup_db();
        let id = create_contract(&conn, "test", "condition", "").unwrap();
        dismiss_contract(&conn, id).unwrap();
        let active = list_active_contracts(&conn).unwrap();
        assert!(active.is_empty());
    }

    #[test]
    fn extract_keywords_drops_stopwords() {
        let kw = extract_keywords("If the build times are over 45 seconds");
        assert!(kw.contains(&"build".to_string()));
        assert!(kw.contains(&"times".to_string()));
        assert!(kw.contains(&"seconds".to_string()));
        assert!(!kw.contains(&"the".to_string()));
        assert!(!kw.contains(&"are".to_string()));
    }

    #[test]
    fn extract_keywords_caps_at_5() {
        let kw = extract_keywords("one two three four five six seven eight nine ten");
        assert_eq!(kw.len(), 5);
    }

    #[test]
    fn extract_keywords_drops_short_tokens() {
        let kw = extract_keywords("a an in if");
        assert!(kw.is_empty());
    }

    #[test]
    fn scan_finds_matching_item() {
        let conn = setup_db();
        create_contract(
            &conn,
            "Adopted tokio",
            "If there is a critical CVE in tokio",
            "tokio",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO source_items (title, content, created_at) VALUES (?1, ?2, datetime('now'))",
            params!["Critical CVE-2026-9999 in tokio 1.x", "Full advisory text mentioning tokio critical vulnerability"],
        )
        .unwrap();
        let triggered = scan_for_refutations(&conn, 24).unwrap();
        assert_eq!(triggered.len(), 1);
        let all = list_all_contracts(&conn).unwrap();
        assert_eq!(all[0].status, ContractStatus::Triggered);
    }

    #[test]
    fn scan_ignores_non_matching_items() {
        let conn = setup_db();
        create_contract(
            &conn,
            "Adopted tokio",
            "If there is a critical CVE in tokio",
            "tokio",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO source_items (title, content, created_at) VALUES (?1, ?2, datetime('now'))",
            params!["React 19 released with new features", "Frontend framework update"],
        )
        .unwrap();
        let triggered = scan_for_refutations(&conn, 24).unwrap();
        assert!(triggered.is_empty());
    }

    #[test]
    fn to_evidence_item_produces_refutation_kind() {
        let c = CommitmentContract {
            id: 1,
            decision_statement: "Adopted tokio".to_string(),
            refutation_condition: "CVE in tokio".to_string(),
            subject: "tokio".to_string(),
            status: ContractStatus::Triggered,
            created_at: "2026-04-17 00:00:00".to_string(),
            triggered_at: Some("2026-04-17 12:00:00".to_string()),
            trigger_item_id: Some(42),
        };
        let item = c.to_evidence_item();
        assert_eq!(item.kind, crate::evidence::EvidenceKind::Refutation);
        assert_eq!(item.urgency, crate::evidence::Urgency::High);
        assert!(item.lens_hints.evidence);
        assert!(crate::evidence::validate_item(&item).is_ok());
    }

    #[test]
    fn contract_roundtrip_serialization() {
        let c = CommitmentContract {
            id: 1,
            decision_statement: "Adopted tokio".to_string(),
            refutation_condition: "CVE discovered".to_string(),
            subject: "tokio".to_string(),
            status: ContractStatus::Active,
            created_at: "2026-04-17".to_string(),
            triggered_at: None,
            trigger_item_id: None,
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: CommitmentContract = serde_json::from_str(&json).unwrap();
        assert_eq!(back.decision_statement, c.decision_statement);
        assert_eq!(back.status, ContractStatus::Active);
    }
}
