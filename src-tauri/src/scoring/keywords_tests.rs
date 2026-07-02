// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::*;

#[test]
fn test_interest_specificity_weight_broad() {
    assert_eq!(interest_specificity_weight("Open Source"), 0.25);
    assert_eq!(interest_specificity_weight("AI"), 0.25);
    assert_eq!(interest_specificity_weight("machine learning"), 0.25);
    assert_eq!(interest_specificity_weight("cloud"), 0.25);
    assert_eq!(interest_specificity_weight("programming"), 0.25);
}

#[test]
fn test_interest_specificity_weight_single_word() {
    // Single non-broad words get moderate weight
    assert_eq!(interest_specificity_weight("Tauri"), 0.60);
    assert_eq!(interest_specificity_weight("Kubernetes"), 0.60);
}

#[test]
fn test_interest_specificity_weight_specific() {
    // Multi-word specific terms get full weight
    assert_eq!(interest_specificity_weight("Tauri plugins"), 1.00);
    assert_eq!(interest_specificity_weight("sqlite-vss indexing"), 1.00);
    assert_eq!(interest_specificity_weight("Rust async patterns"), 1.00);
}

#[test]
fn test_broad_interest_specificity_penalty() {
    // Helper to make an interest
    let make = |topic: &str| context_engine::Interest {
        id: Some(1),
        topic: topic.to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    };

    // 6+ interests: broad terms get full penalty (0.25x)
    let many_interests = vec![
        make("Open Source"),
        make("Rust"),
        make("TypeScript"),
        make("AI"),
        make("Security"),
        make("DevOps"),
    ];
    let specificity = best_interest_specificity_weight(
        "New open source project for data pipelines",
        "",
        &many_interests,
    );
    assert_eq!(
        specificity, 0.25,
        "Broad interest with 6+ interests should return 0.25 weight"
    );

    // 3-5 interests: broad terms get softened penalty (floor 0.60)
    let medium_interests = vec![make("Open Source"), make("Rust"), make("TypeScript")];
    let specificity = best_interest_specificity_weight(
        "New open source project for data pipelines",
        "",
        &medium_interests,
    );
    assert_eq!(
        specificity, 0.60,
        "Broad interest with 3-5 interests should return 0.60 floor"
    );

    // 1-2 interests: focused user — but "Open Source" is GENERIC, so it
    // falls back to its computed specificity (0.25) instead of the forced
    // 1.0 that used to defeat the gate's broad-interest corroboration guard.
    let few_interests = vec![make("Open Source")];
    let specificity = best_interest_specificity_weight(
        "New open source project for data pipelines",
        "",
        &few_interests,
    );
    assert_eq!(
        specificity, 0.25,
        "Focused user with a GENERIC interest keeps the computed broad weight"
    );

    // 1-2 interests with a SPECIFIC single-word interest: full trust stands.
    let focused_specific = vec![make("Tauri")];
    let specificity =
        best_interest_specificity_weight("Tauri 2.0 ships mobile support", "", &focused_specific);
    assert_eq!(
        specificity, 1.00,
        "Focused user with a SPECIFIC interest keeps full 1.0 weight"
    );

    // Alias-expanded match: "kubernetes" in interests, "k8s" in title
    let alias_interests = vec![make("kubernetes"), make("Rust"), make("TypeScript")];
    let specificity = best_interest_specificity_weight(
        "Scaling k8s clusters in production",
        "",
        &alias_interests,
    );
    assert!(
        specificity > 0.0,
        "Alias match should find 'kubernetes' via 'k8s' in title"
    );

    // A specific interest should get full weight regardless of count
    let specific_interests = vec![context_engine::Interest {
        id: Some(2),
        topic: "Tauri plugins".to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    }];
    let specificity = best_interest_specificity_weight(
        "Building Tauri plugins for desktop apps",
        "",
        &specific_interests,
    );
    assert_eq!(
        specificity, 1.00,
        "Specific interest should return 1.0 weight"
    );
}

#[test]
fn test_keyword_stemming_match() {
    let interests = vec![context_engine::Interest {
        id: Some(1),
        topic: "testing".to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    }];
    // "test" in title should match "testing" interest via stemming
    let score = compute_keyword_interest_score("How to test your Rust code", "", &interests);
    assert!(
        score > 0.0,
        "Stemmed match should produce positive score, got {}",
        score
    );
}

#[test]
fn test_keyword_alias_match() {
    let interests = vec![context_engine::Interest {
        id: Some(1),
        topic: "kubernetes".to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    }];
    // "k8s" in title should match "kubernetes" interest via alias
    let score =
        compute_keyword_interest_score("Scaling k8s clusters in production", "", &interests);
    assert!(
        score > 0.0,
        "Alias match should produce positive score, got {}",
        score
    );
}

#[test]
fn test_keyword_alias_reverse() {
    let interests = vec![context_engine::Interest {
        id: Some(1),
        topic: "ts".to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    }];
    // "typescript" in title should match "ts" interest via alias
    let score = compute_keyword_interest_score("Advanced TypeScript patterns", "", &interests);
    assert!(
        score > 0.0,
        "Reverse alias match should produce positive score, got {}",
        score
    );
}

#[test]
fn test_keyword_no_false_stemming() {
    let interests = vec![context_engine::Interest {
        id: Some(1),
        topic: "testing".to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    }];
    // "resting" should NOT match "testing" via stemming — different stems (rest vs test)
    // And "resting" does not contain the substring "testing"
    let score = compute_keyword_interest_score("A resting period for developers", "", &interests);
    assert_eq!(
        score, 0.0,
        "Should not false-match 'testing' from 'resting'"
    );
}

#[test]
fn test_term_density_multiplier() {
    // Single mention = no bonus
    assert_eq!(term_density_multiplier("rust", "learning rust basics"), 1.0);
    // Multiple mentions = density bonus
    let dense = term_density_multiplier(
        "rust",
        "rust is great. rust performance. rust safety. rust ecosystem.",
    );
    assert!(dense > 1.0, "Dense content should get bonus, got {}", dense);
    assert!(
        dense <= 1.5,
        "Density bonus should be capped at 1.5, got {}",
        dense
    );
}

#[test]
fn test_negation_detection() {
    assert!(is_negated_in_context("react", "we don't use react anymore"));
    assert!(is_negated_in_context(
        "kubernetes",
        "alternative to kubernetes for small teams"
    ));
    assert!(is_negated_in_context(
        "vue",
        "moving away from vue to react"
    ));
    assert!(!is_negated_in_context(
        "rust",
        "learning rust for systems programming"
    ));
    assert!(!is_negated_in_context(
        "python",
        "python data science tutorial"
    ));
}

#[test]
fn test_negated_term_reduces_score() {
    let make = |topic: &str| {
        vec![context_engine::Interest {
            id: Some(1),
            topic: topic.to_string(),
            weight: 1.0,
            source: context_engine::InterestSource::Explicit,
            embedding: None,
        }]
    };

    let positive_score = compute_keyword_interest_score(
        "Getting started with React",
        "React is a great framework for building UIs",
        &make("react"),
    );
    let negated_score = compute_keyword_interest_score(
        "Why we stopped using React",
        "We don't use react anymore, switched to Vue",
        &make("react"),
    );
    assert!(
        negated_score < positive_score,
        "Negated context should score lower: positive={}, negated={}",
        positive_score,
        negated_score,
    );
}

#[test]
fn test_dense_content_scores_higher() {
    let make = |topic: &str| {
        vec![context_engine::Interest {
            id: Some(1),
            topic: topic.to_string(),
            weight: 1.0,
            source: context_engine::InterestSource::Explicit,
            embedding: None,
        }]
    };

    let sparse = compute_keyword_interest_score(
        "Various tools for developers",
        "Among many technologies including rust and others for building software applications in production environments with complex requirements",
        &make("rust"),
    );
    let dense = compute_keyword_interest_score(
        "Rust performance benchmarks",
        "rust vs go benchmarks. rust async performance. rust memory safety. rust compiler optimizations",
        &make("rust"),
    );
    assert!(
        dense > sparse,
        "Dense content should score higher: dense={}, sparse={}",
        dense,
        sparse,
    );
}

#[test]
fn test_first_paragraph_boost() {
    let make = |topic: &str| {
        vec![context_engine::Interest {
            id: Some(1),
            topic: topic.to_string(),
            weight: 1.0,
            source: context_engine::InterestSource::Explicit,
            embedding: None,
        }]
    };

    // Term appearing early in content should score higher than buried deep
    let early = compute_keyword_interest_score(
        "Developer tools roundup",
        "Rust is gaining traction in systems programming. Various teams are adopting it for performance-critical services.",
        &make("rust"),
    );
    let late = compute_keyword_interest_score(
        "Developer tools roundup",
        "Many languages compete for developer attention. Teams evaluate options based on performance, safety, and ecosystem maturity. Among the newer contenders gaining traction in systems work beyond the first two hundred characters of content is rust which some teams now use.",
        &make("rust"),
    );
    assert!(
        early > late,
        "Early content match should score higher: early={}, late={}",
        early,
        late,
    );
}

#[test]
fn test_multi_word_phrase_match() {
    let make = |topic: &str| {
        vec![context_engine::Interest {
            id: Some(1),
            topic: topic.to_string(),
            weight: 1.0,
            source: context_engine::InterestSource::Explicit,
            embedding: None,
        }]
    };

    // Exact phrase match should score higher than scattered words
    let phrase_score = compute_keyword_interest_score(
        "Introduction to machine learning",
        "A comprehensive guide to getting started with AI",
        &make("machine learning"),
    );
    let scattered_score = compute_keyword_interest_score(
        "The factory machine needs repair",
        "Our team is learning new protocols for operating industrial equipment in the facility",
        &make("machine learning"),
    );
    assert!(
        phrase_score > scattered_score,
        "Phrase match should beat scattered words: phrase={}, scattered={}",
        phrase_score,
        scattered_score,
    );
}

#[test]
fn test_single_char_interest_r() {
    let interests = vec![context_engine::Interest {
        id: Some(1),
        topic: "R".to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    }];
    let score = compute_keyword_interest_score(
        "Statistical computing with R",
        "R is widely used in data science",
        &interests,
    );
    assert!(
        score > 0.0,
        "Single-char interest 'R' should match, got {}",
        score
    );
}

#[test]
fn test_single_char_interest_no_false_positive() {
    let interests = vec![context_engine::Interest {
        id: Some(1),
        topic: "R".to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    }];
    // "R" should NOT match in "Rust" or "React" (not word-bounded)
    let score = compute_keyword_interest_score(
        "Getting started with Rust",
        "Rust is a systems programming language",
        &interests,
    );
    assert_eq!(score, 0.0, "Single-char 'R' should not match inside 'Rust'");
}

#[test]
fn test_ambiguous_alias_word_boundary() {
    let interests = vec![context_engine::Interest {
        id: Some(1),
        topic: "nextjs".to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    }];
    // "next" alias should match when word-bounded
    let score = compute_keyword_interest_score(
        "Building apps with Next",
        "Next is great for server rendering",
        &interests,
    );
    assert!(
        score > 0.0,
        "Ambiguous alias 'next' should match with word boundary, got {}",
        score
    );
}

#[test]
fn test_weighted_interest() {
    let low_weight = vec![context_engine::Interest {
        id: Some(1),
        topic: "rust".to_string(),
        weight: 0.5,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    }];
    let full_weight = vec![context_engine::Interest {
        id: Some(1),
        topic: "rust".to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    }];
    let low_score = compute_keyword_interest_score("Learning Rust", "rust guide", &low_weight);
    let full_score = compute_keyword_interest_score("Learning Rust", "rust guide", &full_weight);
    assert!(
        low_score < full_score,
        "Lower weight should produce lower score: low={}, full={}",
        low_score,
        full_score
    );
}

#[test]
fn test_empty_content() {
    let interests = vec![context_engine::Interest {
        id: Some(1),
        topic: "rust".to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    }];
    let title_only = compute_keyword_interest_score("Learning Rust basics", "", &interests);
    assert!(
        title_only > 0.0,
        "Should match on title even with empty content, got {}",
        title_only
    );
}

// ============================================================================
// Word-boundary matching + generic-term corroboration (gate count inflation)
// ============================================================================

fn single_interest(topic: &str) -> Vec<context_engine::Interest> {
    vec![context_engine::Interest {
        id: Some(1),
        topic: topic.to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    }]
}

#[test]
fn test_no_substring_false_positive_rust_frustrating() {
    // "rust" must NOT match inside "frustrating"
    let score = compute_keyword_interest_score(
        "A frustrating week of debugging",
        "the whole experience was frustrating",
        &single_interest("rust"),
    );
    assert_eq!(score, 0.0, "'rust' must not match inside 'frustrating'");
}

#[test]
fn test_no_substring_false_positive_react_reaction() {
    // "react" must NOT match inside "reaction"...
    let miss = compute_keyword_interest_score(
        "Community reaction to the new CSS spec",
        "the reaction was mixed across forums",
        &single_interest("react"),
    );
    assert_eq!(miss, 0.0, "'react' must not match inside 'reaction'");

    // ...but genuine word-bounded mentions still match,
    let hit = compute_keyword_interest_score("React 19 released", "", &single_interest("react"));
    assert!(hit > 0.0, "'react' must match 'React 19 released'");

    // including punctuation-bounded compounds.
    let compound = compute_keyword_interest_score(
        "Debugging react-dom hydration errors",
        "",
        &single_interest("react"),
    );
    assert!(
        compound > 0.0,
        "'react' must match 'react-dom' (hyphen bound)"
    );
}

#[test]
fn test_specificity_weight_no_substring_false_positive() {
    // 3 interests (non-focused): a substring-only pseudo-hit must find NO
    // match, so no attenuation is applied (returns the neutral 1.0), instead
    // of the old contains-based 0.60 broad-floor result.
    let make = |topic: &str| context_engine::Interest {
        id: Some(1),
        topic: topic.to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    };
    let interests = vec![make("rust"), make("typescript"), make("kubernetes")];
    let w = best_interest_specificity_weight("A frustrating day at work", "", &interests);
    assert_eq!(
        w, 1.0,
        "substring-only pseudo-hit must not register as an interest match"
    );
}

#[test]
fn test_focused_generic_interest_requires_corroboration_weight() {
    // A focused user with the lone generic interest "ai": a bare title hit
    // must yield sub-0.50 specificity so the confirmation gate demands
    // embedding corroboration (gate.rs broad-interest guard).
    let w = best_interest_specificity_weight(
        "AI coding assistants compared",
        "",
        &single_interest("ai"),
    );
    assert!(
        w < 0.50,
        "focused generic 'ai' must fall below the 0.50 gate guard, got {w}"
    );

    // Same for "api".
    let w_api = best_interest_specificity_weight(
        "Designing a public API for your startup",
        "",
        &single_interest("api"),
    );
    assert!(
        w_api < 0.50,
        "focused generic 'api' must fall below the 0.50 gate guard, got {w_api}"
    );
}

#[test]
fn test_focused_specific_interest_keeps_full_weight() {
    let w = best_interest_specificity_weight(
        "Tauri 2.0 ships mobile support",
        "",
        &single_interest("tauri"),
    );
    assert_eq!(w, 1.0, "focused specific 'tauri' keeps full weight");

    let w_rust = best_interest_specificity_weight(
        "Rust 1.80 stabilizes async closures",
        "",
        &single_interest("rust"),
    );
    assert_eq!(w_rust, 1.0, "focused specific 'rust' keeps full weight");
}

#[test]
fn test_count_word_occurrences_unicode_boundary() {
    // Bug E regression: UTF-8 continuation bytes must not count as word boundaries.
    assert_eq!(count_word_occurrences("go", "иgo"), 0);
    assert_eq!(count_word_occurrences("go", "goи"), 0);
    // ASCII word boundaries still count.
    assert_eq!(count_word_occurrences("go", "go here, let us go"), 2);
    assert_eq!(count_word_occurrences("go", "argo"), 0);
}
