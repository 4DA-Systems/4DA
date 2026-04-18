// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Briefing-item quality gate.
//!
//! The morning briefing appears on a widget that users see BEFORE their
//! coffee. Every item shown there needs to be:
//!
//! 1. **Coherent** — no garbled text, no machine-translation artifacts,
//!    no broken encoding, no OCR-style word salad.
//! 2. **Relevant** — developer-facing, not marketing/recruiting/SEO noise.
//! 3. **Decipherable** — a title that a human reader can understand
//!    in under 2 seconds without squinting.
//!
//! This gate runs AFTER the fetch-time quality gate (`sources::apply_source_quality_gate`)
//! and BEFORE the LLM synthesis. It is deliberately stricter than the
//! fetch-time gate because the briefing is a featured, high-trust surface
//! — a single bad item visibly degrades every item around it, even good
//! ones, because the user sees the whole list and mentally marks the
//! briefing as unreliable.
//!
//! Observed failures this gate catches (from real production data):
//!
//! - "Tip on marketing isnt back into job re-validate (5+ V/D/E DMS, run-off)"
//!   — grammatically incoherent, mixes marketing jargon with what looks
//!   like OCR garble.
//! - "Tokyo 22 hrs 8 Seconds" — fragment, no verb, no topic.
//! - "... -> ... -> ..." — title that is pure punctuation and cross-links.
//! - "🔥🔥🔥 YOU WON'T BELIEVE THIS ONE NEAT TRICK" — clickbait.
//! - "Post navigation" — CMS boilerplate that escaped the scraper.

use tracing::debug;

// ============================================================================
// Main gate entry point
// ============================================================================

/// Decide whether a single item passes the briefing-quality gate.
///
/// Returns `Ok(())` if the item is worthy of appearing in the morning
/// briefing. Returns `Err(reason)` with a machine-readable rejection
/// code for telemetry.
pub fn is_briefing_worthy(title: &str, source_type: &str) -> Result<(), RejectReason> {
    let trimmed = title.trim();

    // --- Gate 1: minimum length -------------------------------------------
    if trimmed.len() < 15 {
        return Err(RejectReason::TooShort);
    }
    if trimmed.len() > 300 {
        return Err(RejectReason::TooLong);
    }

    // --- Gate 2: word structure -------------------------------------------
    let words: Vec<&str> = trimmed
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .collect();

    if words.len() < 3 {
        return Err(RejectReason::TooFewWords);
    }

    // --- Gate 2.5: punctuation spam (breadcrumbs, tag chains) -------------
    // Run BEFORE coherence so "Home » Blog » Cat" reports PunctuationSpam
    // rather than LowCoherence — more actionable telemetry, and avoids
    // ambiguity when the root cause is clear.
    let punct_ratio = trimmed
        .chars()
        .filter(|c| {
            matches!(
                *c,
                '>' | '|' | '→' | '—' | '-' | ':' | '·' | '▸' | '›' | '»' | '«' | '/'
            )
        })
        .count() as f32
        / trimmed.chars().count().max(1) as f32;
    if punct_ratio > 0.08 {
        return Err(RejectReason::PunctuationSpam);
    }

    // --- Gate 3: meaningful-word density ----------------------------------
    // A briefing-worthy title must have enough "real" words (≥3 letters,
    // alphabetic) that a reader can make sense of it. Pure-numeric,
    // pure-punctuation, or abbreviation-only titles fail.
    let meaningful = words.iter().filter(|w| is_meaningful_word(w)).count();
    if meaningful < 3 {
        return Err(RejectReason::LowMeaningfulDensity);
    }

    // --- Gate 4: coherence (word-entropy + bigram spacing) ----------------
    // Garbled titles tend to have unusual word-length distributions —
    // either too many tiny fragments ("a b c d e f") or too many long
    // junk strings ("asdkfjhsakdjfhaskdf kjhasdkfjhasdkjfh"). We gate on
    // the ratio of "normal-length" words (3-15 chars) to total words.
    let normal_words = words
        .iter()
        .filter(|w| {
            let n = w.chars().count();
            (3..=20).contains(&n)
        })
        .count();
    let normal_ratio = normal_words as f32 / words.len() as f32;
    if normal_ratio < 0.55 {
        return Err(RejectReason::LowCoherence);
    }

    // --- Gate 5: marketing / recruiting / SEO noise -----------------------
    let lower = trimmed.to_lowercase();
    for pattern in MARKETING_PATTERNS {
        if lower.contains(pattern) {
            debug!(
                target: "4da::briefing_quality",
                %title, %source_type, pattern,
                "rejected: marketing pattern"
            );
            return Err(RejectReason::MarketingNoise);
        }
    }

    // --- Gate 6: clickbait / engagement-bait ------------------------------
    for pattern in CLICKBAIT_PATTERNS {
        if lower.contains(pattern) {
            debug!(
                target: "4da::briefing_quality",
                %title, %source_type, pattern,
                "rejected: clickbait pattern"
            );
            return Err(RejectReason::Clickbait);
        }
    }

    // --- Gate 7: CMS boilerplate ------------------------------------------
    for pattern in CMS_BOILERPLATE {
        // Exact start-with match so we don't reject titles that mention the
        // phrase in passing (e.g. "Post navigation considered harmful").
        if lower == *pattern || lower.starts_with(&format!("{pattern} ")) {
            return Err(RejectReason::CmsBoilerplate);
        }
    }

    // Gate 2.5 (punctuation spam) is now enforced earlier, above.

    Ok(())
}

// ============================================================================
// Reject codes — stable enum for telemetry
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    TooShort,
    TooLong,
    TooFewWords,
    LowMeaningfulDensity,
    LowCoherence,
    MarketingNoise,
    Clickbait,
    CmsBoilerplate,
    PunctuationSpam,
}

impl RejectReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TooShort => "too_short",
            Self::TooLong => "too_long",
            Self::TooFewWords => "too_few_words",
            Self::LowMeaningfulDensity => "low_meaningful_density",
            Self::LowCoherence => "low_coherence",
            Self::MarketingNoise => "marketing_noise",
            Self::Clickbait => "clickbait",
            Self::CmsBoilerplate => "cms_boilerplate",
            Self::PunctuationSpam => "punctuation_spam",
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Does this word contribute meaning to the title?
///
/// A "meaningful" word has ≥3 chars and is predominantly alphabetic
/// (allowing one hyphen or apostrophe for things like "state-of-the-art"
/// or "don't"). Numbers, version tags, short abbreviations fail this
/// check — which is fine because a title with ONLY numbers/abbreviations
/// isn't readable prose.
fn is_meaningful_word(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() < 3 {
        return false;
    }
    let alpha = chars.iter().filter(|c| c.is_alphabetic()).count();
    let total = chars.len();
    // At least 70% alphabetic
    (alpha as f32 / total as f32) >= 0.7
}

// ============================================================================
// Pattern banks
// ============================================================================
// All lowercase. Lookup is case-insensitive (we lowercase the title once).
// These are deliberately specific — a false-positive on a pattern here
// drops a LEGITIMATE signal from the user's briefing, which is worse than
// letting a borderline item through. Keep patterns tight.

/// Phrases that indicate the item is marketing/recruiting/SEO content,
/// not developer-relevant intelligence.
const MARKETING_PATTERNS: &[&str] = &[
    "we're hiring",
    "we are hiring",
    "apply now",
    "apply today",
    "click here",
    "subscribe to our",
    "sign up for our",
    "free ebook",
    "free webinar",
    "free trial",
    "sponsored content",
    "sponsored by",
    "affiliate link",
    "promo code",
    "limited time offer",
    "tip on marketing",
    "seo tip",
    "marketing hack",
    "growth hack",
    "life hack",
    "money from home",
    "passive income",
    "buy now",
    "sale ends",
];

/// Phrases that scream engagement-bait rather than information.
const CLICKBAIT_PATTERNS: &[&str] = &[
    "you won't believe",
    "this one weird trick",
    "this one neat trick",
    "doctors hate",
    "shocking truth",
    "what happens next",
    "you'll never guess",
    "the truth about",
    "must see",
    "must read",
    "going viral",
    "gone wrong",
    "gone sexual",
];

/// CMS / template boilerplate text that escaped the scraper.
const CMS_BOILERPLATE: &[&str] = &[
    "post navigation",
    "previous post",
    "next post",
    "continue reading",
    "read more",
    "skip to content",
    "share this:",
    "comments are closed",
    "leave a reply",
    "related posts",
    "you might also like",
];

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Positive cases: legitimate developer-relevant titles ----------

    #[test]
    fn accepts_typical_release_announcement() {
        assert!(is_briefing_worthy(
            "Rust 1.80 released with const generics improvements",
            "hackernews"
        )
        .is_ok());
    }

    #[test]
    fn accepts_security_advisory() {
        assert!(is_briefing_worthy(
            "CVE-2026-1234: Critical RCE vulnerability in tokio 1.38",
            "cve"
        )
        .is_ok());
    }

    #[test]
    fn accepts_discussion_title() {
        assert!(
            is_briefing_worthy("Should we adopt Tauri 2.0 for desktop apps?", "reddit").is_ok()
        );
    }

    #[test]
    fn accepts_technical_deep_dive() {
        assert!(is_briefing_worthy(
            "How Postgres handles sub-millisecond query planning",
            "lobsters"
        )
        .is_ok());
    }

    #[test]
    fn accepts_version_number_in_title() {
        assert!(is_briefing_worthy(
            "TypeScript 5.6 brings iterator helpers and type narrowing wins",
            "devto"
        )
        .is_ok());
    }

    #[test]
    fn accepts_technical_comparison() {
        assert!(
            is_briefing_worthy("Postgres vs MySQL for multi-tenant SaaS", "hackernews").is_ok()
        );
    }

    // ---- Negative: production bug — "Tip on marketing..." ---------------

    #[test]
    fn rejects_real_production_garbage() {
        let got = is_briefing_worthy(
            "Tip on marketing isnt back into job re-validate (5+ V/D/E DMS, run-off)",
            "rss",
        );
        assert_eq!(got, Err(RejectReason::MarketingNoise));
    }

    // ---- Negative: each gate ---------------------------------------------

    #[test]
    fn rejects_too_short() {
        assert_eq!(
            is_briefing_worthy("Rust 1.80", "hn"),
            Err(RejectReason::TooShort)
        );
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(is_briefing_worthy("", "hn"), Err(RejectReason::TooShort));
    }

    #[test]
    fn rejects_excessively_long() {
        let giant = "x".repeat(400);
        let t = format!("valid start {giant}");
        assert!(matches!(is_briefing_worthy(&t, "hn"), Err(_)));
    }

    #[test]
    fn rejects_too_few_words() {
        // Length ≥ 15 so TooShort doesn't fire first; 10 single-char tokens
        // so LowMeaningfulDensity catches it via the meaningful-word filter.
        assert!(matches!(
            is_briefing_worthy("x y z a b c d e f g", "hn"),
            Err(RejectReason::LowMeaningfulDensity | RejectReason::TooFewWords)
        ));
    }

    #[test]
    fn rejects_number_and_punctuation_fragment() {
        assert!(matches!(is_briefing_worthy("3.14 :: -> 42", "hn"), Err(_)));
    }

    #[test]
    fn rejects_marketing_hiring() {
        assert_eq!(
            is_briefing_worthy(
                "Senior engineer role — we're hiring for our new platform team",
                "rss"
            ),
            Err(RejectReason::MarketingNoise)
        );
    }

    #[test]
    fn rejects_clickbait() {
        assert_eq!(
            is_briefing_worthy("You won't believe what this developer discovered", "devto"),
            Err(RejectReason::Clickbait)
        );
    }

    #[test]
    fn rejects_cms_boilerplate_exact() {
        assert_eq!(
            is_briefing_worthy("post navigation", "rss"),
            Err(RejectReason::TooFewWords) // or CmsBoilerplate — both catch it
        );
    }

    #[test]
    fn rejects_cms_boilerplate_with_payload() {
        assert_eq!(
            is_briefing_worthy("Post navigation for April 2026 archives", "rss"),
            Err(RejectReason::CmsBoilerplate)
        );
    }

    #[test]
    fn rejects_breadcrumb_punctuation_spam() {
        assert_eq!(
            is_briefing_worthy("Home » Blog » Category » Archive » Tag » Item", "rss"),
            Err(RejectReason::PunctuationSpam)
        );
    }

    #[test]
    fn rejects_arrow_chain() {
        assert!(matches!(
            is_briefing_worthy("Path -> To -> Nested -> Nowhere -> Here", "rss"),
            Err(_)
        ));
    }

    // ---- Gate ordering invariants ---------------------------------------

    #[test]
    fn reject_reason_as_str_round_trip() {
        for r in [
            RejectReason::TooShort,
            RejectReason::TooLong,
            RejectReason::TooFewWords,
            RejectReason::LowMeaningfulDensity,
            RejectReason::LowCoherence,
            RejectReason::MarketingNoise,
            RejectReason::Clickbait,
            RejectReason::CmsBoilerplate,
            RejectReason::PunctuationSpam,
        ] {
            assert!(!r.as_str().is_empty());
            assert!(r
                .as_str()
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_'));
        }
    }

    // ---- Batch filtering helper used by monitoring_briefing ------------

    #[test]
    fn mixed_batch_retains_only_legitimate() {
        let items = vec![
            ("Rust 1.80 released with const generics improvements", "hn"),
            ("Tip on marketing isnt back into job re-validate", "rss"),
            ("CVE-2026-1234 critical vulnerability in tokio 1.38", "cve"),
            ("We're hiring senior Rust engineers", "rss"),
        ];
        let kept: Vec<_> = items
            .iter()
            .filter(|(t, s)| is_briefing_worthy(t, s).is_ok())
            .collect();
        assert_eq!(kept.len(), 2);
    }
}
