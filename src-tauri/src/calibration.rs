//! Intelligence Mesh — Layer 4 (Calibration Manifolds).
//!
//! LLM confidence is not probability. A model that reports 0.8 confidence
//! on its "relevant" judgments is only meaningful if, empirically, 80% of
//! those judgments turn out to be relevant. The pre-mesh pipeline silently
//! assumed that equivalence. The mesh treats it as a hypothesis to verify
//! per model, per task, per prompt version.
//!
//! A `CalibrationCurve` is the learned mapping from raw model confidence
//! to observed positive rate, produced by running a model against a
//! gold-labeled sample and bucketing by predicted confidence. A
//! `CalibratedCore` wraps any `IntelligenceCore` and applies its curve
//! transparently — callers ask for a judgment, get back normalized
//! scores in a stable scale where 0.8 really means "80% of items at
//! this confidence were actually relevant".
//!
//! Two metrics summarize curve quality:
//!   - Brier score: mean squared error between predicted probability
//!     and observed outcome. Lower is better. A perfectly calibrated
//!     uniform-random model scores 0.25.
//!   - Expected Calibration Error (ECE): weighted absolute gap between
//!     predicted and observed per bucket. Closer to 0 is better.
//!
//! ## Phase 5a scope
//!
//! This module ships the data model, the apply function, the scoring
//! utilities, and the CalibratedCore wrapper. It does NOT ship:
//!   - A calibration runner (the tool that produces real curves from a
//!     gold dataset). That is Phase 5b — it needs hours of LLM runs
//!     and a curated gold set of ~50-100 hand-labeled items per task.
//!   - Persistence to `~/4DA/calibrations/{model_hash}/{task}.json`.
//!     Curves travel in memory for now; storage lands alongside the
//!     runner so we persist something real.
//!   - Wiring into `analysis_rerank.rs` — currently the rerank loop
//!     constructs `LlmJudgeCore` directly. Wrapping with `CalibratedCore`
//!     is a one-line change deferred until we have a curve to apply.
//!
//! Once Phase 5b lands, every "uncalibrated" tag in the Phase 7b
//! receipts panel becomes a real curve ID — the final piece of the
//! receipts story.
//!
//! See `docs/strategy/INTELLIGENCE-MESH.md` §2 Layer 4.

use crate::error::Result;
use crate::intelligence_core::{IntelligenceCore, JudgeRequest, JudgeResponse, Validated};
use crate::provenance::ModelIdentity;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One bucket of a calibration curve: "when the model predicted between
/// `raw_bucket_lo` and `raw_bucket_hi`, the actual positive rate was
/// `observed_positive_rate` across `sample_count` observations."
///
/// `raw_bucket_center` is stored redundantly for O(1) lookup during
/// interpolation rather than recomputing from lo/hi each time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationBucket {
    pub raw_bucket_lo: f32,
    pub raw_bucket_hi: f32,
    pub raw_bucket_center: f32,
    pub observed_positive_rate: f32,
    pub sample_count: u32,
}

/// A learned mapping from raw model confidence to calibrated probability.
///
/// Applied via piecewise-linear interpolation between bucket centers.
/// A curve with fewer than 2 buckets is useless — apply falls through
/// to the raw score, and the consumer should treat `calibration_id` as
/// unreliable. We accept-construct such degenerate curves because
/// the Phase 5b runner may produce them from very small gold sets;
/// failing hard there would block the UX of "here is a weak curve"
/// vs. "here is no curve."
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationCurve {
    /// Stable ID for this curve. Stamped onto every `Validated<_>` and
    /// every provenance row so post-hoc analysis can filter by cohort.
    /// Format: `{task}-{model_short}-cal-v{n}-{YYYY-MM-DD}`.
    pub curve_id: String,

    /// Hex-encoded SHA-256 hash of the `ModelIdentity` this curve was
    /// fit against. Two models with the same name served from different
    /// base_urls get distinct curves — correct, because they are different
    /// runtime entities.
    pub model_identity_hash: String,

    /// Task name the curve was fit for: "judge", "summarize", etc.
    /// A judge curve is NOT valid for summarize outputs.
    pub task: String,

    /// Prompt template version at fit time. Changing the prompt
    /// invalidates the curve; consumers must re-calibrate when
    /// `IntelligenceCore::prompt_version()` differs from this value.
    pub prompt_version: String,

    /// Ordered buckets covering [0.0, 1.0]. Must be non-overlapping and
    /// monotonically increasing by `raw_bucket_center`. The apply method
    /// assumes this invariant; the fitter is responsible for producing it.
    pub buckets: Vec<CalibrationBucket>,

    /// Brier score on the fit sample. Lower is better; 0.0 = perfect.
    pub brier_score: f32,

    /// Expected Calibration Error on the fit sample. Closer to 0 is better.
    pub ece: f32,

    /// Total sample count across all buckets.
    pub sample_count: u32,

    pub created_at: DateTime<Utc>,
}

impl CalibrationCurve {
    /// Apply the curve to a raw model confidence.
    ///
    /// Uses piecewise-linear interpolation between adjacent bucket centers.
    /// Below the first bucket: anchors to the first bucket's observed rate.
    /// Above the last bucket: anchors to the last bucket's observed rate.
    /// This is a deliberate choice over linear extrapolation — extrapolating
    /// calibration curves past the observed range produces nonsense
    /// (negative probabilities, rates > 1.0).
    ///
    /// Degenerate curves (fewer than 2 buckets) return the raw score
    /// unchanged. Callers with hard correctness requirements should check
    /// `buckets.len() >= 2` before trusting the output.
    pub fn apply(&self, raw_score: f32) -> f32 {
        if self.buckets.len() < 2 {
            return raw_score.clamp(0.0, 1.0);
        }

        // Clamp input to bucket-center range; anchor outside the range.
        let first = &self.buckets[0];
        let last = &self.buckets[self.buckets.len() - 1];
        if raw_score <= first.raw_bucket_center {
            return first.observed_positive_rate.clamp(0.0, 1.0);
        }
        if raw_score >= last.raw_bucket_center {
            return last.observed_positive_rate.clamp(0.0, 1.0);
        }

        // Find the bracketing buckets and interpolate.
        for pair in self.buckets.windows(2) {
            let (lo, hi) = (&pair[0], &pair[1]);
            if raw_score >= lo.raw_bucket_center && raw_score <= hi.raw_bucket_center {
                let span = hi.raw_bucket_center - lo.raw_bucket_center;
                if span <= 1e-6 {
                    return lo.observed_positive_rate.clamp(0.0, 1.0);
                }
                let t = (raw_score - lo.raw_bucket_center) / span;
                let interpolated = lo.observed_positive_rate
                    + t * (hi.observed_positive_rate - lo.observed_positive_rate);
                return interpolated.clamp(0.0, 1.0);
            }
        }

        // Unreachable under the invariant, but defensive: fall through to raw.
        raw_score.clamp(0.0, 1.0)
    }
}

/// Brier score: mean squared error between predicted probability and
/// observed outcome (0.0 or 1.0). Lower is better; 0.0 is perfect;
/// a uniform-random predictor scores 0.25.
///
/// Returns 0.0 for an empty input — degenerate, but the alternative is
/// a panic; callers should check `predictions.is_empty()` if they
/// need to distinguish "no data" from "perfect data".
pub fn compute_brier_score(predictions: &[(f32, bool)]) -> f32 {
    if predictions.is_empty() {
        return 0.0;
    }
    let sum: f32 = predictions
        .iter()
        .map(|&(pred, actual)| {
            let actual_f = if actual { 1.0 } else { 0.0 };
            (pred - actual_f).powi(2)
        })
        .sum();
    sum / predictions.len() as f32
}

/// Expected Calibration Error: weighted absolute gap between predicted
/// probability and observed positive rate, bucketed by predicted value.
///
/// `n_bins` must be >= 1. The default in literature is 10 equal-width bins.
/// Empty buckets contribute 0 to the sum (weight 0).
pub fn compute_ece(predictions: &[(f32, bool)], n_bins: usize) -> f32 {
    if predictions.is_empty() || n_bins == 0 {
        return 0.0;
    }
    let bin_width = 1.0 / n_bins as f32;
    let total = predictions.len() as f32;
    let mut ece = 0.0_f32;

    for bin_idx in 0..n_bins {
        let lo = bin_idx as f32 * bin_width;
        let hi = lo + bin_width;
        // Closed on lo, open on hi, except last bin closes on 1.0 inclusive.
        let is_last = bin_idx == n_bins - 1;
        let in_bucket: Vec<&(f32, bool)> = predictions
            .iter()
            .filter(|&&(pred, _)| {
                let p = pred.clamp(0.0, 1.0);
                p >= lo && (if is_last { p <= hi } else { p < hi })
            })
            .collect();
        if in_bucket.is_empty() {
            continue;
        }
        let count = in_bucket.len() as f32;
        let mean_pred: f32 = in_bucket
            .iter()
            .map(|&&(p, _)| p.clamp(0.0, 1.0))
            .sum::<f32>()
            / count;
        let observed_rate: f32 = in_bucket.iter().filter(|(_, a)| *a).count() as f32 / count;
        ece += (count / total) * (mean_pred - observed_rate).abs();
    }
    ece
}

/// Wraps any `IntelligenceCore` and applies a calibration curve to its
/// `judge` outputs. When the curve is `None`, behavior is identical to
/// the inner core — which is deliberately what we want during the
/// transition period where some model/task pairs are calibrated and
/// others aren't yet.
pub struct CalibratedCore {
    inner: Box<dyn IntelligenceCore>,
    curve: Option<CalibrationCurve>,
}

impl CalibratedCore {
    pub fn new(inner: Box<dyn IntelligenceCore>, curve: Option<CalibrationCurve>) -> Self {
        Self { inner, curve }
    }
}

#[async_trait]
impl IntelligenceCore for CalibratedCore {
    fn identity(&self) -> ModelIdentity {
        self.inner.identity()
    }

    fn prompt_version(&self) -> &'static str {
        self.inner.prompt_version()
    }

    fn calibration_id(&self) -> Option<String> {
        // When a curve is present, its ID wins over whatever sentinel the
        // inner core returned. This is load-bearing: the whole point of
        // wrapping is to replace the pre-mesh sentinel with a real curve ID.
        match &self.curve {
            Some(c) => Some(c.curve_id.clone()),
            None => self.inner.calibration_id(),
        }
    }

    async fn judge(&self, req: JudgeRequest) -> Result<Validated<JudgeResponse>> {
        let mut validated = self.inner.judge(req).await?;

        if let Some(curve) = &self.curve {
            for judgment in validated.value.judgments.iter_mut() {
                judgment.confidence = curve.apply(judgment.confidence);
            }
            // Re-stamp the wrapper's calibration_id on the returned Validated
            // so receipts show the curve, not the inner core's sentinel.
            validated.calibration_id = Some(curve.curve_id.clone());
        }

        Ok(validated)
    }

    fn estimate_cost_cents(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        self.inner.estimate_cost_cents(input_tokens, output_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::RelevanceJudgment;

    fn bucket(lo: f32, hi: f32, observed: f32, n: u32) -> CalibrationBucket {
        CalibrationBucket {
            raw_bucket_lo: lo,
            raw_bucket_hi: hi,
            raw_bucket_center: (lo + hi) / 2.0,
            observed_positive_rate: observed,
            sample_count: n,
        }
    }

    fn sample_curve() -> CalibrationCurve {
        // A deliberately non-identity curve: an over-confident model
        // that rates everything above 0.7 but is actually right only
        // 50% of the time above 0.7. Below 0.3 it's under-confident;
        // actually right 20% of the time. Between, roughly calibrated.
        CalibrationCurve {
            curve_id: "judge-test-cal-v1-2026-04-15".to_string(),
            model_identity_hash: "deadbeef".to_string(),
            task: "judge".to_string(),
            prompt_version: "judge-v1-2026-04-15".to_string(),
            buckets: vec![
                bucket(0.0, 0.2, 0.05, 20),
                bucket(0.2, 0.4, 0.20, 25),
                bucket(0.4, 0.6, 0.45, 30),
                bucket(0.6, 0.8, 0.50, 25),
                bucket(0.8, 1.0, 0.50, 20),
            ],
            brier_score: 0.18,
            ece: 0.12,
            sample_count: 120,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn apply_on_empty_curve_returns_raw() {
        let curve = CalibrationCurve {
            curve_id: "empty".into(),
            model_identity_hash: "x".into(),
            task: "judge".into(),
            prompt_version: "v1".into(),
            buckets: vec![],
            brier_score: 0.0,
            ece: 0.0,
            sample_count: 0,
            created_at: Utc::now(),
        };
        assert!((curve.apply(0.73) - 0.73).abs() < 1e-5);
    }

    #[test]
    fn apply_single_bucket_falls_through_to_raw() {
        // With one bucket we can't interpolate, so the curve degrades to
        // identity rather than anchoring everything to one value.
        let curve = CalibrationCurve {
            curve_id: "one".into(),
            model_identity_hash: "x".into(),
            task: "judge".into(),
            prompt_version: "v1".into(),
            buckets: vec![bucket(0.4, 0.6, 0.7, 10)],
            brier_score: 0.0,
            ece: 0.0,
            sample_count: 10,
            created_at: Utc::now(),
        };
        assert!((curve.apply(0.5) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn apply_below_range_anchors_to_first_bucket() {
        let curve = sample_curve();
        // First bucket center is 0.1, observed 0.05.
        assert!((curve.apply(0.0) - 0.05).abs() < 1e-5);
        assert!((curve.apply(0.05) - 0.05).abs() < 1e-5);
    }

    #[test]
    fn apply_above_range_anchors_to_last_bucket() {
        let curve = sample_curve();
        // Last bucket center is 0.9, observed 0.50.
        assert!((curve.apply(1.0) - 0.50).abs() < 1e-5);
        assert!((curve.apply(0.95) - 0.50).abs() < 1e-5);
    }

    #[test]
    fn apply_interpolates_linearly_between_centers() {
        let curve = sample_curve();
        // Between bucket centers 0.5 (observed 0.45) and 0.7 (observed 0.50).
        // At raw_score = 0.6, t = 0.5, expect 0.475.
        let out = curve.apply(0.6);
        assert!((out - 0.475).abs() < 1e-4, "expected ~0.475, got {out}");
    }

    #[test]
    fn apply_at_bucket_center_returns_observed_rate() {
        let curve = sample_curve();
        assert!((curve.apply(0.5) - 0.45).abs() < 1e-5);
        assert!((curve.apply(0.7) - 0.50).abs() < 1e-5);
        assert!((curve.apply(0.3) - 0.20).abs() < 1e-5);
    }

    #[test]
    fn apply_clamps_output_to_probability_range() {
        // A pathological bucket with observed_positive_rate > 1.0 (shouldn't
        // happen in practice but guards against bad fitter output).
        let curve = CalibrationCurve {
            curve_id: "bad".into(),
            model_identity_hash: "x".into(),
            task: "judge".into(),
            prompt_version: "v1".into(),
            buckets: vec![bucket(0.0, 0.5, 1.5, 10), bucket(0.5, 1.0, -0.2, 10)],
            brier_score: 0.0,
            ece: 0.0,
            sample_count: 20,
            created_at: Utc::now(),
        };
        assert!(curve.apply(0.25) <= 1.0);
        assert!(curve.apply(0.75) >= 0.0);
    }

    #[test]
    fn brier_score_perfect_predictor_scores_zero() {
        let preds = vec![(1.0, true), (0.0, false), (1.0, true), (0.0, false)];
        assert!(compute_brier_score(&preds).abs() < 1e-6);
    }

    #[test]
    fn brier_score_worst_predictor_scores_one() {
        let preds = vec![(1.0, false), (0.0, true)];
        assert!((compute_brier_score(&preds) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn brier_score_uniform_random_is_quarter() {
        // A model that always predicts 0.5 on a 50/50 dataset scores 0.25.
        let preds = vec![(0.5, true), (0.5, false), (0.5, true), (0.5, false)];
        assert!((compute_brier_score(&preds) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn brier_score_empty_input_returns_zero() {
        assert!(compute_brier_score(&[]).abs() < 1e-6);
    }

    #[test]
    fn ece_perfect_calibration_scores_zero() {
        // Every prediction bucket matches its observed rate exactly.
        let preds = vec![(0.9, true), (0.9, true), (0.9, true), (0.9, false)];
        // At bin 9 (0.9-1.0): mean_pred = 0.9, observed_rate = 0.75.
        // Gap = 0.15. Weight = 1.0. ECE = 0.15.
        let ece = compute_ece(&preds, 10);
        assert!((ece - 0.15).abs() < 1e-4, "got {ece}");
    }

    #[test]
    fn ece_empty_input_returns_zero() {
        assert!(compute_ece(&[], 10).abs() < 1e-6);
    }

    #[test]
    fn ece_zero_bins_returns_zero() {
        let preds = vec![(0.5, true)];
        assert!(compute_ece(&preds, 0).abs() < 1e-6);
    }

    // ------ CalibratedCore tests ---------------------------------------

    /// Minimal IntelligenceCore for testing: returns a fixed judgment list
    /// and lets the test set every provenance field.
    struct StubCore {
        identity: ModelIdentity,
        prompt_version: &'static str,
        calibration_id: Option<String>,
        canned: Vec<RelevanceJudgment>,
    }

    #[async_trait]
    impl IntelligenceCore for StubCore {
        fn identity(&self) -> ModelIdentity {
            self.identity.clone()
        }
        fn prompt_version(&self) -> &'static str {
            self.prompt_version
        }
        fn calibration_id(&self) -> Option<String> {
            self.calibration_id.clone()
        }
        async fn judge(&self, _req: JudgeRequest) -> Result<Validated<JudgeResponse>> {
            Ok(Validated {
                value: JudgeResponse {
                    judgments: self.canned.clone(),
                    input_tokens: 10,
                    output_tokens: 5,
                },
                identity: self.identity.clone(),
                prompt_version: self.prompt_version.to_string(),
                calibration_id: self.calibration_id.clone(),
                raw_response_hash: None,
            })
        }
        fn estimate_cost_cents(&self, _i: u64, _o: u64) -> u64 {
            42
        }
    }

    fn stub_with_confidence(c: f32) -> StubCore {
        StubCore {
            identity: ModelIdentity::new("ollama", "llama3.2"),
            prompt_version: "judge-v1-stub",
            calibration_id: Some("pre-mesh-unknown".to_string()),
            canned: vec![RelevanceJudgment {
                item_id: "a".to_string(),
                relevant: true,
                confidence: c,
                reasoning: "because".to_string(),
                key_connections: vec![],
            }],
        }
    }

    fn req() -> JudgeRequest {
        JudgeRequest {
            context_summary: "x".to_string(),
            items: vec![("a".to_string(), "t".to_string(), "c".to_string())],
        }
    }

    #[tokio::test]
    async fn calibrated_core_with_no_curve_passes_through() {
        let inner = Box::new(stub_with_confidence(0.73));
        let wrapped = CalibratedCore::new(inner, None);
        let v = wrapped.judge(req()).await.unwrap();
        assert!((v.value.judgments[0].confidence - 0.73).abs() < 1e-5);
        // calibration_id falls through to inner's value.
        assert_eq!(v.calibration_id.as_deref(), Some("pre-mesh-unknown"));
    }

    #[tokio::test]
    async fn calibrated_core_applies_curve_to_each_judgment() {
        // Curve maps 0.9 → 0.50 (over-confident model).
        let inner = Box::new(stub_with_confidence(0.9));
        let wrapped = CalibratedCore::new(inner, Some(sample_curve()));
        let v = wrapped.judge(req()).await.unwrap();
        assert!(
            (v.value.judgments[0].confidence - 0.50).abs() < 1e-4,
            "expected ~0.50, got {}",
            v.value.judgments[0].confidence
        );
    }

    #[tokio::test]
    async fn calibrated_core_overrides_calibration_id() {
        let inner = Box::new(stub_with_confidence(0.5));
        let wrapped = CalibratedCore::new(inner, Some(sample_curve()));
        let v = wrapped.judge(req()).await.unwrap();
        assert_eq!(
            v.calibration_id.as_deref(),
            Some("judge-test-cal-v1-2026-04-15")
        );
    }

    #[test]
    fn calibrated_core_calibration_id_method_reflects_curve() {
        let inner = Box::new(stub_with_confidence(0.5));
        let wrapped = CalibratedCore::new(inner, Some(sample_curve()));
        assert_eq!(
            wrapped.calibration_id().as_deref(),
            Some("judge-test-cal-v1-2026-04-15")
        );

        // With None curve, inner's sentinel bubbles up.
        let inner_b = Box::new(stub_with_confidence(0.5));
        let wrapped_b = CalibratedCore::new(inner_b, None);
        assert_eq!(
            wrapped_b.calibration_id().as_deref(),
            Some("pre-mesh-unknown")
        );
    }

    #[tokio::test]
    async fn calibrated_core_delegates_identity_and_prompt_version() {
        let inner = Box::new(stub_with_confidence(0.5));
        let wrapped = CalibratedCore::new(inner, Some(sample_curve()));
        assert_eq!(wrapped.identity().provider, "ollama");
        assert_eq!(wrapped.prompt_version(), "judge-v1-stub");
        assert_eq!(wrapped.estimate_cost_cents(100, 50), 42);
    }

    #[test]
    fn curve_serde_roundtrip_preserves_all_fields() {
        let original = sample_curve();
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: CalibrationCurve = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original.curve_id, restored.curve_id);
        assert_eq!(original.model_identity_hash, restored.model_identity_hash);
        assert_eq!(original.task, restored.task);
        assert_eq!(original.prompt_version, restored.prompt_version);
        assert_eq!(original.buckets.len(), restored.buckets.len());
        assert!((original.brier_score - restored.brier_score).abs() < 1e-6);
        assert!((original.ece - restored.ece).abs() < 1e-6);
        assert_eq!(original.sample_count, restored.sample_count);
    }
}
