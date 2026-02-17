//! Signal Chains for 4DA (Temporal Causal Reasoning)
//!
//! Connects individual signals into causal chains over time.
//! "CVE Monday + your dep uses it Tuesday + patch released today = act now."

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalChain {
    pub id: String,
    pub chain_name: String,
    pub links: Vec<ChainLink>,
    pub overall_priority: String,
    pub resolution: ChainResolution,
    pub suggested_action: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainLink {
    pub signal_type: String,
    pub source_item_id: i64,
    pub title: String,
    pub timestamp: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChainResolution {
    Open,
    Resolved,
    Expired,
    Snoozed,
}

// ============================================================================
// Implementation
// ============================================================================

/// Detect signal chains from recent temporal events
pub fn detect_chains(conn: &rusqlite::Connection) -> Result<Vec<SignalChain>, String> {
    // Get recent signal-worthy source items (with signal classification)
    let mut stmt = conn
        .prepare(
            "SELECT si.id, si.title, si.source_type, si.created_at, si.content
             FROM source_items si
             WHERE si.created_at >= datetime('now', '-7 days')
             ORDER BY si.created_at DESC
             LIMIT 200",
        )
        .map_err(|e| e.to_string())?;

    let items: Vec<(i64, String, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get::<_, String>(4).unwrap_or_default(),
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    if items.is_empty() {
        return Ok(vec![]);
    }

    // Extract topics from each item and group by topic
    let mut topic_items: HashMap<String, Vec<(i64, String, String, String)>> = HashMap::new();

    for (id, title, source_type, created_at, content) in &items {
        let topics = crate::extract_topics(title, content);
        for topic in topics {
            topic_items.entry(topic).or_default().push((
                *id,
                title.clone(),
                source_type.clone(),
                created_at.clone(),
            ));
        }
    }

    // Find chains: topics with 2+ items that span multiple days
    let mut chains = Vec::new();

    for (topic, topic_items_list) in &topic_items {
        if topic_items_list.len() < 2 {
            continue;
        }

        // Check if items span at least 2 different days
        let dates: std::collections::HashSet<String> = topic_items_list
            .iter()
            .filter_map(|(_, _, _, ts)| ts.get(..10).map(String::from))
            .collect();

        if dates.len() < 2 {
            continue;
        }

        // Classify signal types based on keywords
        let mut links: Vec<ChainLink> = topic_items_list
            .iter()
            .map(|(id, title, source_type, timestamp)| {
                let signal_type = classify_chain_signal(title);
                ChainLink {
                    signal_type: signal_type.clone(),
                    source_item_id: *id,
                    title: title.clone(),
                    timestamp: timestamp.clone(),
                    description: format!("{} via {}", signal_type, source_type),
                }
            })
            .collect();

        links.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        links.truncate(5);

        // Determine chain priority
        let has_security = links.iter().any(|l| l.signal_type == "security_alert");
        let has_breaking = links.iter().any(|l| l.signal_type == "breaking_change");

        let priority = if has_security {
            "critical"
        } else if has_breaking {
            "high"
        } else if links.len() >= 3 {
            "medium"
        } else {
            "low"
        };

        let action = if has_security {
            format!(
                "Review security implications for {} in your projects",
                topic
            )
        } else if has_breaking {
            format!("Check if {} breaking changes affect your code", topic)
        } else {
            format!("Multiple signals about {} - review the trend", topic)
        };

        let chain_id = format!(
            "chain_{}_{}",
            topic,
            dates.iter().min().unwrap_or(&String::new())
        );

        chains.push(SignalChain {
            id: chain_id,
            chain_name: format!("{} signal chain ({} events)", topic, links.len()),
            links,
            overall_priority: priority.to_string(),
            resolution: ChainResolution::Open,
            suggested_action: action,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        });
    }

    // Sort by priority
    chains.sort_by(|a, b| {
        priority_rank(&a.overall_priority)
            .cmp(&priority_rank(&b.overall_priority))
            .then(b.links.len().cmp(&a.links.len()))
    });

    chains.truncate(10);
    info!(target: "4da::signal_chains", chains = chains.len(), "Signal chain detection complete");
    Ok(chains)
}

fn classify_chain_signal(title: &str) -> String {
    let lower = title.to_lowercase();
    if lower.contains("cve")
        || lower.contains("vulnerability")
        || lower.contains("security")
        || lower.contains("exploit")
    {
        "security_alert".to_string()
    } else if lower.contains("breaking")
        || lower.contains("deprecated")
        || lower.contains("removed")
        || lower.contains("eol")
    {
        "breaking_change".to_string()
    } else if lower.contains("release")
        || lower.contains("update")
        || lower.contains("v2")
        || lower.contains("v3")
        || lower.contains("launch")
    {
        "tool_discovery".to_string()
    } else if lower.contains("trend")
        || lower.contains("adoption")
        || lower.contains("growing")
        || lower.contains("popular")
    {
        "tech_trend".to_string()
    } else {
        "learning".to_string()
    }
}

fn priority_rank(priority: &str) -> u8 {
    match priority {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        _ => 3,
    }
}

/// Resolve a signal chain
pub fn resolve_chain(
    conn: &rusqlite::Connection,
    chain_id: &str,
    resolution: ChainResolution,
) -> Result<(), String> {
    let data = serde_json::json!({
        "chain_id": chain_id,
        "resolution": resolution,
    });

    crate::temporal::record_event(conn, "chain_resolved", chain_id, &data, None, None)?;

    info!(target: "4da::signal_chains", chain_id, resolution = ?resolution, "Chain resolved");
    Ok(())
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_signal_chains() -> Result<Vec<SignalChain>, String> {
    crate::settings::require_pro_feature("get_signal_chains")?;
    let conn = crate::open_db_connection()?;
    detect_chains(&conn)
}

#[tauri::command]
pub fn resolve_signal_chain(chain_id: String, resolution: String) -> Result<(), String> {
    let conn = crate::open_db_connection()?;
    let res = match resolution.as_str() {
        "resolved" => ChainResolution::Resolved,
        "expired" => ChainResolution::Expired,
        "snoozed" => ChainResolution::Snoozed,
        _ => ChainResolution::Open,
    };
    resolve_chain(&conn, &chain_id, res)
}
