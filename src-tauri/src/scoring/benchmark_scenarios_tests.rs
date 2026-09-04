// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Scenario-benchmark tests, split out of `benchmark_scenarios.rs` (v29) so
//! the harness stays under the file-size gate. `use super::*` sees every
//! private item of the parent (loader, profiles, runner, pass rule).

use super::*;
use crate::scoring::benchmark::bench_db;

// ============================================================================
// Tests
// ============================================================================

const VALID_PROFILES: &[&str] = &[
    "rust_developer",
    "fullstack_js",
    "python_data_scientist",
    "minimal",
];

#[test]
fn scenarios_parse_correctly() {
    let scenarios = load_scenarios();
    // 87 pipeline scenarios + the v29 verdict fence (12 pinned-verdict cases
    // from the 2026-09-04 live audit — ten false positives and two twins —
    // category `verdict_fence`).
    assert_eq!(
        scenarios.len(),
        99,
        "Expected 99 scenarios, got {}",
        scenarios.len()
    );
    for s in &scenarios {
        assert!(!s.id.is_empty(), "Scenario has empty id");
        assert!(
            !s.category.is_empty(),
            "Scenario {} has empty category",
            s.id
        );
    }
}

#[test]
fn scenarios_have_valid_profiles() {
    let scenarios = load_scenarios();
    for s in &scenarios {
        assert!(
            VALID_PROFILES.contains(&s.profile.as_str()),
            "Scenario {} uses unknown profile '{}'",
            s.id,
            s.profile,
        );
    }
}

#[test]
fn scenarios_have_valid_score_ranges() {
    let scenarios = load_scenarios();
    for s in &scenarios {
        assert!(
            s.expected.score_min < s.expected.score_max,
            "Scenario {} has score_min ({}) >= score_max ({})",
            s.id,
            s.expected.score_min,
            s.expected.score_max,
        );
        assert!(
            s.expected.score_min >= 0.0 && s.expected.score_min <= 1.0,
            "Scenario {} has score_min {} outside [0,1]",
            s.id,
            s.expected.score_min,
        );
        assert!(
            s.expected.score_max >= 0.0 && s.expected.score_max <= 1.0,
            "Scenario {} has score_max {} outside [0,1]",
            s.id,
            s.expected.score_max,
        );
    }
}

/// Structural regression gate for the scoring pipeline. **Not** an accuracy
/// measurement — read the caveat before trusting a number from it.
///
/// It was `#[ignore]`d pending "re-baseline after Arctic-M real embeddings
/// replace synthetic test vectors", and stayed off long enough for a dependency
/// axis that was 75% phantom to ship undetected (2026-08-26 audit). Re-measured
/// 2026-08-27 it passes its own 0.75 bar untouched at 80.5% (70/87), and it
/// produces byte-identical results with and without that audit's four scoring
/// fixes — so it is stable enough to gate regressions, and it is ON again,
/// because a gate that is switched off is worth exactly what one that does not
/// exist is worth.
///
/// THE CAVEAT, and the reason the original `#[ignore]` was not merely stale:
/// [`run_benchmark`] scores every scenario with a ZERO embedding. That puts the
/// whole run in the documented degraded state — `embedding_missing` — where the
/// context axis cannot run at all and the semantic ACE boost falls back to
/// keywords. Every current failure is `signals=1 [ace]`, held under the
/// 1-signal confirmation ceiling (0.28), and that is an artefact of the zero
/// vector as much as anything about relevance. Do not read `true_positive
/// 11/20` as a live recall figure.
///
/// What this test CAN do: fail when a change moves the structural floor. What
/// it CANNOT do: notice that the dependency axis is 75% phantom — it did not,
/// for months. Live accuracy is measured against real judgments; see
/// `scoring::judge_agreement_live`.
#[test]
fn benchmark_scoring_accuracy() {
    let db = bench_db();
    let report = run_benchmark(&db);

    println!(
        "PASIFA benchmark: score-range accuracy {:.1}% ({}/{}), relevance accuracy {:.1}%",
        report.accuracy * 100.0,
        report.passed,
        report.total,
        report.relevance_accuracy * 100.0,
    );
    let mut cats: Vec<_> = report.by_category.iter().collect();
    cats.sort_by_key(|(k, _)| k.as_str());
    for (cat, r) in cats {
        println!("  {cat:<22} {}/{}", r.passed, r.total);
    }
    for f in report.failures.iter().take(20) {
        println!(
            "  FAIL {:<26} score={:.3} signals={} [{}] expected_relevant={} actual={}",
            f.scenario_id,
            f.actual_score,
            f.signal_count,
            f.confirmed_signals.join(","),
            f.expected_relevant,
            f.actual_relevant
        );
    }

    assert!(
        report.accuracy >= 0.75,
        "Overall accuracy {:.1}% < 75% threshold ({} of {} passed)",
        report.accuracy * 100.0,
        report.passed,
        report.total,
    );
}

/// Print every scenario's measured score and verdict — the tool for setting
/// a band FROM measurement instead of by hand. Run with
/// `cargo test --lib dump_scenario_scores -- --ignored --nocapture`.
#[test]
#[ignore = "diagnostic dump: run explicitly with --ignored --nocapture"]
fn dump_scenario_scores() {
    let db = bench_db();
    let scenarios = load_scenarios();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];
    for (i, scenario) in scenarios.iter().enumerate() {
        let ctx = profile_ctx(&scenario.profile);
        let opts = scenario_options(scenario);
        let tags: Vec<String> = scenario
            .item
            .tags_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();
        let created_at = scenario_created_at(scenario);
        let input = ScoringInput {
            id: (i + 1) as u64,
            title: &scenario.item.title,
            url: Some("https://example.com"),
            content: &scenario.item.content,
            source_type: &scenario.item.source_type,
            embedding: &zero_emb,
            created_at: created_at.as_ref(),
            detected_lang: "en",
            source_tags: &tags,
            tags_json: scenario.item.tags_json.as_deref(),
            feed_origin: None,
            source_id: scenario.item.source_id.as_deref(),
        };
        let result = score_item(&input, &ctx, &db, &opts, None);
        let signals = result
            .score_breakdown
            .as_ref()
            .map(|b| b.confirmed_signals.join(","))
            .unwrap_or_default();
        println!(
            "SCENARIO {:<44} {:<16} score={:.3} relevant={:<5} expected_relevant={:<5} band=[{:.2},{:.2}] pinned={} signals=[{}]",
            scenario.id,
            scenario.category,
            result.top_score,
            result.relevant,
            scenario.expected.should_be_relevant,
            scenario.expected.score_min,
            scenario.expected.score_max,
            scenario.verdict_pinned,
            signals
        );
    }
}

#[test]
fn cold_start_scores_have_spread() {
    let scenarios = load_scenarios();
    let cold_start: Vec<&Scenario> = scenarios
        .iter()
        .filter(|s| s.category == "cold_start")
        .collect();

    assert!(
        cold_start.len() >= 5,
        "Need at least 5 cold_start scenarios, got {}",
        cold_start.len()
    );

    let db = bench_db();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

    let mut scores = Vec::new();
    for scenario in &cold_start {
        let ctx = profile_ctx(&scenario.profile);
        let opts = scenario_options(scenario);
        let tags: Vec<String> = scenario
            .item
            .tags_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();
        let tags_json_ref = scenario.item.tags_json.as_deref();
        let created_at = scenario_created_at(scenario);

        let input = ScoringInput {
            id: 1,
            title: &scenario.item.title,
            url: Some("https://example.com"),
            content: &scenario.item.content,
            source_type: &scenario.item.source_type,
            embedding: &zero_emb,
            created_at: created_at.as_ref(),
            detected_lang: "en",
            source_tags: &tags,
            tags_json: tags_json_ref,
            feed_origin: None,
            source_id: scenario.item.source_id.as_deref(),
        };

        let result = score_item(&input, &ctx, &db, &opts, None);
        scores.push(result.top_score);
    }

    // Verify non-uniformity: not all scores are identical
    let min = scores.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let spread = max - min;

    assert!(
        spread > 0.01,
        "Cold start scores are uniform (spread={:.4}), expected variation. Scores: {:?}",
        spread,
        scores,
    );
}

/// L1b regression: a CVE confirmed against the user's DIRECT dependency must
/// score clearly relevant (>= the direct-dep fast-path floor), NOT sit at the
/// bare 0.50 generic floor — the flagship preemption case. Hermetic: zero
/// embedding (the floor is additive, so it fires without embedding signal);
/// deps injected the way production's ACE records them.
#[test]
fn direct_dep_cve_clears_the_direct_dep_floor() {
    fn mk_dep(name: &str) -> crate::scoring::dependencies::DepInfo {
        crate::scoring::dependencies::DepInfo {
            package_name: name.to_string(),
            version: None,
            is_dev: false,
            is_direct: true,
            search_terms: crate::scoring::dependencies::extract_search_terms(name),
            ecosystem: "rust".to_string(),
            project_paths: Vec::new(),
            project_relevance: 1.0,
        }
    }

    let db = bench_db();
    let opts = no_freshness();
    let zero_emb = vec![0.0_f32; crate::EMBEDDING_DIMS];

    let mut ctx = rust_developer_ctx();
    ctx.ace_ctx
        .dependency_info
        .insert("hyper".to_string(), mk_dep("hyper"));
    ctx.ace_ctx.dependency_names.insert("hyper".to_string());

    // A pure-dep-signal CVE (no topic/interest overlap, zero embedding) — exactly
    // the case that floored at 0.50 before L1b.
    let tags = vec!["security".to_string(), "cve".to_string()];
    let input = ScoringInput {
        id: 1,
        title: "CVE-2026-5678: hyper HTTP/2 CONTINUATION frame flood",
        url: Some("https://example.com"),
        content: "A vulnerability in the hyper crate's HTTP/2 handling allows a CONTINUATION frame flood.",
        source_type: "cve",
        embedding: &zero_emb,
        created_at: None,
        detected_lang: "en",
        source_tags: &tags,
        tags_json: None,
        feed_origin: None,
        source_id: None,
    };
    let direct = score_item(&input, &ctx, &db, &opts, None).top_score;

    // An unrelated CVE (package NOT a dependency) must still score low — the
    // floor only lifts CONFIRMED direct-dep matches.
    let unrelated_input = ScoringInput {
        id: 2,
        title: "CVE-2026-88888: php-curl remote code execution in cURL wrapper",
        url: Some("https://example.com"),
        content: "A PHP cURL wrapper vulnerability allows remote code execution.",
        source_type: "cve",
        embedding: &zero_emb,
        created_at: None,
        detected_lang: "en",
        source_tags: &tags,
        tags_json: None,
        feed_origin: None,
        source_id: None,
    };
    let unrelated = score_item(&unrelated_input, &ctx, &db, &opts, None).top_score;

    assert!(
        direct >= 0.60,
        "direct-dep CVE must clear the direct-dep floor (~0.65), got {direct:.3}"
    );
    assert!(
        unrelated < 0.30,
        "unrelated CVE (php-curl, not a dep) must stay low, got {unrelated:.3}"
    );
}
