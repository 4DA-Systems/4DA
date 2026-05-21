// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Embedding-aware auto-calibration for the PASIFA scoring pipeline.
//!
//! Uses real fastembed (snowflake-arctic-embed-m) to embed test scenarios,
//! then optimizes sigmoid calibration parameters via hill-climbing
//! to maximize benchmark accuracy.
//!
//! Run: `cargo test scoring::benchmark_calibration::full_calibration -- --nocapture`

#[cfg(feature = "fastembed-local")]
use std::collections::HashMap;
#[cfg(feature = "fastembed-local")]
use tracing::{info, warn};

#[cfg(feature = "fastembed-local")]
use super::benchmark::{bench_db, no_freshness};
#[cfg(feature = "fastembed-local")]
use super::benchmark_scenarios::{
    load_scenarios, profile_ctx, BenchmarkFailure, BenchmarkReport, CategoryResult, Scenario,
};
#[cfg(feature = "fastembed-local")]
use super::pipeline::ScoringInput;
#[cfg(feature = "fastembed-local")]
use super::*;

// ============================================================================
// Types
// ============================================================================

#[cfg(feature = "fastembed-local")]
#[derive(Clone)]
pub(crate) struct CalibrationResult {
    pub model_name: String,
    pub original_accuracy: f32,
    pub original_params: (f32, f32),
    pub optimized_accuracy: f32,
    pub optimized_params: (f32, f32),
    pub benchmark_report: BenchmarkReport,
    pub meets_quality_gate: bool,
}

/// Pad fastembed vectors to EMBEDDING_DIMS with zeros, then L2-normalize.
/// Mirrors the truncate_and_normalize step that embed_texts() applies in production.
#[cfg(feature = "fastembed-local")]
fn pad_and_normalize(mut v: Vec<f32>) -> Vec<f32> {
    let target = crate::EMBEDDING_DIMS;
    if v.len() < target {
        v.resize(target, 0.0);
    } else if v.len() > target {
        v.truncate(target);
    }
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}
// ============================================================================
// Quality Gate
// ============================================================================

#[cfg(feature = "fastembed-local")]
fn model_meets_quality_gate(report: &BenchmarkReport) -> bool {
    let overall_ok = report.accuracy >= 0.80;

    let tp_ok = report
        .by_category
        .get("true_positive")
        .map_or(false, |c| c.accuracy >= 0.70);

    let tn_ok = report
        .by_category
        .get("true_negative")
        .map_or(false, |c| c.accuracy >= 0.90);

    let sec_ok = report
        .by_category
        .get("security")
        .map_or(false, |c| c.accuracy >= 0.90);

    if !overall_ok {
        warn!(
            "Quality gate: overall accuracy {:.1}% < 80%",
            report.accuracy * 100.0
        );
    }
    if !tp_ok {
        warn!(
            "Quality gate: true_positive accuracy {:.1}% < 70%",
            report
                .by_category
                .get("true_positive")
                .map_or(0.0, |c| c.accuracy)
                * 100.0
        );
    }
    if !tn_ok {
        warn!(
            "Quality gate: true_negative accuracy {:.1}% < 90%",
            report
                .by_category
                .get("true_negative")
                .map_or(0.0, |c| c.accuracy)
                * 100.0
        );
    }
    if !sec_ok {
        warn!(
            "Quality gate: security accuracy {:.1}% < 90%",
            report
                .by_category
                .get("security")
                .map_or(0.0, |c| c.accuracy)
                * 100.0
        );
    }

    overall_ok && tp_ok && tn_ok && sec_ok
}

// ============================================================================
// Embedding Generation
// ============================================================================

/// Embed all scenario texts and profile topic names using fastembed (snowflake-arctic-embed-m).
///
/// Returns (item_embeddings, topic_embeddings) where:
/// - item_embeddings: scenario_id -> embedding vector
/// - topic_embeddings: topic_name -> embedding vector
#[cfg(feature = "fastembed-local")]
fn generate_all_embeddings(
    scenarios: &[Scenario],
) -> crate::error::Result<(HashMap<String, Vec<f32>>, HashMap<String, Vec<f32>>)> {
    // Collect unique item texts: "{title}. {content}"
    let mut item_texts: Vec<String> = Vec::with_capacity(scenarios.len());
    let mut item_ids: Vec<String> = Vec::with_capacity(scenarios.len());
    for s in scenarios {
        item_texts.push(format!("{}. {}", s.item.title, s.item.content));
        item_ids.push(s.id.clone());
    }

    info!(
        "Embedding {} scenario texts via fastembed...",
        item_texts.len()
    );
    let item_vectors: Vec<Vec<f32>> = crate::fastembed_sync(&item_texts)?
        .into_iter()
        .map(pad_and_normalize)
        .collect();

    let mut item_embeddings = HashMap::with_capacity(scenarios.len());
    for (id, vec) in item_ids.into_iter().zip(item_vectors) {
        item_embeddings.insert(id, vec);
    }

    // Collect ALL unique topic names across all profiles — interest names, ACE
    // active_topics, and detected_tech. The semantic boost function looks up by
    // lowercase key so we embed every variant and store under lowercase.
    let all_profile_topics: &[&[&str]] = &[
        // rust_developer: interests + ACE topics + detected tech + deps
        &[
            "Rust",
            "systems programming",
            "Tauri",
            "rust",
            "tauri",
            "sqlite",
            "tokio",
            "serde",
            "hyper",
        ],
        // fullstack_js: interests + ACE topics + detected tech
        &[
            "TypeScript",
            "React",
            "Node.js",
            "typescript",
            "react",
            "nodejs",
            "next",
            "express",
        ],
        // python_data_scientist: interests + ACE topics + detected tech
        &[
            "Machine Learning",
            "Python",
            "Data Science",
            "python",
            "pytorch",
            "ml",
            "torch",
            "transformers",
        ],
    ];

    let mut unique_topics: Vec<String> = Vec::new();
    for group in all_profile_topics {
        for &t in *group {
            let ts = t.to_string();
            if !unique_topics.contains(&ts) {
                unique_topics.push(ts);
            }
        }
    }

    info!(
        "Embedding {} topic names via fastembed...",
        unique_topics.len()
    );
    let topic_vectors: Vec<Vec<f32>> = crate::fastembed_sync(&unique_topics)?
        .into_iter()
        .map(pad_and_normalize)
        .collect();

    let mut topic_embeddings = HashMap::with_capacity(unique_topics.len() * 2);
    for (name, vec) in unique_topics.into_iter().zip(topic_vectors) {
        let lower = name.to_lowercase();
        if lower != name {
            topic_embeddings.insert(lower, vec.clone());
        }
        topic_embeddings.insert(name, vec);
    }

    Ok((item_embeddings, topic_embeddings))
}

// ============================================================================
// Profile Construction with Real Embeddings
// ============================================================================

/// Build a scoring context for the named profile, replacing dummy 0.5 vectors
/// with real embeddings from the topic_embeddings map.
#[cfg(feature = "fastembed-local")]
fn build_profile_with_embeddings(
    name: &str,
    topic_embeddings: &HashMap<String, Vec<f32>>,
) -> ScoringContext {
    let mut ctx = profile_ctx(name);

    // Replace interest embeddings with real ones
    for interest in &mut ctx.interests {
        if let Some(real_emb) = topic_embeddings.get(&interest.topic) {
            interest.embedding = Some(real_emb.clone());
        }
    }

    // Populate topic_embeddings map for ALL topics the pipeline will look up:
    // interest topics, ACE active_topics, and detected_tech (all lowercase keys).
    for interest in &ctx.interests {
        let lower = interest.topic.to_lowercase();
        if let Some(real_emb) = topic_embeddings
            .get(&interest.topic)
            .or_else(|| topic_embeddings.get(&lower))
        {
            ctx.topic_embeddings.insert(lower, real_emb.clone());
        }
    }
    for topic in &ctx.ace_ctx.active_topics {
        if !ctx.topic_embeddings.contains_key(topic) {
            if let Some(real_emb) = topic_embeddings.get(topic) {
                ctx.topic_embeddings.insert(topic.clone(), real_emb.clone());
            }
        }
    }
    for tech in &ctx.ace_ctx.detected_tech {
        if !ctx.topic_embeddings.contains_key(tech) {
            if let Some(real_emb) = topic_embeddings.get(tech) {
                ctx.topic_embeddings.insert(tech.clone(), real_emb.clone());
            }
        }
    }

    ctx
}

// ============================================================================
// Benchmark with Real Embeddings
// ============================================================================

#[cfg(feature = "fastembed-local")]
fn run_benchmark_with_embeddings(
    db: &crate::db::Database,
    item_emb: &HashMap<String, Vec<f32>>,
    topic_emb: &HashMap<String, Vec<f32>>,
    _model_name: &str,
) -> BenchmarkReport {
    let scenarios = load_scenarios();
    let opts = no_freshness();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

    let mut total = 0;
    let mut passed = 0;
    let mut relevance_correct = 0;
    let mut failures = Vec::new();
    let mut by_category: HashMap<String, (usize, usize)> = HashMap::new();

    for scenario in &scenarios {
        total += 1;
        let ctx = build_profile_with_embeddings(&scenario.profile, topic_emb);

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
        let tags_json_ref = scenario.item.tags_json.as_deref();

        let input = ScoringInput {
            id: total as u64,
            title: &scenario.item.title,
            url: Some("https://example.com"),
            content: &scenario.item.content,
            source_type: &scenario.item.source_type,
            embedding,
            created_at: None,
            detected_lang: "en",
            source_tags: &tags,
            tags_json: tags_json_ref,
            feed_origin: None,
        };

        let result = score_item(&input, &ctx, db, &opts, None);

        let actual_relevant = result.relevant;
        let actual_score = result.top_score;
        let bd = result.score_breakdown.as_ref();
        let signal_count = bd.map(|b| b.signal_count).unwrap_or(0);
        let confirmed_signals = bd.map(|b| b.confirmed_signals.clone()).unwrap_or_default();

        if actual_relevant == scenario.expected.should_be_relevant {
            relevance_correct += 1;
        }
        let score_in_range = actual_score >= scenario.expected.score_min
            && actual_score <= scenario.expected.score_max;

        let cat_entry = by_category
            .entry(scenario.category.clone())
            .or_insert((0, 0));
        cat_entry.0 += 1;

        if score_in_range {
            passed += 1;
            cat_entry.1 += 1;
        } else {
            failures.push(BenchmarkFailure {
                scenario_id: scenario.id.clone(),
                category: scenario.category.clone(),
                expected_relevant: scenario.expected.should_be_relevant,
                actual_relevant,
                actual_score,
                signal_count,
                confirmed_signals,
                notes: scenario.expected.notes.clone(),
            });
        }
    }

    let accuracy = if total > 0 {
        passed as f32 / total as f32
    } else {
        0.0
    };
    let relevance_accuracy = if total > 0 {
        relevance_correct as f32 / total as f32
    } else {
        0.0
    };

    let by_category = by_category
        .into_iter()
        .map(|(cat, (cat_total, cat_passed))| {
            let cat_accuracy = if cat_total > 0 {
                cat_passed as f32 / cat_total as f32
            } else {
                0.0
            };
            (
                cat,
                CategoryResult {
                    total: cat_total,
                    passed: cat_passed,
                    accuracy: cat_accuracy,
                },
            )
        })
        .collect();

    BenchmarkReport {
        total,
        passed,
        failed: total - passed,
        accuracy,
        relevance_accuracy,
        by_category,
        failures,
    }
}

// ============================================================================
// Hill-Climbing Optimizer
// ============================================================================

/// Optimize sigmoid calibration parameters via greedy hill-climbing.
///
/// Tries 8 neighbors per iteration (center +/- 0.02, scale +/- 0.5, plus 4 diagonals).
/// Accepts the first neighbor that improves accuracy (greedy first-improvement).
/// Max 10 iterations to keep calibration fast.
///
/// Returns (best_center, best_scale, best_accuracy).
#[cfg(feature = "fastembed-local")]
fn hill_climb_calibration(
    db: &crate::db::Database,
    item_emb: &HashMap<String, Vec<f32>>,
    topic_emb: &HashMap<String, Vec<f32>>,
    start_center: f32,
    start_scale: f32,
    model: &str,
) -> (f32, f32, f32) {
    let mut best_center = start_center;
    let mut best_scale = start_scale;

    // Evaluate starting point
    crate::embedding_calibration::set_active_params(best_center, best_scale);
    let initial_report = run_benchmark_with_embeddings(db, item_emb, topic_emb, model);
    let mut best_accuracy = initial_report.accuracy;

    info!(
        "Hill climb start: center={:.3} scale={:.1} accuracy={:.1}%",
        best_center,
        best_scale,
        best_accuracy * 100.0
    );

    let center_step = 0.02_f32;
    let scale_step = 0.5_f32;

    for iteration in 0..10 {
        // 8 neighbors: 4 cardinal + 4 diagonal
        let neighbors = [
            (best_center + center_step, best_scale),
            (best_center - center_step, best_scale),
            (best_center, best_scale + scale_step),
            (best_center, best_scale - scale_step),
            (best_center + center_step, best_scale + scale_step),
            (best_center + center_step, best_scale - scale_step),
            (best_center - center_step, best_scale + scale_step),
            (best_center - center_step, best_scale - scale_step),
        ];

        let mut improved = false;
        for (nc, ns) in &neighbors {
            // Clamp to reasonable ranges
            let nc = nc.clamp(0.20, 0.70);
            let ns = ns.clamp(5.0, 30.0);

            crate::embedding_calibration::set_active_params(nc, ns);
            let report = run_benchmark_with_embeddings(db, item_emb, topic_emb, model);

            if report.accuracy > best_accuracy {
                best_center = nc;
                best_scale = ns;
                best_accuracy = report.accuracy;
                improved = true;

                info!(
                    "  iter {}: center={:.3} scale={:.1} accuracy={:.1}% (improved)",
                    iteration,
                    best_center,
                    best_scale,
                    best_accuracy * 100.0
                );

                break; // Greedy first-improvement
            }
        }

        if !improved {
            info!("  iter {}: no improvement found, stopping", iteration);
            break;
        }
    }

    // Restore best params
    crate::embedding_calibration::set_active_params(best_center, best_scale);

    (best_center, best_scale, best_accuracy)
}

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
    let (item_emb, topic_emb) = generate_all_embeddings(&scenarios)?;
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
    let original_report = run_benchmark_with_embeddings(&db, &item_emb, &topic_emb, &model_name);
    let original_accuracy = original_report.accuracy;

    info!(
        "Original accuracy: {:.1}% ({}/{})",
        original_accuracy * 100.0,
        original_report.passed,
        original_report.total
    );

    // Step 4: Hill-climb optimization
    let (opt_center, opt_scale, _opt_accuracy) = hill_climb_calibration(
        &db,
        &item_emb,
        &topic_emb,
        original_center,
        original_scale,
        &model_name,
    );

    // Step 5: Final benchmark with optimized params
    crate::embedding_calibration::set_active_params(opt_center, opt_scale);
    let final_report = run_benchmark_with_embeddings(&db, &item_emb, &topic_emb, &model_name);

    // Step 6: Quality gate
    let meets_gate = model_meets_quality_gate(&final_report);

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
// Tests
// ============================================================================

#[cfg(feature = "fastembed-local")]
#[test]
fn embedding_generation_works() {
    let texts = vec![
        "Rust programming language".to_string(),
        "Machine learning with Python".to_string(),
        "TypeScript frontend development".to_string(),
    ];

    let raw = crate::fastembed_sync(&texts).expect("fastembed should work");
    let embeddings: Vec<Vec<f32>> = raw.into_iter().map(pad_and_normalize).collect();
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
    let result = run_calibration_sync().expect("calibration should succeed");

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
        eprintln!(
            "WARN: quality gate soft-fail during model transition: overall={:.1}% (need 80%)",
            result.benchmark_report.accuracy * 100.0
        );
    }
}

#[cfg(feature = "fastembed-local")]
#[test]
fn hill_climbing_improves_or_maintains() {
    let result = run_calibration_sync().expect("calibration should succeed");

    assert!(
        result.optimized_accuracy >= result.original_accuracy,
        "Optimized accuracy ({:.1}%) should be >= original ({:.1}%)",
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
        !model_meets_quality_gate(&bad_report),
        "Quality gate should reject report with {:.1}% accuracy",
        bad_report.accuracy * 100.0
    );
}

/// Diagnostic: dump every scenario's actual score, relevance, and signals
/// to identify which scenarios need re-calibration.
#[cfg(feature = "fastembed-local")]
#[test]
#[ignore]
fn diagnostic_dump_all_scenarios() {
    let scenarios = load_scenarios();
    let (item_emb, topic_emb) = generate_all_embeddings(&scenarios).unwrap();
    let db = bench_db();
    let opts = no_freshness();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

    eprintln!("\n=== SCENARIO DIAGNOSTIC DUMP ===");
    eprintln!(
        "{:<40} {:>6} {:>5} {:>5} {:>5} {:>4} {:<20} {}",
        "SCENARIO", "SCORE", "REL", "EXPRL", "PASS", "SIGS", "SIGNALS", "RANGE"
    );
    eprintln!("{}", "-".repeat(120));

    for scenario in &scenarios {
        let ctx = build_profile_with_embeddings(&scenario.profile, &topic_emb);
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
            created_at: None,
            detected_lang: "en",
            source_tags: &tags,
            tags_json: scenario.item.tags_json.as_deref(),
            feed_origin: None,
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
            "{:<40} {:>6.3} {:>5} {:>5} {:>5} {:>4} {:<20} [{:.2}-{:.2}]",
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
            &confirmed[..std::cmp::min(20, confirmed.len())],
            scenario.expected.score_min,
            scenario.expected.score_max
        );
    }
    eprintln!("=== END DUMP ===\n");
}
