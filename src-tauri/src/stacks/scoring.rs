//! Stack-aware scoring functions that plug into the PASIFA pipeline.
//!
//! Three injection points:
//! 1. `compute_stack_boost` — pain point keyword matching (adds to base_score)
//! 2. `detect_ecosystem_shift` — ecosystem shift detection (multiplier)
//! 3. `has_pain_point_match` — gate confirmation for ACE axis

use super::ComposedStack;

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
    let combined = format!("{} {}", title_lower, content_lower);

    let mut best_boost: f32 = 0.0;

    for pp in &stack.pain_points {
        let match_count = pp
            .keywords
            .iter()
            .filter(|kw| combined.contains(**kw))
            .count();

        // Require 2+ keyword matches for a valid pain point hit
        if match_count >= 2 {
            best_boost = best_boost.max(pp.severity);
        }
    }

    // Also apply keyword boosts (simpler: any match triggers)
    for (&kw, &boost) in &stack.keyword_boosts {
        if title_lower.contains(kw) {
            // Title match gets full boost
            best_boost = best_boost.max(boost);
        } else if content_lower.contains(kw) {
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
/// gets a boost. Returns 1.0 (neutral) when no stacks are active.
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
                title_lower.contains(**kw)
                    || topics
                        .iter()
                        .any(|t| t.contains(**kw) || kw.contains(t.as_str()))
            })
            .count();

        // Need at least 1 keyword match to trigger
        if keyword_matches >= 1 {
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
    let combined = format!("{} {}", title_lower, content_lower);

    stack.pain_points.iter().any(|pp| {
        let match_count = pp
            .keywords
            .iter()
            .filter(|kw| combined.contains(**kw))
            .count();
        match_count >= 2
    })
}

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
    // compute_stack_boost
    // ========================================================================

    #[test]
    fn test_no_stack_returns_zero() {
        let boost = compute_stack_boost("Rust async guide", "async tokio", &empty_stack());
        assert_eq!(boost, 0.0);
    }

    #[test]
    fn test_single_keyword_no_boost() {
        let stack = rust_stack();
        // Only "async" alone — needs 2+ keywords to trigger pain point
        let boost = compute_stack_boost("Async programming", "async patterns", &stack);
        // Should still get keyword_boosts for "async rust" if matched, but
        // pain point requires 2 keywords like "async" + "lifetime"
        assert!(boost <= 0.12); // keyword boost at most
    }

    #[test]
    fn test_pain_point_two_keywords_triggers() {
        let stack = rust_stack();
        let boost = compute_stack_boost(
            "Rust async lifetime challenges",
            "Understanding pin and send in async contexts with lifetime annotations",
            &stack,
        );
        // Should match the async pain point (async + lifetime + send + pin = 4 matches)
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

    // ========================================================================
    // detect_ecosystem_shift
    // ========================================================================

    #[test]
    fn test_no_stack_returns_neutral() {
        let mult = detect_ecosystem_shift(
            &["drizzle".to_string()],
            "Drizzle ORM is replacing Prisma",
            &empty_stack(),
        );
        assert_eq!(mult, 1.0);
    }

    #[test]
    fn test_ecosystem_shift_detected() {
        let stack = nextjs_stack();
        let mult = detect_ecosystem_shift(
            &["drizzle".to_string(), "orm".to_string()],
            "Why developers are switching from Prisma to Drizzle-ORM",
            &stack,
        );
        assert!(
            mult > 1.0,
            "Ecosystem shift should boost > 1.0, got {}",
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
        // Only "async" alone — not enough for pain point
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
}
