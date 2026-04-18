// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Decision Impact Tracking — Signal-gated cross-referencing of decisions
//! with incoming signals.
//!
//! For each active decision, finds source items that support or challenge
//! the decision based on keyword matching against the decision subject,
//! rationale, and context tags.

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::Result;
use crate::settings::require_signal_feature;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionSignals {
    pub decision_id: i64,
    pub subject: String,
    pub decision: String,
    pub supporting: Vec<RelatedSignal>,
    pub challenging: Vec<RelatedSignal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedSignal {
    pub item_id: i64,
    pub title: String,
    pub source_type: String,
    pub url: Option<String>,
    pub relevance: f64,
    pub reason: String,
    pub discovered_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionHealth {
    pub decision_id: i64,
    pub subject: String,
    pub decision: String,
    pub created_at: String,
    pub days_since: u32,
    pub supporting_count: usize,
    pub challenging_count: usize,
    /// 0.0 = stable, 1.0 = highly challenged
    pub volatility: f32,
    pub status: DecisionHealthStatus,
    pub latest_evidence: Vec<RelatedSignal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionHealthStatus {
    Confident,
    Challenged,
    Stale,
    NeedsReview,
}

// ============================================================================
// Core logic
// ============================================================================

fn find_signals_for_decision(
    conn: &rusqlite::Connection,
    decision: &crate::decisions::DeveloperDecision,
) -> (Vec<RelatedSignal>, Vec<RelatedSignal>) {
    // Build search terms from decision subject + context tags
    let mut terms: Vec<String> = Vec::new();

    // Split subject into meaningful words
    for word in decision.subject.to_lowercase().split_whitespace() {
        if word.len() > 2 {
            terms.push(word.to_string());
        }
    }

    // Add context tags
    for tag in &decision.context_tags {
        terms.push(tag.to_lowercase());
    }

    if terms.is_empty() {
        return (Vec::new(), Vec::new());
    }

    // Build LIKE conditions (any term matching)
    let conditions: Vec<String> = terms
        .iter()
        .map(|t| {
            format!(
                "(LOWER(s.title) LIKE '%{kw}%' OR LOWER(s.content) LIKE '%{kw}%')",
                kw = t.replace('\'', "''")
            )
        })
        .collect();

    let where_clause = conditions.join(" OR ");

    let sql = format!(
        "SELECT s.id, s.title, s.source_type, s.url, s.content, s.created_at
         FROM source_items s
         WHERE ({where_clause})
         AND s.created_at >= datetime('now', '-30 days')
         ORDER BY s.last_seen DESC
         LIMIT 20"
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => {
            debug!(target: "4da::decisions", error = %e, "Signal query failed");
            return (Vec::new(), Vec::new());
        }
    };

    let rows = match stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let title: String = row.get(1)?;
        let source_type: String = row.get(2)?;
        let url: Option<String> = row.get(3)?;
        let content: String = row.get(4)?;
        let discovered_at: String = row.get(5)?;
        Ok((id, title, source_type, url, content, discovered_at))
    }) {
        Ok(r) => r,
        Err(_) => return (Vec::new(), Vec::new()),
    };

    let mut supporting = Vec::new();
    let mut challenging = Vec::new();

    let subject_lower = decision.subject.to_lowercase();

    // Challenging keywords: words that suggest alternatives or problems
    let challenge_words = [
        "deprecated",
        "vulnerability",
        "security issue",
        "migrating away",
        "end of life",
        "replaced by",
        "alternative to",
        "problems with",
        "issues with",
        "breaking change",
    ];

    for row in rows.flatten() {
        let (id, title, source_type, url, content, discovered_at) = row;
        let content_lower = content.to_lowercase();
        let title_lower = title.to_lowercase();

        // Count matching terms for relevance
        let match_count = terms
            .iter()
            .filter(|t| content_lower.contains(t.as_str()) || title_lower.contains(t.as_str()))
            .count();
        let relevance = (match_count as f64 / terms.len().max(1) as f64).min(1.0);

        // Determine if supporting or challenging
        let is_challenging = challenge_words
            .iter()
            .any(|w| content_lower.contains(w) || title_lower.contains(w));

        let reason = if is_challenging {
            format!("Mentions {subject_lower} in context of potential concerns")
        } else {
            format!("References {subject_lower} ({match_count} keyword matches)")
        };

        let signal = RelatedSignal {
            item_id: id,
            title,
            source_type,
            url,
            relevance,
            reason,
            discovered_at,
        };

        if is_challenging {
            challenging.push(signal);
        } else {
            supporting.push(signal);
        }
    }

    // Sort by relevance
    supporting.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    challenging.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Keep top 5 each
    supporting.truncate(5);
    challenging.truncate(5);

    (supporting, challenging)
}

// ============================================================================
// Tauri Command
// ============================================================================

#[tauri::command]
pub async fn get_decision_signals() -> Result<Vec<DecisionSignals>> {
    require_signal_feature("get_decision_signals")?;

    let conn = crate::open_db_connection()?;

    // Get active decisions (all types, all statuses, reasonable limit)
    let decisions = crate::decisions::list_decisions(&conn, None, None, 50)?;

    debug!(
        target: "4da::decisions",
        count = decisions.len(),
        "Finding signals for active decisions"
    );

    let mut results = Vec::new();

    for decision in &decisions {
        // Only process active decisions
        if decision.status == crate::decisions::DecisionStatus::Superseded {
            continue;
        }

        let (supporting, challenging) = find_signals_for_decision(&conn, decision);

        // Only include decisions that have related signals
        if !supporting.is_empty() || !challenging.is_empty() {
            results.push(DecisionSignals {
                decision_id: decision.id,
                subject: decision.subject.clone(),
                decision: decision.decision.clone(),
                supporting,
                challenging,
            });
        }
    }

    Ok(results)
}

// ============================================================================
// Decision Health
// ============================================================================

/// Parse a date string and compute the number of days elapsed since that date.
fn compute_days_since_str(date_str: &str) -> u32 {
    chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S%.f"))
        .map(|dt| {
            let now = chrono::Utc::now().naive_utc();
            (now - dt).num_days().max(0) as u32
        })
        .unwrap_or(0)
}

/// Get health status for all active decisions, enriched with signal counts
/// and volatility metrics.
pub async fn get_all_decision_health() -> Result<Vec<DecisionHealth>> {
    require_signal_feature("get_decision_health_report")?;

    let conn = crate::open_db_connection()?;

    // Get active decisions
    let decisions = crate::decisions::list_decisions(&conn, None, None, 50)?;

    debug!(
        target: "4da::decisions",
        count = decisions.len(),
        "Computing decision health for active decisions"
    );

    let mut health = Vec::new();

    for decision in &decisions {
        if decision.status == crate::decisions::DecisionStatus::Superseded {
            continue;
        }

        let (supporting, challenging) = find_signals_for_decision(&conn, decision);
        let supporting_count = supporting.len();
        let challenging_count = challenging.len();
        let total = supporting_count + challenging_count;

        // Volatility = proportion of challenging signals
        let volatility = if total > 0 {
            challenging_count as f32 / total as f32
        } else {
            0.0
        };

        let days_since = compute_days_since_str(&decision.created_at);

        // Determine status
        let status = if total == 0 && days_since > 30 {
            DecisionHealthStatus::Stale
        } else if volatility > 0.5 || (days_since > 60 && challenging_count > 2) {
            DecisionHealthStatus::NeedsReview
        } else if challenging_count > supporting_count && total > 3 {
            DecisionHealthStatus::Challenged
        } else {
            DecisionHealthStatus::Confident
        };

        // Latest evidence (max 5 most recent from both supporting and challenging)
        let mut latest: Vec<RelatedSignal> = supporting
            .into_iter()
            .chain(challenging.into_iter())
            .collect();
        latest.sort_by(|a, b| b.discovered_at.cmp(&a.discovered_at));
        latest.truncate(5);

        health.push(DecisionHealth {
            decision_id: decision.id,
            subject: decision.subject.clone(),
            decision: decision.decision.clone(),
            created_at: decision.created_at.clone(),
            days_since,
            supporting_count,
            challenging_count,
            volatility,
            status,
            latest_evidence: latest,
        });
    }

    Ok(health)
}

#[tauri::command]
pub async fn get_decision_health_report() -> std::result::Result<Vec<DecisionHealth>, String> {
    get_all_decision_health().await.map_err(|e| e.to_string())
}
