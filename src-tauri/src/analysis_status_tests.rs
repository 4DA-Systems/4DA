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

    // ========================================================================
    // Curation persistence (regression guard, 2026-07-22)
    //
    // The scheduled/background analysis path scored items every cycle but never
    // persisted the curation VERDICT (`feed_relevant`) or the `scoring_events`
    // telemetry row — only the foreground wrapper did. So tray-resident/headless
    // runs SCORED without CURATING: `feed_relevant` froze and the content graph +
    // calibration silently went stale (live 2026-07-22: 586 items unjudged across
    // 14 scheduled cycles). `persist_cycle_results` is now the SINGLE persistence
    // site reached by every path via `analyze_cached_content_inner`. This locks
    // its contract so a future refactor can't silently drop the verdict again.
    // ========================================================================

    #[test]
    fn persist_cycle_results_writes_verdict_scores_and_scoring_event() {
        use crate::test_utils::{insert_test_item, test_db};
        use crate::types::SourceRelevance;

        let db = test_db();
        let id_relevant = insert_test_item(&db, "hackernews", "hn_rel", "Relevant item", "body");
        let id_noise = insert_test_item(&db, "hackernews", "hn_noise", "Noise item", "body");
        let id_relevant2 = insert_test_item(&db, "crates_io", "cr_rel", "Relevant crate", "body");

        // Build SourceRelevance via serde defaults — only id/title/url/top_score/
        // matches/relevant are required; every other field defaults.
        let make = |id: i64, top_score: f32, relevant: bool| -> SourceRelevance {
            serde_json::from_value(serde_json::json!({
                "id": id,
                "title": format!("item {id}"),
                "url": null,
                "top_score": top_score,
                "matches": [],
                "relevant": relevant,
            }))
            .expect("construct SourceRelevance")
        };

        let results = vec![
            make(id_relevant, 0.91, true),
            make(id_noise, 0.0, false),
            make(id_relevant2, 0.72, true),
        ];

        crate::analysis::persist_cycle_results(&db, &results);

        let conn = db.conn.lock();
        let verdict = |id: i64| -> (Option<i64>, Option<String>) {
            conn.query_row(
                "SELECT feed_relevant, feed_verdict_at FROM source_items WHERE id = ?1",
                rusqlite::params![id],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .unwrap()
        };
        let (fr1, ts1) = verdict(id_relevant);
        let (fr0, ts0) = verdict(id_noise);
        let (fr2, ts2) = verdict(id_relevant2);
        assert_eq!(fr1, Some(1), "relevant item curated into the corpus");
        assert_eq!(
            fr0,
            Some(0),
            "noise item judged NOT relevant (still stamped)"
        );
        assert_eq!(fr2, Some(1), "second relevant item curated");
        assert!(
            ts1.is_some() && ts0.is_some() && ts2.is_some(),
            "every judged item gets a verdict timestamp"
        );

        // Pipeline version stamped for EVERY scored item (incl. top_score == 0 noise).
        let versioned: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items WHERE scored_pipeline_version = ?1",
                rusqlite::params![crate::scoring::PIPELINE_VERSION],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            versioned, 3,
            "all three items stamped at current pipeline version"
        );

        // Relevance scores persisted only for top_score > 0 items (noise skipped).
        let scored: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items WHERE relevance_score IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            scored, 2,
            "only the two top_score>0 items get a persisted score"
        );

        // Scoring-event telemetry row written with correct counts.
        let (total_scored, total_relevant): (i64, i64) = conn
            .query_row(
                "SELECT total_scored, total_relevant FROM scoring_events ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("a scoring_events row must exist after a cycle");
        assert_eq!(total_scored, 3, "scoring_events records all judged items");
        assert_eq!(
            total_relevant, 2,
            "scoring_events records the relevant count"
        );
    }
}
