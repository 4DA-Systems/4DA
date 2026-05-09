// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tauri command handlers for developer decision management.

use super::*;
use rusqlite::{params, Connection};
use tracing::info;

#[tauri::command]
pub async fn get_decisions(
    decision_type: Option<String>,
    status: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<DeveloperDecision>> {
    let conn = crate::open_db_connection()?;
    let dt = decision_type.map(|s| DecisionType::from_str(&s));
    // Default to "active" so superseded decisions don't show in the UI.
    // Pass status="all" to explicitly bypass the filter and see everything.
    let st = match status.as_deref() {
        Some("all") => None,
        Some(s) => Some(DecisionStatus::from_str(s)),
        None => Some(DecisionStatus::Active),
    };
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
) -> Result<i64> {
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
) -> Result<()> {
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

/// Fully delete a technology from all tables where it can persist.
/// Used when ACE scanner detects a technology the user doesn't actually use.
/// Performs hard deletion (not supersede) so the tech never resurfaces in the UI.
#[tauri::command]
pub async fn remove_tech_decision(technology: String) -> Result<()> {
    let conn = crate::open_db_connection()?;

    // 1. Remove from tech_stack (primary tech storage)
    conn.execute(
        "DELETE FROM tech_stack WHERE technology = ?1",
        params![technology],
    )?;

    // 2. Remove from explicit_interests (may have been auto-seeded by ACE)
    conn.execute(
        "DELETE FROM explicit_interests WHERE topic = ?1",
        params![technology],
    )?;

    // 3. Delete any matching tech_choice decision (hard delete, not supersede)
    conn.execute(
        "DELETE FROM developer_decisions WHERE subject = ?1 AND decision_type = 'tech_choice'",
        params![technology],
    )?;

    // 4. Remove from detected_tech (prevent re-seeding on next ACE scan)
    conn.execute(
        "DELETE FROM detected_tech WHERE technology = ?1",
        params![technology],
    )?;

    // 5. Remove any learned affinity for this false tech
    conn.execute(
        "DELETE FROM topic_affinities WHERE topic = ?1",
        params![technology],
    )?;

    info!(target: "4da::decisions", technology = %technology, "Technology fully deleted: tech_stack + interests + decision + detected_tech + topic_affinities");
    Ok(())
}

/// Auto-seed decisions from tech_stack on first run.
/// Creates TechChoice decisions with confidence=0.6 for each tech_stack entry.
pub fn seed_decisions_from_profile(conn: &Connection) -> Result<usize> {
    // Only seed if table is empty
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM developer_decisions", [], |row| {
        row.get(0)
    })?;

    if count > 0 {
        return Ok(0);
    }

    // Get tech_stack entries
    let mut stmt = conn.prepare("SELECT technology FROM tech_stack")?;

    let techs: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in decisions: {e}");
                None
            }
        })
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
            &format!("Using {tech} as part of primary tech stack"),
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
