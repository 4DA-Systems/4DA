// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Phase 4b — Decision voting & retrieval commands.

use super::{get_team_config, queue_team_op};
use crate::audit::log_team_audit;
use crate::error::Result;
use crate::team_sync_types::*;
use rusqlite::params;
use tracing::info;

/// Vote on an existing team decision. Queues the vote for sync and records
/// it locally for immediate UI display.
#[tauri::command]
pub async fn vote_on_decision(
    decision_id: String,
    stance: String,
    rationale: String,
) -> Result<String> {
    let (team_id, client_id) = get_team_config()?;

    // Clone for audit + local insert before move into op
    let audit_decision_id = decision_id.clone();
    let local_decision_id = decision_id.clone();
    let local_client_id = client_id.clone();
    let local_stance = stance.clone();
    let local_rationale = rationale.clone();

    let op = TeamOp::VoteOnDecision {
        decision_id,
        stance,
        rationale,
    };

    let entry_id = queue_team_op(&team_id, &client_id, &op)?;

    // Also record the vote locally for immediate visibility
    if let Ok(conn) = crate::state::open_db_connection() {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO decision_votes (decision_id, voter_id, stance, rationale, voted_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            params![local_decision_id, local_client_id, local_stance, local_rationale],
        );

        // Audit: decision voted
        log_team_audit(
            &conn,
            "decision.voted",
            "decision",
            Some(&audit_decision_id),
            Some(&serde_json::json!({ "stance": local_stance })),
        );
    }

    info!(target: "4da::team_sync", entry_id = %entry_id, "Decision vote queued for team sync");
    Ok(entry_id)
}

/// Get team decisions for the current team, optionally filtered by status.
#[tauri::command]
pub async fn get_team_decisions(status_filter: Option<String>) -> Result<Vec<TeamDecision>> {
    let (team_id, _client_id) = get_team_config()?;

    let conn = crate::state::open_db_connection()?;

    let (query, use_status_filter) = match &status_filter {
        Some(_) => (
            "SELECT d.id, d.team_id, d.title, d.decision_type, d.rationale,
                    d.proposed_by, d.status, d.created_at, d.resolved_at,
                    (SELECT COUNT(*) FROM decision_votes v WHERE v.decision_id = d.id) as vote_count
             FROM team_decisions d
             WHERE d.team_id = ?1 AND d.status = ?2
             ORDER BY d.created_at DESC",
            true,
        ),
        None => (
            "SELECT d.id, d.team_id, d.title, d.decision_type, d.rationale,
                    d.proposed_by, d.status, d.created_at, d.resolved_at,
                    (SELECT COUNT(*) FROM decision_votes v WHERE v.decision_id = d.id) as vote_count
             FROM team_decisions d
             WHERE d.team_id = ?1
             ORDER BY d.created_at DESC",
            false,
        ),
    };

    let mut stmt = conn
        .prepare(query)
        .map_err(|e| format!("Failed to prepare query: {e}"))?;

    let row_mapper = |row: &rusqlite::Row| {
        Ok(TeamDecision {
            id: row.get(0)?,
            team_id: row.get(1)?,
            title: row.get(2)?,
            decision_type: row.get(3)?,
            rationale: row.get(4)?,
            proposed_by: row.get(5)?,
            status: row.get(6)?,
            created_at: row.get(7)?,
            resolved_at: row.get(8)?,
            vote_count: row.get(9)?,
        })
    };

    let decisions: Vec<TeamDecision> = if use_status_filter {
        let status = status_filter.as_deref().unwrap_or_default();
        stmt.query_map(params![team_id, status], row_mapper)
            .map_err(|e| format!("Failed to query decisions: {e}"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read decisions: {e}"))?
    } else {
        stmt.query_map(params![team_id], row_mapper)
            .map_err(|e| format!("Failed to query decisions: {e}"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read decisions: {e}"))?
    };

    Ok(decisions)
}

/// Get full detail of a single decision including all votes.
#[tauri::command]
pub async fn get_decision_detail(decision_id: String) -> Result<DecisionDetail> {
    let (team_id, _client_id) = get_team_config()?;

    let conn = crate::state::open_db_connection()?;

    // Get the decision itself
    let detail = conn
        .query_row(
            "SELECT id, team_id, title, decision_type, rationale, proposed_by, status, created_at, resolved_at
             FROM team_decisions
             WHERE id = ?1 AND team_id = ?2",
            params![decision_id, team_id],
            |row| {
                Ok(DecisionDetail {
                    id: row.get(0)?,
                    team_id: row.get(1)?,
                    title: row.get(2)?,
                    decision_type: row.get(3)?,
                    rationale: row.get(4)?,
                    proposed_by: row.get(5)?,
                    status: row.get(6)?,
                    created_at: row.get(7)?,
                    resolved_at: row.get(8)?,
                    votes: vec![], // Populated below
                })
            },
        )
        .map_err(|e| format!("Decision not found: {e}"))?;

    // Get all votes for this decision
    let mut stmt = conn
        .prepare(
            "SELECT voter_id, stance, rationale, voted_at
             FROM decision_votes
             WHERE decision_id = ?1
             ORDER BY voted_at ASC",
        )
        .map_err(|e| format!("Failed to prepare votes query: {e}"))?;

    let votes = stmt
        .query_map(params![decision_id], |row| {
            Ok(DecisionVote {
                voter_id: row.get(0)?,
                stance: row.get(1)?,
                rationale: row.get(2)?,
                voted_at: row.get(3)?,
            })
        })
        .map_err(|e| format!("Failed to query votes: {e}"))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read votes: {e}"))?;

    Ok(DecisionDetail { votes, ..detail })
}

/// Resolve a team decision (accept or reject). Admin/proposer action.
#[tauri::command]
pub async fn resolve_decision(decision_id: String, new_status: String) -> Result<()> {
    if new_status != "accepted" && new_status != "rejected" {
        return Err("Status must be 'accepted' or 'rejected'".into());
    }

    let (team_id, _client_id) = get_team_config()?;

    let conn = crate::state::open_db_connection()?;

    let updated = conn
        .execute(
            "UPDATE team_decisions SET status = ?1, resolved_at = datetime('now')
             WHERE id = ?2 AND team_id = ?3",
            params![new_status, decision_id, team_id],
        )
        .map_err(|e| format!("Failed to resolve decision: {e}"))?;

    if updated == 0 {
        return Err("Decision not found or not in this team".into());
    }

    // Audit: decision resolved
    log_team_audit(
        &conn,
        "decision.resolved",
        "decision",
        Some(&decision_id),
        Some(&serde_json::json!({ "new_status": new_status })),
    );

    info!(target: "4da::team_sync", decision_id = %decision_id, status = %new_status, "Decision resolved");
    Ok(())
}
