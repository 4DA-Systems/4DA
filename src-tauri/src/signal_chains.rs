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
use crate::scoring_config;
use crate::evidence::{
    Action as EvidenceAction, Confidence, EvidenceCitation, EvidenceFeed, EvidenceItem,
    EvidenceKind, LensHints, Urgency,
};

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
    let result = conn.query_row(
        "SELECT COUNT(*) FROM user_dependencies WHERE LOWER(package_name) LIKE ?1",
        params![format!("%{}%", lower)],
        |row| row.get::<_, i64>(0),
    );
    match result {
        // Graduated match: single project = moderate confidence,
        // multiple projects using the same dep = higher confidence.
        Ok(count) if count >= 1 => {
            // Graduate: each project adds confidence, diminishing returns
            (0.50 + (count as f64 * 0.12).min(0.40)).min(0.90)
        }
        _ => 0.0,
    }
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
        "alert" => 1,
        "advisory" => 2,
        _ => 3, // "watch" and fallback
    }
}

// ============================================================================
// Chain Lifecycle Prediction
// ============================================================================

/// Lifecycle phase of a signal chain
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChainPhase {
    /// Just detected, 1-2 signals — may fizzle
    Nascent,
    /// Multiple signals confirmed, pattern emerging
    Active,
    /// Signal frequency increasing (acceleration detected)
    Escalating,
    /// Highest signal density, maximum relevance
    Peak,
    /// Signals slowing, topic fading
    Resolving,
}

/// Prediction attached to a signal chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainPrediction {
    /// Current lifecycle phase
    pub phase: ChainPhase,
    /// Inter-event intervals in hours (newest first)
    pub intervals_hours: Vec<f64>,
    /// Acceleration: negative = speeding up, positive = slowing down
    pub acceleration: f64,
    /// Estimated hours until next signal (based on trend)
    pub predicted_next_hours: Option<f64>,
    /// Confidence in prediction (0.0 - 1.0)
    pub confidence: f64,
    /// Human-readable forecast
    pub forecast: String,
}

/// Analyze a chain's lifecycle and generate predictions
pub fn predict_chain_lifecycle(chain: &SignalChain) -> ChainPrediction {
    let links = &chain.links;

    if links.len() < 2 {
        return ChainPrediction {
            phase: ChainPhase::Nascent,
            intervals_hours: vec![],
            acceleration: 0.0,
            predicted_next_hours: None,
            confidence: 0.1,
            forecast: "Too early to predict — watching for more signals".to_string(),
        };
    }

    // Calculate inter-event intervals in hours
    let intervals = compute_intervals(links);
    let acceleration = compute_acceleration(&intervals);

    // Determine phase
    let phase = classify_phase(links.len(), acceleration, &intervals);

    // Predict next event timing
    let predicted_next = predict_next_interval(&intervals, acceleration);
    let confidence = compute_confidence(links.len(), &intervals);

    let forecast = build_forecast(&phase, &chain.chain_name, predicted_next, acceleration);

    ChainPrediction {
        phase,
        intervals_hours: intervals,
        acceleration,
        predicted_next_hours: predicted_next,
        confidence,
        forecast,
    }
}

/// Compute time intervals between consecutive chain links (in hours)
fn compute_intervals(links: &[ChainLink]) -> Vec<f64> {
    if links.len() < 2 {
        return vec![];
    }

    let timestamps: Vec<chrono::DateTime<chrono::Utc>> = links
        .iter()
        .filter_map(|l| chrono::DateTime::parse_from_rfc3339(&l.timestamp).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .collect();

    if timestamps.len() < 2 {
        return vec![];
    }

    timestamps
        .windows(2)
        .map(|w| {
            let diff = w[1] - w[0];
            diff.num_minutes() as f64 / 60.0
        })
        .collect()
}

/// Compute acceleration: slope of interval changes (negative = speeding up)
fn compute_acceleration(intervals: &[f64]) -> f64 {
    if intervals.len() < 2 {
        return 0.0;
    }

    // Simple linear regression on interval sequence
    let n = intervals.len() as f64;
    let sum_x: f64 = (0..intervals.len()).map(|i| i as f64).sum();
    let sum_y: f64 = intervals.iter().sum();
    let sum_xy: f64 = intervals
        .iter()
        .enumerate()
        .map(|(i, y)| i as f64 * y)
        .sum();
    let sum_x2: f64 = (0..intervals.len()).map(|i| (i as f64).powi(2)).sum();

    let denom = n * sum_x2 - sum_x.powi(2);
    if denom.abs() < 1e-10 {
        return 0.0;
    }

    (n * sum_xy - sum_x * sum_y) / denom
}

/// Classify chain lifecycle phase
fn classify_phase(link_count: usize, acceleration: f64, intervals: &[f64]) -> ChainPhase {
    if link_count <= 2 {
        return ChainPhase::Nascent;
    }

    let avg_interval = if intervals.is_empty() {
        f64::MAX
    } else {
        intervals.iter().sum::<f64>() / intervals.len() as f64
    };

    if acceleration < scoring_config::SIGNAL_CHAIN_PHASE_ESCALATING_ACCELERATION as f64
        && link_count >= scoring_config::SIGNAL_CHAIN_PHASE_ESCALATING_MIN_LINKS as usize
    {
        return ChainPhase::Escalating;
    }

    if link_count >= scoring_config::SIGNAL_CHAIN_PHASE_PEAK_MIN_LINKS as usize
        && avg_interval < scoring_config::SIGNAL_CHAIN_PHASE_PEAK_MAX_INTERVAL as f64
    {
        return ChainPhase::Peak;
    }

    if acceleration > scoring_config::SIGNAL_CHAIN_PHASE_RESOLVING_ACCELERATION as f64
        && link_count >= scoring_config::SIGNAL_CHAIN_PHASE_RESOLVING_MIN_LINKS as usize
    {
        return ChainPhase::Resolving;
    }

    ChainPhase::Active
}

/// Predict the next interval based on trend
fn predict_next_interval(intervals: &[f64], acceleration: f64) -> Option<f64> {
    if intervals.is_empty() {
        return None;
    }

    let last = *intervals.last()?;
    let predicted = last + acceleration;

    Some(predicted.clamp(
        scoring_config::SIGNAL_CHAIN_PREDICTION_MIN_HOURS as f64,
        scoring_config::SIGNAL_CHAIN_PREDICTION_MAX_HOURS as f64,
    ))
}

/// Compute prediction confidence based on data quality
fn compute_confidence(link_count: usize, intervals: &[f64]) -> f64 {
    // Base: more data = more confidence
    let data_confidence = (link_count as f64 / 6.0).min(1.0);

    // Regularity: consistent intervals = more predictable
    let regularity = if intervals.len() >= 2 {
        let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
        let variance =
            intervals.iter().map(|i| (i - mean).powi(2)).sum::<f64>() / intervals.len() as f64;
        let cv = if mean > 0.0 {
            variance.sqrt() / mean
        } else {
            1.0
        };
        (1.0 - cv.min(1.0)).max(0.0)
    } else {
        0.3
    };

    (data_confidence * 0.6 + regularity * 0.4).min(0.85)
}

/// Build a human-readable forecast
fn build_forecast(
    phase: &ChainPhase,
    chain_name: &str,
    predicted_hours: Option<f64>,
    acceleration: f64,
) -> String {
    let timing = predicted_hours
        .map(|h| {
            if h < 2.0 {
                "within hours".to_string()
            } else if h < 24.0 {
                format!("within ~{h:.0}h")
            } else {
                format!("within ~{:.0} days", h / 24.0)
            }
        })
        .unwrap_or_else(|| "timing uncertain".to_string());

    match phase {
        ChainPhase::Nascent => format!("Early signal for {chain_name} — monitoring"),
        ChainPhase::Active => format!("{chain_name} is developing — next signal expected {timing}"),
        ChainPhase::Escalating => {
            let rate = if acceleration < -5.0 {
                "rapidly"
            } else {
                "steadily"
            };
            format!("{chain_name} is {rate} accelerating — act {timing}")
        }
        ChainPhase::Peak => {
            format!("{chain_name} at peak intensity — high activity expected {timing}")
        }
        ChainPhase::Resolving => format!("{chain_name} is cooling down — signals slowing"),
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
// Ecosystem Trend Velocity
// ============================================================================

/// Compute per-topic velocity over the last 7 days.
///
/// Velocity = (items_last_3_days - items_prior_4_days) / max(items_prior_4_days, 1)
///
/// Positive velocity = rising trend, negative = declining.
/// Topics are extracted from source_items titles using the standard `extract_topics` utility.
#[allow(dead_code)] // Reason: available for scoring pipeline integration (Phase 4 wiring pending)
pub fn compute_topic_velocity(conn: &rusqlite::Connection) -> HashMap<String, f32> {
    // Items from last 3 days (recent window)
    let recent_items = query_items_in_range(conn, "-3 days");
    // Items from 4-7 days ago (baseline window)
    let baseline_items = query_items_in_range_between(conn, "-7 days", "-3 days");

    // Count topics in each window
    let recent_counts = count_topics(&recent_items);
    let baseline_counts = count_topics(&baseline_items);

    // Merge all topic keys
    let all_topics: std::collections::HashSet<&String> =
        recent_counts.keys().chain(baseline_counts.keys()).collect();

    let mut velocities = HashMap::new();
    for topic in all_topics {
        let recent = *recent_counts.get(topic).unwrap_or(&0) as f32;
        let baseline = *baseline_counts.get(topic).unwrap_or(&0) as f32;
        let denominator = baseline.max(1.0);
        let velocity = (recent - baseline) / denominator;
        velocities.insert(topic.clone(), velocity);
    }

    velocities
}

/// Query source items created within a recent window (e.g., "-3 days" from now).
fn query_items_in_range(conn: &rusqlite::Connection, offset: &str) -> Vec<(String, String)> {
    let sql = format!(
        "SELECT title, COALESCE(content, '') FROM source_items
         WHERE created_at >= datetime('now', '{offset}')
         ORDER BY created_at DESC LIMIT 500"
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })
    .ok()
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// Query source items created between two offsets (e.g., "-7 days" to "-3 days").
fn query_items_in_range_between(
    conn: &rusqlite::Connection,
    start_offset: &str,
    end_offset: &str,
) -> Vec<(String, String)> {
    let sql = format!(
        "SELECT title, COALESCE(content, '') FROM source_items
         WHERE created_at >= datetime('now', '{start_offset}')
           AND created_at < datetime('now', '{end_offset}')
         ORDER BY created_at DESC LIMIT 500"
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })
    .ok()
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// Count topic occurrences across a list of (title, content) pairs.
fn count_topics(items: &[(String, String)]) -> HashMap<String, u32> {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for (title, content) in items {
        let topics = crate::extract_topics(title, content, &[]);
        for topic in topics {
            *counts.entry(topic).or_insert(0) += 1;
        }
    }
    counts
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

        // Extract affected deps from the chain_name heuristically: chain
        // names often include the dep ("tokio — CVE + ripout + patch").
        // Leave empty when ambiguous.
        let affected_deps: Vec<String> = self
            .chain
            .chain_name
            .split(|c: char| !c.is_alphanumeric() && c != '-')
            .filter(|s| s.len() >= 3 && s.len() <= 40)
            .take(3)
            .map(str::to_lowercase)
            .collect();

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

    #[test]
    fn chain_extracts_affected_deps_from_name() {
        let item = sample_chain_with_prediction().to_evidence_item();
        // "tokio CVE disclosure + patch sequence" → tokens ≥3 chars
        assert!(item.affected_deps.iter().any(|d| d == "tokio"));
    }
}
