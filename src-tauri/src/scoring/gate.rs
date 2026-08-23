// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::ace_context::ACEContext;
use super::utils::topic_grounds;
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

/// Additional evidence provenance the caller can supply so the gate can tell
/// apart signals that LOOK independent but share one underlying source.
///
/// Introduced 2026-08-23 (adversarial audit items 8e + 14). The legacy
/// `count_confirmed_signals` wrapper passes `GateEvidence::default()`, which
/// reproduces the pre-provenance behavior bit-for-bit — the pipeline call
/// site opts into real provenance by calling
/// `count_confirmed_signals_with_evidence` directly.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GateEvidence {
    /// True when `semantic_boost` came from real embedding similarity
    /// (`compute_semantic_ace_boost` returned `Some`), false when it is the
    /// keyword fallback (`compute_keyword_ace_boost`). The fallback re-reads
    /// the SAME keyword surface as the interest axis, so it must never count
    /// as independent ACE evidence against a keyword-confirmed interest.
    pub semantic_is_embedding_derived: bool,
    /// Raw (un-discounted) keyword score over ONLY the user's own
    /// primary-stack single-word interests
    /// (`keywords::own_stack_single_word_keyword_score`). 0.0 = no such
    /// interest matched. Lets the gate confirm the interest axis for the
    /// user's own stack without inflating the score-side keyword magnitude.
    pub own_stack_keyword_score: f32,
}

impl Default for GateEvidence {
    fn default() -> Self {
        Self {
            // Legacy-compatible: before provenance existed, every
            // above-threshold semantic boost was treated as independent.
            semantic_is_embedding_derived: true,
            own_stack_keyword_score: 0.0,
        }
    }
}

/// Embedding corroboration required before an own-stack single-word keyword
/// hit may confirm the interest axis. Mirrors the broad-interest
/// corroboration bar hardcoded in the broad branch below: keyword evidence
/// alone (one word colliding with a title — "Rust Belt cities…") can never
/// confirm; the item embedding must at least weakly agree with the interest.
///
/// 0.35 is MEASURED, not arbitrary — do not lower it without re-running the
/// experiment (2026-08-24, real-embedding benchmark): at 0.28 the genuine
/// systems-content scenario tp_systems_programming recovers (corroboration
/// exactly 0.28), but edge_mixed_signal ("How Rust Belt cities are
/// reinventing themselves through urban farming") lands at 0.445 — a
/// non-technical region article crossing the 0.40 relevance line for a Rust
/// developer. Arctic-M cannot separate those two classes in [0.28, 0.35);
/// closing that gap is representation quality (audit item 27), not threshold
/// tuning.
// TODO(orchestrator): candidate for promotion to pipeline.scoring as a tunable.
const OWN_STACK_KEYWORD_CORROBORATION: f32 = 0.35;

/// Count how many independent signal axes confirm this item is relevant.
/// Each axis answers a different question:
/// - Context: Does this match code you're actually writing? (KNN embedding similarity)
/// - Interest: Does this match your declared interests? (interest embedding + keyword)
/// - ACE/Tech: Does this involve your tech stack or active topics? (semantic boost + tech detection)
/// - Learned: Has user behavior confirmed this kind of content? (feedback + affinity)
/// - Dependency: Does this mention packages from your installed dependencies?
///
/// Legacy entry point: identical to `count_confirmed_signals_with_evidence`
/// with `GateEvidence::default()` (semantic treated as embedding-derived, no
/// own-stack keyword evidence) — the pre-2026-08-23 behavior.
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
    count_confirmed_signals_with_evidence(
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
        GateEvidence::default(),
    )
}

/// Evidence-aware variant of [`count_confirmed_signals`] — see [`GateEvidence`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn count_confirmed_signals_with_evidence(
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
    evidence: GateEvidence,
) -> SignalConfirmation {
    let context_confirmed = context_score >= scoring_config::CONTEXT_THRESHOLD;
    // Own-stack keyword confirmation (2026-08-23 audit item 14): a single-word
    // interest that IS the user's own primary stack ("Tauri" for a Tauri
    // developer) may confirm the interest axis from its RAW keyword score —
    // the single-word specificity discount (0.60) otherwise caps keyword
    // evidence at 0.80 × 0.60 = 0.48 < the 0.70 threshold, structurally
    // locking the keyword route out for every user with 3+ interests. The
    // discount still applies to the score magnitude; only CONFIRMATION is
    // exempted, and only with embedding corroboration, so a bare word
    // collision on an off-topic item cannot manufacture a signal.
    let own_stack_keyword_confirms = evidence.own_stack_keyword_score
        >= scoring_config::KEYWORD_THRESHOLD
        && interest_score >= OWN_STACK_KEYWORD_CORROBORATION;
    // Broad interests (specificity < 0.50, e.g. "Open Source") cannot confirm the interest
    // axis from keyword matching alone — they need corroboration from embedding similarity.
    let interest_confirmed = own_stack_keyword_confirms
        || if best_keyword_specificity < 0.50 {
            // Broad interest: require BOTH keyword AND embedding, OR very strong embedding alone
            // (>= INTEREST_THRESHOLD of 0.50 indicates high semantic match even without keywords)
            (keyword_score >= scoring_config::KEYWORD_THRESHOLD && interest_score >= 0.35)
                || interest_score >= scoring_config::INTEREST_THRESHOLD
        } else {
            interest_score >= scoring_config::INTEREST_THRESHOLD
                || keyword_score >= scoring_config::KEYWORD_THRESHOLD
        };
    // ACE confirmed: require semantic boost OR active topic match (NOT broad detected_tech).
    // Uses strict topic grounding (v12): a generic shared fragment — item topic
    // "http" against active topic "tower-http" — can never confirm this axis.
    // Stack pain point match also contributes to ACE axis (content about your stack's problems).
    let ace_via_active_topics = topics
        .iter()
        .any(|t| ace_ctx.active_topics.iter().any(|at| topic_grounds(t, at)));
    let ace_confirmed = semantic_boost >= scoring_config::SEMANTIC_THRESHOLD
        || ace_via_active_topics
        || stack_pain_match;
    // Learned axis DEMOTED in v19 (AD-029): topic history alone could flip
    // signal_count 1→2, multiplying the score ceiling 0.28→0.72 (2.57×) —
    // the single highest-leverage lever in the system — while its inputs
    // came from a capture layer with three incompatible strength scales and
    // a documented self-poisoning incident (2026-07-13 doom loop: the
    // user's own stack driven to −1.0 affinity by passive scroll noise).
    // The axis structurally remains (breakdowns, gate table, public "5
    // axes" docs) but never confirms until the re-enable criteria in
    // AD-029 are met. The `feedback_boost`/`affinity_mult` params stay so
    // breakdown plumbing and a future re-enable keep their wiring.
    let _ = (feedback_boost, affinity_mult);
    let learned_confirmed = false;
    let dependency_confirmed = dep_match_score >= scoring_config::DEPENDENCY_THRESHOLD;

    // Deduplicate interest + ACE when ONLY keyword matching drives both axes.
    // ACE active_topics (from scanning the user's actual project files) ARE genuinely
    // independent evidence — the user declared an interest AND their code confirms it.
    // Only dedup when the overlap is purely keyword-level with no project-level backing.
    //
    // Independence is judged on PROVENANCE (2026-08-23 audit item 8e): the old
    // check repeated the `ace_confirmed` disjunction verbatim, so
    // `ace_independent ≡ ace_confirmed` and this guard could never fire. Worse,
    // with embeddings unavailable the KEYWORD-fallback ACE boost (0.25–0.30)
    // exceeds SEMANTIC_THRESHOLD (0.18), so a DEGRADED state confirmed more
    // signals and flipped the 1↔2 cliff (±0.44). A semantic boost only counts
    // as independent when it is embedding-derived; the keyword fallback shares
    // its evidence surface with the interest axis and dedups against it.
    let semantic_independent = evidence.semantic_is_embedding_derived
        && semantic_boost >= scoring_config::SEMANTIC_THRESHOLD;
    let ace_independent = semantic_independent || stack_pain_match || ace_via_active_topics;
    let deduped_ace = if interest_confirmed && ace_confirmed {
        ace_independent
    } else {
        ace_confirmed
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

// NOTE: the former `apply_confirmation_gate` (count → CONFIRMATION_GATE lookup →
// direct-dep ceiling bypass → `base * mult` clamp) was deleted 2026-08-12 with the
// V1 pipeline, its only caller. V2 calls `count_confirmed_signals` directly and
// applies the gate table itself in `pipeline_v2.rs` — it has to, because V2 defers
// the ceiling until after the domain gate multiplier.

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
    fn test_ace_axis_generic_fragment_cannot_confirm() {
        // v12: an item whose ONLY ACE overlap is a generic shared fragment
        // (topic "http" vs active topic "tower-http") must NOT confirm the
        // ACE axis — that was the phantom-CORE disease.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("tower-http".to_string());
        let topics = vec!["http".to_string()];
        let conf = count_confirmed_signals(
            0.10, 0.10, 0.10, 0.01, // all other axes below threshold
            &ace_ctx, &topics, 0.0, 1.0, 0.0, false, 1.0,
        );
        assert!(
            !conf.ace_confirmed,
            "generic fragment 'http' ~ 'tower-http' must not confirm ACE axis"
        );
        assert_eq!(conf.count, 0);

        // A specific topic overlap still confirms.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("tauri".to_string());
        let topics = vec!["tauri".to_string()];
        let conf = count_confirmed_signals(
            0.10, 0.10, 0.10, 0.01, &ace_ctx, &topics, 0.0, 1.0, 0.0, false, 1.0,
        );
        assert!(
            conf.ace_confirmed,
            "specific topic 'tauri' must still confirm ACE axis"
        );
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
    fn learned_axis_never_confirms_even_with_maximal_inputs() {
        // v19 (AD-029): the learned axis is structurally present but demoted
        // — even feedback_boost and affinity values that would have confirmed
        // it pre-v19 must not count. Practical maximum is 4 of 5 axes.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("tokio".to_string());
        let topics = vec!["tokio".to_string()];

        let conf = count_confirmed_signals(
            0.50, // context: confirmed
            0.50, // interest: confirmed
            0.10, 0.30, // semantic: confirmed -> ace confirmed
            &ace_ctx, &topics, 0.20,  // feedback: would have confirmed pre-v19
            1.5,   // affinity: would have confirmed pre-v19
            0.30,  // dep_match_score: confirmed
            false, // no stack pain match
            1.0,   // specific interest
        );

        assert_eq!(
            conf.count, 4,
            "learned axis is demoted (AD-029) — 4 axes is the maximum"
        );
        assert!(!conf.learned_confirmed);

        let names = conf.confirmed_names();
        assert!(names.contains(&"context".to_string()));
        assert!(names.contains(&"interest".to_string()));
        assert!(names.contains(&"ace".to_string()));
        assert!(
            !names.contains(&"learned".to_string()),
            "learned must never appear in confirmed names"
        );
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

    // ========================================================================
    // GateEvidence: ACE independence provenance (2026-08-23 audit item 8e)
    // ========================================================================

    #[test]
    fn legacy_wrapper_equals_default_evidence() {
        // The wrapper must reproduce the evidence variant with defaults exactly.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("tauri".to_string());
        let topics = vec!["tauri".to_string()];
        let legacy = count_confirmed_signals(
            0.50, 0.50, 0.80, 0.25, &ace_ctx, &topics, 0.0, 1.0, 0.30, false, 1.0,
        );
        let with_default = count_confirmed_signals_with_evidence(
            0.50,
            0.50,
            0.80,
            0.25,
            &ace_ctx,
            &topics,
            0.0,
            1.0,
            0.30,
            false,
            1.0,
            GateEvidence::default(),
        );
        assert_eq!(legacy.count, with_default.count);
        assert_eq!(legacy.confirmed_names(), with_default.confirmed_names());
    }

    #[test]
    fn keyword_fallback_ace_dedups_against_keyword_confirmed_interest() {
        // Item 8e: interest confirmed via keywords + ACE "confirmed" ONLY via
        // the keyword-fallback semantic boost (embeddings unavailable) is ONE
        // signal — the fallback re-reads the same keyword surface. The old
        // tautological ace_independent counted this as two.
        let ace_ctx = ACEContext::default(); // no active topics → no grounding
        let topics = vec!["somelib".to_string()];
        let conf = count_confirmed_signals_with_evidence(
            0.10, // context: not confirmed
            0.10, // interest embedding: not confirmed
            0.80, // keyword: confirms interest axis (specific interest)
            0.25, // semantic boost ABOVE 0.18 — but keyword-fallback derived
            &ace_ctx,
            &topics,
            0.0,
            1.0,
            0.0,
            false, // no stack pain
            1.0,   // specific interest
            GateEvidence {
                semantic_is_embedding_derived: false,
                own_stack_keyword_score: 0.0,
            },
        );
        assert!(conf.interest_confirmed, "interest confirmed via keyword");
        assert!(
            conf.ace_confirmed,
            "ACE axis still shows confirmed in the breakdown"
        );
        assert_eq!(
            conf.count, 1,
            "keyword-fallback ACE must dedup against a keyword-confirmed interest"
        );
    }

    #[test]
    fn embedding_derived_semantic_stays_independent() {
        // Same shape, but the semantic boost came from real embeddings —
        // genuinely independent evidence → two signals.
        let ace_ctx = ACEContext::default();
        let topics = vec!["somelib".to_string()];
        let conf = count_confirmed_signals_with_evidence(
            0.10,
            0.10,
            0.80,
            0.25,
            &ace_ctx,
            &topics,
            0.0,
            1.0,
            0.0,
            false,
            1.0,
            GateEvidence {
                semantic_is_embedding_derived: true,
                own_stack_keyword_score: 0.0,
            },
        );
        assert_eq!(
            conf.count, 2,
            "embedding-derived semantic boost is independent ACE evidence"
        );
    }

    #[test]
    fn active_topic_grounding_stays_independent_without_embeddings() {
        // Item 8e positive case: interest confirmed + ACE via an active-topic
        // project scan → two signals even when embeddings are unavailable.
        // Scanning the user's actual project files IS independent evidence.
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("tauri".to_string());
        let topics = vec!["tauri".to_string()];
        let conf = count_confirmed_signals_with_evidence(
            0.10,
            0.10,
            0.80, // keyword confirms interest
            0.25, // fallback boost (not embedding-derived)
            &ace_ctx,
            &topics,
            0.0,
            1.0,
            0.0,
            false,
            1.0,
            GateEvidence {
                semantic_is_embedding_derived: false,
                own_stack_keyword_score: 0.0,
            },
        );
        assert_eq!(
            conf.count, 2,
            "active-topic grounding must keep ACE independent of the interest axis"
        );
    }

    #[test]
    fn fallback_ace_alone_still_confirms_the_axis() {
        // When ACE is the ONLY evidence, the keyword fallback still lights the
        // axis (degraded mode must not go blind) — it just cannot double-count
        // against a keyword-confirmed interest.
        let ace_ctx = ACEContext::default();
        let topics = vec!["somelib".to_string()];
        let conf = count_confirmed_signals_with_evidence(
            0.10,
            0.10,
            0.10, // interest NOT confirmed
            0.25, // fallback boost above threshold
            &ace_ctx,
            &topics,
            0.0,
            1.0,
            0.0,
            false,
            1.0,
            GateEvidence {
                semantic_is_embedding_derived: false,
                own_stack_keyword_score: 0.0,
            },
        );
        assert!(conf.ace_confirmed);
        assert_eq!(conf.count, 1, "fallback ACE alone is still one signal");
    }

    #[test]
    fn stack_pain_match_stays_independent_without_embeddings() {
        let ace_ctx = ACEContext::default();
        let topics = vec!["somelib".to_string()];
        let conf = count_confirmed_signals_with_evidence(
            0.10,
            0.10,
            0.80, // keyword confirms interest
            0.01, // no semantic boost at all
            &ace_ctx,
            &topics,
            0.0,
            1.0,
            0.0,
            true, // stack pain — binary, project-derived, independent
            1.0,
            GateEvidence {
                semantic_is_embedding_derived: false,
                own_stack_keyword_score: 0.0,
            },
        );
        assert_eq!(conf.count, 2, "stack pain keeps ACE independent");
    }

    // ========================================================================
    // GateEvidence: own-stack keyword confirmation (2026-08-23 audit item 14)
    // ========================================================================

    #[test]
    fn own_stack_keyword_with_corroboration_confirms_interest() {
        // "Tauri 2 …" title for a Tauri developer: post-discount keyword is
        // 0.80 × 0.60 = 0.48 (below threshold), but the raw own-stack keyword
        // evidence is 0.80 and the embedding weakly agrees (0.40 ≥ 0.35) →
        // the interest axis confirms.
        let ace_ctx = ACEContext::default();
        let topics: Vec<String> = vec![];
        let conf = count_confirmed_signals_with_evidence(
            0.10,
            0.40, // embedding corroboration ≥ 0.35, below 0.45 threshold
            0.48, // post-discount keyword (0.80 × 0.60) — below 0.70
            0.01,
            &ace_ctx,
            &topics,
            0.0,
            1.0,
            0.0,
            false,
            0.60, // single-word specificity
            GateEvidence {
                semantic_is_embedding_derived: true,
                own_stack_keyword_score: 0.80,
            },
        );
        assert!(
            conf.interest_confirmed,
            "own-stack single-word keyword + corroboration must confirm interest"
        );
        assert_eq!(conf.count, 1);
    }

    #[test]
    fn own_stack_keyword_without_corroboration_does_not_confirm() {
        // The "Rust Belt cities" case: strong raw keyword hit on an own-stack
        // word, but the item embedding does NOT corroborate → no confirmation.
        let ace_ctx = ACEContext::default();
        let topics: Vec<String> = vec![];
        let conf = count_confirmed_signals_with_evidence(
            0.10,
            0.10, // embedding says: not about this interest
            0.48,
            0.01,
            &ace_ctx,
            &topics,
            0.0,
            1.0,
            0.0,
            false,
            0.60,
            GateEvidence {
                semantic_is_embedding_derived: true,
                own_stack_keyword_score: 1.0, // word collision, maximal
            },
        );
        assert!(
            !conf.interest_confirmed,
            "own-stack keyword without embedding corroboration must NOT confirm"
        );
        assert_eq!(conf.count, 0);
    }

    #[test]
    fn off_stack_single_word_keeps_the_discount_lockout() {
        // No own-stack evidence supplied: a single-word interest's discounted
        // keyword score (0.48) still cannot confirm — the exemption is
        // own-domain only.
        let ace_ctx = ACEContext::default();
        let topics: Vec<String> = vec![];
        let conf = count_confirmed_signals_with_evidence(
            0.10,
            0.40,
            0.48,
            0.01,
            &ace_ctx,
            &topics,
            0.0,
            1.0,
            0.0,
            false,
            0.60,
            GateEvidence {
                semantic_is_embedding_derived: true,
                own_stack_keyword_score: 0.0,
            },
        );
        assert!(
            !conf.interest_confirmed,
            "off-stack single-word interests keep the specificity lockout"
        );
    }

    #[test]
    fn own_stack_raw_below_threshold_does_not_confirm() {
        // Own-stack interest matched only in content (raw 0.55 < 0.70): the
        // exemption never lowers the keyword bar, it only un-discounts it.
        let ace_ctx = ACEContext::default();
        let topics: Vec<String> = vec![];
        let conf = count_confirmed_signals_with_evidence(
            0.10,
            0.40,
            0.33, // 0.55 × 0.60
            0.01,
            &ace_ctx,
            &topics,
            0.0,
            1.0,
            0.0,
            false,
            0.60,
            GateEvidence {
                semantic_is_embedding_derived: true,
                own_stack_keyword_score: 0.55,
            },
        );
        assert!(
            !conf.interest_confirmed,
            "content-only own-stack hits stay below the confirmation bar"
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
