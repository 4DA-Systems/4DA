// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! System 2: Content Reality Testing
//!
//! Validates that each persona correctly scores the corpus:
//! Thresholds calibrated to current pipeline behavior (regression baseline).
//! Personas with narrow interests may have low recall — this is expected.

use tracing::info;

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
    let calibrated_embeddings = super::load_corpus_embeddings();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];
    let mut metrics = SimMetrics::new();

    for item in &items {
        let expected = item.expected[persona_idx];
        // Skip borderline items — they're intentionally ambiguous
        if matches!(expected, ExpectedOutcome::MildBorderline) {
            continue;
        }
        let emb = calibrated_embeddings
            .get((item.id - 1) as usize)
            .unwrap_or(&zero_emb);
        let input = sim_input(item.id, item.title, item.content, emb);
        let result = score_item(&input, ctx, &db, &opts, None);
        metrics.record(&result, expected);
    }
    metrics
}

// ============================================================================
// Per-persona reality tests
// ============================================================================

/// (P, R, F1) floors for the active embedding mode.
///
/// The SYNTHETIC floors (first arg) are the block-signature regression
/// baseline the default suite asserts, ratcheted 2026-08-24 to the measured
/// post-audit-fix state wherever it improved (never lowered — a floor whose
/// measured−buffer landed below the old floor keeps the old floor).
///
/// The CALIBRATED floors (second arg) were first pinned 2026-08-24 from the
/// measured post-audit-fix state (`--features calibrated-sim`, REAL fastembed
/// fixtures, at 62ffbacf + Wave 1-2 fixes). Until then the synthetic floors
/// were applied to real-embedding runs, which failed 4 generalist personas —
/// and CI never ran that mode at all, so the failures were invisible
/// (2026-08-23 adversarial audit §2c / item 21). Convention: each floor sits
/// at the measured value minus a buffer of P −0.10, R −0.04, F1 −0.05
/// (matching the historical slack in this file), rounded down to 2dp.
///
/// RATCHET (quality_gate.rs doctrine): when a measured value improves, RAISE
/// the matching floor in the same PR. Where a calibrated floor sits BELOW its
/// synthetic sibling (generalist recall), that is not a lowered guard but the
/// first honest pin of real-embedding recall at its measured level — raising
/// the measured level is the audit's Phase-2+ recall arc.
const fn mode_floors(synthetic: (f64, f64, f64), calibrated: (f64, f64, f64)) -> (f64, f64, f64) {
    if cfg!(feature = "calibrated-sim") {
        calibrated
    } else {
        synthetic
    }
}

#[test]
fn reality_rust_systems_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(0, &personas[0]);
    info!("{}", m.format_report(PERSONA_NAMES[0]));
    // Measured 2026-08-24 — synthetic: P=0.926 R=0.625 F1=0.746 (floors
    // ratcheted UP from 0.55/0.30/0.40); calibrated: P=0.955 R=0.525 F1=0.677.
    let (p, r, f) = mode_floors((0.82, 0.58, 0.69), (0.85, 0.48, 0.62));
    m.assert_quality(PERSONA_NAMES[0], p, r, f);
}

#[test]
fn reality_python_ml_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(1, &personas[1]);
    info!("{}", m.format_report(PERSONA_NAMES[1]));
    // Measured 2026-08-24 — synthetic: P=0.833 R=0.357 F1=0.500 (floors
    // ratcheted UP from 0.35/0.20/0.25); calibrated: identical values.
    let (p, r, f) = mode_floors((0.73, 0.31, 0.45), (0.73, 0.31, 0.45));
    m.assert_quality(PERSONA_NAMES[1], p, r, f);
}

#[test]
fn reality_fullstack_ts_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(2, &personas[2]);
    info!("{}", m.format_report(PERSONA_NAMES[2]));
    // Measured 2026-08-24 — synthetic: P=0.786 R=0.458 F1=0.579 (floors
    // ratcheted UP from 0.45/0.40/0.40); calibrated: P=0.688 R=0.458 F1=0.550.
    let (p, r, f) = mode_floors((0.68, 0.41, 0.52), (0.58, 0.41, 0.50));
    m.assert_quality(PERSONA_NAMES[2], p, r, f);
}

#[test]
fn reality_devops_sre_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(3, &personas[3]);
    info!("{}", m.format_report(PERSONA_NAMES[3]));
    // Measured 2026-08-24 — synthetic: P=0.800 R=0.364 F1=0.500 (floors kept:
    // measured − buffer would sit at/below the existing 0.70/0.35/0.48, so
    // nothing to raise); calibrated: P=0.714 R=0.303 F1=0.426 (R_strong=0.889
    // — the strong lane holds; blended recall is dragged by WeakRelevant
    // adjacency the precision-first gates drop by design).
    let (p, r, f) = mode_floors((0.70, 0.35, 0.48), (0.61, 0.26, 0.37));
    m.assert_quality(PERSONA_NAMES[3], p, r, f);
}

#[test]
fn reality_mobile_dev_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(4, &personas[4]);
    info!("{}", m.format_report(PERSONA_NAMES[4]));
    // Measured 2026-08-24 — synthetic: P=0.600 R=0.500 F1=0.545 (floors
    // ratcheted UP from 0.40/0.30/0.35); calibrated: identical values
    // (R_strong=1.000 in both).
    let (p, r, f) = mode_floors((0.50, 0.46, 0.49), (0.50, 0.46, 0.49));
    m.assert_quality(PERSONA_NAMES[4], p, r, f);
}

#[test]
fn reality_bootstrap_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(5, &personas[5]);
    info!("{}", m.format_report(PERSONA_NAMES[5]));
    // Bootstrap: 1 interest, no feedback, thin context — conservative behavior expected.
    // Measured 2026-08-24 — synthetic: P=1.000 R=0.167 F1=0.286 (P/F1 floors
    // ratcheted UP from 0.10/0.15; R floor kept at 0.15, measured − buffer
    // would lower it); calibrated: P=1.000 R=0.208 F1=0.345.
    let (p, r, f) = mode_floors((0.90, 0.15, 0.23), (0.90, 0.16, 0.29));
    m.assert_quality(PERSONA_NAMES[5], p, r, f);
}

#[test]
fn reality_power_user_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(6, &personas[6]);
    info!("{}", m.format_report(PERSONA_NAMES[6]));
    // Measured 2026-08-24 — synthetic: P=0.886 R=0.304 F1=0.453 (floors
    // ratcheted UP from 0.65/0.25/0.38); calibrated: P=0.880 R=0.216 F1=0.346
    // (R_strong=0.455 — the worst generalist strong-recall; audit §2c. The
    // calibrated R/F1 floors sit below the synthetic ones because this is the
    // first honest real-embedding pin, not a loosened guard — the measured
    // gap IS the audit's headline recall finding, owned by the Phase-2+
    // recall arc.)
    let (p, r, f) = mode_floors((0.78, 0.26, 0.40), (0.78, 0.17, 0.29));
    m.assert_quality(PERSONA_NAMES[6], p, r, f);
}

#[test]
fn reality_context_switcher_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(7, &personas[7]);
    info!("{}", m.format_report(PERSONA_NAMES[7]));
    // Measured 2026-08-24 — synthetic: P=0.917 R=0.319 F1=0.473 (floors
    // ratcheted UP from 0.65/0.27/0.39); calibrated: P=0.947 R=0.261 F1=0.409
    // (R_strong=0.600). Same shape as power_user: the calibrated R/F1 floors
    // are the first real-embedding pin at measured level.
    let (p, r, f) = mode_floors((0.81, 0.27, 0.42), (0.84, 0.22, 0.35));
    m.assert_quality(PERSONA_NAMES[7], p, r, f);
}

#[test]
fn reality_niche_specialist_persona() {
    let personas = all_personas();
    let m = run_persona_simulation(8, &personas[8]);
    info!("{}", m.format_report(PERSONA_NAMES[8]));
    // Measured 2026-08-24 — synthetic AND calibrated: P=1.000 R=0.214 F1=0.353
    // (identical; the audit-era calibrated P=0.750 FP was fixed by Wave 1-2).
    // Synthetic P floor ratcheted UP 0.85 → 0.90; R/F1 kept (measured − buffer
    // would lower them). Dedup of overlapping interest+ace signals reduces
    // recall for niche personas but precision stays perfect.
    let (p, r, f) = mode_floors((0.90, 0.20, 0.33), (0.90, 0.17, 0.30));
    m.assert_quality(PERSONA_NAMES[8], p, r, f);
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
    let emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

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
    let emb = vec![0.0_f32; crate::EMBEDDING_DIMS];
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
    let emb = vec![0.0_f32; crate::EMBEDDING_DIMS];
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
        info!("{}", m.format_report(PERSONA_NAMES[pi]));
        aggregate.merge(&m);
    }

    info!("{}", aggregate.format_report("AGGREGATE"));
    // Aggregate quality floors per embedding mode (see mode_floors).
    // Measured 2026-08-24 — synthetic: P=0.862 F1=0.506 (floors ratcheted UP
    // from 0.70/0.40); calibrated: P=0.842 R=0.304 F1=0.447.
    let (min_p, _, min_f) = mode_floors((0.76, 0.0, 0.45), (0.74, 0.0, 0.40));
    assert!(
        aggregate.f1() >= min_f,
        "Aggregate F1 {:.3} below minimum {min_f:.2}",
        aggregate.f1()
    );
    assert!(
        aggregate.precision() >= min_p,
        "Aggregate precision {:.3} below minimum {min_p:.2}",
        aggregate.precision()
    );
}

/// Diagnostic (run with `--nocapture`): for the personas with the weakest Strong-recall,
/// dump WHY each StrongRelevant item was missed and tally the failure mode, so a recall
/// fix targets the actual dropping stage instead of guessing. Classification:
///   GATE      = fewer than 2 confirmed signal axes (hard-capped below threshold)
///   DOMAIN    = >=2 signals but domain_relevance <= 0.50 (off-stack crush)
///   THRESHOLD = >=2 signals, domain OK, but final score still < relevance threshold
/// Measurement only — never asserts, changes no scoring.
#[test]
fn diagnose_strong_misses() {
    let personas = all_personas();
    // Weakest Strong-recall personas (generalists + python/niche), by index.
    let targets = [
        (1usize, "python_ml"),
        (6, "power_user"),
        (7, "context_switcher"),
    ];
    let items = corpus();
    let db = sim_db();
    let opts = sim_no_freshness();
    let calibrated = super::load_corpus_embeddings();
    let zero = vec![0.0_f32; crate::EMBEDDING_DIMS];

    let mode = if cfg!(feature = "calibrated-sim") {
        "REAL"
    } else {
        "synthetic"
    };
    println!("\n=== Strong-miss diagnosis ({mode}) ===");
    let (mut gate, mut domain, mut threshold) = (0u32, 0u32, 0u32);
    for (pi, name) in targets {
        for item in &items {
            if !matches!(item.expected[pi], ExpectedOutcome::StrongRelevant) {
                continue;
            }
            let emb = calibrated.get((item.id - 1) as usize).unwrap_or(&zero);
            let input = sim_input(item.id, item.title, item.content, emb);
            let r = score_item(&input, &personas[pi], &db, &opts, None);
            if r.relevant && !r.excluded {
                continue; // caught — not a miss
            }
            let bd = r.score_breakdown.as_ref();
            let sig = bd.map(|b| b.signal_count).unwrap_or(0);
            let dom = bd.map(|b| b.domain_relevance).unwrap_or(1.0);
            let cause = if sig < 2 {
                gate += 1;
                "GATE"
            } else if dom <= 0.50 {
                domain += 1;
                "DOMAIN"
            } else {
                threshold += 1;
                "THRESHOLD"
            };
            let signals = bd
                .map(|b| b.confirmed_signals.join("+"))
                .unwrap_or_default();
            println!(
                "  [{cause:<9}] {name:<16} score={:.3} sig={sig} dom={dom:.2} int={:.2} dep={:.2} [{signals}] \"{}\"",
                r.top_score,
                bd.map(|b| b.interest_score).unwrap_or(0.0),
                bd.map(|b| b.dep_match_score).unwrap_or(0.0),
                if item.title.len() > 44 { &item.title[..44] } else { item.title },
            );
        }
    }
    println!("  ----\n  TALLY: GATE={gate} DOMAIN={domain} THRESHOLD={threshold}");
    println!("=== end diagnosis ===\n");
}

/// Measurement harness (run with `--nocapture`): prints the Strong-vs-Weak recall
/// split per persona and in aggregate. Blended recall is dominated by WeakRelevant
/// (tangential/adjacency) items a precision-first brief is *meant* to drop, so it
/// understates quality. Strong-recall is the load-bearing number — a Strong miss
/// (security advisory, release for a declared dep) is a genuine product failure.
/// Asserts only the sound invariant that the system catches Strong items at least as
/// well as Weak ones; the printed numbers drive calibration decisions.
#[test]
fn reality_strong_weak_recall_breakdown() {
    let personas = all_personas();
    let mut aggregate = SimMetrics::new();

    let mode = if cfg!(feature = "calibrated-sim") {
        "REAL fastembed fixtures"
    } else {
        "synthetic embeddings"
    };
    println!("\n=== Strong-vs-Weak recall breakdown ({mode}) ===");
    println!(
        "{:<18} {:>6} {:>6} {:>6} | {:>9} {:>10}",
        "persona", "P", "R", "F1", "R_strong", "R_weak"
    );
    for (pi, persona) in personas.iter().enumerate() {
        let m = run_persona_simulation(pi, persona);
        println!(
            "{:<18} {:>6.3} {:>6.3} {:>6.3} | {:>4.3} {:>2}/{:<2} {:>4.3} {:>2}/{:<2}",
            PERSONA_NAMES[pi],
            m.precision(),
            m.recall(),
            m.f1(),
            m.recall_strong(),
            m.tp_strong,
            m.tp_strong + m.fn_strong,
            m.recall_weak(),
            m.tp_weak,
            m.tp_weak + m.fn_weak,
        );
        aggregate.merge(&m);
    }
    println!(
        "{:<18} {:>6.3} {:>6.3} {:>6.3} | {:>4.3} {:>2}/{:<2} {:>4.3} {:>2}/{:<2}",
        "AGGREGATE",
        aggregate.precision(),
        aggregate.recall(),
        aggregate.f1(),
        aggregate.recall_strong(),
        aggregate.tp_strong,
        aggregate.tp_strong + aggregate.fn_strong,
        aggregate.recall_weak(),
        aggregate.tp_weak,
        aggregate.tp_weak + aggregate.fn_weak,
    );
    println!("=== end breakdown ===\n");

    // Sound invariant: Strong (high-signal) items should be caught at least as well as
    // Weak (tangential) ones. If Strong-recall ever drops below Weak-recall, the pipeline
    // is inverted — surfacing adjacency over substance — and that is a real regression.
    assert!(
        aggregate.recall_strong() + 1e-9 >= aggregate.recall_weak(),
        "INVERTED: aggregate Strong-recall {:.3} < Weak-recall {:.3} — pipeline favours adjacency over substance",
        aggregate.recall_strong(),
        aggregate.recall_weak()
    );
}
