// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Blind Spot Intelligence for 4DA
//!
//! Cross-references what the user is watching with what they SHOULD be
//! watching based on their actual dependencies, projects, and stack.
//! "You have 6 active Rust deps but haven't engaged with Rust signals in 21 days."

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::warn;
use ts_rs::TS;

use crate::error::Result;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct BlindSpotReport {
    /// 0-100, higher = more blind spots
    pub overall_score: f32,
    pub uncovered_dependencies: Vec<UncoveredDep>,
    pub stale_topics: Vec<StaleTopic>,
    pub missed_signals: Vec<MissedSignal>,
    pub recommendations: Vec<BlindSpotRecommendation>,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct UncoveredDep {
    pub name: String,
    /// npm, cargo, pip, etc.
    pub dep_type: String,
    pub projects_using: Vec<String>,
    pub days_since_last_signal: u32,
    /// Signals that exist but the user didn't see
    pub available_signal_count: u32,
    /// critical, high, medium, low
    pub risk_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct StaleTopic {
    pub topic: String,
    pub last_engagement_days: u32,
    pub active_deps_in_topic: u32,
    pub missed_signal_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct MissedSignal {
    pub item_id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub relevance_score: f32,
    pub created_at: String,
    pub why_relevant: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct BlindSpotRecommendation {
    /// e.g., "Set up a watch for Rust security"
    pub action: String,
    pub reason: String,
    /// high, medium, low
    pub priority: String,
}

// Internal struct for dependency coverage tracking
#[derive(Debug, Clone)]
struct DepCoverage {
    name: String,
    dep_type: String,
    projects: Vec<String>,
}

// ============================================================================
// Implementation
// ============================================================================

/// Generate a comprehensive blind spot report.
pub fn generate_blind_spot_report() -> Result<BlindSpotReport> {
    let conn = crate::open_db_connection()?;

    // 1. Get attention report (30-day window)
    let attention = crate::attention::generate_report(30)?;

    // 2. Get knowledge gaps
    let gaps = crate::knowledge_decay::detect_knowledge_gaps(&conn)?;

    // 3. Get all user dependencies with project coverage
    let deps = get_dependency_coverage(&conn)?;

    // 4. Find uncovered dependencies (deps with no interaction in 14+ days)
    let uncovered = find_uncovered_deps(&conn, &deps)?;

    // 5. Find stale topics from attention blind spots
    let stale = attention
        .blind_spots
        .iter()
        .filter(|bs| bs.in_codebase)
        .map(|bs| StaleTopic {
            topic: bs.topic.clone(),
            last_engagement_days: ((1.0 - bs.engagement_level) * 30.0) as u32,
            active_deps_in_topic: count_deps_for_topic(&deps, &bs.topic),
            missed_signal_count: count_missed_for_topic(&gaps, &bs.topic),
        })
        .collect::<Vec<_>>();

    // 6. Find missed signals (high-relevance, not interacted with, last 7 days)
    let missed = find_missed_signals(&conn, 7)?;

    // 7. Generate recommendations
    let recommendations = generate_recommendations(&uncovered, &stale, &gaps);

    // 8. Calculate overall score
    let score = calculate_blind_spot_score(&uncovered, &stale, &missed);

    Ok(BlindSpotReport {
        overall_score: score,
        uncovered_dependencies: uncovered,
        stale_topics: stale,
        missed_signals: missed,
        recommendations,
        generated_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Query user_dependencies JOIN project_dependencies to get all deps with their
/// project count and metadata.
fn get_dependency_coverage(conn: &rusqlite::Connection) -> Result<Vec<DepCoverage>> {
    let mut result: Vec<DepCoverage> = Vec::new();

    // Get unique deps from user_dependencies
    let mut stmt = match conn
        .prepare("SELECT DISTINCT name, COALESCE(dep_type, 'unknown') FROM user_dependencies")
    {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to query user_dependencies (table may not exist): {e}");
            return Ok(result);
        }
    };

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rows {
        let (name, dep_type) = row?;
        // Find which projects use this dep
        let projects = get_projects_for_dep(conn, &name);
        result.push(DepCoverage {
            name,
            dep_type,
            projects,
        });
    }

    Ok(result)
}

/// Look up which projects reference a given dependency.
fn get_projects_for_dep(conn: &rusqlite::Connection, dep_name: &str) -> Vec<String> {
    let mut stmt = match conn
        .prepare("SELECT DISTINCT project_path FROM project_dependencies WHERE package_name = ?1")
    {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map(params![dep_name], |row| row.get::<_, String>(0))
        .ok()
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
}

/// For each dependency, check if any source_items mention it in the last 14 days
/// AND whether the user interacted with them. If no interaction, it's uncovered.
fn find_uncovered_deps(
    conn: &rusqlite::Connection,
    deps: &[DepCoverage],
) -> Result<Vec<UncoveredDep>> {
    let mut uncovered = Vec::new();

    for dep in deps {
        let search_term = format!("%{}%", dep.name);

        // Count available signals mentioning this dep in the last 14 days
        let available: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items
                 WHERE title LIKE ?1
                   AND created_at >= datetime('now', '-14 days')",
                params![search_term],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Count how many of those the user actually interacted with
        let interacted: u32 = conn
            .query_row(
                "SELECT COUNT(DISTINCT si.id) FROM source_items si
                 JOIN interactions i ON i.item_id = si.id
                 WHERE si.title LIKE ?1
                   AND si.created_at >= datetime('now', '-14 days')",
                params![search_term],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Days since last interaction with any signal mentioning this dep
        let days_since: u32 = conn
            .query_row(
                "SELECT COALESCE(
                    CAST(julianday('now') - julianday(MAX(i.created_at)) AS INTEGER),
                    999
                 )
                 FROM source_items si
                 JOIN interactions i ON i.item_id = si.id
                 WHERE si.title LIKE ?1",
                params![search_term],
                |row| row.get(0),
            )
            .unwrap_or(999);

        // Skip if user has recently interacted
        if days_since < 14 && interacted > 0 {
            continue;
        }

        let not_seen = available.saturating_sub(interacted);

        let risk_level = classify_dep_risk(days_since, not_seen, dep.projects.len());

        uncovered.push(UncoveredDep {
            name: dep.name.clone(),
            dep_type: dep.dep_type.clone(),
            projects_using: dep.projects.clone(),
            days_since_last_signal: days_since,
            available_signal_count: not_seen,
            risk_level,
        });
    }

    // Sort by risk: critical first, then by days since last signal
    uncovered.sort_by(|a, b| {
        risk_ord(&a.risk_level)
            .cmp(&risk_ord(&b.risk_level))
            .then(b.days_since_last_signal.cmp(&a.days_since_last_signal))
    });

    Ok(uncovered)
}

/// Classify risk level based on coverage gap severity.
fn classify_dep_risk(days_since: u32, unseen_signals: u32, project_count: usize) -> String {
    if days_since > 60 && project_count > 2 {
        "critical".to_string()
    } else if days_since > 30 || (unseen_signals > 5 && project_count > 1) {
        "high".to_string()
    } else if days_since > 14 || unseen_signals > 2 {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

/// Ordering helper for risk levels (lower = more severe).
fn risk_ord(risk: &str) -> u8 {
    match risk {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        _ => 3,
    }
}

/// Query source_items with relevance_score > 0.5 from the last N days that
/// have NO matching interaction. Limited to 20 results.
fn find_missed_signals(conn: &rusqlite::Connection, days: u32) -> Result<Vec<MissedSignal>> {
    let sql = format!(
        "SELECT si.id, si.title, si.url, si.source_type, si.relevance_score, si.created_at
         FROM source_items si
         LEFT JOIN interactions i ON i.item_id = si.id
         WHERE si.relevance_score > 0.5
           AND si.created_at >= datetime('now', '-{days} days')
           AND i.item_id IS NULL
         ORDER BY si.relevance_score DESC
         LIMIT 20"
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to query missed signals: {e}");
            return Ok(Vec::new());
        }
    };

    let rows = stmt.query_map([], |row| {
        Ok(MissedSignal {
            item_id: row.get(0)?,
            title: row.get(1)?,
            url: row.get(2)?,
            source_type: row.get(3)?,
            relevance_score: row.get(4)?,
            created_at: row.get(5)?,
            why_relevant: String::new(), // populated below
        })
    })?;

    let mut signals: Vec<MissedSignal> = rows.flatten().collect();

    // Populate why_relevant based on relevance score tier
    for signal in &mut signals {
        signal.why_relevant = if signal.relevance_score > 0.8 {
            "Highly relevant to your stack — strong match with your dependencies".to_string()
        } else if signal.relevance_score > 0.65 {
            "Relevant to your interests — matches topics you actively work with".to_string()
        } else {
            "Moderately relevant — may contain useful context for your projects".to_string()
        };
    }

    Ok(signals)
}

/// Count deps whose name fuzzy-matches the given topic.
fn count_deps_for_topic(deps: &[DepCoverage], topic: &str) -> u32 {
    let topic_lower = topic.to_lowercase();
    deps.iter()
        .filter(|d| {
            let name_lower = d.name.to_lowercase();
            name_lower.contains(&topic_lower) || topic_lower.contains(&name_lower)
        })
        .count() as u32
}

/// Count missed items from knowledge gaps that match the given topic.
fn count_missed_for_topic(gaps: &[crate::knowledge_decay::KnowledgeGap], topic: &str) -> u32 {
    let topic_lower = topic.to_lowercase();
    gaps.iter()
        .filter(|g| {
            let dep_lower = g.dependency.to_lowercase();
            dep_lower.contains(&topic_lower) || topic_lower.contains(&dep_lower)
        })
        .map(|g| g.missed_items.len() as u32)
        .sum()
}

/// Generate 3-5 actionable recommendations based on blind spot analysis.
fn generate_recommendations(
    uncovered: &[UncoveredDep],
    stale: &[StaleTopic],
    gaps: &[crate::knowledge_decay::KnowledgeGap],
) -> Vec<BlindSpotRecommendation> {
    let mut recs = Vec::new();

    // Recommendation for critical/high uncovered deps
    let critical_deps: Vec<&UncoveredDep> = uncovered
        .iter()
        .filter(|d| d.risk_level == "critical" || d.risk_level == "high")
        .collect();

    if !critical_deps.is_empty() {
        let dep_names: Vec<&str> = critical_deps
            .iter()
            .take(3)
            .map(|d| d.name.as_str())
            .collect();
        recs.push(BlindSpotRecommendation {
            action: format!("Review signals for: {}", dep_names.join(", ")),
            reason: format!(
                "{} dependencies have critical/high risk blind spots with no recent engagement",
                critical_deps.len()
            ),
            priority: "high".to_string(),
        });
    }

    // Recommendation for stale topics with active deps
    let active_stale: Vec<&StaleTopic> = stale
        .iter()
        .filter(|s| s.active_deps_in_topic > 0)
        .collect();

    if !active_stale.is_empty() {
        let topic_names: Vec<&str> = active_stale
            .iter()
            .take(3)
            .map(|s| s.topic.as_str())
            .collect();
        recs.push(BlindSpotRecommendation {
            action: format!("Catch up on stale topics: {}", topic_names.join(", ")),
            reason: format!(
                "{} topics have active dependencies but declining attention",
                active_stale.len()
            ),
            priority: "high".to_string(),
        });
    }

    // Recommendation for knowledge gaps with severe severity
    let severe_gaps: Vec<_> = gaps
        .iter()
        .filter(|g| {
            g.gap_severity == crate::knowledge_decay::GapSeverity::Critical
                || g.gap_severity == crate::knowledge_decay::GapSeverity::High
        })
        .collect();

    if !severe_gaps.is_empty() {
        let gap_names: Vec<&str> = severe_gaps
            .iter()
            .take(3)
            .map(|g| g.dependency.as_str())
            .collect();
        recs.push(BlindSpotRecommendation {
            action: format!(
                "Address knowledge decay in: {}",
                gap_names.join(", ")
            ),
            reason: format!(
                "{} dependencies have critical/high knowledge gaps — you may be missing important updates",
                severe_gaps.len()
            ),
            priority: "medium".to_string(),
        });
    }

    // General recommendation if many uncovered deps
    if uncovered.len() > 5 {
        recs.push(BlindSpotRecommendation {
            action: "Consider adding RSS feeds or watches for your most-used dependencies"
                .to_string(),
            reason: format!(
                "{} of your dependencies have no recent signal coverage",
                uncovered.len()
            ),
            priority: "medium".to_string(),
        });
    }

    // Positive reinforcement if few blind spots
    if uncovered.is_empty() && stale.is_empty() {
        recs.push(BlindSpotRecommendation {
            action: "Your signal coverage looks solid — keep monitoring for shifts".to_string(),
            reason: "No critical blind spots detected in your current stack".to_string(),
            priority: "low".to_string(),
        });
    }

    recs
}

/// Weighted score: more uncovered deps + more stale topics + more missed
/// signals = higher score. Capped at 100.
fn calculate_blind_spot_score(
    uncovered: &[UncoveredDep],
    stale: &[StaleTopic],
    missed: &[MissedSignal],
) -> f32 {
    // Weights: uncovered deps are heaviest (they represent real risk)
    let uncovered_score = uncovered
        .iter()
        .map(|d| match d.risk_level.as_str() {
            "critical" => 15.0,
            "high" => 10.0,
            "medium" => 5.0,
            _ => 2.0,
        })
        .sum::<f32>();

    let stale_score = stale.len() as f32 * 5.0;

    let missed_score = missed.iter().map(|m| m.relevance_score * 3.0).sum::<f32>();

    let raw = uncovered_score + stale_score + missed_score;
    raw.min(100.0)
}

// ============================================================================
// Tauri Command
// ============================================================================

#[tauri::command]
pub async fn get_blind_spots() -> std::result::Result<BlindSpotReport, String> {
    generate_blind_spot_report().map_err(|e| e.to_string())
}
