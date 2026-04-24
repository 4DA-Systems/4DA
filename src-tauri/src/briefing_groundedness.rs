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
        // A good synthesis MUST reference something specific from the
        // signals — at least 2 salient terms (a technology name, a
        // version, a project). Fewer than 2 means the output is too
        // generic (e.g. "Prioritize configuring your tech stack") and
        // isn't grounded in any actual signal content.
        if self.total_terms < 2 {
            return false;
        }
        // With exactly 2 terms, require both to be grounded — one
        // miss out of two is a 50% hallucination rate.
        if self.total_terms == 2 {
            return self.grounded_terms == self.total_terms;
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
    // Re-enabled with stopword filter (2026-04-25). Previously disabled
    // because "Reactive", "Security", etc. caused false rejections, but
    // the stopword list now covers 200+ common English words. This phase
    // catches hallucinated proper nouns like "Stripe" or "Postgres" that
    // don't appear in any source item — critical for the specificity
    // floor (is_acceptable rejects output with <2 salient terms).
    for token in &tokens {
        let stripped = token.trim_matches(|c: char| !c.is_alphanumeric());
        if let Some(first) = stripped.chars().next() {
            if first.is_uppercase()
                && stripped.len() >= 3
                && !is_stopword(stripped)
                && stripped.chars().filter(|c| c.is_alphabetic()).count() >= 3
            {
                let key = stripped.to_lowercase();
                if !seen.contains(&key) {
                    seen.insert(key);
                    out.push(stripped.to_string());
                }
            }
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
        "Meanwhile",
        "However",
        "Therefore",
        "Furthermore",
        "Additionally",
        "Moreover",
        "Nevertheless",
        "Nonetheless",
        "Consequently",
        "Subsequently",
        "Accordingly",
        "Overall",
        "Notably",
        "Specifically",
        "Importantly",
        "Essentially",
        "Particularly",
        "Ultimately",
        "Interestingly",
        "Several",
        "Various",
        "Multiple",
        "Significant",
        "Large",
        "Small",
        "Major",
        "Minor",
        "Recent",
        "Current",
        "Previous",
        "First",
        "Second",
        "Third",
        "Final",
        "Early",
        "Late",
        "Next",
        "Last",
        "Other",
        "Another",
        "Every",
        "Each",
        "Both",
        "Many",
        "Most",
        "Some",
        "Any",
        "All",
        "Such",
        "Same",
        "New",
        "Old",
        "Modern",
        "Legacy",
        "Popular",
        "Common",
        "General",
        "Specific",
        "Special",
        "Potential",
        "Possible",
        "Available",
        "Existing",
        "Different",
        "Similar",
        "Certain",
        "Related",
        "Based",
        "Given",
        "Note",
        "Keep",
        "Make",
        "Take",
        "Use",
        "Using",
        "Action",
        "Impact",
        "Focus",
        "Language",
        "Models",
        "Model",
        "System",
        "Systems",
        "Paper",
        "Papers",
        "Research",
        "Study",
        "Approach",
        "Method",
        "Results",
        "Analysis",
        // Synthesis vocabulary: words the LLM uses when clustering/summarizing
        "Cluster",
        "Clusters",
        "Signal",
        "Signals",
        "Source",
        "Sources",
        "Theme",
        "Themes",
        "Thread",
        "Threads",
        "Trend",
        "Trends",
        "Publish",
        "Published",
        "Publishing",
        "Hit",
        "Mass",
        "Three",
        "Two",
        "Four",
        "Five",
        "Six",
        "Seven",
        "Eight",
        "Nine",
        "Ten",
        "Dozen",
        "Half",
        "Zero",
        "Fast",
        "Slow",
        "Quick",
        "Rapid",
        "Growing",
        "Rising",
        "Emerging",
        "Maturing",
        "Accelerating",
        "Increasing",
        "Gaining",
        "Traction",
        "Momentum",
        "Security",
        "Secure",
        "Supply",
        "Chain",
        "Audit",
        "Auditing",
        "Hardening",
        "Compiler",
        "Optimizations",
        "Optimization",
        "Performance",
        "Improvements",
        "Improvement",
        "Build",
        "Builds",
        "Config",
        "Configuration",
        "Changes",
        "Change",
        "Teams",
        "Team",
        "Guides",
        "Guide",
        "Week",
        "Weeks",
        "Month",
        "Months",
        "Year",
        "Years",
        "Separately",
        "Worth",
        "Evaluating",
        "Evaluate",
        "Independent",
        "Independently",
        "Testing",
        "Test",
        "Tests",
        "Tooling",
        "Tool",
        "Tools",
        "Framework",
        "Frameworks",
        "Library",
        "Libraries",
        "Package",
        "Packages",
        "Version",
        "Versions",
        "Pinned",
        "Release",
        "Releases",
        "Breaking",
        "Feature",
        "Features",
        "Area",
        "Areas",
        "Demand",
        "Attention",
        "Immediate",
        "Nothing",
        "Scatter",
        "Across",
        "Unrelated",
        "Enough",
        "Demand",
        "Act",
        "Check",
        "Look",
        "Approach",
        "Approaches",
        "Development",
        "Developer",
        "Developers",
        "Engineering",
        "Production",
        "Deployment",
        "Pipeline",
        "Pipelines",
        "Workflow",
        "Workflows",
        "Integration",
        "Continuous",
        "Practices",
        "Practice",
        "Patterns",
        "Code",
        "Codebase",
        "Application",
        "Applications",
        "Project",
        "Projects",
        "Open",
        "Weights",
        "Repository",
        "Repositories",
        "Repo",
        "Repos",
        // Common verbs that get capitalized at sentence starts
        "Recommend",
        "Recommended",
        "Upgrade",
        "Downgrade",
        "Install",
        "Installed",
        "Configure",
        "Configured",
        "Migrate",
        "Migrating",
        "Migration",
        "Deploy",
        "Deployed",
        "Implement",
        "Implemented",
        "Implement",
        "Evaluate",
        "Evaluated",
        "Run",
        "Running",
        "Start",
        "Started",
        "Stop",
        "Stopped",
        "Enable",
        "Enabled",
        "Disable",
        "Disabled",
        "Include",
        "Including",
        "Require",
        "Required",
        "Requires",
        "Suggest",
        "Suggests",
        "Suggested",
        "Indicate",
        "Indicates",
        "Continue",
        "Continues",
        "Continued",
        "Prioritize",
        "Prevent",
        "Avoid",
        "Reduce",
        "Improve",
        "Address",
        "Ensure",
        "Verify",
        "Validate",
        "Review",
        "Audit",
        "Scan",
        "Benchmark",
        "Monitor",
        "Switch",
        "Replace",
        "Remove",
        "Delete",
        "Fix",
        "Patch",
        "Pin",
        "Lock",
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

        // Partial grounding: if any 2+ word subsequence appears in the corpus,
        // the phrase is partially grounded (e.g., "Vulnerability Alerts GitHub
        // Actions" is grounded if "GitHub Actions" appears in any entry).
        for window_size in (2..words.len()).rev() {
            for window in words.windows(window_size) {
                let sub = window.join(" ");
                if corpus_lower.iter().any(|c| c.contains(&sub)) {
                    return true;
                }
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
