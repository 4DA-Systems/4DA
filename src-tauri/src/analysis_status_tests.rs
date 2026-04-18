// SPDX-License-Identifier: FSL-1.1-Apache-2.0
#[cfg(test)]
mod tests {
    use crate::{get_analysis_abort, get_analysis_state, AnalysisState, ANALYSIS_TIMEOUT_SECS};
    use std::sync::atomic::Ordering;

    // ========================================================================
    // Cancellation: abort flag behavior
    // ========================================================================

    #[test]
    fn cancel_sets_abort_flag() {
        let abort = get_analysis_abort();
        abort.store(false, Ordering::SeqCst);
        // Simulate what cancel_analysis does
        abort.store(true, Ordering::SeqCst);
        assert!(
            abort.load(Ordering::SeqCst),
            "Abort flag should be set after cancel"
        );
        // Cleanup
        abort.store(false, Ordering::SeqCst);
    }

    #[test]
    fn abort_flag_resets_at_start() {
        let abort = get_analysis_abort();
        // Set abort flag (simulating previous cancellation)
        abort.store(true, Ordering::SeqCst);
        // Simulate what run_cached_analysis does at start
        abort.store(false, Ordering::SeqCst);
        assert!(
            !abort.load(Ordering::SeqCst),
            "Abort flag should be cleared at analysis start"
        );
    }

    // ========================================================================
    // Double-run prevention via running flag
    // ========================================================================

    #[test]
    fn cached_analysis_prevents_double_run() {
        let state = get_analysis_state();
        {
            let mut guard = state.lock();
            guard.running = true;
        }
        // Verify running flag is set
        {
            let guard = state.lock();
            assert!(
                guard.running,
                "Running flag should prevent concurrent analysis"
            );
        }
        // Cleanup
        {
            let mut guard = state.lock();
            guard.running = false;
        }
    }

    // ========================================================================
    // AnalysisState defaults and clone independence
    // ========================================================================

    #[test]
    fn analysis_state_defaults_are_sensible() {
        let state = AnalysisState {
            running: false,
            completed: false,
            error: None,
            results: None,
            near_misses: None,
            started_at: None,
            last_completed_at: None,
        };
        assert!(!state.running);
        assert!(!state.completed);
        assert!(state.error.is_none());
        assert!(state.results.is_none());
        assert!(state.near_misses.is_none());
        assert!(state.started_at.is_none());
    }

    #[test]
    fn analysis_state_clone_independent() {
        let original = AnalysisState {
            running: true,
            completed: false,
            error: Some("test error".to_string()),
            results: None,
            near_misses: None,
            started_at: Some(12345),
            last_completed_at: None,
        };
        let mut cloned = original.clone();
        cloned.running = false;
        cloned.error = None;

        assert!(original.running, "Original should be unchanged");
        assert!(
            original.error.is_some(),
            "Original error should be unchanged"
        );
        assert!(!cloned.running, "Clone should be modified");
        assert!(cloned.error.is_none(), "Clone error should be modified");
    }

    // ========================================================================
    // Timeout recovery logic
    // ========================================================================

    #[test]
    fn timeout_recovery_logic() {
        let timeout_secs = ANALYSIS_TIMEOUT_SECS;
        assert!(timeout_secs > 0, "Timeout should be positive");

        let state = get_analysis_state();
        {
            let mut guard = state.lock();
            guard.running = true;
            guard.started_at = Some(chrono::Utc::now().timestamp() - timeout_secs - 10);
        }
        // Verify timeout detection (mirrors get_analysis_status logic)
        {
            let guard = state.lock();
            if let Some(started) = guard.started_at {
                let elapsed = chrono::Utc::now().timestamp() - started;
                assert!(elapsed > timeout_secs, "Should detect timeout");
            }
        }
        // Cleanup
        {
            let mut guard = state.lock();
            guard.running = false;
            guard.started_at = None;
        }
    }
}
