// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Organization management — multi-team enterprise orchestration.
//!
//! Organizations group teams under a single entity with shared policies,
//! retention rules, and cross-team signal correlation.
//! Tables: organizations, org_teams, org_admins, retention_policies.

use std::collections::HashMap;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use ts_rs::TS;

use crate::error::Result;

// -- Types --

/// Top-level organization that groups multiple teams.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub team_count: usize,
    pub total_seats: usize,
    pub created_at: String,
}

/// Summary of a team within an organization.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct OrgTeamSummary {
    pub team_id: String,
    pub member_count: usize,
    pub last_active: Option<String>,
}

/// Organization-level policy configuration (stored as JSON in `organizations.settings`).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct OrgPolicies {
    pub default_retention_days: HashMap<String, i32>,
    pub min_monitoring_interval: Option<u64>,
    pub require_decision_tracking: bool,
}

impl Default for OrgPolicies {
    fn default() -> Self {
        Self {
            default_retention_days: HashMap::new(),
            min_monitoring_interval: None,
            require_decision_tracking: false,
        }
    }
}

/// A retention policy for a specific resource type within a team.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RetentionPolicy {
    pub resource_type: String,
    pub retention_days: i32,
}

/// Cross-team signal correlation detected across an organization.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct CrossTeamCorrelation {
    pub correlation_id: String,
    pub signal_type: String,
    pub teams_affected: Vec<(String, usize)>,
    pub org_severity: String,
    pub first_detected: String,
    pub recommendation: String,
}

// -- Schema --

/// Create organization tables (idempotent).
pub fn create_tables(conn: &Connection) -> std::result::Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS organizations (
            id TEXT PRIMARY KEY, name TEXT NOT NULL,
            license_key_hash TEXT, settings TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS org_teams (
            org_id TEXT NOT NULL, team_id TEXT NOT NULL,
            PRIMARY KEY (org_id, team_id)
        );
        CREATE TABLE IF NOT EXISTS org_admins (
            org_id TEXT NOT NULL, member_id TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'org_admin',
            PRIMARY KEY (org_id, member_id)
        );
        CREATE TABLE IF NOT EXISTS retention_policies (
            id TEXT PRIMARY KEY, team_id TEXT NOT NULL,
            resource_type TEXT NOT NULL, retention_days INTEGER NOT NULL,
            updated_at TEXT DEFAULT (datetime('now')),
            UNIQUE(team_id, resource_type)
        );",
    )
}

// -- Core Functions --

/// Create a new organization and assign the initial admin.
pub fn create_organization(
    conn: &Connection,
    name: &str,
    license_key_hash: Option<&str>,
    admin_member_id: &str,
) -> Result<Organization> {
    let org_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO organizations (id, name, license_key_hash) VALUES (?1, ?2, ?3)",
        params![org_id, name, license_key_hash],
    )?;
    conn.execute(
        "INSERT INTO org_admins (org_id, member_id, role) VALUES (?1, ?2, 'org_owner')",
        params![org_id, admin_member_id],
    )?;
    info!(target: "4da::org", org_id = %org_id, name = %name, "Organization created");
    Ok(Organization {
        id: org_id,
        name: name.to_string(),
        team_count: 0,
        total_seats: 0,
        created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    })
}

/// Get an organization by ID, computing team count and total seats.
pub fn get_organization(conn: &Connection, org_id: &str) -> Result<Organization> {
    let (name, created_at): (String, String) = conn
        .query_row(
            "SELECT name, COALESCE(created_at, '') FROM organizations WHERE id = ?1",
            params![org_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                crate::error::FourDaError::Internal(format!("Organization not found: {org_id}"))
            }
            other => crate::error::FourDaError::Db(other),
        })?;
    let team_count: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM org_teams WHERE org_id = ?1",
            params![org_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let total_seats: usize = conn
        .query_row(
            "SELECT COUNT(DISTINCT tmc.client_id)
             FROM org_teams ot JOIN team_members_cache tmc ON tmc.team_id = ot.team_id
             WHERE ot.org_id = ?1",
            params![org_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    Ok(Organization {
        id: org_id.to_string(),
        name,
        team_count,
        total_seats,
        created_at,
    })
}

/// Add a team to an organization.
pub fn add_team_to_org(conn: &Connection, org_id: &str, team_id: &str) -> Result<()> {
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM organizations WHERE id = ?1",
            params![org_id],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if !exists {
        return Err(crate::error::FourDaError::Internal(format!(
            "Organization not found: {org_id}"
        )));
    }
    conn.execute(
        "INSERT OR IGNORE INTO org_teams (org_id, team_id) VALUES (?1, ?2)",
        params![org_id, team_id],
    )?;
    info!(target: "4da::org", org_id = %org_id, team_id = %team_id, "Team added to org");
    Ok(())
}

/// Remove a team from an organization.
pub fn remove_team_from_org(conn: &Connection, org_id: &str, team_id: &str) -> Result<()> {
    let removed = conn.execute(
        "DELETE FROM org_teams WHERE org_id = ?1 AND team_id = ?2",
        params![org_id, team_id],
    )?;
    if removed == 0 {
        warn!(target: "4da::org", org_id = %org_id, team_id = %team_id, "Team was not in org (no-op)");
    }
    Ok(())
}

/// List all teams in an organization with summary info.
pub fn get_org_teams(conn: &Connection, org_id: &str) -> Result<Vec<OrgTeamSummary>> {
    let mut stmt = conn.prepare(
        "SELECT ot.team_id,
                COALESCE((SELECT COUNT(*) FROM team_members_cache tmc WHERE tmc.team_id = ot.team_id), 0),
                (SELECT MAX(tmc.last_seen) FROM team_members_cache tmc WHERE tmc.team_id = ot.team_id)
         FROM org_teams ot WHERE ot.org_id = ?1 ORDER BY ot.team_id",
    )?;
    let teams = stmt
        .query_map(params![org_id], |row| {
            Ok(OrgTeamSummary {
                team_id: row.get(0)?,
                member_count: row.get(1)?,
                last_active: row.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(teams)
}

/// Check if a member is an org admin (org_admin or org_owner).
pub fn is_org_admin(conn: &Connection, org_id: &str, member_id: &str) -> Result<bool> {
    let is_admin: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM org_admins WHERE org_id = ?1 AND member_id = ?2",
            params![org_id, member_id],
            |row| row.get(0),
        )
        .unwrap_or(false);
    Ok(is_admin)
}

// -- Org Policies --

/// Store organization-level policies as JSON in the `settings` column.
pub fn set_org_policies(conn: &Connection, org_id: &str, policies: &OrgPolicies) -> Result<()> {
    let json = serde_json::to_string(policies)
        .map_err(|e| crate::error::FourDaError::Internal(format!("Serialize policies: {e}")))?;
    let updated = conn.execute(
        "UPDATE organizations SET settings = ?1 WHERE id = ?2",
        params![json, org_id],
    )?;
    if updated == 0 {
        return Err(crate::error::FourDaError::Internal(format!(
            "Organization not found: {org_id}"
        )));
    }
    info!(target: "4da::org", org_id = %org_id, "Organization policies updated");
    Ok(())
}

/// Retrieve organization-level policies. Returns defaults if none set.
pub fn get_org_policies(conn: &Connection, org_id: &str) -> Result<OrgPolicies> {
    let settings_json: Option<String> = conn
        .query_row(
            "SELECT settings FROM organizations WHERE id = ?1",
            params![org_id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                crate::error::FourDaError::Internal(format!("Organization not found: {org_id}"))
            }
            other => crate::error::FourDaError::Db(other),
        })?;
    match settings_json {
        Some(json) if !json.is_empty() => Ok(serde_json::from_str(&json).unwrap_or_default()),
        _ => Ok(OrgPolicies::default()),
    }
}

// -- Retention Policies --

/// Set a retention policy for a specific resource type within a team.
pub fn set_retention_policy(
    conn: &Connection,
    team_id: &str,
    resource_type: &str,
    days: i32,
) -> Result<()> {
    let policy_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO retention_policies (id, team_id, resource_type, retention_days, updated_at)
         VALUES (?1, ?2, ?3, ?4, datetime('now'))
         ON CONFLICT(team_id, resource_type)
         DO UPDATE SET retention_days = excluded.retention_days, updated_at = excluded.updated_at",
        params![policy_id, team_id, resource_type, days],
    )?;
    info!(target: "4da::org", team_id = %team_id, resource_type = %resource_type, days = days, "Retention policy set");
    Ok(())
}

/// Get all retention policies for a team.
pub fn get_retention_policies(conn: &Connection, team_id: &str) -> Result<Vec<RetentionPolicy>> {
    let mut stmt = conn.prepare(
        "SELECT resource_type, retention_days FROM retention_policies WHERE team_id = ?1 ORDER BY resource_type",
    )?;
    let policies = stmt
        .query_map(params![team_id], |row| {
            Ok(RetentionPolicy {
                resource_type: row.get(0)?,
                retention_days: row.get(1)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(policies)
}

/// Enforce retention policies by purging expired records. Returns total records purged.
///
/// Supported resource types:
/// - `source_items` — ingested content items
/// - `team_sync_log` — inbound relay entries
/// - `temporal_events` — time-series events
/// - `audit_log` — enterprise audit trail
/// - `signals` — team-shared signals
/// - `briefings` — generated briefings
/// - `shared_resources` — shared team resources
///
/// A 7-day grace period is applied: items are deleted only after `retention_days + 7`.
pub fn enforce_retention(conn: &Connection, team_id: &str) -> Result<usize> {
    let policies = get_retention_policies(conn, team_id)?;
    let mut total_purged = 0usize;

    for policy in &policies {
        if policy.retention_days == 0 {
            continue; // 0 = unlimited, no cleanup
        }
        // Add 7-day grace period
        let effective_days = policy.retention_days + 7;
        let cutoff_modifier = format!("-{effective_days} days");

        let purged = match policy.resource_type.as_str() {
            "source_items" => conn
                .execute(
                    "DELETE FROM source_items WHERE created_at < datetime('now', ?1)",
                    params![cutoff_modifier],
                )
                .unwrap_or(0),
            "team_sync_log" => conn
                .execute(
                    "DELETE FROM team_sync_log WHERE received_at < unixepoch() - (?1 * 86400)",
                    params![effective_days],
                )
                .unwrap_or(0),
            "temporal_events" => conn
                .execute(
                    "DELETE FROM temporal_events WHERE created_at < datetime('now', ?1)",
                    params![cutoff_modifier],
                )
                .unwrap_or(0),
            "audit_log" => conn
                .execute(
                    "DELETE FROM audit_log WHERE team_id = ?1 AND created_at < datetime('now', ?2)",
                    params![team_id, cutoff_modifier],
                )
                .unwrap_or(0),
            "signals" => conn
                .execute(
                    "DELETE FROM team_signals WHERE team_id = ?1 AND first_detected_at < datetime('now', ?2)",
                    params![team_id, cutoff_modifier],
                )
                .unwrap_or(0),
            "briefings" => conn
                .execute(
                    "DELETE FROM briefings WHERE created_at < datetime('now', ?1)",
                    params![cutoff_modifier],
                )
                .unwrap_or(0),
            "shared_resources" => conn
                .execute(
                    "DELETE FROM shared_resources WHERE team_id = ?1 AND created_at < datetime('now', ?2)",
                    params![team_id, cutoff_modifier],
                )
                .unwrap_or(0),
            other => {
                warn!(target: "4da::org", resource_type = %other, "Unknown retention resource type, skipping");
                0
            }
        };
        if purged > 0 {
            info!(
                target: "4da::org",
                team_id = %team_id,
                resource_type = %policy.resource_type,
                purged = purged,
                retention_days = policy.retention_days,
                "Retention enforcement: purged expired records"
            );
            // Audit the cleanup (fire-and-forget)
            crate::audit::log_team_audit(
                conn,
                "admin.retention_cleanup",
                &policy.resource_type,
                None,
                Some(&serde_json::json!({
                    "purged_count": purged,
                    "retention_days": policy.retention_days,
                    "effective_days": effective_days,
                })),
            );
        }
        total_purged += purged;
    }
    if total_purged > 0 {
        info!(target: "4da::org", team_id = %team_id, purged = total_purged, "Retention enforcement complete");
    }
    Ok(total_purged)
}

// -- Cross-Team Signal Correlation --

/// Detect signals appearing across 2+ teams in an org within 48 hours.
/// Correlates by topic overlap in `team_sync_queue` ShareSignal entries.
pub fn detect_cross_team_signals(
    conn: &Connection,
    org_id: &str,
) -> Result<Vec<CrossTeamCorrelation>> {
    let mut team_stmt = conn.prepare("SELECT team_id FROM org_teams WHERE org_id = ?1")?;
    let team_ids: Vec<String> = team_stmt
        .query_map(params![org_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    if team_ids.len() < 2 {
        return Ok(vec![]);
    }

    // Collect ShareSignal entries per team (last 48h): (chain_name, priority, topics)
    let mut team_signals: HashMap<String, Vec<(String, String, Vec<String>)>> = HashMap::new();
    for tid in &team_ids {
        let mut stmt = conn.prepare(
            "SELECT operation FROM team_sync_queue
             WHERE team_id = ?1 AND created_at > unixepoch() - 172800 LIMIT 200",
        )?;
        let ops: Vec<String> = stmt
            .query_map(params![tid], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        let mut signals = Vec::new();
        for op_json in &ops {
            if let Ok(op) = serde_json::from_str::<serde_json::Value>(op_json) {
                if op.get("type").and_then(|t| t.as_str()) != Some("ShareSignal") {
                    continue;
                }
                let sig = op
                    .get("chain_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let pri = op
                    .get("priority")
                    .and_then(|v| v.as_str())
                    .unwrap_or("medium")
                    .to_string();
                let topics: Vec<String> = op
                    .get("tech_topics")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                            .collect()
                    })
                    .unwrap_or_default();
                if !topics.is_empty() {
                    signals.push((sig, pri, topics));
                }
            }
        }
        if !signals.is_empty() {
            team_signals.insert(tid.clone(), signals);
        }
    }

    // Aggregate: topic -> [(team_id, count)], track signal type and max priority
    let mut topic_teams: HashMap<String, Vec<(String, usize)>> = HashMap::new();
    let mut topic_sig: HashMap<String, String> = HashMap::new();
    let mut topic_pri: HashMap<String, String> = HashMap::new();
    for (tid, signals) in &team_signals {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for (sig_type, priority, topics) in signals {
            for topic in topics {
                *counts.entry(topic.clone()).or_insert(0) += 1;
                topic_sig
                    .entry(topic.clone())
                    .or_insert_with(|| sig_type.clone());
                let cur = topic_pri
                    .entry(topic.clone())
                    .or_insert_with(|| priority.clone());
                if priority_rank(priority) > priority_rank(cur) {
                    *cur = priority.clone();
                }
            }
        }
        for (topic, count) in counts {
            topic_teams
                .entry(topic)
                .or_default()
                .push((tid.clone(), count));
        }
    }

    // Build correlations for topics seen by 2+ teams
    let mut correlations = Vec::new();
    for (topic, teams) in &topic_teams {
        if teams.len() < 2 {
            continue;
        }
        let sig = topic_sig
            .get(topic)
            .cloned()
            .unwrap_or_else(|| "unknown".into());
        let pri = topic_pri
            .get(topic)
            .cloned()
            .unwrap_or_else(|| "medium".into());
        let severity = match pri.as_str() {
            "critical" => "critical",
            "high" => "high",
            _ if teams.len() >= 3 => "high",
            _ => "medium",
        };
        correlations.push(CrossTeamCorrelation {
            correlation_id: uuid::Uuid::new_v4().to_string(),
            signal_type: sig,
            teams_affected: teams.clone(),
            org_severity: severity.to_string(),
            first_detected: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            recommendation: format!(
                "{} teams are tracking '{}' — coordinate response to avoid duplicate effort",
                teams.len(),
                topic
            ),
        });
    }
    correlations
        .sort_by(|a, b| priority_rank(&b.org_severity).cmp(&priority_rank(&a.org_severity)));
    Ok(correlations)
}

fn priority_rank(p: &str) -> u8 {
    match p {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

// -- Helpers --

fn get_current_org_id() -> Option<String> {
    let conn = crate::state::open_db_connection().ok()?;
    let settings = crate::state::get_settings_manager().lock();
    let client_id = settings
        .get()
        .team_relay
        .as_ref()
        .and_then(|c| c.client_id.clone())?;
    drop(settings);
    conn.query_row(
        "SELECT org_id FROM org_admins WHERE member_id = ?1 LIMIT 1",
        params![client_id],
        |row| row.get(0),
    )
    .ok()
}

fn get_current_team_id() -> Option<String> {
    let settings = crate::state::get_settings_manager().lock();
    settings
        .get()
        .team_relay
        .as_ref()
        .and_then(|c| c.team_id.clone())
}

// -- Tauri Commands --

#[tauri::command]
pub async fn get_organization_cmd() -> crate::error::Result<Option<Organization>> {
    let org_id = match get_current_org_id() {
        Some(id) => id,
        None => return Ok(None),
    };
    let conn = crate::state::open_db_connection()?;
    Ok(Some(get_organization(&conn, &org_id)?))
}

#[tauri::command]
pub async fn get_org_teams_cmd() -> crate::error::Result<Vec<OrgTeamSummary>> {
    let org_id = match get_current_org_id() {
        Some(id) => id,
        None => return Ok(vec![]),
    };
    let conn = crate::state::open_db_connection()?;
    get_org_teams(&conn, &org_id)
}

#[tauri::command]
pub async fn get_retention_policies_cmd() -> crate::error::Result<Vec<RetentionPolicy>> {
    let team_id = match get_current_team_id() {
        Some(id) => id,
        None => return Ok(vec![]),
    };
    let conn = crate::state::open_db_connection()?;
    get_retention_policies(&conn, &team_id)
}

#[tauri::command]
pub async fn set_retention_policy_cmd(
    resource_type: String,
    days: i32,
) -> crate::error::Result<()> {
    let team_id = get_current_team_id()
        .ok_or_else(|| crate::error::FourDaError::Internal("No team configured".into()))?;
    if days < 0 {
        return Err(crate::error::FourDaError::Internal(
            "Retention days must be 0 (unlimited) or a positive number".into(),
        ));
    }
    let conn = crate::state::open_db_connection()?;
    set_retention_policy(&conn, &team_id, &resource_type, days)?;

    // Audit: retention policy changed
    crate::audit::log_team_audit(
        &conn,
        "admin.policy_changed",
        "retention_policy",
        None,
        Some(&serde_json::json!({
            "resource_type": resource_type,
            "days": days,
        })),
    );

    Ok(())
}

#[tauri::command]
pub async fn get_cross_team_signals_cmd() -> crate::error::Result<Vec<CrossTeamCorrelation>> {
    let org_id = match get_current_org_id() {
        Some(id) => id,
        None => return Ok(vec![]),
    };
    let conn = crate::state::open_db_connection()?;
    detect_cross_team_signals(&conn, &org_id)
}

// ============================================================================
// Background Retention Scheduler
// ============================================================================

/// Start a background task that enforces retention policies once per day.
///
/// Runs at startup, then every 24 hours. Skips execution if no team is configured.
/// All errors are caught and logged — never panics.
pub fn start_retention_scheduler() {
    info!(target: "4da::org", "Starting retention enforcement scheduler");

    tauri::async_runtime::spawn(async {
        use std::time::Duration;

        // Wait 60 seconds after startup before first enforcement check
        tokio::time::sleep(Duration::from_secs(60)).await;

        loop {
            // Run retention enforcement if a team is configured
            let team_id = get_current_team_id();
            if let Some(ref team_id) = team_id {
                match crate::state::open_db_connection() {
                    Ok(conn) => match enforce_retention(&conn, team_id) {
                        Ok(purged) if purged > 0 => {
                            info!(
                                target: "4da::org",
                                team_id = %team_id,
                                purged = purged,
                                "Daily retention enforcement complete"
                            );
                        }
                        Ok(_) => {
                            info!(target: "4da::org", "Retention check: nothing to purge");
                        }
                        Err(e) => {
                            warn!(target: "4da::org", error = %e, "Retention enforcement failed");
                        }
                    },
                    Err(e) => {
                        warn!(target: "4da::org", error = %e, "Failed to open DB for retention enforcement");
                    }
                }
            }

            // Sleep 24 hours before next check
            tokio::time::sleep(Duration::from_secs(86400)).await;
        }
    });
}
