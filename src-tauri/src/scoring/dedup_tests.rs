// SPDX-License-Identifier: FSL-1.1-Apache-2.0

use super::*;
use crate::SourceRelevance;

/// Helper: create a minimal SourceRelevance for testing
fn make_item(title: &str, url: Option<&str>, score: f32) -> SourceRelevance {
    SourceRelevance {
        id: 0,
        title: title.to_string(),
        url: url.map(|u| u.to_string()),
        top_score: score,
        matches: vec![],
        relevant: true,
        context_score: 0.0,
        interest_score: 0.0,
        excluded: false,
        excluded_by: None,
        source_type: "test".to_string(),
        explanation: None,
        confidence: None,
        score_breakdown: None,
        signal_type: None,
        signal_priority: None,
        signal_action: None,
        signal_triggers: None,
        signal_horizon: None,
        similar_count: 0,
        similar_titles: vec![],
        serendipity: false,
        detected_lang: "en".to_string(),
        streets_engine: None,
        decision_window_match: None,
        decision_boost_applied: 0.0,
        created_at: None,
        is_critical_alert: false,
        applicability: None,
        advisory_id: None,
    }
}

#[test]
fn test_dedup_by_url_keeps_highest_score() {
    let mut items = vec![
        make_item(
            "Low score article",
            Some("https://example.com/article"),
            0.3,
        ),
        make_item(
            "High score article",
            Some("https://example.com/article"),
            0.9,
        ),
        make_item("Different article", Some("https://other.com/page"), 0.5),
    ];

    dedup_results(&mut items);

    // Should keep 2 items (one per unique URL)
    assert_eq!(items.len(), 2, "Should have 2 items after URL dedup");
    // The first item should be the highest scoring one for the duplicate URL
    assert_eq!(
        items[0].top_score, 0.9,
        "Highest scoring item should be kept"
    );
    assert_eq!(items[1].top_score, 0.5, "Non-duplicate item should remain");
}

#[test]
fn test_dedup_by_normalized_title() {
    let mut items = vec![
        make_item("Show HN: My Cool Project", None, 0.8),
        make_item("My Cool Project", None, 0.6),
        make_item("Something Completely Different", None, 0.5),
    ];

    dedup_results(&mut items);

    // "Show HN: My Cool Project" and "My Cool Project" normalize to the same title
    assert_eq!(items.len(), 2, "Should have 2 items after title dedup");
    // Highest scoring duplicate kept first
    assert_eq!(
        items[0].top_score, 0.8,
        "Highest scoring title duplicate should be kept"
    );
    assert_eq!(items[1].top_score, 0.5, "Unique title should remain");
}

#[test]
fn test_sort_excluded_items_last() {
    let mut items = vec![
        {
            let mut item = make_item("Excluded high score", None, 0.9);
            item.excluded = true;
            item
        },
        make_item("Normal low score", None, 0.3),
        make_item("Normal mid score", None, 0.6),
    ];

    sort_results(&mut items);

    // Non-excluded items should come first, excluded last
    assert!(!items[0].excluded, "First item should not be excluded");
    assert!(!items[1].excluded, "Second item should not be excluded");
    assert!(items[2].excluded, "Last item should be excluded");
    // Non-excluded items should be sorted by score desc
    assert!(
        items[0].top_score >= items[1].top_score,
        "Non-excluded items should be sorted by score descending"
    );
}

#[test]
fn test_sort_by_score_descending() {
    let mut items = vec![
        make_item("Low", None, 0.2),
        make_item("High", None, 0.9),
        make_item("Mid", None, 0.5),
        make_item("Very High", None, 0.95),
    ];

    sort_results(&mut items);

    for i in 0..items.len() - 1 {
        assert!(
            items[i].top_score >= items[i + 1].top_score,
            "Items should be sorted by score descending: {} >= {} failed at index {}",
            items[i].top_score,
            items[i + 1].top_score,
            i
        );
    }
}

#[test]
fn test_empty_input_returns_empty() {
    let mut empty: Vec<SourceRelevance> = vec![];

    dedup_results(&mut empty);
    assert!(empty.is_empty(), "Dedup of empty vec should remain empty");

    sort_results(&mut empty);
    assert!(empty.is_empty(), "Sort of empty vec should remain empty");
}

// ====================================================================
// normalize_result_url tests
// ====================================================================

#[test]
fn test_normalize_url_strips_fragment() {
    assert_eq!(
        normalize_result_url("https://example.com/page#section"),
        "https://example.com/page"
    );
}

#[test]
fn test_normalize_url_strips_query() {
    assert_eq!(
        normalize_result_url("https://example.com/page?ref=hn"),
        "https://example.com/page"
    );
}

#[test]
fn test_normalize_url_http_to_https() {
    assert_eq!(
        normalize_result_url("http://example.com/page"),
        "https://example.com/page"
    );
}

#[test]
fn test_normalize_url_strips_www() {
    assert_eq!(
        normalize_result_url("https://www.example.com/page"),
        "https://example.com/page"
    );
}

#[test]
fn test_normalize_url_strips_trailing_slash() {
    assert_eq!(
        normalize_result_url("https://example.com/page/"),
        "https://example.com/page"
    );
}

#[test]
fn test_normalize_url_lowercases() {
    assert_eq!(
        normalize_result_url("https://Example.COM/Page"),
        "https://example.com/page"
    );
}

#[test]
fn test_normalize_url_combined() {
    assert_eq!(
        normalize_result_url("http://www.Example.COM/Page/?ref=hn#section"),
        "https://example.com/page"
    );
}

// ====================================================================
// normalize_result_title tests
// ====================================================================

#[test]
fn test_normalize_title_strips_show_hn() {
    let a = normalize_result_title("Show HN: My Cool Project");
    let b = normalize_result_title("My Cool Project");
    assert_eq!(a, b);
}

#[test]
fn test_normalize_title_strips_ask_hn() {
    let a = normalize_result_title("Ask HN: Best Rust Resources?");
    let b = normalize_result_title("Best Rust Resources?");
    assert_eq!(a, b);
}

#[test]
fn test_normalize_title_strips_punctuation() {
    let normalized = normalize_result_title("Hello, World! (2025)");
    // Should strip commas, exclamation, parens
    assert!(!normalized.contains(','));
    assert!(!normalized.contains('!'));
    assert!(!normalized.contains('('));
}

#[test]
fn test_normalize_title_lowercases() {
    let normalized = normalize_result_title("Rust Async Patterns");
    assert_eq!(normalized, "rust async patterns");
}

#[test]
fn test_normalize_title_normalizes_whitespace() {
    let normalized = normalize_result_title("  Too   Many    Spaces  ");
    assert_eq!(normalized, "too many spaces");
}

// ====================================================================
// dedup additional edge cases
// ====================================================================

#[test]
fn test_dedup_no_url_no_dup() {
    let mut items = vec![
        make_item("Unique Title One", None, 0.8),
        make_item("Unique Title Two", None, 0.6),
    ];
    dedup_results(&mut items);
    assert_eq!(items.len(), 2, "Unique titles should not be deduped");
}

#[test]
fn test_dedup_url_normalization_catches_variants() {
    let mut items = vec![
        make_item("Article A", Some("http://www.example.com/page/"), 0.8),
        make_item("Article B", Some("https://example.com/page"), 0.6),
    ];
    dedup_results(&mut items);
    assert_eq!(
        items.len(),
        1,
        "URL variants should be deduped after normalization"
    );
}

#[test]
fn test_sort_all_excluded() {
    let mut items = vec![
        {
            let mut item = make_item("A", None, 0.9);
            item.excluded = true;
            item
        },
        {
            let mut item = make_item("B", None, 0.3);
            item.excluded = true;
            item
        },
    ];
    sort_results(&mut items);
    assert!(items[0].top_score >= items[1].top_score);
}

// ====================================================================
// compute_serendipity_candidates tests
// ====================================================================

#[test]
fn test_serendipity_empty_results() {
    let results: Vec<SourceRelevance> = vec![];
    let candidates = compute_serendipity_candidates(&results, 20);
    assert!(candidates.is_empty());
}

#[test]
fn test_serendipity_all_relevant() {
    // If all items are relevant, no serendipity candidates
    let results = vec![make_item("Relevant", None, 0.8)];
    let candidates = compute_serendipity_candidates(&results, 20);
    assert!(
        candidates.is_empty(),
        "All-relevant results should yield no serendipity"
    );
}

#[test]
fn test_serendipity_marks_items_correctly() {
    let mut items = vec![make_item("Relevant", None, 0.8), {
        let mut item = make_item("Near miss", None, 0.4);
        item.relevant = false;
        item.context_score = 0.3; // Above SERENDIPITY_MIN_AXIS_SCORE
        item
    }];
    items[0].relevant = true;
    let candidates = compute_serendipity_candidates(&items, 100);
    for c in &candidates {
        assert!(c.serendipity, "Serendipity candidates should be marked");
        assert!(c.relevant, "Serendipity candidates should be made relevant");
        assert!(c.explanation.is_some(), "Should have explanation");
    }
}

#[test]
fn test_serendipity_budget_caps_at_five() {
    let mut results = vec![make_item("Relevant", None, 0.8)];
    // Add many non-relevant items with signal
    for i in 0..20 {
        let mut item = make_item(&format!("Miss {}", i), None, 0.3);
        item.relevant = false;
        item.context_score = 0.3;
        results.push(item);
    }
    let candidates = compute_serendipity_candidates(&results, 100);
    assert!(
        candidates.len() <= 5,
        "Budget should cap at 5, got {}",
        candidates.len()
    );
}

// ===== Fuzzy dedup tests =====

#[test]
fn test_jaccard_identical_titles() {
    let sim = jaccard_word_similarity("rust async patterns", "rust async patterns");
    assert!(
        (sim - 1.0).abs() < f32::EPSILON,
        "Identical titles should score 1.0"
    );
}

#[test]
fn test_jaccard_completely_different() {
    let sim = jaccard_word_similarity("rust async patterns", "python data science");
    assert!(
        sim < 0.1,
        "Completely different titles should score near 0.0, got {sim}"
    );
}

#[test]
fn test_jaccard_cross_post_caught_at_065() {
    // Near-duplicate: 4 of 5 words shared = Jaccard 0.67
    let sim = jaccard_word_similarity(
        "kubernetes pod networking deep dive",
        "kubernetes pod networking explained dive",
    );
    assert!(
        sim >= 0.65,
        "Cross-post variant should be caught at 0.65 threshold, got {sim}"
    );
}

#[test]
fn test_jaccard_different_topics_not_deduped() {
    let sim = jaccard_word_similarity(
        "rust error handling patterns",
        "rust async runtime comparison",
    );
    assert!(
        sim < 0.65,
        "Different Rust topics should not be deduped, got {sim}"
    );
}

#[test]
fn test_fuzzy_dedup_removes_near_duplicates() {
    let mut results = vec![
        make_item("Kubernetes pod networking deep dive", None, 0.8),
        make_item("Kubernetes pod networking explained", None, 0.7),
        make_item("Rust async patterns guide", None, 0.6),
    ];
    fuzzy_dedup_results(&mut results);
    // First two are near-duplicates — second should be removed
    let titles: Vec<&str> = results
        .iter()
        .filter(|r| !r.excluded)
        .map(|r| r.title.as_str())
        .collect();
    assert!(
        titles.contains(&"Kubernetes pod networking deep dive"),
        "Higher-scored item should survive"
    );
    assert!(
        titles.contains(&"Rust async patterns guide"),
        "Unrelated item should survive"
    );
}

#[test]
fn test_fuzzy_dedup_preserves_distinct_items() {
    let mut results = vec![
        make_item("React server components tutorial", None, 0.9),
        make_item("Vue composition API patterns", None, 0.8),
        make_item("Svelte stores deep dive", None, 0.7),
    ];
    let before_count = results.len();
    fuzzy_dedup_results(&mut results);
    let after_count = results.iter().filter(|r| !r.excluded).count();
    assert_eq!(
        before_count, after_count,
        "Distinct items should all survive dedup"
    );
}

// ===== Domain diversity tests =====

#[test]
fn domain_diversity_penalizes_repeated_domains() {
    let mut results = vec![
        make_item("Article A", Some("https://blog.example.com/a"), 0.80),
        make_item("Article B", Some("https://blog.example.com/b"), 0.75),
        make_item("Article C", Some("https://other.com/c"), 0.70),
        make_item("Article D", Some("https://blog.example.com/d"), 0.65),
    ];
    apply_domain_diversity(&mut results);
    // First from blog.example.com untouched
    assert!((results[0].top_score - 0.80).abs() < 0.001);
    // Second from blog.example.com penalized
    assert!(results[1].top_score < 0.75);
    // other.com untouched (first from that domain)
    assert!((results[2].top_score - 0.70).abs() < 0.001);
    // Third from blog.example.com penalized more
    assert!(results[3].top_score < results[1].top_score);
}

#[test]
fn domain_diversity_skips_excluded_items() {
    let mut results = vec![
        make_item("A", Some("https://example.com/a"), 0.80),
        {
            let mut r = make_item("B", Some("https://example.com/b"), 0.75);
            r.excluded = true;
            r
        },
        make_item("C", Some("https://example.com/c"), 0.70),
    ];
    apply_domain_diversity(&mut results);
    assert!((results[0].top_score - 0.80).abs() < 0.001);
    // Excluded item's score untouched
    assert!((results[1].top_score - 0.75).abs() < 0.001);
    // C is position 1 (not 2) because B was excluded
    let expected = 0.70 * ((1.0 - 0.15) * 0.55_f32.powf(1.0) + 0.15);
    assert!((results[2].top_score - expected).abs() < 0.01);
}

#[test]
fn domain_diversity_no_url_items_untouched() {
    let mut results = vec![
        make_item("A", None, 0.80),
        make_item("B", None, 0.75),
    ];
    apply_domain_diversity(&mut results);
    assert!((results[0].top_score - 0.80).abs() < 0.001);
    assert!((results[1].top_score - 0.75).abs() < 0.001);
}

#[test]
fn domain_diversity_floor_prevents_zero() {
    let mut results: Vec<_> = (0..10)
        .map(|i| make_item(&format!("Item {i}"), Some("https://same.com/page"), 0.50))
        .collect();
    apply_domain_diversity(&mut results);
    // Even the 10th item should have score > 0 (floor prevents complete suppression)
    assert!(results[9].top_score > 0.0);
    // Floor: multiplier converges to floor (0.15), so min score approaches 0.50 * 0.15 = 0.075
    assert!(results[9].top_score >= 0.50 * 0.14);
}

#[test]
fn extract_domain_strips_www_and_port() {
    assert_eq!(
        extract_domain("https://www.example.com/path"),
        Some("example.com".to_string())
    );
    assert_eq!(
        extract_domain("http://localhost:8080/api"),
        Some("localhost".to_string())
    );
    assert_eq!(
        extract_domain("https://blog.rust-lang.org/2026/05/post"),
        Some("blog.rust-lang.org".to_string())
    );
}
