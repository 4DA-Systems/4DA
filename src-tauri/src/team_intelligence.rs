//! Team Intelligence — aggregated analytics across all team members.
//!
//! Queries the `team_sync_log` and `team_members_cache` tables to build
//! a collective picture of the team's technology coverage, blind spots,
//! bus-factor risks, and merged signal detections.
//!
//! Aggregation helpers live in `team_intelligence_aggregation` (file-size split).

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::warn;
use ts_rs::TS;

use crate::team_sync_types::TeamOp;

#[path = "team_intelligence_aggregation.rs"]
mod aggregation;
use aggregation::*;

// ============================================================================
// Exported Structs
// ============================================================================

/// Aggregated team intelligence profile.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TeamProfile {
    pub team_id: String,
    pub member_count: usize,
    /// Collective tech stack with per-member confidence.
    pub collective_stack: Vec<TeamTechEntry>,
    /// Fraction of the team's adjacent ecosystem covered (0.0–1.0).
    pub stack_coverage: f32,
    /// Topics relevant to the team's stack that no member tracks.
    pub blind_spots: Vec<TeamBlindSpot>,
    /// Topics watched by 3+ members (redundancy / shared interest).
    pub overlap_zones: Vec<OverlapZone>,
    /// Tech known by exactly one member (bus-factor = 1).
    pub unique_strengths: Vec<UniqueStrength>,
    /// ISO-8601 timestamp when this profile was computed.
    pub generated_at: String,
}

/// A technology in the collective stack.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TeamTechEntry {
    pub tech: String,
    /// Members who list this in their primary stack.
    pub members: Vec<String>,
    /// Fraction of team that knows this tech (0.0–1.0).
    pub team_confidence: f32,
}

/// A topic adjacent to the team's stack that nobody monitors.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TeamBlindSpot {
    pub topic: String,
    /// The stack entry/entries that make this topic relevant.
    pub related_to: Vec<String>,
    /// "high" if related to 3+ team members' stack, else "medium".
    pub severity: String,
}

/// A topic tracked by 3 or more team members.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OverlapZone {
    pub topic: String,
    pub members: Vec<String>,
    pub member_count: usize,
}

/// A technology known by exactly one member (bus-factor risk).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UniqueStrength {
    pub tech: String,
    pub sole_expert: String,
    pub risk_level: String,
}

/// A single member's detection of a shared signal.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MemberDetection {
    pub client_id: String,
    pub display_name: String,
    pub detected_at: String,
}

/// Aggregated signal summary across the team.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TeamSignalSummary {
    pub signal_id: String,
    pub chain_name: String,
    pub priority: String,
    pub tech_topics: Vec<String>,
    pub detected_by: Vec<MemberDetection>,
    /// Higher when multiple seats confirm (base 0.5 + 0.15 per additional detector, capped 1.0).
    pub team_confidence: f32,
    pub first_detected_at: String,
    pub suggested_action: String,
    pub resolved: bool,
}

// ============================================================================
// DB query: parse DNA summaries from team_sync_log
// ============================================================================

/// Parse all applied ShareDnaSummary entries for a team.
/// Takes the latest entry per client_id (by relay_seq DESC).
fn load_member_dnas(conn: &rusqlite::Connection, team_id: &str) -> Result<Vec<MemberDna>> {
    let mut stmt = conn.prepare(
        "SELECT tsl.client_id, tsl.encrypted, COALESCE(tmc.display_name, tsl.client_id)
         FROM team_sync_log tsl
         LEFT JOIN team_members_cache tmc
           ON tmc.team_id = tsl.team_id AND tmc.client_id = tsl.client_id
         WHERE tsl.team_id = ?1 AND tsl.applied = 1
         ORDER BY tsl.relay_seq DESC",
    )?;

    let rows: Vec<(String, Vec<u8>, String)> = stmt
        .query_map(params![team_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut seen: HashSet<String> = HashSet::new();
    let mut dnas = Vec::new();

    for (client_id, blob, display_name) in rows {
        if seen.contains(&client_id) {
            continue; // Already have latest for this member
        }

        let entry: crate::team_sync_types::TeamMetadataEntry = match serde_json::from_slice(&blob) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if let TeamOp::ShareDnaSummary {
            primary_stack,
            interests,
            blind_spots,
            ..
        } = entry.operation
        {
            seen.insert(client_id.clone());
            dnas.push(MemberDna {
                client_id,
                display_name,
                primary_stack,
                interests,
                blind_spots,
            });
        }
    }

    Ok(dnas)
}

// ============================================================================
// Core Functions
// ============================================================================

/// Build the full team intelligence profile.
pub fn get_team_profile(conn: &rusqlite::Connection, team_id: &str) -> Result<TeamProfile> {
    let member_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM team_members_cache WHERE team_id = ?1",
            params![team_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let dnas = load_member_dnas(conn, team_id)?;

    let collective_stack = build_collective_stack(&dnas, member_count as usize);
    let blind_spots = compute_blind_spots(&dnas, &collective_stack);
    let overlap_zones = compute_overlap_zones(&dnas);
    let unique_strengths = compute_unique_strengths(&dnas);
    let stack_coverage = compute_stack_coverage(&dnas, &collective_stack);

    Ok(TeamProfile {
        team_id: team_id.to_string(),
        member_count: member_count as usize,
        collective_stack,
        stack_coverage,
        blind_spots,
        overlap_zones,
        unique_strengths,
        generated_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Identify topics adjacent to the team's stack that nobody tracks.
pub fn get_team_blind_spots(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<Vec<TeamBlindSpot>> {
    let dnas = load_member_dnas(conn, team_id)?;
    let member_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM team_members_cache WHERE team_id = ?1",
            params![team_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let collective_stack = build_collective_stack(&dnas, member_count as usize);
    Ok(compute_blind_spots(&dnas, &collective_stack))
}

/// Identify single-expert tech risks (bus-factor = 1).
pub fn get_bus_factor_report(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<Vec<UniqueStrength>> {
    let dnas = load_member_dnas(conn, team_id)?;
    Ok(compute_unique_strengths(&dnas))
}

/// Aggregate SharedSignal entries, merging when multiple members detect
/// the same signal chain within 48 hours.
pub fn get_team_signal_summary(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<Vec<TeamSignalSummary>> {
    let mut stmt = conn.prepare(
        "SELECT tsl.client_id, tsl.encrypted, tsl.received_at,
                COALESCE(tmc.display_name, tsl.client_id)
         FROM team_sync_log tsl
         LEFT JOIN team_members_cache tmc
           ON tmc.team_id = tsl.team_id AND tmc.client_id = tsl.client_id
         WHERE tsl.team_id = ?1 AND tsl.applied = 1
         ORDER BY tsl.relay_seq ASC",
    )?;

    let rows: Vec<(String, Vec<u8>, i64, String)> = stmt
        .query_map(params![team_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut signal_groups: HashMap<String, TeamSignalSummary> = HashMap::new();
    let mut resolved_signals: HashSet<String> = HashSet::new();
    let merge_window_secs: i64 = 48 * 3600;

    for (client_id, blob, received_at, display_name) in &rows {
        let entry: crate::team_sync_types::TeamMetadataEntry = match serde_json::from_slice(blob) {
            Ok(e) => e,
            Err(_) => continue,
        };

        match entry.operation {
            TeamOp::ShareSignal {
                signal_id,
                chain_name,
                priority,
                tech_topics,
                suggested_action,
            } => {
                let detected_at = format_unix_timestamp(*received_at);

                if let Some(existing) = signal_groups.get_mut(&chain_name) {
                    let first_ts = parse_iso_to_unix(&existing.first_detected_at).unwrap_or(0);
                    if (*received_at - first_ts).abs() <= merge_window_secs {
                        if !existing
                            .detected_by
                            .iter()
                            .any(|d| d.client_id == *client_id)
                        {
                            existing.detected_by.push(MemberDetection {
                                client_id: client_id.clone(),
                                display_name: display_name.clone(),
                                detected_at: detected_at.clone(),
                            });
                        }
                        existing.team_confidence =
                            compute_signal_confidence(existing.detected_by.len());
                        if priority_rank(&priority) > priority_rank(&existing.priority) {
                            existing.priority = priority;
                        }
                        for topic in &tech_topics {
                            if !existing.tech_topics.contains(topic) {
                                existing.tech_topics.push(topic.clone());
                            }
                        }
                    } else {
                        let key = format!("{}_{}", chain_name, signal_id);
                        signal_groups.insert(
                            key,
                            TeamSignalSummary {
                                signal_id: signal_id.clone(),
                                chain_name: chain_name.clone(),
                                priority,
                                tech_topics,
                                detected_by: vec![MemberDetection {
                                    client_id: client_id.clone(),
                                    display_name: display_name.clone(),
                                    detected_at: detected_at.clone(),
                                }],
                                team_confidence: compute_signal_confidence(1),
                                first_detected_at: detected_at,
                                suggested_action,
                                resolved: false,
                            },
                        );
                    }
                } else {
                    signal_groups.insert(
                        chain_name.clone(),
                        TeamSignalSummary {
                            signal_id,
                            chain_name,
                            priority,
                            tech_topics,
                            detected_by: vec![MemberDetection {
                                client_id: client_id.clone(),
                                display_name: display_name.clone(),
                                detected_at: detected_at.clone(),
                            }],
                            team_confidence: compute_signal_confidence(1),
                            first_detected_at: detected_at,
                            suggested_action,
                            resolved: false,
                        },
                    );
                }
            }
            TeamOp::ResolveSignal { signal_id, .. } => {
                resolved_signals.insert(signal_id);
            }
            _ => {}
        }
    }

    let mut summaries: Vec<TeamSignalSummary> = signal_groups
        .into_values()
        .map(|mut s| {
            if resolved_signals.contains(&s.signal_id) {
                s.resolved = true;
            }
            s
        })
        .collect();

    summaries.sort_by(|a, b| {
        b.team_confidence
            .partial_cmp(&a.team_confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.first_detected_at.cmp(&a.first_detected_at))
    });

    Ok(summaries)
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get the full team intelligence profile.
#[tauri::command]
pub async fn get_team_profile_cmd() -> crate::error::Result<TeamProfile> {
    let team_id = get_team_id()?;
    let conn = crate::state::open_db_connection()?;
    get_team_profile(&conn, &team_id).map_err(|e| {
        warn!(target: "4da::team_intel", error = %e, "Failed to get team profile");
        format!("Failed to get team profile: {e}").into()
    })
}

/// Get team blind spots.
#[tauri::command]
pub async fn get_team_blind_spots_cmd() -> crate::error::Result<Vec<TeamBlindSpot>> {
    let team_id = get_team_id()?;
    let conn = crate::state::open_db_connection()?;
    get_team_blind_spots(&conn, &team_id).map_err(|e| {
        warn!(target: "4da::team_intel", error = %e, "Failed to get team blind spots");
        format!("Failed to get team blind spots: {e}").into()
    })
}

/// Get bus-factor risk report.
#[tauri::command]
pub async fn get_bus_factor_report_cmd() -> crate::error::Result<Vec<UniqueStrength>> {
    let team_id = get_team_id()?;
    let conn = crate::state::open_db_connection()?;
    get_bus_factor_report(&conn, &team_id).map_err(|e| {
        warn!(target: "4da::team_intel", error = %e, "Failed to get bus factor report");
        format!("Failed to get bus factor report: {e}").into()
    })
}

/// Get aggregated team signal summary.
#[tauri::command]
pub async fn get_team_signal_summary_cmd() -> crate::error::Result<Vec<TeamSignalSummary>> {
    let team_id = get_team_id()?;
    let conn = crate::state::open_db_connection()?;
    get_team_signal_summary(&conn, &team_id).map_err(|e| {
        warn!(target: "4da::team_intel", error = %e, "Failed to get team signals");
        format!("Failed to get team signals: {e}").into()
    })
}

/// Extract team_id from settings. Shared by all commands.
fn get_team_id() -> crate::error::Result<String> {
    let settings = crate::state::get_settings_manager().lock();
    let team_id = settings
        .get()
        .team_relay
        .as_ref()
        .and_then(|c| c.team_id.clone())
        .unwrap_or_default();
    drop(settings);

    if team_id.is_empty() {
        return Err("Team sync not configured".into());
    }
    Ok(team_id)
}
