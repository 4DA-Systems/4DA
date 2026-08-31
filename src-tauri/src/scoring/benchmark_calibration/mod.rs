// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Embedding-aware auto-calibration for the PASIFA scoring pipeline.
//!
//! Uses real fastembed (snowflake-arctic-embed-m) to embed test scenarios,
//! then optimizes sigmoid calibration parameters via hill-climbing
//! to maximize benchmark accuracy.
//!
//! Run: `cargo test scoring::benchmark_calibration::full_calibration -- --nocapture`

#[cfg(feature = "fastembed-local")]
mod embeddings;
#[cfg(feature = "fastembed-local")]
mod optimizer;
#[cfg(feature = "fastembed-local")]
mod profile;
#[cfg(feature = "fastembed-local")]
mod quality_gate;
#[cfg(feature = "fastembed-local")]
mod runner;
#[cfg(feature = "fastembed-local")]
mod types;

#[cfg(feature = "fastembed-local")]
use std::collections::HashMap;
#[cfg(feature = "fastembed-local")]
use tracing::info;

#[cfg(feature = "fastembed-local")]
use super::benchmark::bench_db;
#[cfg(feature = "fastembed-local")]
use super::benchmark_scenarios::{
    load_scenarios, profile_ctx, scenario_created_at, scenario_options, BenchmarkFailure,
    BenchmarkReport, CategoryResult, Scenario,
};
#[cfg(feature = "fastembed-local")]
use super::types::ScoringInput;
#[cfg(feature = "fastembed-local")]
use super::*;

// `self::` is explicit on purpose: `use super::*` above now also glob-imports
// `scoring::types` (the ScoringInput/ScoringOptions home). The local `mod types`
// shadows the glob by Rust's resolution rules, but naming it leaves nothing to
// infer at a glance.
#[cfg(feature = "fastembed-local")]
pub(crate) use self::types::CalibrationResult;

// ============================================================================
// Full Calibration Orchestrator
// ============================================================================

/// Run the complete calibration pipeline:
/// 1. Load scenarios
/// 2. Generate real embeddings for all texts
/// 3. Run benchmark with default params
/// 4. Hill-climb to optimize params
/// 5. Run final benchmark with optimized params
/// 6. Check quality gate
#[cfg(feature = "fastembed-local")]
pub(crate) fn run_calibration_sync() -> crate::error::Result<CalibrationResult> {
    let model_name = "snowflake-arctic-embed-m".to_string();

    info!("=== PASIFA Auto-Calibration ===");
    info!("Model: {}", model_name);

    // Step 1: Load scenarios
    let scenarios = load_scenarios();
    info!("Loaded {} scenarios", scenarios.len());

    // Step 2: Generate embeddings
    let (item_emb, topic_emb) = embeddings::generate_all_embeddings(&scenarios)?;
    info!(
        "Generated {} item embeddings, {} topic embeddings",
        item_emb.len(),
        topic_emb.len()
    );

    // Step 3: Run benchmark with current default params
    let db = bench_db();
    let original_center = crate::embedding_calibration::get_sigmoid_center();
    let original_scale = crate::embedding_calibration::get_sigmoid_scale();

    info!(
        "Default params: center={:.3} scale={:.1}",
        original_center, original_scale
    );

    crate::embedding_calibration::set_active_params(original_center, original_scale);
    let original_report =
        runner::run_benchmark_with_embeddings(&db, &item_emb, &topic_emb, &model_name);
    let original_accuracy = original_report.accuracy;

    info!(
        "Original accuracy: {:.1}% ({}/{})",
        original_accuracy * 100.0,
        original_report.passed,
        original_report.total
    );

    // Step 4: Hill-climb optimization
    let (opt_center, opt_scale, _opt_accuracy) = optimizer::hill_climb_calibration(
        &db,
        &item_emb,
        &topic_emb,
        original_center,
        original_scale,
        &model_name,
    );

    // Step 5: Final benchmark with optimized params
    crate::embedding_calibration::set_active_params(opt_center, opt_scale);
    let final_report =
        runner::run_benchmark_with_embeddings(&db, &item_emb, &topic_emb, &model_name);

    // Step 6: Quality gate
    let meets_gate = quality_gate::model_meets_quality_gate(&final_report);

    info!("\n=== Calibration Results ===");
    info!(
        "Original:  center={:.3} scale={:.1} accuracy={:.1}%",
        original_center,
        original_scale,
        original_accuracy * 100.0
    );
    info!(
        "Optimized: center={:.3} scale={:.1} accuracy={:.1}%",
        opt_center,
        opt_scale,
        final_report.accuracy * 100.0
    );
    info!(
        "Quality gate: {}",
        if meets_gate { "PASSED" } else { "FAILED" }
    );

    // Restore original params (caller decides whether to apply optimized)
    crate::embedding_calibration::set_active_params(original_center, original_scale);

    Ok(CalibrationResult {
        model_name,
        original_accuracy,
        original_params: (original_center, original_scale),
        optimized_accuracy: final_report.accuracy,
        optimized_params: (opt_center, opt_scale),
        benchmark_report: final_report,
        meets_quality_gate: meets_gate,
    })
}

// ============================================================================
// Model-unavailable guard (2026-08-23 adversarial audit, item 22)
// ============================================================================

/// True when the environment demands that real-embedding tests actually
/// measure. Set `FOURDA_REQUIRE_REAL_EMBEDDINGS=1` wherever a green result is
/// consumed as evidence (CI's real-embedding step sets it after fetching the
/// model): with it, an unavailable model FAILS instead of skipping, and the
/// benchmark quality-gate ratchet hard-fails instead of soft-warning.
#[cfg(feature = "fastembed-local")]
pub(crate) fn real_embeddings_required() -> bool {
    std::env::var("FOURDA_REQUIRE_REAL_EMBEDDINGS").is_ok_and(|v| v == "1")
}

/// The one policy point for "the embedding model failed to load" in a
/// real-embedding test. Default (required=false): a LOUD skip — the historic
/// behavior for hermetic/offline runners that legitimately cannot fetch the
/// model, now impossible to mistake for a measurement. Required=true: panic —
/// a harness told to measure real embeddings must never green-pass having
/// measured nothing (the audit's E8 silent-skip hazard: CI could report the
/// calibration suite green while the model download had failed).
///
/// Pure in `required` so the policy is unit-testable without process-global
/// env mutation (env reads race across parallel tests).
#[cfg(feature = "fastembed-local")]
fn skip_or_fail_model_unavailable(test_name: &str, err: &str, required: bool) {
    if required {
        panic!(
            "FOURDA_REQUIRE_REAL_EMBEDDINGS=1 but the embedding model is unavailable — \
             {test_name} would have SKIPPED and reported a vacuous green pass. \
             Fetch the model first (node scripts/download-ort.cjs && \
             node scripts/download-embedding-model.cjs, or `pnpm run bundle:resources`) \
             or unset the env var to accept a loud skip. Underlying error: {err}"
        );
    }
    eprintln!(
        "SKIP {test_name}: embedding model unavailable ({err}) — \
         NO measurement was made; this green result is vacuous. \
         Set FOURDA_REQUIRE_REAL_EMBEDDINGS=1 to make this a hard failure."
    );
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(feature = "fastembed-local")]
#[test]
fn guard_model_unavailable_skips_loudly_by_default() {
    // required=false must NOT panic (the hermetic/offline skip survives).
    skip_or_fail_model_unavailable("guard_selftest", "simulated: model missing", false);
}

#[cfg(feature = "fastembed-local")]
#[test]
fn guard_model_unavailable_panics_when_required() {
    // required=true must panic — the env-gated path can never green-pass a
    // skipped measurement. catch_unwind on the pure function keeps this
    // hermetic (no env mutation, no model needed).
    let outcome = std::panic::catch_unwind(|| {
        skip_or_fail_model_unavailable("guard_selftest", "simulated: model missing", true);
    });
    let err = outcome.expect_err("required=true must panic on an unavailable model");
    let msg = err.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(
        msg.contains("FOURDA_REQUIRE_REAL_EMBEDDINGS"),
        "panic must name the env gate so the CI log is self-explanatory: {msg}"
    );
    assert!(
        msg.contains("guard_selftest"),
        "panic must name the skipping test: {msg}"
    );
}

#[cfg(feature = "fastembed-local")]
#[test]
fn embedding_generation_works() {
    let texts = vec![
        "Rust programming language".to_string(),
        "Machine learning with Python".to_string(),
        "TypeScript frontend development".to_string(),
    ];

    // Non-hermetic: real embeddings make fastembed download the model from the
    // network on first use. A fresh or offline runner can receive a truncated
    // archive — the hermetic Fresh-Clone CI hit exactly this on Linux ("invalid
    // Zip archive: Could not find central directory end", 2026-06-13). Skip
    // (loudly) rather than fail the whole suite when the model is unavailable;
    // the assertions below still run wherever the model loads. Under
    // FOURDA_REQUIRE_REAL_EMBEDDINGS=1 the skip becomes a hard failure — see
    // skip_or_fail_model_unavailable.
    let raw = match crate::fastembed_sync(&texts) {
        Ok(v) => v,
        Err(e) => {
            skip_or_fail_model_unavailable(
                "embedding_generation_works",
                &e.to_string(),
                real_embeddings_required(),
            );
            return;
        }
    };
    let embeddings: Vec<Vec<f32>> = raw
        .into_iter()
        .map(self::types::pad_and_normalize)
        .collect();
    assert_eq!(embeddings.len(), 3, "Should get one embedding per text");

    for (i, emb) in embeddings.iter().enumerate() {
        assert_eq!(
            emb.len(),
            crate::EMBEDDING_DIMS,
            "Embedding {} should be {}-dim, got {}",
            i,
            crate::EMBEDDING_DIMS,
            emb.len()
        );

        // Verify approximately unit norm (fastembed normalizes output)
        let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.1,
            "Embedding {} should be approximately unit norm, got {:.4}",
            i,
            norm
        );
    }
}

#[cfg(feature = "fastembed-local")]
#[test]
fn full_calibration_with_real_embeddings() {
    // Real-embedding calibration needs the fastembed model (network download on
    // first use). Skip (loudly) when unavailable instead of failing the
    // hermetic suite — see embedding_generation_works for the full rationale.
    // Under FOURDA_REQUIRE_REAL_EMBEDDINGS=1 an unavailable model FAILS: this
    // is the audit's E8 hazard test — a CI run must never pass having measured
    // nothing.
    let result = match run_calibration_sync() {
        Ok(r) => r,
        Err(e) => {
            skip_or_fail_model_unavailable(
                "full_calibration_with_real_embeddings",
                &e.to_string(),
                real_embeddings_required(),
            );
            return;
        }
    };

    let r = &result.benchmark_report;
    eprintln!("\n=== PASIFA Auto-Calibration Results ===");
    eprintln!("Model: {}", result.model_name);
    eprintln!(
        "Original:  center={:.3} scale={:.1} score-range={:.1}%",
        result.original_params.0,
        result.original_params.1,
        result.original_accuracy * 100.0
    );
    eprintln!(
        "Optimized: center={:.3} scale={:.1} score-range={:.1}%",
        result.optimized_params.0,
        result.optimized_params.1,
        result.optimized_accuracy * 100.0
    );
    eprintln!(
        "Relevance accuracy: {:.1}% (pipeline quality metric)",
        r.relevance_accuracy * 100.0
    );
    eprintln!(
        "Quality gate: {}",
        if result.meets_quality_gate {
            "PASSED"
        } else {
            "FAILED"
        }
    );
    for (cat, cr) in &r.by_category {
        eprintln!(
            "  {:16} {}/{} ({:.0}%)",
            cat,
            cr.passed,
            cr.total,
            cr.accuracy * 100.0
        );
    }
    if !r.failures.is_empty() {
        eprintln!("Score-range failures ({}):", r.failures.len());
        for f in &r.failures {
            eprintln!(
                "  [{}] {} score={:.3} range expected",
                f.category, f.scenario_id, f.actual_score
            );
        }
    }
    if !result.meets_quality_gate {
        // Local dev keeps the soft warning (a model transition may be mid-
        // flight); the env-gated CI path enforces the ratchet — otherwise CI
        // would still be blind to score-range regressions even while running
        // real-embedding mode (2026-08-23 audit, items 21/22).
        if real_embeddings_required() {
            panic!(
                "quality-gate RATCHET FAILED in required real-embedding mode: \
                 overall score-range {:.1}% — see the floors and doctrine in \
                 benchmark_calibration/quality_gate.rs (fix the regression or \
                 consciously lower the floor in the same PR)",
                result.benchmark_report.accuracy * 100.0
            );
        }
        eprintln!(
            "WARN: quality gate soft-fail during model transition: overall={:.1}%",
            result.benchmark_report.accuracy * 100.0
        );
    }
}

#[cfg(feature = "fastembed-local")]
#[test]
fn hill_climbing_improves_or_maintains() {
    // Same network-model dependency as the other real-embedding tests — skip
    // loudly when the model cannot be loaded rather than failing the suite;
    // FOURDA_REQUIRE_REAL_EMBEDDINGS=1 turns the skip into a hard failure.
    let result = match run_calibration_sync() {
        Ok(r) => r,
        Err(e) => {
            skip_or_fail_model_unavailable(
                "hill_climbing_improves_or_maintains",
                &e.to_string(),
                real_embeddings_required(),
            );
            return;
        }
    };

    // Allow 2% tolerance: the hill climber is stochastic and parameter
    // changes (e.g. dampening, exposure thresholds) can shift the landscape
    // enough that a fixed iteration count doesn't always find a better peak.
    assert!(
        result.optimized_accuracy >= result.original_accuracy - 0.02,
        "Optimized accuracy ({:.1}%) should be within 2% of original ({:.1}%)",
        result.optimized_accuracy * 100.0,
        result.original_accuracy * 100.0,
    );
}

#[cfg(feature = "fastembed-local")]
#[test]
fn quality_gate_rejects_bad_results() {
    use super::benchmark_scenarios::{BenchmarkReport, CategoryResult};

    // Construct a report with bad accuracy
    let mut by_category = HashMap::new();
    by_category.insert(
        "true_positive".to_string(),
        CategoryResult {
            total: 15,
            passed: 8,
            accuracy: 0.53, // < 70%
        },
    );
    by_category.insert(
        "true_negative".to_string(),
        CategoryResult {
            total: 15,
            passed: 12,
            accuracy: 0.80, // < 90%
        },
    );
    by_category.insert(
        "security".to_string(),
        CategoryResult {
            total: 10,
            passed: 7,
            accuracy: 0.70, // < 90%
        },
    );

    let bad_report = BenchmarkReport {
        total: 62,
        passed: 40,
        failed: 22,
        accuracy: 0.645, // < 80%
        relevance_accuracy: 0.50,
        by_category,
        failures: vec![],
    };

    assert!(
        !quality_gate::model_meets_quality_gate(&bad_report),
        "Quality gate should reject report with {:.1}% accuracy",
        bad_report.accuracy * 100.0
    );
}

/// Ratchet regression: a report that sailed through the ORIGINAL generous
/// thresholds (overall 80 / TP 70 / TN 90 / sec 90) must now FAIL — that
/// slack is exactly how five under-scoring regressions accumulated silently
/// between v7 and v21 (2026-08 audit). See quality_gate.rs header.
#[cfg(feature = "fastembed-local")]
#[test]
fn quality_gate_ratchet_rejects_pre_audit_drift_levels() {
    use super::benchmark_scenarios::{BenchmarkReport, CategoryResult};

    let mut by_category = HashMap::new();
    by_category.insert(
        "true_positive".to_string(),
        CategoryResult {
            total: 20,
            passed: 15,
            accuracy: 0.75,
        }, // old gate: fine; ratchet: < 0.80
    );
    by_category.insert(
        "true_negative".to_string(),
        CategoryResult {
            total: 20,
            passed: 19,
            accuracy: 0.95,
        }, // old gate: fine; ratchet: < 1.00
    );
    by_category.insert(
        "security".to_string(),
        CategoryResult {
            total: 12,
            passed: 11,
            accuracy: 0.9167,
        },
    );
    by_category.insert(
        "cold_start".to_string(),
        CategoryResult {
            total: 12,
            passed: 11,
            accuracy: 0.9167,
        },
    );

    let drifted = BenchmarkReport {
        total: 64,
        passed: 56,
        failed: 8,
        accuracy: 0.875, // old gate: >= 0.80 fine; ratchet: < 0.92
        relevance_accuracy: 0.70,
        by_category,
        failures: vec![],
    };

    assert!(
        !quality_gate::model_meets_quality_gate(&drifted),
        "Ratchet must reject drift the pre-audit thresholds tolerated"
    );

    // The 2026-08-22 achieved state (overall 92.3%, TP 16/20, sec 11/12,
    // cold 11/12) must now ALSO fail — the 2026-08-24 ratchet raise locked in
    // the Wave 1-2 recall recoveries; sliding back to the pre-fix level is a
    // regression, not an acceptable state.
    let mut pre_raise = HashMap::new();
    pre_raise.insert(
        "true_positive".to_string(),
        CategoryResult {
            total: 20,
            passed: 16,
            accuracy: 0.80,
        },
    );
    pre_raise.insert(
        "true_negative".to_string(),
        CategoryResult {
            total: 20,
            passed: 20,
            accuracy: 1.00,
        },
    );
    pre_raise.insert(
        "security".to_string(),
        CategoryResult {
            total: 12,
            passed: 11,
            accuracy: 0.9167,
        },
    );
    pre_raise.insert(
        "cold_start".to_string(),
        CategoryResult {
            total: 12,
            passed: 11,
            accuracy: 0.9167,
        },
    );
    let pre_raise_report = BenchmarkReport {
        total: 78,
        passed: 72,
        failed: 6,
        accuracy: 0.923,
        relevance_accuracy: 0.744,
        by_category: pre_raise,
        failures: vec![],
    };
    assert!(
        !quality_gate::model_meets_quality_gate(&pre_raise_report),
        "The pre-raise (2026-08-22) level must fail the raised ratchet"
    );

    // And the CURRENT achieved state passes — the ratchet locks, it does not
    // overreach. Measured 2026-08-24 (62ffbacf + Wave 1-2 + harness wiring):
    // 82/85, TP 17/20, TN 20/20, sec 12/12, cold 12/12, harness 7/7.
    let mut current = HashMap::new();
    current.insert(
        "true_positive".to_string(),
        CategoryResult {
            total: 20,
            passed: 17,
            accuracy: 0.85,
        },
    );
    current.insert(
        "true_negative".to_string(),
        CategoryResult {
            total: 20,
            passed: 20,
            accuracy: 1.00,
        },
    );
    current.insert(
        "security".to_string(),
        CategoryResult {
            total: 12,
            passed: 12,
            accuracy: 1.00,
        },
    );
    current.insert(
        "cold_start".to_string(),
        CategoryResult {
            total: 12,
            passed: 12,
            accuracy: 1.00,
        },
    );
    current.insert(
        "harness_coverage".to_string(),
        CategoryResult {
            total: 7,
            passed: 7,
            accuracy: 1.00,
        },
    );
    let achieved = BenchmarkReport {
        total: 85,
        passed: 82,
        failed: 3,
        accuracy: 0.9647,
        relevance_accuracy: 0.765,
        by_category: current,
        failures: vec![],
    };
    assert!(
        quality_gate::model_meets_quality_gate(&achieved),
        "The currently-achieved state must pass the ratchet"
    );
}

/// Cross-machine noise-margin semantics (2026-08-25, see quality_gate.rs
/// module doc): hosted CI runners' float paths can flip exactly ONE
/// near-threshold scenario (observed: edge_deprecated_tech 0.413 vs band max
/// 0.30 on ubuntu-hosted, in-band across 3 byte-identical local runs — PR
/// #527 run 32734979346, a PR with zero scoring changes). The floors absorb
/// exactly one such flip; a second concurrent failure is drift and stays red.
#[test]
fn quality_gate_tolerates_one_cross_machine_flip_but_not_two() {
    use super::benchmark_scenarios::{BenchmarkReport, CategoryResult};

    let base_categories = |edge_passed: usize| {
        let mut m = HashMap::new();
        m.insert(
            "true_positive".to_string(),
            CategoryResult {
                total: 20,
                passed: 17,
                accuracy: 0.85,
            },
        );
        m.insert(
            "true_negative".to_string(),
            CategoryResult {
                total: 20,
                passed: 20,
                accuracy: 1.00,
            },
        );
        m.insert(
            "security".to_string(),
            CategoryResult {
                total: 12,
                passed: 12,
                accuracy: 1.00,
            },
        );
        m.insert(
            "cold_start".to_string(),
            CategoryResult {
                total: 12,
                passed: 12,
                accuracy: 1.00,
            },
        );
        m.insert(
            "harness_coverage".to_string(),
            CategoryResult {
                total: 7,
                passed: 7,
                accuracy: 1.00,
            },
        );
        m.insert(
            "edge_case".to_string(),
            CategoryResult {
                total: 14,
                passed: edge_passed,
                accuracy: edge_passed as f32 / 14.0,
            },
        );
        m
    };

    // Exactly the #527 CI state: one edge scenario flipped, 81/85 overall.
    let one_flip = BenchmarkReport {
        total: 85,
        passed: 81,
        failed: 4,
        accuracy: 0.9529,
        relevance_accuracy: 0.76,
        by_category: base_categories(13),
        failures: vec![],
    };
    assert!(
        quality_gate::model_meets_quality_gate(&one_flip),
        "one cross-machine threshold-flip (81/85, edge 13/14) must pass — \
         this exact state red-blocked the zero-scoring-change PR #527"
    );

    // A second concurrent flip is drift, not noise: red via BOTH the overall
    // floor (80/85 = 94.1% < 0.95) and the edge floor (12/14 = 85.7% < 0.92).
    let two_flips = BenchmarkReport {
        total: 85,
        passed: 80,
        failed: 5,
        accuracy: 0.9412,
        relevance_accuracy: 0.75,
        by_category: base_categories(12),
        failures: vec![],
    };
    assert!(
        !quality_gate::model_meets_quality_gate(&two_flips),
        "two concurrent flips (80/85) must stay red — that is drift"
    );

    // A single TRUE-NEGATIVE flip must stay red regardless of the overall
    // margin: precision-first is a hard gate, not a noise candidate.
    let mut tn_flip_categories = base_categories(14);
    tn_flip_categories.insert(
        "true_negative".to_string(),
        CategoryResult {
            total: 20,
            passed: 19,
            accuracy: 0.95,
        },
    );
    let tn_flip = BenchmarkReport {
        total: 85,
        passed: 81,
        failed: 4,
        accuracy: 0.9529,
        relevance_accuracy: 0.76,
        by_category: tn_flip_categories,
        failures: vec![],
    };
    assert!(
        !quality_gate::model_meets_quality_gate(&tn_flip),
        "a false positive (TN 19/20) must stay red even inside the overall margin"
    );
}

/// Diagnostic: dump every scenario's actual score, relevance, and signals
/// to identify which scenarios need re-calibration.
#[cfg(feature = "fastembed-local")]
#[test]
#[ignore]
fn diagnostic_dump_all_scenarios() {
    let scenarios = load_scenarios();
    let (item_emb, topic_emb) = embeddings::generate_all_embeddings(&scenarios).unwrap();
    let db = bench_db();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

    eprintln!("\n=== SCENARIO DIAGNOSTIC DUMP ===");
    eprintln!(
        "{:<40} {:>6} {:>5} {:>5} {:>5} {:>4} {:>5} {:>5} {:>5} {:<20} {}",
        "SCENARIO", "SCORE", "REL", "EXPRL", "PASS", "SIGS", "INT", "KW", "DEP", "SIGNALS", "RANGE"
    );
    eprintln!("{}", "-".repeat(132));

    for scenario in &scenarios {
        let ctx = profile::build_profile_with_embeddings(&scenario.profile, &topic_emb);
        let opts = scenario_options(scenario);
        let created_at = scenario_created_at(scenario);
        let embedding = item_emb
            .get(&scenario.id)
            .map(|v| v.as_slice())
            .unwrap_or(&zero_emb);
        let tags: Vec<String> = scenario
            .item
            .tags_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();

        let input = ScoringInput {
            id: 1,
            title: &scenario.item.title,
            url: Some("https://example.com"),
            content: &scenario.item.content,
            source_type: &scenario.item.source_type,
            embedding,
            created_at: created_at.as_ref(),
            detected_lang: "en",
            source_tags: &tags,
            tags_json: scenario.item.tags_json.as_deref(),
            feed_origin: None,
            source_id: scenario.item.source_id.as_deref(),
        };

        let result = score_item(&input, &ctx, &db, &opts, None);
        let bd = result.score_breakdown.as_ref();
        let sigs = bd.map(|b| b.signal_count).unwrap_or(0);
        let confirmed = bd
            .map(|b| b.confirmed_signals.join(","))
            .unwrap_or_default();

        let rel_ok = result.relevant == scenario.expected.should_be_relevant;
        let range_ok = result.top_score >= scenario.expected.score_min
            && result.top_score <= scenario.expected.score_max;
        let pass = rel_ok && range_ok;
        let pass_str = if pass {
            "OK"
        } else if !rel_ok {
            "REL!"
        } else {
            "RNG!"
        };

        eprintln!(
            "{:<40} {:>6.3} {:>5} {:>5} {:>5} {:>4} {:>5.2} {:>5.2} {:>5.2} {:<20} [{:.2}-{:.2}]",
            format!(
                "[{}] {}",
                &scenario.category[..std::cmp::min(3, scenario.category.len())],
                &scenario.id
            ),
            result.top_score,
            result.relevant,
            scenario.expected.should_be_relevant,
            pass_str,
            sigs,
            bd.map(|b| b.interest_score).unwrap_or(0.0),
            bd.map(|b| b.keyword_score).unwrap_or(0.0),
            bd.map(|b| b.dep_match_score).unwrap_or(0.0),
            &confirmed[..std::cmp::min(20, confirmed.len())],
            scenario.expected.score_min,
            scenario.expected.score_max
        );
    }
    eprintln!("=== END DUMP ===\n");
}
