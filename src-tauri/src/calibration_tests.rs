//! Tests for calibration_commands — 4-dimension grading, Metrics utilities,
//! and integration with calibration_probes.

use super::*;
use crate::calibration_probes;

// ------------------------------------------------------------------
// Metrics basics
// ------------------------------------------------------------------

#[test]
fn metrics_new_all_zeros() {
    let m = Metrics::new();
    assert_eq!(m.tp, 0);
    assert_eq!(m.fp, 0);
    assert_eq!(m.tn, 0);
    assert_eq!(m.r#fn, 0);
    assert!(m.relevant_scores.is_empty());
    assert!(m.noise_scores.is_empty());
}

#[test]
fn record_strong_relevant_increments_tp() {
    let mut m = Metrics::new();
    m.record(0.8, true, Expected::Strong);
    assert_eq!(m.tp, 1);
    assert_eq!(m.r#fn, 0);
    assert_eq!(m.relevant_scores.len(), 1);
}

#[test]
fn record_strong_not_relevant_increments_fn() {
    let mut m = Metrics::new();
    m.record(0.3, false, Expected::Strong);
    assert_eq!(m.tp, 0);
    assert_eq!(m.r#fn, 1);
}

#[test]
fn record_noise_relevant_increments_fp() {
    let mut m = Metrics::new();
    m.record(0.7, true, Expected::Noise);
    assert_eq!(m.fp, 1);
    assert_eq!(m.tn, 0);
}

#[test]
fn record_noise_not_relevant_increments_tn() {
    let mut m = Metrics::new();
    m.record(0.1, false, Expected::Noise);
    assert_eq!(m.fp, 0);
    assert_eq!(m.tn, 1);
}

#[test]
fn record_borderline_changes_nothing() {
    let mut m = Metrics::new();
    m.record(0.5, true, Expected::Borderline);
    m.record(0.5, false, Expected::Borderline);
    assert_eq!(m.tp, 0);
    assert_eq!(m.fp, 0);
    assert_eq!(m.tn, 0);
    assert_eq!(m.r#fn, 0);
}

// ------------------------------------------------------------------
// Precision / Recall / F1
// ------------------------------------------------------------------

#[test]
fn precision_normal() {
    let mut m = Metrics::new();
    m.tp = 3;
    m.fp = 1;
    assert!((m.precision() - 0.75).abs() < 1e-9);
}

#[test]
fn precision_no_predictions_returns_1() {
    let m = Metrics::new();
    assert!((m.precision() - 1.0).abs() < 1e-9);
}

#[test]
fn recall_normal() {
    let mut m = Metrics::new();
    m.tp = 4;
    m.r#fn = 1;
    assert!((m.recall() - 0.8).abs() < 1e-9);
}

#[test]
fn recall_no_relevant_returns_1() {
    let m = Metrics::new();
    assert!((m.recall() - 1.0).abs() < 1e-9);
}

#[test]
fn f1_normal() {
    let mut m = Metrics::new();
    m.tp = 4;
    m.fp = 1;
    m.r#fn = 1;
    assert!((m.f1() - 0.8).abs() < 1e-9);
}

#[test]
fn f1_zero_when_no_correct() {
    let mut m = Metrics::new();
    m.fp = 5;
    m.r#fn = 5;
    assert!((m.f1() - 0.0).abs() < 1e-9);
}

// ------------------------------------------------------------------
// Separation gap
// ------------------------------------------------------------------

#[test]
fn separation_gap_normal() {
    let mut m = Metrics::new();
    m.relevant_scores = vec![0.8, 0.9, 0.7];
    m.noise_scores = vec![0.2, 0.3, 0.1];
    assert!((m.separation_gap() - 0.6).abs() < 1e-9);
}

#[test]
fn separation_gap_both_empty() {
    let m = Metrics::new();
    assert!((m.separation_gap() - 0.0).abs() < 1e-9);
}

#[test]
fn separation_gap_negative_when_noise_higher() {
    let mut m = Metrics::new();
    m.relevant_scores = vec![0.2];
    m.noise_scores = vec![0.8];
    assert!((m.separation_gap() - (-0.6)).abs() < 1e-9);
}

// ------------------------------------------------------------------
// Merge
// ------------------------------------------------------------------

#[test]
fn merge_combines_counters() {
    let mut a = Metrics::new();
    a.tp = 3;
    a.fp = 1;
    a.tn = 5;
    a.r#fn = 2;
    a.relevant_scores = vec![0.9, 0.8];
    a.noise_scores = vec![0.1];

    let mut b = Metrics::new();
    b.tp = 2;
    b.fp = 3;
    b.tn = 1;
    b.r#fn = 4;
    b.relevant_scores = vec![0.7];
    b.noise_scores = vec![0.2, 0.3];

    a.merge(&b);
    assert_eq!(a.tp, 5);
    assert_eq!(a.fp, 4);
    assert_eq!(a.tn, 6);
    assert_eq!(a.r#fn, 6);
    assert_eq!(a.relevant_scores.len(), 3);
    assert_eq!(a.noise_scores.len(), 3);
}

// ------------------------------------------------------------------
// 4-Dimension Grade Computation
// ------------------------------------------------------------------

#[test]
fn grade_from_dimensions_perfect() {
    let (grade, score) = calibration_probes::compute_grade_from_dimensions(25, 25, 25, 25);
    assert_eq!(grade, "A");
    assert_eq!(score, 100);
}

#[test]
fn grade_from_dimensions_zero() {
    let (grade, score) = calibration_probes::compute_grade_from_dimensions(0, 0, 0, 0);
    assert_eq!(grade, "F");
    assert_eq!(score, 0);
}

#[test]
fn grade_from_dimensions_zero_setup_with_keyword_disc() {
    // No infra, no context, no signal, small discrimination from keywords
    let (grade, score) = calibration_probes::compute_grade_from_dimensions(0, 0, 0, 5);
    assert_eq!(grade, "F");
    assert_eq!(score, 5);
}

#[test]
fn grade_from_dimensions_moderate_setup() {
    // Ollama+embed(20) + 3 interests(7) + 2 axes(10) + ok disc(12)
    let (grade, score) = calibration_probes::compute_grade_from_dimensions(20, 7, 10, 12);
    assert_eq!(score, 49);
    assert_eq!(grade, "D");
}

#[test]
fn grade_from_dimensions_good_setup() {
    // Full infra(25) + rich context(20) + 4 axes(20) + good disc(18)
    let (grade, score) = calibration_probes::compute_grade_from_dimensions(25, 20, 20, 18);
    assert_eq!(score, 83);
    assert_eq!(grade, "B+");
}

#[test]
fn grade_from_dimensions_clamped_at_100() {
    let (_, score) = calibration_probes::compute_grade_from_dimensions(25, 25, 25, 25);
    assert!(score <= 100);
}

// ------------------------------------------------------------------
// Infrastructure score
// ------------------------------------------------------------------

#[test]
fn infra_score_nothing() {
    let rig = RigRequirements {
        ollama_running: false,
        ollama_url: String::new(),
        embedding_model: None,
        embedding_available: false,
        gpu_detected: false,
        recommended_model: String::new(),
        estimated_ram_gb: 0.0,
        can_reach_grade_a: false,
        grade_a_requirements: vec![],
    };
    assert_eq!(calibration_probes::compute_infrastructure_score(&rig), 0);
}

#[test]
fn infra_score_ollama_only() {
    let rig = RigRequirements {
        ollama_running: true,
        ollama_url: String::new(),
        embedding_model: None,
        embedding_available: false,
        gpu_detected: false,
        recommended_model: String::new(),
        estimated_ram_gb: 0.0,
        can_reach_grade_a: false,
        grade_a_requirements: vec![],
    };
    assert_eq!(calibration_probes::compute_infrastructure_score(&rig), 8);
}

#[test]
fn infra_score_full() {
    let rig = RigRequirements {
        ollama_running: true,
        ollama_url: String::new(),
        embedding_model: Some("nomic".into()),
        embedding_available: true,
        gpu_detected: true,
        recommended_model: String::new(),
        estimated_ram_gb: 0.0,
        can_reach_grade_a: true,
        grade_a_requirements: vec![],
    };
    assert_eq!(calibration_probes::compute_infrastructure_score(&rig), 25);
}

// ------------------------------------------------------------------
// Context richness score
// ------------------------------------------------------------------

#[test]
fn context_score_empty() {
    let ctx = crate::scoring::ScoringContext::builder().build();
    assert_eq!(calibration_probes::compute_context_score(&ctx), 0);
}

#[test]
fn context_score_3_interests() {
    let ctx = crate::scoring::ScoringContext::builder()
        .interest_count(3)
        .build();
    // 3 * 2.5 = 7.5 → 7
    assert_eq!(calibration_probes::compute_context_score(&ctx), 7);
}

#[test]
fn context_score_5_interests_capped() {
    let ctx = crate::scoring::ScoringContext::builder()
        .interest_count(10) // capped at 5 → 12.5
        .build();
    // 5 * 2.5 = 12.5 → 12
    assert_eq!(calibration_probes::compute_context_score(&ctx), 12);
}

// ------------------------------------------------------------------
// Signal coverage score
// ------------------------------------------------------------------

#[test]
fn signal_score_zero_axes() {
    let audit = calibration_probes::SignalAudit {
        axes: vec![],
        context_fires: false,
        interest_fires: false,
        ace_fires: false,
        learned_fires: false,
        dependency_fires: false,
    };
    assert_eq!(calibration_probes::compute_signal_score(&audit), 0);
}

#[test]
fn signal_score_all_axes() {
    let audit = calibration_probes::SignalAudit {
        axes: vec![
            "context".into(),
            "interest".into(),
            "ace".into(),
            "learned".into(),
            "dependency".into(),
        ],
        context_fires: true,
        interest_fires: true,
        ace_fires: true,
        learned_fires: true,
        dependency_fires: true,
    };
    assert_eq!(calibration_probes::compute_signal_score(&audit), 25);
}

#[test]
fn signal_score_two_axes() {
    let audit = calibration_probes::SignalAudit {
        axes: vec!["interest".into(), "ace".into()],
        context_fires: false,
        interest_fires: true,
        ace_fires: true,
        learned_fires: false,
        dependency_fires: false,
    };
    assert_eq!(calibration_probes::compute_signal_score(&audit), 10);
}

// ------------------------------------------------------------------
// Discrimination score
// ------------------------------------------------------------------

#[test]
fn disc_score_perfect() {
    let probes = calibration_probes::ProbeResults {
        f1: 1.0,
        precision: 1.0,
        recall: 1.0,
        separation_gap: 1.0,
        passed: 12,
        total: 12,
        failures: vec![],
    };
    assert_eq!(
        calibration_probes::compute_discrimination_score(&probes),
        25
    );
}

#[test]
fn disc_score_zero() {
    let probes = calibration_probes::ProbeResults {
        f1: 0.0,
        precision: 0.0,
        recall: 0.0,
        separation_gap: 0.0,
        passed: 0,
        total: 12,
        failures: vec![],
    };
    assert_eq!(calibration_probes::compute_discrimination_score(&probes), 0);
}

// ------------------------------------------------------------------
// Probe calibration — end-to-end via calibration_probes
// ------------------------------------------------------------------

#[test]
fn probe_calibration_returns_sensible_counts() {
    let db = crate::test_utils::test_db();
    let ctx = crate::scoring::ScoringContext::builder()
        .interest_count(3)
        .interests(vec![
            crate::context_engine::Interest {
                id: Some(1),
                topic: "Rust".to_string(),
                weight: 1.0,
                embedding: Some(vec![0.0_f32; 384]),
                source: crate::context_engine::InterestSource::Explicit,
            },
            crate::context_engine::Interest {
                id: Some(2),
                topic: "systems programming".to_string(),
                weight: 1.0,
                embedding: Some(vec![0.0_f32; 384]),
                source: crate::context_engine::InterestSource::Explicit,
            },
            crate::context_engine::Interest {
                id: Some(3),
                topic: "software development".to_string(),
                weight: 0.8,
                embedding: Some(vec![0.0_f32; 384]),
                source: crate::context_engine::InterestSource::Explicit,
            },
        ])
        .declared_tech(vec!["rust".to_string(), "tauri".to_string()])
        .feedback_interaction_count(50)
        .build();

    let results = calibration_probes::run_probe_calibration(&ctx, &db);
    assert_eq!(results.total, 12); // 12 probes selected
    assert!(results.passed >= 1);
    assert!(results.passed <= results.total);
}

#[test]
fn probe_calibration_empty_context() {
    let db = crate::test_utils::test_db();
    let ctx = crate::test_utils::empty_scoring_context();
    let results = calibration_probes::run_probe_calibration(&ctx, &db);
    assert_eq!(results.total, 12);
    assert!(results.passed + results.failures.len() as u32 == results.total);
}

// ------------------------------------------------------------------
// Action type on recommendations
// ------------------------------------------------------------------

#[test]
fn recommendation_has_action_type_field() {
    let rec = Recommendation {
        priority: "P0".into(),
        title: "test".into(),
        description: "test".into(),
        action: None,
        action_type: Some("pull_embedding_model".into()),
    };
    assert_eq!(rec.action_type.as_deref(), Some("pull_embedding_model"));
}

#[test]
fn recommendation_action_type_none() {
    let rec = Recommendation {
        priority: "P2".into(),
        title: "test".into(),
        description: "test".into(),
        action: None,
        action_type: None,
    };
    assert!(rec.action_type.is_none());
}

// ------------------------------------------------------------------
// Integration: full workflow
// ------------------------------------------------------------------

#[test]
fn metrics_full_workflow() {
    let mut m = Metrics::new();
    m.record(0.9, true, Expected::Strong);
    m.record(0.8, true, Expected::Weak);
    m.record(0.3, false, Expected::Strong);
    m.record(0.1, false, Expected::Noise);
    m.record(0.6, true, Expected::Noise);
    m.record(0.5, true, Expected::Borderline);

    assert_eq!(m.tp, 2);
    assert_eq!(m.r#fn, 1);
    assert_eq!(m.tn, 1);
    assert_eq!(m.fp, 1);
    assert!((m.precision() - 2.0 / 3.0).abs() < 1e-9);
    assert!((m.recall() - 2.0 / 3.0).abs() < 1e-9);
    assert!((m.f1() - 2.0 / 3.0).abs() < 1e-9);
}
