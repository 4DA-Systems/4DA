// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Anomaly Detection for ACE
//!
//! Detects unusual patterns in context and behavior:
//! - Stale data (no context updates in >24h)
//! - Abnormal volume (activity z-score >2 from 7-day mean)
//!
//! v20b (AD-031): the three affinity/anti-topic detectors — context drift,
//! contradiction, confidence mismatch — were removed with the implicit-capture
//! layer. Their `AnomalyType` variants remain so historical stored anomaly
//! rows still deserialize and render.

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

    match detect_abnormal_volume(conn) {
        Ok(results) => anomalies.extend(results),
        Err(e) => warn!(target: "4da::anomaly", error = %e, "Abnormal volume detection failed"),
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

#[cfg(test)]
#[path = "anomaly_tests.rs"]
mod tests;
