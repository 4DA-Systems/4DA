// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Cross-team signal correlation — detects signals appearing across
//! multiple teams in an organization within a 48-hour window.

use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::error::Result;

use super::CrossTeamCorrelation;

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
