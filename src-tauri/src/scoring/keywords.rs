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
            if term.len() < 3 && !SHORT_TECH_KEYWORDS.contains(term) {
                return false;
            }
            // Fast path: direct match (word-boundary for short terms)
            if term.len() <= 2 {
                if has_word_boundary_match(&title_lower, term)
                    || has_word_boundary_match(&text_lower, term)
                {
                    return true;
                }
            } else if title_lower.contains(term) || text_lower.contains(term) {
                return true;
            }
            // Alias expansion
            if let Some(group) = aliases::get_aliases(term) {
                if group.iter().any(|alias| {
                    if alias.len() <= 2 || AMBIGUOUS_ALIASES.contains(alias) {
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
                    .any(|w| w.len() >= 3 && stemming::stems_equiv(&stemming::stem(w), &term_stem));
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

/// Alias terms that are common English words and need word-boundary matching
/// to avoid false positives (e.g., "express delivery" matching Express.js interest).
const AMBIGUOUS_ALIASES: &[&str] = &[
    "next",
    "solid",
    "fly",
    "echo",
    "fiber",
    "gin",
    "spring",
    "express",
    "compose",
    "helm",
    "rest",
    "elastic",
    "container",
    "phoenix",
];

/// Negation patterns that indicate a term is mentioned in a negative context.
/// Returns true if the term appears near a negation phrase in the text.
fn is_negated_in_context(term: &str, text: &str) -> bool {
    const NEGATION_PREFIXES: &[&str] = &[
        "not ",
        "no ",
        "don't ",
        "doesn't ",
        "didn't ",
        "won't ",
        "isn't ",
        "aren't ",
        "without ",
        "never ",
        "avoid ",
        "stop using ",
        "alternative to ",
        "instead of ",
        "replace ",
        "replacing ",
        "moved away from ",
        "moving away from ",
        "migrating from ",
        "leaving ",
        "dropped ",
        "dropping ",
        "removed ",
        "removing ",
        "don't use ",
        "doesn't use ",
        "didn't use ",
        "won't use ",
        "not using ",
        "stopped using ",
        "quit ",
        "quitting ",
    ];

    let text_lower = text.to_lowercase();
    let term_lower = term.to_lowercase();

    for (idx, _) in text_lower.match_indices(&term_lower) {
        let before_start = text_lower.floor_char_boundary(idx.saturating_sub(30));
        let before = &text_lower[before_start..idx];
        if NEGATION_PREFIXES.iter().any(|neg| before.ends_with(neg)) {
            return true;
        }
    }
    false
}

/// Count word-boundary-aware occurrences of a term in text.
fn count_word_occurrences(term: &str, text: &str) -> usize {
    let mut count = 0;
    for (i, _) in text.match_indices(term) {
        let before_ok = i == 0
            || !text
                .as_bytes()
                .get(i.wrapping_sub(1))
                .map_or(false, |b| b.is_ascii_alphanumeric());
        let after_pos = i + term.len();
        let after_ok = after_pos >= text.len()
            || !text
                .as_bytes()
                .get(after_pos)
                .map_or(false, |b| b.is_ascii_alphanumeric());
        if before_ok && after_ok {
            count += 1;
        }
    }
    count
}

/// BM25-inspired term density: rewards content where matched terms appear frequently
/// relative to document length. Returns a multiplier in [1.0, 1.5].
///
/// Uses simplified BM25 formula: tf(t,d) = freq / (freq + k1 * (1 - b + b * dl/avgdl))
/// where k1=1.2, b=0.75, avgdl=150 (typical dev article word count after truncation).
fn term_density_multiplier(term: &str, text: &str) -> f32 {
    let term_lower = term.to_lowercase();
    let freq = count_word_occurrences(&term_lower, text) as f32;
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
    let content_lower = content.to_lowercase();
    let text_lower = format!("{} {}", title_lower, content_lower);
    let mut max_score: f32 = 0.0;

    for interest in interests {
        let interest_lower = interest.topic.to_lowercase();
        let terms: Vec<&str> = interest_lower.split_whitespace().collect();
        if terms.is_empty() {
            continue;
        }

        // Multi-word phrase check: exact phrase match is the strongest keyword signal
        if terms.len() > 1 {
            let phrase = &interest_lower;
            let in_title = title_lower.contains(phrase.as_str());
            let in_content = !in_title && text_lower.contains(phrase.as_str());
            if in_title || in_content {
                let mut phrase_score = if in_title { 0.95 } else { 0.80 };
                let density = term_density_multiplier(phrase, &text_lower);
                phrase_score = (phrase_score * density).min(1.0);
                if is_negated_in_context(phrase, &text_lower) {
                    phrase_score *= 0.5;
                }
                max_score = max_score.max(phrase_score * interest.weight);
                continue;
            }
        }

        let mut hits = 0.0_f32;
        let mut counted_terms = 0_usize;
        for term in &terms {
            // Skip generic short words, but allow known tech abbreviations
            if term.len() < 3 && !SHORT_TECH_KEYWORDS.contains(term) {
                continue;
            }
            counted_terms += 1;

            // Determine match and effective search term for density/negation
            let (base_hit, search_term): (f32, Option<&str>) = {
                // Direct match check (word-boundary for short terms)
                let direct_title = if term.len() <= 2 {
                    has_word_boundary_match(&title_lower, term)
                } else {
                    title_lower.contains(term)
                };
                let direct_content = if !direct_title {
                    if term.len() <= 2 {
                        has_word_boundary_match(&text_lower, term)
                    } else {
                        text_lower.contains(term)
                    }
                } else {
                    false
                };

                if direct_title {
                    (0.80, Some(*term))
                } else if direct_content {
                    (0.55, Some(*term))
                } else {
                    // Alias expansion — find which alias actually matched
                    let alias_match: Option<(&str, bool)> =
                        aliases::get_aliases(term).and_then(|group| {
                            for alias in group.iter() {
                                let needs_boundary =
                                    alias.len() <= 2 || AMBIGUOUS_ALIASES.contains(alias);
                                let in_title = if needs_boundary {
                                    has_word_boundary_match(&title_lower, alias)
                                } else {
                                    title_lower.contains(alias)
                                };
                                if in_title {
                                    return Some((*alias, true));
                                }
                                let in_content = if needs_boundary {
                                    has_word_boundary_match(&text_lower, alias)
                                } else {
                                    text_lower.contains(alias)
                                };
                                if in_content {
                                    return Some((*alias, false));
                                }
                            }
                            None
                        });

                    if let Some((matched_alias, in_title)) = alias_match {
                        (if in_title { 0.80 } else { 0.55 }, Some(matched_alias))
                    } else {
                        // Stemmed match (no effective term for density/negation)
                        let term_stem = stemming::stem(term);
                        if term_stem.len() >= 3 {
                            let title_stem_hit =
                                title_lower.split(|c: char| !c.is_alphanumeric()).any(|w| {
                                    w.len() >= 3
                                        && stemming::stems_equiv(&stemming::stem(w), &term_stem)
                                });
                            let content_stem_hit = !title_stem_hit
                                && text_lower.split(|c: char| !c.is_alphanumeric()).any(|w| {
                                    w.len() >= 3
                                        && stemming::stems_equiv(&stemming::stem(w), &term_stem)
                                });

                            if title_stem_hit {
                                (0.65, None)
                            } else if content_stem_hit {
                                (0.45, None)
                            } else {
                                (0.0, None)
                            }
                        } else {
                            (0.0, None)
                        }
                    }
                }
            };

            let mut term_hit = base_hit;

            // First-paragraph boost: content matches in the first 200 chars
            // are stronger signals — the article is likely ABOUT this topic
            if term_hit > 0.0 && term_hit < 0.80 && content_lower.len() >= 3 {
                let effective = search_term.unwrap_or(term);
                let end = content_lower.floor_char_boundary(content_lower.len().min(200));
                if content_lower[..end].contains(effective) {
                    term_hit = (term_hit + 0.10).min(0.80);
                }
            }

            // Density bonus: only for direct/alias matches where we know the search term
            if term_hit > 0.0 {
                if let Some(st) = search_term {
                    let density = term_density_multiplier(st, &text_lower);
                    term_hit *= density;
                }
            }

            // Negation penalty: only for direct/alias matches
            if term_hit > 0.0 {
                if let Some(st) = search_term {
                    if st.len() >= 3 && is_negated_in_context(st, &text_lower) {
                        term_hit *= 0.5;
                    }
                }
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
        let full_score =
            compute_keyword_interest_score("Learning Rust", "rust guide", &full_weight);
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
}
