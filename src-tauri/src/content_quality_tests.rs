// SPDX-License-Identifier: FSL-1.1-Apache-2.0

use super::*;

#[test]
fn test_clickbait_penalty() {
    let q = assess_title_quality("You Won't Believe What Rust Can Do!!!");
    assert!(q <= 0.5, "Clickbait should score low: {}", q);
}

#[test]
fn test_technical_title_quality() {
    let q = assess_title_quality("Tokio 1.34 release: new task scheduling improvements");
    assert!(q > 0.8, "Technical title should score high: {}", q);
}

#[test]
fn test_all_caps_penalty() {
    let q = assess_title_quality("BREAKING: EVERYTHING IS BROKEN");
    assert!(q < 0.8, "ALL CAPS should be penalized: {}", q);
}

#[test]
fn test_content_depth_empty() {
    let d = assess_content_depth("");
    assert_eq!(d, 0.3);
}

#[test]
fn test_content_depth_with_code() {
    let content = "Here is how to use it:\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\nThis function prints hello to stdout.";
    let d = assess_content_depth(content);
    assert!(
        d > 0.3,
        "Content with code should score higher than baseline: {}",
        d
    );
}

#[test]
fn test_source_authority() {
    assert!(assess_source_authority("https://github.com/rust-lang/rust") > 1.0);
    assert!(assess_source_authority("https://medium.com/some-article") < 1.0);
    assert_eq!(assess_source_authority("https://example.com/post"), 1.0);
}

#[test]
fn test_quality_multiplier_range() {
    let high = compute_content_quality(
        "Tokio v1.34: task scheduling",
        "Long technical content with ```code blocks``` and detailed analysis...",
        Some("https://github.com/tokio-rs/tokio"),
    );
    assert!(
        high.multiplier >= 0.5 && high.multiplier <= 1.2,
        "Multiplier out of range: {}",
        high.multiplier
    );

    let low = compute_content_quality(
        "You Won't Believe This INSANE Trick!!!",
        "",
        Some("https://clickbait.com"),
    );
    assert!(
        low.multiplier < high.multiplier,
        "Low quality should have lower multiplier"
    );
}

#[test]
fn test_short_vague_title_penalty() {
    let vague = assess_title_quality("where to start");
    let specific = assess_title_quality("Building REST APIs with Axum and Tokio");
    assert!(
        vague < specific,
        "Vague title ({}) should score lower than specific ({})",
        vague,
        specific
    );
    assert!(
        vague <= 0.7,
        "Vague 3-word title should be penalized: {}",
        vague
    );
}

// ====================================================================
// assess_information_density tests
// ====================================================================

#[test]
fn test_info_density_version_numbers() {
    let dense = assess_information_density("React v19 released with server components");
    let vague = assess_information_density("New React features released");
    assert!(
        dense > vague,
        "Version numbers should boost density: {} vs {}",
        dense,
        vague
    );
}

#[test]
fn test_info_density_quantified_claims() {
    let dense = assess_information_density("Built a Julia IDE in 10MB install");
    let vague = assess_information_density("Built a Julia IDE");
    assert!(
        dense > vague,
        "Quantified claims should boost density: {} vs {}",
        dense,
        vague
    );
}

#[test]
fn test_info_density_vague_penalty() {
    let specific = assess_information_density("SQLite migration guide for breaking changes");
    let vague = assess_information_density("Interesting thoughts on databases anyone else");
    assert!(
        specific > vague,
        "Vague titles should score lower: {} vs {}",
        specific,
        vague
    );
}

#[test]
fn test_info_density_comparison_boost() {
    let dense = assess_information_density("Bun vs Node.js benchmark throughput comparison");
    let plain = assess_information_density("Testing Bun and Node.js together");
    assert!(
        dense > plain,
        "Comparison content should boost density: {} vs {}",
        dense,
        plain
    );
}

#[test]
fn test_info_density_range() {
    let high = assess_information_density(
        "React v19.1 benchmark: 100x faster rendering migration changelog",
    );
    let low = assess_information_density("thoughts on stuff is it just me hot take");
    assert!(high <= 1.0 && high >= 0.0, "Should be in range: {}", high);
    assert!(low <= 1.0 && low >= 0.0, "Should be in range: {}", low);
}

// ====================================================================
// Anti-gaming defense tests
// ====================================================================

// --- keyword_concentration_penalty ---

#[test]
fn test_keyword_concentration_no_repeats() {
    let p = keyword_concentration_penalty("Building REST APIs with Axum and Tokio");
    assert_eq!(p, 0.0, "No repeats should have zero penalty: {}", p);
}

#[test]
fn test_keyword_concentration_two_repeats_allowed() {
    // 2 repeats is legitimate: "React vs React Native"
    let p = keyword_concentration_penalty("Rust patterns in Rust systems programming");
    assert_eq!(p, 0.0, "2 repeats should be allowed (legitimate): {}", p);
}

#[test]
fn test_keyword_concentration_three_repeats() {
    let p = keyword_concentration_penalty("Rust async Rust patterns async Rust");
    assert!(
        (p - (-0.10)).abs() < f32::EPSILON,
        "3 repeats of 'rust' should give -0.10: {}",
        p
    );
}

#[test]
fn test_keyword_concentration_four_repeats() {
    let p = keyword_concentration_penalty("Rust Rust Rust Rust framework");
    assert!(
        (p - (-0.20)).abs() < f32::EPSILON,
        "4 repeats should give -0.20: {}",
        p
    );
}

#[test]
fn test_keyword_concentration_stop_words_exempt() {
    // "this" and "with" are stop words — repeated but should not trigger
    let p = keyword_concentration_penalty("this with this with this with");
    assert_eq!(
        p, 0.0,
        "Stop word repeats should not trigger penalty: {}",
        p
    );
}

#[test]
fn test_keyword_concentration_short_words_exempt() {
    // Words under 4 chars should be ignored
    let p = keyword_concentration_penalty("the API API API use for fun");
    assert_eq!(p, 0.0, "Short word repeats should not trigger: {}", p);
}

// --- title_body_coherence_penalty ---

#[test]
fn test_coherence_good_match() {
    let p = title_body_coherence_penalty(
        "Building React apps with Tauri and Rust",
        "This article covers building React applications using the Tauri framework powered by Rust.",
    );
    assert_eq!(
        p, 0.0,
        "Good title-body match should have no penalty: {}",
        p
    );
}

#[test]
fn test_coherence_poor_match() {
    let p = title_body_coherence_penalty(
        "React Rust Tauri performance benchmarks",
        "Today we discuss cooking recipes and gardening tips for beginners.",
    );
    assert!(p <= -0.10, "Title-body mismatch should be penalized: {}", p);
}

#[test]
fn test_coherence_empty_body_no_penalty() {
    let p = title_body_coherence_penalty("React Rust Tauri performance benchmarks", "");
    assert_eq!(p, 0.0, "Empty body should not trigger penalty: {}", p);
}

#[test]
fn test_coherence_few_title_words_no_penalty() {
    // Title with fewer than 3 significant words should not trigger
    let p = title_body_coherence_penalty(
        "Rust news",
        "Completely unrelated body content about cooking.",
    );
    assert_eq!(
        p, 0.0,
        "Short title should not trigger coherence check: {}",
        p
    );
}

// --- title_diversity_penalty ---

#[test]
fn test_diversity_normal_title() {
    let p = title_diversity_penalty("Building REST APIs with Axum and Tokio");
    assert_eq!(p, 0.0, "Normal diverse title should have no penalty: {}", p);
}

#[test]
fn test_diversity_keyword_soup() {
    // Every word repeated: diversity = 3/6 = 0.50 exactly, which is < 0.50? No, 0.50 is not < 0.50.
    // Need diversity strictly < 0.50 for -0.15
    let p = title_diversity_penalty("Rust Rust Rust React React React AI AI");
    // unique=3, total=8 → 0.375
    assert!(
        (p - (-0.15)).abs() < f32::EPSILON,
        "Low diversity keyword soup should get -0.15: {}",
        p
    );
}

#[test]
fn test_diversity_moderate_repetition_allowed() {
    // Legitimate comparative titles repeat terms naturally.
    // "Rust patterns Rust async patterns Rust async guide"
    // unique: rust, patterns, async, guide = 4, total = 8 → 0.50
    // 0.50 is NOT < 0.50, so no penalty (mild tier removed).
    let p = title_diversity_penalty("Rust patterns Rust async patterns Rust async guide");
    assert_eq!(
        p, 0.0,
        "0.50 diversity should NOT be penalized (legitimate repetition): {}",
        p
    );
}

#[test]
fn test_diversity_empty_title() {
    let p = title_diversity_penalty("");
    assert_eq!(p, 0.0, "Empty title should have no penalty: {}", p);
}

// --- Integration test: anti-gaming titles get lower multiplier ---

#[test]
fn test_gaming_title_lower_than_genuine() {
    let genuine = compute_content_quality(
        "How we migrated our PostgreSQL database to CockroachDB",
        "This article describes our migration from PostgreSQL to CockroachDB including schema changes and performance results.",
        None,
    );
    let gamed = compute_content_quality(
        "Rust Rust Rust AI AI AI Docker Docker Docker",
        "A short post about nothing in particular.",
        None,
    );
    assert!(
        gamed.multiplier < genuine.multiplier,
        "Gamed title ({}) should score lower than genuine ({})",
        gamed.multiplier,
        genuine.multiplier
    );
}

#[test]
fn test_gaming_multiplier_still_in_range() {
    let q = compute_content_quality("Rust Rust Rust Rust async Rust performance Rust", "", None);
    assert!(
        q.multiplier >= 0.5 && q.multiplier <= 1.2,
        "Multiplier must stay in [0.5, 1.2]: {}",
        q.multiplier
    );
}
