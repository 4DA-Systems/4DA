//! Tests for calibration_commands — extracted for file size limit compliance.

use super::*;

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
// Grade computation
// ------------------------------------------------------------------

#[test]
fn grade_a() {
    let (grade, score) = compute_grade(1.0, 1.0, 1.0);
    assert_eq!(grade, "A");
    assert_eq!(score, 100);
}

#[test]
fn grade_a_boundary() {
    let (grade, score) = compute_grade(1.0, 1.0, 0.5);
    assert_eq!(grade, "A");
    assert_eq!(score, 90);
}

#[test]
fn grade_b_plus() {
    let (grade, score) = compute_grade(0.8, 0.8, 0.8);
    assert_eq!(grade, "B+");
    assert_eq!(score, 80);
}

#[test]
fn grade_b() {
    let (grade, score) = compute_grade(0.8, 0.5, 0.75);
    assert_eq!(grade, "B");
    assert_eq!(score, 70);
}

#[test]
fn grade_c() {
    let (grade, score) = compute_grade(0.6, 0.5, 0.5);
    assert_eq!(grade, "C");
    assert_eq!(score, 55);
}

#[test]
fn grade_d() {
    let (grade, score) = compute_grade(0.4, 0.5, 0.5);
    assert_eq!(grade, "D");
    assert_eq!(score, 45);
}

#[test]
fn grade_f() {
    let (grade, score) = compute_grade(0.0, 0.0, 0.0);
    assert_eq!(grade, "F");
    assert_eq!(score, 0);
}

#[test]
fn grade_separation_clamped() {
    let (g1, s1) = compute_grade(1.0, 5.0, 1.0);
    let (g2, s2) = compute_grade(1.0, 1.0, 1.0);
    assert_eq!(g1, g2);
    assert_eq!(s1, s2);
}

#[test]
fn grade_negative_separation_clamped() {
    let (grade, score) = compute_grade(1.0, -0.5, 1.0);
    assert_eq!(grade, "B");
    assert_eq!(score, 70);
}

// ------------------------------------------------------------------
// Persona display names
// ------------------------------------------------------------------

#[test]
fn persona_names() {
    assert_eq!(persona_display_name("rust_systems"), "Rust / Systems");
    assert_eq!(persona_display_name("python_ml"), "Python / ML");
    assert_eq!(
        persona_display_name("fullstack_ts"),
        "Full-Stack TypeScript"
    );
    assert_eq!(persona_display_name("devops_sre"), "DevOps / SRE");
    assert_eq!(persona_display_name("bootstrap"), "First-Run (Bootstrap)");
    assert_eq!(persona_display_name("unknown_thing"), "unknown_thing");
}

// ------------------------------------------------------------------
// Universal probes
// ------------------------------------------------------------------

#[test]
fn probes_has_6_items() {
    assert_eq!(universal_probes().len(), 6);
}

#[test]
fn probes_3_relevant_3_noise() {
    let probes = universal_probes();
    let relevant = probes.iter().filter(|p| p.expected_relevant).count();
    let noise = probes.iter().filter(|p| !p.expected_relevant).count();
    assert_eq!(relevant, 3);
    assert_eq!(noise, 3);
}

// ------------------------------------------------------------------
// Probe calibration — end-to-end
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

    let (passed, total, _failures) = run_probe_calibration(&ctx, &db);
    assert_eq!(total, 6);
    assert!(passed >= 1);
    assert!(passed <= total);
}

#[test]
fn probe_calibration_empty_context() {
    let db = crate::test_utils::test_db();
    let ctx = crate::test_utils::empty_scoring_context();
    let (passed, total, failures) = run_probe_calibration(&ctx, &db);
    assert_eq!(total, 6);
    assert!(passed + failures.len() as u32 == total);
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
