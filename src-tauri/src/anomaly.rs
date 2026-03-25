//! Anomaly Detection for ACE
//!
//! Detects unusual patterns in context and behavior:
//! - Stale data (no context updates in >24h)
//! - Context drift (high variance in topic weights over 7 days)
//! - Contradictions (topic with both high affinity AND anti-topic status)
//! - Abnormal volume (activity z-score >2 from 7-day mean)
//! - Confidence mismatch (high confidence with <3 supporting interactions)

use crate::error::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

// ============================================================================
// Anomaly Types
// ============================================================================

/// Types of anomalies that can be detected
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    /// Data hasn't been updated recently
    StaleData,
    /// Rapid change in topic interests
    ContextDrift,
    /// Conflicting signals about a topic
    Contradiction,
    /// Unusually high or low activity volume
    AbnormalVolume,
    /// Signal confidence doesn't match evidence
    ConfidenceMismatch,
}

impl AnomalyType {
    fn as_str(&self) -> &'static str {
        match self {
            AnomalyType::StaleData => "stale_data",
            AnomalyType::ContextDrift => "context_drift",
            AnomalyType::Contradiction => "contradiction",
            AnomalyType::AbnormalVolume => "abnormal_volume",
            AnomalyType::ConfidenceMismatch => "confidence_mismatch",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "stale_data" => AnomalyType::StaleData,
            "context_drift" => AnomalyType::ContextDrift,
            "contradiction" => AnomalyType::Contradiction,
            "abnormal_volume" => AnomalyType::AbnormalVolume,
            "confidence_mismatch" => AnomalyType::ConfidenceMismatch,
            _ => AnomalyType::ConfidenceMismatch, // fallback
        }
    }
}

/// Severity of a detected anomaly
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl AnomalySeverity {
    fn as_str(&self) -> &'static str {
        match self {
            AnomalySeverity::Low => "low",
            AnomalySeverity::Medium => "medium",
            AnomalySeverity::High => "high",
            AnomalySeverity::Critical => "critical",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "low" => AnomalySeverity::Low,
            "medium" => AnomalySeverity::Medium,
            "high" => AnomalySeverity::High,
            "critical" => AnomalySeverity::Critical,
            _ => AnomalySeverity::Medium, // fallback
        }
    }
}

/// A detected anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub id: Option<i64>,
    pub anomaly_type: AnomalyType,
    pub topic: Option<String>,
    pub description: String,
    pub confidence: f32,
    pub severity: AnomalySeverity,
    pub evidence: Vec<String>,
    pub detected_at: String,
    pub resolved: bool,
}

// ============================================================================
// Detection Functions
// ============================================================================

/// Run all anomaly detection checks
pub fn detect_all(conn: &Connection) -> Result<Vec<Anomaly>> {
    let mut anomalies = Vec::new();

    match detect_stale_data(conn) {
        Ok(results) => anomalies.extend(results),
        Err(e) => warn!(target: "4da::anomaly", error = %e, "Stale data detection failed"),
    }

    match detect_context_drift(conn) {
        Ok(results) => anomalies.extend(results),
        Err(e) => warn!(target: "4da::anomaly", error = %e, "Context drift detection failed"),
    }

    match detect_contradictions(conn) {
        Ok(results) => anomalies.extend(results),
        Err(e) => warn!(target: "4da::anomaly", error = %e, "Contradiction detection failed"),
    }

    match detect_abnormal_volume(conn) {
        Ok(results) => anomalies.extend(results),
        Err(e) => warn!(target: "4da::anomaly", error = %e, "Abnormal volume detection failed"),
    }

    match detect_confidence_mismatch(conn) {
        Ok(results) => anomalies.extend(results),
        Err(e) => warn!(target: "4da::anomaly", error = %e, "Confidence mismatch detection failed"),
    }

    debug!(target: "4da::anomaly", count = anomalies.len(), "Anomaly detection complete");
    Ok(anomalies)
}

/// Detect stale data - no context updates in >24 hours
///
/// Checks the `file_signals` table for the most recent signal timestamp.
/// If no signals exist or the most recent is >24h old, flags as stale.
pub fn detect_stale_data(conn: &Connection) -> Result<Vec<Anomaly>> {
    let mut anomalies = Vec::new();

    // Check file_signals for most recent timestamp
    let last_signal: Option<String> = conn
        .query_row("SELECT MAX(timestamp) FROM file_signals", [], |row| {
            row.get(0)
        })
        .unwrap_or(None);

    let stale_threshold_hours: i64 = 24;

    match last_signal {
        None => {
            // No file signals at all = stale
            anomalies.push(Anomaly {
                id: None,
                anomaly_type: AnomalyType::StaleData,
                topic: None,
                description: "No file signals recorded - context may be uninitialized".to_string(),
                confidence: 0.9,
                severity: AnomalySeverity::Medium,
                evidence: vec!["No entries in file_signals table".to_string()],
                detected_at: chrono::Utc::now().to_rfc3339(),
                resolved: false,
            });
        }
        Some(timestamp) => {
            // Try parsing as SQLite datetime format first, then RFC3339
            let hours_since = parse_hours_since(&timestamp);

            if let Some(hours) = hours_since {
                if hours > stale_threshold_hours {
                    let severity = if hours > stale_threshold_hours * 3 {
                        AnomalySeverity::High
                    } else if hours > stale_threshold_hours * 2 {
                        AnomalySeverity::Medium
                    } else {
                        AnomalySeverity::Low
                    };

                    anomalies.push(Anomaly {
                        id: None,
                        anomaly_type: AnomalyType::StaleData,
                        topic: None,
                        description: format!("No context updates for {hours} hours"),
                        confidence: (hours as f32 / (stale_threshold_hours * 2) as f32).min(1.0),
                        severity,
                        evidence: vec![
                            format!("Last signal: {}", timestamp),
                            format!("Hours since: {}", hours),
                            format!("Threshold: {} hours", stale_threshold_hours),
                        ],
                        detected_at: chrono::Utc::now().to_rfc3339(),
                        resolved: false,
                    });
                }
            }
        }
    }

    Ok(anomalies)
}

/// Detect context drift - high variance in topic weights over 7 days
///
/// Checks `topic_affinities` for high variance in affinity scores.
/// If std deviation of affinity scores exceeds threshold, flags as drift.
pub fn detect_context_drift(conn: &Connection) -> Result<Vec<Anomaly>> {
    let mut anomalies = Vec::new();
    let drift_threshold: f32 = 0.3;

    // Get topic affinities that were updated in the last 7 days
    let mut stmt = conn.prepare(
        "SELECT topic, affinity_score FROM topic_affinities
             WHERE last_interaction > datetime('now', '-7 days')
             ORDER BY last_interaction DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;

    let topics: Vec<(String, f64)> = rows.flatten().collect();

    if topics.len() >= 5 {
        let scores: Vec<f64> = topics.iter().map(|(_, s)| *s).collect();
        let mean = scores.iter().sum::<f64>() / scores.len() as f64;
        let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev > drift_threshold as f64 {
            let severity = if std_dev > drift_threshold as f64 * 2.0 {
                AnomalySeverity::High
            } else {
                AnomalySeverity::Medium
            };

            anomalies.push(Anomaly {
                id: None,
                anomaly_type: AnomalyType::ContextDrift,
                topic: None,
                description: format!("High context volatility detected (sigma = {std_dev:.2})"),
                confidence: (std_dev as f32 / drift_threshold).min(1.0),
                severity,
                evidence: vec![
                    format!("Topics analyzed: {}", topics.len()),
                    format!("Standard deviation: {:.2}", std_dev),
                    format!("Threshold: {:.2}", drift_threshold),
                ],
                detected_at: chrono::Utc::now().to_rfc3339(),
                resolved: false,
            });
        }
    }

    Ok(anomalies)
}

/// Detect contradictions - topics with both high affinity AND anti-topic status
///
/// Cross-checks `topic_affinities` and `anti_topics` tables for topics
/// that appear in both with significant confidence.
pub fn detect_contradictions(conn: &Connection) -> Result<Vec<Anomaly>> {
    let mut anomalies = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT ta.topic, ta.affinity_score, at.confidence as anti_confidence
             FROM topic_affinities ta
             JOIN anti_topics at ON ta.topic = at.topic
             WHERE ta.affinity_score > 0.3 AND at.confidence > 0.3",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, f64>(2)?,
        ))
    })?;

    for row in rows.flatten() {
        let (topic, affinity, anti_confidence) = row;
        anomalies.push(Anomaly {
            id: None,
            anomaly_type: AnomalyType::Contradiction,
            topic: Some(topic.clone()),
            description: format!(
                "Topic '{}' has conflicting signals: affinity {:.0}% vs rejection {:.0}%",
                topic,
                affinity * 100.0,
                anti_confidence * 100.0
            ),
            confidence: f64::midpoint(affinity, anti_confidence) as f32,
            severity: AnomalySeverity::Medium,
            evidence: vec![
                format!("Affinity score: {:.0}%", affinity * 100.0),
                format!("Anti-topic confidence: {:.0}%", anti_confidence * 100.0),
            ],
            detected_at: chrono::Utc::now().to_rfc3339(),
            resolved: false,
        });
    }

    Ok(anomalies)
}

/// Return topic names that have contradictory signals (high affinity AND anti-topic).
///
/// Lightweight query for scoring pipeline — returns just the names, not full anomaly records.
/// Used to boost necessity_score for content touching conflicted topics.
pub fn get_contradicted_topics(conn: &Connection) -> Result<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT ta.topic
         FROM topic_affinities ta
         JOIN anti_topics at ON ta.topic = at.topic
         WHERE ta.affinity_score > 0.3 AND at.confidence > 0.3",
    )?;

    let topics = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(std::result::Result::ok)
        .collect();

    Ok(topics)
}

/// Detect abnormal volume - z-score >2 from 7-day mean
///
/// Analyzes daily interaction counts from the `interactions` table.
/// If today's count deviates by more than 2 standard deviations, flags it.
pub fn detect_abnormal_volume(conn: &Connection) -> Result<Vec<Anomaly>> {
    let mut anomalies = Vec::new();
    let volume_std_threshold: f32 = 2.0;

    // Get daily interaction counts for the past 7 days
    let mut stmt = conn.prepare(
        "SELECT date(timestamp) as day, COUNT(*) as count
             FROM interactions
             WHERE timestamp > datetime('now', '-7 days')
             GROUP BY day
             ORDER BY day",
    )?;

    let rows = stmt.query_map([], |row| row.get::<_, u32>(1))?;

    let volumes: Vec<u32> = rows.flatten().collect();

    if volumes.len() >= 3 {
        let mean = volumes.iter().sum::<u32>() as f32 / volumes.len() as f32;
        let variance = volumes
            .iter()
            .map(|v| (*v as f32 - mean).powi(2))
            .sum::<f32>()
            / volumes.len() as f32;
        let std_dev = variance.sqrt();

        // Get today's volume
        let today: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM interactions WHERE date(timestamp) = date('now')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let z_score = if std_dev > 0.0 {
            (today as f32 - mean).abs() / std_dev
        } else {
            0.0
        };

        if z_score > volume_std_threshold {
            let is_high = today as f32 > mean;
            let severity = if z_score > volume_std_threshold * 2.0 {
                AnomalySeverity::High
            } else {
                AnomalySeverity::Medium
            };

            anomalies.push(Anomaly {
                id: None,
                anomaly_type: AnomalyType::AbnormalVolume,
                topic: None,
                description: format!(
                    "Activity volume is {}normal: {} today vs {:.0} average",
                    if is_high { "ab" } else { "sub" },
                    today,
                    mean
                ),
                confidence: (z_score / (volume_std_threshold * 2.0)).min(1.0),
                severity,
                evidence: vec![
                    format!("Today's count: {}", today),
                    format!("7-day average: {:.0}", mean),
                    format!("Z-score: {:.2}", z_score),
                ],
                detected_at: chrono::Utc::now().to_rfc3339(),
                resolved: false,
            });
        }
    }

    Ok(anomalies)
}

/// Detect confidence mismatch - high confidence with <3 supporting interactions
///
/// Checks `topic_affinities` for topics where confidence is high (>0.7)
/// but total evidence (positive_signals + negative_signals) is less than 3.
pub fn detect_confidence_mismatch(conn: &Connection) -> Result<Vec<Anomaly>> {
    let mut anomalies = Vec::new();

    // Find topics with high confidence but low interaction count
    // The actual schema uses positive_signals + negative_signals instead of interaction_count
    let mut stmt = conn
        .prepare(
            "SELECT topic, confidence, affinity_score,
                    (COALESCE(positive_signals, 0) + COALESCE(negative_signals, 0)) as evidence_count
             FROM topic_affinities
             WHERE confidence > 0.7
               AND (COALESCE(positive_signals, 0) + COALESCE(negative_signals, 0)) < 3",
        )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, i32>(3)?,
        ))
    })?;

    for row in rows.flatten() {
        let (topic, confidence, _affinity, evidence_count) = row;

        let severity = if confidence > 0.9 && evidence_count == 0 {
            AnomalySeverity::High
        } else {
            AnomalySeverity::Low
        };

        anomalies.push(Anomaly {
            id: None,
            anomaly_type: AnomalyType::ConfidenceMismatch,
            topic: Some(topic.clone()),
            description: format!(
                "Topic '{}' has {:.0}% confidence but only {} supporting interactions",
                topic,
                confidence * 100.0,
                evidence_count
            ),
            confidence: 0.7,
            severity,
            evidence: vec![
                format!("Topic confidence: {:.0}%", confidence * 100.0),
                format!("Supporting interactions: {}", evidence_count),
            ],
            detected_at: chrono::Utc::now().to_rfc3339(),
            resolved: false,
        });
    }

    Ok(anomalies)
}

// ============================================================================
// Storage Functions
// ============================================================================

/// Store an anomaly in the database, returns the new row id
pub fn store_anomaly(conn: &Connection, anomaly: &Anomaly) -> Result<i64> {
    let evidence_json =
        serde_json::to_string(&anomaly.evidence).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO anomalies (anomaly_type, topic, description, confidence, severity, evidence, detected_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            anomaly.anomaly_type.as_str(),
            anomaly.topic,
            anomaly.description,
            anomaly.confidence,
            anomaly.severity.as_str(),
            evidence_json,
            anomaly.detected_at,
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

/// Get all unresolved anomalies, ordered by most recent first
pub fn get_unresolved(conn: &Connection) -> Result<Vec<Anomaly>> {
    let mut stmt = conn.prepare(
        "SELECT id, anomaly_type, topic, description, confidence, severity, evidence, detected_at
             FROM anomalies
             WHERE resolved = 0
             ORDER BY detected_at DESC
             LIMIT 50",
    )?;

    let rows = stmt.query_map([], |row| {
        let evidence_json: String = row.get::<_, String>(6).unwrap_or_else(|_| "[]".to_string());

        Ok(Anomaly {
            id: Some(row.get(0)?),
            anomaly_type: AnomalyType::from_str(&row.get::<_, String>(1)?),
            topic: row.get(2)?,
            description: row.get(3)?,
            confidence: row.get(4)?,
            severity: AnomalySeverity::from_str(&row.get::<_, String>(5)?),
            evidence: serde_json::from_str(&evidence_json).unwrap_or_default(),
            detected_at: row.get(7)?,
            resolved: false,
        })
    })?;

    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(std::convert::Into::into)
}

/// Mark an anomaly as resolved
pub fn resolve_anomaly(conn: &Connection, id: i64) -> Result<()> {
    let changed = conn.execute("UPDATE anomalies SET resolved = 1 WHERE id = ?1", [id])?;

    if changed == 0 {
        return Err(format!("Anomaly with id {id} not found").into());
    }

    info!(target: "4da::anomaly", anomaly_id = id, "Anomaly resolved");
    Ok(())
}

// ============================================================================
// Helpers
// ============================================================================

/// Parse hours since a timestamp string (supports both SQLite datetime and RFC3339)
fn parse_hours_since(timestamp: &str) -> Option<i64> {
    // Try RFC3339 first
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(timestamp) {
        let hours = (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_hours();
        return Some(hours);
    }

    // Try SQLite datetime format (YYYY-MM-DD HH:MM:SS)
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S") {
        let dt = naive.and_utc();
        let hours = (chrono::Utc::now() - dt).num_hours();
        return Some(hours);
    }

    None
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // Create minimal schema needed for tests
        conn.execute_batch(
            r#"
            CREATE TABLE file_signals (
                id INTEGER PRIMARY KEY,
                path TEXT,
                change_type TEXT,
                timestamp TEXT,
                extracted_topics TEXT,
                content_hash TEXT,
                processed INTEGER DEFAULT 0
            );
            CREATE TABLE topic_affinities (
                id INTEGER PRIMARY KEY,
                topic TEXT UNIQUE,
                affinity_score REAL,
                confidence REAL,
                positive_signals INTEGER DEFAULT 0,
                negative_signals INTEGER DEFAULT 0,
                total_exposures INTEGER DEFAULT 0,
                last_interaction TEXT,
                last_decay_at TEXT
            );
            CREATE TABLE anti_topics (
                id INTEGER PRIMARY KEY,
                topic TEXT UNIQUE,
                confidence REAL,
                rejection_count INTEGER DEFAULT 0,
                auto_detected INTEGER DEFAULT 1,
                user_confirmed INTEGER DEFAULT 0,
                first_rejection TEXT,
                last_rejection TEXT
            );
            CREATE TABLE interactions (
                id INTEGER PRIMARY KEY,
                item_id INTEGER,
                action_type TEXT,
                action_data TEXT,
                item_topics TEXT,
                item_source TEXT,
                signal_strength REAL,
                timestamp TEXT DEFAULT (datetime('now'))
            );
            CREATE TABLE anomalies (
                id INTEGER PRIMARY KEY,
                anomaly_type TEXT NOT NULL,
                topic TEXT,
                description TEXT NOT NULL,
                confidence REAL DEFAULT 0.5,
                severity TEXT DEFAULT 'medium',
                evidence TEXT DEFAULT '[]',
                detected_at TEXT DEFAULT (datetime('now')),
                resolved INTEGER DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_anomalies_resolved ON anomalies(resolved);
            CREATE INDEX IF NOT EXISTS idx_anomalies_type ON anomalies(anomaly_type);
            "#,
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_detect_stale_data_no_signals() {
        let conn = setup_test_db();
        // No file signals at all = stale
        let anomalies = detect_stale_data(&conn).unwrap();
        assert!(
            !anomalies.is_empty(),
            "Should detect stale data when no signals exist"
        );
        assert_eq!(anomalies[0].anomaly_type, AnomalyType::StaleData);
    }

    #[test]
    fn test_detect_stale_data_recent_signals() {
        let conn = setup_test_db();
        // Insert a recent file signal
        conn.execute(
            "INSERT INTO file_signals (path, change_type, timestamp) VALUES ('test.rs', 'modified', datetime('now'))",
            [],
        )
        .unwrap();
        let anomalies = detect_stale_data(&conn).unwrap();
        assert!(
            anomalies.is_empty(),
            "Should not detect stale data when recent signals exist"
        );
    }

    #[test]
    fn test_detect_contradiction() {
        let conn = setup_test_db();
        // Insert a topic that's both an affinity AND an anti-topic
        conn.execute(
            "INSERT INTO topic_affinities (topic, affinity_score, confidence, last_interaction, positive_signals) VALUES ('rust', 0.8, 0.9, datetime('now'), 5)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO anti_topics (topic, confidence, rejection_count) VALUES ('rust', 0.7, 3)",
            [],
        )
        .unwrap();
        let anomalies = detect_contradictions(&conn).unwrap();
        assert!(!anomalies.is_empty(), "Should detect contradiction");
        assert_eq!(anomalies[0].anomaly_type, AnomalyType::Contradiction);
        assert_eq!(anomalies[0].topic, Some("rust".to_string()));
    }

    #[test]
    fn test_store_and_retrieve_anomaly() {
        let conn = setup_test_db();
        let anomaly = Anomaly {
            id: None,
            anomaly_type: AnomalyType::StaleData,
            topic: Some("test".to_string()),
            description: "Test anomaly".to_string(),
            confidence: 0.8,
            severity: AnomalySeverity::Medium,
            evidence: vec!["evidence1".to_string()],
            detected_at: chrono::Utc::now().to_rfc3339(),
            resolved: false,
        };
        let id = store_anomaly(&conn, &anomaly).unwrap();
        assert!(id > 0);

        let unresolved = get_unresolved(&conn).unwrap();
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].description, "Test anomaly");
    }

    #[test]
    fn test_resolve_anomaly() {
        let conn = setup_test_db();
        let anomaly = Anomaly {
            id: None,
            anomaly_type: AnomalyType::Contradiction,
            topic: Some("python".to_string()),
            description: "Contradiction found".to_string(),
            confidence: 0.7,
            severity: AnomalySeverity::High,
            evidence: vec![],
            detected_at: chrono::Utc::now().to_rfc3339(),
            resolved: false,
        };
        let id = store_anomaly(&conn, &anomaly).unwrap();
        resolve_anomaly(&conn, id).unwrap();
        let unresolved = get_unresolved(&conn).unwrap();
        assert_eq!(unresolved.len(), 0);
    }

    #[test]
    fn test_detect_confidence_mismatch() {
        let conn = setup_test_db();
        // High confidence but low interaction count
        conn.execute(
            "INSERT INTO topic_affinities (topic, affinity_score, confidence, last_interaction, positive_signals, negative_signals) VALUES ('obscure-topic', 0.7, 0.95, datetime('now'), 1, 0)",
            [],
        )
        .unwrap();
        let anomalies = detect_confidence_mismatch(&conn).unwrap();
        assert!(
            !anomalies.is_empty(),
            "Should detect confidence mismatch with <3 interactions"
        );
        assert_eq!(anomalies[0].anomaly_type, AnomalyType::ConfidenceMismatch);
    }

    #[test]
    fn test_detect_all_runs_without_error() {
        let conn = setup_test_db();
        let anomalies = detect_all(&conn).unwrap();
        // Should at least detect stale data (no signals in test DB)
        assert!(
            !anomalies.is_empty(),
            "detect_all should find at least stale data anomaly"
        );
    }

    #[test]
    fn test_anomaly_type_roundtrip() {
        let types = vec![
            AnomalyType::StaleData,
            AnomalyType::ContextDrift,
            AnomalyType::Contradiction,
            AnomalyType::AbnormalVolume,
            AnomalyType::ConfidenceMismatch,
        ];
        for t in types {
            let s = t.as_str();
            let recovered = AnomalyType::from_str(s);
            assert_eq!(t, recovered, "Roundtrip failed for {:?}", t);
        }
    }

    #[test]
    fn test_anomaly_severity_ordering() {
        assert!(AnomalySeverity::Low < AnomalySeverity::Medium);
        assert!(AnomalySeverity::Medium < AnomalySeverity::High);
        assert!(AnomalySeverity::High < AnomalySeverity::Critical);
    }

    #[test]
    fn test_resolve_nonexistent_anomaly() {
        let conn = setup_test_db();
        let result = resolve_anomaly(&conn, 99999);
        assert!(
            result.is_err(),
            "Should error when resolving nonexistent anomaly"
        );
    }
}
