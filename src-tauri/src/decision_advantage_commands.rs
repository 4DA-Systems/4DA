//! Decision Advantage module — types re-exported for tests.
//!
//! Command functions were removed (not registered in invoke_handler).
//! The core logic lives in `decision_advantage::windows` and is called
//! directly from `monitoring.rs` and `scoring/context.rs`.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::decision_advantage::{CompoundAdvantageScore, DecisionWindow};

    #[test]
    fn test_decision_window_default_construction() {
        let window = DecisionWindow {
            id: 0,
            window_type: "adoption".to_string(),
            title: "Consider Bun runtime".to_string(),
            description: "Bun 1.2 brings Node compat".to_string(),
            urgency: 0.5,
            relevance: 0.7,
            dependency: None,
            status: "open".to_string(),
            opened_at: "2025-06-01 12:00:00".to_string(),
            expires_at: None,
            lead_time_hours: None,
            streets_engine: None,
        };
        assert_eq!(window.window_type, "adoption");
        assert!(window.dependency.is_none());
        assert!(window.expires_at.is_none());
    }

    #[test]
    fn test_compound_advantage_score_zero_state() {
        let score = CompoundAdvantageScore {
            score: 0.0,
            period: "weekly".to_string(),
            items_surfaced: 0,
            avg_lead_time_hours: 0.0,
            windows_opened: 0,
            windows_acted: 0,
            windows_expired: 0,
            knowledge_gaps_closed: 0,
            calibration_accuracy: 0.0,
            trend: 0.0,
        };
        let json = serde_json::to_string(&score).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed["score"], 0.0);
        assert_eq!(parsed["period"], "weekly");
    }
}
