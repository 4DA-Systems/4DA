// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Preemption Engine for 4DA
//!
//! Orchestrates forward-looking intelligence by combining signal chains,
//! project health, knowledge gaps, and attention analysis into ranked
//! preemptive alerts. Tells the user what matters BEFORE it becomes painful.

use serde::{Deserialize, Serialize};
use tracing::warn;
use ts_rs::TS;

use crate::error::Result;
use crate::knowledge_decay::GapSeverity;
use crate::signal_chains::ChainResolution;

// ============================================================================
// Types
// ============================================================================

/// Category of preemption alert.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum PreemptionType {
    SecurityAdvisory,
    BreakingChange,
    MigrationWindow,
    EcosystemShift,
    MaintainerDecline,
    KnowledgeBlindSpot,
}

/// How urgently the user should act on this alert.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum AlertUrgency {
    Critical,
    High,
    Medium,
    Watch,
}

/// A single piece of evidence backing a preemption alert.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlertEvidence {
    pub source: String,
    pub title: String,
    pub url: Option<String>,
    pub freshness_days: f32,
    pub relevance_score: f32,
}

/// An action the user can take in response to an alert.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SuggestedAction {
    /// One of: "dismiss", "watch", "investigate", "review_decision"
    pub action_type: String,
    pub label: String,
    pub description: String,
}

/// A single preemption alert combining evidence from multiple intelligence sources.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PreemptionAlert {
    pub id: String,
    pub alert_type: PreemptionType,
    pub title: String,
    pub explanation: String,
    pub evidence: Vec<AlertEvidence>,
    pub affected_projects: Vec<String>,
    pub affected_dependencies: Vec<String>,
    pub urgency: AlertUrgency,
    pub confidence: f32,
    pub predicted_window: Option<String>,
    pub suggested_actions: Vec<SuggestedAction>,
    pub created_at: String,
}

/// The full preemption feed with summary counts.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PreemptionFeed {
    pub alerts: Vec<PreemptionAlert>,
    pub total: usize,
    pub critical_count: usize,
    pub high_count: usize,
}

// ============================================================================
// Implementation
// ============================================================================

/// Generate the preemption feed by combining all intelligence sources.
pub fn get_preemption_feed() -> Result<PreemptionFeed> {
    let conn = crate::open_db_connection()?;
    let mut alerts = Vec::new();

    // 1. Signal chain predictions
    match crate::signal_chains::detect_chains(&conn) {
        Ok(chains) => {
            for chain in &chains {
                let prediction = crate::signal_chains::predict_chain_lifecycle(chain);
                if prediction.confidence > 0.4 && chain.resolution == ChainResolution::Open {
                    alerts.push(chain_to_alert(chain, &prediction));
                }
            }
        }
        Err(e) => warn!(target: "4da::preemption", error = %e, "Failed to detect signal chains"),
    }

    // 2. Project health alerts
    match crate::project_health::compute_all_project_health(&conn) {
        Ok(health_list) => {
            for ph in &health_list {
                if ph.overall_score < 0.6 {
                    alerts.push(health_to_alert(ph));
                }
                for alert in &ph.alerts {
                    if alert.severity == "critical" || alert.severity == "high" {
                        alerts.push(health_alert_to_preemption(ph, alert));
                    }
                }
            }
        }
        Err(e) => warn!(target: "4da::preemption", error = %e, "Failed to compute project health"),
    }

    // 3. Knowledge gaps as blind-spot alerts
    match crate::knowledge_decay::detect_knowledge_gaps(&conn) {
        Ok(gaps) => {
            for gap in &gaps {
                if gap.gap_severity == GapSeverity::Critical
                    || gap.gap_severity == GapSeverity::High
                {
                    alerts.push(gap_to_alert(gap));
                }
            }
        }
        Err(e) => warn!(target: "4da::preemption", error = %e, "Failed to detect knowledge gaps"),
    }

    // Sort: Critical first, then High, Medium, Watch. Within same urgency, highest confidence first.
    alerts.sort_by(|a, b| {
        urgency_rank(&a.urgency)
            .cmp(&urgency_rank(&b.urgency))
            .then(
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    let critical_count = alerts
        .iter()
        .filter(|a| matches!(a.urgency, AlertUrgency::Critical))
        .count();
    let high_count = alerts
        .iter()
        .filter(|a| matches!(a.urgency, AlertUrgency::High))
        .count();
    let total = alerts.len();

    Ok(PreemptionFeed {
        alerts,
        total,
        critical_count,
        high_count,
    })
}

// ============================================================================
// Converters
// ============================================================================

/// Convert a signal chain + its lifecycle prediction into a preemption alert.
fn chain_to_alert(
    chain: &crate::signal_chains::SignalChain,
    prediction: &crate::signal_chains::ChainPrediction,
) -> PreemptionAlert {
    use crate::signal_chains::ChainPhase;

    let urgency = match &prediction.phase {
        ChainPhase::Escalating | ChainPhase::Peak => {
            if chain.overall_priority == "critical" {
                AlertUrgency::Critical
            } else {
                AlertUrgency::High
            }
        }
        ChainPhase::Active => AlertUrgency::Medium,
        ChainPhase::Nascent | ChainPhase::Resolving => AlertUrgency::Watch,
    };

    let alert_type = classify_chain_type(&chain.chain_name);

    let predicted_window = prediction
        .predicted_next_hours
        .map(|h| format_time_window(h));

    let evidence: Vec<AlertEvidence> = chain
        .links
        .iter()
        .map(|link| {
            let freshness = freshness_from_timestamp(&link.timestamp);
            AlertEvidence {
                source: link.signal_type.clone(),
                title: link.title.clone(),
                url: None,
                freshness_days: freshness,
                relevance_score: chain.confidence as f32,
            }
        })
        .collect();

    let suggested_actions = vec![
        SuggestedAction {
            action_type: "investigate".to_string(),
            label: format!("Investigate {}", chain.chain_name),
            description: chain.suggested_action.clone(),
        },
        SuggestedAction {
            action_type: "watch".to_string(),
            label: "Monitor chain".to_string(),
            description: format!(
                "Keep watching — {} signals tracked so far",
                chain.links.len()
            ),
        },
    ];

    PreemptionAlert {
        id: uuid::Uuid::new_v4().to_string(),
        alert_type,
        title: format!("{} — {}", chain.chain_name, prediction.forecast),
        explanation: format!(
            "Signal chain \"{}\" is in {} phase with {} confidence. {}",
            chain.chain_name,
            phase_label(&prediction.phase),
            format_confidence(prediction.confidence),
            prediction.forecast
        ),
        evidence,
        affected_projects: vec![],
        affected_dependencies: vec![],
        urgency,
        confidence: prediction.confidence as f32,
        predicted_window,
        suggested_actions,
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Convert a low-scoring project health report into a preemption alert.
fn health_to_alert(ph: &crate::project_health::ProjectHealth) -> PreemptionAlert {
    let urgency = if ph.overall_score < 0.3 {
        AlertUrgency::Critical
    } else if ph.overall_score < 0.45 {
        AlertUrgency::High
    } else {
        AlertUrgency::Medium
    };

    let weak_dimensions: Vec<String> = [
        (&ph.freshness, "freshness"),
        (&ph.security, "security"),
        (&ph.momentum, "momentum"),
        (&ph.community, "community"),
    ]
    .iter()
    .filter(|(dim, _)| dim.score < 0.5)
    .map(|(_, name)| (*name).to_string())
    .collect();

    let explanation = if weak_dimensions.is_empty() {
        format!(
            "Project \"{}\" has an overall health score of {:.0}% — below the 60% threshold.",
            ph.project_name,
            ph.overall_score * 100.0
        )
    } else {
        format!(
            "Project \"{}\" scored {:.0}% overall. Weak areas: {}.",
            ph.project_name,
            ph.overall_score * 100.0,
            weak_dimensions.join(", ")
        )
    };

    let suggested_actions = vec![
        SuggestedAction {
            action_type: "investigate".to_string(),
            label: format!("Review {} health", ph.project_name),
            description: format!(
                "Check dependency freshness and security for {}",
                ph.project_name
            ),
        },
        SuggestedAction {
            action_type: "review_decision".to_string(),
            label: "Review dependency decisions".to_string(),
            description:
                "Consider whether stale or vulnerable dependencies should be updated or replaced"
                    .to_string(),
        },
    ];

    PreemptionAlert {
        id: uuid::Uuid::new_v4().to_string(),
        alert_type: PreemptionType::MaintainerDecline,
        title: format!(
            "{} — health {:.0}%",
            ph.project_name,
            ph.overall_score * 100.0
        ),
        explanation,
        evidence: vec![],
        affected_projects: vec![ph.project_path.clone()],
        affected_dependencies: vec![],
        urgency,
        confidence: 1.0 - ph.overall_score,
        predicted_window: None,
        suggested_actions,
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Convert an individual health alert (e.g. a critical CVE) into a preemption alert.
fn health_alert_to_preemption(
    ph: &crate::project_health::ProjectHealth,
    alert: &crate::project_health::HealthAlert,
) -> PreemptionAlert {
    let urgency = if alert.severity == "critical" {
        AlertUrgency::Critical
    } else {
        AlertUrgency::High
    };

    let alert_type = if alert.message.contains("CVE")
        || alert.message.contains("vulnerabilit")
        || alert.message.contains("security")
    {
        PreemptionType::SecurityAdvisory
    } else if alert.message.contains("deprecat") || alert.message.contains("breaking") {
        PreemptionType::BreakingChange
    } else {
        PreemptionType::EcosystemShift
    };

    let mut affected_deps = Vec::new();
    if let Some(dep) = &alert.dependency {
        affected_deps.push(dep.clone());
    }

    let evidence = vec![AlertEvidence {
        source: "project_health".to_string(),
        title: alert.message.clone(),
        url: None,
        freshness_days: 0.0,
        relevance_score: 1.0,
    }];

    let suggested_actions = vec![SuggestedAction {
        action_type: "investigate".to_string(),
        label: format!("Address {} alert", alert.severity),
        description: alert.message.clone(),
    }];

    PreemptionAlert {
        id: uuid::Uuid::new_v4().to_string(),
        alert_type,
        title: format!("{}: {}", ph.project_name, truncate(&alert.message, 80)),
        explanation: format!(
            "{} severity alert in project \"{}\": {}",
            capitalize(&alert.severity),
            ph.project_name,
            alert.message
        ),
        evidence,
        affected_projects: vec![ph.project_path.clone()],
        affected_dependencies: affected_deps,
        urgency,
        confidence: 0.9,
        predicted_window: None,
        suggested_actions,
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Convert a knowledge gap into a blind-spot preemption alert.
fn gap_to_alert(gap: &crate::knowledge_decay::KnowledgeGap) -> PreemptionAlert {
    let urgency = match gap.gap_severity {
        GapSeverity::Critical => AlertUrgency::Critical,
        GapSeverity::High => AlertUrgency::High,
        GapSeverity::Medium => AlertUrgency::Medium,
        GapSeverity::Low => AlertUrgency::Watch,
    };

    let evidence: Vec<AlertEvidence> = gap
        .missed_items
        .iter()
        .map(|item| {
            let freshness = freshness_from_timestamp(&item.created_at);
            AlertEvidence {
                source: item.source_type.clone(),
                title: item.title.clone(),
                url: item.url.clone(),
                freshness_days: freshness,
                relevance_score: 0.8,
            }
        })
        .collect();

    let suggested_actions = vec![
        SuggestedAction {
            action_type: "investigate".to_string(),
            label: format!("Review {} updates", gap.dependency),
            description: format!(
                "You have {} unread signals about {} — last engagement was {} days ago",
                gap.missed_items.len(),
                gap.dependency,
                gap.days_since_last_engagement
            ),
        },
        SuggestedAction {
            action_type: "dismiss".to_string(),
            label: "Not relevant".to_string(),
            description: format!(
                "Dismiss if {} is no longer part of your active stack",
                gap.dependency
            ),
        },
    ];

    PreemptionAlert {
        id: uuid::Uuid::new_v4().to_string(),
        alert_type: PreemptionType::KnowledgeBlindSpot,
        title: format!(
            "Blind spot: {} ({} missed signals)",
            gap.dependency,
            gap.missed_items.len()
        ),
        explanation: format!(
            "You haven't engaged with {} content in {} days, but {} relevant signals appeared. \
             This dependency is used in your project at \"{}\".",
            gap.dependency,
            gap.days_since_last_engagement,
            gap.missed_items.len(),
            gap.project_path
        ),
        evidence,
        affected_projects: vec![gap.project_path.clone()],
        affected_dependencies: vec![gap.dependency.clone()],
        urgency,
        confidence: severity_to_confidence(&gap.gap_severity),
        predicted_window: None,
        suggested_actions,
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Map urgency to a sort rank (lower = more urgent).
fn urgency_rank(urgency: &AlertUrgency) -> u8 {
    match urgency {
        AlertUrgency::Critical => 0,
        AlertUrgency::High => 1,
        AlertUrgency::Medium => 2,
        AlertUrgency::Watch => 3,
    }
}

/// Classify a chain name into a preemption type based on keywords.
fn classify_chain_type(chain_name: &str) -> PreemptionType {
    let lower = chain_name.to_lowercase();
    if lower.contains("cve") || lower.contains("security") || lower.contains("vulnerab") {
        PreemptionType::SecurityAdvisory
    } else if lower.contains("breaking") || lower.contains("deprecat") {
        PreemptionType::BreakingChange
    } else if lower.contains("migrat") || lower.contains("upgrade") {
        PreemptionType::MigrationWindow
    } else if lower.contains("maintain") || lower.contains("abandon") {
        PreemptionType::MaintainerDecline
    } else {
        PreemptionType::EcosystemShift
    }
}

/// Format hours into a human-readable time window string.
fn format_time_window(hours: f64) -> String {
    if hours < 1.0 {
        "within the hour".to_string()
    } else if hours < 24.0 {
        format!("within ~{:.0} hours", hours)
    } else {
        let days = hours / 24.0;
        format!("within ~{:.0} days", days)
    }
}

/// Compute approximate freshness in days from an RFC3339/ISO timestamp.
fn freshness_from_timestamp(timestamp: &str) -> f32 {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .or_else(|_| {
            // Try parsing as "YYYY-MM-DD HH:MM:SS" (SQLite default)
            chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S").map(|naive| {
                naive
                    .and_local_timezone(chrono::Utc)
                    .single()
                    .unwrap_or_else(chrono::Utc::now)
                    .fixed_offset()
            })
        })
        .map(|dt| {
            let duration = chrono::Utc::now().signed_duration_since(dt);
            (duration.num_hours() as f32 / 24.0).max(0.0)
        })
        .unwrap_or(0.0)
}

/// Human-readable label for a chain phase.
fn phase_label(phase: &crate::signal_chains::ChainPhase) -> &'static str {
    use crate::signal_chains::ChainPhase;
    match phase {
        ChainPhase::Nascent => "nascent",
        ChainPhase::Active => "active",
        ChainPhase::Escalating => "escalating",
        ChainPhase::Peak => "peak",
        ChainPhase::Resolving => "resolving",
    }
}

/// Format confidence as a percentage string.
fn format_confidence(confidence: f64) -> String {
    format!("{:.0}%", confidence * 100.0)
}

/// Convert gap severity to a numeric confidence value.
fn severity_to_confidence(severity: &GapSeverity) -> f32 {
    match severity {
        GapSeverity::Critical => 0.95,
        GapSeverity::High => 0.80,
        GapSeverity::Medium => 0.60,
        GapSeverity::Low => 0.40,
    }
}

/// Truncate a string to a maximum length, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .nth(max_len.saturating_sub(3))
            .map(|(i, _)| i)
            .unwrap_or(max_len.saturating_sub(3));
        format!("{}...", &s[..end])
    }
}

/// Capitalize the first letter of a string.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            upper + chars.as_str()
        }
    }
}

// ============================================================================
// Tauri Command
// ============================================================================

#[tauri::command]
pub async fn get_preemption_alerts() -> std::result::Result<PreemptionFeed, String> {
    get_preemption_feed().map_err(|e| e.to_string())
}
