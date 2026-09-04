// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Corpus-relative calibration for the two embedding axes (v29, 2026-09-04).
//!
//! Both embedding axes used to be calibrated with a per-MODEL cosine sigmoid
//! (`embedding_calibration`) that never described what the axes consumed:
//!
//! * the CONTEXT axis fed `1/(1+L2)` — not cosine — into that sigmoid. For
//!   unit vectors an unrelated pair sits at L2 ≈ 1.0–1.4 → raw 0.42–0.50 →
//!   calibrated 0.50–0.75, so the axis "confirmed" on 87.7% of the whole
//!   corpus and 100% of the feed (political headlines at 0.73–0.82). The
//!   2-signal gate was a 1-signal gate and "Similar to your code in main.rs"
//!   was the explanation on a dog-charity job ad;
//! * the SEMANTIC ACE boost averaged the cosine over every active topic and
//!   detected tech (~55 entries) into a uniform ~0.47, so its 0.18 threshold
//!   was reached by 0 of 20,014 breakdowns.
//!
//! Model tables cannot fix this: nomic's cosine range is compressed and the
//! right threshold depends on what the user's corpus looks like. So both axes
//! are calibrated against the LIVE corpus when the scoring context is built:
//!
//! * context: the cosine at the corpus `context_confirm_percentile` of top-1
//!   KNN similarity maps to the confirmation threshold, and the
//!   `context_strong_percentile` maps to 0.85. "Confirmed" therefore means
//!   "closer to the user's code than ~75% of what we ingest";
//! * semantic: the TOP-3 mean stack similarity at the corpus median is the
//!   zero-boost pivot and the `semantic_confirm_percentile` reaches exactly
//!   `SEMANTIC_THRESHOLD`, so ~10% of ingested items can confirm the ACE axis
//!   on embeddings alone.
//!
//! Both fall back to the per-model sigmoid / legacy pivot when the corpus is
//! too small to calibrate (`min_sample`), which is the cold-start state.

use crate::scoring_config;

/// Where a calibration came from — carried so a log line (and the breakdown
/// consumer that wants it) can tell a corpus-fitted axis from a fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CalibrationSource {
    /// Fitted from a sample of the live corpus.
    Corpus,
    /// Corpus too small (or unavailable): the pre-v29 per-model behaviour.
    Fallback,
}

/// Sigmoid parameters for the context axis, applied to COSINE similarity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ContextCalibration {
    pub center: f32,
    pub scale: f32,
    pub source: CalibrationSource,
}

impl Default for ContextCalibration {
    /// The per-model cosine sigmoid — correct for cosine input, which is what
    /// `calibrate_context` now feeds it (the pre-v29 bug was the INPUT unit,
    /// so the fallback is already a strict improvement).
    fn default() -> Self {
        Self {
            center: crate::embedding_calibration::get_sigmoid_center(),
            scale: crate::embedding_calibration::get_sigmoid_scale(),
            source: CalibrationSource::Fallback,
        }
    }
}

/// Linear map for the semantic ACE boost: `(top_k_mean - pivot) * gain`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SemanticCalibration {
    pub pivot: f32,
    pub gain: f32,
    pub source: CalibrationSource,
}

impl Default for SemanticCalibration {
    /// The legacy formula `(avg - 0.5) * 1.0`, kept as the cold-start shape.
    fn default() -> Self {
        Self {
            pivot: 0.5,
            gain: 1.0,
            source: CalibrationSource::Fallback,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct CorpusCalibration {
    pub context: ContextCalibration,
    pub semantic: SemanticCalibration,
}

/// Calibrated value the context axis is confirmed at (`CONTEXT_THRESHOLD`)
/// when the raw cosine sits at the corpus confirm percentile.
const CONTEXT_CONFIRM_TARGET: f32 = scoring_config::CONTEXT_THRESHOLD;
/// Calibrated value a clearly-strong match reaches at the strong percentile.
const CONTEXT_STRONG_TARGET: f32 = 0.85;
/// The semantic boost clamp — unchanged from the pre-v29 formula so every
/// downstream consumer (gate threshold 0.18, multiplicative relevance term)
/// keeps its range.
const SEMANTIC_BOOST_MIN: f32 = -0.3;
const SEMANTIC_BOOST_MAX: f32 = 0.5;
/// Top-k for the semantic ACE boost: the mean of the three closest stack
/// elements. Three is the smallest k that is not a single-topic spike while
/// still ignoring the ~50 unrelated topics an average would drown in.
pub(crate) const SEMANTIC_TOP_K: usize = 3;

/// Cosine similarity of two UNIT vectors from their L2 distance
/// (`|a-b|² = 2 - 2·cos`). Every stored embedding is L2-normalized by
/// `embeddings::truncate_and_normalize`, so this is exact; a distance beyond
/// 2.0 (a non-unit legacy blob) clamps to −1 rather than inventing similarity.
pub(crate) fn l2_to_cosine(distance: f32) -> f32 {
    if !distance.is_finite() {
        return -1.0;
    }
    (1.0 - distance * distance / 2.0).clamp(-1.0, 1.0)
}

/// Value at percentile `p` (0..=1) of an ASCENDING-sorted, non-empty slice.
pub(crate) fn percentile(sorted: &[f32], p: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f32 * p.clamp(0.0, 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn logit(p: f32) -> f32 {
    (p / (1.0 - p)).ln()
}

fn enough(sample: usize) -> bool {
    sample >= scoring_config::CORPUS_CALIBRATION_MIN_SAMPLE as usize
}

/// Fit the context sigmoid so the corpus confirm percentile of top-1 cosine
/// lands on `CONTEXT_THRESHOLD` and the strong percentile on 0.85.
///
/// `None` when the sample is too small or degenerate (the two percentiles
/// coincide — a constant axis cannot be calibrated, and the caller falls back).
pub(crate) fn context_from_top1_cosines(cosines: &mut [f32]) -> Option<ContextCalibration> {
    if !enough(cosines.len()) {
        return None;
    }
    cosines.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let c_confirm = percentile(
        cosines,
        scoring_config::CORPUS_CALIBRATION_CONTEXT_CONFIRM_PERCENTILE,
    );
    let c_strong = percentile(
        cosines,
        scoring_config::CORPUS_CALIBRATION_CONTEXT_STRONG_PERCENTILE,
    );
    let spread = c_strong - c_confirm;
    if spread < 0.005 {
        return None;
    }
    // sigmoid(x) = 1 / (1 + exp(-(x - center) * scale))
    //   logit(target_confirm) = (c_confirm - center) * scale
    //   logit(target_strong)  = (c_strong  - center) * scale
    let scale =
        ((logit(CONTEXT_STRONG_TARGET) - logit(CONTEXT_CONFIRM_TARGET)) / spread).clamp(1.0, 500.0);
    let center = c_confirm - logit(CONTEXT_CONFIRM_TARGET) / scale;
    Some(ContextCalibration {
        center,
        scale,
        source: CalibrationSource::Corpus,
    })
}

/// Fit the semantic boost so the corpus median top-k mean is the zero pivot
/// and the confirm percentile reaches exactly `SEMANTIC_THRESHOLD`.
pub(crate) fn semantic_from_top_k_means(means: &mut [f32]) -> Option<SemanticCalibration> {
    if !enough(means.len()) {
        return None;
    }
    means.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pivot = percentile(
        means,
        scoring_config::CORPUS_CALIBRATION_SEMANTIC_PIVOT_PERCENTILE,
    );
    let confirm = percentile(
        means,
        scoring_config::CORPUS_CALIBRATION_SEMANTIC_CONFIRM_PERCENTILE,
    );
    let spread = confirm - pivot;
    if spread < 0.005 {
        return None;
    }
    let gain = (scoring_config::SEMANTIC_THRESHOLD / spread).clamp(0.5, 50.0);
    Some(SemanticCalibration {
        pivot,
        gain,
        source: CalibrationSource::Corpus,
    })
}

/// Calibrated context score for a raw COSINE similarity.
pub(crate) fn calibrate_context(cosine: f32, cal: &ContextCalibration) -> f32 {
    // No match (raw 0.0), orthogonal, or opposed: no context evidence at all.
    // The sigmoid alone would hand a zero-vector item a non-zero axis.
    if !cosine.is_finite() || cosine <= 0.0 {
        return 0.0;
    }
    if cosine >= 1.0 {
        return 1.0;
    }
    1.0 / (1.0 + ((cal.center - cosine) * cal.scale).exp())
}

/// Semantic ACE boost for a top-k mean stack similarity.
pub(crate) fn semantic_boost(top_k_mean: f32, cal: &SemanticCalibration) -> f32 {
    ((top_k_mean - cal.pivot) * cal.gain).clamp(SEMANTIC_BOOST_MIN, SEMANTIC_BOOST_MAX)
}

/// Mean of the `k` largest values (all of them when fewer than `k`).
pub(crate) fn top_k_mean(sims: &mut Vec<f32>, k: usize) -> Option<f32> {
    if sims.is_empty() || k == 0 {
        return None;
    }
    sims.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let take = k.min(sims.len());
    Some(sims[..take].iter().sum::<f32>() / take as f32)
}

/// Top-k mean similarity of `item` against every stack embedding — the ONE
/// definition shared by the per-item boost and the corpus sample it is
/// calibrated against (they must agree or the percentiles mean nothing).
pub(crate) fn stack_top_k_mean(
    item: &[f32],
    item_norm: f32,
    stack_embeddings: &[&Vec<f32>],
) -> Option<f32> {
    if item_norm < f32::EPSILON {
        return None;
    }
    let mut sims: Vec<f32> = stack_embeddings
        .iter()
        .filter(|e| crate::vector_norm(e) >= f32::EPSILON)
        .map(|e| crate::cosine_similarity_with_norm(item, item_norm, e))
        .collect();
    top_k_mean(&mut sims, SEMANTIC_TOP_K)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(n: usize, lo: f32, hi: f32) -> Vec<f32> {
        (0..n)
            .map(|i| lo + (hi - lo) * (i as f32 / (n.max(2) - 1) as f32))
            .collect()
    }

    #[test]
    fn l2_to_cosine_is_exact_for_unit_vectors() {
        assert!((l2_to_cosine(0.0) - 1.0).abs() < 1e-6);
        assert!((l2_to_cosine(2.0_f32.sqrt()) - 0.0).abs() < 1e-6);
        assert!((l2_to_cosine(2.0) + 1.0).abs() < 1e-6);
        // Live shape: rank-0 distances cluster at 0.8–1.0 → cosine 0.68–0.50.
        assert!((l2_to_cosine(0.8) - 0.68).abs() < 1e-6);
        assert!((l2_to_cosine(1.0) - 0.5).abs() < 1e-6);
        // Non-unit legacy blob (live outlier at 19.4) clamps, never invents.
        assert_eq!(l2_to_cosine(19.4), -1.0);
        assert_eq!(l2_to_cosine(f32::NAN), -1.0);
    }

    #[test]
    fn context_fit_lands_the_percentiles_on_their_targets() {
        let mut cosines = sample(1000, 0.40, 0.80);
        let cal = context_from_top1_cosines(&mut cosines).expect("enough sample");
        assert_eq!(cal.source, CalibrationSource::Corpus);
        let mut sorted = sample(1000, 0.40, 0.80);
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let c75 = percentile(&sorted, 0.75);
        let c95 = percentile(&sorted, 0.95);
        assert!((calibrate_context(c75, &cal) - CONTEXT_CONFIRM_TARGET).abs() < 0.01);
        assert!((calibrate_context(c95, &cal) - CONTEXT_STRONG_TARGET).abs() < 0.01);
        // The median of the corpus is NOT confirmed any more.
        let c50 = percentile(&sorted, 0.50);
        assert!(calibrate_context(c50, &cal) < CONTEXT_CONFIRM_TARGET);
    }

    #[test]
    fn context_fit_refuses_small_or_constant_samples() {
        let mut few = sample(10, 0.4, 0.8);
        assert!(context_from_top1_cosines(&mut few).is_none());
        let mut flat = vec![0.55_f32; 1000];
        assert!(context_from_top1_cosines(&mut flat).is_none());
    }

    #[test]
    fn semantic_fit_puts_the_confirm_percentile_exactly_on_the_threshold() {
        // Live-shaped: corpus top-3 means p50 0.530, p90 0.594.
        let mut means = sample(2000, 0.44, 0.62);
        let cal = semantic_from_top_k_means(&mut means).expect("enough sample");
        let mut sorted = sample(2000, 0.44, 0.62);
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p50 = percentile(&sorted, 0.50);
        let p90 = percentile(&sorted, 0.90);
        assert!(semantic_boost(p50, &cal).abs() < 1e-4);
        assert!((semantic_boost(p90, &cal) - scoring_config::SEMANTIC_THRESHOLD).abs() < 1e-3);
        // Feed-shaped item (p50 0.603 live) clears the threshold; corpus
        // median does not.
        assert!(semantic_boost(0.603, &cal) > scoring_config::SEMANTIC_THRESHOLD);
        assert!(semantic_boost(0.530, &cal) < scoring_config::SEMANTIC_THRESHOLD);
    }

    #[test]
    fn semantic_boost_keeps_the_legacy_clamp() {
        let cal = SemanticCalibration::default();
        assert_eq!(semantic_boost(1.0, &cal), SEMANTIC_BOOST_MAX);
        assert_eq!(semantic_boost(0.0, &cal), SEMANTIC_BOOST_MIN);
        assert!((semantic_boost(0.5, &cal)).abs() < 1e-6);
    }

    #[test]
    fn top_k_mean_takes_the_largest_three() {
        let mut sims = vec![0.1, 0.9, 0.5, 0.8, 0.2];
        let m = top_k_mean(&mut sims, 3).unwrap();
        assert!((m - (0.9 + 0.8 + 0.5) / 3.0).abs() < 1e-6);
        let mut two = vec![0.4, 0.6];
        assert!((top_k_mean(&mut two, 3).unwrap() - 0.5).abs() < 1e-6);
        assert!(top_k_mean(&mut Vec::new(), 3).is_none());
    }

    #[test]
    fn default_context_calibration_is_the_model_sigmoid_on_cosine() {
        let cal = ContextCalibration::default();
        assert_eq!(cal.source, CalibrationSource::Fallback);
        // A cosine at the model center calibrates to exactly 0.5.
        assert!((calibrate_context(cal.center, &cal) - 0.5).abs() < 1e-5);
        assert_eq!(calibrate_context(-1.0, &cal), 0.0);
        assert_eq!(calibrate_context(1.0, &cal), 1.0);
    }
}
