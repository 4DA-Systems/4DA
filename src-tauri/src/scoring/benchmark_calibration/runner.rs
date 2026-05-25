// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Benchmark runner with real embeddings for calibration.

use std::collections::HashMap;

use super::profile::build_profile_with_embeddings;
use super::{
    load_scenarios, no_freshness, score_item, BenchmarkFailure, BenchmarkReport, CategoryResult,
    ScoringInput,
};

pub(super) fn run_benchmark_with_embeddings(
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
