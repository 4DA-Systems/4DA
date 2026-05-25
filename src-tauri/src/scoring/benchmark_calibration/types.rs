// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Types and utilities for benchmark calibration.

use super::BenchmarkReport;

#[derive(Clone)]
pub(crate) struct CalibrationResult {
    pub model_name: String,
    pub original_accuracy: f32,
    pub original_params: (f32, f32),
    pub optimized_accuracy: f32,
    pub optimized_params: (f32, f32),
    pub benchmark_report: BenchmarkReport,
    pub meets_quality_gate: bool,
}

/// Pad fastembed vectors to EMBEDDING_DIMS with zeros, then L2-normalize.
/// Mirrors the truncate_and_normalize step that embed_texts() applies in production.
pub(super) fn pad_and_normalize(mut v: Vec<f32>) -> Vec<f32> {
    let target = crate::EMBEDDING_DIMS;
    if v.len() < target {
        v.resize(target, 0.0);
    } else if v.len() > target {
        v.truncate(target);
    }
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}
