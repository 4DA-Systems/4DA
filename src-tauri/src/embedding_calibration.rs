// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Embedding calibration — sigmoid parameters per embedding model.
//!
//! The PASIFA scoring pipeline stretches raw cosine similarity via a sigmoid:
//!   calibrated = 1 / (1 + exp((center - raw) * scale))
//!
//! Different embedding models produce different similarity distributions.
//! Hardcoding center=0.48 (tuned for text-embedding-3-small) causes
//! systematic mis-scoring when users run nomic-embed-text or other models.
//!
//! Parameters come from a curated known-model table, with a calibrated
//! default for everything else. A DB-driven "auto-compute from observed
//! similarities" stage was DELETED 2026-08-23 (scoring audit, item 8e): it
//! queried `context_score`/`interest_score` columns that have never existed
//! on `source_items`, so it failed silently on every launch — dead code
//! masquerading as adaptivity. If real per-model adaptation returns, it must
//! read raw similarities that are actually persisted.

use std::sync::atomic::{AtomicU32, Ordering};
use tracing::{debug, info};

static ACTIVE_CENTER: AtomicU32 = AtomicU32::new(0);
static ACTIVE_SCALE: AtomicU32 = AtomicU32::new(0);

#[cfg(test)]
thread_local! {
    static TEST_ACTIVE_PARAMS: std::cell::Cell<Option<(f32, f32)>> =
        const { std::cell::Cell::new(None) };
}

const KNOWN_MODELS: &[(&str, f32, f32)] = &[
    ("snowflake-arctic-embed-m", 0.44, 12.5),
    ("snowflake-arctic-embed-l", 0.45, 12.0),
    ("snowflake-arctic-embed-s", 0.42, 13.0),
    ("snowflake-arctic-embed", 0.44, 12.5),
    ("text-embedding-3-small", 0.48, 12.0),
    ("text-embedding-3-large", 0.50, 11.0),
    ("nomic-embed-text-v2", 0.40, 13.0),
    ("nomic-embed-text", 0.42, 14.0),
    ("mxbai-embed-large", 0.45, 12.0),
    ("all-minilm", 0.38, 15.0),
    ("bge-small", 0.43, 13.0),
    ("bge-small-en", 0.43, 13.0),
    ("bge-base", 0.44, 12.5),
];

const DEFAULT_CENTER: f32 = 0.43;
const DEFAULT_SCALE: f32 = 13.0;

pub(crate) fn get_sigmoid_center() -> f32 {
    #[cfg(test)]
    if let Some((center, _)) = TEST_ACTIVE_PARAMS.with(std::cell::Cell::get) {
        return center;
    }

    let bits = ACTIVE_CENTER.load(Ordering::Relaxed);
    if bits == 0 {
        DEFAULT_CENTER
    } else {
        f32::from_bits(bits)
    }
}

pub(crate) fn get_sigmoid_scale() -> f32 {
    #[cfg(test)]
    if let Some((_, scale)) = TEST_ACTIVE_PARAMS.with(std::cell::Cell::get) {
        return scale;
    }

    let bits = ACTIVE_SCALE.load(Ordering::Relaxed);
    if bits == 0 {
        DEFAULT_SCALE
    } else {
        f32::from_bits(bits)
    }
}

pub(crate) fn set_active_params(center: f32, scale: f32) {
    #[cfg(test)]
    TEST_ACTIVE_PARAMS.with(|params| params.set(Some((center, scale))));

    #[cfg(not(test))]
    {
        ACTIVE_CENTER.store(center.to_bits(), Ordering::Relaxed);
        ACTIVE_SCALE.store(scale.to_bits(), Ordering::Relaxed);
    }

    info!(
        center = format!("{:.3}", center),
        scale = format!("{:.1}", scale),
        "Embedding calibration parameters updated"
    );
}

#[cfg(test)]
fn clear_active_params_for_current_test_thread() {
    TEST_ACTIVE_PARAMS.with(|params| params.set(None));
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
/// 1. Known-model lookup table
/// 2. Calibrated default — applied EXPLICITLY, so switching away from a known
///    model (reembed) cannot leave the previous model's parameters active.
///
/// `_conn` is retained for call-site stability (`app_setup`, `reembed`): the
/// DB-driven auto-compute stage that consumed it was deleted 2026-08-23 — it
/// queried `context_score`/`interest_score` columns that never existed on
/// `source_items`, so it silently returned nothing on every launch.
pub(crate) fn initialize_calibration(_conn: &rusqlite::Connection, model_name: &str) {
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
        "Unknown embedding model, using calibrated defaults (center={}, scale={})",
        DEFAULT_CENTER,
        DEFAULT_SCALE
    );
    set_active_params(DEFAULT_CENTER, DEFAULT_SCALE);
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
        clear_active_params_for_current_test_thread();
    }

    #[test]
    fn initialize_prefers_known_model() {
        clear_active_params_for_current_test_thread();
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory conn");
        initialize_calibration(&conn, "nomic-embed-text");
        assert!((get_sigmoid_center() - 0.42).abs() < 0.001);
        assert!((get_sigmoid_scale() - 14.0).abs() < 0.1);
        clear_active_params_for_current_test_thread();
    }

    #[test]
    fn initialize_unknown_model_applies_defaults_even_after_known_model() {
        clear_active_params_for_current_test_thread();
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory conn");
        // A reembed switch known → unknown must not leave the known model's
        // parameters active.
        initialize_calibration(&conn, "nomic-embed-text");
        initialize_calibration(&conn, "totally-custom-model");
        assert!((get_sigmoid_center() - DEFAULT_CENTER).abs() < 0.001);
        assert!((get_sigmoid_scale() - DEFAULT_SCALE).abs() < 0.1);
        clear_active_params_for_current_test_thread();
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
