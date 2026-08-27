// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Analysis functions extracted from lib.rs
//!
//! Contains: run_multi_source_analysis, run_cached_analysis, get_analysis_status,
//! get_actionable_signals, and their implementation helpers.
//!
//! Implementation split across sibling modules to stay under file-size limits.

use once_cell::sync::Lazy;
use std::sync::atomic::Ordering;

use crate::{get_analysis_abort, signals};

// Singleton SignalClassifier - created once and reused across all analyses
static SIGNAL_CLASSIFIER: Lazy<signals::SignalClassifier> =
    Lazy::new(signals::SignalClassifier::new);

/// Get a reference to the singleton SignalClassifier (used by analysis_rerank)
pub(crate) fn signal_classifier() -> &'static signals::SignalClassifier {
    &SIGNAL_CLASSIFIER
}

/// Check if analysis has been aborted by the user
#[inline]
fn is_aborted() -> bool {
    get_analysis_abort().load(Ordering::SeqCst)
}

// --- Sibling modules ---

#[path = "analysis_cycle.rs"]
pub(crate) mod analysis_cycle;
// Re-exported for the sibling test modules (`crate::analysis::persist_cycle_results`
// et al.); production callers reach these through `analysis_status`'s wrappers.
#[allow(unused_imports)]
pub(crate) use analysis_cycle::*;

#[path = "analysis_deep_scan.rs"]
mod analysis_deep_scan;

#[path = "analysis_fast_path.rs"]
mod analysis_fast_path;

#[path = "analysis_status.rs"]
mod analysis_status;
pub(crate) use analysis_status::*;

#[path = "analysis_tests.rs"]
mod analysis_tests;

#[path = "analysis_status_tests.rs"]
mod analysis_status_tests;

#[path = "analysis_deep_scan_tests.rs"]
mod analysis_deep_scan_tests;
