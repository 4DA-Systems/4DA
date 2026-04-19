// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::ace_context::ACEContext;
use super::utils::topic_overlaps;
use crate::scoring_config;
use fourda_macros::confirmation_gate;

/// Result of counting how many independent signal axes confirm relevance
#[confirmation_gate(axes = ["context", "interest", "ace", "learned", "dependency"])]
pub(crate) struct SignalConfirmation {
    pub context_confirmed: bool,
    pub interest_confirmed: bool,
    pub ace_confirmed: bool,
    pub learned_confirmed: bool,
    pub dependency_confirmed: bool,
    pub count: u8,
}

impl SignalConfirmation {
    pub fn confirmed_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        if self.context_confirmed {
            names.push("context".to_string());
        }
        if self.interest_confirmed {
            names.push("interest".to_string());
        }
        if self.ace_confirmed {
            names.push("ace".to_string());
        }
        if self.learned_confirmed {
            names.push("learned".to_string());
        }
        if self.dependency_confirmed {
            names.push("dependency".to_string());
        }
        names
    }
}

/// Count how many independent signal axes confirm this item is relevant.
/// Each axis answers a different question:
/// - Context: Does this match code you're actually writing? (KNN embedding similarity)
/// - Interest: Does this match your declared interests? (interest embedding + keyword)
/// - ACE/Tech: Does this involve your tech stack or active topics? (semantic boost + tech detection)
/// - Learned: Has user behavior confirmed this kind of content? (feedback + affinity)
/// - Dependency: Does this mention packages from your installed dependencies?
#[allow(clippy::too_many_arguments)]
pub(crate) fn count_confirmed_signals(
    context_score: f32,
    interest_score: f32,
    keyword_score: f32,
    semantic_boost: f32,
    ace_ctx: &ACEContext,
    topics: &[String],
    feedback_boost: f32,
    affinity_mult: f32,
    dep_match_score: f32,
    stack_pain_match: bool,
    best_keyword_specificity: f32,
) -> SignalConfirmation {
    let context_confirmed = context_score >= scoring_config::CONTEXT_THRESHOLD;
    // Broad interests (specificity < 0.50, e.g. "Open Source") cannot confirm the interest
    // axis from keyword matching alone — they need corroboration from embedding similarity.
    let interest_confirmed = if best_keyword_specificity < 0.50 {
        // Broad interest: require BOTH keyword AND embedding, OR very strong embedding alone
        // (>= INTEREST_THRESHOLD of 0.50 indicates high semantic match even without keywords)
        (keyword_score >= scoring_config::KEYWORD_THRESHOLD && interest_score >= 0.35)
            || interest_score >= scoring_config::INTEREST_THRESHOLD
    } else {
        interest_score >= scoring_config::INTEREST_THRESHOLD
            || keyword_score >= scoring_config::KEYWORD_THRESHOLD
    };
    // ACE confirmed: require semantic boost OR active topic match (NOT broad detected_tech).
    // Uses word-boundary-aware matching to prevent "frustrating"->"rust" false positives.
    // Stack pain point match also contributes to ACE axis (content about your stack's problems).
    let ace_confirmed = semantic_boost >= scoring_config::SEMANTIC_THRESHOLD
        || topics
            .iter()
            .any(|t| ace_ctx.active_topics.iter().any(|at| topic_overlaps(t, at)))
        || stack_pain_match;
    let learned_confirmed = feedback_boost > scoring_config::FEEDBACK_THRESHOLD
        || affinity_mult >= scoring_config::AFFINITY_THRESHOLD;
    let dependency_confirmed = dep_match_score >= scoring_config::DEPENDENCY_THRESHOLD;

    // Deduplicate interest + ACE: these axes often fire on the same technology
    // (e.g. user has interest "React" AND React is in ACE active_topics/detected_tech).
    // When both fire, count them as a single signal to prevent one technology from
    // masquerading as two independent signals. If ACE fires via a genuinely independent
    // path (semantic_boost above threshold or stack_pain_match), count it separately.
    let ace_independent =
        ace_confirmed && (semantic_boost >= scoring_config::SEMANTIC_THRESHOLD || stack_pain_match);
    let deduped_ace = if interest_confirmed && ace_confirmed {
        ace_independent // only count ACE separately if it has an independent signal path
    } else {
        ace_confirmed // no overlap possible, count normally
    };

    let count = [
        context_confirmed,
        interest_confirmed,
        deduped_ace,
        learned_confirmed,
        dependency_confirmed,
    ]
    .iter()
    .filter(|&&x| x)
    .count() as u8;

    SignalConfirmation {
        context_confirmed,
        interest_confirmed,
        ace_confirmed,
        learned_confirmed,
        dependency_confirmed,
        count,
    }
}

/// Apply the multi-signal confirmation gate to a base score.
/// Returns (gated_score, confirmation_count, confirmation_multiplier, confirmed_signal_names).
///
/// Key property: with only 1 confirmed signal, score is capped at 0.28 — well below the
/// 0.35 relevance threshold. This means a single signal (no matter how strong) can
/// NEVER make an item relevant on its own.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_confirmation_gate(
    base_score: f32,
    context_score: f32,
    interest_score: f32,
    keyword_score: f32,
    semantic_boost: f32,
    ace_ctx: &ACEContext,
    topics: &[String],
    feedback_boost: f32,
    affinity_mult: f32,
    dep_match_score: f32,
    stack_pain_match: bool,
    best_keyword_specificity: f32,
) -> (f32, u8, f32, Vec<String>) {
    let confirmation = count_confirmed_signals(
        context_score,
        interest_score,
        keyword_score,
        semantic_boost,
        ace_ctx,
        topics,
        feedback_boost,
        affinity_mult,
        dep_match_score,
        stack_pain_match,
        best_keyword_specificity,
    );

    let idx = (confirmation.count as usize).min(scoring_config::CONFIRMATION_GATE.len() - 1);
    let (conf_mult, score_ceiling) = scoring_config::CONFIRMATION_GATE[idx];

    let gated = (base_score * conf_mult).min(score_ceiling);
    let names = confirmation.confirmed_names();

    (gated, confirmation.count, conf_mult, names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirmation_count_no_signals() {
        let ace_ctx = ACEContext::default();
        let topics = vec!["test".to_string()];
        let conf = count_confirmed_signals(
            0.10, // low context
            0.10, // low interest
            0.10, // low keyword
            0.01, // low semantic
            &ace_ctx, &topics, 0.0,   // no feedback
            1.0,   // neutral affinity
            0.0,   // no dep match
            false, // no stack pain match
            1.0,   // specific interest
        );
        assert_eq!(conf.count, 0);
        assert!(!conf.context_confirmed);
        assert!(!conf.interest_confirmed);
        assert!(!conf.ace_confirmed);
        assert!(!conf.learned_confirmed);
        assert!(!conf.dependency_confirmed);
    }

    #[test]
    fn test_confirmation_count_one_signal_interest() {
        let ace_ctx = ACEContext::default();
        let topics = vec!["test".to_string()];
        let conf = count_confirmed_signals(
            0.10, // low context
            0.60, // HIGH interest
            0.10, // low keyword
            0.01, // low semantic
            &ace_ctx, &topics, 0.0,   // no feedback
            1.0,   // neutral affinity
            0.0,   // no dep match
            false, // no stack pain match
            1.0,   // specific interest
        );
        assert_eq!(conf.count, 1);
        assert!(!conf.context_confirmed);
        assert!(conf.interest_confirmed);
    }

    #[test]
    fn test_confirmation_count_two_signals() {
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("rust".to_string());
        let topics = vec!["rust".to_string()];
        let conf = count_confirmed_signals(
            0.50, // HIGH context
            0.10, // low interest
            0.10, // low keyword
            0.01, // low semantic, but ace_confirmed via active_topics
            &ace_ctx, &topics, 0.0,   // no feedback
            1.0,   // neutral affinity
            0.0,   // no dep match
            false, // no stack pain match
            1.0,   // specific interest
        );
        assert_eq!(conf.count, 2);
        assert!(conf.context_confirmed);
        assert!(conf.ace_confirmed);
    }

    #[test]
    fn test_single_signal_cannot_pass_threshold() {
        // The key property: with only 1 confirmed signal, ceiling is 0.45 < 0.50 threshold
        // The quality floor always requires 2+ signals (no bootstrap bypass).
        // This gate test validates the raw confirmation gate behavior.
        let ace_ctx = ACEContext::default();
        let topics = vec!["test".to_string()];

        // Even with high base_score (0.90), single signal caps below threshold
        let (gated, count, _, _) = apply_confirmation_gate(
            0.90, // Very high base
            0.10, // low context
            0.60, // HIGH interest (1 signal)
            0.10, // low keyword
            0.01, // low semantic
            &ace_ctx, &topics, 0.0,   // no feedback
            1.0,   // neutral affinity
            0.0,   // no dep match
            false, // no stack pain match
            1.0,   // specific interest
        );
        assert_eq!(count, 1);
        assert!(
            gated <= 0.28,
            "Single signal should cap at 0.28, got {}",
            gated
        );
        assert!(
            gated < 0.35,
            "Single signal score must be below 0.35 threshold"
        );
    }

    #[test]
    fn test_two_signals_can_pass_threshold() {
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("rust".to_string());
        let topics = vec!["rust".to_string()];

        let (gated, count, _, names) = apply_confirmation_gate(
            0.70, // Good base score
            0.50, // HIGH context
            0.55, // HIGH interest
            0.10, 0.01, // low semantic, but ace_confirmed via detected_tech
            &ace_ctx, &topics, 0.0, 1.0, 0.0, false, // no stack pain match
            1.0,   // specific interest
        );
        assert!(count >= 2, "Expected 2+ confirmed signals, got {}", count);
        assert!(
            gated >= 0.50,
            "Two signals should allow passing threshold, got {}",
            gated
        );
        assert!(!names.is_empty());
    }

    #[test]
    fn test_four_signals_boost() {
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("rust".to_string());
        ace_ctx
            .topic_affinities
            .insert("rust".to_string(), (0.8, 0.9));
        let topics = vec!["rust".to_string()];

        let (gated, count, mult, _) = apply_confirmation_gate(
            0.70, 0.50, // context confirmed
            0.55, // interest confirmed
            0.10,
            0.20, // ace confirmed via semantic boost (above 0.18 threshold = independent signal)
            &ace_ctx, &topics, 0.10,  // feedback confirmed
            1.20,  // affinity confirmed
            0.0,   // no dep match
            false, // no stack pain match
            1.0,   // specific interest
        );
        assert_eq!(count, 4);
        assert_eq!(mult, 1.20);
        assert!(
            gated > 0.70,
            "4 signals should boost above base, got {}",
            gated
        );
    }

    #[test]
    fn test_zero_signals_heavy_penalty() {
        let ace_ctx = ACEContext::default();
        let topics = vec!["test".to_string()];

        let (gated, count, _, _) = apply_confirmation_gate(
            0.60, 0.10, // low context
            0.10, // low interest
            0.10, 0.01, // low semantic
            &ace_ctx, &topics, 0.0, 1.0, 0.0, false, // no stack pain match
            1.0,   // specific interest
        );
        assert_eq!(count, 0);
        assert!(
            gated <= 0.20,
            "Zero signals should cap at 0.20, got {}",
            gated
        );
    }

    #[test]
    fn test_confirmed_signal_names() {
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("rust".to_string());
        let topics = vec!["rust".to_string()];

        let conf = count_confirmed_signals(
            0.50, // context confirmed
            0.10, // interest NOT confirmed
            0.10, 0.01, // ace confirmed via tech
            &ace_ctx, &topics, 0.0, 1.0, 0.0, false, // no stack pain match
            1.0,   // specific interest
        );
        let names = conf.confirmed_names();
        assert!(names.contains(&"context".to_string()));
        assert!(names.contains(&"ace".to_string()));
        assert!(!names.contains(&"interest".to_string()));
        assert!(!names.contains(&"learned".to_string()));
    }

    #[test]
    fn test_5th_axis_gate_dep_plus_interest() {
        // Dependency + interest = 2 signals = passes the 2-signal gate
        let ace_ctx = ACEContext::default();
        let topics = vec!["tokio".to_string()];

        let conf = count_confirmed_signals(
            0.10, // context: NOT confirmed (below threshold)
            0.50, // interest: confirmed (above 0.25 threshold)
            0.10, // keyword: below threshold
            0.01, // semantic: below threshold
            &ace_ctx, &topics, 0.0,   // feedback: none
            1.0,   // affinity: neutral
            0.30,  // dep_match_score: confirmed (above 0.20 threshold)
            false, // no stack pain match
            1.0,   // specific interest
        );

        assert!(conf.interest_confirmed, "Interest should be confirmed");
        assert!(conf.dependency_confirmed, "Dependency should be confirmed");
        assert_eq!(conf.count, 2, "Should have 2 confirmed signals");

        // With 2 signals, the gate multiplier should be >= 1.0 (passes)
        let gate_mult = scoring_config::CONFIRMATION_GATE[conf.count as usize].0;
        assert!(
            gate_mult >= 1.0,
            "2 signals should pass the gate (mult={})",
            gate_mult
        );
    }

    #[test]
    fn test_5th_axis_gate_dep_alone_fails() {
        // Dependency alone = 1 signal = does NOT pass (capped at 0.45)
        let ace_ctx = ACEContext::default();
        let topics: Vec<String> = vec![];

        let conf = count_confirmed_signals(
            0.10, // context: NOT confirmed
            0.10, // interest: NOT confirmed
            0.10, // keyword: below threshold
            0.01, // semantic: below threshold
            &ace_ctx, &topics, 0.0,   // feedback: none
            1.0,   // affinity: neutral
            0.30,  // dep_match_score: confirmed
            false, // no stack pain match
            1.0,   // specific interest
        );

        assert!(conf.dependency_confirmed, "Dependency should be confirmed");
        assert_eq!(conf.count, 1, "Should have only 1 confirmed signal");

        // With 1 signal, the gate cap should be below 0.50 (relevance threshold)
        let gate_cap = scoring_config::CONFIRMATION_GATE[conf.count as usize].1;
        assert!(
            gate_cap < 0.50,
            "1 signal gate cap ({}) should be below 0.50 relevance threshold",
            gate_cap
        );
    }

    // ========================================================================
    // stack_pain_match integration tests
    // ========================================================================

    #[test]
    fn test_stack_pain_match_confirms_ace_axis() {
        // stack_pain_match: true should confirm ACE axis even when no other ACE signal fires.
        // Removing `|| stack_pain_match` from line 69 must make this fail.
        let ace_ctx = ACEContext::default(); // no active_topics
        let topics = vec!["borrow".to_string()];

        let with_pain = count_confirmed_signals(
            0.10, 0.10, 0.10, 0.01, // all below thresholds
            &ace_ctx, &topics, 0.0, 1.0, 0.0, true, // stack_pain_match
            1.0,  // specific interest
        );
        assert!(
            with_pain.ace_confirmed,
            "stack_pain_match=true should confirm ACE axis"
        );
        assert_eq!(with_pain.count, 1, "Only ACE axis should be confirmed");

        let without_pain = count_confirmed_signals(
            0.10, 0.10, 0.10, 0.01, &ace_ctx, &topics, 0.0, 1.0, 0.0,
            false, // no stack_pain_match
            1.0,   // specific interest
        );
        assert!(
            !without_pain.ace_confirmed,
            "Without stack_pain_match, ACE should NOT be confirmed"
        );
        assert_eq!(without_pain.count, 0);
    }

    #[test]
    fn test_stack_pain_match_plus_interest_passes_gate() {
        // Interest confirmed + ACE confirmed via pain match = 2 signals = passes gate
        let ace_ctx = ACEContext::default();
        let topics = vec!["borrow".to_string()];

        let (gated, count, _, _) = apply_confirmation_gate(
            0.70, // good base score
            0.10, // low context
            0.60, // HIGH interest (confirmed)
            0.10, 0.01, &ace_ctx, &topics, 0.0, 1.0, 0.0,
            true, // stack_pain_match → ACE confirmed
            1.0,  // specific interest
        );
        assert_eq!(count, 2, "Interest + ACE(pain) = 2 signals");
        assert!(
            gated >= 0.50,
            "Two signals should pass relevance threshold, got {}",
            gated
        );
    }

    #[test]
    fn test_stack_pain_match_alone_cannot_pass() {
        // Only stack_pain_match=true, everything else below threshold.
        // Single signal property: score capped below 0.45
        let ace_ctx = ACEContext::default();
        let topics = vec!["test".to_string()];

        let (gated, count, _, _) = apply_confirmation_gate(
            0.90, // very high base
            0.10, 0.10, 0.10, 0.01, &ace_ctx, &topics, 0.0, 1.0, 0.0, true, // only signal
            1.0,  // specific interest
        );
        assert_eq!(count, 1, "Only ACE (via pain match) should be confirmed");
        assert!(
            gated < 0.45,
            "Single signal (pain match) should cap below 0.45, got {}",
            gated
        );
    }

    #[test]
    fn test_stack_pain_match_does_not_double_count_with_ace() {
        // ACE already confirmed via topic overlap + stack_pain_match also true.
        // ACE is ONE axis — count must not increase beyond what topic overlap gives.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("rust".to_string());
        let topics = vec!["rust".to_string()];

        // ACE confirmed via topic overlap alone
        let without_pain = count_confirmed_signals(
            0.10, 0.10, 0.10, 0.01, &ace_ctx, &topics, 0.0, 1.0, 0.0, false,
            1.0, // specific interest
        );
        assert!(without_pain.ace_confirmed);
        let count_without = without_pain.count;

        // ACE confirmed via topic overlap AND stack_pain_match
        let with_pain = count_confirmed_signals(
            0.10, 0.10, 0.10, 0.01, &ace_ctx, &topics, 0.0, 1.0, 0.0, true,
            1.0, // specific interest
        );
        assert!(with_pain.ace_confirmed);
        assert_eq!(
            with_pain.count, count_without,
            "stack_pain_match should not double-count ACE (both {} vs {})",
            with_pain.count, count_without
        );
    }

    #[test]
    fn test_5th_axis_gate_all_five_signals() {
        // All 5 signals confirmed
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("tokio".to_string());
        let topics = vec!["tokio".to_string()];

        let conf = count_confirmed_signals(
            0.50, // context: confirmed
            0.50, // interest: confirmed
            0.10, 0.30, // semantic: confirmed -> ace confirmed
            &ace_ctx, &topics, 0.20,  // feedback: confirmed
            1.5,   // affinity: confirmed (>= 1.3)
            0.30,  // dep_match_score: confirmed
            false, // no stack pain match
            1.0,   // specific interest
        );

        assert_eq!(conf.count, 5, "All 5 signals should be confirmed");

        let names = conf.confirmed_names();
        assert!(names.contains(&"context".to_string()));
        assert!(names.contains(&"interest".to_string()));
        assert!(names.contains(&"ace".to_string()));
        assert!(names.contains(&"learned".to_string()));
        assert!(names.contains(&"dependency".to_string()));
    }

    #[test]
    fn broad_interest_keyword_only_does_not_confirm() {
        // "Open Source" has specificity 0.25 — keyword match alone shouldn't confirm interest axis
        let ace = ACEContext::default();
        let conf = count_confirmed_signals(
            0.0,  // no context
            0.30, // below interest threshold (0.50)
            0.80, // above keyword threshold (0.70)
            0.0,  // no semantic
            &ace,
            &[],
            0.0, // no feedback
            1.0, // neutral affinity
            0.0, // no deps
            false,
            0.25, // broad interest specificity
        );
        assert!(
            !conf.interest_confirmed,
            "Broad interest keyword-only should NOT confirm interest axis"
        );
    }

    #[test]
    fn broad_interest_with_embedding_confirms() {
        // Broad interest with BOTH keyword AND embedding similarity should confirm
        let ace = ACEContext::default();
        let conf = count_confirmed_signals(
            0.0,
            0.40, // above 0.35 corroboration threshold
            0.80, // above keyword threshold
            0.0,
            &ace,
            &[],
            0.0,
            1.0,
            0.0,
            false,
            0.25, // broad interest
        );
        assert!(
            conf.interest_confirmed,
            "Broad interest with keyword+embedding should confirm"
        );
    }
}
