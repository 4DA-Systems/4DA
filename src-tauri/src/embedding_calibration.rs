// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Adaptive embedding calibration — auto-computes sigmoid parameters per model.
//!
//! The PASIFA scoring pipeline stretches raw cosine similarity via a sigmoid:
//!   calibrated = 1 / (1 + exp((center - raw) * scale))
//!
//! Different embedding models produce different similarity distributions.
//! Hardcoding center=0.48 (tuned for text-embedding-3-small) causes
//! systematic mis-scoring when users run nomic-embed-text or other models.
//!
//! This module auto-computes optimal parameters from observed data, with
//! known-good fallbacks for popular models.

use std::sync::atomic::{AtomicU32, Ordering};
use tracing::{debug, info, warn};

static ACTIVE_CENTER: AtomicU32 = AtomicU32::new(0);
static ACTIVE_SCALE: AtomicU32 = AtomicU32::new(0);

const KNOWN_MODELS: &[(&str, f32, f32)] = &[
    ("text-embedding-3-small", 0.48, 12.0),
    ("text-embedding-3-large", 0.50, 11.0),
    ("nomic-embed-text-v2", 0.40, 13.0),
    ("nomic-embed-text", 0.42, 14.0),
    ("mxbai-embed-large", 0.45, 12.0),
    ("all-minilm", 0.38, 15.0),
    ("bge-small", 0.43, 13.0),
    ("bge-base", 0.44, 12.5),
    ("snowflake-arctic-embed", 0.46, 12.0),
];

const DEFAULT_CENTER: f32 = 0.48;
const DEFAULT_SCALE: f32 = 12.0;
const MIN_SAMPLES_FOR_AUTO: usize = 50;

pub(crate) fn get_sigmoid_center() -> f32 {
    let bits = ACTIVE_CENTER.load(Ordering::Relaxed);
    if bits == 0 {
        DEFAULT_CENTER
    } else {
        f32::from_bits(bits)
    }
}

pub(crate) fn get_sigmoid_scale() -> f32 {
    let bits = ACTIVE_SCALE.load(Ordering::Relaxed);
    if bits == 0 {
        DEFAULT_SCALE
    } else {
        f32::from_bits(bits)
    }
}

fn set_active_params(center: f32, scale: f32) {
    ACTIVE_CENTER.store(center.to_bits(), Ordering::Relaxed);
    ACTIVE_SCALE.store(scale.to_bits(), Ordering::Relaxed);
    info!(
        center = format!("{:.3}", center),
        scale = format!("{:.1}", scale),
        "Embedding calibration parameters updated"
    );
}

pub(crate) fn lookup_known_model(model_name: &str) -> Option<(f32, f32)> {
    let lower = model_name.to_lowercase();
    KNOWN_MODELS
        .iter()
        .find(|(prefix, _, _)| lower.starts_with(prefix))
        .map(|(_, center, scale)| (*center, *scale))
}

/// Initialize calibration for the current embedding model.
///
/// Priority:
/// 1. Auto-compute from observed similarity distribution (most accurate)
/// 2. Known-model lookup table
/// 3. Default (text-embedding-3-small parameters)
pub(crate) fn initialize_calibration(conn: &rusqlite::Connection, model_name: &str) {
    if let Some((center, scale)) = auto_compute_from_db(conn) {
        info!(
            model = model_name,
            center = format!("{:.3}", center),
            scale = format!("{:.1}", scale),
            "Using auto-computed embedding calibration"
        );
        set_active_params(center, scale);
        return;
    }

    if let Some((center, scale)) = lookup_known_model(model_name) {
        info!(
            model = model_name,
            center = format!("{:.3}", center),
            scale = format!("{:.1}", scale),
            "Using known-model embedding calibration"
        );
        set_active_params(center, scale);
        return;
    }

    debug!(
        model = model_name,
        "No calibration data, using defaults (center={}, scale={})", DEFAULT_CENTER, DEFAULT_SCALE
    );
}

/// Auto-compute sigmoid parameters from observed cosine similarity distribution.
///
/// Samples raw similarities between source items, computes mean and stddev,
/// then derives: center = mean, scale = 2.5 / stddev.
fn auto_compute_from_db(conn: &rusqlite::Connection) -> Option<(f32, f32)> {
    let has_embeddings: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='source_vec'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false);

    if !has_embeddings {
        return None;
    }

    // Sample raw embedding similarity scores from scored items.
    // context_score and interest_score in source_items are the raw cosine values
    // BEFORE calibration — exactly what we need to characterise the distribution.
    let mut stmt = match conn.prepare(
        "SELECT context_score FROM source_items \
         WHERE context_score IS NOT NULL AND context_score > 0.01 \
         ORDER BY RANDOM() LIMIT 500",
    ) {
        Ok(s) => s,
        Err(e) => {
            debug!(error = %e, "Could not query for auto-calibration");
            return None;
        }
    };

    let similarities: Vec<f32> = match stmt.query_map([], |row| row.get::<_, f64>(0)) {
        Ok(rows) => rows.filter_map(|r| r.ok()).map(|v| v as f32).collect(),
        Err(e) => {
            debug!(error = %e, "Failed to collect calibration samples");
            return None;
        }
    };

    // Fall back to interest_score if no context scores yet
    let similarities = if similarities.len() < MIN_SAMPLES_FOR_AUTO {
        let mut stmt2 = match conn.prepare(
            "SELECT interest_score FROM source_items \
             WHERE interest_score IS NOT NULL AND interest_score > 0.01 \
             ORDER BY RANDOM() LIMIT 500",
        ) {
            Ok(s) => s,
            Err(_) => return None,
        };
        let fallback: Vec<f32> = match stmt2.query_map([], |row| row.get::<_, f64>(0)) {
            Ok(rows) => rows.filter_map(|r| r.ok()).map(|v| v as f32).collect(),
            Err(_) => return None,
        };
        fallback
    } else {
        similarities
    };

    if similarities.len() < MIN_SAMPLES_FOR_AUTO {
        debug!(
            samples = similarities.len(),
            min = MIN_SAMPLES_FOR_AUTO,
            "Insufficient samples for auto-calibration"
        );
        return None;
    }

    let n = similarities.len() as f32;
    let mean = similarities.iter().sum::<f32>() / n;
    let variance = similarities.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / n;
    let stddev = variance.sqrt();

    if stddev < 0.01 {
        warn!(
            mean = format!("{:.3}", mean),
            stddev = format!("{:.4}", stddev),
            "Embedding distribution too narrow for calibration"
        );
        return None;
    }

    let center = mean.clamp(0.20, 0.70);
    let scale = (2.5 / stddev).clamp(5.0, 30.0);

    info!(
        samples = similarities.len(),
        mean = format!("{:.3}", mean),
        stddev = format!("{:.4}", stddev),
        center = format!("{:.3}", center),
        scale = format!("{:.1}", scale),
        "Auto-computed embedding calibration"
    );

    Some((center, scale))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_model_lookup_exact() {
        let (c, s) = lookup_known_model("nomic-embed-text").unwrap();
        assert!((c - 0.42).abs() < 0.01);
        assert!((s - 14.0).abs() < 0.1);
    }

    #[test]
    fn known_model_lookup_prefix() {
        let (c, _) = lookup_known_model("nomic-embed-text-v2-moe").unwrap();
        assert!((c - 0.40).abs() < 0.01);
    }

    #[test]
    fn known_model_lookup_case_insensitive() {
        assert!(lookup_known_model("Nomic-Embed-Text").is_some());
    }

    #[test]
    fn known_model_lookup_unknown() {
        assert!(lookup_known_model("some-custom-model").is_none());
    }

    #[test]
    fn default_values_before_calibration() {
        // Before set_active_params, atomics are 0 → returns defaults
        let fresh_center = AtomicU32::new(0);
        let bits = fresh_center.load(Ordering::Relaxed);
        assert_eq!(bits, 0);
    }

    #[test]
    fn set_and_get_params() {
        set_active_params(0.42, 14.0);
        assert!((get_sigmoid_center() - 0.42).abs() < 0.001);
        assert!((get_sigmoid_scale() - 14.0).abs() < 0.1);
        // Reset
        ACTIVE_CENTER.store(0, Ordering::Relaxed);
        ACTIVE_SCALE.store(0, Ordering::Relaxed);
    }

    #[test]
    fn known_model_ordering_matters() {
        // nomic-embed-text-v2 must match before nomic-embed-text
        let (c, _) = lookup_known_model("nomic-embed-text-v2-moe").unwrap();
        assert!(
            (c - 0.40).abs() < 0.01,
            "v2 variant should match v2 entry, not base"
        );
    }
}
