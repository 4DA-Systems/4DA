//! Signal Chains for 4DA (Temporal Causal Reasoning)
//!
//! Connects individual signals into causal chains over time.
//! "CVE Monday + your dep uses it Tuesday + patch released today = act now."

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

use crate::error::Result;

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

    // Escalating: intervals shrinking significantly (acceleration < -2 hours/step)
    if acceleration < -2.0 && link_count >= 3 {
        return ChainPhase::Escalating;
    }

    // Peak: many signals with small intervals
    if link_count >= 4 && avg_interval < 24.0 {
        return ChainPhase::Peak;
    }

    // Resolving: intervals growing (acceleration > 4 hours/step) or very old last signal
    if acceleration > 4.0 && link_count >= 3 {
        return ChainPhase::Resolving;
    }

    ChainPhase::Active
}

/// Predict the next interval based on trend
fn predict_next_interval(intervals: &[f64], acceleration: f64) -> Option<f64> {
    if intervals.is_empty() {
        return None;
    }

    let last = *intervals.last().unwrap();
    let predicted = last + acceleration;

    // Clamp to reasonable range (1 hour to 7 days)
    Some(predicted.max(1.0).min(168.0))
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

    (data_confidence * 0.6 + regularity * 0.4).min(0.95)
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
                format!("within ~{:.0}h", h)
            } else {
                format!("within ~{:.0} days", h / 24.0)
            }
        })
        .unwrap_or_else(|| "timing uncertain".to_string());

    match phase {
        ChainPhase::Nascent => format!("Early signal for {} — monitoring", chain_name),
        ChainPhase::Active => format!(
            "{} is developing — next signal expected {}",
            chain_name, timing
        ),
        ChainPhase::Escalating => {
            let rate = if acceleration < -5.0 {
                "rapidly"
            } else {
                "steadily"
            };
            format!("{} is {} accelerating — act {}", chain_name, rate, timing)
        }
        ChainPhase::Peak => format!(
            "{} at peak intensity — high activity expected {}",
            chain_name, timing
        ),
        ChainPhase::Resolving => format!("{} is cooling down — signals slowing", chain_name),
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

#[tauri::command]
pub fn get_signal_chains() -> Result<Vec<SignalChain>> {
    crate::settings::require_pro_feature("get_signal_chains")?;
    let conn = crate::open_db_connection()?;
    detect_chains(&conn)
}

/// Get signal chains with lifecycle predictions (Pro)
#[tauri::command]
pub fn get_signal_chains_predicted() -> Result<Vec<SignalChainWithPrediction>> {
    crate::settings::require_pro_feature("get_signal_chains_predicted")?;
    let conn = crate::open_db_connection()?;
    let chains = detect_chains(&conn)?;
    Ok(chains
        .into_iter()
        .map(|c| {
            let prediction = predict_chain_lifecycle(&c);
            SignalChainWithPrediction {
                chain: c,
                prediction,
            }
        })
        .collect())
}

#[tauri::command]
pub fn resolve_signal_chain(chain_id: String, resolution: String) -> Result<()> {
    let conn = crate::open_db_connection()?;
    let res = match resolution.as_str() {
        "resolved" => ChainResolution::Resolved,
        "expired" => ChainResolution::Expired,
        "snoozed" => ChainResolution::Snoozed,
        _ => ChainResolution::Open,
    };
    resolve_chain(&conn, &chain_id, res)
}

#[cfg(test)]
#[path = "signal_chains_tests.rs"]
mod tests;
