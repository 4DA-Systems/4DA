//! Decision Advantage — detects time-bounded decision windows from signal chains
//! and dependency intelligence, tracks user response, and builds a compound
//! advantage score.
//!
//! Window types:
//! - `security_patch` — CVE/vulnerability affecting a project dependency
//! - `migration` — breaking change or deprecation in a dependency
//! - `adoption` — new tool/library adjacent to the user's stack
//! - `knowledge` — knowledge gap with escalating severity

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub(crate) mod compound_score;
pub(crate) mod scoring_boost;
pub(crate) mod windows;

pub(crate) use compound_score::compute_compound_score;
pub(crate) use scoring_boost::compute_decision_window_boost;
pub(crate) use windows::{
    detect_decision_windows, expire_stale_windows, get_open_windows, transition_window,
};

// ============================================================================
// Types
// ============================================================================

/// A time-bounded decision opportunity surfaced by the intelligence pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DecisionWindow {
    pub id: i64,
    /// One of: security_patch, migration, adoption, knowledge
    pub window_type: String,
    pub title: String,
    pub description: String,
    /// 0.0 - 1.0 urgency score
    pub urgency: f32,
    /// 0.0 - 1.0 relevance to user's context
    pub relevance: f32,
    /// Package name or topic that triggered this window
    pub dependency: Option<String>,
    /// One of: open, acted, expired, closed
    pub status: String,
    pub opened_at: String,
    pub expires_at: Option<String>,
    /// Hours between window open and now (or when acted)
    pub lead_time_hours: Option<f32>,
    /// STREETS engine mapping (Automation, Consulting, Digital Products, Education)
    pub streets_engine: Option<String>,
}

/// Compound advantage metric — measures cumulative value of the intelligence pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct CompoundAdvantageScore {
    /// 0 - 100 composite score
    pub score: f32,
    /// Period this score covers: daily, weekly, monthly
    pub period: String,
    pub items_surfaced: i64,
    pub avg_lead_time_hours: f32,
    pub windows_opened: i64,
    pub windows_acted: i64,
    pub windows_expired: i64,
    pub knowledge_gaps_closed: i64,
    /// How well the system's predictions match reality (0.0 - 1.0)
    pub calibration_accuracy: f32,
    /// -1.0 (declining) to +1.0 (growing) trend vs previous period
    pub trend: f32,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_window_serialization() {
        let window = DecisionWindow {
            id: 1,
            window_type: "security_patch".to_string(),
            title: "CVE-2025-1234 in lodash".to_string(),
            description: "Critical prototype pollution vulnerability".to_string(),
            urgency: 0.9,
            relevance: 0.85,
            dependency: Some("lodash".to_string()),
            status: "open".to_string(),
            opened_at: "2025-01-15 10:00:00".to_string(),
            expires_at: Some("2025-01-22 10:00:00".to_string()),
            lead_time_hours: None,
            streets_engine: Some("Automation".to_string()),
        };
        let json = serde_json::to_string(&window).expect("serialize");
        let parsed: DecisionWindow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.id, 1);
        assert_eq!(parsed.window_type, "security_patch");
        assert!((parsed.urgency - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_compound_score_serialization() {
        let score = CompoundAdvantageScore {
            score: 72.5,
            period: "weekly".to_string(),
            items_surfaced: 150,
            avg_lead_time_hours: 48.0,
            windows_opened: 5,
            windows_acted: 3,
            windows_expired: 1,
            knowledge_gaps_closed: 2,
            calibration_accuracy: 0.78,
            trend: 0.15,
        };
        let json = serde_json::to_string(&score).expect("serialize");
        let parsed: CompoundAdvantageScore = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.period, "weekly");
        assert!((parsed.score - 72.5).abs() < f32::EPSILON);
        assert_eq!(parsed.windows_acted, 3);
    }
}
