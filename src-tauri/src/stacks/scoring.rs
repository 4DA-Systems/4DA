// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Stack-aware scoring functions that plug into the PASIFA pipeline.
//!
//! Four injection points:
//! 1. `compute_stack_boost` — pain point keyword matching (adds to base_score)
//! 2. `detect_ecosystem_shift` — ecosystem shift detection (multiplier)
//! 3. `has_pain_point_match` — gate confirmation for ACE axis
//! 4. `compute_competing_penalty` — suppresses competing-only content (multiplier)

use super::ComposedStack;

// ============================================================================
// Word-boundary matching
// ============================================================================

/// Check if `text` contains `term` at word boundaries.
///
/// Prevents false positives like "go" matching "google" or "rust" matching
/// "trust". Multi-word terms like "app router" are matched as a phrase
/// with word boundaries at the start and end only.
///
/// Both `text` and `term` must already be lowercase.
pub fn text_contains_term(text: &str, term: &str) -> bool {
    /// Treat alphanumeric, hyphens, and underscores as word characters.
    /// This prevents "react" from matching inside "react-native" and
    /// "go" from matching inside "go-fiber".
    #[inline]
    fn is_word_char(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
    }

    let text_bytes = text.as_bytes();
    let term_len = term.len();

    if term_len == 0 || term_len > text_bytes.len() {
        return false;
    }

    let mut start = 0;
    while start + term_len <= text_bytes.len() {
        if let Some(pos) = text[start..].find(term) {
            let abs_pos = start + pos;
            let end_pos = abs_pos + term_len;

            let left_ok = abs_pos == 0 || !is_word_char(text_bytes[abs_pos - 1]);
            let right_ok = end_pos == text_bytes.len() || !is_word_char(text_bytes[end_pos]);

            if left_ok && right_ok {
                return true;
            }

            start = abs_pos + 1;
        } else {
            break;
        }
    }

    false
}

// ============================================================================
// Scoring functions
// ============================================================================

/// Compute a stack-based score boost from pain point keyword matching.
///
/// Requires 2+ keywords from the same PainPoint to match (prevents false
/// positives like "async" alone triggering the Rust async pain point).
///
/// Returns 0.0 when no stacks are active, 0.0-0.20 otherwise.
pub fn compute_stack_boost(title: &str, content: &str, stack: &ComposedStack) -> f32 {
    if !stack.active {
        return 0.0;
    }

    let title_lower = title.to_lowercase();
    let content_lower = content.to_lowercase();
    let combined = format!("{title_lower} {content_lower}");

    let mut best_boost: f32 = 0.0;

    for pp in &stack.pain_points {
        let match_count = pp
            .keywords
            .iter()
            .filter(|kw| text_contains_term(&combined, kw))
            .count();

        // Require 2+ keyword matches for a valid pain point hit
        if match_count >= 2 {
            best_boost = best_boost.max(pp.severity);
        }
    }

    // Also apply keyword boosts (simpler: any match triggers)
    for (&kw, &boost) in &stack.keyword_boosts {
        if text_contains_term(&title_lower, kw) {
            // Title match gets full boost
            best_boost = best_boost.max(boost);
        } else if text_contains_term(&content_lower, kw) {
            // Content-only match gets half boost
            best_boost = best_boost.max(boost * 0.5);
        }
    }

    best_boost.min(0.20)
}

/// Detect ecosystem shifts in content. Returns a multiplier (0.95-1.25).
///
/// When the content discusses a shift that matters to the user's stack
/// (e.g., "Drizzle replacing Prisma" for a Next.js user), the content
/// gets a boost. Requires 2+ keyword matches to prevent single-word false
/// triggers. Returns 1.0 (neutral) when no stacks are active.
pub fn detect_ecosystem_shift(topics: &[String], title: &str, stack: &ComposedStack) -> f32 {
    if !stack.active {
        return 1.0;
    }

    let title_lower = title.to_lowercase();
    let mut best_mult: f32 = 1.0;

    for es in &stack.ecosystem_shifts {
        let keyword_matches = es
            .keywords
            .iter()
            .filter(|kw| {
                text_contains_term(&title_lower, kw)
                    || topics.iter().any(|t| {
                        let t_lower = t.to_lowercase();
                        text_contains_term(&t_lower, kw) || text_contains_term(kw, &t_lower)
                    })
            })
            .count();

        // Require 2+ keyword matches (same threshold as pain points)
        if keyword_matches >= 2 {
            best_mult = best_mult.max(es.boost);
        }
    }

    best_mult
}

/// Check if content matches any pain point (for ACE axis confirmation in gate).
///
/// Returns false when no stacks are active. Uses the same 2-keyword threshold
/// as `compute_stack_boost` to prevent false positive gate confirmations.
pub fn has_pain_point_match(title: &str, content: &str, stack: &ComposedStack) -> bool {
    if !stack.active {
        return false;
    }

    let title_lower = title.to_lowercase();
    let content_lower = content.to_lowercase();
    let combined = format!("{title_lower} {content_lower}");

    stack.pain_points.iter().any(|pp| {
        let match_count = pp
            .keywords
            .iter()
            .filter(|kw| text_contains_term(&combined, kw))
            .count();
        match_count >= 2
    })
}

/// Compute a penalty multiplier for competing-tech-only content.
///
/// When content mentions competing technologies (e.g., "Go" for a Rust user)
/// WITHOUT also mentioning core/companion tech, apply a small suppression.
/// Returns 1.0 (neutral) when no stacks active, or when the content also
/// references the user's tech. Returns 0.95 when content is competing-only.
pub fn compute_competing_penalty(title: &str, content: &str, stack: &ComposedStack) -> f32 {
    if !stack.active {
        return 1.0;
    }

    let title_lower = title.to_lowercase();
    let content_lower = content.to_lowercase();

    // Check if any competing tech appears in title
    let has_competing = stack
        .competing
        .iter()
        .any(|tech| text_contains_term(&title_lower, tech));

    if !has_competing {
        return 1.0;
    }

    // Check if any of the user's own tech also appears (title + content)
    let combined = format!("{title_lower} {content_lower}");
    let has_own_tech = stack
        .all_tech
        .iter()
        .any(|tech| text_contains_term(&combined, tech));

    if has_own_tech {
        // Mixed content (e.g., "Rust vs Go performance") — no penalty
        1.0
    } else {
        // Competing-only content — mild suppression
        0.95
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stacks::compose_profiles;

    fn rust_stack() -> ComposedStack {
        compose_profiles(&["rust_systems".to_string()])
    }

    fn nextjs_stack() -> ComposedStack {
        compose_profiles(&["nextjs_fullstack".to_string()])
    }

    fn empty_stack() -> ComposedStack {
        ComposedStack::default()
    }

    // ========================================================================
    // text_contains_term — word boundary matching
    // ========================================================================

    #[test]
    fn test_word_boundary_exact_match() {
        assert!(text_contains_term("learn rust today", "rust"));
        assert!(text_contains_term("rust is great", "rust"));
        assert!(text_contains_term("i love rust", "rust"));
    }

    #[test]
    fn test_word_boundary_rejects_substring() {
        assert!(!text_contains_term("frustrated with bugs", "rust"));
        assert!(!text_contains_term("trust issues", "rust"));
        assert!(!text_contains_term("google is hiring", "go"));
        assert!(!text_contains_term("algorithm design", "go"));
        assert!(!text_contains_term("mongodb performance", "go"));
        assert!(!text_contains_term("ergonomic keyboards", "go"));
        assert!(!text_contains_term("reactive programming", "react"));
    }

    #[test]
    fn test_word_boundary_with_punctuation() {
        assert!(text_contains_term("learn rust, the language", "rust"));
        assert!(text_contains_term("what is rust?", "rust"));
        assert!(text_contains_term("rust: a systems language", "rust"));
        assert!(text_contains_term("(rust) programming", "rust"));
    }

    #[test]
    fn test_word_boundary_multi_word_phrase() {
        assert!(text_contains_term(
            "the app router is complex",
            "app router"
        ));
        assert!(text_contains_term(
            "use the borrow checker",
            "borrow checker"
        ));
        assert!(!text_contains_term("happy routering", "app router"));
    }

    #[test]
    fn test_word_boundary_hyphen_underscore() {
        // Hyphens and underscores are NOT word boundaries (they're part of tech names)
        assert!(!text_contains_term("react-native is great", "react"));
        assert!(text_contains_term("react-native is great", "react-native"));
        assert!(text_contains_term("use drizzle-orm today", "drizzle-orm"));
    }

    #[test]
    fn test_word_boundary_at_edges() {
        assert!(text_contains_term("rust", "rust"));
        assert!(text_contains_term("go", "go"));
        assert!(!text_contains_term("", "rust"));
        assert!(!text_contains_term("rust", ""));
    }

    // ========================================================================
    // compute_stack_boost
    // ========================================================================

    #[test]
    fn test_no_stack_returns_zero() {
        let boost = compute_stack_boost("Rust async guide", "async tokio", &empty_stack());
        assert_eq!(boost, 0.0);
    }

    #[test]
    fn test_single_keyword_no_pain_point() {
        let stack = rust_stack();
        // Only "async" alone — needs 2+ keywords to trigger pain point
        let boost = compute_stack_boost("Async programming", "async patterns", &stack);
        // Pain point won't trigger, but keyword_boost for individual terms may
        assert!(boost <= 0.12);
    }

    #[test]
    fn test_pain_point_two_keywords_triggers() {
        let stack = rust_stack();
        let boost = compute_stack_boost(
            "Rust async lifetime challenges",
            "Understanding pin and send in async contexts with lifetime annotations",
            &stack,
        );
        assert!(
            boost >= 0.10,
            "Pain point with 2+ keywords should boost >= 0.10, got {}",
            boost
        );
    }

    #[test]
    fn test_keyword_boost_title_match() {
        let stack = rust_stack();
        let boost =
            compute_stack_boost("New tokio release features", "runtime improvements", &stack);
        assert!(
            boost >= 0.08,
            "Title keyword 'tokio' should boost >= 0.08, got {}",
            boost
        );
    }

    #[test]
    fn test_boost_capped_at_020() {
        let stack = rust_stack();
        let boost = compute_stack_boost(
            "Rust async pin send lifetime borrow checker cargo tokio",
            "async tokio pin send lifetime error handling thiserror anyhow result",
            &stack,
        );
        assert!(boost <= 0.20, "Boost should cap at 0.20, got {}", boost);
    }

    #[test]
    fn test_no_false_positive_go_in_google() {
        let stack = compose_profiles(&["go_backend".to_string()]);
        let boost = compute_stack_boost(
            "Google Cloud Platform Updates",
            "google cloud storage algorithms and ergonomic apis",
            &stack,
        );
        // "go" should NOT match "google", "algorithms", "ergonomic"
        assert_eq!(
            boost, 0.0,
            "Should not boost Google content for Go stack, got {}",
            boost
        );
    }

    #[test]
    fn test_legitimate_go_match() {
        let stack = compose_profiles(&["go_backend".to_string()]);
        let boost = compute_stack_boost(
            "Go 1.23 Generics Improvements",
            "golang generics type parameter improvements",
            &stack,
        );
        assert!(
            boost > 0.0,
            "Should boost legitimate Go content, got {}",
            boost
        );
    }

    // ========================================================================
    // detect_ecosystem_shift
    // ========================================================================

    #[test]
    fn test_no_stack_returns_neutral() {
        let mult = detect_ecosystem_shift(
            &["biome".to_string()],
            "Biome is replacing ESLint",
            &empty_stack(),
        );
        assert_eq!(mult, 1.0);
    }

    #[test]
    fn test_ecosystem_shift_detected_with_two_keywords() {
        let stack = nextjs_stack();
        // Title has "biome" and "eslint alternative" — 2 keyword matches
        let mult = detect_ecosystem_shift(
            &["biome".to_string(), "linter".to_string()],
            "Why developers switching to Biome as an ESLint alternative",
            &stack,
        );
        assert!(
            mult > 1.0,
            "Ecosystem shift with 2+ keywords should boost > 1.0, got {}",
            mult
        );
    }

    #[test]
    fn test_ecosystem_shift_single_keyword_no_trigger() {
        let stack = nextjs_stack();
        // Only "biome" matches — below 2-keyword threshold
        let mult = detect_ecosystem_shift(&[], "Biome is a formatter", &stack);
        assert_eq!(
            mult, 1.0,
            "Single keyword should not trigger shift, got {}",
            mult
        );
    }

    #[test]
    fn test_no_shift_returns_neutral() {
        let stack = nextjs_stack();
        let mult =
            detect_ecosystem_shift(&["css".to_string()], "New CSS features in browsers", &stack);
        assert_eq!(mult, 1.0);
    }

    // ========================================================================
    // has_pain_point_match
    // ========================================================================

    #[test]
    fn test_no_stack_no_match() {
        assert!(!has_pain_point_match(
            "Rust async",
            "async lifetime",
            &empty_stack()
        ));
    }

    #[test]
    fn test_pain_point_match_true() {
        let stack = rust_stack();
        assert!(has_pain_point_match(
            "Understanding async lifetimes in Rust",
            "The async and lifetime interaction is tricky with pin",
            &stack,
        ));
    }

    #[test]
    fn test_pain_point_single_keyword_no_match() {
        let stack = rust_stack();
        assert!(!has_pain_point_match(
            "Async programming basics",
            "Introduction to concurrent programming",
            &stack,
        ));
    }

    #[test]
    fn test_nextjs_server_component_pain_point() {
        let stack = nextjs_stack();
        assert!(has_pain_point_match(
            "Server Components vs Client Components in Next.js",
            "When to use server component and when to add use client directive",
            &stack,
        ));
    }

    #[test]
    fn test_no_false_positive_pain_point_rust_in_trust() {
        let stack = rust_stack();
        // "trust" and "frustrated" should NOT match "rust" keyword boosts
        assert!(!has_pain_point_match(
            "Building Trust in Software Teams",
            "Frustrated developers need better frustrated onboarding trust",
            &stack,
        ));
    }

    // ========================================================================
    // compute_competing_penalty
    // ========================================================================

    #[test]
    fn test_competing_penalty_no_stack() {
        assert_eq!(
            compute_competing_penalty("Go 1.23 features", "golang improvements", &empty_stack()),
            1.0
        );
    }

    #[test]
    fn test_competing_penalty_competing_only() {
        let stack = rust_stack();
        // Go is competing tech for Rust, and no Rust tech is mentioned
        let penalty = compute_competing_penalty(
            "Go Backend Performance Tips",
            "golang goroutine optimization patterns",
            &stack,
        );
        assert_eq!(penalty, 0.95, "Competing-only should get 0.95 penalty");
    }

    #[test]
    fn test_competing_penalty_mixed_content() {
        let stack = rust_stack();
        // Both "go" and "rust" mentioned — comparison article, no penalty
        let penalty = compute_competing_penalty(
            "Rust vs Go Performance Comparison",
            "comparing rust and go for backend services",
            &stack,
        );
        assert_eq!(penalty, 1.0, "Mixed content should have no penalty");
    }

    #[test]
    fn test_competing_penalty_own_tech_only() {
        let stack = rust_stack();
        let penalty = compute_competing_penalty(
            "Tokio Runtime Deep Dive",
            "async rust tokio executor internals",
            &stack,
        );
        assert_eq!(penalty, 1.0, "Own tech should have no penalty");
    }

    #[test]
    fn test_competing_penalty_unrelated_content() {
        let stack = rust_stack();
        let penalty = compute_competing_penalty(
            "CSS Grid Layout Tutorial",
            "css grid flexbox responsive design",
            &stack,
        );
        assert_eq!(penalty, 1.0, "Unrelated content should have no penalty");
    }

    #[test]
    fn test_competing_no_false_positive_go_in_google() {
        let stack = rust_stack();
        // "go" is competing for rust, but "google" should NOT match "go"
        let penalty = compute_competing_penalty(
            "Google Cloud Announces New Features",
            "google cloud platform updates and algorithms",
            &stack,
        );
        assert_eq!(
            penalty, 1.0,
            "Google should not trigger Go competing penalty"
        );
    }
}
