// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

#[path = "briefing_groundedness_nlp/mod.rs"]
mod nlp;
use nlp::*;

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
    // REMOVE BY 2026-08-01
    #[allow(dead_code)] // Serde: populated during grounding analysis
    pub grounded_terms: usize,
    pub ungrounded_terms: Vec<String>,
}

impl GroundednessReport {
    /// Is this output safe to show the user at the given threshold?
    pub fn is_acceptable(&self, threshold: f32) -> bool {
        // A good synthesis MUST reference something specific from the
        // signals — at least 2 salient terms (a technology name, a
        // version, a project). Fewer than 2 means the output is too
        // generic (e.g. "Prioritize configuring your tech stack") and
        // isn't grounded in any actual signal content.
        if self.total_terms < 2 {
            return false;
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
    fn generic_output_with_no_terms_is_rejected() {
        // A synthesis with zero salient terms is too generic to be
        // useful — it's not referencing anything specific from the
        // signals. The specificity floor rejects this.
        let output = "Your stack is quiet overnight.";
        let r = validate_groundedness(output, &[]);
        assert!(!r.is_acceptable(0.8));
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

    #[test]
    fn llm_expansion_with_few_terms_rejected_by_specificity_floor() {
        // This output rephrases a title without naming specific
        // technologies — the kind of vague synthesis a small model
        // produces. The specificity floor correctly rejects it because
        // it extracts fewer than 2 salient terms (only "LLMs").
        let output = "Meanwhile, Large Language Models (LLMs) continue to show \
                      limited environmental curiosity in agent scenarios.";
        let c = corpus(&[
            "Agents Explore but Agents Ignore: LLMs Lack Environmental Curiosity",
            "Beyond the YAML File: Understanding Real-World GitHub Actions Workflow Adoption",
        ]);
        let r = validate_groundedness(output, &c);
        assert!(
            !r.is_acceptable(0.65),
            "vague rephrasing with <2 salient terms should be rejected"
        );
    }
}
