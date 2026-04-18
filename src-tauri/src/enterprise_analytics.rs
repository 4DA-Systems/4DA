// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Enterprise Analytics — organization-level usage and intelligence metrics.
//!
//! Aggregates activity across all teams in an organization for
//! admin dashboards, compliance reporting, and CSV export.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tracing::info;
use ts_rs::TS;

use crate::error::Result;

// ============================================================================
// Types
// ============================================================================

/// Aggregated analytics for an organization over a time period.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct OrgAnalytics {
    /// Human-readable period label (e.g. "Last 30 days").
    pub period: String,
    /// Number of seats with activity in the period.
    pub active_seats: usize,
    /// Total provisioned seats across all org teams.
    pub total_seats: usize,
    /// Total signals detected across all teams.
    pub signals_detected: usize,
    /// Signals marked as resolved.
    pub signals_resolved: usize,
    /// Decisions tracked across all teams.
    pub decisions_tracked: usize,
    /// AI briefings generated across all teams.
    pub briefings_generated: usize,
    /// Top signal categories with counts.
    pub top_signal_categories: Vec<(String, usize)>,
    /// Per-team activity breakdown.
    pub team_activity: Vec<TeamActivity>,
}

/// Activity metrics for a single team within the analytics period.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct TeamActivity {
    pub team_id: String,
    pub active_members: usize,
    pub signals_this_period: usize,
    pub decisions_this_period: usize,
    /// Engagement score: 0.0 (dormant) to 1.0 (highly active).
    pub engagement_score: f32,
}

// ============================================================================
// Core Functions
// ============================================================================

/// Compute organization-level analytics for the specified number of days.
pub fn get_org_analytics(conn: &Connection, org_id: &str, days: i32) -> Result<OrgAnalytics> {
    let period_label = if days == 1 {
        "Last 24 hours".to_string()
    } else {
        format!("Last {} days", days)
    };

    let day_offset = format!("-{} days", days);

    // Get all team IDs in this org
    let team_ids = get_org_team_ids(conn, org_id)?;

    if team_ids.is_empty() {
        return Ok(OrgAnalytics {
            period: period_label,
            active_seats: 0,
            total_seats: 0,
            signals_detected: 0,
            signals_resolved: 0,
            decisions_tracked: 0,
            briefings_generated: 0,
            top_signal_categories: vec![],
            team_activity: vec![],
        });
    }

    // Total seats across all org teams
    let total_seats = count_total_seats(conn, &team_ids);

    // Active seats: members with sync activity in the period
    let active_seats = count_active_seats(conn, &team_ids, &day_offset);

    // Signals detected: ShareSignal ops in team_sync_queue within the period
    let (signals_detected, signal_categories) = count_signals(conn, &team_ids, &day_offset);

    // Resolved signals: ResolveSignal ops
    let signals_resolved = count_resolved_signals(conn, &team_ids, &day_offset);

    // Decisions tracked: ProposeDecision ops
    let decisions_tracked = count_decisions(conn, &team_ids, &day_offset);

    // Briefings: count from briefing cache table if it exists
    let briefings_generated = count_briefings(conn, &day_offset);

    // Top signal categories (sorted by count descending, top 10)
    let mut sorted_categories: Vec<(String, usize)> = signal_categories.into_iter().collect();
    sorted_categories.sort_by(|a, b| b.1.cmp(&a.1));
    sorted_categories.truncate(10);

    // Per-team activity
    let mut team_activity = Vec::new();
    for tid in &team_ids {
        let activity = compute_team_activity(conn, tid, &day_offset);
        team_activity.push(activity);
    }

    // Sort teams by engagement score descending
    team_activity.sort_by(|a, b| {
        b.engagement_score
            .partial_cmp(&a.engagement_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(OrgAnalytics {
        period: period_label,
        active_seats,
        total_seats,
        signals_detected,
        signals_resolved,
        decisions_tracked,
        briefings_generated,
        top_signal_categories: sorted_categories,
        team_activity,
    })
}

/// Export organization analytics as a CSV string.
pub fn export_org_analytics_csv(conn: &Connection, org_id: &str, days: i32) -> Result<String> {
    let analytics = get_org_analytics(conn, org_id, days)?;

    let mut csv = String::new();

    // Summary section
    csv.push_str("# Organization Analytics Report\r\n");
    csv.push_str(&format!("# Period: {}\r\n", analytics.period));
    csv.push_str(&format!(
        "# Generated: {}\r\n",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
    ));
    csv.push_str("\r\n");

    // Summary metrics
    csv.push_str("Metric,Value\r\n");
    csv.push_str(&format!("Active Seats,{}\r\n", analytics.active_seats));
    csv.push_str(&format!("Total Seats,{}\r\n", analytics.total_seats));
    csv.push_str(&format!(
        "Signals Detected,{}\r\n",
        analytics.signals_detected
    ));
    csv.push_str(&format!(
        "Signals Resolved,{}\r\n",
        analytics.signals_resolved
    ));
    csv.push_str(&format!(
        "Decisions Tracked,{}\r\n",
        analytics.decisions_tracked
    ));
    csv.push_str(&format!(
        "Briefings Generated,{}\r\n",
        analytics.briefings_generated
    ));
    csv.push_str("\r\n");

    // Signal categories
    csv.push_str("Signal Category,Count\r\n");
    for (category, count) in &analytics.top_signal_categories {
        csv.push_str(&format!(
            "\"{}\",{}\r\n",
            category.replace('"', "\"\""),
            count
        ));
    }
    csv.push_str("\r\n");

    // Team activity
    csv.push_str("Team ID,Active Members,Signals,Decisions,Engagement Score\r\n");
    for team in &analytics.team_activity {
        csv.push_str(&format!(
            "{},{},{},{},{:.2}\r\n",
            team.team_id,
            team.active_members,
            team.signals_this_period,
            team.decisions_this_period,
            team.engagement_score
        ));
    }

    info!(
        target: "4da::analytics",
        org_id = %org_id,
        days = days,
        csv_bytes = csv.len(),
        "Analytics CSV exported"
    );

    Ok(csv)
}

// ============================================================================
// Internal Helpers
// ============================================================================

/// Get all team IDs belonging to an organization.
fn get_org_team_ids(conn: &Connection, org_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT team_id FROM org_teams WHERE org_id = ?1")?;

    let ids = stmt
        .query_map(params![org_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(ids)
}

/// Count total seats (unique members) across the given teams.
fn count_total_seats(conn: &Connection, team_ids: &[String]) -> usize {
    if team_ids.is_empty() {
        return 0;
    }

    let placeholders = make_placeholders(team_ids.len());
    let sql = format!(
        "SELECT COUNT(DISTINCT client_id) FROM team_members_cache WHERE team_id IN ({})",
        placeholders
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let params: Vec<&dyn rusqlite::types::ToSql> = team_ids
        .iter()
        .map(|s| s as &dyn rusqlite::types::ToSql)
        .collect();

    stmt.query_row(params.as_slice(), |row| row.get(0))
        .unwrap_or(0)
}

/// Count members with recent sync activity.
fn count_active_seats(conn: &Connection, team_ids: &[String], day_offset: &str) -> usize {
    if team_ids.is_empty() {
        return 0;
    }

    let placeholders = make_placeholders(team_ids.len());
    let sql = format!(
        "SELECT COUNT(DISTINCT client_id) FROM team_sync_queue
         WHERE team_id IN ({})
           AND created_at > unixepoch('now', ?{})",
        placeholders,
        team_ids.len() + 1
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = team_ids
        .iter()
        .map(|s| Box::new(s.clone()) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    params.push(Box::new(day_offset.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    stmt.query_row(param_refs.as_slice(), |row| row.get(0))
        .unwrap_or(0)
}

/// Count ShareSignal operations and collect category counts.
fn count_signals(
    conn: &Connection,
    team_ids: &[String],
    day_offset: &str,
) -> (usize, std::collections::HashMap<String, usize>) {
    let mut total = 0usize;
    let mut categories: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for tid in team_ids {
        let mut stmt = match conn.prepare(
            "SELECT operation FROM team_sync_queue
             WHERE team_id = ?1
               AND created_at > unixepoch('now', ?2)",
        ) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let ops: Vec<String> = stmt
            .query_map(params![tid, day_offset], |row| row.get(0))
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        for op_json in &ops {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(op_json) {
                if val.get("type").and_then(|t| t.as_str()) == Some("ShareSignal") {
                    total += 1;
                    let chain_name = val
                        .get("chain_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("uncategorized")
                        .to_string();
                    *categories.entry(chain_name).or_insert(0) += 1;
                }
            }
        }
    }

    (total, categories)
}

/// Count ResolveSignal operations.
fn count_resolved_signals(conn: &Connection, team_ids: &[String], day_offset: &str) -> usize {
    let mut total = 0usize;

    for tid in team_ids {
        let mut stmt = match conn.prepare(
            "SELECT operation FROM team_sync_queue
             WHERE team_id = ?1
               AND created_at > unixepoch('now', ?2)",
        ) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let ops: Vec<String> = stmt
            .query_map(params![tid, day_offset], |row| row.get(0))
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        for op_json in &ops {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(op_json) {
                if val.get("type").and_then(|t| t.as_str()) == Some("ResolveSignal") {
                    total += 1;
                }
            }
        }
    }

    total
}

/// Count ProposeDecision operations.
fn count_decisions(conn: &Connection, team_ids: &[String], day_offset: &str) -> usize {
    let mut total = 0usize;

    for tid in team_ids {
        let mut stmt = match conn.prepare(
            "SELECT operation FROM team_sync_queue
             WHERE team_id = ?1
               AND created_at > unixepoch('now', ?2)",
        ) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let ops: Vec<String> = stmt
            .query_map(params![tid, day_offset], |row| row.get(0))
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        for op_json in &ops {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(op_json) {
                if val.get("type").and_then(|t| t.as_str()) == Some("ProposeDecision") {
                    total += 1;
                }
            }
        }
    }

    total
}

/// Count briefings generated (from briefing_cache table if it exists).
fn count_briefings(conn: &Connection, day_offset: &str) -> usize {
    // Briefing cache may not exist in all environments
    conn.query_row(
        "SELECT COUNT(*) FROM briefing_cache
         WHERE generated_at > datetime('now', ?1)",
        params![day_offset],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// Compute activity metrics for a single team.
fn compute_team_activity(conn: &Connection, team_id: &str, day_offset: &str) -> TeamActivity {
    // Active members: distinct clients with sync entries in the period
    let active_members: usize = conn
        .query_row(
            "SELECT COUNT(DISTINCT client_id) FROM team_sync_queue
             WHERE team_id = ?1 AND created_at > unixepoch('now', ?2)",
            params![team_id, day_offset],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Total members for engagement calculation
    let total_members: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM team_members_cache WHERE team_id = ?1",
            params![team_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Count signals and decisions for this team
    let mut signals = 0usize;
    let mut decisions = 0usize;

    if let Ok(mut stmt) = conn.prepare(
        "SELECT operation FROM team_sync_queue
         WHERE team_id = ?1 AND created_at > unixepoch('now', ?2)",
    ) {
        let ops: Vec<String> = stmt
            .query_map(params![team_id, day_offset], |row| row.get(0))
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        for op_json in &ops {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(op_json) {
                match val.get("type").and_then(|t| t.as_str()) {
                    Some("ShareSignal") => signals += 1,
                    Some("ProposeDecision") => decisions += 1,
                    _ => {}
                }
            }
        }
    }

    // Engagement score: weighted combination of member activity ratio and action volume
    let member_ratio = if total_members > 0 {
        (active_members as f32) / (total_members as f32)
    } else {
        0.0
    };
    // Action density: normalize to a 0-1 scale (10+ actions = full score)
    let action_density = ((signals + decisions) as f32 / 10.0).min(1.0);
    let engagement_score = (member_ratio * 0.6 + action_density * 0.4).min(1.0);

    TeamActivity {
        team_id: team_id.to_string(),
        active_members,
        signals_this_period: signals,
        decisions_this_period: decisions,
        engagement_score,
    }
}

/// Build SQL placeholders like "?1, ?2, ?3".
fn make_placeholders(count: usize) -> String {
    (1..=count)
        .map(|i| format!("?{}", i))
        .collect::<Vec<_>>()
        .join(", ")
}

// ============================================================================
// Helpers
// ============================================================================

/// Get the org_id for the current user from settings/DB.
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

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get organization-level analytics for the specified period.
#[tauri::command]
pub async fn get_org_analytics_cmd(days: Option<i32>) -> crate::error::Result<OrgAnalytics> {
    let org_id = get_current_org_id().ok_or_else(|| {
        crate::error::FourDaError::Internal("No organization configured".to_string())
    })?;

    let days = days.unwrap_or(30).max(1);
    let conn = crate::state::open_db_connection()?;
    get_org_analytics(&conn, &org_id, days)
}

/// Export organization analytics as CSV for the specified period.
#[tauri::command]
pub async fn export_org_analytics_cmd(days: Option<i32>) -> crate::error::Result<String> {
    let org_id = get_current_org_id().ok_or_else(|| {
        crate::error::FourDaError::Internal("No organization configured".to_string())
    })?;

    let days = days.unwrap_or(30).max(1);
    let conn = crate::state::open_db_connection()?;
    export_org_analytics_csv(&conn, &org_id, days)
}
