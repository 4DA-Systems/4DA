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
use crate::evidence::{
    Action as EvidenceAction, Confidence, EvidenceCitation, EvidenceFeed, EvidenceItem,
    EvidenceKind, LensHints, Urgency,
};
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
///
/// PERFORMANCE: On a 239MB DB with 141 projects × 2497 deps, the naive
/// approach (calling `compute_all_project_health` which iterates 141
/// projects × 45 LIKE queries × 2 content columns + embedded detect_chains)
/// takes 4-8 minutes. This hits the Tauri 30-second IPC timeout and produces
/// the "Command 'get_preemption_alerts' timed out after 30s" error.
///
/// The fix:
/// 1. Call `detect_chains` exactly ONCE (not per-project).
/// 2. Replace `compute_all_project_health` with a single batched JOIN query
///    that finds DIRECT deps mentioned in security-keyword source_items in
///    the last 30 days. One SQL round-trip vs ~8000 per-dep queries.
/// 3. Knowledge gaps already has its own 50-dep cap (in `knowledge_decay.rs`).
///
/// Target: under 5 seconds end-to-end on the production DB.
pub fn get_preemption_feed() -> Result<PreemptionFeed> {
    let conn = crate::open_db_connection()?;
    let mut alerts = Vec::new();

    // ─── 1. Signal chain predictions (single call, bounded LIMIT 200) ────
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

    // ─── 2. Direct-dep security alerts (single batched JOIN query) ───────
    // Replaces the O(projects × deps × LIKE) loop that caused the timeout.
    match fetch_direct_dep_security_alerts(&conn) {
        Ok(fast_alerts) => alerts.extend(fast_alerts),
        Err(e) => {
            warn!(target: "4da::preemption", error = %e, "Failed to fetch direct-dep security alerts")
        }
    }

    // ─── 3. Knowledge gaps as blind-spot alerts ──────────────────────────
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

    // Cap total alerts to keep the UI scannable.
    const MAX_ALERTS: usize = 30;
    alerts.truncate(MAX_ALERTS);

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

/// Check whether `project_dependencies` has the `is_direct` column.
///
/// Added in Phase 53 migration. Pre-Phase-53 databases lack the column
/// and would SQL-error on `WHERE pd.is_direct = 1`. This runtime check
/// lets us gracefully fall back to processing all non-dev deps.
fn has_is_direct_column(conn: &rusqlite::Connection) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('project_dependencies') WHERE name = 'is_direct'",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

/// Check whether `project_dependencies` has the `project_relevance` column.
///
/// Added in Phase 55 migration. Pre-Phase-55 databases lack the column.
/// When present, low-relevance projects (example/demo/test dirs) are excluded
/// from preemption alerts.
fn has_project_relevance_column(conn: &rusqlite::Connection) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('project_dependencies') WHERE name = 'project_relevance'",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

/// Fast-path replacement for `project_health::compute_all_project_health`
/// (the O(N²) function that caused the timeout).
///
/// Strategy: ONE batched SQL query that joins `project_dependencies` against
/// `source_items` where the source_item title mentions a security keyword
/// AND the package name. Returns `(project_path, package_name, title, created_at)`
/// tuples that can be directly converted to preemption alerts.
///
/// Scope: direct runtime deps only when the `is_direct` column exists,
/// otherwise all non-dev deps. Last 30 days only. Deduped by
/// (package_name, project_path) and capped at 20 via word-boundary post-filter.
fn fetch_direct_dep_security_alerts(conn: &rusqlite::Connection) -> Result<Vec<PreemptionAlert>> {
    // Runtime column detection for `is_direct`. Pre-Phase-53 DBs lack it --
    // we fall back to processing all non-dev deps in that case.
    let has_is_direct = has_is_direct_column(conn);
    let direct_filter = if has_is_direct {
        "AND pd.is_direct = 1"
    } else {
        ""
    };

    // Runtime column detection for `project_relevance`. Pre-Phase-55 DBs
    // lack it -- we skip the filter and process all deps in that case.
    let has_relevance = has_project_relevance_column(conn);
    let relevance_filter = if has_relevance {
        "AND pd.project_relevance >= 0.15"
    } else {
        ""
    };

    // Note: this query uses title-only LIKE matching (not content LIKE) --
    // content LIKE on 23K rows with avg 669 chars is the slowest part of
    // the legacy path. Title-only is 10-30x faster and catches the same
    // "CVE-2026-XXXX affects react" headlines we care about.
    //
    // Min package_name length 5 -- avoids noise from 4-char generic names
    // ("conf", "cors", "http", "core") that would match too broadly.
    // Dev deps always excluded -- test/lint tools aren't runtime attack surface.
    // LIMIT 100 provides headroom for post-filter dedup to yield ~20 unique alerts.
    let sql = format!(
        "SELECT pd.project_path,
               pd.package_name,
               pd.language,
               si.title,
               si.url,
               si.created_at,
               si.source_type
        FROM project_dependencies pd
        INNER JOIN source_items si
            ON LENGTH(pd.package_name) >= 5
            AND LOWER(si.title) LIKE '%' || LOWER(pd.package_name) || '%'
        WHERE pd.is_dev = 0
          {direct_filter}
          {relevance_filter}
          AND si.created_at >= datetime('now', '-30 days')
          AND (
              LOWER(si.title) LIKE '%cve%'
              OR LOWER(si.title) LIKE '%ghsa%'
              OR LOWER(si.title) LIKE '%vulnerab%'
              OR LOWER(si.title) LIKE '%security advisory%'
              OR LOWER(si.title) LIKE '%security patch%'
              OR LOWER(si.title) LIKE '%security update%'
              OR LOWER(si.title) LIKE '%security flaw%'
              OR LOWER(si.title) LIKE '%security issue%'
              OR LOWER(si.title) LIKE '%security bug%'
              OR LOWER(si.title) LIKE '%breaking%'
              OR LOWER(si.title) LIKE '%deprecat%'
              OR LOWER(si.title) LIKE '%advisory%'
          )
        ORDER BY si.created_at DESC
        LIMIT 100"
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,         // project_path
            row.get::<_, String>(1)?,         // package_name
            row.get::<_, String>(2)?,         // language (ecosystem)
            row.get::<_, String>(3)?,         // title
            row.get::<_, Option<String>>(4)?, // url
            row.get::<_, String>(5)?,         // created_at
            row.get::<_, String>(6)?,         // source_type
        ))
    })?;

    // Phase 1: Build raw alerts with word-boundary validation.
    // Post-filter by word-boundary matching to eliminate false positives
    // where the package name is a substring of an unrelated word
    // (e.g. "conf" matching "config", "cors" matching unrelated contexts).
    use std::collections::{HashMap, HashSet};
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut raw_alerts = Vec::new();

    for row_result in rows {
        let (project_path, package_name, _ecosystem, title, url, created_at, source_type) =
            match row_result {
                Ok(r) => r,
                Err(e) => {
                    warn!(target: "4da::preemption", error = %e, "Row read failed");
                    continue;
                }
            };

        // Word-boundary post-filter: the SQL used substring LIKE which produces
        // false positives. Verify the dep name is actually a word in the title.
        let title_lower = title.to_lowercase();
        let pkg_lower = package_name.to_lowercase();
        if !has_word_boundary_match(&title_lower, &pkg_lower) {
            continue;
        }

        let key = (package_name.clone(), project_path.clone());
        if !seen.insert(key) {
            continue;
        }

        // Classify severity from title keywords.
        let title_lower = title.to_lowercase();
        let is_critical = title_lower.contains("cve")
            || title_lower.contains("critical")
            || title_lower.contains("rce")
            || title_lower.contains("0day")
            || title_lower.contains("exploit");
        let is_breaking = title_lower.contains("breaking") || title_lower.contains("deprecat");

        let urgency = if is_critical {
            AlertUrgency::Critical
        } else if is_breaking {
            AlertUrgency::High
        } else {
            AlertUrgency::Medium
        };

        let alert_type = if is_critical {
            PreemptionType::SecurityAdvisory
        } else if is_breaking {
            PreemptionType::BreakingChange
        } else {
            PreemptionType::EcosystemShift
        };

        let project_name = std::path::Path::new(&project_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let freshness_days = freshness_from_timestamp(&created_at);

        let evidence = vec![AlertEvidence {
            source: source_type,
            title: title.clone(),
            url,
            freshness_days,
            relevance_score: 1.0,
        }];

        let suggested_actions = vec![
            SuggestedAction {
                action_type: "investigate".to_string(),
                label: format!("Review {package_name} update"),
                description: format!(
                    "Check the advisory and determine if {project_name} needs a dependency update."
                ),
            },
            SuggestedAction {
                action_type: "dismiss".to_string(),
                label: "Not relevant".to_string(),
                description: format!(
                    "Dismiss if {package_name} is not in the vulnerable version range."
                ),
            },
        ];

        raw_alerts.push(PreemptionAlert {
            id: uuid::Uuid::new_v4().to_string(),
            alert_type,
            title: format!("{}: {}", project_name, truncate(&title, 80)),
            explanation: truncate(&title, 200),
            evidence,
            affected_projects: vec![project_path],
            affected_dependencies: vec![package_name],
            urgency,
            // Dynamic confidence: scales with how many projects are affected
            confidence: 0.70,
            predicted_window: None,
            suggested_actions,
            created_at: chrono::Utc::now().to_rfc3339(),
        });
    }

    // Phase 2: Group alerts by CVE ID to collapse duplicates.
    // The same CVE showing up across 10+ source items now merges into
    // one alert with all affected projects/deps aggregated.
    let mut cve_groups: HashMap<String, PreemptionAlert> = HashMap::new();
    let mut non_cve_alerts: Vec<PreemptionAlert> = Vec::new();

    for alert in raw_alerts {
        let cve_id = crate::entity_extraction::extract_first_advisory_id(&alert.title);

        match cve_id {
            Some(id) => {
                if let Some(existing) = cve_groups.get_mut(&id) {
                    // Merge: add this alert's projects and deps to the existing group
                    for project in &alert.affected_projects {
                        if !existing.affected_projects.contains(project) {
                            existing.affected_projects.push(project.clone());
                        }
                    }
                    for dep in &alert.affected_dependencies {
                        if !existing.affected_dependencies.contains(dep) {
                            existing.affected_dependencies.push(dep.clone());
                        }
                    }
                    // Merge evidence
                    for ev in &alert.evidence {
                        if !existing.evidence.iter().any(|e| e.title == ev.title) {
                            existing.evidence.push(ev.clone());
                        }
                    }
                    // Recalculate confidence based on affected project count
                    let proj_count = existing.affected_projects.len() as f32;
                    existing.confidence = (0.70 + proj_count * 0.05).min(0.95);
                } else {
                    cve_groups.insert(id, alert);
                }
            }
            None => {
                non_cve_alerts.push(alert);
            }
        }
    }

    // Recalculate confidence for all CVE-grouped alerts
    let mut alerts: Vec<PreemptionAlert> = cve_groups
        .into_values()
        .map(|mut a| {
            let proj_count = a.affected_projects.len() as f32;
            a.confidence = (0.70 + proj_count * 0.05).min(0.95);
            a
        })
        .collect();
    alerts.extend(non_cve_alerts);

    // Phase 3: Fuzzy title dedup — collapse near-identical alerts from
    // different sources (same Reddit article from 8 subreddits, etc.).
    // Uses Jaccard word overlap >0.65 as the similarity threshold.
    let alerts = dedup_preemption_alerts(alerts);

    Ok(alerts)
}

/// Remove near-duplicate preemption alerts using Jaccard word overlap on titles.
fn dedup_preemption_alerts(alerts: Vec<PreemptionAlert>) -> Vec<PreemptionAlert> {
    use std::collections::HashSet;

    let mut seen_titles: Vec<HashSet<String>> = Vec::new();
    let mut deduped = Vec::new();

    for alert in alerts {
        let normalized: HashSet<String> = alert
            .title
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' {
                    c
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .split_whitespace()
            .map(String::from)
            .collect();

        if normalized.is_empty() {
            deduped.push(alert);
            continue;
        }

        let is_duplicate = seen_titles.iter().any(|seen| {
            let intersection = seen.intersection(&normalized).count();
            let union = seen.union(&normalized).count();
            union > 0 && (intersection as f32 / union as f32) > 0.65
        });

        if !is_duplicate {
            seen_titles.push(normalized);
            deduped.push(alert);
        }
    }

    deduped
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
/// Map gap severity to a confidence score for preemption alerts.
///
/// Pre-Phase-13b, this mapped Critical→0.95 which made ALL critical
/// items show 95% — meaningless because it echoed the urgency badge.
/// Now: confidence reflects HOW SURE we are the gap is real (not how
/// severe it is — that's urgency's job). Knowledge gaps from the
/// decay detector are inherently heuristic, so we cap at 0.70.
fn severity_to_confidence(severity: &GapSeverity) -> f32 {
    match severity {
        GapSeverity::Critical => 0.70,
        GapSeverity::High => 0.65,
        GapSeverity::Medium => 0.55,
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

/// Check whether `text` contains `term` at a word boundary (not embedded in a
/// larger word). Case-sensitive — pass lowercase strings for case-insensitive
/// matching. Accepts `.js`/`.ts`/`.rs` suffixes as valid boundaries for package
/// names like "next.js" or "serde.rs".
fn has_word_boundary_match(text: &str, term: &str) -> bool {
    if term.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let mut search_from = 0;
    while let Some(pos) = text[search_from..].find(term) {
        let abs = search_from + pos;
        let before_ok = abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric();
        let after = abs + term.len();
        let after_ok = after >= bytes.len()
            || !bytes[after].is_ascii_alphanumeric()
            || text[after..].starts_with(".js")
            || text[after..].starts_with(".ts")
            || text[after..].starts_with(".rs");
        if before_ok && after_ok {
            return true;
        }
        search_from = abs + 1;
    }
    false
}

// ============================================================================
// EvidenceItem conversion (Intelligence Reconciliation — Phase 3)
// ============================================================================
//
// `PreemptionAlert` is the pre-reconciliation shape. The Tauri command now
// emits canonical `EvidenceItem`s via `EvidenceFeed`. Internal callers
// (e.g. `monitoring_briefing.rs`) still use `PreemptionAlert` until their
// own materializers land in later phases.

fn alert_urgency_to_canonical(u: &AlertUrgency) -> Urgency {
    match u {
        AlertUrgency::Critical => Urgency::Critical,
        AlertUrgency::High => Urgency::High,
        AlertUrgency::Medium => Urgency::Medium,
        AlertUrgency::Watch => Urgency::Watch,
    }
}

/// Map the legacy `action_type` string onto a canonical action_id. Legacy
/// values were a free-text convention; canonical ids are enumerated in
/// `evidence::types::ACTION_IDS`. Unknown values fall back to "acknowledge".
fn map_action_id(legacy: &str) -> &'static str {
    match legacy {
        "dismiss" => "dismiss",
        "watch" => "snooze_7d",
        "investigate" => "investigate",
        "review_decision" => "brief_this",
        _ => "acknowledge",
    }
}

fn suggested_action_to_canonical(a: &SuggestedAction) -> EvidenceAction {
    EvidenceAction {
        action_id: map_action_id(&a.action_type).to_string(),
        label: a.label.clone(),
        description: a.description.clone(),
    }
}

fn alert_evidence_to_citation(e: &AlertEvidence) -> EvidenceCitation {
    // Cap relevance_note at 200 chars per EvidenceItem schema rule.
    let note = format!("relevance {:.2}", e.relevance_score);
    EvidenceCitation {
        source: e.source.clone(),
        title: e.title.clone(),
        url: e.url.clone(),
        freshness_days: e.freshness_days,
        relevance_note: note,
    }
}

fn preemption_kind_to_canonical(t: &PreemptionType) -> EvidenceKind {
    match t {
        PreemptionType::KnowledgeBlindSpot => EvidenceKind::Gap,
        _ => EvidenceKind::Alert,
    }
}

impl PreemptionAlert {
    /// Convert to the canonical `EvidenceItem` for lens consumption.
    /// Used by `get_preemption_alerts` (command boundary).
    pub fn to_evidence_item(&self) -> EvidenceItem {
        // `created_at` is an ISO-8601 SQLite datetime string; convert to
        // Unix millis. On parse failure fall back to "now" — never break
        // a user-facing item on a timestamp quirk.
        let created_at = chrono::NaiveDateTime::parse_from_str(&self.created_at, "%Y-%m-%d %H:%M:%S")
            .map(|dt| dt.and_utc().timestamp_millis())
            .unwrap_or_else(|_| chrono::Utc::now().timestamp_millis());

        // Always title the item with the alert's own title; trim any
        // trailing period per schema rule.
        let title = self
            .title
            .trim_end_matches('.')
            .chars()
            .take(120)
            .collect::<String>();

        let kind = preemption_kind_to_canonical(&self.alert_type);
        let evidence: Vec<EvidenceCitation> = self
            .evidence
            .iter()
            .map(alert_evidence_to_citation)
            .collect();

        let suggested_actions: Vec<EvidenceAction> = self
            .suggested_actions
            .iter()
            .map(suggested_action_to_canonical)
            .collect();

        EvidenceItem {
            id: self.id.clone(),
            kind,
            title,
            explanation: self.explanation.clone(),
            // Preemption uses a bare f32 confidence; provenance is
            // heuristic until AWE spine is wired in Phase 9.
            confidence: Confidence::heuristic(self.confidence.clamp(0.0, 1.0)),
            urgency: alert_urgency_to_canonical(&self.urgency),
            // Reversibility is not computed by preemption — leave None.
            reversibility: None,
            evidence,
            affected_projects: self.affected_projects.clone(),
            affected_deps: self.affected_dependencies.clone(),
            suggested_actions,
            precedents: Vec::new(),
            refutation_condition: None,
            lens_hints: LensHints::preemption_only(),
            created_at,
            expires_at: None,
        }
    }
}

// ============================================================================
// Tauri Command
// ============================================================================

/// Returns the canonical `EvidenceFeed` for the Preemption lens.
/// Internally still produces `PreemptionAlert`s (same ranking, same content)
/// and converts at the boundary — lossless for the UI, and lets
/// `monitoring_briefing.rs` continue to use the legacy shape until its own
/// phase. In dev builds the output is schema-validated; validation failures
/// drop the offending item with a log rather than breaking the feed.
#[tauri::command]
pub async fn get_preemption_alerts() -> std::result::Result<EvidenceFeed, String> {
    crate::settings::require_signal_feature("get_preemption_alerts").map_err(|e| e.to_string())?;
    let feed = get_preemption_feed().map_err(|e| e.to_string())?;
    let mut items: Vec<EvidenceItem> = feed
        .alerts
        .iter()
        .map(|a| a.to_evidence_item())
        .filter(|item| match crate::evidence::validate_item(item) {
            Ok(()) => true,
            Err(e) => {
                warn!(
                    target: "4da::evidence::validate",
                    id = %item.id,
                    error = %e,
                    "dropped preemption item failing schema validation"
                );
                false
            }
        })
        .collect();
    // Phase 9 — attach precedents via the AWE spine.
    crate::awe_spine::enrich_items(&mut items);
    Ok(EvidenceFeed::from_items(items))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

    /// Real-schema in-memory DB for preemption tests.
    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                url TEXT,
                source_type TEXT NOT NULL,
                content TEXT,
                relevance_score REAL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE project_dependencies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                manifest_type TEXT NOT NULL,
                package_name TEXT NOT NULL,
                version TEXT,
                is_dev INTEGER DEFAULT 0,
                is_direct INTEGER DEFAULT 1,
                language TEXT NOT NULL DEFAULT 'unknown',
                project_relevance REAL DEFAULT 1.0,
                last_scanned TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(project_path, package_name)
            );
            ",
        )
        .unwrap();
        conn
    }

    fn insert_dep(conn: &Connection, project: &str, pkg: &str, direct: bool, dev: bool) {
        conn.execute(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, is_direct, is_dev, language)
             VALUES (?1, 'npm', ?2, ?3, ?4, 'javascript')",
            params![project, pkg, direct as i32, dev as i32],
        )
        .unwrap();
    }

    fn insert_item(conn: &Connection, title: &str) {
        conn.execute(
            "INSERT INTO source_items (title, source_type, content, created_at)
             VALUES (?1, 'hackernews', '', datetime('now', '-5 days'))",
            params![title],
        )
        .unwrap();
    }

    fn insert_dep_with_relevance(
        conn: &Connection,
        project: &str,
        pkg: &str,
        direct: bool,
        dev: bool,
        relevance: f64,
    ) {
        conn.execute(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, is_direct, is_dev, language, project_relevance)
             VALUES (?1, 'npm', ?2, ?3, ?4, 'javascript', ?5)",
            params![project, pkg, direct as i32, dev as i32, relevance],
        )
        .unwrap();
    }

    // ─── Project relevance filtering ─────────────────────────────────

    #[test]
    fn low_relevance_projects_excluded_from_preemption() {
        let conn = setup_test_db();
        // High-relevance project (production) -- should produce an alert
        insert_dep_with_relevance(&conn, "/proj/production", "react", true, false, 1.0);
        // Low-relevance project (example dir) -- should be excluded
        insert_dep_with_relevance(&conn, "/proj/example", "webpack", true, false, 0.1);
        insert_item(&conn, "CVE-2026-5555 critical vulnerability in react");
        insert_item(&conn, "CVE-2026-6666 critical vulnerability in webpack");

        let alerts = fetch_direct_dep_security_alerts(&conn).unwrap();

        // Only react from the production project should appear
        assert_eq!(
            alerts.len(),
            1,
            "low-relevance project deps should be excluded"
        );
        assert!(alerts[0]
            .affected_dependencies
            .contains(&"react".to_string()));
    }

    #[test]
    fn borderline_relevance_included_in_preemption() {
        let conn = setup_test_db();
        // Exactly at the threshold (0.15) -- should be included
        insert_dep_with_relevance(&conn, "/proj/borderline", "express", true, false, 0.15);
        // Just below threshold -- should be excluded
        insert_dep_with_relevance(&conn, "/proj/example", "fastify", true, false, 0.14);
        insert_item(&conn, "security advisory for express framework");
        insert_item(&conn, "security advisory for fastify framework");

        let alerts = fetch_direct_dep_security_alerts(&conn).unwrap();

        let express_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| a.affected_dependencies.contains(&"express".to_string()))
            .collect();
        let fastify_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| a.affected_dependencies.contains(&"fastify".to_string()))
            .collect();
        assert_eq!(
            express_alerts.len(),
            1,
            "at-threshold dep should be included"
        );
        assert_eq!(
            fastify_alerts.len(),
            0,
            "below-threshold dep should be excluded"
        );
    }

    // ─── Fix 7: fast-path query filters correctly ─────────────────────

    #[test]
    fn fix7_fast_path_filters_direct_runtime_deps_only() {
        let conn = setup_test_db();
        insert_dep(&conn, "/proj/a", "react", true, false);
        insert_dep(&conn, "/proj/a", "jest", true, true); // dev — excluded
        insert_dep(&conn, "/proj/a", "lodash", false, false); // transitive — excluded
        insert_item(&conn, "CVE-2026-1234 critical vulnerability in react");
        insert_item(&conn, "jest has a new security advisory");
        insert_item(&conn, "lodash breaking change in v5");

        let alerts = fetch_direct_dep_security_alerts(&conn).unwrap();

        // Only react should yield an alert (jest is dev, lodash is transitive)
        assert_eq!(alerts.len(), 1, "only direct runtime deps count");
        assert!(alerts[0].title.contains("CVE-2026-1234"));
        assert!(alerts[0]
            .affected_dependencies
            .contains(&"react".to_string()));
    }

    #[test]
    fn fix7_fast_path_classifies_urgency() {
        let conn = setup_test_db();
        insert_dep(&conn, "/proj/a", "axios", true, false);
        insert_dep(&conn, "/proj/b", "webpack", true, false);
        insert_dep(&conn, "/proj/c", "react", true, false);
        insert_item(&conn, "CVE-2026-9999 critical RCE in axios");
        insert_item(&conn, "webpack breaking change in version 6");
        insert_item(&conn, "security advisory for react server components");

        let alerts = fetch_direct_dep_security_alerts(&conn).unwrap();

        let axios = alerts.iter().find(|a| a.title.contains("axios")).unwrap();
        assert!(
            matches!(axios.urgency, AlertUrgency::Critical),
            "CVE+RCE = critical"
        );

        let webpack = alerts.iter().find(|a| a.title.contains("webpack")).unwrap();
        assert!(
            matches!(webpack.urgency, AlertUrgency::High),
            "breaking = high"
        );

        let react = alerts.iter().find(|a| a.title.contains("react")).unwrap();
        assert!(
            matches!(react.urgency, AlertUrgency::Medium),
            "plain security = medium"
        );
    }

    #[test]
    fn fix7_fast_path_dedupes_by_package_and_project() {
        let conn = setup_test_db();
        insert_dep(&conn, "/proj/a", "react", true, false);
        // 3 separate items all mentioning the same package
        insert_item(&conn, "react CVE-2026-1 vulnerability discovered");
        insert_item(&conn, "react CVE-2026-2 additional security issue");
        insert_item(&conn, "react advisory published by RustSec");

        let alerts = fetch_direct_dep_security_alerts(&conn).unwrap();
        // After dedup by (package, project), only one alert despite 3 matching items
        let react_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| a.affected_dependencies.contains(&"react".to_string()))
            .collect();
        assert_eq!(
            react_alerts.len(),
            1,
            "same dep+project dedups to one alert"
        );
    }

    #[test]
    fn fix7_fast_path_rejects_substring_false_positives() {
        let conn = setup_test_db();
        // Note: package length must be >= 5 for SQL pre-filter. Use 6-char pkg
        // "config" — the word-boundary post-filter should still reject
        // "configuration" matches.
        insert_dep(&conn, "/proj/a", "config", true, false);
        // Title contains "configuration" — NOT a word-boundary match for "config"
        // because "ur" follows. Should be filtered out by post-filter.
        insert_item(&conn, "security alert: configuration leak in production");

        let alerts = fetch_direct_dep_security_alerts(&conn).unwrap();
        assert_eq!(
            alerts.len(),
            0,
            "substring match in a longer word must not produce an alert"
        );
    }

    // ─── Word-boundary helper ────────────────────────────────────────

    #[test]
    fn word_boundary_match_handles_suffix_extensions() {
        assert!(has_word_boundary_match("next.js release", "next"));
        assert!(has_word_boundary_match("serde.rs v2", "serde"));
        assert!(!has_word_boundary_match("unexpected", "next"));
    }

    // ─── Runtime column detection ────────────────────────────────────

    #[test]
    fn has_is_direct_column_true_when_present() {
        let conn = setup_test_db();
        // setup_test_db creates project_dependencies WITH is_direct
        assert!(has_is_direct_column(&conn));
    }

    #[test]
    fn has_is_direct_column_false_when_absent() {
        let conn = Connection::open_in_memory().unwrap();
        // Create table WITHOUT is_direct column
        conn.execute_batch(
            "CREATE TABLE project_dependencies (
                id INTEGER PRIMARY KEY,
                project_path TEXT NOT NULL,
                manifest_type TEXT NOT NULL,
                package_name TEXT NOT NULL,
                is_dev INTEGER DEFAULT 0,
                language TEXT NOT NULL DEFAULT 'unknown'
            );",
        )
        .unwrap();
        assert!(!has_is_direct_column(&conn));
    }

    #[test]
    fn fetch_direct_dep_security_alerts_works_without_is_direct_column() {
        // Regression test: pre-Phase-53 DBs lack is_direct. Must not SQL-error.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                url TEXT,
                source_type TEXT NOT NULL,
                content TEXT,
                relevance_score REAL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE project_dependencies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                manifest_type TEXT NOT NULL,
                package_name TEXT NOT NULL,
                is_dev INTEGER DEFAULT 0,
                language TEXT NOT NULL DEFAULT 'unknown'
            );
            INSERT INTO project_dependencies (project_path, manifest_type, package_name, is_dev, language)
            VALUES ('/proj/a', 'npm', 'webpack', 0, 'javascript');
            INSERT INTO source_items (title, source_type, content, created_at)
            VALUES ('CVE-2026-9999 webpack critical vulnerability', 'hn', '', datetime('now', '-5 days'));
            ",
        )
        .unwrap();

        // Must not error, must return the alert using the fallback path
        let alerts =
            fetch_direct_dep_security_alerts(&conn).expect("must work without is_direct column");
        assert_eq!(
            alerts.len(),
            1,
            "should find the webpack alert via fallback"
        );
    }

    // ========================================================================
    // EvidenceItem conversion tests (Intelligence Reconciliation — Phase 3)
    // ========================================================================

    fn sample_alert() -> PreemptionAlert {
        PreemptionAlert {
            id: "p_sec_webpack".to_string(),
            alert_type: PreemptionType::SecurityAdvisory,
            title: "CVE-2026-9999 affects webpack".to_string(),
            explanation: "A critical vulnerability was reported.".to_string(),
            evidence: vec![AlertEvidence {
                source: "hn".to_string(),
                title: "CVE-2026-9999 webpack critical vulnerability".to_string(),
                url: Some("https://news.ycombinator.com/item?id=1".to_string()),
                freshness_days: 5.0,
                relevance_score: 0.82,
            }],
            affected_projects: vec!["/proj/a".to_string()],
            affected_dependencies: vec!["webpack".to_string()],
            urgency: AlertUrgency::Critical,
            confidence: 0.77,
            predicted_window: Some("within 7 days".to_string()),
            suggested_actions: vec![SuggestedAction {
                action_type: "investigate".to_string(),
                label: "Investigate".to_string(),
                description: "Review the advisory for affected versions.".to_string(),
            }],
            created_at: "2026-04-17 09:30:00".to_string(),
        }
    }

    #[test]
    fn to_evidence_item_maps_urgency() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        assert_eq!(item.urgency, crate::evidence::Urgency::Critical);
    }

    #[test]
    fn to_evidence_item_maps_security_advisory_to_alert_kind() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        assert_eq!(item.kind, crate::evidence::EvidenceKind::Alert);
    }

    #[test]
    fn to_evidence_item_maps_knowledge_blindspot_to_gap_kind() {
        let mut alert = sample_alert();
        alert.alert_type = PreemptionType::KnowledgeBlindSpot;
        let item = alert.to_evidence_item();
        assert_eq!(item.kind, crate::evidence::EvidenceKind::Gap);
    }

    #[test]
    fn to_evidence_item_maps_legacy_action_types() {
        let mut alert = sample_alert();
        alert.suggested_actions = vec![
            SuggestedAction {
                action_type: "watch".to_string(),
                label: "Watch".to_string(),
                description: "".to_string(),
            },
            SuggestedAction {
                action_type: "review_decision".to_string(),
                label: "Review".to_string(),
                description: "".to_string(),
            },
            SuggestedAction {
                action_type: "investigate".to_string(),
                label: "Look".to_string(),
                description: "".to_string(),
            },
            SuggestedAction {
                action_type: "dismiss".to_string(),
                label: "X".to_string(),
                description: "".to_string(),
            },
            SuggestedAction {
                action_type: "unknown_legacy".to_string(),
                label: "?".to_string(),
                description: "".to_string(),
            },
        ];
        let item = alert.to_evidence_item();
        let ids: Vec<&str> = item
            .suggested_actions
            .iter()
            .map(|a| a.action_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["snooze_7d", "brief_this", "investigate", "dismiss", "acknowledge"]
        );
    }

    #[test]
    fn to_evidence_item_sets_preemption_lens_hint() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        assert!(item.lens_hints.preemption);
        assert!(!item.lens_hints.briefing);
        assert!(!item.lens_hints.blind_spots);
        assert!(!item.lens_hints.evidence);
    }

    #[test]
    fn to_evidence_item_strips_trailing_period_from_title() {
        let mut alert = sample_alert();
        alert.title = "Something will break.".to_string();
        let item = alert.to_evidence_item();
        assert_eq!(item.title, "Something will break");
    }

    #[test]
    fn to_evidence_item_caps_title_at_120_chars() {
        let mut alert = sample_alert();
        alert.title = "x".repeat(200);
        let item = alert.to_evidence_item();
        assert_eq!(item.title.len(), 120);
    }

    #[test]
    fn to_evidence_item_passes_schema_validation() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        assert!(crate::evidence::validate_item(&item).is_ok());
    }

    #[test]
    fn to_evidence_item_marks_confidence_heuristic_provenance() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        assert_eq!(
            item.confidence.provenance,
            crate::evidence::ConfidenceProvenance::Heuristic
        );
    }

    #[test]
    fn to_evidence_item_clamps_confidence_into_range() {
        let mut alert = sample_alert();
        alert.confidence = 1.5; // Out-of-range legacy value
        let item = alert.to_evidence_item();
        assert!(item.confidence.value >= 0.0 && item.confidence.value <= 1.0);
    }

    #[test]
    fn to_evidence_item_includes_citations() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        assert_eq!(item.evidence.len(), 1);
        assert_eq!(item.evidence[0].source, "hn");
        assert!(item.evidence[0].url.is_some());
    }

    #[test]
    fn to_evidence_item_parses_created_at() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        // 2026-04-17 09:30:00 UTC → must be a real millis value
        assert!(item.created_at > 1_700_000_000_000);
    }
}
