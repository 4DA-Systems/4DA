//! V1 vs V2 Pipeline Comparison & Enrichment Impact Tests
//!
//! 4 test categories:
//!   a. V1 vs V2 regression detection across all personas
//!   b. Per-signal enrichment impact measurement
//!   c. Enriched reality tests (full-fidelity per-persona)
//!   d. Cross-version score stability

use tracing::{debug, info};

use super::corpus::corpus;
use super::enrichment::{EnrichmentConfig, EnrichmentField};
use super::metrics::SimMetrics;
use super::persona_data::all_enrichments;
use super::personas::{all_personas, all_personas_enriched};
use super::version_registry::{compare_versions, score_with_version, PipelineVersion};
use super::{load_corpus_embeddings, sim_db, sim_input, sim_no_freshness};
use super::{ExpectedOutcome, PERSONA_NAMES};

// ============================================================================
// Shared: score a persona through a specific pipeline version
// ============================================================================

fn run_persona_versioned(
    persona_idx: usize,
    ctx: &super::super::ScoringContext,
    version: PipelineVersion,
) -> SimMetrics {
    let items = corpus();
    let db = sim_db();
    let opts = sim_no_freshness();
    let calibrated_embeddings = load_corpus_embeddings();
    let zero_emb = vec![0.0_f32; 384];
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
        let result = score_with_version(version, &input, ctx, &db, &opts);
        metrics.record(&result, expected);
    }
    metrics
}

fn run_persona_simulation(persona_idx: usize, ctx: &super::super::ScoringContext) -> SimMetrics {
    run_persona_versioned(persona_idx, ctx, PipelineVersion::V2)
}

// ============================================================================
// 4a. V1 vs V2 regression detection
// ============================================================================

#[test]
fn version_comparison_all_personas() {
    let personas = all_personas();

    info!("\n=== V1 vs V2 COMPARISON (base personas) ===");
    info!(
        "{:<20} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "Persona", "V1_P", "V1_R", "V1_F1", "V2_P", "V2_R", "V2_F1"
    );

    let mut v1_f1_sum = 0.0_f64;
    let mut v2_f1_sum = 0.0_f64;

    for (pi, persona) in personas.iter().enumerate() {
        let v1 = run_persona_versioned(pi, persona, PipelineVersion::V1);
        let v2 = run_persona_versioned(pi, persona, PipelineVersion::V2);

        info!(
            "{:<20} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>8.3}",
            PERSONA_NAMES[pi],
            v1.precision(),
            v1.recall(),
            v1.f1(),
            v2.precision(),
            v2.recall(),
            v2.f1()
        );

        v1_f1_sum += v1.f1();
        v2_f1_sum += v2.f1();
    }

    let v1_avg = v1_f1_sum / 9.0;
    let v2_avg = v2_f1_sum / 9.0;
    info!("\nAverage F1: V1={v1_avg:.3} V2={v2_avg:.3}");

    // V2 must not regress more than 5% vs V1 on average
    assert!(
        v2_avg >= v1_avg * 0.95,
        "V2 avg F1 ({v2_avg:.3}) regressed more than 5% vs V1 ({v1_avg:.3})"
    );
}

// ============================================================================
// 4b. Enrichment impact measurement
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
// 4c. Enriched reality tests — full-fidelity per-persona
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
    // Enriched niche has aggressive anti-topics (javascript, python, java) and
    // exclusions (javascript, web development, career) that broadly match many
    // corpus items. This causes higher FP rate in the exclusion-counting logic.
    // Thresholds relaxed to reflect the anti-topic/exclusion interaction.
    m.assert_quality("enriched_niche_specialist", 0.15, 0.20, 0.18);
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

// ============================================================================
// 4d. Cross-version score stability
// ============================================================================

#[test]
fn version_score_stability() {
    let personas = all_personas();
    let items = corpus();
    let db = sim_db();
    let opts = sim_no_freshness();
    let calibrated_embeddings = load_corpus_embeddings();
    let zero_emb = vec![0.0_f32; 384];

    // Sample 20 items spread across the corpus
    let step = items.len().max(1) / 20;
    let mut large_divergences = 0u32;
    let mut total_checked = 0u32;

    for (idx, item) in items.iter().enumerate() {
        if step > 0 && idx % step != 0 {
            continue;
        }
        let emb = calibrated_embeddings
            .get((item.id - 1) as usize)
            .unwrap_or(&zero_emb);
        let input = sim_input(item.id, item.title, item.content, emb);
        let cmp = compare_versions(&input, &personas[0], &db, &opts);
        total_checked += 1;

        if cmp.score_delta.abs() > 0.30 {
            large_divergences += 1;
        }
    }

    info!("[version_stability] checked={total_checked} large_divergences={large_divergences}");

    // Allow some divergence but not catastrophic
    let divergence_rate = if total_checked > 0 {
        large_divergences as f64 / total_checked as f64
    } else {
        0.0
    };
    assert!(
        divergence_rate <= 0.40,
        "Too many items diverge >0.30 between V1/V2: {large_divergences}/{total_checked} ({divergence_rate:.2})"
    );
}
