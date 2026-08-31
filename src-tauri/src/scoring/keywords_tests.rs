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
fn test_broad_classification_token_equality_not_substring() {
    // Specific tech that CONTAINS a broad term as a substring must classify
    // as SPECIFIC — the old raw-`contains` check misread "tailwind" as "ai",
    // "html" as "ml", "fastapi" as "api" and dropped a focused user's lone
    // interest to 0.25, structurally killing it when embeddings are down.
    for specific in [
        "tailwind",
        "langchain",
        "fastapi",
        "html",
        "webpack",
        "cloudflare",
    ] {
        assert!(
            !is_generic_interest_term(specific, None),
            "'{specific}' must classify as SPECIFIC, not generic"
        );
    }
    // Genuinely generic terms still classify as generic.
    for generic in ["ai", "api", "open source"] {
        assert!(
            is_generic_interest_term(generic, None),
            "'{generic}' must classify as generic"
        );
    }
    // Token equality inside a multi-word interest still counts as broad
    // ("ai agents" is an ai interest), and multi-word broad entries match
    // as word-bounded phrases.
    assert!(is_generic_interest_term("ai agents", None));
    assert!(is_generic_interest_term("machine learning ops", None));
}

#[test]
fn test_focused_specific_interest_with_broad_substring_keeps_weight() {
    // End-to-end: a focused user whose lone interest is "Tailwind" must keep
    // full 1.0 weight on a genuine Tailwind title (old contains-based broad
    // check dropped this to 0.25).
    let w = best_interest_specificity_weight(
        "Tailwind v4 rewrites its engine",
        "",
        &single_interest("tailwind"),
    );
    assert_eq!(w, 1.0, "focused 'tailwind' must keep full weight, got {w}");

    let w_lc = best_interest_specificity_weight(
        "Building agents with LangChain",
        "",
        &single_interest("langchain"),
    );
    assert_eq!(w_lc, 1.0, "focused 'langchain' must keep full weight");
}

#[test]
fn test_specificity_weight_broad_substring_not_penalized_non_focused() {
    // Non-focused path: "railway" (contains "ai") is a single specific word —
    // 0.60, not the broad 0.25.
    assert_eq!(interest_specificity_weight("railway"), 0.60);
    assert_eq!(interest_specificity_weight("tailwind"), 0.60);
    // Exact broad terms still get the broad weight.
    assert_eq!(interest_specificity_weight("ai"), 0.25);
}

#[test]
fn test_plural_fallback_for_alias_group_terms() {
    // Alias-group terms skip English stemming but must still match bare
    // plurals — alias groups carry no plural variants.
    let llm = compute_keyword_interest_score(
        "Why LLMs still fail at long-horizon planning",
        "",
        &single_interest("llm"),
    );
    assert!(llm > 0.0, "'llm' must match 'LLMs' via plural fallback");

    let api = compute_keyword_interest_score(
        "Designing APIs that survive versioning",
        "",
        &single_interest("api"),
    );
    assert!(api > 0.0, "'api' must match 'APIs' via plural fallback");

    let container = compute_keyword_interest_score(
        "Debugging containers in production",
        "",
        &single_interest("container"),
    );
    assert!(
        container > 0.0,
        "'container' must match 'containers' via plural fallback"
    );

    // Specificity path gets the same fallback. Use a non-focused (3-interest)
    // fixture where a registered match (single-word "container" -> 0.60) is
    // distinguishable from no-match (neutral 1.0).
    let make = |topic: &str| context_engine::Interest {
        id: Some(1),
        topic: topic.to_string(),
        weight: 1.0,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    };
    let interests = vec![make("container"), make("rust"), make("kubernetes")];
    let w = best_interest_specificity_weight("Debugging containers in production", "", &interests);
    assert_eq!(
        w, 0.60,
        "plural hit must register as a match in the specificity path (matched \
         single-word weight), not fall through to the neutral 1.0"
    );
}

#[test]
fn test_plural_fallback_does_not_match_derived_forms() {
    // "dockerized" is a derived form, not a plural — losing it is the
    // accepted cost of skipping English stemming for tech names.
    let score = compute_keyword_interest_score(
        "How we dockerized our monolith",
        "the team dockerized everything",
        &single_interest("docker"),
    );
    assert_eq!(
        score, 0.0,
        "'docker' must not match 'dockerized' via the plural fallback"
    );
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

// ============================================================================
// Own-stack single-word keyword evidence (2026-08-23 audit item 14)
// ============================================================================

fn stack_set(items: &[&str]) -> std::collections::HashSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn weighted_interest(topic: &str, weight: f32) -> context_engine::Interest {
    context_engine::Interest {
        id: Some(1),
        topic: topic.to_string(),
        weight,
        source: context_engine::InterestSource::Explicit,
        embedding: None,
    }
}

#[test]
fn test_own_stack_score_reaches_threshold_for_primary_stack_title_hit() {
    // A Tauri developer with 3 interests: today "Tauri 2 ..." titles cap at
    // 0.80 × 0.60 = 0.48 post-discount. The own-stack evidence channel must
    // expose the RAW score (≥ 0.70) for the gate's confirmation route.
    let primary = stack_set(&["rust", "tauri", "sqlite"]);
    let all = primary.clone();
    let profile = crate::scoring::calibration::SpecificityProfile {
        user_role: None,
        primary_stack: &primary,
        all_tech: &all,
    };
    let interests = vec![
        weighted_interest("Rust", 1.0),
        weighted_interest("systems programming", 1.0),
        weighted_interest("Tauri", 1.0),
    ];
    let score = own_stack_single_word_keyword_score(
        "Tauri 2 right-click menu replacement on Windows",
        "A practical guide to replacing the default context menu in Tauri 2 desktop apps.",
        &interests,
        Some(&profile),
    );
    assert!(
        score >= 0.70,
        "own-stack title hit must reach the keyword confirmation threshold, got {score:.3}"
    );
}

#[test]
fn test_own_stack_score_zero_for_off_stack_interest() {
    // "React" is NOT in this user's primary stack — no own-stack evidence,
    // even though the keyword clearly hits.
    let primary = stack_set(&["rust", "tauri"]);
    let all = primary.clone();
    let profile = crate::scoring::calibration::SpecificityProfile {
        user_role: None,
        primary_stack: &primary,
        all_tech: &all,
    };
    let interests = vec![weighted_interest("React", 1.0)];
    let score = own_stack_single_word_keyword_score(
        "React 19 released with the new compiler",
        "The React team shipped React 19 today.",
        &interests,
        Some(&profile),
    );
    assert_eq!(
        score, 0.0,
        "off-stack single-word interests get no own-stack evidence"
    );
}

#[test]
fn test_own_stack_score_zero_without_profile() {
    let interests = vec![weighted_interest("Rust", 1.0)];
    let score = own_stack_single_word_keyword_score(
        "Rust 1.80 released",
        "The Rust team shipped a new release.",
        &interests,
        None,
    );
    assert_eq!(score, 0.0, "no profile → no own-stack evidence");
}

#[test]
fn test_own_stack_score_ignores_multi_word_interests() {
    // Multi-word interests already carry full specificity weight — they never
    // need (or get) the own-stack channel.
    let primary = stack_set(&["rust"]);
    let all = primary.clone();
    let profile = crate::scoring::calibration::SpecificityProfile {
        user_role: None,
        primary_stack: &primary,
        all_tech: &all,
    };
    let interests = vec![weighted_interest("rust async patterns", 1.0)];
    let score = own_stack_single_word_keyword_score(
        "Rust async patterns in production",
        "",
        &interests,
        Some(&profile),
    );
    assert_eq!(score, 0.0, "multi-word interests are out of scope");
}

#[test]
fn test_own_stack_score_all_tech_membership_is_not_enough() {
    // Narrow scope: primary_stack ONLY. Adjacent tech in all_tech does not
    // qualify — "typescript" here is adjacent, not primary.
    let primary = stack_set(&["rust", "tauri"]);
    let all = stack_set(&["rust", "tauri", "typescript", "tokio"]);
    let profile = crate::scoring::calibration::SpecificityProfile {
        user_role: None,
        primary_stack: &primary,
        all_tech: &all,
    };
    let interests = vec![weighted_interest("TypeScript", 1.0)];
    let score = own_stack_single_word_keyword_score(
        "TypeScript 5.6 beta announced",
        "The TypeScript team published the 5.6 beta.",
        &interests,
        Some(&profile),
    );
    assert_eq!(
        score, 0.0,
        "all_tech membership alone must not qualify as own-stack"
    );
}

#[test]
fn test_own_stack_score_low_weight_synthesized_interest_stays_below_threshold() {
    // Interest weight multiplies through: a dep-synthesized interest at 0.3
    // can never reach the 0.70 confirmation threshold via this channel.
    let primary = stack_set(&["tokio"]);
    let all = primary.clone();
    let profile = crate::scoring::calibration::SpecificityProfile {
        user_role: None,
        primary_stack: &primary,
        all_tech: &all,
    };
    let interests = vec![weighted_interest("tokio", 0.3)];
    let score = own_stack_single_word_keyword_score(
        "Tokio 2.0 runtime released",
        "Tokio ships a major runtime overhaul.",
        &interests,
        Some(&profile),
    );
    assert!(
        score > 0.0 && score < 0.70,
        "synthesized-weight interests must stay below the confirmation bar, got {score:.3}"
    );
}

#[test]
fn test_own_stack_score_word_collision_still_scores_keyword_side() {
    // "Rust Belt cities" DOES produce raw keyword evidence — TN protection
    // lives at the gate (embedding corroboration), not here. This pins the
    // division of responsibility.
    let primary = stack_set(&["rust"]);
    let all = primary.clone();
    let profile = crate::scoring::calibration::SpecificityProfile {
        user_role: None,
        primary_stack: &primary,
        all_tech: &all,
    };
    let interests = vec![weighted_interest("Rust", 1.0)];
    let score = own_stack_single_word_keyword_score(
        "How Rust Belt cities are reinventing themselves through urban farming",
        "Former industrial cities across the American Rust Belt are finding new vitality.",
        &interests,
        Some(&profile),
    );
    assert!(
        score >= 0.70,
        "keyword side is provenance-blind by design (gate corroboration guards it), got {score:.3}"
    );
}
