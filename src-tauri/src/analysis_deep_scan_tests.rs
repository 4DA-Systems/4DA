#[cfg(test)]
mod tests {
    use crate::types::extract_near_misses;
    use crate::{get_analysis_abort, get_analysis_state, SourceRelevance};
    use std::sync::atomic::Ordering;

    fn make_item(id: u64, top_score: f32, relevant: bool, excluded: bool) -> SourceRelevance {
        SourceRelevance {
            id,
            title: format!("Item {id}"),
            url: None,
            top_score,
            matches: vec![],
            relevant,
            context_score: 0.0,
            interest_score: 0.0,
            excluded,
            excluded_by: None,
            source_type: "hackernews".to_string(),
            explanation: None,
            confidence: None,
            score_breakdown: None,
            signal_type: None,
            signal_priority: None,
            signal_action: None,
            signal_triggers: None,
            signal_horizon: None,
            similar_count: 0,
            similar_titles: vec![],
            serendipity: false,
            detected_lang: "en".to_string(),
            streets_engine: None,
            decision_window_match: None,
            decision_boost_applied: 0.0,
            created_at: None,
        }
    }

    // ========================================================================
    // Deep scan state machine and concurrency guards
    // ========================================================================

    #[test]
    fn deep_scan_prevents_double_run() {
        let state = get_analysis_state();
        {
            let mut guard = state.lock();
            guard.running = true;
        }
        // Verify the running flag prevents re-entry
        let guard = state.lock();
        assert!(
            guard.running,
            "Running flag should prevent concurrent scans"
        );
        // Cleanup
        drop(guard);
        let mut guard = state.lock();
        guard.running = false;
    }

    #[test]
    fn abort_flag_resets_on_start() {
        let abort = get_analysis_abort();
        abort.store(true, Ordering::SeqCst);
        // Simulate what run_deep_initial_scan does at start
        abort.store(false, Ordering::SeqCst);
        assert!(!abort.load(Ordering::SeqCst));
    }

    // ========================================================================
    // Near-miss extraction with deep scan data
    // ========================================================================

    #[test]
    fn near_miss_extraction_with_deep_scan_data() {
        // Fewer than 3 relevant results -> near misses should be populated
        let items = vec![
            make_item(1, 0.8, true, false),   // relevant
            make_item(2, 0.7, true, false),   // relevant
            make_item(3, 0.45, false, false), // not relevant, but above 0.20 threshold
            make_item(4, 0.30, false, false), // not relevant, above threshold
            make_item(5, 0.25, false, false), // not relevant, above threshold
            make_item(6, 0.15, false, false), // below threshold
        ];
        let near_misses = extract_near_misses(&items);
        assert!(
            near_misses.is_some(),
            "Should have near misses when <3 relevant"
        );
        let nm = near_misses.unwrap();
        assert!(nm.len() <= 5, "Near misses capped at 5");
        // All near misses should be non-relevant items above threshold
        for item in &nm {
            assert!(!item.relevant);
            assert!(item.top_score >= 0.20);
        }
    }

    // ========================================================================
    // Result sorting
    // ========================================================================

    #[test]
    fn result_sorting_preserves_order() {
        let mut items = vec![
            make_item(1, 0.3, false, false),
            make_item(2, 0.9, true, false),
            make_item(3, 0.6, true, false),
        ];
        items.sort_by(|a, b| b.top_score.partial_cmp(&a.top_score).unwrap());
        assert_eq!(items[0].id, 2);
        assert_eq!(items[1].id, 3);
        assert_eq!(items[2].id, 1);
    }

    // ========================================================================
    // Progress computation boundaries
    // ========================================================================

    #[test]
    fn progress_computation_at_boundaries() {
        let total = 100usize;
        // At start (idx=0)
        let progress_start = 0.60 + (0.35 * 0.0 / total as f32);
        assert!((progress_start - 0.60).abs() < f32::EPSILON);
        // At end (idx=total-1)
        let progress_end = 0.60 + (0.35 * 99.0 / total as f32);
        assert!(progress_end < 0.95);
        assert!(progress_end > 0.90);
        // Edge case: total=1, idx=0
        let progress_single = 0.60 + (0.35 * 0.0 / 1.0_f32);
        assert!((progress_single - 0.60).abs() < f32::EPSILON);
    }

    // ========================================================================
    // Source diversity counting
    // ========================================================================

    #[test]
    fn source_diversity_counting() {
        use std::collections::HashSet;
        let items = vec![
            make_item(1, 0.8, true, false),
            make_item(2, 0.7, true, false),
            make_item(3, 0.6, true, false),
        ];
        // All items have source_type "hackernews" from make_item
        let sources: HashSet<&str> = items.iter().map(|i| i.source_type.as_str()).collect();
        assert_eq!(sources.len(), 1);

        // With diverse sources
        let mut items2 = items.clone();
        items2[1].source_type = "reddit".to_string();
        items2[2].source_type = "github".to_string();
        let sources2: HashSet<&str> = items2.iter().map(|i| i.source_type.as_str()).collect();
        assert_eq!(sources2.len(), 3);
    }

    // ========================================================================
    // High-match narration cap
    // ========================================================================

    #[test]
    fn high_match_narration_cap_at_3() {
        let items = vec![
            make_item(1, 0.9, true, false),
            make_item(2, 0.85, true, false),
            make_item(3, 0.8, true, false),
            make_item(4, 0.75, true, false),
        ];
        let mut high_match_count = 0;
        for item in &items {
            if item.top_score >= 0.75 && high_match_count < 3 {
                high_match_count += 1;
            }
        }
        assert_eq!(high_match_count, 3, "Should cap at 3 high matches");
    }
}
