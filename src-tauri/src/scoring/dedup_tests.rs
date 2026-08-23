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
        primary_topic: None,
        evidence_score: score,
        rank_factors: None,
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

fn make_grounded_item(title: &str, score: f32) -> SourceRelevance {
    let mut item = make_item(title, None, score);
    let json = serde_json::json!({
        "context_score": 0.0,
        "interest_score": 0.0,
        "ace_boost": 0.0,
        "affinity_mult": 1.0,
        "anti_penalty": 0.0,
        "confidence_by_signal": {},
        "strongly_grounded": true,
    });
    item.score_breakdown = Some(serde_json::from_value(json).expect("breakdown"));
    item
}

#[test]
fn test_sort_grounded_tier_outranks_ungrounded_score() {
    // Phase-4 binding rule: a grounded on-stack release must sit above
    // higher-scoring ungrounded noise (the bevy-react-macros@0.90 class).
    let mut items = vec![
        make_item("Ungrounded crate-name collision", None, 0.90),
        make_grounded_item("Grounded on-stack release", 0.84),
        make_item("Ungrounded listicle", None, 0.88),
        make_grounded_item("Grounded advisory", 0.95),
    ];

    sort_results(&mut items);

    assert_eq!(items[0].title, "Grounded advisory");
    assert_eq!(items[1].title, "Grounded on-stack release");
    assert_eq!(items[2].title, "Ungrounded crate-name collision");
    assert_eq!(items[3].title, "Ungrounded listicle");
}

#[test]
fn test_sort_excluded_grounded_still_sinks() {
    // Exclusion (user or brief verdict) outranks the grounding tier.
    let mut items = vec![
        {
            let mut item = make_grounded_item("Excluded grounded", 0.99);
            item.excluded = true;
            item
        },
        make_item("Ungrounded but included", None, 0.4),
    ];

    sort_results(&mut items);

    assert_eq!(items[0].title, "Ungrounded but included");
    assert!(items[1].excluded);
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
fn test_normalize_url_strips_tracking_query_params() {
    // TRACKING params are campaign noise and are dropped…
    assert_eq!(
        normalize_result_url("https://example.com/page?ref=hn"),
        "https://example.com/page"
    );
    assert_eq!(
        normalize_result_url("https://example.com/page?utm_source=x&fbclid=abc&gclid=def"),
        "https://example.com/page"
    );
}

/// Regression: this pass used to discard the ENTIRE query string, which is the
/// identity of the page for `?v=` / `?p=` / `?id=` permalinks. Every YouTube
/// video collapsed onto `https://youtube.com/watch`, so `dedup_results` kept
/// exactly one of them per scored batch.
#[test]
fn test_normalize_url_keeps_content_query_params() {
    assert_eq!(
        normalize_result_url("https://youtube.com/watch?v=dQw4w9WgXcQ"),
        "https://youtube.com/watch?v=dQw4w9WgXcQ"
    );
    assert_ne!(
        normalize_result_url("https://youtube.com/watch?v=dQw4w9WgXcQ"),
        normalize_result_url("https://youtube.com/watch?v=9bZkp7q19f0"),
        "distinct videos must not share a dedup key"
    );
}

#[test]
fn test_normalize_url_param_order_does_not_defeat_dedup() {
    assert_eq!(
        normalize_result_url("https://example.com/p?b=2&a=1"),
        normalize_result_url("https://example.com/p?a=1&b=2")
    );
}

/// End-to-end through `dedup_results`: distinct videos survive, campaign
/// variants of one video do not. This is the pass that decides what reaches the
/// feed, so the collapse was directly visible to the user as "YouTube only ever
/// shows one item".
#[test]
fn test_dedup_results_keeps_distinct_videos_and_folds_tracking_variants() {
    let mut items = vec![
        make_item(
            "Rust async deep dive",
            Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            0.9,
        ),
        make_item(
            "Tauri IPC internals",
            Some("https://www.youtube.com/watch?v=9bZkp7q19f0"),
            0.8,
        ),
        make_item(
            "Rust async deep dive (newsletter link)",
            Some("http://youtube.com/watch?utm_source=nl&v=dQw4w9WgXcQ"),
            0.7,
        ),
    ];
    dedup_results(&mut items);

    let urls: Vec<&str> = items.iter().filter_map(|i| i.url.as_deref()).collect();
    assert_eq!(
        urls,
        vec![
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=9bZkp7q19f0",
        ],
        "two distinct videos survive; the campaign variant of the first does not"
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
    // 10 relevant at 100% budget would allow 10 — the hard cap holds at 5.
    let mut results: Vec<SourceRelevance> = (0..10)
        .map(|i| make_item(&format!("Relevant {i}"), None, 0.8))
        .collect();
    // Add many non-relevant items with signal (context_score must EXCEED
    // SERENDIPITY_MIN_AXIS_SCORE = 0.35 to be candidates — the old 0.3
    // fixture produced zero candidates and the ≤5 assertion passed
    // vacuously, which is how the forced-floor bug stayed invisible)
    for i in 0..20 {
        let mut item = make_item(&format!("Miss {}", i), None, 0.4);
        item.relevant = false;
        item.context_score = 0.4;
        results.push(item);
    }
    let candidates = compute_serendipity_candidates(&results, 100);
    assert_eq!(
        candidates.len(),
        5,
        "Budget should cap at 5, got {}",
        candidates.len()
    );
}

/// v19 regression (2026-08-11): the old budget formula seeded the count with
/// `total_relevant.max(5)` then `.clamp(1, 5)`, FORCING at least one
/// scorer-rejected item into the feed every cycle. On a ~4-relevant/cycle
/// feed those forced injections accumulated to 17.6% of the curated set
/// against an 8% budget. Budget-true means 8% of 4 relevant = ZERO.
#[test]
fn test_serendipity_budget_true_no_forced_floor() {
    let mut results: Vec<SourceRelevance> = (0..4)
        .map(|i| make_item(&format!("Relevant {i}"), None, 0.8))
        .collect();
    for i in 0..10 {
        let mut item = make_item(&format!("Miss {}", i), None, 0.45);
        item.relevant = false;
        item.context_score = 0.4;
        results.push(item);
    }
    let candidates = compute_serendipity_candidates(&results, 8);
    assert!(
        candidates.is_empty(),
        "8% of 4 relevant items is 0 injections — got {}",
        candidates.len()
    );
}

/// The budget scales with the relevant count: 8% of 50 relevant = 4.
#[test]
fn test_serendipity_budget_proportional() {
    let mut results: Vec<SourceRelevance> = (0..50)
        .map(|i| make_item(&format!("Relevant {i}"), None, 0.8))
        .collect();
    for i in 0..10 {
        let mut item = make_item(&format!("Miss {}", i), None, 0.45);
        item.relevant = false;
        item.context_score = 0.4;
        results.push(item);
    }
    let candidates = compute_serendipity_candidates(&results, 8);
    assert_eq!(
        candidates.len(),
        4,
        "8% of 50 relevant items rounds down to 4 injections"
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
    let mut results = vec![make_item("A", None, 0.80), make_item("B", None, 0.75)];
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

// ====================================================================
// Grounded-first dedup (adversarial audit 2026-08-23, item 8f)
// ====================================================================

/// The arrayref case: duplicate copies of the same story at 0.892 ungrounded
/// vs 0.50 dependency-grounded. Grounding is the one axis a content author
/// can't fabricate — the grounded copy must survive dedup.
#[test]
fn test_dedup_url_keeps_grounded_duplicate_over_higher_score() {
    let mut items = vec![
        make_item(
            "arrayref 0.4 release",
            Some("https://example.com/arrayref"),
            0.892,
        ),
        {
            let mut g = make_grounded_item("arrayref 0.4 release grounded copy", 0.50);
            g.url = Some("https://example.com/arrayref".to_string());
            g
        },
    ];
    dedup_results(&mut items);
    assert_eq!(items.len(), 1, "URL duplicate should be removed");
    assert!(
        items[0]
            .score_breakdown
            .as_ref()
            .is_some_and(|b| b.strongly_grounded),
        "the grounded duplicate must survive, not the higher-scored ungrounded one"
    );
    assert!((items[0].top_score - 0.50).abs() < 0.001);
}

#[test]
fn test_dedup_title_keeps_grounded_duplicate_over_higher_score() {
    let mut items = vec![
        make_item("Show HN: Tokio 2.0 Released", None, 0.9),
        make_grounded_item("Tokio 2.0 Released", 0.6),
    ];
    dedup_results(&mut items);
    assert_eq!(items.len(), 1, "Title duplicate should be removed");
    assert!(
        items[0]
            .score_breakdown
            .as_ref()
            .is_some_and(|b| b.strongly_grounded),
        "grounded title-duplicate must survive"
    );
}

/// Both duplicates grounded: the tie-break is score, as before.
#[test]
fn test_dedup_grounded_duplicates_tie_break_by_score() {
    let mut items = vec![
        {
            let mut g = make_grounded_item("Same story low", 0.55);
            g.url = Some("https://example.com/story".to_string());
            g
        },
        {
            let mut g = make_grounded_item("Same story high", 0.80);
            g.url = Some("https://example.com/story".to_string());
            g
        },
    ];
    dedup_results(&mut items);
    assert_eq!(items.len(), 1);
    assert!(
        (items[0].top_score - 0.80).abs() < 0.001,
        "between two grounded duplicates the higher score survives"
    );
}

/// Side effect of the canonical order, intended: an excluded duplicate can no
/// longer claim the URL key and knock out the visible copy.
#[test]
fn test_dedup_excluded_duplicate_no_longer_claims_key() {
    let mut items = vec![
        {
            let mut r = make_item("Story excluded copy", Some("https://example.com/s"), 0.9);
            r.excluded = true;
            r
        },
        make_item("Story visible copy", Some("https://example.com/s"), 0.4),
    ];
    dedup_results(&mut items);
    assert_eq!(items.len(), 1);
    assert!(
        !items[0].excluded,
        "the visible duplicate must survive over the excluded higher-scored one"
    );
}

// ====================================================================
// Platform-domain diversity exemption (adversarial audit 2026-08-23, 8d)
// ====================================================================

#[test]
fn platform_domain_matching() {
    assert!(is_platform_domain("crates.io"));
    assert!(is_platform_domain("github.com"));
    assert!(
        is_platform_domain("gist.github.com"),
        "subdomains of a platform are the same platform"
    );
    assert!(
        !is_platform_domain("xbox.com"),
        "suffix match requires the dot boundary — xbox.com is not x.com"
    );
    assert!(!is_platform_domain("blog.example.com"));
    assert!(!is_platform_domain("hachyderm.io"));
}

/// The churn arithmetic this exemption removes: three of the user's own crate
/// releases in one batch used to decay the 3rd 0.73 -> 0.297 (-0.43), and the
/// next run's different cohort put it back — the +/-0.42-0.43 oscillation in
/// the churn table. Registry items are different crates, not one prolific blog.
#[test]
fn domain_diversity_exempts_platform_domains() {
    let mut results = vec![
        make_item(
            "my-crate-a 1.2.0",
            Some("https://crates.io/crates/my-crate-a"),
            0.73,
        ),
        make_item(
            "my-crate-b 0.5.1",
            Some("https://crates.io/crates/my-crate-b"),
            0.73,
        ),
        make_item(
            "my-crate-c 2.0.0",
            Some("https://crates.io/crates/my-crate-c"),
            0.73,
        ),
    ];
    let adjusted = apply_domain_diversity(&mut results);
    assert_eq!(adjusted, 0, "platform-domain items must not be decayed");
    for r in &results {
        assert!(
            (r.top_score - 0.73).abs() < 0.001,
            "score must survive intact, got {}",
            r.top_score
        );
    }
}

#[test]
fn domain_diversity_platform_subdomain_exempt() {
    let mut results = vec![
        make_item("Gist A", Some("https://gist.github.com/u/a"), 0.6),
        make_item("Gist B", Some("https://gist.github.com/u/b"), 0.6),
    ];
    let adjusted = apply_domain_diversity(&mut results);
    assert_eq!(adjusted, 0);
    assert!((results[1].top_score - 0.6).abs() < 0.001);
}

/// The decay's real purpose is untouched: a genuine single-author blog still
/// decays, with platform items interleaved in the same batch.
#[test]
fn domain_diversity_still_decays_blogs_alongside_exempt_platforms() {
    let mut results = vec![
        make_item("Blog post A", Some("https://prolific.blog/a"), 0.80),
        make_item("crate rel", Some("https://crates.io/crates/x"), 0.78),
        make_item("Blog post B", Some("https://prolific.blog/b"), 0.75),
        make_item("repo rel", Some("https://github.com/owner/repo"), 0.74),
        make_item("Blog post C", Some("https://prolific.blog/c"), 0.70),
    ];
    let adjusted = apply_domain_diversity(&mut results);
    assert_eq!(adjusted, 2, "only the 2nd and 3rd blog items decay");
    assert!((results[0].top_score - 0.80).abs() < 0.001);
    assert!((results[1].top_score - 0.78).abs() < 0.001);
    assert!(results[2].top_score < 0.75, "2nd blog item decays");
    assert!((results[3].top_score - 0.74).abs() < 0.001);
    assert!(
        results[4].top_score < results[2].top_score,
        "3rd blog item decays harder"
    );
}

// ====================================================================
// Serendipity ceiling exclusion + in-place injection (audit 8c)
// ====================================================================

/// Helper: a scorer-rejected item whose breakdown carries the categorical
/// score ceiling (the 0.37 = 0.35 commodity cap + 0.02 offset cluster of
/// ungrounded registry releases). Interest is above the axis floor so ONLY
/// the ceiling check keeps it out of the serendipity pool.
fn make_capped_near_miss(title: &str, score: f32) -> SourceRelevance {
    let mut item = make_item(title, None, score);
    item.relevant = false;
    item.interest_score = 0.4;
    let json = serde_json::json!({
        "context_score": 0.0,
        "interest_score": 0.0,
        "ace_boost": 0.0,
        "affinity_mult": 1.0,
        "anti_penalty": 0.0,
        "confidence_by_signal": {},
        "score_ceiling": 0.37,
    });
    item.score_breakdown = Some(serde_json::from_value(json).expect("breakdown"));
    item
}

#[test]
fn test_serendipity_excludes_score_ceilinged_items() {
    let mut results: Vec<SourceRelevance> = (0..10)
        .map(|i| make_item(&format!("Relevant {i}"), None, 0.8))
        .collect();
    // Three capped look-alike registry releases at the 0.37 cluster …
    for i in 0..3 {
        results.push(make_capped_near_miss(
            &format!("look-alike crate {i}"),
            0.37,
        ));
    }
    // … and two genuine uncapped near-misses scoring LOWER.
    for i in 0..2 {
        let mut item = make_item(&format!("Genuine miss {i}"), None, 0.30);
        item.relevant = false;
        item.interest_score = 0.4;
        results.push(item);
    }
    // Budget: 20% of 10 relevant = 2 slots.
    let candidates = compute_serendipity_candidates(&results, 20);
    assert_eq!(candidates.len(), 2);
    for c in &candidates {
        assert!(
            c.score_breakdown
                .as_ref()
                .is_none_or(|b| b.score_ceiling.is_none()),
            "capped item won a serendipity slot: {}",
            c.title
        );
        assert!(c.title.starts_with("Genuine miss"));
    }
}

/// The exact failure shape: the capped item outscores the genuine near-miss,
/// so by sort order it used to win the only slot.
#[test]
fn test_serendipity_capped_item_does_not_win_slot_by_sort_order() {
    let mut results = vec![make_item("Relevant", None, 0.8)];
    results.push(make_capped_near_miss("capped high", 0.45));
    let mut genuine = make_item("genuine lower", None, 0.40);
    genuine.relevant = false;
    genuine.interest_score = 0.4;
    results.push(genuine);

    // Budget: 100% of 1 relevant = 1 slot.
    let candidates = compute_serendipity_candidates(&results, 100);
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].title, "genuine lower",
        "the ceiling-capped item must not take the slot from the genuine near-miss"
    );
}

#[test]
fn inject_serendipity_replaces_originals_no_duplicate_ids() {
    let mut results: Vec<SourceRelevance> = (0..10)
        .map(|i| {
            let mut r = make_item(&format!("Relevant {i}"), None, 0.8);
            r.id = i + 1;
            r
        })
        .collect();
    for i in 0..5u64 {
        let mut r = make_item(&format!("Miss {i}"), None, 0.45);
        r.id = 100 + i;
        r.relevant = false;
        r.context_score = 0.4;
        results.push(r);
    }
    let before = results.len();

    // Budget: 20% of 10 relevant = 2 picks.
    let injected = inject_serendipity_candidates(&mut results, 20);

    assert_eq!(injected, 2);
    assert_eq!(
        results.len(),
        before,
        "swap in place: each pick replaces its scorer-rejected original"
    );
    let mut seen_ids = std::collections::HashSet::new();
    for r in &results {
        assert!(
            seen_ids.insert(r.id),
            "id {} appears twice — the duplicate-persist clone is back",
            r.id
        );
    }
    assert_eq!(
        results.iter().filter(|r| r.serendipity).count(),
        2,
        "exactly the injected picks are serendipity-marked"
    );
    for r in results.iter().filter(|r| r.serendipity) {
        assert!(r.relevant, "picks surface as relevant");
    }
    assert_eq!(
        results.iter().filter(|r| !r.relevant).count(),
        3,
        "the unpicked misses remain, still not relevant"
    );
}

#[test]
fn inject_serendipity_zero_budget_leaves_results_untouched() {
    let mut results: Vec<SourceRelevance> = (0..4)
        .map(|i| {
            let mut r = make_item(&format!("Relevant {i}"), None, 0.8);
            r.id = i + 1;
            r
        })
        .collect();
    let mut miss = make_item("Miss", None, 0.45);
    miss.id = 100;
    miss.relevant = false;
    miss.context_score = 0.4;
    results.push(miss);

    // 8% of 4 relevant = 0 injections (budget-true, rounds down).
    let injected = inject_serendipity_candidates(&mut results, 8);
    assert_eq!(injected, 0);
    assert_eq!(results.len(), 5);
    assert!(results.iter().all(|r| !r.serendipity));
}
