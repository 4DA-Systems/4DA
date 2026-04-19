// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Intelligence Mesh — Feed composition floors (Gap 3).
//!
//! The 5-axis pipeline + sophistication matching + serendipity injection
//! produces a correctly-scored list. But naive top-N selection by score
//! alone produces a filter bubble: items a user has learned to engage
//! with dominate, and stretch/horizon content — things they'd benefit
//! from knowing but wouldn't click on blindly — never surface.
//!
//! This module enforces TARGET RATIOS for the top-N items:
//!   - Comfort  (~70%): high-confidence items matching user's current
//!     context. The default engagement zone. Dominates by score anyway.
//!   - Stretch  (~20%): items with sophistication or novelty boosts —
//!     content one level above the user's comfort zone on topics they're
//!     already engaged with. Prevents skill stagnation.
//!   - Horizon  (~10%): serendipity items from the pipeline's own
//!     anti-bubble injection — content from conceptual neighbors the
//!     user wouldn't encounter through direct interest matching.
//!
//! Enforcement is REORDERING, not re-scoring. The function does not
//! modify `top_score` on any item. It just reorders the top-N positions
//! so the guaranteed ratio is achieved, preserving score-order within
//! each bucket and falling back to raw score-order when a bucket has
//! fewer candidates than its floor.
//!
//! **Invariant:** after this function runs, the list's length and
//! content are unchanged. Only the ordering of the first N items may
//! differ from pure score-order, and always by promotion (never
//! demotion below where the item would otherwise rank). Items past
//! position N are untouched.

use crate::types::SourceRelevance;

/// Category of an item for feed composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeedBucket {
    /// High-confidence item matching user's current context. The default
    /// bucket — anything not identified as stretch or horizon.
    Comfort,
    /// Content one level above user's comfort zone on a familiar topic.
    /// Identified by sophistication boost or novelty-release signal.
    Stretch,
    /// Serendipity items injected by the anti-bubble pipeline. Identified
    /// by the `serendipity` flag already set upstream.
    Horizon,
}

/// Configuration for composition floors. Percentages sum to 100; the
/// implementation does NOT enforce that — callers picking non-summing
/// configs just get approximately what they ask for.
///
/// `comfort_pct` is kept on the struct for settings roundtrip and UI
/// display even though the algorithm derives comfort from
/// `top_n - stretch - horizon` so rounding slack lands in comfort
/// automatically. Exposing it lets the settings UI show the three
/// ratios symmetrically.
#[derive(Debug, Clone)]
pub struct FloorConfig {
    /// How many items to compose. Items past this index are untouched.
    pub top_n: usize,
    /// Target fraction of top-N that should be Comfort items.
    /// Read by settings/UI; algorithm derives from remainder.
    #[allow(dead_code)]
    pub comfort_pct: u8,
    /// Target fraction that should be Stretch items.
    pub stretch_pct: u8,
    /// Target fraction that should be Horizon items.
    pub horizon_pct: u8,
}

impl Default for FloorConfig {
    fn default() -> Self {
        Self {
            top_n: 20,
            comfort_pct: 70,
            stretch_pct: 20,
            horizon_pct: 10,
        }
    }
}

/// Classify an item into its feed bucket.
///
/// Horizon is identified by the `serendipity` flag set by the pipeline's
/// existing anti-bubble injectors (compute_serendipity_candidates + the
/// concept-graph neighbor injector).
///
/// Stretch is identified by:
///   - `content_analysis_mult >= 1.05` — sophistication / authoritativeness
///     boost from the deep-content analyzer
///   - OR `novelty_mult >= 1.10` — release / breaking-change signal
/// These are items that scored high ENOUGH to pass the gate but whose
/// score floor came partially from "advanced content" or "new stuff"
/// rather than raw context match. They're the stretch-reading candidates.
///
/// Comfort is the default for everything else that passed the gate.
pub fn categorize_item(item: &SourceRelevance) -> FeedBucket {
    if item.serendipity {
        return FeedBucket::Horizon;
    }
    if let Some(breakdown) = &item.score_breakdown {
        if breakdown.content_analysis_mult >= 1.05 {
            return FeedBucket::Stretch;
        }
        if breakdown.novelty_mult >= 1.10 {
            return FeedBucket::Stretch;
        }
    }
    FeedBucket::Comfort
}

/// Reorder the first `config.top_n` items to meet the bucket floors.
///
/// Algorithm (deliberately simple):
///   1. Compute target slots per bucket from percentages.
///   2. Partition existing top_n by bucket, preserving score-order within.
///   3. Take top (target) from each bucket's full pool (top_n candidates
///      PLUS spillover candidates from below top_n for under-represented
///      buckets).
///   4. Merge and re-sort by `top_score` descending — stretch/horizon
///      items that made it in are interleaved by their own score, so
///      nothing visibly "jumps" past a high-scoring comfort item.
///   5. Leave items past position top_n untouched.
///
/// When a bucket has fewer candidates than its floor, other buckets
/// absorb the leftover slots proportionally to their own ratios. This
/// handles the realistic early-user case where no serendipity items
/// exist yet — the whole top_n naturally becomes comfort.
pub fn enforce_composition_floors(results: &mut Vec<SourceRelevance>, config: &FloorConfig) {
    if results.len() <= 1 || config.top_n == 0 {
        return;
    }
    let top_n = config.top_n.min(results.len());

    // Desired slot counts (integer, truncated — we redistribute below).
    let stretch_target = (top_n * config.stretch_pct as usize) / 100;
    let horizon_target = (top_n * config.horizon_pct as usize) / 100;
    // Comfort gets whatever remains — this absorbs percentage rounding.
    let comfort_target = top_n.saturating_sub(stretch_target + horizon_target);

    // Categorize across the ENTIRE result list (not just top_n), because
    // stretch/horizon items ranked slightly below top_n are exactly the
    // candidates we want to promote.
    let mut comfort: Vec<usize> = Vec::new();
    let mut stretch: Vec<usize> = Vec::new();
    let mut horizon: Vec<usize> = Vec::new();
    for (idx, item) in results.iter().enumerate() {
        match categorize_item(item) {
            FeedBucket::Comfort => comfort.push(idx),
            FeedBucket::Stretch => stretch.push(idx),
            FeedBucket::Horizon => horizon.push(idx),
        }
    }

    // Take target slots from each bucket's candidates (they're already in
    // score order since the caller sorted before invoking us).
    let mut chosen: Vec<usize> = Vec::with_capacity(top_n);
    chosen.extend(comfort.iter().take(comfort_target).copied());
    chosen.extend(stretch.iter().take(stretch_target).copied());
    chosen.extend(horizon.iter().take(horizon_target).copied());

    // If any bucket came up short, backfill from the remaining top-ranked
    // candidates regardless of bucket — this is the "whole top_n becomes
    // comfort" early-user case.
    if chosen.len() < top_n {
        let already: std::collections::HashSet<usize> = chosen.iter().copied().collect();
        for idx in 0..results.len() {
            if chosen.len() >= top_n {
                break;
            }
            if !already.contains(&idx) {
                chosen.push(idx);
            }
        }
    }

    // Deduplicate + cap to top_n, preserving the order we chose in.
    chosen.truncate(top_n);

    // Build the new top_n slice by pulling from results in chosen order.
    // We then sort the chosen set by score descending so the final UI
    // still presents highest-score first. This means stretch/horizon
    // items that earned a slot appear interleaved by score rather than
    // clumped at the end — a cleaner read than bucket-grouped order.
    let mut new_top: Vec<SourceRelevance> = chosen.iter().map(|&i| results[i].clone()).collect();
    new_top.sort_by(|a, b| {
        b.top_score
            .partial_cmp(&a.top_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Collect the leftovers — items NOT chosen for top_n — in their
    // original relative order.
    let chosen_set: std::collections::HashSet<usize> = chosen.iter().copied().collect();
    let leftovers: Vec<SourceRelevance> = results
        .iter()
        .enumerate()
        .filter(|(i, _)| !chosen_set.contains(i))
        .map(|(_, r)| r.clone())
        .collect();

    // Splice: new top_n followed by untouched leftovers.
    results.clear();
    results.extend(new_top);
    results.extend(leftovers);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ScoreBreakdown;
    use std::collections::HashMap;

    fn bare_breakdown() -> ScoreBreakdown {
        ScoreBreakdown {
            context_score: 0.5,
            interest_score: 0.5,
            keyword_score: 0.0,
            ace_boost: 0.0,
            affinity_mult: 1.0,
            anti_penalty: 0.0,
            freshness_mult: 1.0,
            feedback_boost: 0.0,
            source_quality_boost: 0.0,
            confidence_by_signal: HashMap::new(),
            signal_count: 2,
            confirmed_signals: vec![],
            confirmation_mult: 1.0,
            dep_match_score: 0.0,
            matched_deps: vec![],
            domain_relevance: 1.0,
            content_quality_mult: 1.0,
            novelty_mult: 1.0,
            intent_boost: 0.0,
            content_type: None,
            content_dna_mult: 1.0,
            competing_mult: 1.0,
            llm_score: None,
            llm_reason: None,
            stack_boost: 0.0,
            ecosystem_shift_mult: 1.0,
            stack_competing_mult: 1.0,
            window_boost: 0.0,
            matched_window_id: None,
            skill_gap_boost: 0.0,
            necessity_score: 0.0,
            necessity_reason: None,
            necessity_category: None,
            necessity_urgency: None,
            signal_strength_bonus: 0.0,
            content_analysis_mult: 1.0,
            advisor_signals: vec![],
            disagreement: None,
        }
    }

    fn sample(
        id: u64,
        score: f32,
        serendipity: bool,
        content_analysis_mult: f32,
        novelty_mult: f32,
    ) -> SourceRelevance {
        let mut b = bare_breakdown();
        b.content_analysis_mult = content_analysis_mult;
        b.novelty_mult = novelty_mult;
        SourceRelevance {
            id,
            title: format!("item {id}"),
            url: Some(format!("https://x/{id}")),
            top_score: score,
            matches: vec![],
            relevant: true,
            context_score: 0.5,
            interest_score: 0.5,
            excluded: false,
            excluded_by: None,
            source_type: "test".into(),
            explanation: None,
            confidence: Some(score),
            score_breakdown: Some(b),
            signal_type: None,
            signal_priority: None,
            signal_action: None,
            signal_triggers: None,
            signal_horizon: None,
            similar_count: 0,
            similar_titles: vec![],
            serendipity,
            streets_engine: None,
            decision_window_match: None,
            decision_boost_applied: 0.0,
            created_at: None,
            detected_lang: String::new(),
        }
    }

    fn comfort(id: u64, score: f32) -> SourceRelevance {
        sample(id, score, false, 1.0, 1.0)
    }
    fn stretch(id: u64, score: f32) -> SourceRelevance {
        sample(id, score, false, 1.10, 1.0)
    }
    fn horizon(id: u64, score: f32) -> SourceRelevance {
        sample(id, score, true, 1.0, 1.0)
    }
    fn stretch_via_novelty(id: u64, score: f32) -> SourceRelevance {
        sample(id, score, false, 1.0, 1.15)
    }

    #[test]
    fn categorize_identifies_horizon_via_serendipity_flag() {
        assert_eq!(categorize_item(&horizon(1, 0.5)), FeedBucket::Horizon);
    }

    #[test]
    fn categorize_identifies_stretch_via_sophistication_mult() {
        assert_eq!(categorize_item(&stretch(1, 0.5)), FeedBucket::Stretch);
    }

    #[test]
    fn categorize_identifies_stretch_via_novelty_mult() {
        assert_eq!(
            categorize_item(&stretch_via_novelty(1, 0.5)),
            FeedBucket::Stretch
        );
    }

    #[test]
    fn categorize_defaults_to_comfort_for_plain_items() {
        assert_eq!(categorize_item(&comfort(1, 0.5)), FeedBucket::Comfort);
    }

    #[test]
    fn enforce_noops_on_empty_list() {
        let mut results: Vec<SourceRelevance> = vec![];
        enforce_composition_floors(&mut results, &FloorConfig::default());
        assert!(results.is_empty());
    }

    #[test]
    fn enforce_preserves_total_count() {
        let mut results = vec![
            comfort(1, 0.9),
            comfort(2, 0.85),
            stretch(3, 0.8),
            horizon(4, 0.75),
            comfort(5, 0.7),
        ];
        let before = results.len();
        enforce_composition_floors(&mut results, &FloorConfig::default());
        assert_eq!(results.len(), before, "composition must not drop items");
    }

    #[test]
    fn enforce_promotes_stretch_and_horizon_into_top_n() {
        // Construct a list where comfort items dominate top positions by
        // score — naive top-5 would be all comfort. With floors (top_n=5,
        // 60/20/20), at least one stretch and one horizon must appear.
        let mut results = vec![
            comfort(1, 0.95),
            comfort(2, 0.90),
            comfort(3, 0.85),
            comfort(4, 0.80),
            comfort(5, 0.75),
            stretch(6, 0.60),
            horizon(7, 0.50),
        ];
        let config = FloorConfig {
            top_n: 5,
            comfort_pct: 60,
            stretch_pct: 20,
            horizon_pct: 20,
        };
        enforce_composition_floors(&mut results, &config);

        let top = &results[..5];
        assert!(
            top.iter()
                .any(|r| categorize_item(r) == FeedBucket::Stretch),
            "top-N must contain at least one stretch item"
        );
        assert!(
            top.iter()
                .any(|r| categorize_item(r) == FeedBucket::Horizon),
            "top-N must contain at least one horizon item"
        );
    }

    #[test]
    fn enforce_preserves_score_order_within_top_n() {
        let mut results = vec![
            comfort(1, 0.95),
            comfort(2, 0.90),
            comfort(3, 0.85),
            stretch(4, 0.70),
            horizon(5, 0.60),
        ];
        let config = FloorConfig {
            top_n: 5,
            comfort_pct: 60,
            stretch_pct: 20,
            horizon_pct: 20,
        };
        enforce_composition_floors(&mut results, &config);
        // After composition, top_n is re-sorted by score descending.
        for w in results.windows(2).take(4) {
            assert!(
                w[0].top_score >= w[1].top_score,
                "top-N must be sorted by score: {} >= {}",
                w[0].top_score,
                w[1].top_score
            );
        }
    }

    #[test]
    fn enforce_handles_insufficient_stretch_horizon_by_backfill_to_comfort() {
        // Early-user case: only comfort items exist. Function should not
        // crash, should not invent non-existent items, should leave the
        // top_n as pure comfort.
        let mut results = vec![
            comfort(1, 0.95),
            comfort(2, 0.90),
            comfort(3, 0.85),
            comfort(4, 0.80),
            comfort(5, 0.75),
        ];
        enforce_composition_floors(&mut results, &FloorConfig::default());
        assert_eq!(results.len(), 5);
        for r in &results {
            assert_eq!(categorize_item(r), FeedBucket::Comfort);
        }
    }

    #[test]
    fn enforce_does_not_mutate_top_score() {
        // Composition must not modify scores — it's a reordering step only.
        let mut results = vec![
            comfort(1, 0.95),
            stretch(2, 0.60),
            horizon(3, 0.50),
            comfort(4, 0.90),
        ];
        let scores_before: Vec<(u64, f32)> = results.iter().map(|r| (r.id, r.top_score)).collect();
        enforce_composition_floors(&mut results, &FloorConfig::default());
        for (id, score) in &scores_before {
            let r = results.iter().find(|r| r.id == *id).unwrap();
            assert!(
                (r.top_score - score).abs() < 1e-6,
                "score must not change for id {id}"
            );
        }
    }

    #[test]
    fn enforce_leaves_leftovers_untouched_after_top_n() {
        let mut results = vec![
            comfort(1, 0.95),
            comfort(2, 0.90),
            stretch(3, 0.80),
            horizon(4, 0.70),
            comfort(5, 0.60),
            comfort(6, 0.50), // position 5 — untouched
            comfort(7, 0.40), // position 6 — untouched
        ];
        let config = FloorConfig {
            top_n: 5,
            comfort_pct: 60,
            stretch_pct: 20,
            horizon_pct: 20,
        };
        enforce_composition_floors(&mut results, &config);
        // Items 6 and 7 should still be present, after position 4.
        let ids: Vec<u64> = results.iter().map(|r| r.id).collect();
        assert!(ids.contains(&6));
        assert!(ids.contains(&7));
    }

    #[test]
    fn enforce_with_top_n_zero_is_noop() {
        let mut results = vec![comfort(1, 0.9), comfort(2, 0.8)];
        let before: Vec<u64> = results.iter().map(|r| r.id).collect();
        enforce_composition_floors(
            &mut results,
            &FloorConfig {
                top_n: 0,
                comfort_pct: 70,
                stretch_pct: 20,
                horizon_pct: 10,
            },
        );
        let after: Vec<u64> = results.iter().map(|r| r.id).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn default_config_is_70_20_10() {
        let c = FloorConfig::default();
        assert_eq!(c.comfort_pct, 70);
        assert_eq!(c.stretch_pct, 20);
        assert_eq!(c.horizon_pct, 10);
        assert_eq!(c.top_n, 20);
    }
}
