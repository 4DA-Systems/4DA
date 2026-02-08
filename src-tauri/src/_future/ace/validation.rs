//! Signal Validation Layer
//!
//! Validates signals before they enter the context model:
//! - Confidence scoring
//! - Cross-validation between sources
//! - Anomaly detection
//! - Contradiction detection

#![allow(dead_code)]

use super::confidence::{ConfidenceScore, SignalConfidence};
use super::scanner::ProjectSignal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Signal Validator - ensures signal quality
pub struct SignalValidator {
    /// Historical signals for anomaly detection
    history: HashMap<String, Vec<HistoricalSignal>>,
    /// Configuration thresholds
    config: ValidationConfig,
}

#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Minimum confidence to accept a signal
    pub min_confidence: f32,
    /// Threshold for sudden appearance anomaly
    pub sudden_appearance_threshold: f32,
    /// Threshold for contradiction detection
    pub contradiction_threshold: f32,
    /// Maximum history entries per topic
    pub max_history: usize,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.3,
            sudden_appearance_threshold: 0.7,
            contradiction_threshold: 0.5,
            max_history: 100,
        }
    }
}

/// Result of signal validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub confidence: f32,
    pub anomalies: Vec<Anomaly>,
    pub contradictions: Vec<Contradiction>,
    pub evidence: Vec<String>,
}

/// A validated signal ready for use
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedSignal {
    pub signal_type: String,
    pub topic: String,
    pub confidence: f32,
    pub evidence_sources: Vec<String>,
    pub validation_time: String,
}

/// Historical signal for trend analysis
#[derive(Debug, Clone)]
struct HistoricalSignal {
    confidence: f32,
    timestamp: chrono::DateTime<chrono::Utc>,
}

/// Detected anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub anomaly_type: AnomalyType,
    pub topic: String,
    pub confidence: f32,
    pub recommendation: AnomalyAction,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    SuddenAppearance,    // New topic appears with high confidence
    SuddenDisappearance, // Active topic suddenly gone
    ContextDrift,        // Gradual shift detected
    Contradiction,       // Signals conflict
    StaleContext,        // No updates for extended period
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyAction {
    AcceptWithCaution,   // Apply but flag
    RequestConfirmation, // Ask user
    Reject,              // Don't apply
    RefreshRequired,     // Force refresh
}

/// Detected contradiction between signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contradiction {
    pub topic: String,
    pub source_a: String,
    pub source_b: String,
    pub confidence_a: f32,
    pub confidence_b: f32,
    pub severity: f32,
}

impl SignalValidator {
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
            config: ValidationConfig::default(),
        }
    }

    /// Validate a project signal from manifest scanning
    pub fn validate_project_signal(&self, signal: &ProjectSignal) -> ValidationResult {
        let mut anomalies = Vec::new();
        let mut evidence = Vec::new();

        // Calculate base confidence
        let has_manifest = true; // By definition, we found a manifest
        let has_config = signal.languages.len() > 0 || signal.frameworks.len() > 0;
        let file_count = signal.dependencies.len() + signal.dev_dependencies.len();
        let days_since = 0.0; // Assume recent for now

        let confidence_score =
            SignalConfidence::for_manifest(has_manifest, file_count, has_config, days_since);

        evidence.push(format!("Manifest: {}", signal.manifest_path.display()));
        if !signal.languages.is_empty() {
            evidence.push(format!("Languages: {}", signal.languages.join(", ")));
        }
        if !signal.frameworks.is_empty() {
            evidence.push(format!("Frameworks: {}", signal.frameworks.join(", ")));
        }
        if file_count > 0 {
            evidence.push(format!("Dependencies: {} total", file_count));
        }

        // Check for anomalies
        for lang in &signal.languages {
            if let Some(anomaly) = self.check_sudden_appearance(lang, confidence_score.value) {
                anomalies.push(anomaly);
            }
        }

        ValidationResult {
            valid: confidence_score.usable,
            confidence: confidence_score.value,
            anomalies,
            contradictions: Vec::new(),
            evidence,
        }
    }

    /// Validate a topic signal
    pub fn validate_topic(&self, topic: &str, confidence: f32, source: &str) -> ValidationResult {
        let mut anomalies = Vec::new();
        let contradictions = Vec::new();

        // Check for sudden appearance
        if let Some(anomaly) = self.check_sudden_appearance(topic, confidence) {
            anomalies.push(anomaly);
        }

        // Check for stale context
        if let Some(history) = self.history.get(topic) {
            if let Some(last) = history.last() {
                let hours_since = (chrono::Utc::now() - last.timestamp).num_hours();
                if hours_since > 168 {
                    // 1 week
                    anomalies.push(Anomaly {
                        anomaly_type: AnomalyType::StaleContext,
                        topic: topic.to_string(),
                        confidence,
                        recommendation: AnomalyAction::RefreshRequired,
                        description: format!("No updates for {} hours", hours_since),
                    });
                }
            }
        }

        let valid = confidence >= self.config.min_confidence
            && !anomalies
                .iter()
                .any(|a| a.recommendation == AnomalyAction::Reject);

        ValidationResult {
            valid,
            confidence,
            anomalies,
            contradictions,
            evidence: vec![format!("Source: {}", source)],
        }
    }

    /// Cross-validate signals from multiple sources
    pub fn cross_validate(&self, signals: &[ValidatedSignal]) -> CrossValidationResult {
        let mut topic_sources: HashMap<String, Vec<&ValidatedSignal>> = HashMap::new();

        // Group signals by topic
        for signal in signals {
            topic_sources
                .entry(signal.topic.clone())
                .or_default()
                .push(signal);
        }

        let mut validated_topics = Vec::new();
        let mut contradictions = Vec::new();

        for (topic, sources) in topic_sources {
            let source_count = sources.len();

            // Calculate combined confidence
            let confidence_scores: Vec<ConfidenceScore> = sources
                .iter()
                .map(|s| ConfidenceScore::new(s.confidence, 1))
                .collect();

            let combined = SignalConfidence::combine(&confidence_scores);

            // Check for contradictions (large confidence gaps)
            if source_count > 1 {
                let max_conf = sources.iter().map(|s| s.confidence).fold(0.0f32, f32::max);
                let min_conf = sources.iter().map(|s| s.confidence).fold(1.0f32, f32::min);

                if max_conf - min_conf > self.config.contradiction_threshold {
                    let high_source = sources
                        .iter()
                        .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
                        .unwrap();
                    let low_source = sources
                        .iter()
                        .min_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
                        .unwrap();

                    contradictions.push(Contradiction {
                        topic: topic.clone(),
                        source_a: high_source
                            .evidence_sources
                            .first()
                            .cloned()
                            .unwrap_or_default(),
                        source_b: low_source
                            .evidence_sources
                            .first()
                            .cloned()
                            .unwrap_or_default(),
                        confidence_a: max_conf,
                        confidence_b: min_conf,
                        severity: max_conf - min_conf,
                    });
                }
            }

            validated_topics.push(CrossValidatedTopic {
                topic: topic.clone(),
                combined_confidence: combined.value,
                source_count,
                evidence: sources
                    .iter()
                    .flat_map(|s| s.evidence_sources.clone())
                    .collect(),
            });
        }

        // Compute overall confidence before moving
        let overall_confidence = if validated_topics.is_empty() {
            0.0
        } else {
            validated_topics
                .iter()
                .map(|t| t.combined_confidence)
                .sum::<f32>()
                / validated_topics.len() as f32
        };

        CrossValidationResult {
            topics: validated_topics,
            contradictions,
            overall_confidence,
        }
    }

    /// Check for sudden appearance anomaly
    fn check_sudden_appearance(&self, topic: &str, confidence: f32) -> Option<Anomaly> {
        if confidence < self.config.sudden_appearance_threshold {
            return None;
        }

        if !self.history.contains_key(topic) {
            Some(Anomaly {
                anomaly_type: AnomalyType::SuddenAppearance,
                topic: topic.to_string(),
                confidence,
                recommendation: AnomalyAction::AcceptWithCaution,
                description: format!(
                    "New topic '{}' appeared with high confidence ({:.0}%)",
                    topic,
                    confidence * 100.0
                ),
            })
        } else {
            None
        }
    }

    /// Record a signal in history for future validation
    pub fn record_signal(&mut self, topic: &str, confidence: f32) {
        let entry = HistoricalSignal {
            confidence,
            timestamp: chrono::Utc::now(),
        };

        let history = self.history.entry(topic.to_string()).or_default();
        history.push(entry);

        // Limit history size
        if history.len() > self.config.max_history {
            history.remove(0);
        }
    }

    /// Detect context drift over time
    pub fn detect_drift(&self, topic: &str) -> Option<Anomaly> {
        let history = self.history.get(topic)?;

        if history.len() < 5 {
            return None;
        }

        // Compare recent average to historical average
        let recent: Vec<_> = history.iter().rev().take(5).collect();
        let older: Vec<_> = history.iter().rev().skip(5).take(10).collect();

        if older.is_empty() {
            return None;
        }

        let recent_avg: f32 =
            recent.iter().map(|s| s.confidence).sum::<f32>() / recent.len() as f32;
        let older_avg: f32 = older.iter().map(|s| s.confidence).sum::<f32>() / older.len() as f32;

        let drift = (recent_avg - older_avg).abs();

        if drift > 0.2 {
            Some(Anomaly {
                anomaly_type: AnomalyType::ContextDrift,
                topic: topic.to_string(),
                confidence: recent_avg,
                recommendation: AnomalyAction::AcceptWithCaution,
                description: format!(
                    "Confidence drifted from {:.0}% to {:.0}%",
                    older_avg * 100.0,
                    recent_avg * 100.0
                ),
            })
        } else {
            None
        }
    }
}

impl Default for SignalValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of cross-validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossValidationResult {
    pub topics: Vec<CrossValidatedTopic>,
    pub contradictions: Vec<Contradiction>,
    pub overall_confidence: f32,
}

/// A topic validated across multiple sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossValidatedTopic {
    pub topic: String,
    pub combined_confidence: f32,
    pub source_count: usize,
    pub evidence: Vec<String>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::scanner::ManifestType;
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_validate_project_signal() {
        let validator = SignalValidator::new();

        let signal = ProjectSignal {
            manifest_type: ManifestType::CargoToml,
            manifest_path: PathBuf::from("/test/Cargo.toml"),
            project_name: Some("test-project".to_string()),
            languages: vec!["rust".to_string()],
            frameworks: vec!["tokio".to_string(), "axum".to_string()],
            dependencies: vec!["tokio".to_string(), "serde".to_string()],
            dev_dependencies: vec!["pretty_assertions".to_string()],
            detected_at: String::new(),
        };

        let result = validator.validate_project_signal(&signal);
        assert!(result.valid);
        assert!(result.confidence > 0.3);
        assert!(!result.evidence.is_empty());
    }

    #[test]
    fn test_sudden_appearance_detection() {
        let validator = SignalValidator::new();

        // New topic with high confidence should trigger anomaly
        let result = validator.validate_topic("new_topic", 0.8, "manifest");
        assert!(result
            .anomalies
            .iter()
            .any(|a| a.anomaly_type == AnomalyType::SuddenAppearance));
    }

    #[test]
    fn test_cross_validation() {
        let validator = SignalValidator::new();

        let signals = vec![
            ValidatedSignal {
                signal_type: "language".to_string(),
                topic: "rust".to_string(),
                confidence: 0.9,
                evidence_sources: vec!["Cargo.toml".to_string()],
                validation_time: String::new(),
            },
            ValidatedSignal {
                signal_type: "language".to_string(),
                topic: "rust".to_string(),
                confidence: 0.8,
                evidence_sources: vec!["file_extension".to_string()],
                validation_time: String::new(),
            },
        ];

        let result = validator.cross_validate(&signals);
        assert_eq!(result.topics.len(), 1);
        assert!(result.topics[0].combined_confidence > 0.8); // Multi-source boost
        assert_eq!(result.topics[0].source_count, 2);
    }

    #[test]
    fn test_contradiction_detection() {
        let validator = SignalValidator::new();

        let signals = vec![
            ValidatedSignal {
                signal_type: "interest".to_string(),
                topic: "crypto".to_string(),
                confidence: 0.9,
                evidence_sources: vec!["file_content".to_string()],
                validation_time: String::new(),
            },
            ValidatedSignal {
                signal_type: "interest".to_string(),
                topic: "crypto".to_string(),
                confidence: 0.2,
                evidence_sources: vec!["behavior".to_string()],
                validation_time: String::new(),
            },
        ];

        let result = validator.cross_validate(&signals);
        assert!(!result.contradictions.is_empty());
        assert_eq!(result.contradictions[0].topic, "crypto");
    }
}
