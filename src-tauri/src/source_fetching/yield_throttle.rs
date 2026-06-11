// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Adaptive per-source fetch throttling by relevance YIELD.
//!
//! The firehose problem: some sources flood the corpus with content that is
//! near-uniformly irrelevant to THIS user (e.g. for a Rust-systems developer,
//! HuggingFace ML posts score ~0.06 on average — 4 useful items out of 10k).
//! Embedding + storing that volume forever is pure waste.
//!
//! Rather than gate individual items at ingest (measured + rejected: a semantic
//! gate has no good operating point, it shreds recall — see
//! `project_scoring_relevance_funnel`), we throttle at the SOURCE: a source that
//! consistently yields little relevance for this user gets fetched less. This is
//! the "gets sharper every day" thesis applied to acquisition — and it is
//! self-correcting, because:
//!   - the yield is measured over a RECENT window, so if the user's interests
//!     shift (or a quiet source starts producing signal) its yield rises and
//!     volume returns;
//!   - throttled sources keep a DISCOVERY trickle (never throttled to zero), so
//!     "yesterday's noise" still gets a chance to become "tomorrow's signal".
//!
//! What we DON'T throttle:
//!   - sources with too little scored history (cold-start: no evidence yet);
//!   - high-value-but-low-score sources (security advisories score low on
//!     generic relevance but are never noise) — see [`is_relevance_exempt`].

/// Relevance-yield stats for one source over the measurement window.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceYield {
    /// Number of scored items from this source in the window.
    pub scored: i64,
    /// Fraction of those that scored at or above the relevance floor (0.0..=1.0).
    pub hit_rate: f64,
}

/// Items must score at or above this to count as a "hit" for the source.
/// Matches the proactive-feed relevance floor (settings `embedding_threshold` 0.2).
const RELEVANCE_FLOOR: f64 = 0.20;
/// A source whose hit-rate is at/above this earns its FULL fetch budget.
const TARGET_HIT_RATE: f64 = 0.25;
/// Never throttle below this fraction of base — keeps a discovery trickle alive.
const MIN_MULTIPLIER: f64 = 0.10;
/// Don't throttle until we have at least this many scored items (cold-start).
const MIN_SAMPLE: i64 = 200;
/// Absolute floor on items/cycle for any source, so even a hard-throttled source
/// keeps feeding a few items for re-evaluation.
const DISCOVERY_FLOOR: usize = 5;

pub(crate) const RELEVANCE_FLOOR_PUB: f64 = RELEVANCE_FLOOR;

/// Sources whose items are high-value even when their generic relevance_score is
/// low (security advisories about a dependency you DO use score low on topical
/// similarity but are exactly what Preemption exists to surface). These are never
/// throttled by relevance yield.
pub(crate) fn is_relevance_exempt(source_type: &str) -> bool {
    matches!(source_type, "osv" | "cve")
}

/// Compute the per-cycle fetch cap for a source given its base budget and its
/// measured relevance yield. Pure + deterministic — the policy lives here so it
/// can be tested without a database.
pub(crate) fn fetch_cap(base: usize, stats: Option<&SourceYield>, source_type: &str) -> usize {
    if base == 0 {
        return 0;
    }
    // High-value sources keep their full budget regardless of generic relevance.
    if is_relevance_exempt(source_type) {
        return base;
    }
    // Cold-start: not enough scored history to judge — don't throttle yet.
    let stats = match stats {
        Some(s) if s.scored >= MIN_SAMPLE => s,
        _ => return base,
    };
    let multiplier = (stats.hit_rate / TARGET_HIT_RATE).clamp(MIN_MULTIPLIER, 1.0);
    let cap = (base as f64 * multiplier).round() as usize;
    // Never throttle a source completely silent; keep a discovery trickle, but
    // never raise above the base budget either.
    cap.clamp(DISCOVERY_FLOOR.min(base), base)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y(scored: i64, hit_rate: f64) -> SourceYield {
        SourceYield { scored, hit_rate }
    }

    #[test]
    fn full_budget_when_no_stats_or_cold_start() {
        assert_eq!(fetch_cap(50, None, "hackernews"), 50);
        // Below MIN_SAMPLE → not enough evidence → full budget.
        assert_eq!(fetch_cap(50, Some(&y(50, 0.0)), "hackernews"), 50);
    }

    #[test]
    fn high_yield_source_keeps_full_budget() {
        // reddit-like: 32% hit rate, above target → full.
        assert_eq!(fetch_cap(50, Some(&y(5000, 0.32)), "reddit"), 50);
        // exactly at target → full.
        assert_eq!(fetch_cap(50, Some(&y(5000, 0.25)), "devto"), 50);
    }

    #[test]
    fn near_pure_noise_source_is_hard_throttled_but_not_silenced() {
        // huggingface-like: 2% hit rate over a large sample.
        let cap = fetch_cap(50, Some(&y(10000, 0.02)), "huggingface");
        // 0.02/0.25 = 0.08 → clamped up to MIN_MULTIPLIER 0.10 → 5; floor also 5.
        assert_eq!(cap, 5);
        assert!(cap >= DISCOVERY_FLOOR, "must keep a discovery trickle");
    }

    #[test]
    fn moderate_yield_is_partially_throttled() {
        // crates_io-like: 16% hit rate → 0.16/0.25 = 0.64 → 32.
        assert_eq!(fetch_cap(50, Some(&y(12000, 0.16)), "crates_io"), 32);
    }

    #[test]
    fn security_sources_are_never_relevance_throttled() {
        // osv/cve score low on generic relevance but are high-value.
        assert_eq!(fetch_cap(50, Some(&y(5000, 0.01)), "osv"), 50);
        assert_eq!(fetch_cap(50, Some(&y(5000, 0.01)), "cve"), 50);
    }

    #[test]
    fn never_exceeds_base_and_handles_small_base() {
        assert_eq!(fetch_cap(50, Some(&y(5000, 5.0)), "rss"), 50); // impossible hit_rate clamped
                                                                   // base smaller than the discovery floor must not inflate the budget.
        assert_eq!(fetch_cap(3, Some(&y(10000, 0.0)), "huggingface"), 3);
    }
}
