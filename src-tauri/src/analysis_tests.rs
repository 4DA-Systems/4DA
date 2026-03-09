#[cfg(test)]
mod tests {
    use crate::analysis::is_aborted;
    use crate::{get_analysis_abort, AnalysisState, ANALYSIS_TIMEOUT_SECS};

    // ========================================================================
    // AnalysisState construction and defaults
    // ========================================================================

    #[test]
    fn test_analysis_state_default_values() {
        let state = AnalysisState {
            running: false,
            completed: false,
            error: None,
            results: None,
            started_at: None,
            last_completed_at: None,
        };
        assert!(!state.running);
        assert!(!state.completed);
        assert!(state.error.is_none());
        assert!(state.results.is_none());
        assert!(state.started_at.is_none());
        assert!(state.last_completed_at.is_none());
    }

    #[test]
    fn test_analysis_state_running_with_timestamp() {
        let now = chrono::Utc::now().timestamp();
        let state = AnalysisState {
            running: true,
            completed: false,
            error: None,
            results: None,
            started_at: Some(now),
            last_completed_at: None,
        };
        assert!(state.running);
        assert!(state.started_at.is_some());
        assert_eq!(state.started_at.unwrap(), now);
    }

    #[test]
    fn test_analysis_state_serialization_roundtrip() {
        let state = AnalysisState {
            running: true,
            completed: false,
            error: Some("test error".to_string()),
            results: None,
            started_at: Some(1700000000),
            last_completed_at: Some("2025-01-01 00:00:00".to_string()),
        };

        let json = serde_json::to_string(&state).expect("serialize");
        let deserialized: AnalysisState = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.running, state.running);
        assert_eq!(deserialized.completed, state.completed);
        assert_eq!(deserialized.error, state.error);
        assert_eq!(deserialized.started_at, state.started_at);
        assert_eq!(deserialized.last_completed_at, state.last_completed_at);
    }

    // ========================================================================
    // Timeout auto-recovery logic (pure computation)
    // ========================================================================

    #[test]
    fn test_analysis_timeout_constant() {
        assert_eq!(
            ANALYSIS_TIMEOUT_SECS, 300,
            "Timeout should be 300 seconds (5 minutes)"
        );
    }

    #[test]
    fn test_timeout_detection_logic() {
        // Simulate the timeout detection from get_analysis_status
        let started_at = chrono::Utc::now().timestamp() - 400; // Started 400s ago
        let elapsed = chrono::Utc::now().timestamp() - started_at;

        assert!(
            elapsed > ANALYSIS_TIMEOUT_SECS,
            "400s elapsed should exceed 300s timeout"
        );
    }

    #[test]
    fn test_no_timeout_when_recent() {
        // Analysis started just now should not be timed out
        let started_at = chrono::Utc::now().timestamp() - 10; // Started 10s ago
        let elapsed = chrono::Utc::now().timestamp() - started_at;

        assert!(
            elapsed <= ANALYSIS_TIMEOUT_SECS,
            "10s elapsed should not exceed 300s timeout"
        );
    }

    #[test]
    fn test_timeout_recovery_state_mutation() {
        // Simulate what get_analysis_status does on timeout
        let mut state = AnalysisState {
            running: true,
            completed: false,
            error: None,
            results: None,
            started_at: Some(chrono::Utc::now().timestamp() - 600),
            last_completed_at: None,
        };

        // Apply timeout recovery logic (mirror of get_analysis_status)
        if state.running {
            if let Some(started) = state.started_at {
                let elapsed = chrono::Utc::now().timestamp() - started;
                if elapsed > ANALYSIS_TIMEOUT_SECS {
                    state.running = false;
                    state.error = Some(format!("Analysis timed out after {}s", elapsed));
                    state.started_at = None;
                }
            }
        }

        assert!(!state.running, "Should be reset to not running");
        assert!(state.error.is_some(), "Should have error message");
        assert!(
            state.error.as_ref().unwrap().contains("timed out"),
            "Error should mention timeout"
        );
        assert!(state.started_at.is_none(), "started_at should be cleared");
    }

    // ========================================================================
    // is_aborted helper
    // ========================================================================

    #[test]
    fn test_is_aborted_reads_atomic() {
        // Reset the abort flag, check it reads false
        get_analysis_abort().store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(!is_aborted(), "Should not be aborted initially");

        // Set it, check it reads true
        get_analysis_abort().store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(is_aborted(), "Should be aborted after setting flag");

        // Clean up
        get_analysis_abort().store(false, std::sync::atomic::Ordering::SeqCst);
    }
}
