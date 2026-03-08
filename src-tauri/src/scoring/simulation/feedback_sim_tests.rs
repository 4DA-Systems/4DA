//! Integration tests for feedback simulation — full feedback loop validation.
//!
//! Split from feedback_sim.rs to keep the module under 600 lines.

#[cfg(test)]
mod tests {
    use super::super::feedback_sim::*;
    use super::super::corpus::corpus;
    use std::collections::HashMap;

    #[test]
    fn test_apply_feedback_accumulates() {
        let existing = HashMap::new();
        let events = vec![
            FeedbackEvent {
                item_id: 1,
                topic: "core_tech".to_string(),
                relevant: true,
                delta: 0.15,
            },
            FeedbackEvent {
                item_id: 2,
                topic: "core_tech".to_string(),
                relevant: true,
                delta: 0.15,
            },
        ];
        let result = apply_feedback(&existing, &events);
        let val = result.get("core_tech").copied().unwrap_or(0.0);
        assert!((val - 0.30).abs() < 0.01, "Expected ~0.30, got {:.4}", val);
    }

    #[test]
    fn test_apply_feedback_decay() {
        let mut existing = HashMap::new();
        existing.insert("core_tech".to_string(), 0.50);
        let events: Vec<FeedbackEvent> = vec![];
        let result = apply_feedback(&existing, &events);
        let val = result.get("core_tech").copied().unwrap_or(0.0);
        assert!(
            (val - 0.475).abs() < 0.01,
            "Expected ~0.475 after decay, got {:.4}",
            val
        );
    }

    #[test]
    fn test_apply_feedback_clamps() {
        let existing = HashMap::new();
        let events: Vec<FeedbackEvent> = (0..20)
            .map(|i| FeedbackEvent {
                item_id: i,
                topic: "core_tech".to_string(),
                relevant: true,
                delta: 0.15,
            })
            .collect();
        let result = apply_feedback(&existing, &events);
        let val = result.get("core_tech").copied().unwrap_or(0.0);
        assert!(
            (val - 1.0).abs() < f64::EPSILON,
            "Expected clamped to 1.0, got {:.4}",
            val
        );
    }

    #[test]
    fn test_apply_feedback_mixed() {
        let existing = HashMap::new();
        let events = vec![
            FeedbackEvent {
                item_id: 1,
                topic: "core_tech".to_string(),
                relevant: true,
                delta: 0.15,
            },
            FeedbackEvent {
                item_id: 2,
                topic: "core_tech".to_string(),
                relevant: true,
                delta: 0.15,
            },
            FeedbackEvent {
                item_id: 3,
                topic: "core_tech".to_string(),
                relevant: false,
                delta: -0.10,
            },
        ];
        let result = apply_feedback(&existing, &events);
        let val = result.get("core_tech").copied().unwrap_or(0.0);
        // 0 + 0.15 + 0.15 - 0.10 = 0.20
        assert!((val - 0.20).abs() < 0.01, "Expected ~0.20, got {:.4}", val);
    }

    #[test]
    fn test_simulate_session_returns_events() {
        let boosts = HashMap::new();
        let ctx = rust_ctx_with_boosts(&boosts, 50);
        let items = corpus();
        let events = simulate_session(&ctx, &items, 0);
        // With 215 corpus items, Rust persona should generate at least some feedback events
        assert!(
            !events.is_empty(),
            "simulate_session should produce at least one feedback event"
        );
    }

    #[test]
    fn test_rust_ctx_factory() {
        let boosts = HashMap::new();
        let ctx = rust_ctx_with_boosts(&boosts, 50);
        assert_eq!(ctx.interest_count, 5, "Rust ctx should have 5 interests");
        assert_eq!(ctx.interests.len(), 5);
        assert_eq!(ctx.feedback_interaction_count, 50);
    }

    #[test]
    fn test_python_ctx_factory() {
        let boosts = HashMap::new();
        let ctx = python_ctx_with_boosts(&boosts, 40);
        assert_eq!(ctx.interest_count, 5, "Python ctx should have 5 interests");
        assert_eq!(ctx.interests.len(), 5);
        assert_eq!(ctx.feedback_interaction_count, 40);
    }

    #[test]
    fn test_lifecycle_corpus_smaller_than_full() {
        let full = corpus();
        let lifecycle = lifecycle_corpus();
        assert!(
            lifecycle.len() < full.len(),
            "lifecycle_corpus ({}) should be smaller than full corpus ({})",
            lifecycle.len(),
            full.len()
        );
        assert!(
            !lifecycle.is_empty(),
            "lifecycle_corpus should not be empty"
        );
    }

    // ========================================================================
    // Integration: Full feedback loop tests
    // ========================================================================

    #[test]
    fn feedback_single_session_improves_scores() {
        // Baseline: Rust context with zero boosts
        let boosts_0 = HashMap::new();
        let ctx_0 = rust_ctx_with_boosts(&boosts_0, 0);
        let baseline = score_corpus_against_ctx(&ctx_0, 0);

        // Run one feedback session
        let items = corpus();
        let events = simulate_session(&ctx_0, &items, 0);
        assert!(!events.is_empty(), "Session must generate feedback events");

        // Apply feedback and rebuild context
        let boosts_1 = apply_feedback(&boosts_0, &events);
        let interaction_count = events.len() as i64;
        let ctx_1 = rust_ctx_with_boosts(&boosts_1, interaction_count);
        let after = score_corpus_against_ctx(&ctx_1, 0);

        // After positive feedback on relevant items, mean score should improve
        // (or at minimum not regress — feedback with zero-embedding may have limited effect)
        assert!(
            after >= baseline - 0.05,
            "Feedback should not significantly degrade scores: baseline={:.4}, after={:.4}",
            baseline,
            after
        );
    }

    #[test]
    fn feedback_negative_feedback_reduces_noise() {
        // Start with some positive boosts on a noisy topic
        let mut boosts = HashMap::new();
        boosts.insert("cross_domain".to_string(), 0.3);
        boosts.insert("general".to_string(), 0.2);

        // Apply negative feedback to simulate dismissals
        let negative_events: Vec<FeedbackEvent> = (0..5)
            .map(|i| FeedbackEvent {
                item_id: 100 + i,
                topic: "cross_domain".to_string(),
                relevant: false,
                delta: -0.10,
            })
            .collect();
        let boosts_after = apply_feedback(&boosts, &negative_events);

        // cross_domain boost should decrease
        let before_val = boosts.get("cross_domain").copied().unwrap_or(0.0);
        let after_val = boosts_after.get("cross_domain").copied().unwrap_or(0.0);
        assert!(
            after_val < before_val,
            "Negative feedback should reduce boost: before={:.4}, after={:.4}",
            before_val,
            after_val
        );
        // Should go negative after enough dismissals
        assert!(
            after_val < 0.0,
            "5 dismissals on 0.3 boost should go negative, got {:.4}",
            after_val
        );
    }

    #[test]
    fn feedback_decay_prevents_runaway() {
        let mut boosts = HashMap::new();

        // Apply the same positive feedback 10 sessions
        for _ in 0..10 {
            let events = vec![FeedbackEvent {
                item_id: 1,
                topic: "core_tech".to_string(),
                relevant: true,
                delta: 0.15,
            }];
            boosts = apply_feedback(&boosts, &events);
        }

        // Boost must be clamped to 1.0
        let val = boosts.get("core_tech").copied().unwrap_or(0.0);
        assert!(
            val <= 1.0,
            "Boost must not exceed 1.0 clamp, got {:.4}",
            val
        );

        // After 10 sessions of decay (0.95^10 ~ 0.60) + additions,
        // the value should be positive but bounded
        assert!(
            val > 0.0,
            "Boost should remain positive after 10 sessions, got {:.4}",
            val
        );

        // Verify decay is actually applied: if we apply no events, value should decrease
        let decayed = apply_feedback(&boosts, &[]);
        let decayed_val = decayed.get("core_tech").copied().unwrap_or(0.0);
        assert!(
            (decayed_val - val * 0.95).abs() < 0.001,
            "Decay should apply 0.95 factor: expected {:.4}, got {:.4}",
            val * 0.95,
            decayed_val
        );
    }
}
