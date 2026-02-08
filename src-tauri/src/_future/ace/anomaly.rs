//! Anomaly Detection for ACE
//!
//! Detects unusual patterns in context and behavior:
//! - Sudden topic appearances without evidence
//! - Context drift (rapid change in interests)
//! - Contradictory signals
//! - Suspicious activity patterns
//! - Stale data warnings

use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

// ============================================================================
// Anomaly Types
// ============================================================================

/// Types of anomalies that can be detected
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    /// Topic appeared suddenly with no prior evidence
    SuddenAppearance,
    /// Rapid change in topic interests
    ContextDrift,
    /// Conflicting signals about a topic
    Contradiction,
    /// Activity at unusual times
    SuspiciousActivity,
    /// Data hasn't been updated recently
    StaleData,
    /// Unusually high or low activity volume
    AbnormalVolume,
    /// Signal confidence doesn't match evidence
    ConfidenceMismatch,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
    Critical,
}

// ============================================================================
// Anomaly Detector
// ============================================================================

/// Configuration for anomaly detection
#[derive(Debug, Clone)]
pub struct AnomalyConfig {
    /// Threshold for sudden appearance (days since first seen)
    pub sudden_appearance_threshold_days: f32,
    /// Minimum confidence for sudden appearance alert
    pub sudden_appearance_min_confidence: f32,
    /// Threshold for context drift (change rate per day)
    pub drift_threshold: f32,
    /// Window size for drift detection (days)
    pub drift_window_days: u32,
    /// Threshold for stale data warning (hours)
    pub stale_threshold_hours: i64,
    /// Activity volume standard deviations for abnormal
    pub volume_std_threshold: f32,
    /// Maximum anomalies to keep in history
    pub max_history: usize,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            sudden_appearance_threshold_days: 0.5,
            sudden_appearance_min_confidence: 0.7,
            drift_threshold: 0.3,
            drift_window_days: 7,
            stale_threshold_hours: 24,
            volume_std_threshold: 2.0,
            max_history: 100,
        }
    }
}

/// Anomaly detector
pub struct AnomalyDetector {
    conn: Arc<Mutex<Connection>>,
    config: AnomalyConfig,
    /// Recent anomalies
    history: VecDeque<Anomaly>,
    /// Topic first-seen times for sudden appearance detection
    topic_first_seen: HashMap<String, String>,
    /// Activity volume history for abnormal volume detection
    activity_volumes: VecDeque<(String, u32)>, // (date, count)
}

impl AnomalyDetector {
    pub fn new(conn: Arc<Mutex<Connection>>, config: AnomalyConfig) -> Self {
        // Initialize anomaly table
        let _ = Self::init_table(&conn);

        Self {
            conn,
            config,
            history: VecDeque::new(),
            topic_first_seen: HashMap::new(),
            activity_volumes: VecDeque::new(),
        }
    }

    /// Initialize anomaly tracking table
    fn init_table(conn: &Arc<Mutex<Connection>>) -> Result<(), String> {
        let conn = conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS anomalies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                anomaly_type TEXT NOT NULL,
                topic TEXT,
                description TEXT NOT NULL,
                confidence REAL NOT NULL,
                severity TEXT NOT NULL,
                evidence TEXT,
                detected_at TEXT DEFAULT (datetime('now')),
                resolved INTEGER DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_anomalies_type ON anomalies(anomaly_type);
            CREATE INDEX IF NOT EXISTS idx_anomalies_detected ON anomalies(detected_at);",
        )
        .map_err(|e| format!("Failed to create anomalies table: {}", e))?;
        Ok(())
    }

    /// Run all anomaly detection checks
    pub fn detect_all(&mut self) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();

        // Check for sudden appearances
        anomalies.extend(self.detect_sudden_appearances());

        // Check for context drift
        if let Some(drift) = self.detect_context_drift() {
            anomalies.push(drift);
        }

        // Check for stale data
        if let Some(stale) = self.detect_stale_data() {
            anomalies.push(stale);
        }

        // Check for abnormal volume
        if let Some(volume) = self.detect_abnormal_volume() {
            anomalies.push(volume);
        }

        // Check for contradictions
        anomalies.extend(self.detect_contradictions());

        // Check for suspicious activity patterns
        if let Some(suspicious) = self.detect_suspicious_activity() {
            anomalies.push(suspicious);
        }

        // Check for confidence mismatches
        anomalies.extend(self.detect_confidence_mismatch());

        // Store and update history
        for anomaly in &anomalies {
            let _ = self.store_anomaly(anomaly);
            self.history.push_back(anomaly.clone());
        }

        // Trim history
        while self.history.len() > self.config.max_history {
            self.history.pop_front();
        }

        anomalies
    }

    /// Detect topics that appeared suddenly with high confidence
    fn detect_sudden_appearances(&mut self) -> Vec<Anomaly> {
        let conn = self.conn.lock();
        let mut anomalies = Vec::new();

        // Find topics with high confidence but recent creation
        let result = conn.prepare(
            "SELECT topic, confidence, created_at, source
             FROM active_topics
             WHERE confidence >= ?1
               AND julianday('now') - julianday(created_at) < ?2
               AND topic NOT IN (
                   SELECT topic FROM anomalies
                   WHERE anomaly_type = 'sudden_appearance'
                     AND resolved = 0
               )",
        );

        if let Ok(mut stmt) = result {
            let rows = stmt.query_map(
                rusqlite::params![
                    self.config.sudden_appearance_min_confidence,
                    self.config.sudden_appearance_threshold_days
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, f32>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            );

            if let Ok(rows) = rows {
                for row in rows.flatten() {
                    let (topic, confidence, created_at, source) = row;

                    // Check if this topic has prior evidence
                    let has_evidence = self.check_topic_evidence(&topic);

                    if !has_evidence {
                        anomalies.push(Anomaly {
                            id: None,
                            anomaly_type: AnomalyType::SuddenAppearance,
                            topic: Some(topic.clone()),
                            description: format!(
                                "Topic '{}' appeared with {:.0}% confidence but no prior evidence",
                                topic,
                                confidence * 100.0
                            ),
                            confidence: 0.8,
                            severity: if confidence > 0.9 {
                                AnomalySeverity::High
                            } else {
                                AnomalySeverity::Medium
                            },
                            evidence: vec![
                                format!("Created: {}", created_at),
                                format!("Source: {}", source),
                                format!("Confidence: {:.0}%", confidence * 100.0),
                            ],
                            detected_at: chrono::Utc::now().to_rfc3339(),
                            resolved: false,
                        });
                    }
                }
            }
        }

        anomalies
    }

    /// Check if a topic has prior evidence
    fn check_topic_evidence(&self, topic: &str) -> bool {
        let conn = self.conn.lock();

        // Check if topic exists in git history
        let git_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM git_signals WHERE extracted_topics LIKE ?1",
                [format!("%{}%", topic)],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if git_count > 0 {
            return true;
        }

        // Check if topic exists in file signals
        let file_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM file_signals WHERE extracted_topics LIKE ?1",
                [format!("%{}%", topic)],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if file_count > 0 {
            return true;
        }

        // Check if topic exists in detected tech
        let tech_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM detected_tech WHERE name = ?1",
                [topic],
                |row| row.get(0),
            )
            .unwrap_or(0);

        tech_count > 0
    }

    /// Detect rapid changes in topic interests (context drift)
    fn detect_context_drift(&self) -> Option<Anomaly> {
        let conn = self.conn.lock();

        // Get topic changes over the window
        let result = conn.prepare(
            "SELECT topic, weight,
                    julianday('now') - julianday(last_seen) as days_ago
             FROM active_topics
             WHERE julianday('now') - julianday(last_seen) <= ?1
             ORDER BY days_ago",
        );

        if let Ok(mut stmt) = result {
            let rows = stmt.query_map([self.config.drift_window_days], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
            });

            if let Ok(rows) = rows {
                let topics: Vec<(String, f32)> = rows.flatten().collect();

                // Calculate variance in topic weights
                if topics.len() >= 5 {
                    let weights: Vec<f32> = topics.iter().map(|(_, w)| *w).collect();
                    let mean = weights.iter().sum::<f32>() / weights.len() as f32;
                    let variance = weights.iter().map(|w| (w - mean).powi(2)).sum::<f32>()
                        / weights.len() as f32;
                    let std_dev = variance.sqrt();

                    if std_dev > self.config.drift_threshold {
                        return Some(Anomaly {
                            id: None,
                            anomaly_type: AnomalyType::ContextDrift,
                            topic: None,
                            description: format!(
                                "High context volatility detected (σ = {:.2})",
                                std_dev
                            ),
                            confidence: (std_dev / self.config.drift_threshold).min(1.0),
                            severity: if std_dev > self.config.drift_threshold * 2.0 {
                                AnomalySeverity::High
                            } else {
                                AnomalySeverity::Medium
                            },
                            evidence: vec![
                                format!("Topics analyzed: {}", topics.len()),
                                format!("Standard deviation: {:.2}", std_dev),
                                format!("Threshold: {:.2}", self.config.drift_threshold),
                            ],
                            detected_at: chrono::Utc::now().to_rfc3339(),
                            resolved: false,
                        });
                    }
                }
            }
        }

        None
    }

    /// Detect stale data
    fn detect_stale_data(&self) -> Option<Anomaly> {
        let conn = self.conn.lock();

        // Check when the last topic was seen
        let result: Result<Option<String>, _> =
            conn.query_row("SELECT MAX(last_seen) FROM active_topics", [], |row| {
                row.get(0)
            });

        if let Ok(Some(last_seen)) = result {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&last_seen) {
                let hours_since = (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_hours();

                if hours_since > self.config.stale_threshold_hours {
                    return Some(Anomaly {
                        id: None,
                        anomaly_type: AnomalyType::StaleData,
                        topic: None,
                        description: format!("No context updates for {} hours", hours_since),
                        confidence: (hours_since as f32
                            / (self.config.stale_threshold_hours * 2) as f32)
                            .min(1.0),
                        severity: if hours_since > self.config.stale_threshold_hours * 3 {
                            AnomalySeverity::High
                        } else if hours_since > self.config.stale_threshold_hours * 2 {
                            AnomalySeverity::Medium
                        } else {
                            AnomalySeverity::Low
                        },
                        evidence: vec![
                            format!("Last update: {}", last_seen),
                            format!("Hours since: {}", hours_since),
                            format!("Threshold: {} hours", self.config.stale_threshold_hours),
                        ],
                        detected_at: chrono::Utc::now().to_rfc3339(),
                        resolved: false,
                    });
                }
            }
        }

        None
    }

    /// Detect abnormal activity volume
    fn detect_abnormal_volume(&mut self) -> Option<Anomaly> {
        let conn = self.conn.lock();

        // Get daily activity counts for the past week
        let result = conn.prepare(
            "SELECT date(timestamp) as day, COUNT(*) as count
             FROM interactions
             WHERE timestamp > datetime('now', '-7 days')
             GROUP BY day
             ORDER BY day",
        );

        if let Ok(mut stmt) = result {
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
            });

            if let Ok(rows) = rows {
                let volumes: Vec<u32> = rows.flatten().map(|(_, c)| c).collect();

                if volumes.len() >= 3 {
                    // Calculate mean and std dev
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

                    if z_score > self.config.volume_std_threshold {
                        let is_high = today as f32 > mean;
                        return Some(Anomaly {
                            id: None,
                            anomaly_type: AnomalyType::AbnormalVolume,
                            topic: None,
                            description: format!(
                                "Activity volume is {}normal: {} today vs {:.0} average",
                                if is_high { "ab" } else { "sub" },
                                today,
                                mean
                            ),
                            confidence: (z_score / (self.config.volume_std_threshold * 2.0))
                                .min(1.0),
                            severity: if z_score > self.config.volume_std_threshold * 2.0 {
                                AnomalySeverity::High
                            } else {
                                AnomalySeverity::Medium
                            },
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
            }
        }

        None
    }

    /// Detect contradictions between signals
    fn detect_contradictions(&self) -> Vec<Anomaly> {
        let conn = self.conn.lock();
        let mut anomalies = Vec::new();

        // Find topics that have both high affinity and anti-topic status
        let result = conn.prepare(
            "SELECT ta.topic, ta.affinity_score, at.confidence as anti_confidence
             FROM topic_affinities ta
             JOIN anti_topics at ON ta.topic = at.topic
             WHERE ta.affinity_score > 0.3 AND at.confidence > 0.3",
        );

        if let Ok(mut stmt) = result {
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f32>(1)?,
                    row.get::<_, f32>(2)?,
                ))
            });

            if let Ok(rows) = rows {
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
                        confidence: (affinity + anti_confidence) / 2.0,
                        severity: AnomalySeverity::Medium,
                        evidence: vec![
                            format!("Affinity score: {:.0}%", affinity * 100.0),
                            format!("Anti-topic confidence: {:.0}%", anti_confidence * 100.0),
                        ],
                        detected_at: chrono::Utc::now().to_rfc3339(),
                        resolved: false,
                    });
                }
            }
        }

        anomalies
    }

    /// Detect suspicious activity patterns (e.g., unusual hours)
    fn detect_suspicious_activity(&self) -> Option<Anomaly> {
        let conn = self.conn.lock();

        // Check for activity outside normal hours (assuming normal is 6am-11pm)
        // Count interactions in unusual hours (midnight to 6am)
        let unusual_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM interactions
                 WHERE strftime('%H', timestamp) < '06'
                   AND timestamp > datetime('now', '-7 days')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let total_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM interactions
                 WHERE timestamp > datetime('now', '-7 days')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if total_count > 10 {
            let unusual_ratio = unusual_count as f32 / total_count as f32;

            // Flag if >20% of activity is during unusual hours
            if unusual_ratio > 0.2 {
                return Some(Anomaly {
                    id: None,
                    anomaly_type: AnomalyType::SuspiciousActivity,
                    topic: None,
                    description: format!(
                        "Unusual activity pattern: {:.0}% of interactions during unusual hours (midnight-6am)",
                        unusual_ratio * 100.0
                    ),
                    confidence: unusual_ratio.min(1.0),
                    severity: if unusual_ratio > 0.4 {
                        AnomalySeverity::High
                    } else {
                        AnomalySeverity::Medium
                    },
                    evidence: vec![
                        format!("Unusual hour interactions: {}", unusual_count),
                        format!("Total interactions: {}", total_count),
                        format!("Ratio: {:.0}%", unusual_ratio * 100.0),
                    ],
                    detected_at: chrono::Utc::now().to_rfc3339(),
                    resolved: false,
                });
            }
        }

        None
    }

    /// Detect confidence mismatches (high confidence with low evidence)
    fn detect_confidence_mismatch(&self) -> Vec<Anomaly> {
        let conn = self.conn.lock();
        let mut anomalies = Vec::new();

        // Find topics with high confidence but low interaction count
        let result = conn.prepare(
            "SELECT at.topic, at.confidence, at.source,
                    COALESCE((SELECT COUNT(*) FROM interactions i
                              WHERE i.item_topics LIKE '%' || at.topic || '%'), 0) as evidence_count
             FROM active_topics at
             WHERE at.confidence > 0.7
             HAVING evidence_count < 3",
        );

        if let Ok(mut stmt) = result {
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f32>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i32>(3)?,
                ))
            });

            if let Ok(rows) = rows {
                for row in rows.flatten() {
                    let (topic, confidence, source, evidence_count) = row;

                    // Skip if source is 'manual' (user-defined topics are allowed high confidence)
                    if source == "manual" {
                        continue;
                    }

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
                        severity: if confidence > 0.9 && evidence_count == 0 {
                            AnomalySeverity::High
                        } else {
                            AnomalySeverity::Low
                        },
                        evidence: vec![
                            format!("Topic confidence: {:.0}%", confidence * 100.0),
                            format!("Supporting interactions: {}", evidence_count),
                            format!("Source: {}", source),
                        ],
                        detected_at: chrono::Utc::now().to_rfc3339(),
                        resolved: false,
                    });
                }
            }
        }

        anomalies
    }

    /// Store an anomaly in the database
    fn store_anomaly(&self, anomaly: &Anomaly) -> Result<i64, String> {
        let conn = self.conn.lock();

        let type_str = match anomaly.anomaly_type {
            AnomalyType::SuddenAppearance => "sudden_appearance",
            AnomalyType::ContextDrift => "context_drift",
            AnomalyType::Contradiction => "contradiction",
            AnomalyType::SuspiciousActivity => "suspicious_activity",
            AnomalyType::StaleData => "stale_data",
            AnomalyType::AbnormalVolume => "abnormal_volume",
            AnomalyType::ConfidenceMismatch => "confidence_mismatch",
        };

        let severity_str = match anomaly.severity {
            AnomalySeverity::Low => "low",
            AnomalySeverity::Medium => "medium",
            AnomalySeverity::High => "high",
            AnomalySeverity::Critical => "critical",
        };

        let evidence_json = serde_json::to_string(&anomaly.evidence).unwrap_or_default();

        conn.execute(
            "INSERT INTO anomalies (anomaly_type, topic, description, confidence, severity, evidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                type_str,
                anomaly.topic,
                anomaly.description,
                anomaly.confidence,
                severity_str,
                evidence_json
            ],
        ).map_err(|e| format!("Failed to store anomaly: {}", e))?;

        Ok(conn.last_insert_rowid())
    }

    /// Get recent anomalies
    pub fn get_recent(&self, limit: usize) -> Vec<Anomaly> {
        self.history.iter().rev().take(limit).cloned().collect()
    }

    /// Get unresolved anomalies
    pub fn get_unresolved(&self) -> Result<Vec<Anomaly>, String> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT id, anomaly_type, topic, description, confidence, severity, evidence, detected_at
             FROM anomalies
             WHERE resolved = 0
             ORDER BY detected_at DESC
             LIMIT 50"
        ).map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                let type_str: String = row.get(1)?;
                let severity_str: String = row.get(5)?;
                let evidence_json: String = row.get(6)?;

                Ok(Anomaly {
                    id: Some(row.get(0)?),
                    anomaly_type: match type_str.as_str() {
                        "sudden_appearance" => AnomalyType::SuddenAppearance,
                        "context_drift" => AnomalyType::ContextDrift,
                        "contradiction" => AnomalyType::Contradiction,
                        "suspicious_activity" => AnomalyType::SuspiciousActivity,
                        "stale_data" => AnomalyType::StaleData,
                        "abnormal_volume" => AnomalyType::AbnormalVolume,
                        _ => AnomalyType::ConfidenceMismatch,
                    },
                    topic: row.get(2)?,
                    description: row.get(3)?,
                    confidence: row.get(4)?,
                    severity: match severity_str.as_str() {
                        "low" => AnomalySeverity::Low,
                        "medium" => AnomalySeverity::Medium,
                        "high" => AnomalySeverity::High,
                        _ => AnomalySeverity::Critical,
                    },
                    evidence: serde_json::from_str(&evidence_json).unwrap_or_default(),
                    detected_at: row.get(7)?,
                    resolved: false,
                })
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    /// Resolve an anomaly
    pub fn resolve(&self, anomaly_id: i64) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE anomalies SET resolved = 1 WHERE id = ?1",
            [anomaly_id],
        )
        .map_err(|e| format!("Failed to resolve anomaly: {}", e))?;
        Ok(())
    }

    /// Check if anomaly detection is operational
    pub fn is_operational(&self) -> bool {
        true // Always operational
    }
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        let conn = Arc::new(Mutex::new(
            Connection::open_in_memory().expect("Failed to create in-memory connection"),
        ));
        Self::new(conn, AnomalyConfig::default())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Arc<Mutex<Connection>> {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE active_topics (
                topic TEXT UNIQUE,
                weight REAL,
                confidence REAL,
                source TEXT,
                created_at TEXT,
                last_seen TEXT
            );
            CREATE TABLE git_signals (id INTEGER PRIMARY KEY, extracted_topics TEXT);
            CREATE TABLE file_signals (id INTEGER PRIMARY KEY, extracted_topics TEXT);
            CREATE TABLE detected_tech (name TEXT UNIQUE);
            CREATE TABLE topic_affinities (topic TEXT UNIQUE, affinity_score REAL);
            CREATE TABLE anti_topics (topic TEXT UNIQUE, confidence REAL);
            CREATE TABLE interactions (id INTEGER PRIMARY KEY, timestamp TEXT);",
        )
        .unwrap();
        Arc::new(Mutex::new(conn))
    }

    #[test]
    fn test_anomaly_config_defaults() {
        let config = AnomalyConfig::default();
        assert_eq!(config.stale_threshold_hours, 24);
        assert_eq!(config.drift_window_days, 7);
    }

    #[test]
    fn test_anomaly_severity_ordering() {
        assert!(AnomalySeverity::Low < AnomalySeverity::Medium);
        assert!(AnomalySeverity::Medium < AnomalySeverity::High);
        assert!(AnomalySeverity::High < AnomalySeverity::Critical);
    }

    #[test]
    fn test_anomaly_detector_creation() {
        let conn = setup_test_db();
        let detector = AnomalyDetector::new(conn, AnomalyConfig::default());
        assert!(detector.is_operational());
    }

    #[test]
    fn test_detect_stale_data() {
        let conn = setup_test_db();

        // Insert old topic with RFC3339 format timestamp (48 hours ago)
        let old_time = chrono::Utc::now() - chrono::Duration::hours(48);
        let old_time_str = old_time.to_rfc3339();

        {
            let c = conn.lock();
            c.execute(
                "INSERT INTO active_topics (topic, last_seen) VALUES ('old_topic', ?1)",
                [&old_time_str],
            )
            .unwrap();
        }

        let detector = AnomalyDetector::new(conn, AnomalyConfig::default());
        let stale = detector.detect_stale_data();

        assert!(stale.is_some());
        assert_eq!(stale.unwrap().anomaly_type, AnomalyType::StaleData);
    }

    #[test]
    fn test_detect_contradiction() {
        let conn = setup_test_db();

        // Insert contradicting data
        {
            let c = conn.lock();
            c.execute(
                "INSERT INTO topic_affinities (topic, affinity_score) VALUES ('conflicting', 0.8)",
                [],
            )
            .unwrap();
            c.execute(
                "INSERT INTO anti_topics (topic, confidence) VALUES ('conflicting', 0.7)",
                [],
            )
            .unwrap();
        }

        let detector = AnomalyDetector::new(conn, AnomalyConfig::default());
        let contradictions = detector.detect_contradictions();

        assert!(!contradictions.is_empty());
        assert_eq!(contradictions[0].anomaly_type, AnomalyType::Contradiction);
    }

    #[test]
    fn test_store_and_get_anomaly() {
        let conn = setup_test_db();
        let detector = AnomalyDetector::new(conn.clone(), AnomalyConfig::default());

        let anomaly = Anomaly {
            id: None,
            anomaly_type: AnomalyType::SuddenAppearance,
            topic: Some("test_topic".to_string()),
            description: "Test anomaly".to_string(),
            confidence: 0.8,
            severity: AnomalySeverity::Medium,
            evidence: vec!["evidence1".to_string()],
            detected_at: chrono::Utc::now().to_rfc3339(),
            resolved: false,
        };

        let id = detector.store_anomaly(&anomaly).unwrap();
        assert!(id > 0);

        let unresolved = detector.get_unresolved().unwrap();
        assert!(!unresolved.is_empty());
    }
}
