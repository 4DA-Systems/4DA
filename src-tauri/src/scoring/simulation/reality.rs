//! System 2: Content Reality Testing
//!
//! Validates that each persona correctly scores the corpus:
//! Thresholds calibrated to current pipeline behavior (regression baseline).
//! Personas with narrow interests may have low recall — this is expected.

use super::super::{score_item, ScoringContext};
use super::corpus::corpus;
use super::metrics::SimMetrics;
use super::personas::all_personas;
use super::{sim_db, sim_input, sim_no_freshness};
use super::{ExpectedOutcome, PERSONA_NAMES};

// ============================================================================
// Shared runner
// ============================================================================

fn run_persona_simulation(persona_idx: usize, ctx: &ScoringContext) -> SimMetrics {
    let items = corpus();
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let mut metrics = SimMetrics::new();

    for item in &items {
        let expected = item.expected[persona_idx];
        // Skip borderline items — they're intentionally ambiguous
        if matches!(expected, ExpectedOutcome::MildBorderline) {
            continue;
        }
        let input = sim_input(item.id, item.title, item.content, &emb);
        let result = score_item(&input, ctx, &db, &opts, None);
        metrics.record(&result, expected);
    }
    metrics
}

// ============================================================================
// Per-persona reality tests
// ============================================================================

#[test]
fn reality_rust_systems_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(0, &personas[0]);
    println!("{}", m.format_report(PERSONA_NAMES[0]));
    m.assert_quality(PERSONA_NAMES[0], 0.55, 0.30, 0.40);
}

#[test]
fn reality_python_ml_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(1, &personas[1]);
    println!("{}", m.format_report(PERSONA_NAMES[1]));
    m.assert_quality(PERSONA_NAMES[1], 0.35, 0.20, 0.25);
}

#[test]
fn reality_fullstack_ts_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(2, &personas[2]);
    println!("{}", m.format_report(PERSONA_NAMES[2]));
    m.assert_quality(PERSONA_NAMES[2], 0.45, 0.40, 0.40);
}

#[test]
fn reality_devops_sre_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(3, &personas[3]);
    println!("{}", m.format_report(PERSONA_NAMES[3]));
    m.assert_quality(PERSONA_NAMES[3], 0.60, 0.10, 0.15);
}

#[test]
fn reality_mobile_dev_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(4, &personas[4]);
    println!("{}", m.format_report(PERSONA_NAMES[4]));
    m.assert_quality(PERSONA_NAMES[4], 0.45, 0.30, 0.35);
}

#[test]
fn reality_bootstrap_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(5, &personas[5]);
    println!("{}", m.format_report(PERSONA_NAMES[5]));
    m.assert_quality(PERSONA_NAMES[5], 0.20, 0.20, 0.20);
}

#[test]
fn reality_power_user_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(6, &personas[6]);
    println!("{}", m.format_report(PERSONA_NAMES[6]));
    // Generalist persona — low recall expected; validate no excessive false positives
    assert!(
        m.precision() >= 0.30 || m.tp + m.fp == 0,
        "Power user precision too low: {:.3}",
        m.precision()
    );
}

#[test]
fn reality_context_switcher_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(7, &personas[7]);
    println!("{}", m.format_report(PERSONA_NAMES[7]));
    // Generalist persona — low recall expected; validate no excessive false positives
    assert!(
        m.precision() >= 0.30 || m.tp + m.fp == 0,
        "Context switcher precision too low: {:.3}",
        m.precision()
    );
}

#[test]
fn reality_niche_specialist_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(8, &personas[8]);
    println!("{}", m.format_report(PERSONA_NAMES[8]));
    // Narrow focus — precision matters more than recall
    assert!(
        m.precision() >= 0.50 || m.tp + m.fp == 0,
        "Niche specialist precision too low: {:.3}",
        m.precision()
    );
}

// ============================================================================
// Cross-persona isolation
// ============================================================================

#[test]
fn reality_rust_persona_does_not_score_python_content() {
    use super::corpus::corpus;
    use super::ContentCategory;

    let personas = all_personas();
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    // Find items that are StrongRelevant for Python but NotRelevant for Rust
    let items = corpus();
    let mut fp_count = 0u32;
    let mut total = 0u32;

    for item in &items {
        if item.category == ContentCategory::CrossDomainNoise
            && item.expected[1] == ExpectedOutcome::StrongRelevant
            && item.expected[0] == ExpectedOutcome::NotRelevant
        {
            let input = sim_input(item.id, item.title, item.content, &emb);
            let result = score_item(&input, &personas[0], &db, &opts, None);
            total += 1;
            if result.relevant {
                fp_count += 1;
            }
        }
    }

    if total > 0 {
        let fp_rate = fp_count as f64 / total as f64;
        assert!(fp_rate <= 0.30,
            "Rust persona scores too much Python-only content: {fp_count}/{total} FP ({fp_rate:.2})");
    }
}

#[test]
fn reality_noise_rejection_all_personas() {
    use super::ContentCategory;
    let personas = all_personas();
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let items = corpus();

    let noise_items: Vec<_> = items
        .iter()
        .filter(|i| {
            matches!(
                i.category,
                ContentCategory::CareerNoise
                    | ContentCategory::ShowHNNoise
                    | ContentCategory::MetaNoise
            )
        })
        .collect();

    for (pi, persona) in personas.iter().enumerate() {
        let mut noise_scored_relevant = 0u32;
        for item in &noise_items {
            let expected = item.expected[pi];
            if expected != ExpectedOutcome::NotRelevant {
                continue;
            }
            let input = sim_input(item.id, item.title, item.content, &emb);
            let result = score_item(&input, persona, &db, &opts, None);
            if result.relevant {
                noise_scored_relevant += 1;
            }
        }
        let noise_count = noise_items
            .iter()
            .filter(|i| i.expected[pi] == ExpectedOutcome::NotRelevant)
            .count();
        if noise_count > 0 {
            let fp_rate = noise_scored_relevant as f64 / noise_count as f64;
            assert!(fp_rate <= 0.20,
                "Persona {} has {fp_rate:.2} false-positive rate on noise ({noise_scored_relevant}/{noise_count})",
                PERSONA_NAMES[pi]);
        }
    }
}

#[test]
fn reality_score_distribution_separation() {
    let personas = all_personas();
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];
    let items = corpus();

    // For Rust persona: relevant scores should be higher than noise scores
    let mut relevant_scores = Vec::new();
    let mut noise_scores = Vec::new();

    for item in &items {
        let e = item.expected[0];
        let input = sim_input(item.id, item.title, item.content, &emb);
        let result = score_item(&input, &personas[0], &db, &opts, None);
        match e {
            ExpectedOutcome::StrongRelevant => relevant_scores.push(result.top_score as f64),
            ExpectedOutcome::NotRelevant => noise_scores.push(result.top_score as f64),
            _ => {}
        }
    }

    let mean_rel = relevant_scores.iter().sum::<f64>() / relevant_scores.len().max(1) as f64;
    let mean_noise = noise_scores.iter().sum::<f64>() / noise_scores.len().max(1) as f64;

    assert!(
        mean_rel > mean_noise,
        "Rust relevant mean ({mean_rel:.3}) should exceed noise mean ({mean_noise:.3})"
    );
    assert!(
        mean_rel - mean_noise >= 0.05,
        "Separation gap ({:.3}) too small",
        mean_rel - mean_noise
    );
}

#[test]
fn reality_aggregate_summary() {
    let personas = all_personas();
    let mut aggregate = SimMetrics::new();

    for (pi, persona) in personas.iter().enumerate() {
        let m = run_persona_simulation(pi, persona);
        println!("{}", m.format_report(PERSONA_NAMES[pi]));
        aggregate.merge(&m);
    }

    println!("{}", aggregate.format_report("AGGREGATE"));
    // Aggregate quality — calibrated to current pipeline
    assert!(
        aggregate.f1() >= 0.15,
        "Aggregate F1 {:.3} below minimum 0.15",
        aggregate.f1()
    );
    assert!(
        aggregate.precision() >= 0.45,
        "Aggregate precision {:.3} below minimum 0.45",
        aggregate.precision()
    );
}
