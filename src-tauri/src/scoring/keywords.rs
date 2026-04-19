// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::calibration::BROAD_INTEREST_TERMS;
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
            term.len() >= 2 && (title_lower.contains(term) || text_lower.contains(term))
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

            if matched_title {
                hits += 1.5; // title match = 1.5x weight
            } else if matched_content {
                hits += 1.0;
            }
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
}
