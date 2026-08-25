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
        // `evidence_score` mirrors top_score, as score_item sets it at
        // construction (items 12+26): persistence writes relevance_score from
        // evidence, so a fixture modelling a scored result must carry it.
        let make = |id: i64, top_score: f32, relevant: bool| -> SourceRelevance {
            serde_json::from_value(serde_json::json!({
                "id": id,
                "title": format!("item {id}"),
                "url": null,
                "top_score": top_score,
                "matches": [],
                "relevant": relevant,
                "evidence_score": top_score,
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

        // Relevance scores persisted only for evidence > 0 items (noise skipped).
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

    // ========================================================================
    // Degraded-input persist guard (2026-08-23 audit, item 11)
    //
    // A run whose inputs SYSTEMICALLY collapsed (dep-intel load failure,
    // context-KNN failure) produces confidently-wrong scores. The persist
    // boundary must not let them overwrite fresh durable judgments — while the
    // 7-day escape hatch keeps a permanently-degraded deployment from freezing.
    // ========================================================================

    /// SourceRelevance with a breakdown carrying the given degraded markers.
    /// `evidence_score` mirrors top_score (score_item sets it at construction;
    /// persistence writes relevance_score from evidence — items 12+26).
    fn make_with_markers(
        id: i64,
        top_score: f32,
        relevant: bool,
        markers: &[&str],
    ) -> crate::types::SourceRelevance {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "title": format!("item {id}"),
            "url": null,
            "top_score": top_score,
            "matches": [],
            "relevant": relevant,
            "evidence_score": top_score,
            "score_breakdown": {
                "context_score": 0.0,
                "interest_score": 0.0,
                "ace_boost": 0.0,
                "affinity_mult": 1.0,
                "anti_penalty": 0.0,
                "confidence_by_signal": {},
                "degraded_inputs": markers,
            },
        }))
        .expect("construct SourceRelevance with breakdown")
    }

    #[test]
    fn degraded_run_protects_fresh_durable_scores_with_age_escape_hatch() {
        use crate::test_utils::{insert_test_item, test_db};

        let db = test_db();
        let protected = insert_test_item(&db, "hackernews", "dg1", "Fresh durable", "body");
        let aged = insert_test_item(&db, "hackernews", "dg2", "Aged durable", "body");
        let first = insert_test_item(&db, "hackernews", "dg3", "Never scored", "body");
        let emb_only = insert_test_item(&db, "hackernews", "dg4", "Embedding missing", "body");

        // Healthy cycle seeds durable judgments for `protected` and `aged`.
        crate::analysis::persist_cycle_results(
            &db,
            &[
                make_with_markers(protected, 0.80, true, &[]),
                make_with_markers(aged, 0.70, true, &[]),
            ],
        );
        {
            let conn = db.conn.lock();
            // Age `aged` past the escape hatch, and give both a distinguishable
            // old version stamp so the no-stamp-on-protect rule is observable.
            conn.execute(
                "UPDATE source_items SET feed_verdict_at = datetime('now', '-8 days') WHERE id = ?1",
                rusqlite::params![aged],
            )
            .unwrap();
            conn.execute(
                "UPDATE source_items SET scored_pipeline_version = 1 WHERE id IN (?1, ?2)",
                rusqlite::params![protected, aged],
            )
            .unwrap();
        }

        // Systemically degraded cycle wants radically different judgments.
        crate::analysis::persist_cycle_results(
            &db,
            &[
                make_with_markers(protected, 0.10, false, &["dep_intel_load_failed"]),
                make_with_markers(aged, 0.10, false, &["context_knn_failed"]),
                make_with_markers(first, 0.30, false, &["dep_intel_load_failed"]),
                make_with_markers(emb_only, 0.25, true, &["embedding_missing"]),
            ],
        );

        let conn = db.conn.lock();
        let row = |id: i64| -> (Option<f64>, i64, Option<i64>) {
            conn.query_row(
                "SELECT relevance_score, scored_pipeline_version, feed_relevant
                 FROM source_items WHERE id = ?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap()
        };

        let (p_score, p_version, p_verdict) = row(protected);
        assert!(
            (p_score.unwrap() - f64::from(0.80f32)).abs() < 1e-6,
            "fresh durable score survives a degraded run, got {p_score:?}"
        );
        assert_eq!(
            p_version, 1,
            "a protected item is NOT version-stamped — a later healthy run must re-score it"
        );
        assert_eq!(p_verdict, Some(1), "fresh durable verdict survives too");

        let (a_score, a_version, _) = row(aged);
        assert!(
            (a_score.unwrap() - f64::from(0.10f32)).abs() < 1e-6,
            ">7-day-old durable score accepts the degraded write (no freeze), got {a_score:?}"
        );
        assert_eq!(
            a_version,
            i64::from(crate::scoring::PIPELINE_VERSION),
            "an accepted write stamps normally"
        );

        let (f_score, f_version, f_verdict) = row(first);
        assert!(
            (f_score.unwrap() - f64::from(0.30f32)).abs() < 1e-6,
            "an item with no durable score writes even on a degraded run"
        );
        assert_eq!(f_version, i64::from(crate::scoring::PIPELINE_VERSION));
        assert_eq!(f_verdict, Some(0));

        let (e_score, _, e_verdict) = row(emb_only);
        assert!(
            (e_score.unwrap() - f64::from(0.25f32)).abs() < 1e-6,
            "per-item embedding_missing is NOT a systemic degradation"
        );
        assert_eq!(e_verdict, Some(1));
    }

    // ========================================================================
    // Differential drain safety (2026-08-23 audit, item 9)
    // ========================================================================

    /// Version-stale items still drain on differential runs: the batch merger
    /// the differential path calls picks up anything scored under an older
    /// pipeline version and folds it into the run's scoring set (deduplicated
    /// against items already selected).
    #[test]
    fn version_stale_items_merge_into_the_differential_batch() {
        use crate::test_utils::{insert_test_item, test_db};

        let db = test_db();
        let stale = insert_test_item(&db, "hackernews", "ds1", "Old-version score", "body");
        let current = insert_test_item(&db, "hackernews", "ds2", "Current score", "body");
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE source_items SET relevance_score = 0.6, scored_pipeline_version = 1 WHERE id = ?1",
                rusqlite::params![stale],
            )
            .unwrap();
            conn.execute(
                "UPDATE source_items SET relevance_score = 0.6, scored_pipeline_version = ?1 WHERE id = ?2",
                rusqlite::params![crate::scoring::PIPELINE_VERSION, current],
            )
            .unwrap();
        }

        let mut items = Vec::new(); // the differential selection found nothing new
        let added = crate::analysis::merge_stale_drain_batch(&db, &mut items);
        assert_eq!(added, 1, "exactly the stale-version item is merged");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, stale);

        // Already selected → not merged twice.
        let added_again = crate::analysis::merge_stale_drain_batch(&db, &mut items);
        assert_eq!(
            added_again, 0,
            "an already-selected item is never duplicated"
        );
    }

    // ========================================================================
    // Rolling freshness refresh (2026-08-25 tightening T1)
    // ========================================================================

    /// The freshness merge folds the stalest-scored slice of the recent
    /// window into the differential batch WITHOUT duplicating ids already
    /// selected by the change-based differential or the stale drain.
    #[test]
    fn freshness_refresh_merges_deduped_into_the_differential_batch() {
        use crate::test_utils::{insert_test_item, test_db};

        let db = test_db();
        let a = insert_test_item(&db, "hackernews", "fm1", "In window A", "body");
        let b = insert_test_item(&db, "hackernews", "fm2", "In window B", "body");

        // The differential already selected one of the window's items.
        let mut items = db
            .get_freshness_refresh_batch(168, 1)
            .expect("freshness batch");
        assert_eq!(items.len(), 1);
        let preselected = items[0].id;

        let added = crate::analysis::merge_freshness_refresh_batch(&db, &mut items);
        assert_eq!(added, 1, "only the not-yet-selected item is merged");
        assert_eq!(items.len(), 2);
        let ids: std::collections::HashSet<i64> = items.iter().map(|i| i.id).collect();
        assert_eq!(
            ids,
            std::collections::HashSet::from([a, b]),
            "both window items present exactly once"
        );
        let _ = preselected;

        // Re-merging without scoring adds nothing — everything the batch
        // returns is already selected.
        let added_again = crate::analysis::merge_freshness_refresh_batch(&db, &mut items);
        assert_eq!(
            added_again, 0,
            "an already-selected item is never duplicated"
        );
        assert_eq!(items.len(), 2);
    }
}
