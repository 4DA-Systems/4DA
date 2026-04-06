// ============================================================================
// Tests (pipeline-level tests that don't belong in individual modules)
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::context_engine;
    use crate::scoring::pipeline::{score_item, ScoringInput, ScoringOptions};
    use crate::scoring::{ACEContext, ScoringContext};
    use crate::test_utils::{empty_scoring_context, test_db};

    /// Helper: build a ScoringInput with a dummy 384-dim embedding
    fn test_input<'a>(title: &'a str, content: &'a str, embedding: &'a [f32]) -> ScoringInput<'a> {
        ScoringInput {
            id: 1,
            title,
            url: Some("https://example.com"),
            content,
            source_type: "hackernews",
            embedding,
            created_at: None,
            detected_lang: "en",
        }
    }

    #[test]
    fn test_score_item_zero_context_returns_low_score() {
        let db = test_db();
        let ctx = empty_scoring_context();
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Some random article about gardening",
            "Plants and soil",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);

        assert!(
            result.top_score < 0.35,
            "Empty context should produce low score, got {}",
            result.top_score
        );
        assert!(
            !result.relevant,
            "Should not be relevant with empty context"
        );
        assert!(!result.excluded, "Should not be excluded");
    }

    #[test]
    fn test_score_item_excluded_item_returns_zero() {
        let db = test_db();
        let ctx = ScoringContext::builder()
            .exclusions(vec!["blockchain".to_string()])
            .build();
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Blockchain scaling solutions for enterprise",
            "blockchain distributed ledger technology",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);

        assert_eq!(result.top_score, 0.0, "Excluded item should have score 0");
        assert!(result.excluded, "Should be marked as excluded");
        assert!(
            result.excluded_by.is_some(),
            "Should report what excluded it"
        );
        assert!(
            result.excluded_by.as_ref().unwrap().contains("blockchain"),
            "Should be excluded by 'blockchain', got {:?}",
            result.excluded_by
        );
    }

    #[test]
    fn test_score_item_two_signals_can_pass() {
        let db = test_db();
        let ace_ctx = ACEContext::default();

        let interest_embedding = vec![0.5_f32; 384];
        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "rust".to_string(),
            weight: 1.0,
            embedding: Some(interest_embedding.clone()),
            source: context_engine::InterestSource::Explicit,
        }];

        // Two genuinely independent signals: interest (via embedding) + learned (via feedback)
        let mut feedback_boosts = std::collections::HashMap::new();
        feedback_boosts.insert("performance".to_string(), 0.50); // net_score * FEEDBACK_SCALE must exceed FEEDBACK_THRESHOLD (0.05)

        let ctx = ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace_ctx)
            .feedback_boosts(feedback_boosts)
            .build();

        // Use same embedding as interest so interest_score is high
        let input = test_input(
            "Rust performance improvements in async runtimes",
            "rust performance benchmarks async await tokio",
            &interest_embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let breakdown = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        // Interest confirmed (high interest_score via same embedding)
        // Learned confirmed (feedback_boost > 0.05 via "async" topic match)
        assert!(
            breakdown.signal_count >= 2,
            "Expected 2+ confirmed signals, got {} ({:?})",
            breakdown.signal_count,
            breakdown.confirmed_signals
        );
    }

    #[test]
    fn test_score_item_single_signal_cannot_pass() {
        // Single-signal items must never pass. The confirmation gate caps single-signal
        // scores at 0.28 (well below the 0.35 threshold), and the quality floor requires
        // min 2 signals regardless of feedback interaction count.
        let db = test_db();
        // Only set up interests, no ACE context, no context chunks
        let interest_embedding = vec![0.5_f32; 384];
        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "machine learning".to_string(),
            weight: 1.0,
            embedding: Some(interest_embedding.clone()),
            source: context_engine::InterestSource::Explicit,
        }];

        let ctx = ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .build();

        // Same embedding as interest so interest_score is high,
        // but no ACE topics, no context, no dependencies
        let input = test_input(
            "Machine learning model training tips",
            "machine learning neural networks training optimization",
            &interest_embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let breakdown = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        // Only interest axis should confirm (via keyword or embedding match)
        // With just 1 signal the confirmation gate caps below the threshold
        assert!(
            breakdown.signal_count <= 1,
            "Expected at most 1 confirmed signal, got {} ({:?})",
            breakdown.signal_count,
            breakdown.confirmed_signals
        );
        assert!(
            !result.relevant,
            "Single-signal item should not pass relevance gate (score={}, signals={})",
            result.top_score, breakdown.signal_count
        );
    }

    // ========================================================================
    // Stack Intelligence pipeline integration tests
    // ========================================================================

    #[test]
    fn test_score_item_stack_boost_in_breakdown() {
        // When a Rust stack is active and content matches Rust pain points,
        // the score_breakdown should contain a positive stack_boost.
        let db = test_db();
        let rust_stack = crate::stacks::compose_profiles(&["rust_systems".to_string()]);
        let ctx = ScoringContext::builder().composed_stack(rust_stack).build();
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Understanding Pin and Send in Async Rust Lifetimes",
            "async pin send lifetime future tokio complexity borrow checker annotations",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let breakdown = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        assert!(
            breakdown.stack_boost > 0.0,
            "Rust pain point content should produce stack_boost > 0, got {}",
            breakdown.stack_boost
        );
        // The boost should be capped at 0.20 (max from scoring function)
        assert!(
            breakdown.stack_boost <= 0.20,
            "stack_boost should be capped at 0.20, got {}",
            breakdown.stack_boost
        );
    }

    #[test]
    fn test_score_item_no_stack_zero_boost() {
        // When no stack profiles are selected, stack_boost must be exactly 0.0
        // and ecosystem_shift_mult must be exactly 1.0.
        let db = test_db();
        let ctx = empty_scoring_context(); // no composed_stack → inactive
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Understanding Pin and Send in Async Rust Lifetimes",
            "async pin send lifetime future tokio complexity",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let breakdown = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        assert_eq!(
            breakdown.stack_boost, 0.0,
            "No stack selected → stack_boost must be 0.0, got {}",
            breakdown.stack_boost
        );
        assert_eq!(
            breakdown.ecosystem_shift_mult, 1.0,
            "No stack selected → ecosystem_shift_mult must be 1.0, got {}",
            breakdown.ecosystem_shift_mult
        );
        assert_eq!(
            breakdown.stack_competing_mult, 1.0,
            "No stack selected → stack_competing_mult must be 1.0, got {}",
            breakdown.stack_competing_mult
        );
    }

    #[test]
    fn test_score_item_stack_pain_match_confirms_ace_axis() {
        let db = test_db();
        let rust_stack = crate::stacks::compose_profiles(&["rust_systems".to_string()]);
        let ctx = ScoringContext::builder().composed_stack(rust_stack).build();
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Rust Borrow Checker: Ownership and Move Semantics Deep Dive",
            "borrow checker ownership move semantics lifetime annotation rust patterns",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let breakdown = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        // ACE should be confirmed (via stack_pain_match or topic overlap)
        assert!(
            breakdown.confirmed_signals.contains(&"ace".to_string()),
            "ACE axis should be confirmed with stack pain point match, got {:?}",
            breakdown.confirmed_signals
        );

        // Pain point content should produce a positive stack_boost
        assert!(
            breakdown.stack_boost > 0.0,
            "Borrow checker content should trigger Rust pain point, got stack_boost={}",
            breakdown.stack_boost
        );

        // Compare: same content WITHOUT stack should NOT have ACE confirmed
        let ctx_no_stack = empty_scoring_context();
        let result_no_stack = score_item(&input, &ctx_no_stack, &db, &options, None);
        let breakdown_ns = result_no_stack
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        assert!(
            !breakdown_ns.confirmed_signals.contains(&"ace".to_string()),
            "Without stack, ACE should NOT be confirmed (no topics, no semantic), got {:?}",
            breakdown_ns.confirmed_signals
        );
    }

    // ========================================================================
    // Existing unit tests
    // ========================================================================

    // Test source quality boost: positive score
    #[test]
    fn test_source_quality_positive_boost() {
        let score = 0.5_f32;
        let source_score = 0.8_f32;
        let boost = (source_score * 0.10).clamp(-0.10, 0.10);
        let result = (score + boost).clamp(0.0, 1.0);
        assert!(
            (result - 0.58).abs() < 0.01,
            "Positive source should boost by up to 10%: got {}",
            result
        );
    }

    // Test source quality boost: negative reduction
    #[test]
    fn test_source_quality_negative_reduction() {
        let score = 0.5_f32;
        let source_score = -0.6_f32;
        let boost = (source_score * 0.10).clamp(-0.10, 0.10);
        let result = (score + boost).clamp(0.0, 1.0);
        assert!(
            (result - 0.44).abs() < 0.01,
            "Negative source should reduce by up to 10%: got {}",
            result
        );
    }

    // Test source quality boost: unknown source returns 0
    #[test]
    fn test_source_quality_unknown_neutral() {
        let source_quality: std::collections::HashMap<String, f32> =
            std::collections::HashMap::new();
        let boost = source_quality
            .get("unknown_source")
            .copied()
            .map(|score| (score * 0.10).clamp(-0.10, 0.10))
            .unwrap_or(0.0);
        assert_eq!(boost, 0.0, "Unknown source should be neutral");
    }

    // Test source quality boost: cap enforcement
    #[test]
    fn test_source_quality_cap_enforcement() {
        // Even with extreme source score, boost capped at +/-10%
        let extreme_positive = (2.0_f32 * 0.10).clamp(-0.10, 0.10);
        assert_eq!(extreme_positive, 0.10, "Positive boost should cap at 0.10");

        let extreme_negative = (-2.0_f32 * 0.10).clamp(-0.10, 0.10);
        assert_eq!(
            extreme_negative, -0.10,
            "Negative boost should cap at -0.10"
        );
    }

    // ========================================================================
    // Phase 2: Pipeline Integration Tests
    // ========================================================================

    #[test]
    fn test_pipeline_stack_boost_survives_dampening() {
        // Verify stack_boost actually changes the final top_score (not dampened to nothing).
        let db = test_db();
        let rust_stack = crate::stacks::compose_profiles(&["rust_systems".to_string()]);
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Understanding Pin and Send in Async Rust Lifetimes",
            "async pin send lifetime future tokio complexity borrow checker annotations",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        // With stack
        let ctx_with_stack = ScoringContext::builder().composed_stack(rust_stack).build();
        let result_with = score_item(&input, &ctx_with_stack, &db, &options, None);

        // Without stack
        let ctx_no_stack = empty_scoring_context();
        let result_without = score_item(&input, &ctx_no_stack, &db, &options, None);

        let bd = result_with.score_breakdown.as_ref().unwrap();
        assert!(
            bd.stack_boost > 0.0,
            "Stack boost should be positive, got {}",
            bd.stack_boost
        );
        assert!(
            result_with.top_score > result_without.top_score,
            "Stack boost must survive dampening: with={} > without={}",
            result_with.top_score,
            result_without.top_score
        );
    }

    #[test]
    fn test_pipeline_ecosystem_shift_in_composite() {
        // Rust stack active, content with ecosystem shift keywords → mult > 1.0
        let db = test_db();
        let rust_stack = crate::stacks::compose_profiles(&["rust_systems".to_string()]);
        let ctx = ScoringContext::builder().composed_stack(rust_stack).build();
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Async Fn in Trait Is Finally Stable in Rust",
            "native async trait async fn in trait return position impl trait stabilization rust",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let bd = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        assert!(
            bd.ecosystem_shift_mult > 1.0,
            "Rust shift keywords should trigger ecosystem_shift_mult > 1.0, got {}",
            bd.ecosystem_shift_mult
        );
    }

    #[test]
    fn test_pipeline_competing_penalty_suppresses() {
        // Rust stack active, pure Go content → competing penalty < 1.0
        let db = test_db();
        let rust_stack = crate::stacks::compose_profiles(&["rust_systems".to_string()]);
        let ctx = ScoringContext::builder().composed_stack(rust_stack).build();
        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Go 1.23 Performance Improvements for Backend Services",
            "go golang backend services performance goroutine scheduling concurrency",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let bd = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        assert!(
            bd.stack_competing_mult < 1.0,
            "Pure Go content with Rust stack should get competing penalty < 1.0, got {}",
            bd.stack_competing_mult
        );
    }

    #[test]
    fn test_pipeline_new_user_still_requires_two_signals() {
        // Even with zero feedback interactions, the quality floor still requires 2 signals.
        // Previously "bootstrap mode" relaxed this to 1, causing false positives.
        let db = test_db();
        let ace_ctx = ACEContext::default();

        let interest_embedding = vec![0.5_f32; 384];
        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "rust".to_string(),
            weight: 1.0,
            embedding: Some(interest_embedding.clone()),
            source: context_engine::InterestSource::Explicit,
        }];

        // Two genuinely independent signals: interest (embedding) + learned (feedback)
        let mut feedback_boosts = std::collections::HashMap::new();
        feedback_boosts.insert("performance".to_string(), 0.50);

        // New user: feedback_interaction_count = 0
        let ctx = ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace_ctx)
            .feedback_boosts(feedback_boosts)
            .feedback_interaction_count(0)
            .build();

        let input = test_input(
            "Rust performance improvements in async runtimes",
            "rust performance benchmarks async await tokio",
            &interest_embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let bd = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        // With 2+ independent signals (interest + learned), score should pass
        assert!(
            bd.signal_count >= 2,
            "Expected 2+ signals even for new user, got {} ({:?})",
            bd.signal_count, bd.confirmed_signals
        );
        assert!(
            result.relevant,
            "2-signal item should pass for new user (score={}, signals={})",
            result.top_score, bd.signal_count
        );
    }

    #[test]
    fn test_pipeline_normal_mode_requires_two_signals() {
        let db = test_db();
        let interest_embedding = vec![0.5_f32; 384];
        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "machine learning".to_string(),
            weight: 1.0,
            embedding: Some(interest_embedding.clone()),
            source: context_engine::InterestSource::Explicit,
        }];

        let ctx = ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .feedback_interaction_count(50) // normal mode
            .build();

        let input = test_input(
            "Machine Learning Model Training Tips",
            "machine learning neural networks training optimization",
            &interest_embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);
        let bd = result
            .score_breakdown
            .as_ref()
            .expect("should have breakdown");

        assert!(
            bd.signal_count <= 1,
            "Expected at most 1 signal, got {} ({:?})",
            bd.signal_count,
            bd.confirmed_signals
        );
        assert!(
            !result.relevant,
            "Single signal must NOT pass in normal mode (score={}, signals={})",
            result.top_score, bd.signal_count
        );
    }

    #[test]
    fn test_pipeline_base_score_interest_only_path() {
        let db = test_db();
        let interest_embedding = vec![0.5_f32; 384];
        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "rust".to_string(),
            weight: 1.0,
            embedding: Some(interest_embedding.clone()),
            source: context_engine::InterestSource::Explicit,
        }];

        let ctx = ScoringContext::builder()
            .cached_context_count(0) // no context chunks
            .interest_count(1)
            .interests(interests)
            .build();

        let input = test_input(
            "Rust Async Performance Guide",
            "rust tokio async performance optimization",
            &interest_embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);

        assert!(
            result.top_score > 0.0,
            "Interest-only path must produce score > 0, got {}",
            result.top_score
        );
    }

    #[test]
    fn test_pipeline_base_score_context_only_path() {
        let db = test_db();

        let ctx = ScoringContext::builder()
            .cached_context_count(10) // has context chunks
            .interest_count(0) // no interests
            .build();

        let embedding = vec![0.1_f32; 384];
        let input = test_input(
            "Building REST APIs with Rust and Axum",
            "axum rust web api server tokio serde",
            &embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);

        // Score should be >= 0 (might be 0 if no matching context in DB, but shouldn't panic)
        assert!(
            result.top_score >= 0.0,
            "Context-only path must not panic, got score {}",
            result.top_score
        );
        assert!(!result.excluded, "Should not be excluded");
    }

    // ========================================================================
    // Existing unit tests
    // ========================================================================

    // ========================================================================
    // Edge case tests: title, embedding, and language corner cases
    // ========================================================================

    /// Empty title should have a reduced score due to the short-title cap.
    /// The pipeline caps items with < 3 meaningful words (>= 2 chars each)
    /// at QUALITY_FLOOR_SHORT_TITLE_CAP (0.40), which is well below the
    /// 0.35 relevance threshold for strong signals but still caps the max.
    #[test]
    fn test_empty_title_gets_capped() {
        let db = test_db();
        let interest_embedding = vec![0.5_f32; 384];
        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "rust".to_string(),
            weight: 1.0,
            embedding: Some(interest_embedding.clone()),
            source: context_engine::InterestSource::Explicit,
        }];
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("rust".to_string());
        ace_ctx.topic_confidence.insert("rust".to_string(), 0.9);

        let ctx = ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace_ctx)
            .build();

        // Empty title — this triggers the short-title cap
        let input = test_input("", "rust tokio async await performance", &interest_embedding);
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);

        // Empty title has 0 meaningful words (< 3), so score is capped at 0.40
        assert!(
            result.top_score <= 0.40,
            "Empty title should be capped at 0.40 (short-title floor), got {}",
            result.top_score
        );
    }

    /// A title with 100+ words should still score normally — no penalty for verbosity.
    /// The pipeline only penalizes SHORT titles (< 3 meaningful words), not long ones.
    #[test]
    fn test_very_long_title_not_penalized() {
        let db = test_db();
        let interest_embedding = vec![0.5_f32; 384];
        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "rust".to_string(),
            weight: 1.0,
            embedding: Some(interest_embedding.clone()),
            source: context_engine::InterestSource::Explicit,
        }];
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("rust".to_string());
        ace_ctx.topic_confidence.insert("rust".to_string(), 0.9);

        let ctx = ScoringContext::builder()
            .interest_count(1)
            .interests(interests.clone())
            .ace_ctx(ace_ctx.clone())
            .build();

        // Very long title (100+ words) about a relevant topic
        let long_title = std::iter::repeat("rust async performance tokio runtime optimization")
            .take(20)
            .collect::<Vec<_>>()
            .join(" ");
        // Same content but with a normal-length title for comparison
        let normal_title = "Rust Async Runtime Performance Improvements in Tokio";

        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let long_input = ScoringInput {
            id: 1,
            title: &long_title,
            url: Some("https://example.com/long"),
            content: "rust tokio async await performance benchmarks optimization",
            source_type: "hackernews",
            embedding: &interest_embedding,
            created_at: None,
            detected_lang: "en",
        };

        let normal_input = ScoringInput {
            id: 2,
            title: normal_title,
            url: Some("https://example.com/normal"),
            content: "rust tokio async await performance benchmarks optimization",
            source_type: "hackernews",
            embedding: &interest_embedding,
            created_at: None,
            detected_lang: "en",
        };

        let long_result = score_item(&long_input, &ctx, &db, &options, None);
        let normal_result = score_item(&normal_input, &ctx, &db, &options, None);

        // Long title should NOT be penalized — score should be similar to normal title.
        // Allow 20% variance because topic extraction differs with repeated words.
        let ratio = if normal_result.top_score > 0.0 {
            long_result.top_score / normal_result.top_score
        } else {
            1.0
        };
        assert!(
            ratio >= 0.70,
            "Long title should not be penalized vs normal title: long={}, normal={}, ratio={}",
            long_result.top_score, normal_result.top_score, ratio
        );
    }

    /// Zero-vector embeddings should still produce a score via keyword and dependency axes.
    /// The pipeline checks `has_real_embedding` and skips KNN/embedding when all zeros,
    /// but keyword interest matching, ACE keyword boost, and dependency matching still work.
    #[test]
    fn test_all_zero_embeddings_still_scores() {
        let db = test_db();
        let zero_embedding = vec![0.0_f32; 384];
        let real_embedding = vec![0.5_f32; 384];

        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "rust".to_string(),
            weight: 1.0,
            embedding: Some(real_embedding),
            source: context_engine::InterestSource::Explicit,
        }];
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("rust".to_string());
        ace_ctx.topic_confidence.insert("rust".to_string(), 0.9);

        let ctx = ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace_ctx)
            .build();

        // Zero embedding, but the title/content mention "rust" which matches interests + ACE
        let input = test_input(
            "Rust Borrow Checker Deep Dive",
            "rust borrow checker ownership move semantics",
            &zero_embedding,
        );
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);

        assert!(
            result.top_score > 0.0,
            "Zero embedding should still produce a score via keyword/ACE axes, got {}",
            result.top_score
        );

        // Verify keyword_score is what's contributing (not embedding similarity)
        let bd = result.score_breakdown.as_ref().expect("should have breakdown");
        assert!(
            bd.keyword_score > 0.0,
            "Keyword interest score should be positive when title matches interest topic, got {}",
            bd.keyword_score
        );
    }

    /// Content with `detected_lang` different from the user's configured language
    /// should be severely capped. V1 pipeline caps at 0.05 (well below 0.35 threshold).
    /// We detect the user's current language at runtime and set `detected_lang` to
    /// something definitively different.
    #[test]
    fn test_language_mismatch_severely_capped() {
        let db = test_db();
        let interest_embedding = vec![0.5_f32; 384];

        let interests = vec![context_engine::Interest {
            id: Some(1),
            topic: "rust".to_string(),
            weight: 1.0,
            embedding: Some(interest_embedding.clone()),
            source: context_engine::InterestSource::Explicit,
        }];
        let mut ace_ctx = ACEContext::default();
        ace_ctx.active_topics.push("rust".to_string());
        ace_ctx.topic_confidence.insert("rust".to_string(), 0.9);

        let ctx = ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace_ctx)
            .build();

        // Determine user's current language so we can pick a definitively different one
        let user_lang = crate::i18n::get_user_language();
        let mismatched_lang = if user_lang == "zz-test" { "en" } else { "zz-test" };

        // Content is about "rust" (matching interests + ACE) but in a mismatched language
        let input = ScoringInput {
            id: 1,
            title: "Rust async runtime performance improvements",
            url: Some("https://example.com/rust"),
            content: "rust tokio async await performance benchmarks",
            source_type: "hackernews",
            embedding: &interest_embedding,
            created_at: None,
            detected_lang: mismatched_lang,
        };
        let options = ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        };

        let result = score_item(&input, &ctx, &db, &options, None);

        // V1 pipeline applies: `combined_score.min(0.05)` for lang_mismatch
        assert!(
            result.top_score <= 0.05,
            "Language mismatch (user={}, content={}) should cap score at 0.05, got {}",
            user_lang, mismatched_lang, result.top_score
        );
        assert!(
            !result.relevant,
            "Language-mismatched content should never be relevant (score={})",
            result.top_score
        );
    }

    // ========================================================================
    // Existing unit tests
    // ========================================================================

    // Phase 2: Dependency prefix filter test
    #[test]
    fn test_dependency_prefix_filtered_from_seeding() {
        let topics = vec![
            "@radix-ui/react-select",
            "@types/node",
            "react",
            "typescript",
        ];
        let filtered: Vec<_> = topics
            .into_iter()
            .filter(|t| !t.starts_with('@') && !t.contains('/') && t.len() > 2)
            .collect();
        assert_eq!(filtered, vec!["react", "typescript"]);
    }
}
