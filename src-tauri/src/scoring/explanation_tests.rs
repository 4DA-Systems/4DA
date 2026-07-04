// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::*;

#[test]
fn test_extract_short_phrase_long_text() {
    let phrase =
        extract_short_phrase("Vector search implementation using sqlite-vss for fast KNN queries");
    assert!(phrase.contains("Vector search"));
    assert!(!phrase.is_empty());
}

#[test]
fn test_extract_short_phrase_short_text() {
    let phrase = extract_short_phrase("short");
    assert!(phrase.is_empty()); // Too short to be useful
}

#[test]
fn test_temporal_freshness_very_recent() {
    let now = chrono::Utc::now();
    let freshness = compute_temporal_freshness(&now);
    assert_eq!(freshness, 1.10, "Items just created should get max boost");
}

#[test]
fn test_temporal_freshness_few_hours() {
    let four_hours_ago = chrono::Utc::now() - chrono::Duration::hours(4);
    let freshness = compute_temporal_freshness(&four_hours_ago);
    assert_eq!(freshness, 1.08, "Items 4h old should get 1.08x boost");
}

#[test]
fn test_temporal_freshness_half_day() {
    let thirteen_hours_ago = chrono::Utc::now() - chrono::Duration::hours(13);
    let freshness = compute_temporal_freshness(&thirteen_hours_ago);
    assert_eq!(freshness, 1.05, "Items 13h old should get 1.05x boost");
}

#[test]
fn test_temporal_freshness_one_day() {
    let thirty_hours_ago = chrono::Utc::now() - chrono::Duration::hours(30);
    let freshness = compute_temporal_freshness(&thirty_hours_ago);
    assert_eq!(freshness, 1.0, "Items 30h old should be neutral");
}

#[test]
fn test_temporal_freshness_old() {
    let four_days_ago = chrono::Utc::now() - chrono::Duration::hours(96);
    let freshness = compute_temporal_freshness(&four_days_ago);
    assert_eq!(freshness, 0.92, "Items 4 days old should decay to 0.92");
}

#[test]
fn test_temporal_freshness_very_old() {
    let old = chrono::Utc::now() - chrono::Duration::hours(200);
    let freshness = compute_temporal_freshness(&old);
    assert_eq!(freshness, 0.85, "Items 8+ days old should decay to 0.85");
}

// ====================================================================
// extract_short_phrase additional tests
// ====================================================================

#[test]
fn test_extract_short_phrase_with_period() {
    let phrase = extract_short_phrase(
        "Vector search is powerful. It enables fast nearest neighbor lookups.",
    );
    // Should stop at the first period
    assert!(phrase.contains("Vector search"));
    assert!(!phrase.contains("enables"));
}

#[test]
fn test_extract_short_phrase_with_newline() {
    let phrase = extract_short_phrase(
        "Async runtime improvements\nThe new version includes better scheduling",
    );
    // Should stop at the newline
    assert!(phrase.contains("Async runtime"));
    assert!(!phrase.contains("new version"));
}

#[test]
fn test_extract_short_phrase_with_ellipsis() {
    let phrase = extract_short_phrase("A long context about development practices...");
    assert!(!phrase.ends_with("..."));
}

#[test]
fn test_extract_short_phrase_strips_markdown_markers() {
    // Leading list bullet must not leak into the quoted snippet.
    assert_eq!(
        extract_short_phrase("- Built with Claude Code and agents"),
        "Built with Claude Code and agents"
    );
    // Leading bold emphasis ("**Why ...").
    assert_eq!(
        extract_short_phrase("**Why this matters for your stack"),
        "Why this matters for your stack"
    );
    // Blockquote + heading markers.
    assert_eq!(
        extract_short_phrase("> ## Important architectural note here"),
        "Important architectural note here"
    );
    // snake_case identifiers must survive — no mid-token underscore stripping.
    let phrase = extract_short_phrase("uses anthropic_ai_sdk for the integration");
    assert!(phrase.contains("anthropic_ai_sdk"), "got: {phrase}");
}

#[test]
fn test_extract_short_phrase_too_short_returns_empty() {
    assert!(extract_short_phrase("tiny").is_empty());
    assert!(extract_short_phrase("ab").is_empty());
    assert!(extract_short_phrase("").is_empty());
}

#[test]
fn test_extract_short_phrase_exactly_ten_chars() {
    // 10 chars should be included
    let phrase = extract_short_phrase("abcdefghij");
    assert_eq!(phrase, "abcdefghij");
}

#[test]
fn test_extract_short_phrase_nine_chars_empty() {
    // 9 chars should be too short
    let phrase = extract_short_phrase("abcdefghi");
    assert!(phrase.is_empty());
}

// ====================================================================
// calculate_confidence tests
// ====================================================================

#[test]
fn test_calculate_confidence_no_signals() {
    let ctx = ACEContext::default();
    let confidence = calculate_confidence(0.0, 0.0, 0.0, &ctx, &[], 0, 0, 0);
    assert_eq!(confidence, scoring_config::CONFIDENCE_FLOOR_NO_SIGNAL);
}

#[test]
fn test_calculate_confidence_context_only() {
    let ctx = ACEContext::default();
    let confidence = calculate_confidence(0.8, 0.0, 0.0, &ctx, &[], 10, 0, 1);
    assert!(confidence > scoring_config::CONFIDENCE_FLOOR_NO_SIGNAL);
}

#[test]
fn test_calculate_confidence_higher_confirmation_boosts() {
    let ctx = ACEContext::default();
    let conf_1 = calculate_confidence(0.5, 0.5, 0.0, &ctx, &[], 10, 5, 1);
    let conf_3 = calculate_confidence(0.5, 0.5, 0.0, &ctx, &[], 10, 5, 3);
    assert!(
        conf_3 > conf_1,
        "More confirmed signals should increase confidence: {} > {}",
        conf_3,
        conf_1
    );
}

#[test]
fn test_calculate_confidence_clamped() {
    let ctx = ACEContext::default();
    let confidence = calculate_confidence(1.0, 1.0, 1.0, &ctx, &[], 100, 100, 5);
    assert!(confidence <= 1.0, "Confidence should not exceed 1.0");
    assert!(confidence >= 0.0, "Confidence should not be negative");
}

#[test]
fn test_calculate_confidence_with_topic_affinities() {
    let mut ctx = ACEContext::default();
    ctx.topic_affinities.insert("rust".to_string(), (0.8, 0.9));
    let topics = vec!["rust".to_string()];
    let confidence = calculate_confidence(0.5, 0.0, 0.0, &ctx, &topics, 10, 0, 2);
    // Should be higher than without affinities since we have an additional signal
    let conf_no_aff =
        calculate_confidence(0.5, 0.0, 0.0, &ACEContext::default(), &topics, 10, 0, 2);
    assert!(
        confidence >= conf_no_aff,
        "Topic affinities should boost or maintain confidence"
    );
}

// ====================================================================
// generate_relevance_explanation tests
// ====================================================================

#[test]
fn test_generate_explanation_declared_tech() {
    let ace_ctx = ACEContext {
        detected_tech: vec!["rust".to_string()],
        tech_weights: std::collections::HashMap::new(),
        ..Default::default()
    };
    let explanation = generate_relevance_explanation(
        "Rust Performance Tips",
        0.2,
        0.2,
        &[],
        &ace_ctx,
        &["rust".to_string()],
        &[],
        &["Rust".to_string()],
        &[],
        0,
    );
    assert!(
        explanation.contains("your stack"),
        "Should mention 'your stack': {}",
        explanation
    );
}

#[test]
fn test_generate_explanation_skill_gap_annotation() {
    let ace_ctx = ACEContext::default();
    let explanation = generate_relevance_explanation(
        "Getting started with Tokio async runtime",
        0.1,
        0.1,
        &[],
        &ace_ctx,
        &["tokio".to_string()],
        &[],
        &[],
        &["tokio".to_string()],
        0,
    );
    assert!(
        explanation.contains("Closes skill gap: tokio"),
        "Should annotate skill gap: {}",
        explanation
    );
}

#[test]
fn test_generate_explanation_skill_gap_with_stack() {
    let ace_ctx = ACEContext {
        detected_tech: vec!["rust".to_string()],
        tech_weights: std::collections::HashMap::new(),
        ..Default::default()
    };
    let explanation = generate_relevance_explanation(
        "Tokio and Rust async patterns",
        0.2,
        0.2,
        &[],
        &ace_ctx,
        &["rust".to_string(), "tokio".to_string()],
        &[],
        &["Rust".to_string()],
        &["tokio".to_string()],
        0,
    );
    assert!(
        explanation.contains("your stack"),
        "Should still show stack match: {}",
        explanation
    );
    assert!(
        explanation.contains("Closes skill gap: tokio"),
        "Should also show skill gap: {}",
        explanation
    );
}

#[test]
fn test_generate_explanation_declared_tech_with_version() {
    use crate::scoring::dependencies::DepInfo;
    let mut dep_info = std::collections::HashMap::new();
    dep_info.insert(
        "react".to_string(),
        DepInfo {
            package_name: "react".to_string(),
            version: Some("18.3.1".to_string()),
            is_dev: false,
            is_direct: true,
            search_terms: vec![],
            ecosystem: "javascript".to_string(),
        },
    );
    let ace_ctx = ACEContext {
        dependency_info: dep_info,
        ..Default::default()
    };
    let explanation = generate_relevance_explanation(
        "React 19 Features",
        0.2,
        0.2,
        &[],
        &ace_ctx,
        &["react".to_string()],
        &[],
        &["React".to_string()],
        &[],
        0,
    );
    assert!(
        explanation.contains("v18.3.1"),
        "Should include version: {}",
        explanation
    );
    assert!(
        explanation.contains("your stack"),
        "Should still mention stack: {}",
        explanation
    );
}

#[test]
fn test_generate_explanation_skill_gap_dedup_with_stack() {
    let ace_ctx = ACEContext {
        detected_tech: vec!["react".to_string()],
        ..Default::default()
    };
    let explanation = generate_relevance_explanation(
        "React Patterns",
        0.2,
        0.2,
        &[],
        &ace_ctx,
        &["react".to_string()],
        &[],
        &["React".to_string()],
        &["react".to_string()],
        0,
    );
    assert!(
        !explanation.contains("Closes skill gap: react"),
        "Should NOT repeat react as skill gap when already in stack: {}",
        explanation
    );
    assert!(
        explanation.contains("has unread updates"),
        "Should show 'has unread updates' annotation: {}",
        explanation
    );
}

#[test]
fn test_generate_explanation_empty_when_no_signals() {
    let ace_ctx = ACEContext::default();
    let explanation = generate_relevance_explanation(
        "Some Random Title",
        0.1,
        0.1,
        &[],
        &ace_ctx,
        &["random".to_string()],
        &[],
        &[],
        &[],
        0,
    );
    assert!(
        explanation.is_empty(),
        "Should be empty with no signals: '{}'",
        explanation
    );
}

#[test]
fn test_generate_explanation_signal_count_shown_at_3_plus() {
    let ace_ctx = ACEContext {
        detected_tech: vec!["rust".to_string()],
        ..Default::default()
    };
    let explanation = generate_relevance_explanation(
        "Rust Performance",
        0.2,
        0.2,
        &[],
        &ace_ctx,
        &["rust".to_string()],
        &[],
        &["Rust".to_string()],
        &[],
        3,
    );
    assert!(
        explanation.contains("3 signals confirmed"),
        "Should show signal count at 3+: {}",
        explanation
    );
}

#[test]
fn test_generate_explanation_signal_count_hidden_at_2() {
    let ace_ctx = ACEContext {
        detected_tech: vec!["rust".to_string()],
        ..Default::default()
    };
    let explanation = generate_relevance_explanation(
        "Rust Performance",
        0.2,
        0.2,
        &[],
        &ace_ctx,
        &["rust".to_string()],
        &[],
        &["Rust".to_string()],
        &[],
        2,
    );
    assert!(
        !explanation.contains("signals confirmed"),
        "Should NOT show signal count at 2: {}",
        explanation
    );
}

#[test]
fn test_generate_explanation_multi_reason() {
    let mut ace_ctx = ACEContext::default();
    ace_ctx.active_topics = vec!["testing".to_string()];
    let explanation = generate_relevance_explanation(
        "Rust Testing Frameworks",
        0.5,
        0.3,
        &[],
        &ace_ctx,
        &["rust".to_string(), "testing".to_string()],
        &[],
        &["Rust".to_string()],
        &[],
        0,
    );
    assert!(
        explanation.contains("your stack"),
        "Should have stack match: {}",
        explanation
    );
    assert!(
        explanation.contains("active project"),
        "Should also have active project match: {}",
        explanation
    );
}

#[test]
fn test_generate_explanation_dedups_declared_tech() {
    // Item topics "react" and "react-dom" both hit declared "react" — the
    // explanation must name it once, not "Uses react, react (your stack)".
    let ace_ctx = ACEContext::default();
    let explanation = generate_relevance_explanation(
        "React 19 and React DOM",
        0.2,
        0.1,
        &[],
        &ace_ctx,
        &["react".to_string(), "react-dom".to_string()],
        &[],
        &["react".to_string()],
        &[],
        0,
    );
    assert_eq!(
        explanation, "Uses react (your stack)",
        "duplicate declared-tech hits must be deduped: {explanation}"
    );
}

fn interest(topic: &str) -> context_engine::Interest {
    context_engine::Interest {
        id: None,
        topic: topic.to_string(),
        weight: 1.0,
        embedding: None,
        source: Default::default(),
    }
}

#[test]
fn test_interest_fragment_does_not_claim_match() {
    // Item topic "http" must NOT produce "Matches interest: tower-http" — the
    // full interest token has to occur in the item topic at a word boundary.
    let ace_ctx = ACEContext::default();
    let explanation = generate_relevance_explanation(
        "Generic HTTP roundup",
        0.1,
        0.3,
        &[],
        &ace_ctx,
        &["http".to_string()],
        &[interest("tower-http")],
        &[],
        &[],
        0,
    );
    assert!(
        !explanation.contains("tower-http"),
        "fragment 'http' must not claim interest 'tower-http': {explanation}"
    );
}

#[test]
fn test_interest_exact_match_still_claims() {
    let ace_ctx = ACEContext::default();
    let explanation = generate_relevance_explanation(
        "tower-http middleware deep dive",
        0.1,
        0.3,
        &[],
        &ace_ctx,
        &["tower-http".to_string()],
        &[interest("tower-http")],
        &[],
        &[],
        0,
    );
    assert!(
        explanation.contains("Matches interest: tower-http"),
        "exact interest match must still be cited: {explanation}"
    );
}

#[test]
fn test_interest_segment_match_still_claims() {
    // Interest occurring as a whole delimited segment of the topic still counts.
    let ace_ctx = ACEContext::default();
    let explanation = generate_relevance_explanation(
        "React Native release",
        0.1,
        0.3,
        &[],
        &ace_ctx,
        &["react-native".to_string()],
        &[interest("react")],
        &[],
        &[],
        0,
    );
    assert!(
        explanation.contains("Matches interest: react"),
        "whole-segment interest match must still be cited: {explanation}"
    );
}

#[test]
fn test_interest_alias_match_is_cited() {
    // F6: the citation path must consult the curated alias database like
    // the scoring path does — "reactjs" and "react" are the same alias
    // group, so an item topic "reactjs" cites the declared interest
    // "react" even though neither is a delimited segment of the other.
    assert!(aliases::are_aliases("reactjs", "react"));
    let ace_ctx = ACEContext::default();
    let explanation = generate_relevance_explanation(
        "ReactJS 19 released",
        0.1,
        0.3,
        &[],
        &ace_ctx,
        &["reactjs".to_string()],
        &[interest("react")],
        &[],
        &[],
        0,
    );
    assert!(
        explanation.contains("Matches interest: react"),
        "alias-group interest match must be cited: {explanation}"
    );
}

#[test]
fn test_topic_word_match_no_infix() {
    // Exact and whole-segment matches hold; infix fragments do not.
    assert!(topic_word_match("tokio", "tokio"));
    assert!(topic_word_match("react-native", "react"));
    assert!(topic_word_match("next.js", "next"));
    assert!(!topic_word_match("macos", "os"));
    assert!(!topic_word_match("typescript", "types"));
}

#[test]
fn test_active_topic_reasoning_filters_noise() {
    // Noise fragments in active_topics must NOT surface as "active project" reasons:
    // "os" is low-quality (2 chars) AND only infix-matches "macos"; a credible,
    // word-matched topic ("tokio") still gets cited.
    let mut ace_ctx = ACEContext::default();
    ace_ctx.active_topics = vec!["os".to_string(), "tokio".to_string()];
    let explanation = generate_relevance_explanation(
        "Tokio async runtime on macOS",
        0.5,
        0.3,
        &[],
        &ace_ctx,
        &["tokio".to_string(), "macos".to_string()],
        &[],
        &[],
        &[],
        0,
    );
    assert!(
        !explanation.contains("os (active project)") && !explanation.contains(", os "),
        "noise topic 'os' must not appear as an active-project reason: {explanation}"
    );
    assert!(
        explanation.contains("tokio") && explanation.contains("active project"),
        "credible topic 'tokio' should be cited as an active-project reason: {explanation}"
    );
}
