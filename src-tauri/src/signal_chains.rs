// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Signal Chains for 4DA (Temporal Causal Reasoning)
//!
//! Connects individual signals into causal chains over time.
//! "CVE Monday + your dep uses it Tuesday + patch released today = act now."

use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

use crate::error::Result;
#[path = "signal_chains_prediction.rs"]
mod signal_chains_prediction;
pub use signal_chains_prediction::*;

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
    pub confidence: f64,
    pub created_at: String,
    pub updated_at: String,
    /// The chain's topic IFF it exactly matches one of the user's actually-installed
    /// dependencies (verified at build via `has_dependency_match`). This is the ONLY
    /// trustworthy "affected dependency" for the chain — replacing the old heuristic that
    /// regex-split the chain_name and emitted boilerplate ("signal", "chain") and topic
    /// words as fake affected dependencies. `None` when the topic isn't a real dep.
    #[serde(default)]
    pub verified_dep: Option<String>,
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
pub fn detect_chains(conn: &rusqlite::Connection) -> Result<Vec<SignalChain>> {
    // Get recent signal-worthy source items (with signal classification)
    let mut stmt = conn.prepare(
        "SELECT si.id, si.title, si.source_type, si.created_at, si.content
             FROM source_items si
             WHERE si.created_at >= datetime('now', '-7 days')
             ORDER BY si.created_at DESC
             LIMIT 200",
    )?;

    let items: Vec<(i64, String, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get::<_, String>(4).unwrap_or_default(),
            ))
        })?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in signal_chains: {e}");
                None
            }
        })
        .collect();

    if items.is_empty() {
        return Ok(vec![]);
    }

    // Extract topics from each item and group by topic
    let mut topic_items: HashMap<String, Vec<(i64, String, String, String)>> = HashMap::new();

    for (id, title, source_type, created_at, content) in &items {
        let topics = crate::extract_topics(title, content, &[]);
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
                    description: format!("{signal_type} via {source_type}"),
                }
            })
            .collect();

        links.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        links.truncate(5);

        let has_security = links.iter().any(|l| l.signal_type == "security_alert");
        let has_breaking = links.iter().any(|l| l.signal_type == "breaking_change");

        // Verify installed-dependency relevance BEFORE assigning urgency. The
        // security_alert / breaking_change signal_type is KEYWORD-INFERRED from titles
        // (a "cve-" / "breaking change" substring), never OSV-verified — so it must not,
        // on its own, mint a "critical" alert. A chain only earns critical/alert urgency
        // (and full confidence) when it actually touches one of the user's installed
        // dependencies. Otherwise it is ecosystem awareness, not a personal threat.
        let dep_match = has_dependency_match(conn, topic);
        let has_dep = dep_match > 0.0;
        let policy = chain_policy(has_security, has_breaking, dep_match, links.len());
        let priority = policy.priority;
        let confidence = policy.confidence;

        let action = match (has_security, has_breaking, has_dep) {
            (true, _, true) => format!("Review security implications for {topic} in your projects"),
            (true, _, false) => format!(
                "Security activity around {topic} — not in your tracked dependencies, awareness only"
            ),
            (false, true, true) => format!("Check if {topic} breaking changes affect your code"),
            (false, true, false) => {
                format!("Breaking-change signals for {topic} — not currently in your stack")
            }
            _ => format!("Multiple signals about {topic} - review the trend"),
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
            confidence,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            // Topic is a real affected dependency only when it exactly matched the user's
            // installed deps above (dep_match > 0). Otherwise we claim no affected dep
            // rather than fabricate one from the chain name.
            verified_dep: if dep_match > 0.0 {
                Some(topic.to_string())
            } else {
                None
            },
        });
    }

    chains.retain(|c| c.confidence >= 0.3);

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

/// Confidence ceiling for a chain whose topic is NOT one of the user's installed
/// dependencies. Grounded chains start at ~0.43 (dep_match ≥ 0.5 → 0.25, plus the
/// minimum corroboration/severity), so this cap keeps every ungrounded chain strictly
/// below the grounded band — it can still surface as low-urgency awareness when
/// well-corroborated, but never out-ranks (or masquerades as) a chain that actually
/// touches the user's stack.
const UNGROUNDED_CONFIDENCE_CAP: f64 = 0.35;

/// Pure urgency/confidence policy for a detected chain, separated from DB access so the
/// grounding rules are unit-testable without a live database.
///
/// `dep_match` is the installed-dependency relevance (0.0 when the chain's topic is not a
/// tracked dependency). `has_security` / `has_breaking` are KEYWORD-INFERRED, so they only
/// escalate urgency when there is also a dependency match; without one the chain is capped
/// at `watch` and its confidence is held below the grounded band.
struct ChainPolicy {
    priority: &'static str,
    confidence: f64,
}

fn chain_policy(
    has_security: bool,
    has_breaking: bool,
    dep_match: f64,
    links_len: usize,
) -> ChainPolicy {
    let has_dep = dep_match > 0.0;

    let priority = if has_security && has_dep {
        "critical"
    } else if has_breaking && has_dep {
        "alert"
    } else if has_dep && links_len >= 3 {
        "advisory"
    } else {
        // Ungrounded (no installed dep), or a thin grounded signal → awareness only.
        "watch"
    };

    let corroboration = (links_len as f64 / 5.0).min(1.0);
    let severity = if has_security {
        1.0
    } else if has_breaking {
        0.7
    } else {
        0.3
    };
    // Weighted confidence: dep relevance matters most (50%), corroboration from
    // multiple sources adds credibility (30%), keyword-inferred severity is least
    // reliable (20%).
    let mut confidence = dep_match * 0.5 + corroboration * 0.3 + severity * 0.2;
    if !has_dep {
        confidence = confidence.min(UNGROUNDED_CONFIDENCE_CAP);
    }

    ChainPolicy {
        priority,
        confidence,
    }
}

fn has_dependency_match(conn: &rusqlite::Connection, topic: &str) -> f64 {
    let lower = topic.to_lowercase();
    // Check both user-curated and ACE-scanned dependencies with exact name match.
    // Substring LIKE was producing false positives ("node" matching "nodemon").
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (
                SELECT package_name FROM user_dependencies WHERE LOWER(package_name) = ?1
                UNION
                SELECT package_name FROM project_dependencies WHERE LOWER(package_name) = ?1
            )",
            params![lower],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if count >= 1 {
        (0.50 + (count as f64 * 0.12).min(0.40)).min(0.90)
    } else {
        0.0
    }
}

fn classify_chain_signal(title: &str) -> String {
    let lower = title.to_lowercase();
    // Security: strong signal keywords — false positive rate is low
    if lower.contains("cve-")
        || lower.contains("vulnerability")
        || lower.contains("security advisory")
        || lower.contains("exploit")
        || lower.contains("ghsa-")
    {
        return "security_alert".to_string();
    }
    // Breaking: require "breaking change" phrase or "deprecated" (strong signals).
    // Bare "removed" and "eol" produce too many false positives.
    if lower.contains("breaking change")
        || lower.contains("deprecated")
        || lower.contains("end of life")
        || lower.contains("end-of-life")
    {
        return "breaking_change".to_string();
    }
    // Release: require version-like patterns, not bare "update"/"launch"
    if lower.contains("released")
        || lower.contains("new release")
        || lower.contains(" v2")
        || lower.contains(" v3")
        || lower.contains(" v4")
        || lower.contains(" v5")
    {
        return "tool_discovery".to_string();
    }
    "learning".to_string()
}

fn priority_rank(priority: &str) -> u8 {
    match priority {
        "critical" => 0,
        "alert" => 1,
        "advisory" => 2,
        _ => 3, // "watch" and fallback
    }
}

// ============================================================================
// Tests — split into signal_chains_tests.rs to keep this file under the size
// limit (test files are exempt). Included via #[path] so they stay a child
// module with access to private items (chain_policy, UNGROUNDED_CONFIDENCE_CAP).
// ============================================================================

#[cfg(test)]
#[path = "signal_chains_tests.rs"]
mod tests;
