// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! NLP helpers for briefing groundedness validation.
//!
//! Extracted from `briefing_groundedness.rs` to keep both files under the
//! 700-line Rust warning threshold. Contains term extraction, stopword
//! filtering, and grounding checks.

mod stopwords;

use std::collections::HashSet;
use stopwords::{is_phrase_only_stopword, is_stopword};

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
pub(super) fn extract_salient_terms(text: &str) -> Vec<String> {
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
                } else if !cap_run.is_empty() && looks_like_version(stripped) {
                    // Allow version tokens to continue (not start) a
                    // capitalized run — "React 19.2" is a meaningful
                    // compound term even though "19.2" isn't capitalized.
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
                && !is_phrase_only_stopword(stripped)
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

    // --- Phase 4: scoped package identifiers (case-insensitive) ------------
    // Modern dependency/security briefs are dominated by lowercase package
    // names the capitalized phases (2 & 3) never see. Scoped npm names like
    // "@ai-sdk/provider-utils" or "@clerk/clerk-react" are unambiguous package
    // tokens regardless of case, so we surface them here. Bare lowercase names
    // ("axios", "jsonwebtoken") and hyphenated ones ("clerk-react") are handled
    // by the package allowlist in `validate_groundedness_with_packages`, which
    // matches the brief's actual dependencies and so carries no risk of
    // counting ordinary hyphenated English ("real-world", "well-known").
    for token in &tokens {
        let stripped = token.trim_matches(|c: char| {
            matches!(
                c,
                '.' | ',' | ';' | ':' | '!' | '?' | '(' | ')' | '"' | '\'' | '`' | '[' | ']'
            )
        });
        if stripped.starts_with('@')
            && stripped.contains('/')
            && stripped.chars().filter(|c| c.is_alphabetic()).count() >= 3
        {
            let key = stripped.to_lowercase();
            if !seen.contains(&key) {
                seen.insert(key);
                out.push(stripped.to_string());
            }
        }
    }

    out
}

/// Returns true for tokens that look like version numbers ("19.2", "v2.0",
/// "1.38.3"). Used by Phase 2 to let versions continue a capitalized run
/// so that "React 19.2" forms a single phrase.
fn looks_like_version(token: &str) -> bool {
    let chars: Vec<char> = token.chars().collect();
    if chars.is_empty() {
        return false;
    }
    let start =
        if (chars[0] == 'v' || chars[0] == 'V') && chars.len() > 1 && chars[1].is_ascii_digit() {
            1
        } else if chars[0].is_ascii_digit() {
            0
        } else {
            return false;
        };
    let mut saw_dot = false;
    for &c in &chars[start..] {
        if c == '.' {
            saw_dot = true;
        } else if !c.is_ascii_digit() {
            return false;
        }
    }
    saw_dot
}

pub(super) fn looks_like_date(token: &str) -> bool {
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

// ============================================================================
// Grounding check
// ============================================================================

/// A term is grounded if it appears as a substring in any corpus entry,
/// case-insensitively. For multi-word phrases we also accept partial
/// matches where ALL component words appear (in any order) in a single
/// corpus entry — this tolerates the LLM reordering "React Server
/// Components" as "Server Components in React".
pub(super) fn is_term_grounded(term: &str, corpus_lower: &[String]) -> bool {
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
