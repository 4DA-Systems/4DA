// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::aliases;
use super::calibration::BROAD_INTEREST_TERMS;
use super::stemming;
use super::utils::has_word_boundary_match;
use crate::context_engine;
use crate::scoring_config;
use fourda_macros::score_component;

/// Compute how specific an interest topic is.
/// Broad terms ("Open Source", "AI") return low weight (0.25),
/// single-word terms return moderate weight (0.60),
/// multi-word specific terms get full weight (1.0).
pub(crate) fn interest_specificity_weight(interest_topic: &str) -> f32 {
    let topic_lower = interest_topic.to_lowercase();
    let word_count = topic_lower.split_whitespace().count();

    let is_broad = BROAD_INTEREST_TERMS
        .iter()
        .any(|b| topic_lower == *b || topic_lower.contains(b));

    if is_broad {
        scoring_config::SPECIFICITY_BROAD // Broad terms contribute 25% of normal weight
    } else if word_count <= 1 {
        scoring_config::SPECIFICITY_SINGLE_WORD // Single-word terms are moderately specific
    } else {
        scoring_config::SPECIFICITY_MULTI_WORD // Multi-word specific terms get full weight
    }
}

/// Find the best-matching interest for an item and return its specificity weight.
/// Used to attenuate keyword_score for broad interests.
///
/// When the user has very few interests (1-2), ALL interests get full 1.0x
/// weight — someone who only declares "AI" and "Rust" clearly means both.
/// The broad-term discount only kicks in when there are 3+ interests, and
/// at a gentler rate (0.60x for 3-5 interests) than the default (0.25x for 6+).
pub(crate) fn best_interest_specificity_weight(
    title: &str,
    content: &str,
    interests: &[context_engine::Interest],
) -> f32 {
    if interests.is_empty() {
        return 1.0;
    }

    // Focused users: if 1-2 declared interests, trust all of them at full weight.
    // They chose few → each one is definitional, not casual.
    let focused_user = interests.len() <= 2;

    let title_lower = title.to_lowercase();
    let text_lower = format!("{} {}", title_lower, content.to_lowercase());
    let mut best_weight: f32 = 1.0;
    let mut found_match = false;

    for interest in interests {
        let interest_lower = interest.topic.to_lowercase();
        let terms: Vec<&str> = interest_lower.split_whitespace().collect();

        // Check if any term from this interest appears in the item
        let has_hit = terms.iter().any(|term| {
            if term.len() < 2 {
                return false;
            }
            // Fast path: direct substring
            if title_lower.contains(term) || text_lower.contains(term) {
                return true;
            }
            // Alias expansion
            if let Some(group) = aliases::get_aliases(term) {
                if group.iter().any(|alias| {
                    if alias.len() <= 2 {
                        has_word_boundary_match(&title_lower, alias)
                            || has_word_boundary_match(&text_lower, alias)
                    } else {
                        title_lower.contains(alias) || text_lower.contains(alias)
                    }
                }) {
                    return true;
                }
            }
            // Stemmed match
            let term_stem = stemming::stem(term);
            if term_stem.len() >= 3 {
                let words_match = title_lower
                    .split(|c: char| !c.is_alphanumeric())
                    .chain(text_lower.split(|c: char| !c.is_alphanumeric()))
                    .any(|w| w.len() >= 3 && stemming::stem(w) == term_stem);
                if words_match {
                    return true;
                }
            }
            false
        });

        if has_hit {
            let w = if focused_user {
                1.0 // Full weight for focused users
            } else if interests.len() <= 5 {
                // 3-5 interests: softer discount (0.60x floor for broad terms)
                let raw_w = interest_specificity_weight(&interest.topic);
                raw_w.max(0.60)
            } else {
                // 6+ interests: full specificity logic (0.25x for broad)
                interest_specificity_weight(&interest.topic)
            };
            if !found_match || w > best_weight {
                best_weight = w;
                found_match = true;
            }
        }
    }

    if found_match {
        best_weight
    } else {
        1.0 // No keyword match -> don't attenuate
    }
}

/// Short tech keywords that are valid despite being <3 chars.
/// These are common abbreviations that users declare as interests.
const SHORT_TECH_KEYWORDS: &[&str] = &[
    "ai", "ml", "go", "r", "c", "ui", "ux", "db", "os", "ci", "cd", "qa", "js", "ts", "py", "rx",
    "vm", "k8", "tf", "gcp", "aws", "api", "cli", "css", "sql", "llm", "nlp", "cv",
];

/// Negation patterns that indicate a term is mentioned in a negative context.
/// Returns true if the term appears near a negation phrase in the text.
fn is_negated_in_context(term: &str, text: &str) -> bool {
    const NEGATION_PREFIXES: &[&str] = &[
        "not ", "no ", "don't ", "doesn't ", "didn't ", "won't ", "isn't ", "aren't ",
        "without ", "never ", "avoid ", "stop using ", "alternative to ", "instead of ",
        "replace ", "replacing ", "moved away from ", "moving away from ", "migrating from ",
        "leaving ", "dropped ", "dropping ", "removed ", "removing ",
    ];

    let text_lower = text.to_lowercase();
    let term_lower = term.to_lowercase();

    for (idx, _) in text_lower.match_indices(&term_lower) {
        let before_start = idx.saturating_sub(30);
        let before = &text_lower[before_start..idx];
        if NEGATION_PREFIXES.iter().any(|neg| before.ends_with(neg) || before.contains(neg)) {
            return true;
        }
    }
    false
}

/// BM25-inspired term density: rewards content where matched terms appear frequently
/// relative to document length. Returns a multiplier in [1.0, 1.5].
///
/// Uses simplified BM25 formula: tf(t,d) = freq / (freq + k1 * (1 - b + b * dl/avgdl))
/// where k1=1.2, b=0.75, avgdl=150 (typical dev article word count after truncation).
fn term_density_multiplier(term: &str, text: &str) -> f32 {
    let term_lower = term.to_lowercase();
    let freq = text.matches(&term_lower).count() as f32;
    if freq <= 1.0 {
        return 1.0;
    }

    let word_count = text.split_whitespace().count().max(1) as f32;
    let k1: f32 = 1.2;
    let b: f32 = 0.75;
    let avgdl: f32 = 150.0;
    let tf = freq / (freq + k1 * (1.0 - b + b * word_count / avgdl));

    // Map tf (range ~0.45-0.83 for typical values) to a 1.0-1.5 multiplier
    (1.0 + tf * 0.6).min(1.5)
}

/// Keyword-based interest matching: boosts items that literally contain declared interest terms.
/// Complements semantic matching which can miss exact keyword matches.
#[score_component(output_range = "0.0..=1.0")]
pub(crate) fn compute_keyword_interest_score(
    title: &str,
    content: &str,
    interests: &[context_engine::Interest],
) -> f32 {
    if interests.is_empty() {
        return 0.0;
    }

    let title_lower = title.to_lowercase();
    let text_lower = format!("{} {}", title_lower, content.to_lowercase());
    let mut max_score: f32 = 0.0;

    for interest in interests {
        let interest_lower = interest.topic.to_lowercase();
        let terms: Vec<&str> = interest_lower.split_whitespace().collect();
        if terms.is_empty() {
            continue;
        }

        let mut hits = 0.0_f32;
        let mut counted_terms = 0_usize;
        for term in &terms {
            // Skip generic short words, but allow known tech abbreviations
            if term.len() < 2 {
                continue;
            }
            if term.len() < 3 && !SHORT_TECH_KEYWORDS.contains(term) {
                continue;
            }
            counted_terms += 1;

            // For very short terms (1-2 chars), require word boundary match to avoid false positives
            // e.g. "go" shouldn't match "google", "algorithm"
            let matched_title = if term.len() <= 2 {
                has_word_boundary_match(&title_lower, term)
            } else {
                title_lower.contains(term)
            };
            let matched_content = if !matched_title && term.len() <= 2 {
                has_word_boundary_match(&text_lower, term)
            } else if !matched_title {
                text_lower.contains(term)
            } else {
                false
            };

            // Base hit weights: calibrated to leave room for density bonus
            // within the [0, 1] output range (title_exact=0.80, content=0.55,
            // alias same, stem slightly lower)
            let mut term_hit = if matched_title {
                0.80
            } else if matched_content {
                0.55
            } else {
                // Slow path: try alias expansion
                let alias_title_hit = aliases::get_aliases(term)
                    .map(|group| {
                        group.iter().any(|alias| {
                            if alias.len() <= 2 {
                                has_word_boundary_match(&title_lower, alias)
                            } else {
                                title_lower.contains(alias)
                            }
                        })
                    })
                    .unwrap_or(false);

                if alias_title_hit {
                    0.80
                } else {
                    let alias_content_hit = aliases::get_aliases(term)
                        .map(|group| {
                            group.iter().any(|alias| {
                                if alias.len() <= 2 {
                                    has_word_boundary_match(&text_lower, alias)
                                } else {
                                    text_lower.contains(alias)
                                }
                            })
                        })
                        .unwrap_or(false);

                    if alias_content_hit {
                        0.55
                    } else {
                        // Stemmed match
                        let term_stem = stemming::stem(term);
                        if term_stem.len() >= 3 {
                            let title_stem_hit = title_lower
                                .split(|c: char| !c.is_alphanumeric())
                                .any(|w| w.len() >= 3 && stemming::stem(w) == term_stem);
                            let content_stem_hit = !title_stem_hit
                                && text_lower
                                    .split(|c: char| !c.is_alphanumeric())
                                    .any(|w| w.len() >= 3 && stemming::stem(w) == term_stem);

                            if title_stem_hit {
                                0.65
                            } else if content_stem_hit {
                                0.45
                            } else {
                                0.0
                            }
                        } else {
                            0.0
                        }
                    }
                }
            };

            // Density bonus: push score up to 1.0 for content-dense matches
            if term_hit > 0.0 {
                let density = term_density_multiplier(term, &text_lower);
                term_hit *= density;
            }

            // Negation penalty: halve contribution when term appears in negative context
            if term_hit > 0.0 && term.len() >= 3 && is_negated_in_context(term, &text_lower) {
                term_hit *= 0.5;
            }

            hits += term_hit;
        }

        let divisor = counted_terms.max(1) as f32;
        let score = (hits / divisor).min(1.0) * interest.weight;
        max_score = max_score.max(score);
    }

    max_score
}

#[cfg(test)]
mod tests {
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

        // 1-2 interests: focused user, all interests get 1.0x (trust them)
        let few_interests = vec![make("Open Source")];
        let specificity = best_interest_specificity_weight(
            "New open source project for data pipelines",
            "",
            &few_interests,
        );
        assert_eq!(
            specificity, 1.00,
            "Focused user (1-2 interests) should get 1.0 weight even for broad terms"
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
        let score =
            compute_keyword_interest_score("Advanced TypeScript patterns", "", &interests);
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
        let score =
            compute_keyword_interest_score("A resting period for developers", "", &interests);
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
        assert!(dense <= 1.5, "Density bonus should be capped at 1.5, got {}", dense);
    }

    #[test]
    fn test_negation_detection() {
        assert!(is_negated_in_context("react", "we don't use react anymore"));
        assert!(is_negated_in_context("kubernetes", "alternative to kubernetes for small teams"));
        assert!(is_negated_in_context("vue", "moving away from vue to react"));
        assert!(!is_negated_in_context("rust", "learning rust for systems programming"));
        assert!(!is_negated_in_context("python", "python data science tutorial"));
    }

    #[test]
    fn test_negated_term_reduces_score() {
        let make = |topic: &str| vec![context_engine::Interest {
            id: Some(1),
            topic: topic.to_string(),
            weight: 1.0,
            source: context_engine::InterestSource::Explicit,
            embedding: None,
        }];

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
            positive_score, negated_score,
        );
    }

    #[test]
    fn test_dense_content_scores_higher() {
        let make = |topic: &str| vec![context_engine::Interest {
            id: Some(1),
            topic: topic.to_string(),
            weight: 1.0,
            source: context_engine::InterestSource::Explicit,
            embedding: None,
        }];

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
            dense, sparse,
        );
    }
}
