//! Confidence Scoring Infrastructure
//!
//! Every signal in ACE must have a confidence score. No unvalidated data enters the model.
//!
//! Confidence Thresholds:
//! - 0.9 - 1.0: Certain (full weight)
//! - 0.7 - 0.9: Confident (normal weight)
//! - 0.5 - 0.7: Probable (reduced weight)
//! - 0.3 - 0.5: Uncertain (minimal weight)
//! - 0.0 - 0.3: Rejected (discarded)

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Confidence score wrapper with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceScore {
    /// The raw confidence value (0.0 - 1.0)
    pub value: f32,
    /// Classification based on thresholds
    pub level: ConfidenceLevel,
    /// Sources contributing to this confidence
    pub evidence_count: usize,
    /// Whether this signal should be used
    pub usable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfidenceLevel {
    Certain,   // 0.9 - 1.0
    Confident, // 0.7 - 0.9
    Probable,  // 0.5 - 0.7
    Uncertain, // 0.3 - 0.5
    Rejected,  // 0.0 - 0.3
}

impl ConfidenceScore {
    /// Create a new confidence score
    pub fn new(value: f32, evidence_count: usize) -> Self {
        let value = value.clamp(0.0, 1.0);
        let level = ConfidenceLevel::from_value(value);
        let usable = level != ConfidenceLevel::Rejected;

        Self {
            value,
            level,
            evidence_count,
            usable,
        }
    }

    /// Create from explicit user input (always certain)
    pub fn from_explicit() -> Self {
        Self {
            value: 1.0,
            level: ConfidenceLevel::Certain,
            evidence_count: 1,
            usable: true,
        }
    }

    /// Apply multi-source bonus
    /// +10% confidence per additional source
    pub fn with_multi_source_bonus(mut self, source_count: usize) -> Self {
        if source_count > 1 {
            let bonus = (source_count - 1) as f32 * 0.1;
            self.value = (self.value + bonus).min(0.95); // Cap at 0.95 for inferred
            self.level = ConfidenceLevel::from_value(self.value);
        }
        self
    }

    /// Get weight to apply to this signal
    pub fn weight(&self) -> f32 {
        match self.level {
            ConfidenceLevel::Certain => 1.0,
            ConfidenceLevel::Confident => 0.9,
            ConfidenceLevel::Probable => 0.7,
            ConfidenceLevel::Uncertain => 0.4,
            ConfidenceLevel::Rejected => 0.0,
        }
    }
}

impl ConfidenceLevel {
    pub fn from_value(value: f32) -> Self {
        match value {
            v if v >= 0.9 => ConfidenceLevel::Certain,
            v if v >= 0.7 => ConfidenceLevel::Confident,
            v if v >= 0.5 => ConfidenceLevel::Probable,
            v if v >= 0.3 => ConfidenceLevel::Uncertain,
            _ => ConfidenceLevel::Rejected,
        }
    }
}

/// Confidence calculation for different signal types
pub struct SignalConfidence;

impl SignalConfidence {
    /// Calculate confidence for a project manifest detection
    pub fn for_manifest(
        has_manifest: bool,
        file_count: usize,
        has_config_files: bool,
        days_since_modified: f32,
    ) -> ConfidenceScore {
        if !has_manifest {
            return ConfidenceScore::new(0.1, 0);
        }

        let manifest_factor = 0.4;
        let file_factor = 0.3 * (file_count as f32 / 10.0).min(1.0);
        let config_factor = if has_config_files { 0.2 } else { 0.0 };
        let recency_factor = 0.1 * recency_decay(days_since_modified);

        let confidence = manifest_factor + file_factor + config_factor + recency_factor;
        let evidence =
            1 + if file_count > 5 { 1 } else { 0 } + if has_config_files { 1 } else { 0 };

        ConfidenceScore::new(confidence, evidence)
    }

    /// Calculate confidence for a file type detection
    pub fn for_file_type(file_count: usize, total_files: usize) -> ConfidenceScore {
        if total_files == 0 || file_count == 0 {
            return ConfidenceScore::new(0.0, 0);
        }

        let ratio = file_count as f32 / total_files as f32;
        let count_factor = (file_count as f32 / 10.0).min(1.0);

        // Higher confidence if this type is dominant
        let confidence = ratio * 0.5 + count_factor * 0.5;
        ConfidenceScore::new(confidence.min(0.8), 1) // Cap at 0.8 for file type alone
    }

    /// Calculate confidence for git-based detection
    pub fn for_git_activity(
        commit_count: usize,
        days_span: f32,
        recent_commits: usize, // in last 7 days
    ) -> ConfidenceScore {
        if commit_count == 0 {
            return ConfidenceScore::new(0.0, 0);
        }

        let volume_factor = (commit_count as f32 / 20.0).min(1.0) * 0.3;
        let recency_factor = (recent_commits as f32 / 5.0).min(1.0) * 0.4;
        let consistency_factor = if days_span > 0.0 {
            (commit_count as f32 / days_span).min(1.0) * 0.3
        } else {
            0.0
        };

        let confidence = volume_factor + recency_factor + consistency_factor;
        let evidence = 1 + if recent_commits > 0 { 1 } else { 0 };

        ConfidenceScore::new(confidence, evidence)
    }

    /// Calculate confidence for behavior-based learning
    pub fn for_behavior(
        positive_signals: u32,
        negative_signals: u32,
        total_exposures: u32,
    ) -> ConfidenceScore {
        if total_exposures < 5 {
            return ConfidenceScore::new(0.2, 0); // Not enough data
        }

        // More data = more confidence
        let data_factor = (total_exposures as f32 / 20.0).min(1.0) * 0.4;

        // Clearer signal = more confidence
        let signal_clarity = if total_exposures > 0 {
            let ratio =
                (positive_signals as f32 - negative_signals as f32).abs() / total_exposures as f32;
            ratio * 0.6
        } else {
            0.0
        };

        let confidence = data_factor + signal_clarity;
        ConfidenceScore::new(confidence, 1)
    }

    /// Combine confidence scores from multiple sources
    pub fn combine(scores: &[ConfidenceScore]) -> ConfidenceScore {
        if scores.is_empty() {
            return ConfidenceScore::new(0.0, 0);
        }

        // Weighted average favoring higher confidence scores
        let total_weight: f32 = scores.iter().map(|s| s.value).sum();
        let weighted_sum: f32 = scores.iter().map(|s| s.value * s.value).sum();

        let base_confidence = if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        };

        // Multi-source bonus
        let source_bonus = (scores.len() - 1) as f32 * 0.1;
        let final_confidence = (base_confidence + source_bonus).min(0.95);

        let total_evidence: usize = scores.iter().map(|s| s.evidence_count).sum();

        ConfidenceScore::new(final_confidence, total_evidence)
    }
}

/// Calculate recency decay factor
/// Half-life of 30 days
fn recency_decay(days_since: f32) -> f32 {
    0.5_f32.powf(days_since / 30.0)
}

/// Calculate temporal decay for learned signals
/// Half-life of 30 days
pub fn temporal_decay(days_since: f32) -> f32 {
    0.5_f32.powf(days_since / 30.0)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_levels() {
        assert_eq!(ConfidenceLevel::from_value(0.95), ConfidenceLevel::Certain);
        assert_eq!(
            ConfidenceLevel::from_value(0.85),
            ConfidenceLevel::Confident
        );
        assert_eq!(ConfidenceLevel::from_value(0.60), ConfidenceLevel::Probable);
        assert_eq!(
            ConfidenceLevel::from_value(0.40),
            ConfidenceLevel::Uncertain
        );
        assert_eq!(ConfidenceLevel::from_value(0.20), ConfidenceLevel::Rejected);
    }

    #[test]
    fn test_manifest_confidence() {
        // Good manifest detection
        let score = SignalConfidence::for_manifest(true, 15, true, 1.0);
        assert!(score.value > 0.7);
        assert!(score.usable);

        // Poor manifest detection
        let score = SignalConfidence::for_manifest(false, 0, false, 100.0);
        assert!(score.value < 0.3);
        assert!(!score.usable);
    }

    #[test]
    fn test_multi_source_bonus() {
        let score = ConfidenceScore::new(0.6, 1);
        let boosted = score.with_multi_source_bonus(3);
        assert!(boosted.value > 0.6);
        assert!(boosted.value <= 0.95);
    }

    #[test]
    fn test_combine_scores() {
        let scores = vec![
            ConfidenceScore::new(0.8, 1),
            ConfidenceScore::new(0.7, 1),
            ConfidenceScore::new(0.6, 1),
        ];

        let combined = SignalConfidence::combine(&scores);
        assert!(combined.value > 0.7); // Should be boosted by multi-source
        assert_eq!(combined.evidence_count, 3);
    }

    #[test]
    fn test_behavior_confidence() {
        // Not enough data
        let score = SignalConfidence::for_behavior(2, 1, 3);
        assert!(score.value < 0.3);

        // Good data with clear signal
        let score = SignalConfidence::for_behavior(15, 2, 20);
        assert!(score.value > 0.5);
    }

    #[test]
    fn test_temporal_decay() {
        assert!((temporal_decay(0.0) - 1.0).abs() < 0.01);
        assert!((temporal_decay(30.0) - 0.5).abs() < 0.01);
        assert!((temporal_decay(60.0) - 0.25).abs() < 0.01);
    }
}
