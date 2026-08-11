// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Signal Chains for 4DA (Temporal Causal Reasoning)
//!
//! Connects individual signals into causal chains over time.
//! "CVE Monday + your dep uses it Tuesday + patch released today = act now."

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

use crate::error::Result;
#[path = "signal_chains_candidates.rs"]
mod signal_chains_candidates;
#[path = "signal_chains_grounding.rs"]
mod signal_chains_grounding;
#[path = "signal_chains_persistence.rs"]
mod signal_chains_persistence;
#[path = "signal_chains_prediction.rs"]
mod signal_chains_prediction;
use signal_chains_candidates::{
    load_chain_candidate_items_by_id, load_recent_chain_candidate_items, ChainCandidateItem,
};
use signal_chains_grounding::{chain_policy, dependency_evidence};
use signal_chains_persistence::record_signal_chain_events;
pub use signal_chains_prediction::*;

const SIGNAL_CHAIN_WINDOW_DAYS: i64 = 7;
const SIGNAL_CHAIN_PER_SOURCE_DAY: usize = 25;
const SIGNAL_CHAIN_MAX_ITEMS: usize = 3_000;
type TopicChainItem = (i64, String, String, String, String);

/// Confidence ceiling for a chain whose topic is NOT one of the user's installed
/// dependencies. Grounded chains start at ~0.43 (dep_match >= 0.5 -> 0.25, plus the
/// minimum corroboration/severity), so this cap keeps every ungrounded chain strictly
/// below the grounded band. It can still surface as low-urgency awareness when
/// well-corroborated, but never out-ranks a chain that actually touches the user's stack.
pub(crate) const UNGROUNDED_CONFIDENCE_CAP: f64 = 0.35;

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
    /// dependencies (verified at build via `dependency_match_score`). This is the ONLY
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
    let items = load_recent_chain_candidate_items(conn)?;
    detect_chains_from_items(conn, items)
}

/// Detect current chains and refresh the persisted `temporal_events` snapshot
/// consumed by MCP/export surfaces. Detection remains available as a pure read
/// through [`detect_chains`]; this producer path is intentionally explicit.
pub fn detect_and_record_chains(conn: &rusqlite::Connection) -> Result<Vec<SignalChain>> {
    let chains = detect_chains(conn)?;
    match record_signal_chain_events(conn, &chains) {
        Ok(persisted) => {
            info!(
                target: "4da::signal_chains",
                persisted,
                "Signal chain temporal snapshot refreshed"
            );
        }
        Err(e) => {
            tracing::warn!(
                target: "4da::signal_chains",
                error = %e,
                "Signal chain temporal snapshot refresh failed"
            );
        }
    }
    Ok(chains)
}

/// Detect chains among a SPECIFIC item set (by id), instead of the global
/// 200-most-recent window.
///
/// The content graph needs this: it loads its nodes by RELEVANCE while
/// `detect_chains` reads by RECENCY — live-measured 2026-07-19, the two sets
/// shared 0 of 150 items, so graph chain edges could structurally never fire.
pub fn detect_chains_for_items(
    conn: &rusqlite::Connection,
    item_ids: &[i64],
) -> Result<Vec<SignalChain>> {
    if item_ids.is_empty() {
        return Ok(vec![]);
    }
    let items = load_chain_candidate_items_by_id(conn, item_ids)?;
    detect_chains_from_items(conn, items)
}

/// Core chain detection over an already-loaded item set.
fn detect_chains_from_items(
    conn: &rusqlite::Connection,
    items: Vec<ChainCandidateItem>,
) -> Result<Vec<SignalChain>> {
    if items.is_empty() {
        return Ok(vec![]);
    }

    let sampled_items = items.len();

    // Extract topics from each item and group by topic
    let mut topic_items: HashMap<String, Vec<TopicChainItem>> = HashMap::new();

    for (id, title, source_type, created_at, content, tags) in &items {
        let topics = crate::extract_topics(title, content, tags);
        for topic in topics {
            topic_items.entry(topic).or_default().push((
                *id,
                title.clone(),
                source_type.clone(),
                created_at.clone(),
                content.clone(),
            ));
        }
    }

    // Find chains: topics with 2+ items that span multiple days
    let mut chains = Vec::new();
    let topic_count = topic_items.len();
    let mut candidate_topics = 0_usize;
    let mut multi_day_topics = 0_usize;
    let mut rejected_same_day = 0_usize;
    let mut rejected_low_confidence = 0_usize;

    for (topic, topic_items_list) in &topic_items {
        if topic_items_list.len() < 2 {
            continue;
        }
        candidate_topics += 1;

        // Check if items span at least 2 different days
        let dates: std::collections::HashSet<String> = topic_items_list
            .iter()
            .filter_map(|(_, _, _, ts, _)| ts.get(..10).map(String::from))
            .collect();

        if dates.len() < 2 {
            rejected_same_day += 1;
            continue;
        }
        multi_day_topics += 1;

        // Verify installed-dependency relevance BEFORE assigning urgency. The
        // security_alert / breaking_change signal_type is KEYWORD-INFERRED from titles
        // (a "cve-" / "breaking change" substring), never OSV-verified — so it must not,
        // on its own, mint a "critical" alert. A chain only earns critical/alert urgency
        // (and full confidence) when it actually touches one of the user's installed
        // dependencies. Otherwise it is ecosystem awareness, not a personal threat.
        let dep_evidence = dependency_evidence(conn, topic, topic_items_list);
        let dep_match = dep_evidence.score;
        let has_dep = dep_match > 0.0;

        // Classify signal types based on keywords. For a verified dependency chain,
        // display only item-level grounded links; otherwise one real package hit plus
        // several ordinary same-word articles can masquerade as a personal chain.
        let mut links: Vec<ChainLink> = topic_items_list
            .iter()
            .filter(|(id, _, _, _, _)| !has_dep || dep_evidence.grounded_item_ids.contains(id))
            .map(|(id, title, source_type, timestamp, _)| {
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

        // Keep the most decision-relevant links before timeline display. A
        // critical chain must not hide the grounded security item merely because
        // five older learning links came first chronologically.
        links.sort_by(|a, b| {
            signal_type_rank(&a.signal_type)
                .cmp(&signal_type_rank(&b.signal_type))
                .then(a.timestamp.cmp(&b.timestamp))
                .then(a.source_item_id.cmp(&b.source_item_id))
        });
        links.truncate(5);
        links.sort_by(|a, b| {
            a.timestamp
                .cmp(&b.timestamp)
                .then(a.source_item_id.cmp(&b.source_item_id))
        });

        let has_security = links.iter().any(|l| l.signal_type == "security_alert");
        let has_breaking = links.iter().any(|l| l.signal_type == "breaking_change");

        let policy = chain_policy(
            dep_evidence.security_signal,
            dep_evidence.breaking_signal,
            dep_match,
            links.len(),
        );
        let priority = policy.priority;
        let confidence = policy.confidence;
        if confidence < 0.3 {
            rejected_low_confidence += 1;
            continue;
        }

        let action = if dep_evidence.security_signal {
            format!("Review security implications for {topic} in your projects")
        } else if has_security {
            format!(
                "Security activity around {topic} — not confirmed against your tracked dependencies"
            )
        } else if dep_evidence.breaking_signal {
            format!("Check if {topic} breaking changes affect your code")
        } else if has_breaking {
            format!("Breaking-change signals for {topic} — not confirmed against your stack")
        } else if has_dep {
            format!("Multiple signals about {topic} in your stack - review the trend")
        } else {
            format!("Multiple signals about {topic} - review the trend")
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

    // Sort by priority. The id tie-break matters: chains come out of a
    // HashMap, and equal (priority, length) chains would otherwise keep
    // hash-random relative order — truncate(10) below then picks different
    // survivors per run (found via the content-graph determinism audit,
    // 2026-07-19).
    chains.sort_by(|a, b| {
        priority_rank(&a.overall_priority)
            .cmp(&priority_rank(&b.overall_priority))
            .then(b.links.len().cmp(&a.links.len()))
            .then_with(|| a.id.cmp(&b.id))
    });

    chains.truncate(10);
    info!(
        target: "4da::signal_chains",
        sampled_items,
        topics = topic_count,
        candidate_topics,
        multi_day_topics,
        rejected_same_day,
        rejected_low_confidence,
        chains = chains.len(),
        "Signal chain detection complete"
    );
    Ok(chains)
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

fn signal_type_rank(signal_type: &str) -> u8 {
    match signal_type {
        "security_alert" => 0,
        "breaking_change" => 1,
        "tool_discovery" => 2,
        _ => 3,
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
