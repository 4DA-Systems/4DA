#![allow(dead_code)]
//! Intelligent Autophagy — extracts meta-intelligence from old content before pruning.
//!
//! Analyzes items approaching their retention limit to produce:
//! - **Calibration deltas**: score-vs-reality gaps per topic
//! - **Topic decay profiles**: per-topic engagement half-lives
//! - **Source autopsies**: per-source engagement quality
//! - **Anti-patterns**: systematic over/under-scoring detection
//!
//! All intelligence is stored in `digested_intelligence` and consumed by the
//! scoring pipeline for continuous self-improvement.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

mod anti_patterns;
mod calibration;
mod digest;
mod source_autopsy;
mod topic_decay;

// Public API for orchestration
pub(crate) use digest::run_autophagy_cycle;

// Scoring pipeline integrations
pub(crate) use calibration::load_calibration_deltas;
pub(crate) use topic_decay::load_topic_decay_profiles;

// Individual analyzers (for granular invocation)
pub(crate) use anti_patterns::detect_anti_patterns;
pub(crate) use calibration::analyze_calibration;
pub(crate) use source_autopsy::analyze_sources;
pub(crate) use topic_decay::analyze_topic_decay;

// ============================================================================
// Types
// ============================================================================

/// Result of a complete autophagy cycle, surfaced to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AutophagyCycleResult {
    pub items_analyzed: i64,
    pub items_pruned: i64,
    pub calibrations_produced: i64,
    pub topic_decay_rates_updated: i64,
    pub source_autopsies_produced: i64,
    pub anti_patterns_detected: i64,
    pub duration_ms: i64,
}

/// Score-vs-reality gap for a topic. Positive delta means the system under-scored
/// items the user actually engaged with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CalibrationDelta {
    pub topic: String,
    pub scored_avg: f32,
    pub engaged_avg: f32,
    pub delta: f32,
    pub sample_size: i64,
    pub confidence: f32,
}

/// Topic-specific engagement decay characteristics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TopicDecayProfile {
    pub topic: String,
    pub half_life_hours: f32,
    pub peak_relevance_age_hours: f32,
}

/// Per-source per-topic engagement quality analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SourceAutopsy {
    pub source_type: String,
    pub topic: String,
    pub items_surfaced: i64,
    pub items_engaged: i64,
    pub engagement_rate: f32,
}

/// Systematic scoring bias detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AntiPattern {
    pub pattern_type: String,
    pub topic: String,
    pub avg_score: f32,
    pub engagement_count: i64,
    pub exposure_count: i64,
    pub suggested_penalty: f32,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cycle_result_serialization() {
        let result = AutophagyCycleResult {
            items_analyzed: 100,
            items_pruned: 0,
            calibrations_produced: 5,
            topic_decay_rates_updated: 3,
            source_autopsies_produced: 4,
            anti_patterns_detected: 1,
            duration_ms: 42,
        };
        let json = serde_json::to_string(&result).expect("serialize");
        let parsed: AutophagyCycleResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.items_analyzed, 100);
        assert_eq!(parsed.duration_ms, 42);
    }

    #[test]
    fn test_calibration_delta_fields() {
        let delta = CalibrationDelta {
            topic: "hackernews".to_string(),
            scored_avg: 1.0,
            engaged_avg: 0.15,
            delta: -0.85,
            sample_size: 20,
            confidence: 1.0,
        };
        assert_eq!(delta.topic, "hackernews");
        assert!((delta.delta - (-0.85)).abs() < f32::EPSILON);
    }
}
