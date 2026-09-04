// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Fast-path analysis orchestration helpers.

use std::sync::atomic::Ordering;
use std::time::Instant;

use tauri::AppHandle;
use tracing::{info, warn};

#[derive(Clone, Copy, Debug)]
pub(super) struct CachedAnalysisRun {
    pub(super) silent: bool,
    pub(super) repair_pending_embeddings: bool,
    pub(super) drain_stale_backlog: bool,
    pub(super) llm_rerank: bool,
    pub(super) run_type: &'static str,
}

impl CachedAnalysisRun {
    pub(super) fn foreground_fast() -> Self {
        Self {
            silent: false,
            repair_pending_embeddings: false,
            drain_stale_backlog: false,
            llm_rerank: false,
            run_type: "foreground_fast",
        }
    }

    pub(super) fn background_deep() -> Self {
        Self {
            silent: true,
            repair_pending_embeddings: true,
            drain_stale_backlog: true,
            llm_rerank: true,
            run_type: "background_deep",
        }
    }
}

pub(super) fn elapsed_ms(started: Instant) -> u128 {
    started.elapsed().as_millis()
}

/// A finished cycle plus the boundary's verdict on whether its results were
/// LLM-judged. Kept beside the cycle rather than on `CycleResults` so the
/// judged flag is stamped exactly once, at `analyze_cached_content_inner`,
/// and lands in `AnalysisState::judged` from there.
pub(super) struct JudgedCycle {
    pub(super) cycle: super::analysis_cycle::CycleResults,
    /// See [`judged_for_run`].
    pub(super) judged: bool,
}

/// Did THIS run's results receive an applied LLM judge pass?
///
/// `passes_before` / `passes_after` are `analysis_rerank::applied_rerank_passes()`
/// snapshots taken around the cycle. The `llm_rerank` guard matters: in the
/// GUI process only scheduled (`background_deep`) runs ever rerank, so a
/// foreground fast pass that happens to overlap a scheduled rerank must not
/// inherit that pass's counter tick and call its own pipeline-only results
/// "judged". Conservative by design — this can under-report (a cache-empty
/// deep-scan fallback reranks even on a fast run) but never over-reports.
pub(super) fn judged_for_run(llm_rerank: bool, passes_before: u64, passes_after: u64) -> bool {
    llm_rerank && passes_after > passes_before
}

#[cfg(test)]
mod judged_tests {
    use super::judged_for_run;

    #[test]
    fn fast_pass_is_never_judged_even_if_a_pass_applied_meanwhile() {
        assert!(!judged_for_run(false, 3, 4));
        assert!(!judged_for_run(false, 3, 3));
    }

    #[test]
    fn deep_run_is_judged_only_when_a_pass_actually_applied() {
        assert!(judged_for_run(true, 3, 4));
        assert!(
            !judged_for_run(true, 3, 3),
            "every Skipped reason stays unjudged"
        );
    }
}

pub(super) fn spawn_post_foreground_cache_fill(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let monitoring = crate::get_monitoring_state().clone();
        if monitoring.is_checking.swap(true, Ordering::SeqCst) {
            info!(
                target: "4da::analysis",
                "Post-analysis source refresh skipped — another background check is already running"
            );
            return;
        }

        let started = Instant::now();
        info!(target: "4da::analysis", "Refreshing sources after foreground cache-first analysis");
        let result = tokio::time::timeout(
            std::time::Duration::from_mins(2),
            crate::source_fetching::fill_cache_background(&app),
        )
        .await;

        match result {
            Ok(Ok(summary)) => {
                info!(
                    target: "4da::analysis",
                    elapsed_ms = elapsed_ms(started),
                    succeeded = summary.succeeded,
                    failed = summary.failed,
                    skipped = summary.skipped_disabled,
                    new_items = summary.new_items,
                    cached_touches = summary.cached_touches,
                    "Post-analysis source refresh complete"
                );
            }
            Ok(Err(e)) => {
                warn!(
                    target: "4da::analysis",
                    elapsed_ms = elapsed_ms(started),
                    error = %e,
                    "Post-analysis source refresh failed"
                );
            }
            Err(_) => {
                warn!(
                    target: "4da::analysis",
                    elapsed_ms = elapsed_ms(started),
                    "Post-analysis source refresh timed out"
                );
            }
        }

        monitoring.is_checking.store(false, Ordering::SeqCst);
    });
}
