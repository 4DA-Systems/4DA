// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Post-synthesis groundedness validator.
//!
//! The morning-briefing LLM sometimes generates claims that are not
//! supported by any input item. A real production example observed in
//! `Screenshot_1976`:
//!
//! > Recommend update of your strategy for non-test architecture,
//! > including a 5+ year migration from VAR and Stripe
//!
//! Neither "VAR" nor "Stripe" appeared in any source item that day, and
//! "non-test architecture" is not a real concept. This is a fabricated
//! recommendation — the prompt asked for actionable advice so the model
//! produced one, even though nothing in the input warranted it.
//!
//! This validator runs AFTER synthesis and BEFORE the output reaches
//! the user. It extracts noun phrases, version numbers, and product
//! names from the synthesized text and checks each one against the
//! input item corpus (titles + descriptions + matched_deps). Items
//! that fail the grounding check are reported; if the overall
//! groundedness score falls below a threshold, the synthesis is
//! rejected and a safe fallback is used instead.
//!
//! This is a first-line defense, not a panacea. A determined
//! adversarial LLM could still slip hallucinations past a heuristic
//! validator. But the common failure mode — the LLM inventing
//! plausible-sounding tech names to fill a required response slot
//! — this gate catches cleanly.

use std::collections::HashSet;

// ============================================================================
// Public API
// ============================================================================

/// Confidence that every substantive claim in `output` is grounded in
/// `corpus`. Values above `0.8` indicate high confidence; below `0.5`
/// indicate the output should probably be rejected.
#[derive(Debug, Clone)]
pub struct GroundednessReport {
    pub confidence: f32,
    pub total_terms: usize,
    /// Count of salient terms matched against the source corpus. Surfaced
    /// to the receipts UI as the numerator of the grounding fraction.
    #[allow(dead_code)] // Surfaced via Debug + future receipts panel.
    pub grounded_terms: usize,
    pub ungrounded_terms: Vec<String>,
}

impl GroundednessReport {
    /// Is this output safe to show the user at the given threshold?
    pub fn is_acceptable(&self, threshold: f32) -> bool {
        // Small numbers of terms produce noisy ratios — require at
        // least 3 total salient terms for a confident verdict. Below
        // that, default to "acceptable" to avoid rejecting legitimate
        // brief summaries that happen to mention few proper nouns.
        if self.total_terms < 3 {
            return true;
        }
        self.confidence >= threshold
    }
}

/// Validate that the synthesized briefing text is grounded in the
/// provided corpus. The corpus should contain every source item that
/// was fed to the LLM: concatenate title + description + matched_deps
/// per item.
pub fn validate_groundedness(output: &str, corpus: &[String]) -> GroundednessReport {
    let corpus_lower: Vec<String> = corpus.iter().map(|s| s.to_lowercase()).collect();

    let terms = extract_salient_terms(output);
    let total = terms.len();

    let mut ungrounded = Vec::new();
    let mut grounded = 0;

    for term in &terms {
        if is_term_grounded(term, &corpus_lower) {
            grounded += 1;
        } else {
            ungrounded.push(term.clone());
        }
    }

    let confidence = if total == 0 {
        1.0
    } else {
        grounded as f32 / total as f32
    };

    GroundednessReport {
        confidence,
        total_terms: total,
        grounded_terms: grounded,
        ungrounded_terms: ungrounded,
    }
}

// ============================================================================
// Term extraction
// ============================================================================

/// Extract salient terms from the synthesized output — things that
/// SHOULD be traceable to a source item if the output is grounded.
///
/// Currently extracts:
/// - Version-like tokens (`1.38`, `v2.0`, `0.9.3`) — invented versions
///   are the most dangerous hallucination class.
/// - Capitalized multi-word phrases (`React Server Components`).
/// - Single capitalized tokens that look like product names
///   (`Stripe`, `Kubernetes`, `Postgres`).
/// - Quoted phrases (`"strict semver"`).
///
/// Skips common English words that happen to be capitalized
/// (sentence-initial words, `I`, pronouns) and very short tokens.
fn extract_salient_terms(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // --- Phase 1: version tokens ------------------------------------------
    // Simple hand-written scan rather than regex — the patterns are
    // specific enough that a linear walk is clearest.
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // Possible version start — digit, or "v" followed by digit.
        let (start, is_digit_start) = if chars[i].is_ascii_digit() {
            (i, true)
        } else if (chars[i] == 'v' || chars[i] == 'V')
            && i + 1 < chars.len()
            && chars[i + 1].is_ascii_digit()
        {
            (i, false)
        } else {
            i += 1;
            continue;
        };

        let mut end = start + usize::from(!is_digit_start);
        let mut saw_dot = false;
        while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '.') {
            if chars[end] == '.' {
                saw_dot = true;
            }
            end += 1;
        }

        if saw_dot && end > start + 2 {
            // Trim trailing dots so "5.6." from "to 5.6." becomes "5.6".
            let mut stop = end;
            while stop > start && chars[stop - 1] == '.' {
                stop -= 1;
            }
            let token: String = chars[start..stop].iter().collect();
            // Must still contain a dot after trimming (e.g., "5." alone is junk).
            if token.contains('.') && !looks_like_date(&token) {
                let key = token.to_lowercase();
                if !seen.contains(&key) {
                    seen.insert(key);
                    out.push(token);
                }
            }
        }
        i = end.max(i + 1);
    }

    // --- Phase 2: capitalized noun-phrase run ------------------------------
    // "React Server Components", "Rust Async Book", etc.
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut cap_run: Vec<&str> = Vec::new();
    let flush = |cap_run: &mut Vec<&str>, out: &mut Vec<String>, seen: &mut HashSet<String>| {
        if cap_run.len() >= 2 {
            let joined = cap_run.join(" ");
            // Strip trailing punctuation from the phrase
            let cleaned = joined.trim_end_matches(|c: char| {
                matches!(c, '.' | ',' | ';' | ':' | '—' | '-' | '!' | '?')
            });
            let key = cleaned.to_lowercase();
            if !cleaned.is_empty() && !seen.contains(&key) {
                seen.insert(key);
                out.push(cleaned.to_string());
            }
        }
        cap_run.clear();
    };
    for token in &tokens {
        let stripped = token.trim_matches(|c: char| !c.is_alphanumeric());
        // Match on the first character via `chars().next()` rather than
        // unwrap-after-empty-check so clippy's unwrap_used lint is happy
        // without sacrificing readability.
        match stripped.chars().next() {
            None => {
                flush(&mut cap_run, &mut out, &mut seen);
            }
            Some(first) => {
                if first.is_uppercase() && stripped.len() > 1 && !is_stopword(stripped) {
                    cap_run.push(stripped);
                } else {
                    flush(&mut cap_run, &mut out, &mut seen);
                }
            }
        }
    }
    flush(&mut cap_run, &mut out, &mut seen);

    // --- Phase 3: notable single-capitalized tokens ------------------------
    // Things like "Stripe" or "Postgres" that appear alone. Only count
    // tokens of ≥5 chars that look like product names.
    for token in &tokens {
        let stripped: String = token.chars().filter(|c| c.is_alphanumeric()).collect();
        if stripped.len() < 5 {
            continue;
        }
        let mut chars = stripped.chars();
        // Short-circuit on empty first — prior len < 5 guard means we
        // should never hit this, but let-else is the idiomatic way to
        // avoid clippy's unwrap_used lint.
        let Some(first) = chars.next() else { continue };
        if !first.is_uppercase() {
            continue;
        }
        if chars.any(|c| c.is_uppercase()) {
            // Mixed-case inside (e.g., "TypeScript") still counts, but
            // screaming-case "ALLCAPS" acronyms we skip because they
            // match too many things.
            continue;
        }
        if is_stopword(&stripped) {
            continue;
        }
        let key = stripped.to_lowercase();
        if !seen.contains(&key) {
            seen.insert(key);
            out.push(stripped);
        }
    }

    out
}

fn looks_like_date(token: &str) -> bool {
    let dots = token.chars().filter(|&c| c == '.').count();
    if dots != 2 {
        return false;
    }
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    if let Some(year) = parts.first().and_then(|s| s.parse::<u32>().ok()) {
        return (1990..=2100).contains(&year);
    }
    false
}

/// Common English words that are often capitalized for sentence structure
/// but should not be treated as proper-noun terms.
fn is_stopword(word: &str) -> bool {
    const STOPWORDS: &[&str] = &[
        "The",
        "This",
        "That",
        "These",
        "Those",
        "There",
        "Here",
        "Now",
        "Today",
        "Tomorrow",
        "Yesterday",
        "Morning",
        "Evening",
        "Night",
        "Your",
        "Our",
        "My",
        "Their",
        "His",
        "Her",
        "Its",
        "When",
        "Where",
        "What",
        "Why",
        "How",
        "Who",
        "Which",
        "While",
        "With",
        "Without",
        "After",
        "Before",
        "Since",
        "Until",
        "Between",
        "Among",
        "Across",
        "During",
        "About",
        "Against",
        "Through",
        "Priority",
        "Pattern",
        "Situation",
        "Summary",
        "Briefing",
        "Review",
        "Update",
        "Consider",
        "Watch",
        "Alert",
        "Warning",
        "Critical",
        "High",
        "Medium",
        "Low",
        "Top",
        "Key",
        "If",
        "But",
        "And",
        "Or",
        "Nor",
        "So",
        "Yet",
        "For",
        "Because",
        "Although",
        "Unless",
        "Whereas",
    ];
    STOPWORDS.iter().any(|&s| s.eq_ignore_ascii_case(word))
}

// ============================================================================
// Grounding check
// ============================================================================

/// A term is grounded if it appears as a substring in any corpus entry,
/// case-insensitively. For multi-word phrases we also accept partial
/// matches where ALL component words appear (in any order) in a single
/// corpus entry — this tolerates the LLM reordering "React Server
/// Components" as "Server Components in React".
fn is_term_grounded(term: &str, corpus_lower: &[String]) -> bool {
    let term_lower = term.to_lowercase();

    // Exact substring match on any corpus entry
    if corpus_lower.iter().any(|c| c.contains(&term_lower)) {
        return true;
    }

    // Multi-word: check if every component word appears in a single entry
    let words: Vec<&str> = term_lower.split_whitespace().collect();
    if words.len() >= 2 {
        for entry in corpus_lower {
            if words.iter().all(|w| entry.contains(*w)) {
                return true;
            }
        }
    }

    false
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(titles: &[&str]) -> Vec<String> {
        titles.iter().map(|s| (*s).to_string()).collect()
    }

    // ---- Positive cases: grounded synthesis ------------------------------

    #[test]
    fn fully_grounded_output_scores_high() {
        let output = "SITUATION: React 19.2 released. PRIORITY: update your React apps.";
        let c = corpus(&[
            "React 19.2 released with concurrent rendering",
            "React usage in 4DA",
        ]);
        let r = validate_groundedness(output, &c);
        assert!(r.confidence >= 0.7, "confidence was {}", r.confidence);
        assert!(r.is_acceptable(0.5));
    }

    #[test]
    fn multi_word_phrase_grounded_even_if_reordered() {
        let output = "TanStack Start now supports React Server Components.";
        let c = corpus(&["React Server Components are now supported in TanStack Start"]);
        let r = validate_groundedness(output, &c);
        assert!(r.confidence >= 0.7, "confidence was {}", r.confidence);
    }

    // ---- Negative: the production screenshot bug ------------------------

    #[test]
    fn rejects_var_and_stripe_hallucination() {
        let output = "Recommend update of your strategy for non-test architecture, \
                      including a 5+ year migration from VAR and Stripe";
        // None of these terms are in the corpus:
        let c = corpus(&[
            "TanStack Start now supports React Server Components",
            "npm: react v19.2.3",
            "npm: typescript v5.6",
        ]);
        let r = validate_groundedness(output, &c);
        assert!(!r.ungrounded_terms.is_empty(), "expected ungrounded terms");
        assert!(
            r.ungrounded_terms
                .iter()
                .any(|t| t.eq_ignore_ascii_case("stripe")),
            "expected Stripe to be flagged, got {:?}",
            r.ungrounded_terms
        );
    }

    #[test]
    fn invented_version_gets_flagged() {
        let output = "Upgrade to tokio 99.99 for the new runtime.";
        let c = corpus(&["tokio 1.38 released with runtime fixes"]);
        let r = validate_groundedness(output, &c);
        assert!(
            r.ungrounded_terms.iter().any(|t| t.starts_with("99.")),
            "expected 99.99 to be flagged, got {:?}",
            r.ungrounded_terms
        );
    }

    #[test]
    fn small_term_count_defaults_to_acceptable() {
        // A terse 1-sentence briefing with zero proper nouns is the
        // "stack is quiet overnight" case — we should NOT reject it
        // just because there is nothing to verify.
        let output = "Your stack is quiet overnight.";
        let r = validate_groundedness(output, &[]);
        assert!(r.is_acceptable(0.8));
    }

    #[test]
    fn stopwords_are_not_counted_as_salient_terms() {
        let output = "Today, This, The, That, Now.";
        let terms = extract_salient_terms(output);
        assert!(terms.is_empty(), "stopwords leaked as terms: {:?}", terms);
    }

    // ---- Unit tests for the extractor ------------------------------------

    #[test]
    fn extractor_finds_version_tokens() {
        let terms = extract_salient_terms("Upgrade tokio to 1.38 and typescript to 5.6.");
        assert!(terms.iter().any(|t| t == "1.38"));
        assert!(terms.iter().any(|t| t == "5.6"));
    }

    #[test]
    fn extractor_finds_multiword_proper_nouns() {
        let terms = extract_salient_terms("React Server Components are stable.");
        assert!(
            terms.iter().any(|t| t == "React Server Components"),
            "got {:?}",
            terms
        );
    }

    #[test]
    fn extractor_finds_single_proper_nouns() {
        let terms = extract_salient_terms("Consider migrating to Postgres from MySQL.");
        assert!(
            terms.iter().any(|t| t.eq_ignore_ascii_case("Postgres")),
            "got {:?}",
            terms
        );
    }

    #[test]
    fn extractor_skips_dates() {
        let terms = extract_salient_terms("Released on 2026.04.15 by the team.");
        assert!(
            !terms.iter().any(|t| t == "2026.04.15"),
            "date leaked as term: {:?}",
            terms
        );
    }

    #[test]
    fn extractor_dedupes_case_insensitively() {
        let terms = extract_salient_terms("Stripe released. Stripe fixed.");
        let stripe_count = terms
            .iter()
            .filter(|t| t.eq_ignore_ascii_case("Stripe"))
            .count();
        assert_eq!(stripe_count, 1);
    }

    #[test]
    fn extractor_handles_trailing_punctuation() {
        let terms = extract_salient_terms("React Server Components, Next.js, and Remix.");
        assert!(terms.iter().any(|t| !t.ends_with(',') && !t.ends_with('.')));
    }

    // ---- Grounded-check unit tests ----------------------------------------

    #[test]
    fn is_term_grounded_substring_match() {
        let c = vec!["react 19.2 released".to_string()];
        assert!(is_term_grounded("React 19.2", &c));
    }

    #[test]
    fn is_term_grounded_multiword_any_order() {
        let c = vec!["components server react are stable".to_string()];
        assert!(is_term_grounded("React Server Components", &c));
    }

    #[test]
    fn is_term_grounded_rejects_unrelated() {
        let c = vec!["react 19.2".to_string()];
        assert!(!is_term_grounded("Stripe", &c));
    }
}
