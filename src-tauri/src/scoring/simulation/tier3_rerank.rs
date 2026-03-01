//! Tier 3: Reranking Validation
//!
//! Tests the post-scoring reranking layer: sort_results, dedup_results,
//! topic_dedup_results, and compute_serendipity_candidates.
//! Tier 1 = keyword matching, Tier 2 = semantic scoring, Tier 3 = reranking.

#[cfg(test)]
mod tests {
    use super::super::super::{
        compute_serendipity_candidates, dedup_results, score_item, sort_results,
        topic_dedup_results,
    };
    use super::super::corpus::corpus;
    use super::super::personas::all_personas;
    use super::super::PI_RUST;
    use super::super::{sim_db, sim_input, sim_no_freshness};
    use crate::SourceRelevance;

    // ========================================================================
    // Helper
    // ========================================================================

    fn mock_result(
        id: u64,
        title: &str,
        url: &str,
        score: f32,
        relevant: bool,
        excluded: bool,
    ) -> SourceRelevance {
        SourceRelevance {
            id,
            title: title.to_string(),
            url: Some(url.to_string()),
            top_score: score,
            relevant,
            excluded,
            matches: vec![],
            context_score: 0.0,
            interest_score: 0.0,
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
            streets_engine: None,
        }
    }

    // ========================================================================
    // Sort invariants
    // ========================================================================

    #[test]
    fn rerank_sort_excluded_items_last() {
        let mut items = vec![
            mock_result(1, "Excluded high", "https://a.com/1", 0.9, true, true),
            mock_result(2, "Normal mid", "https://a.com/2", 0.5, true, false),
            mock_result(3, "Excluded low", "https://a.com/3", 0.3, true, true),
            mock_result(4, "Normal high", "https://a.com/4", 0.8, true, false),
            mock_result(5, "Normal low", "https://a.com/5", 0.2, true, false),
        ];

        sort_results(&mut items);

        // First 3 items must be non-excluded, last 2 excluded
        for (i, item) in items.iter().enumerate() {
            if i < 3 {
                assert!(
                    !item.excluded,
                    "Item at position {} should NOT be excluded, got '{}' (excluded=true)",
                    i, item.title
                );
            } else {
                assert!(
                    item.excluded,
                    "Item at position {} should be excluded, got '{}' (excluded=false)",
                    i, item.title
                );
            }
        }
    }

    #[test]
    fn rerank_sort_by_score_descending() {
        let mut items = vec![
            mock_result(1, "Low", "https://a.com/1", 0.2, true, false),
            mock_result(2, "High", "https://a.com/2", 0.9, true, false),
            mock_result(3, "Mid", "https://a.com/3", 0.5, true, false),
            mock_result(4, "Very high", "https://a.com/4", 0.95, true, false),
            mock_result(5, "Medium", "https://a.com/5", 0.6, true, false),
        ];

        sort_results(&mut items);

        for i in 0..items.len() - 1 {
            assert!(
                items[i].top_score >= items[i + 1].top_score,
                "Score at position {} ({}) should be >= position {} ({})",
                i,
                items[i].top_score,
                i + 1,
                items[i + 1].top_score
            );
        }
    }

    #[test]
    fn rerank_sort_stable_for_equal_scores() {
        // Items with equal scores should maintain relative order (stable sort)
        let mut items = vec![
            mock_result(1, "Alpha", "https://a.com/1", 0.7, true, false),
            mock_result(2, "Bravo", "https://a.com/2", 0.7, true, false),
            mock_result(3, "Charlie", "https://a.com/3", 0.7, true, false),
        ];

        sort_results(&mut items);

        // All have the same score, so IDs should stay in original order
        assert_eq!(
            items[0].id, 1,
            "Stable sort: first item should remain first"
        );
        assert_eq!(
            items[1].id, 2,
            "Stable sort: second item should remain second"
        );
        assert_eq!(
            items[2].id, 3,
            "Stable sort: third item should remain third"
        );
    }

    // ========================================================================
    // Dedup quality
    // ========================================================================

    #[test]
    fn rerank_dedup_removes_url_duplicates() {
        let mut items = vec![
            mock_result(1, "Article A", "https://example.com/post", 0.8, true, false),
            mock_result(2, "Article B", "https://example.com/post", 0.3, true, false),
            mock_result(3, "Different", "https://other.com/page", 0.5, true, false),
        ];

        dedup_results(&mut items);

        assert_eq!(items.len(), 2, "URL duplicate should be removed");
        // The surviving duplicate should have the highest score
        let has_high = items.iter().any(|i| i.top_score == 0.8);
        assert!(has_high, "Highest-scoring URL duplicate should survive");
    }

    #[test]
    fn rerank_dedup_removes_title_duplicates() {
        let mut items = vec![
            mock_result(
                1,
                "Show HN: My Cool Project",
                "https://a.com/1",
                0.8,
                true,
                false,
            ),
            mock_result(2, "My Cool Project", "https://b.com/2", 0.6, true, false),
            mock_result(
                3,
                "Something Else Entirely",
                "https://c.com/3",
                0.5,
                true,
                false,
            ),
        ];

        dedup_results(&mut items);

        // "Show HN: My Cool Project" and "My Cool Project" normalize to same title
        assert_eq!(items.len(), 2, "Title duplicate should be removed");
    }

    #[test]
    fn rerank_dedup_preserves_unique_items() {
        let mut items: Vec<SourceRelevance> = (0..10)
            .map(|i| {
                mock_result(
                    i,
                    &format!("Unique title number {}", i),
                    &format!("https://site{}.com/page", i),
                    0.5 + (i as f32) * 0.03,
                    true,
                    false,
                )
            })
            .collect();

        dedup_results(&mut items);

        assert_eq!(items.len(), 10, "All unique items should survive dedup");
    }

    #[test]
    fn rerank_dedup_keeps_highest_scoring() {
        let mut items = vec![
            mock_result(
                1,
                "Same article",
                "https://example.com/same",
                0.3,
                true,
                false,
            ),
            mock_result(
                2,
                "Same article copy",
                "https://example.com/same",
                0.8,
                true,
                false,
            ),
        ];

        dedup_results(&mut items);

        assert_eq!(items.len(), 1, "Duplicate should be removed");
        assert_eq!(
            items[0].top_score, 0.8,
            "The higher-scoring duplicate (0.8) should survive, not 0.3"
        );
    }

    // ========================================================================
    // Topic dedup
    // ========================================================================

    #[test]
    fn rerank_topic_dedup_limits_per_cluster() {
        // 6 items all about "rust" -- topic_dedup groups items sharing
        // the same primary extracted topic
        let mut items: Vec<SourceRelevance> = (0..6)
            .map(|i| {
                mock_result(
                    i,
                    &format!("Advanced Rust programming techniques part {}", i + 1),
                    &format!("https://rust{}.com/post", i),
                    0.9 - (i as f32) * 0.05,
                    true,
                    false,
                )
            })
            .collect();

        // Must be sorted before topic dedup (function contract)
        sort_results(&mut items);
        topic_dedup_results(&mut items);

        // Topic dedup keeps the representative and removes items sharing the
        // same primary topic. With 6 same-topic items, at least some should
        // be removed (only the representative survives per topic).
        assert!(
            items.len() < 6,
            "Topic dedup should remove some same-topic items, got {} remaining",
            items.len()
        );
    }

    #[test]
    fn rerank_topic_dedup_preserves_diversity() {
        // Items across distinctly different topics
        let topics = [
            "Rust async runtime internals",
            "Python machine learning with PyTorch",
            "Kubernetes deployment strategies",
            "Swift iOS development patterns",
        ];
        let mut items: Vec<SourceRelevance> = topics
            .iter()
            .enumerate()
            .map(|(i, title)| {
                mock_result(
                    i as u64,
                    title,
                    &format!("https://site{}.com/post", i),
                    0.8 - (i as f32) * 0.05,
                    true,
                    false,
                )
            })
            .collect();

        sort_results(&mut items);
        topic_dedup_results(&mut items);

        // Each topic is distinct, so all items should survive
        assert_eq!(
            items.len(),
            4,
            "Diverse topics should all survive topic dedup"
        );
    }

    #[test]
    fn rerank_topic_dedup_keeps_highest_per_topic() {
        // 5 items about the same topic with different scores
        let mut items = vec![
            mock_result(
                1,
                "Rust memory safety deep dive",
                "https://a.com/1",
                0.9,
                true,
                false,
            ),
            mock_result(
                2,
                "Rust ownership model explained",
                "https://a.com/2",
                0.7,
                true,
                false,
            ),
            mock_result(
                3,
                "Rust borrow checker tutorial",
                "https://a.com/3",
                0.5,
                true,
                false,
            ),
            mock_result(
                4,
                "Rust lifetimes for beginners",
                "https://a.com/4",
                0.3,
                true,
                false,
            ),
            mock_result(
                5,
                "Rust trait objects patterns",
                "https://a.com/5",
                0.1,
                true,
                false,
            ),
        ];

        sort_results(&mut items);
        topic_dedup_results(&mut items);

        // The representative (highest scored) should survive
        assert!(
            items[0].top_score >= 0.9 - f32::EPSILON,
            "Highest-scoring topic item should be the representative, got {}",
            items[0].top_score
        );

        // If any were removed, the remaining should be the higher-scored ones
        for i in 0..items.len().saturating_sub(1) {
            assert!(
                items[i].top_score >= items[i + 1].top_score,
                "Surviving items should maintain score order"
            );
        }
    }

    // ========================================================================
    // Serendipity
    // ========================================================================

    #[test]
    fn rerank_serendipity_returns_bounded_count() {
        let mut items = vec![mock_result(
            1,
            "Relevant item",
            "https://a.com/1",
            0.8,
            true,
            false,
        )];
        // Add many non-relevant items with partial signal
        for i in 0..20 {
            let mut item = mock_result(
                (i + 10) as u64,
                &format!("Near miss item {}", i),
                &format!("https://miss{}.com", i),
                0.3,
                false,
                false,
            );
            item.context_score = 0.3; // Above serendipity min axis score (0.2)
            items.push(item);
        }

        let candidates = compute_serendipity_candidates(&items, 20);

        // Budget is capped at 5 by the clamp in compute_serendipity_candidates
        assert!(
            candidates.len() <= 5,
            "Serendipity should be bounded at 5, got {}",
            candidates.len()
        );
    }

    #[test]
    fn rerank_serendipity_excludes_already_relevant() {
        // All items are relevant -- none should appear as serendipity
        let items = vec![
            mock_result(1, "Relevant A", "https://a.com/1", 0.8, true, false),
            mock_result(2, "Relevant B", "https://a.com/2", 0.7, true, false),
            mock_result(3, "Relevant C", "https://a.com/3", 0.6, true, false),
        ];

        let candidates = compute_serendipity_candidates(&items, 100);

        assert!(
            candidates.is_empty(),
            "Already-relevant items should not become serendipity candidates"
        );
    }

    #[test]
    fn rerank_serendipity_empty_on_empty_input() {
        let items: Vec<SourceRelevance> = vec![];
        let candidates = compute_serendipity_candidates(&items, 20);
        assert!(
            candidates.is_empty(),
            "Empty input should produce empty serendipity"
        );
    }

    // ========================================================================
    // Integration with corpus
    // ========================================================================

    #[test]
    fn rerank_full_pipeline_corpus_sweep() {
        let personas = all_personas();
        let ctx = &personas[PI_RUST];
        let db = sim_db();
        let opts = sim_no_freshness();
        let items = corpus();
        let emb = vec![0.0_f32; 384];

        // Score all corpus items
        let mut results: Vec<SourceRelevance> = items
            .iter()
            .map(|item| {
                let input = sim_input(item.id, item.title, item.content, &emb);
                score_item(&input, ctx, &db, &opts, None)
            })
            .collect();

        let original_count = results.len();

        // Run the full reranking pipeline
        sort_results(&mut results);
        dedup_results(&mut results);
        topic_dedup_results(&mut results);

        // Count should be <= original (dedup may remove some)
        assert!(
            results.len() <= original_count,
            "Reranked count {} should be <= original {}",
            results.len(),
            original_count
        );

        // All scores in valid range
        for item in &results {
            assert!(
                item.top_score >= 0.0 && item.top_score <= 1.0,
                "Score {} out of [0.0, 1.0] range for '{}'",
                item.top_score,
                item.title
            );
        }

        // Excluded items at end
        let first_excluded = results.iter().position(|r| r.excluded);
        if let Some(pos) = first_excluded {
            for item in &results[pos..] {
                assert!(
                    item.excluded,
                    "After first excluded item at position {}, '{}' should also be excluded",
                    pos, item.title
                );
            }
        }
    }

    #[test]
    fn rerank_dedup_reduces_similar_titles() {
        let personas = all_personas();
        let ctx = &personas[PI_RUST];
        let db = sim_db();
        let opts = sim_no_freshness();
        let items = corpus();
        let emb = vec![0.0_f32; 384];

        let mut results: Vec<SourceRelevance> = items
            .iter()
            .map(|item| {
                let input = sim_input(item.id, item.title, item.content, &emb);
                score_item(&input, ctx, &db, &opts, None)
            })
            .collect();

        let before_dedup = results.len();
        dedup_results(&mut results);
        let after_dedup = results.len();

        // Dedup should not increase count
        assert!(
            after_dedup <= before_dedup,
            "Dedup should not increase item count: {} -> {}",
            before_dedup,
            after_dedup
        );

        // All remaining items should have unique normalized URLs
        let urls: Vec<String> = results
            .iter()
            .filter_map(|r| r.url.as_ref())
            .map(|u| u.to_lowercase())
            .collect();
        let unique_urls: std::collections::HashSet<&String> = urls.iter().collect();
        // Note: URL normalization may differ slightly, but no exact duplicates
        // should remain after dedup
        assert_eq!(
            urls.len(),
            unique_urls.len(),
            "No duplicate URLs should remain after dedup"
        );
    }

    #[test]
    fn rerank_ordering_preserved_after_full_pipeline() {
        let personas = all_personas();
        let ctx = &personas[PI_RUST];
        let db = sim_db();
        let opts = sim_no_freshness();
        let items = corpus();
        let emb = vec![0.0_f32; 384];

        let mut results: Vec<SourceRelevance> = items
            .iter()
            .map(|item| {
                let input = sim_input(item.id, item.title, item.content, &emb);
                score_item(&input, ctx, &db, &opts, None)
            })
            .collect();

        sort_results(&mut results);
        dedup_results(&mut results);
        // Re-sort after dedup since dedup sorts internally by score
        sort_results(&mut results);
        topic_dedup_results(&mut results);

        // Non-excluded section should be in descending score order
        let non_excluded: Vec<&SourceRelevance> = results.iter().filter(|r| !r.excluded).collect();

        for i in 0..non_excluded.len().saturating_sub(1) {
            assert!(
                non_excluded[i].top_score >= non_excluded[i + 1].top_score,
                "Non-excluded items should be in descending score order: \
                 position {} ({}) >= position {} ({}) failed for '{}' vs '{}'",
                i,
                non_excluded[i].top_score,
                i + 1,
                non_excluded[i + 1].top_score,
                non_excluded[i].title,
                non_excluded[i + 1].title,
            );
        }
    }
}
