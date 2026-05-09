// SPDX-License-Identifier: FSL-1.1-Apache-2.0

use super::*;
use std::collections::HashSet;

const SENIOR_TECH_COUNT: usize = 25;
const JUNIOR_TECH_COUNT: usize = 2;

fn mock_domain_senior() -> DomainProfile {
    let mut dp = DomainProfile::default();
    dp.all_tech = (0..20).map(|i| format!("tech_{i}")).collect();
    dp.dependency_names = (0..60).map(|i| format!("dep_{i}")).collect();
    dp
}

fn mock_domain_junior() -> DomainProfile {
    let mut dp = DomainProfile::default();
    dp.all_tech = ["react".to_string()].into_iter().collect();
    dp.dependency_names = ["react".to_string(), "vite".to_string()]
        .into_iter()
        .collect();
    dp
}

#[test]
fn beginner_title_scores_low() {
    let score = assess_title_complexity("Getting Started with React - A Beginner's Guide");
    assert!(score < 0.3, "Expected <0.3, got {score}");
}

#[test]
fn advanced_title_scores_high() {
    let score = assess_title_complexity(
        "Lock-Free CRDT Consensus in Distributed Systems with Causal Consistency",
    );
    assert!(score > 0.6, "Expected >0.6, got {score}");
}

#[test]
fn moderate_title_scores_middle() {
    let score = assess_title_complexity("Building a Plugin System for a Tauri App");
    assert!(
        (0.2..=0.7).contains(&score),
        "Expected 0.2-0.7, got {score}"
    );
}

#[test]
fn senior_audience_penalizes_beginner_content() {
    let dp = mock_domain_senior();
    let result =
        compute_sophistication("Getting Started with React", "", SENIOR_TECH_COUNT, &dp);
    assert!(
        result.multiplier <= 0.60,
        "Expected <=0.60, got {}",
        result.multiplier
    );
    assert!(result.audience_is_senior);
}

#[test]
fn senior_audience_boosts_advanced_content() {
    let dp = mock_domain_senior();
    let result = compute_sophistication(
        "Zero-Copy Deserialization with Lifetime Elision in Rust",
        "impl<'a> Deserialize<'a> for MyStruct { fn deserialize() -> Result<Self> { ... } }",
        SENIOR_TECH_COUNT,
        &dp,
    );
    assert!(
        result.multiplier >= 1.10,
        "Expected >=1.10, got {}",
        result.multiplier
    );
}

#[test]
fn junior_audience_mild_beginner_penalty() {
    let dp = mock_domain_junior();
    let result =
        compute_sophistication("Getting Started with React", "", JUNIOR_TECH_COUNT, &dp);
    assert!(
        result.multiplier >= 0.85,
        "Junior should get mild penalty, got {}",
        result.multiplier
    );
}

#[test]
fn code_depth_with_error_handling_scores_higher() {
    let simple = assess_content_depth_signals("console.log('hello world');");
    let complex = assess_content_depth_signals(
        "async fn process() -> Result<Data> { let conn = pool.get().await.map_err(|e| ...); }",
    );
    assert!(
        complex > simple,
        "Complex ({complex}) should > simple ({simple})"
    );
}

#[test]
fn multiplier_range_is_valid() {
    for soph in [0.0, 0.15, 0.3, 0.5, 0.7, 0.85, 1.0] {
        for senior in [true, false] {
            let m = compute_multiplier(soph, senior);
            assert!(
                (0.60..=1.15).contains(&m),
                "Out of range: soph={soph} senior={senior} m={m}"
            );
        }
    }
}

#[test]
fn typing_master_tutorial_scores_low() {
    let result = compute_sophistication(
        "Typing Master Web App React + Vite -- Full Project Breakdown",
        "Step 1: Create a new Vite project. npm install. Step 2: Create components.",
        SENIOR_TECH_COUNT,
        &mock_domain_senior(),
    );
    assert!(
        result.multiplier <= 0.60,
        "Tutorial penalized for senior, got {}",
        result.multiplier
    );
}

#[test]
fn bootstrapping_senior_multi_stack() {
    let profile = DomainProfile {
        all_tech: HashSet::from([
            "react".to_string(),
            "express".to_string(),
            "rust".to_string(),
            "sqlite".to_string(),
        ]),
        dependency_names: HashSet::new(),
        ..Default::default()
    };
    // 4 families: frontend, backend, systems, data
    assert!(infer_senior_audience(5, &profile));
}

#[test]
fn single_family_not_senior() {
    let profile = DomainProfile {
        all_tech: HashSet::from([
            "react".to_string(),
            "typescript".to_string(),
            "next".to_string(),
            "tailwind".to_string(),
        ]),
        dependency_names: HashSet::new(),
        ..Default::default()
    };
    // All frontend = 1 family
    assert!(!infer_senior_audience(5, &profile));
}

#[test]
fn three_families_is_senior() {
    let profile = DomainProfile {
        all_tech: HashSet::from([
            "python".to_string(),
            "django".to_string(),
            "postgresql".to_string(),
            "docker".to_string(),
        ]),
        dependency_names: HashSet::new(),
        ..Default::default()
    };
    // backend, data, devops = 3 families
    assert!(infer_senior_audience(5, &profile));
}

#[test]
fn too_few_detected_tech_not_senior() {
    let profile = DomainProfile {
        all_tech: HashSet::from([
            "react".to_string(),
            "rust".to_string(),
            "sqlite".to_string(),
        ]),
        dependency_names: HashSet::new(),
        ..Default::default()
    };
    // 3 families but only 3 detected_tech (< 5 minimum)
    assert!(!infer_senior_audience(3, &profile));
}
