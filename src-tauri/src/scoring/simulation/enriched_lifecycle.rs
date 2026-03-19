//! Enriched Lifecycle Tests — full-fidelity multi-session convergence.
//!
//! Re-runs lifecycle convergence tests with fully enriched personas to validate
//! that full-fidelity context (anti_topics, source_quality, topic_confidence,
//! etc.) doesn't destabilize the feedback loop.

use std::collections::HashMap;

use tracing::debug;

use super::super::score_item;
use super::enrichment::EnrichmentConfig;
use super::feedback_sim::{apply_feedback, lifecycle_corpus, simulate_session_with_embeddings};
use super::metrics::SimMetrics;
use super::persona_data::all_enrichments;
use super::personas;
use super::ExpectedOutcome;
use super::{sim_db, sim_input, sim_no_freshness};

const N_SESSIONS: usize = 20;

// ============================================================================
// Enriched context factories
// ============================================================================

/// Build an enriched Rust persona context with feedback boosts applied.
fn enriched_rust_ctx(
    boosts: &HashMap<String, f64>,
    interaction_count: i64,
) -> super::super::ScoringContext {
    // Start from the base persona builder
    let base = personas::rust_systems_dev();
    let enrichments = all_enrichments();
    let config = EnrichmentConfig::all();
    let mut enriched = super::enrichment::enrich_persona(base, &enrichments[0], &config);

    // Apply feedback boosts on top of enrichment
    enriched.feedback_boosts = boosts.iter().map(|(k, v)| (k.clone(), *v)).collect();
    enriched.feedback_interaction_count = interaction_count;

    // Merge feedback-derived affinities into ACE context
    for (topic, &boost) in boosts {
        if boost.abs() > 0.05 {
            let affinity = boost.clamp(-1.0, 1.0) as f32;
            let confidence = (boost.abs() as f32).min(0.9);
            enriched
                .ace_ctx
                .topic_affinities
                .entry(topic.clone())
                .or_insert((affinity, confidence));
        }
    }

    enriched
}

/// Build an enriched Python persona context with feedback boosts applied.
fn enriched_python_ctx(
    boosts: &HashMap<String, f64>,
    interaction_count: i64,
) -> super::super::ScoringContext {
    let base = personas::python_ml_engineer();
    let enrichments = all_enrichments();
    let config = EnrichmentConfig::all();
    let mut enriched = super::enrichment::enrich_persona(base, &enrichments[1], &config);

    enriched.feedback_boosts = boosts.iter().map(|(k, v)| (k.clone(), *v)).collect();
    enriched.feedback_interaction_count = interaction_count;

    for (topic, &boost) in boosts {
        if boost.abs() > 0.05 {
            let affinity = boost.clamp(-1.0, 1.0) as f32;
            let confidence = (boost.abs() as f32).min(0.9);
            enriched
                .ace_ctx
                .topic_affinities
                .entry(topic.clone())
                .or_insert((affinity, confidence));
        }
    }

    enriched
}

// ============================================================================
// Session runners
// ============================================================================

fn run_enriched_rust_sessions(n: usize) -> Vec<f64> {
    let items = lifecycle_corpus();
    let calibrated_embeddings = super::load_corpus_embeddings();
    let zero_emb = vec![0.0_f32; 384];
    let mut boosts: HashMap<String, f64> = HashMap::new();
    let mut f1_per_session = Vec::new();

    for session_idx in 0..n {
        let interaction_count = (session_idx as i64 + 1) * 10;
        let ctx = enriched_rust_ctx(&boosts, interaction_count);

        let db = sim_db();
        let opts = sim_no_freshness();
        let mut metrics = SimMetrics::new();

        for item in &items {
            let expected = item.expected[0];
            if matches!(expected, ExpectedOutcome::MildBorderline) {
                continue;
            }
            let emb = calibrated_embeddings
                .get((item.id - 1) as usize)
                .unwrap_or(&zero_emb);
            let input = sim_input(item.id, item.title, item.content, emb);
            let result = score_item(&input, &ctx, &db, &opts, None);
            metrics.record(&result, expected);
        }
        f1_per_session.push(metrics.f1());

        let events = simulate_session_with_embeddings(&ctx, &items, 0, &calibrated_embeddings);
        boosts = apply_feedback(&boosts, &events);
    }
    f1_per_session
}

fn run_enriched_python_sessions(n: usize) -> Vec<f64> {
    let items = lifecycle_corpus();
    let calibrated_embeddings = super::load_corpus_embeddings();
    let zero_emb = vec![0.0_f32; 384];
    let mut boosts: HashMap<String, f64> = HashMap::new();
    let mut f1_per_session = Vec::new();

    for session_idx in 0..n {
        let interaction_count = (session_idx as i64 + 1) * 10;
        let ctx = enriched_python_ctx(&boosts, interaction_count);

        let db = sim_db();
        let opts = sim_no_freshness();
        let mut metrics = SimMetrics::new();

        for item in &items {
            let expected = item.expected[1];
            if matches!(expected, ExpectedOutcome::MildBorderline) {
                continue;
            }
            let emb = calibrated_embeddings
                .get((item.id - 1) as usize)
                .unwrap_or(&zero_emb);
            let input = sim_input(item.id, item.title, item.content, emb);
            let result = score_item(&input, &ctx, &db, &opts, None);
            metrics.record(&result, expected);
        }
        f1_per_session.push(metrics.f1());

        let events = simulate_session_with_embeddings(&ctx, &items, 1, &calibrated_embeddings);
        boosts = apply_feedback(&boosts, &events);
    }
    f1_per_session
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn enriched_lifecycle_rust_does_not_degrade() {
    let f1s = run_enriched_rust_sessions(N_SESSIONS);
    assert!(!f1s.is_empty(), "No sessions ran");

    let first = f1s[0];
    let last = f1s[f1s.len() - 1];

    debug!("[enriched_lifecycle_rust] first_f1={first:.3} last_f1={last:.3}");

    // Last session F1 must be at least 85% of first session F1
    let min_acceptable = first * 0.85;
    assert!(
        last >= min_acceptable,
        "Enriched Rust persona degraded over {N_SESSIONS} sessions: first={first:.3} last={last:.3} min={min_acceptable:.3}"
    );
}

#[test]
fn enriched_lifecycle_python_does_not_degrade() {
    let f1s = run_enriched_python_sessions(N_SESSIONS);
    assert!(!f1s.is_empty(), "No sessions ran");

    let first = f1s[0];
    let last = f1s[f1s.len() - 1];

    debug!("[enriched_lifecycle_python] first_f1={first:.3} last_f1={last:.3}");

    let min_acceptable = first * 0.85;
    assert!(
        last >= min_acceptable,
        "Enriched Python persona degraded over {N_SESSIONS} sessions: first={first:.3} last={last:.3} min={min_acceptable:.3}"
    );
}

#[test]
fn enriched_lifecycle_cross_persona_isolation() {
    // Run enriched Rust lifecycle, verify Python content stays low.
    // Anti-topics should IMPROVE isolation vs base lifecycle tests.
    let items = lifecycle_corpus();
    let calibrated_embeddings = super::load_corpus_embeddings();
    let zero_emb = vec![0.0_f32; 384];
    let mut boosts: HashMap<String, f64> = HashMap::new();

    for session_idx in 0..10 {
        let interaction_count = (session_idx as i64 + 1) * 10;
        let ctx = enriched_rust_ctx(&boosts, interaction_count);
        let events = simulate_session_with_embeddings(&ctx, &items, 0, &calibrated_embeddings);
        boosts = apply_feedback(&boosts, &events);
    }

    let final_ctx = enriched_rust_ctx(&boosts, 100);
    let db = sim_db();
    let opts = sim_no_freshness();

    let mut python_false_positives = 0u32;
    let mut python_total = 0u32;

    for item in &items {
        if item.expected[0] != ExpectedOutcome::NotRelevant {
            continue;
        }
        if item.expected[1] != ExpectedOutcome::StrongRelevant {
            continue;
        }
        let emb = calibrated_embeddings
            .get((item.id - 1) as usize)
            .unwrap_or(&zero_emb);
        let input = sim_input(item.id, item.title, item.content, emb);
        let result = score_item(&input, &final_ctx, &db, &opts, None);
        python_total += 1;
        if result.relevant {
            python_false_positives += 1;
        }
    }

    if python_total > 0 {
        let fp_rate = python_false_positives as f64 / python_total as f64;
        debug!(
            "[enriched_cross_isolation] python_fp={python_false_positives}/{python_total} rate={fp_rate:.2}"
        );
        // Enriched context with anti-topics should maintain or improve isolation
        assert!(
            fp_rate <= 0.40,
            "Enriched cross-persona isolation failed: {python_false_positives}/{python_total} ({fp_rate:.2})"
        );
    }
}

#[test]
fn enriched_lifecycle_noise_stays_rejected() {
    // After all enriched sessions, noise items should still be mostly rejected.
    // Source quality signals should help noise rejection.
    let items = lifecycle_corpus();
    let calibrated_embeddings = super::load_corpus_embeddings();
    let zero_emb = vec![0.0_f32; 384];
    let db = sim_db();
    let opts = sim_no_freshness();
    let mut boosts: HashMap<String, f64> = HashMap::new();

    for session_idx in 0..N_SESSIONS {
        let ctx = enriched_rust_ctx(&boosts, (session_idx as i64 + 1) * 10);
        let events = simulate_session_with_embeddings(&ctx, &items, 0, &calibrated_embeddings);
        boosts = apply_feedback(&boosts, &events);
    }

    let final_ctx = enriched_rust_ctx(&boosts, (N_SESSIONS as i64) * 10);
    let mut noise_relevant = 0u32;
    let mut noise_total = 0u32;

    for item in &items {
        if item.expected[0] != ExpectedOutcome::NotRelevant {
            continue;
        }
        let emb = calibrated_embeddings
            .get((item.id - 1) as usize)
            .unwrap_or(&zero_emb);
        let input = sim_input(item.id, item.title, item.content, emb);
        let result = score_item(&input, &final_ctx, &db, &opts, None);
        noise_total += 1;
        if result.relevant {
            noise_relevant += 1;
        }
    }

    if noise_total > 0 {
        let noise_fp_rate = noise_relevant as f64 / noise_total as f64;
        debug!(
            "[enriched_noise_rejection] noise_fp={noise_relevant}/{noise_total} rate={noise_fp_rate:.2}"
        );
        assert!(
            noise_fp_rate <= 0.35,
            "Enriched noise rejection degraded: {noise_relevant}/{noise_total} ({noise_fp_rate:.2})"
        );
    }
}
