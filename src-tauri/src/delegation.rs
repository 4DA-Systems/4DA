// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Delegation Scoring — AI Task Delegation Reliability for 4DA
//!
//! Scores how reliably tasks can be delegated to AI agents, based on
//! existing data: tech radar position, security signals, codebase complexity,
//! active decisions, and AI track record.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tracing::info;
use ts_rs::TS;

use crate::error::{Result, ResultExt};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DelegationScore {
    pub subject: String,
    pub overall_score: f64,
    pub factors: DelegationFactors,
    pub recommendation: DelegationRec,
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DelegationFactors {
    pub pattern_stability: f64,
    pub security_sensitivity: f64,
    pub codebase_complexity: f64,
    pub decision_density: f64,
    pub ai_track_record: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum DelegationRec {
    FullyDelegate,
    DelegateWithReview,
    CollaborateRealtime,
    HumanOnly,
}

// ============================================================================
// Factor Weights
// ============================================================================

const W_PATTERN_STABILITY: f64 = 0.30;
const W_SECURITY_SENSITIVITY: f64 = 0.25;
const W_CODEBASE_COMPLEXITY: f64 = 0.20;
const W_DECISION_DENSITY: f64 = 0.15;
const W_AI_TRACK_RECORD: f64 = 0.10;

// ============================================================================
// Core Computation
// ============================================================================

/// Compute a delegation score for a given subject based on existing 4DA data.
pub fn compute_delegation_score(conn: &Connection, subject: &str) -> Result<DelegationScore> {
    let subject_lower = subject.to_lowercase();

    let pattern_stability = compute_pattern_stability(conn, &subject_lower);
    let security_sensitivity = compute_security_sensitivity(conn, &subject_lower);
    let codebase_complexity = compute_codebase_complexity(conn, &subject_lower);
    let decision_density = compute_decision_density(conn, &subject_lower);
    let ai_track_record = compute_ai_track_record(conn, &subject_lower);

    let overall = (pattern_stability * W_PATTERN_STABILITY
        + (1.0 - security_sensitivity) * W_SECURITY_SENSITIVITY
        + (1.0 - codebase_complexity) * W_CODEBASE_COMPLEXITY
        + (1.0 - decision_density * 0.5) * W_DECISION_DENSITY
        + ai_track_record * W_AI_TRACK_RECORD)
        .clamp(0.0, 1.0);

    let recommendation = match overall {
        s if s >= 0.8 => DelegationRec::FullyDelegate,
        s if s >= 0.5 => DelegationRec::DelegateWithReview,
        s if s >= 0.3 => DelegationRec::CollaborateRealtime,
        _ => DelegationRec::HumanOnly,
    };

    // Generate caveats from factor values
    let mut caveats = Vec::new();

    if security_sensitivity > 0.7 {
        let alert_count = count_security_alerts(conn, &subject_lower);
        caveats.push(format!(
            "Security-sensitive area: {} security alerts in 90 days",
            alert_count
        ));
    }
    if decision_density > 0.5 {
        caveats.push("Active decisions constrain this area".to_string());
    }
    if ai_track_record < 0.5 {
        caveats.push("Previous AI warnings recorded for this subject".to_string());
    }
    if pattern_stability < 0.4 {
        caveats.push("Technology is in flux".to_string());
    }

    let factors = DelegationFactors {
        pattern_stability,
        security_sensitivity,
        codebase_complexity,
        decision_density,
        ai_track_record,
    };

    info!(
        target: "4da::delegation",
        subject = subject,
        overall = format!("{:.3}", overall),
        rec = ?recommendation,
        "Delegation score computed"
    );

    Ok(DelegationScore {
        subject: subject.to_string(),
        overall_score: overall,
        factors,
        recommendation,
        caveats,
    })
}

/// Compute delegation scores for all entries in tech_stack, sorted by score DESC.
pub fn compute_all_delegation_scores(conn: &Connection) -> Result<Vec<DelegationScore>> {
    let mut stmt = conn
        .prepare("SELECT technology FROM tech_stack")
        .context("Failed to query tech_stack")?;

    let techs: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .context("Failed to read tech_stack")?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Row processing failed in delegation: {e}");
                None
            }
        })
        .collect();

    let mut scores = Vec::with_capacity(techs.len());
    for tech in &techs {
        scores.push(compute_delegation_score(conn, tech)?);
    }

    scores.sort_by(|a, b| {
        b.overall_score
            .partial_cmp(&a.overall_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    info!(
        target: "4da::delegation",
        count = scores.len(),
        "All delegation scores computed"
    );

    Ok(scores)
}

// ============================================================================
// Factor Computations
// ============================================================================

/// Pattern stability (weight 0.30): How established is this technology in the user's stack?
fn compute_pattern_stability(conn: &Connection, subject: &str) -> f64 {
    // Check tech_stack (primary = Adopt tier) -> 0.9
    let in_stack: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM tech_stack WHERE LOWER(technology) = ?1",
            params![subject],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if in_stack {
        // Check if decisions are reconsidering this tech
        let reconsidering: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM developer_decisions
                 WHERE LOWER(subject) LIKE ?1 AND status = 'reconsidering'",
                params![format!("%{}%", subject)],
                |row| row.get(0),
            )
            .unwrap_or(0);

        return if reconsidering > 0 { 0.3 } else { 0.9 };
    }

    // Check project_dependencies (Trial tier) -> 0.7
    let in_deps: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM project_dependencies WHERE LOWER(package_name) LIKE ?1",
            params![format!("%{}%", subject)],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if in_deps {
        return 0.7;
    }

    // Check detected_tech (Assess tier) -> 0.4
    let in_detected: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM detected_tech WHERE LOWER(name) LIKE ?1",
            params![format!("%{}%", subject)],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if in_detected {
        return 0.4;
    }

    // Check if rejected in decisions -> 0.1
    let rejected: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM developer_decisions
             WHERE LOWER(alternatives_rejected) LIKE ?1 AND status = 'active'",
            params![format!("%{}%", subject)],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if rejected {
        return 0.1;
    }

    // Default: unknown tech
    0.5
}

/// Security sensitivity (weight 0.25): How many security-related signals exist?
/// Higher value = more sensitive = worse for delegation.
fn compute_security_sensitivity(conn: &Connection, subject: &str) -> f64 {
    let alert_count = count_security_alerts(conn, subject);

    match alert_count {
        0 => 0.1,
        1..=2 => 0.5,
        _ => 0.9,
    }
}

/// Count security-related source_items mentioning the subject in the last 90 days.
fn count_security_alerts(conn: &Connection, subject: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM source_items
         WHERE (LOWER(title) LIKE ?1 OR LOWER(content) LIKE ?1)
         AND (LOWER(title) LIKE '%security%' OR LOWER(title) LIKE '%vulnerability%' OR LOWER(title) LIKE '%cve%')
         AND created_at >= datetime('now', '-90 days')",
        params![format!("%{}%", subject)],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// Codebase complexity (weight 0.20): How deeply embedded is this in the codebase?
/// Higher value = more complex = worse for delegation.
fn compute_codebase_complexity(conn: &Connection, subject: &str) -> f64 {
    let dep_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM project_dependencies WHERE LOWER(package_name) LIKE ?1",
            params![format!("%{}%", subject)],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let detected_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM detected_tech WHERE LOWER(name) LIKE ?1",
            params![format!("%{}%", subject)],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let dep_score: f64 = match dep_count {
        0 => 0.1,
        1..=3 => 0.3,
        4..=10 => 0.6,
        _ => 0.9,
    };

    let detected_score: f64 = match detected_count {
        0 => 0.0,
        1 => 0.1,
        2..=3 => 0.2,
        _ => 0.3,
    };

    (dep_score + detected_score).clamp(0.0, 1.0)
}

/// Decision density (weight 0.15): How many active decisions constrain this area?
/// Higher value = more constrained = worse for delegation.
fn compute_decision_density(conn: &Connection, subject: &str) -> f64 {
    let decision_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM developer_decisions
             WHERE status = 'active'
             AND (LOWER(subject) LIKE ?1 OR LOWER(context_tags) LIKE ?1)",
            params![format!("%{}%", subject)],
            |row| row.get(0),
        )
        .unwrap_or(0);

    match decision_count {
        0 => 0.1,
        1 => 0.3,
        2..=3 => 0.6,
        _ => 0.9,
    }
}

/// AI track record (weight 0.10): How well has AI performed on this subject?
/// Higher value = better track record = better for delegation.
fn compute_ai_track_record(conn: &Connection, subject: &str) -> f64 {
    let warning_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM agent_memory
             WHERE memory_type = 'warning'
             AND LOWER(subject) LIKE ?1",
            params![format!("%{}%", subject)],
            |row| row.get(0),
        )
        .unwrap_or(0);

    match warning_count {
        0 => 0.9,
        1 => 0.6,
        2 => 0.3,
        _ => 0.1,
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn get_delegation_score(subject: String) -> Result<DelegationScore> {
    let conn = crate::open_db_connection()?;
    compute_delegation_score(&conn, &subject)
}

#[tauri::command]
pub async fn get_all_delegation_scores() -> Result<Vec<DelegationScore>> {
    let conn = crate::open_db_connection()?;
    compute_all_delegation_scores(&conn)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        crate::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tech_stack (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                technology TEXT NOT NULL UNIQUE
            );
            CREATE TABLE project_dependencies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT,
                manifest_type TEXT,
                package_name TEXT,
                version TEXT,
                is_dev INTEGER DEFAULT 0,
                is_direct INTEGER DEFAULT 1,
                language TEXT,
                last_scanned TEXT DEFAULT (datetime('now')),
                UNIQUE(project_path, package_name)
            );
            CREATE TABLE detected_tech (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                category TEXT,
                confidence REAL DEFAULT 0.5
            );
            CREATE TABLE developer_decisions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                decision_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                decision TEXT NOT NULL,
                rationale TEXT,
                alternatives_rejected TEXT DEFAULT '[]',
                context_tags TEXT DEFAULT '[]',
                confidence REAL NOT NULL DEFAULT 0.8,
                status TEXT NOT NULL DEFAULT 'active',
                superseded_by INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE agent_memory (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                agent_type TEXT NOT NULL,
                memory_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                content TEXT NOT NULL,
                context_tags TEXT DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT,
                promoted_to_decision_id INTEGER
            );
            CREATE TABLE source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                url TEXT,
                title TEXT NOT NULL,
                content TEXT DEFAULT '',
                content_hash TEXT DEFAULT '',
                embedding BLOB DEFAULT x'00',
                created_at TEXT DEFAULT (datetime('now')),
                last_seen TEXT DEFAULT (datetime('now')),
                UNIQUE(source_type, source_id)
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_high_delegation_score() {
        let conn = setup_test_db();

        // Subject in primary tech stack -> high pattern stability
        conn.execute("INSERT INTO tech_stack (technology) VALUES ('rust')", [])
            .unwrap();

        // No security alerts, no warnings, minimal complexity
        let score = compute_delegation_score(&conn, "rust").unwrap();

        assert_eq!(score.subject, "rust");
        assert!(
            score.overall_score >= 0.7,
            "Expected high score, got {}",
            score.overall_score
        );
        assert!(
            score.recommendation == DelegationRec::FullyDelegate
                || score.recommendation == DelegationRec::DelegateWithReview,
            "Expected FullyDelegate or DelegateWithReview, got {:?}",
            score.recommendation
        );
        assert!(score.factors.pattern_stability > 0.8);
        assert!(score.factors.security_sensitivity < 0.3);
        assert!(score.factors.ai_track_record > 0.8);
        assert!(score.caveats.is_empty());
    }

    #[test]
    fn test_low_delegation_score() {
        let conn = setup_test_db();

        // Subject rejected in decisions -> low pattern stability
        conn.execute(
            "INSERT INTO developer_decisions (decision_type, subject, decision, alternatives_rejected, status)
             VALUES ('tech_choice', 'sqlite', 'Use SQLite', '[\"mongodb\"]', 'active')",
            [],
        )
        .unwrap();

        // Security alerts for the subject
        for i in 0..4 {
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, content)
                 VALUES ('hackernews', ?1, ?2, 'mongodb related content')",
                params![
                    format!("hn-sec-{}", i),
                    format!("Critical MongoDB security vulnerability CVE-2025-{}", i)
                ],
            )
            .unwrap();
        }

        // AI warnings for the subject
        for i in 0..3 {
            conn.execute(
                "INSERT INTO agent_memory (session_id, agent_type, memory_type, subject, content)
                 VALUES ('s1', 'claude', 'warning', 'mongodb', ?1)",
                params![format!("Warning {} about mongodb usage", i)],
            )
            .unwrap();
        }

        let score = compute_delegation_score(&conn, "mongodb").unwrap();

        assert!(
            score.overall_score < 0.5,
            "Expected low score, got {}",
            score.overall_score
        );
        assert!(
            score.recommendation == DelegationRec::HumanOnly
                || score.recommendation == DelegationRec::CollaborateRealtime,
            "Expected HumanOnly or CollaborateRealtime, got {:?}",
            score.recommendation
        );
        assert!(score.factors.pattern_stability < 0.2);
        assert!(score.factors.security_sensitivity > 0.7);
        assert!(score.factors.ai_track_record < 0.2);
        assert!(!score.caveats.is_empty());
        assert!(score.caveats.iter().any(|c| c.contains("security")));
        assert!(score.caveats.iter().any(|c| c.contains("AI warnings")));
    }

    #[test]
    fn test_compute_all_delegation_scores() {
        let conn = setup_test_db();

        conn.execute("INSERT INTO tech_stack (technology) VALUES ('rust')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO tech_stack (technology) VALUES ('typescript')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO tech_stack (technology) VALUES ('python')", [])
            .unwrap();

        // Add some complexity for typescript
        for i in 0..5 {
            conn.execute(
                "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, language)
                 VALUES ('/proj', 'npm', ?1, '1.0', 'typescript')",
                params![format!("typescript-pkg-{}", i)],
            )
            .unwrap();
        }

        let scores = compute_all_delegation_scores(&conn).unwrap();

        assert_eq!(scores.len(), 3);
        // All should have scores since they're in tech_stack
        for score in &scores {
            assert!(score.overall_score > 0.0);
            assert!(score.overall_score <= 1.0);
        }
        // Should be sorted by overall_score DESC
        for window in scores.windows(2) {
            assert!(window[0].overall_score >= window[1].overall_score);
        }
    }

    #[test]
    fn test_reconsidering_lowers_stability() {
        let conn = setup_test_db();

        conn.execute("INSERT INTO tech_stack (technology) VALUES ('react')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO developer_decisions (decision_type, subject, decision, status)
             VALUES ('tech_choice', 'react', 'Use React', 'reconsidering')",
            [],
        )
        .unwrap();

        let score = compute_delegation_score(&conn, "react").unwrap();

        assert!(
            score.factors.pattern_stability <= 0.3,
            "Expected low stability for reconsidering tech, got {}",
            score.factors.pattern_stability
        );
        assert!(score.caveats.iter().any(|c| c.contains("in flux")));
    }

    #[test]
    fn test_decision_density_caveat() {
        let conn = setup_test_db();

        // Multiple active decisions about the subject
        for i in 0..4 {
            conn.execute(
                "INSERT INTO developer_decisions (decision_type, subject, decision, status)
                 VALUES ('tech_choice', ?1, ?2, 'active')",
                params![
                    format!("sqlite-{}", i),
                    format!("Decision {} about sqlite", i)
                ],
            )
            .unwrap();
        }

        let score = compute_delegation_score(&conn, "sqlite").unwrap();

        assert!(score.factors.decision_density > 0.5);
        assert!(
            score.caveats.iter().any(|c| c.contains("Active decisions")),
            "Expected decision density caveat, got: {:?}",
            score.caveats
        );
    }
}
