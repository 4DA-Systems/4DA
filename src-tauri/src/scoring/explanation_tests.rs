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
