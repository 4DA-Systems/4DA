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
use crate::evidence::{
    Action as EvidenceAction, Confidence, EvidenceCitation, EvidenceFeed, EvidenceItem,
    EvidenceKind, LensHints, Urgency,
};
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

        // Determine chain priority
        let has_security = links.iter().any(|l| l.signal_type == "security_alert");
        let has_breaking = links.iter().any(|l| l.signal_type == "breaking_change");

        let priority = if has_security {
            "critical"
        } else if has_breaking {
            "alert"
        } else if links.len() >= 3 {
            "advisory"
        } else {
            "watch"
        };

        let action = if has_security {
            format!("Review security implications for {topic} in your projects")
        } else if has_breaking {
            format!("Check if {topic} breaking changes affect your code")
        } else {
            format!("Multiple signals about {topic} - review the trend")
        };

        let chain_id = format!(
            "chain_{}_{}",
            topic,
            dates.iter().min().unwrap_or(&String::new())
        );

        let dep_match = has_dependency_match(conn, topic);
        let corroboration = (links.len() as f64 / 5.0).min(1.0);
        let severity = if has_security {
            1.0
        } else if has_breaking {
            0.7
        } else {
            0.3
        };
        // Weighted confidence: dep relevance matters most (50%), corroboration
        // from multiple sources adds credibility (30%), keyword-inferred
        // severity is least reliable (20%).
        let confidence = dep_match * 0.5 + corroboration * 0.3 + severity * 0.2;

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

/// Resolve a signal chain
pub fn resolve_chain(
    conn: &rusqlite::Connection,
    chain_id: &str,
    resolution: ChainResolution,
) -> Result<()> {
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

/// Signal chain with prediction attached
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalChainWithPrediction {
    #[serde(flatten)]
    pub chain: SignalChain,
    pub prediction: ChainPrediction,
}

// ============================================================================
// EvidenceItem conversion (Intelligence Reconciliation — Phase 5)
// ============================================================================

fn priority_str_to_urgency(priority: &str) -> Urgency {
    match priority {
        "critical" => Urgency::Critical,
        "high" => Urgency::High,
        "medium" => Urgency::Medium,
        _ => Urgency::Watch,
    }
}

fn truncate_chain_title(s: &str) -> String {
    s.trim_end_matches('.').chars().take(120).collect()
}

fn truncate_chain_note(s: &str) -> String {
    s.chars().take(200).collect()
}

fn chain_link_to_citation(link: &ChainLink) -> EvidenceCitation {
    let freshness_days =
        chrono::NaiveDateTime::parse_from_str(&link.timestamp, "%Y-%m-%d %H:%M:%S")
            .map(|dt| {
                let secs = chrono::Utc::now().timestamp() - dt.and_utc().timestamp();
                (secs as f32 / 86_400.0).max(0.0)
            })
            .unwrap_or(0.0);
    EvidenceCitation {
        source: link.signal_type.clone(),
        title: truncate_chain_title(&link.title),
        url: None,
        freshness_days,
        relevance_note: truncate_chain_note(&link.description),
    }
}

impl SignalChainWithPrediction {
    /// Convert to the canonical `EvidenceItem` with `kind: Chain`.
    /// Maps chain's overall_priority → urgency; the prediction's forecast
    /// becomes the explanation; links become citations. Lens hints:
    /// preemption + evidence (chains are forward-looking but also
    /// compound evidence of a developing pattern).
    pub fn to_evidence_item(&self) -> EvidenceItem {
        let title = truncate_chain_title(&format!("Chain: {}", self.chain.chain_name));
        let explanation = if self.prediction.forecast.is_empty() {
            self.chain.suggested_action.clone()
        } else {
            self.prediction.forecast.clone()
        };

        // Build citations from chain links (cap at 5 for scannable payload).
        let evidence: Vec<EvidenceCitation> = if self.chain.links.is_empty() {
            // Synthesize an inferred citation for the schema.
            vec![EvidenceCitation {
                source: "signal-chain-detector".to_string(),
                title: truncate_chain_title(&self.chain.chain_name),
                url: None,
                freshness_days: 0.0,
                relevance_note: truncate_chain_note("no concrete links yet"),
            }]
        } else {
            self.chain
                .links
                .iter()
                .take(5)
                .map(chain_link_to_citation)
                .collect()
        };

        // Affected deps = ONLY the chain's verified dependency (its topic matched the
        // user's actually-installed deps at build time). Never the old regex-split of the
        // chain_name, which emitted boilerplate ("signal", "chain") and topic words as
        // fabricated affected dependencies the user + adversarial LLM would trust.
        let affected_deps: Vec<String> = self
            .chain
            .verified_dep
            .clone()
            .map(|d| vec![d])
            .unwrap_or_default();

        let suggested_actions = if self.chain.suggested_action.is_empty() {
            vec![EvidenceAction {
                action_id: "investigate".to_string(),
                label: "Investigate".to_string(),
                description: "Review the signal chain's development.".to_string(),
            }]
        } else {
            vec![
                EvidenceAction {
                    action_id: "investigate".to_string(),
                    label: "Investigate".to_string(),
                    description: self.chain.suggested_action.clone(),
                },
                EvidenceAction {
                    action_id: "snooze_7d".to_string(),
                    label: "Snooze 7 days".to_string(),
                    description: "Pause this chain from surfacing for a week.".to_string(),
                },
            ]
        };

        EvidenceItem {
            id: format!("sc_{}", self.chain.id),
            kind: EvidenceKind::Chain,
            title,
            explanation,
            confidence: Confidence::heuristic((self.prediction.confidence as f32).clamp(0.0, 1.0)),
            urgency: priority_str_to_urgency(&self.chain.overall_priority),
            reversibility: None,
            evidence,
            affected_projects: Vec::new(),
            affected_deps,
            suggested_actions,
            precedents: Vec::new(),
            refutation_condition: None,
            lens_hints: LensHints {
                briefing: false,
                preemption: true,
                blind_spots: false,
                evidence: true,
            },
            created_at: chrono::Utc::now().timestamp_millis(),
            expires_at: None,
        }
    }
}

#[tauri::command]
pub fn get_signal_chains() -> Result<Vec<SignalChain>> {
    crate::settings::require_signal_feature("get_signal_chains")?;
    let conn = crate::open_db_connection()?;
    detect_chains(&conn)
}

/// Get signal chains with lifecycle predictions (Signal-gated)
#[tauri::command]
pub fn get_signal_chains_predicted() -> Result<EvidenceFeed> {
    crate::settings::require_signal_feature("get_signal_chains_predicted")?;
    let conn = crate::open_db_connection()?;
    let chains = detect_chains(&conn)?;
    let items: Vec<EvidenceItem> = chains
        .into_iter()
        .filter_map(|c| {
            let prediction = predict_chain_lifecycle(&c);
            if c.resolution != ChainResolution::Open {
                return None;
            }
            let wrapped = SignalChainWithPrediction {
                chain: c,
                prediction,
            };
            let item = wrapped.to_evidence_item();
            match crate::evidence::validate_item(&item) {
                Ok(()) => Some(item),
                Err(e) => {
                    tracing::warn!(
                        target: "4da::evidence::validate",
                        id = %item.id,
                        error = %e,
                        "dropped signal-chain item failing schema validation"
                    );
                    None
                }
            }
        })
        .collect();
    Ok(EvidenceFeed::from_items(items))
}

#[tauri::command]
pub fn resolve_signal_chain(chain_id: String, resolution: String) -> Result<()> {
    crate::settings::require_signal_feature("resolve_signal_chain")?;
    let conn = crate::open_db_connection()?;
    let res = match resolution.as_str() {
        "resolved" => ChainResolution::Resolved,
        "expired" => ChainResolution::Expired,
        "snoozed" => ChainResolution::Snoozed,
        _ => ChainResolution::Open,
    };
    resolve_chain(&conn, &chain_id, res)
}

// ============================================================================
// Tests — EvidenceItem conversion (Intelligence Reconciliation — Phase 5)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_link(signal_type: &str, title: &str) -> ChainLink {
        ChainLink {
            signal_type: signal_type.to_string(),
            source_item_id: 1,
            title: title.to_string(),
            timestamp: "2026-04-15 12:00:00".to_string(),
            description: format!("Signal from {signal_type}"),
        }
    }

    fn sample_chain_with_prediction() -> SignalChainWithPrediction {
        SignalChainWithPrediction {
            chain: SignalChain {
                id: "chain_tokio_cve".to_string(),
                chain_name: "tokio CVE disclosure + patch sequence".to_string(),
                links: vec![
                    sample_link("cve", "CVE-2026-1234 disclosed"),
                    sample_link("blog", "Tokio maintainers respond"),
                    sample_link("release", "Tokio 1.37 released with fix"),
                ],
                overall_priority: "high".to_string(),
                resolution: ChainResolution::Open,
                suggested_action: "Upgrade tokio to 1.37 immediately.".to_string(),
                confidence: 0.78,
                created_at: "2026-04-15 00:00:00".to_string(),
                updated_at: "2026-04-17 00:00:00".to_string(),
                verified_dep: Some("tokio".to_string()),
            },
            prediction: ChainPrediction {
                phase: ChainPhase::Escalating,
                intervals_hours: vec![36.0, 24.0, 12.0],
                acceleration: -0.3,
                predicted_next_hours: Some(8.0),
                confidence: 0.72,
                forecast: "Escalating — expect patch guidance within the day.".to_string(),
            },
        }
    }

    #[test]
    fn chain_maps_to_chain_kind() {
        let item = sample_chain_with_prediction().to_evidence_item();
        assert_eq!(item.kind, crate::evidence::EvidenceKind::Chain);
    }

    #[test]
    fn chain_priority_maps_to_urgency() {
        let mut c = sample_chain_with_prediction();
        c.chain.overall_priority = "critical".to_string();
        assert_eq!(
            c.to_evidence_item().urgency,
            crate::evidence::Urgency::Critical
        );
        c.chain.overall_priority = "high".to_string();
        assert_eq!(c.to_evidence_item().urgency, crate::evidence::Urgency::High);
        c.chain.overall_priority = "medium".to_string();
        assert_eq!(
            c.to_evidence_item().urgency,
            crate::evidence::Urgency::Medium
        );
        c.chain.overall_priority = "low".to_string();
        assert_eq!(
            c.to_evidence_item().urgency,
            crate::evidence::Urgency::Watch
        );
    }

    #[test]
    fn chain_forecast_is_explanation() {
        let item = sample_chain_with_prediction().to_evidence_item();
        assert!(item.explanation.contains("Escalating"));
    }

    #[test]
    fn affected_deps_use_verified_dep_only_not_chain_name_tokens() {
        // The chain_name is "tokio CVE disclosure + patch sequence" — the OLD regex split
        // would have emitted ["tokio","cve","disclosure"] (boilerplate/topic words as fake
        // deps). Now affected_deps is exactly the verified dependency, nothing else.
        let item = sample_chain_with_prediction().to_evidence_item();
        assert_eq!(item.affected_deps, vec!["tokio".to_string()]);

        // When the topic was NOT a verified dependency, claim no affected dep at all
        // (never fabricate one from the chain name).
        let mut c = sample_chain_with_prediction();
        c.chain.verified_dep = None;
        c.chain.chain_name = "security vulnerability updates signal chain (3 events)".to_string();
        let item = c.to_evidence_item();
        assert!(
            item.affected_deps.is_empty(),
            "no verified dep → no fabricated affected deps, got {:?}",
            item.affected_deps
        );
    }

    #[test]
    fn chain_falls_back_to_suggested_action_when_no_forecast() {
        let mut c = sample_chain_with_prediction();
        c.prediction.forecast.clear();
        let item = c.to_evidence_item();
        assert_eq!(item.explanation, "Upgrade tokio to 1.37 immediately.");
    }

    #[test]
    fn chain_citations_built_from_links() {
        let item = sample_chain_with_prediction().to_evidence_item();
        assert_eq!(item.evidence.len(), 3);
        assert_eq!(item.evidence[0].source, "cve");
    }

    #[test]
    fn chain_without_links_synthesizes_citation() {
        let mut c = sample_chain_with_prediction();
        c.chain.links.clear();
        let item = c.to_evidence_item();
        assert_eq!(item.evidence.len(), 1);
        assert_eq!(item.evidence[0].source, "signal-chain-detector");
    }

    #[test]
    fn chain_caps_citations_at_5() {
        let mut c = sample_chain_with_prediction();
        c.chain.links = (0..10)
            .map(|i| sample_link("hn", &format!("link #{i}")))
            .collect();
        let item = c.to_evidence_item();
        assert_eq!(item.evidence.len(), 5);
    }

    #[test]
    fn chain_lens_hints_preemption_and_evidence() {
        let item = sample_chain_with_prediction().to_evidence_item();
        assert!(item.lens_hints.preemption);
        assert!(item.lens_hints.evidence);
        assert!(!item.lens_hints.briefing);
        assert!(!item.lens_hints.blind_spots);
    }

    #[test]
    fn chain_passes_schema_validation() {
        let item = sample_chain_with_prediction().to_evidence_item();
        assert!(crate::evidence::validate_item(&item).is_ok());
    }

    #[test]
    fn chain_confidence_clamps_out_of_range() {
        let mut c = sample_chain_with_prediction();
        c.prediction.confidence = 1.5;
        let item = c.to_evidence_item();
        assert!(item.confidence.value >= 0.0 && item.confidence.value <= 1.0);
    }
}
