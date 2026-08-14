// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Enrichment Impact Tests
//!
//! 2 test categories:
//!   a. Per-signal enrichment impact measurement
//!   b. Enriched reality tests (full-fidelity per-persona)
//!
//! The V1-vs-V2 regression and cross-version score-stability tests that used to
//! head this file were deleted 2026-08-12 with the V1 pipeline — there is only
//! one pipeline version left to compare against.

use tracing::info;

use super::corpus::corpus;
use super::enrichment::{EnrichmentConfig, EnrichmentField};
use super::metrics::SimMetrics;
use super::persona_data::all_enrichments;
use super::personas::{all_personas, all_personas_enriched};
use super::{load_corpus_embeddings, sim_db, sim_input, sim_no_freshness};
use super::{ExpectedOutcome, PERSONA_NAMES};

// ============================================================================
// Shared: score a persona through the scoring pipeline
// ============================================================================

fn run_persona_simulation(persona_idx: usize, ctx: &super::super::ScoringContext) -> SimMetrics {
    let items = corpus();
    let db = sim_db();
    let opts = sim_no_freshness();
    let calibrated_embeddings = load_corpus_embeddings();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let mut metrics = SimMetrics::new();

    for item in &items {
        let expected = item.expected[persona_idx];
        if matches!(expected, ExpectedOutcome::MildBorderline) {
            continue;
        }
        let emb = calibrated_embeddings
            .get((item.id - 1) as usize)
            .unwrap_or(&zero_emb);
        let input = sim_input(item.id, item.title, item.content, emb);
        let result = super::super::score_item(&input, ctx, &db, &opts, None);
        metrics.record(&result, expected);
    }
    metrics
}

// ============================================================================
// a. Enrichment impact measurement
// ============================================================================

#[test]
fn enrichment_impact_per_signal() {
    let bases = all_personas();
    let enrichments = all_enrichments();

    // Use Rust persona as the measurement target
    let persona_idx = 0;
    let base_ctx = &bases[persona_idx];
    let base_metrics = run_persona_simulation(persona_idx, base_ctx);
    let base_f1 = base_metrics.f1();

    info!("\n=== ENRICHMENT IMPACT (rust_systems persona) ===");
    info!(
        "Base F1: {base_f1:.3} (P={:.3} R={:.3})",
        base_metrics.precision(),
        base_metrics.recall()
    );
    info!(
        "{:<25} {:>8} {:>8} {:>8} {:>10}",
        "Signal", "P", "R", "F1", "Delta_F1"
    );

    for field in EnrichmentField::all_variants() {
        let config = EnrichmentConfig::only(*field);
        // Rebuild base persona fresh each time (ScoringContext is not Clone)
        let fresh_base = super::personas::rust_systems_dev();
        let enriched = super::enrichment::enrich_persona(fresh_base, &enrichments[0], &config);
        let m = run_persona_simulation(persona_idx, &enriched);
        let delta = m.f1() - base_f1;
        info!(
            "{:<25} {:>8.3} {:>8.3} {:>8.3} {:>+10.3}",
            field.name(),
            m.precision(),
            m.recall(),
            m.f1(),
            delta
        );
    }
}

// ============================================================================
// b. Enriched reality tests — full-fidelity per-persona
// ============================================================================

#[test]
fn enriched_reality_rust_systems() {
    let personas = all_personas_enriched();
    let m = run_persona_simulation(0, &personas[0]);
    info!("{}", m.format_report("enriched_rust_systems"));
    // Enriched thresholds: same or better than base (enrichment should not hurt)
    m.assert_quality("enriched_rust_systems", 0.45, 0.25, 0.35);
}

#[test]
fn enriched_reality_python_ml() {
    let personas = all_personas_enriched();
    let m = run_persona_simulation(1, &personas[1]);
    info!("{}", m.format_report("enriched_python_ml"));
    m.assert_quality("enriched_python_ml", 0.30, 0.15, 0.20);
}

#[test]
fn enriched_reality_fullstack_ts() {
    let personas = all_personas_enriched();
    let m = run_persona_simulation(2, &personas[2]);
    info!("{}", m.format_report("enriched_fullstack_ts"));
    m.assert_quality("enriched_fullstack_ts", 0.35, 0.30, 0.30);
}

#[test]
fn enriched_reality_devops_sre() {
    let personas = all_personas_enriched();
    let m = run_persona_simulation(3, &personas[3]);
    info!("{}", m.format_report("enriched_devops_sre"));
    m.assert_quality("enriched_devops_sre", 0.55, 0.25, 0.35);
}

#[test]
fn enriched_reality_mobile_dev() {
    let personas = all_personas_enriched();
    let m = run_persona_simulation(4, &personas[4]);
    info!("{}", m.format_report("enriched_mobile_dev"));
    m.assert_quality("enriched_mobile_dev", 0.30, 0.20, 0.25);
}

#[test]
fn enriched_reality_bootstrap() {
    let personas = all_personas_enriched();
    let m = run_persona_simulation(5, &personas[5]);
    info!("{}", m.format_report("enriched_bootstrap"));
    // Bootstrap with minimal enrichment — expect similar to base
    m.assert_quality("enriched_bootstrap", 0.08, 0.10, 0.10);
}

#[test]
fn enriched_reality_power_user() {
    let personas = all_personas_enriched();
    let m = run_persona_simulation(6, &personas[6]);
    info!("{}", m.format_report("enriched_power_user"));
    m.assert_quality("enriched_power_user", 0.50, 0.20, 0.30);
}

#[test]
fn enriched_reality_context_switcher() {
    let personas = all_personas_enriched();
    let m = run_persona_simulation(7, &personas[7]);
    info!("{}", m.format_report("enriched_context_switcher"));
    m.assert_quality("enriched_context_switcher", 0.50, 0.20, 0.30);
}

#[test]
fn enriched_reality_niche_specialist() {
    let personas = all_personas_enriched();
    let m = run_persona_simulation(8, &personas[8]);
    info!("{}", m.format_report("enriched_niche_specialist"));
    // v19 (AD-029) recall bar: this persona's enrichment is dominated by
    // BEHAVIORAL data (affinities 0.7-0.9, anti-topics, taste embedding,
    // calibration deltas) — pristine, perfectly-labeled inputs that the
    // production capture layer never actually produced (three incompatible
    // strength scales; the 2026-07-13 doom loop). With behavioral signals
    // demoted from scoring authority, the items only reachable through
    // that enrichment are unreachable BY DESIGN, and measured quality is
    // P=1.000 R=0.143 (every surfaced item correct; weak-relevance recall
    // gone). The recall bar drops to the measured static-only baseline.
    // Restoring it to 0.20+ is an explicit AD-029 re-enable criterion —
    // if learning earns its way back, THIS bar is where the lift must
    // show up first.
    m.assert_quality("enriched_niche_specialist", 0.15, 0.12, 0.18);
}

#[test]
fn enriched_reality_aggregate() {
    let personas = all_personas_enriched();
    let mut aggregate = SimMetrics::new();

    info!("\n=== ENRICHED REALITY AGGREGATE ===");
    for (pi, persona) in personas.iter().enumerate() {
        let m = run_persona_simulation(pi, persona);
        info!(
            "{}",
            m.format_report(&format!("enriched_{}", PERSONA_NAMES[pi]))
        );
        aggregate.merge(&m);
    }

    info!("{}", aggregate.format_report("ENRICHED_AGGREGATE"));
    // Aggregate quality should stay reasonable with enrichment
    assert!(
        aggregate.f1() >= 0.30,
        "Enriched aggregate F1 {:.3} below minimum 0.30",
        aggregate.f1()
    );
    assert!(
        aggregate.precision() >= 0.50,
        "Enriched aggregate precision {:.3} below minimum 0.50",
        aggregate.precision()
    );
}
