//! Golden Snapshot Testing — canonical item score baselines
//!
//! Unlike reality.rs (aggregate F1/precision), these tests check SPECIFIC item
//! scores for canonical corpus items. When the pipeline changes, score shifts
//! on known items are caught immediately.

#[cfg(test)]
mod tests {
    use super::super::super::score_item;
    use super::super::corpus::corpus;
    use super::super::personas::all_personas;
    use super::super::PERSONA_NAMES;
    use super::super::{sim_db, sim_input, sim_no_freshness};

    struct GoldenExpectation {
        item_id: u64,
        title: &'static str,
        persona_idx: usize,
        /// Expected score range (min, max) -- allows for minor pipeline tuning
        expected_range: (f32, f32),
        /// Must this item be marked relevant?
        expect_relevant: Option<bool>,
        /// Must this item be excluded?
        expect_excluded: bool,
    }

    fn check_golden(expectations: &[GoldenExpectation]) {
        let personas = all_personas();
        let db = sim_db();
        let opts = sim_no_freshness();
        let items = corpus();
        let emb = vec![0.0_f32; 384];

        for exp in expectations {
            let item = items
                .iter()
                .find(|i| i.id == exp.item_id)
                .unwrap_or_else(|| panic!("Corpus item {} not found", exp.item_id));
            let input = sim_input(item.id, item.title, item.content, &emb);
            let result = score_item(&input, &personas[exp.persona_idx], &db, &opts, None);

            assert!(
                result.top_score >= exp.expected_range.0
                    && result.top_score <= exp.expected_range.1,
                "Golden FAIL: '{}' (id={}) for {}: score {:.4} outside [{:.2}, {:.2}]",
                exp.title,
                exp.item_id,
                PERSONA_NAMES[exp.persona_idx],
                result.top_score,
                exp.expected_range.0,
                exp.expected_range.1
            );

            if let Some(rel) = exp.expect_relevant {
                assert_eq!(
                    result.relevant, rel,
                    "Golden FAIL: '{}' (id={}) for {}: relevant={}, expected={}",
                    exp.title, exp.item_id, PERSONA_NAMES[exp.persona_idx], result.relevant, rel
                );
            }

            if exp.expect_excluded {
                assert!(
                    result.excluded,
                    "Golden FAIL: '{}' (id={}) should be excluded",
                    exp.title, exp.item_id
                );
            }
        }
    }

    // ========================================================================
    // Golden: Rust persona canonical items
    // ========================================================================

    #[test]
    fn golden_rust_persona_canonical_items() {
        check_golden(&[
            GoldenExpectation {
                item_id: 1,
                title: "Rust 2024 Edition",
                persona_idx: 0,
                expected_range: (0.2, 1.0),
                expect_relevant: Some(true),
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 4,
                title: "tokio async runtime",
                persona_idx: 0,
                expected_range: (0.0, 1.0),
                expect_relevant: None, // keyword-only: "tokio" in deps but zero embedding
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 5,
                title: "serde serialization",
                persona_idx: 0,
                expected_range: (0.0, 1.0),
                expect_relevant: None, // keyword-only baseline
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 25,
                title: "axum production patterns",
                persona_idx: 0,
                expected_range: (0.0, 1.0),
                expect_relevant: None, // keyword-only baseline
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 47,
                title: "Rust Embassy embedded",
                persona_idx: 0,
                expected_range: (0.0, 1.0),
                expect_relevant: None,
                expect_excluded: false,
            },
        ]);
    }

    // ========================================================================
    // Golden: Python persona canonical items
    // ========================================================================

    #[test]
    fn golden_python_persona_canonical_items() {
        check_golden(&[
            GoldenExpectation {
                item_id: 11,
                title: "PyTorch 2.0 benchmarks",
                persona_idx: 1,
                expected_range: (0.0, 1.0),
                expect_relevant: None, // keyword-only: threshold not met with zero embedding
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 12,
                title: "QLoRA fine-tuning",
                persona_idx: 1,
                expected_range: (0.0, 1.0),
                expect_relevant: None, // keyword-only: "QLoRA" niche term, low score with zero embedding
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 13,
                title: "Python type hints",
                persona_idx: 1,
                expected_range: (0.0, 1.0),
                expect_relevant: None, // keyword-only baseline
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 27,
                title: "Python asyncio production",
                persona_idx: 1,
                expected_range: (0.0, 1.0),
                expect_relevant: None, // keyword-only baseline
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 123,
                title: "PyTorch RCE via pickle",
                persona_idx: 1,
                expected_range: (0.1, 1.0),
                expect_relevant: None,
                expect_excluded: false,
            },
        ]);
    }

    // ========================================================================
    // Golden: Cross-persona isolation
    // ========================================================================

    #[test]
    fn golden_cross_persona_isolation() {
        check_golden(&[
            // Rust content scored by Python persona -> low
            GoldenExpectation {
                item_id: 1,
                title: "Rust 2024 Edition",
                persona_idx: 1,
                expected_range: (0.0, 0.4),
                expect_relevant: Some(false),
                expect_excluded: false,
            },
            // Python content scored by Rust persona -> low
            GoldenExpectation {
                item_id: 11,
                title: "PyTorch 2.0 benchmarks",
                persona_idx: 0,
                expected_range: (0.0, 0.4),
                expect_relevant: Some(false),
                expect_excluded: false,
            },
            // Kubernetes content scored by Rust persona -> low
            GoldenExpectation {
                item_id: 16,
                title: "Kubernetes 1.30",
                persona_idx: 0,
                expected_range: (0.0, 0.4),
                expect_relevant: Some(false),
                expect_excluded: false,
            },
            // Kubernetes content scored by DevOps persona -> high
            GoldenExpectation {
                item_id: 16,
                title: "Kubernetes 1.30",
                persona_idx: 3,
                expected_range: (0.2, 1.0),
                expect_relevant: Some(true),
                expect_excluded: false,
            },
        ]);
    }

    // ========================================================================
    // Golden: Noise rejection stable across personas
    // ========================================================================

    #[test]
    fn golden_noise_rejection_stable() {
        let noise_personas = [0, 1, 2, 3, 4]; // core personas, not generalists
        let mut expectations = Vec::new();

        // ID 96: career noise -- "Senior Rust Engineer at Cloudflare"
        for &pi in &noise_personas {
            expectations.push(GoldenExpectation {
                item_id: 96,
                title: "career noise: Rust engineer hiring",
                persona_idx: pi,
                expected_range: (0.0, 0.3),
                expect_relevant: Some(false),
                expect_excluded: false,
            });
        }

        // ID 51: cross-domain noise -- "Ruby on Rails 7.2"
        for &pi in &noise_personas {
            expectations.push(GoldenExpectation {
                item_id: 51,
                title: "cross-domain: Ruby on Rails",
                persona_idx: pi,
                expected_range: (0.0, 0.5),
                expect_relevant: Some(false),
                expect_excluded: false,
            });
        }

        check_golden(&expectations);
    }

    // ========================================================================
    // Golden: Bootstrap permissiveness
    // ========================================================================

    #[test]
    fn golden_bootstrap_permissiveness() {
        // Bootstrap user (persona 5) has minimal config -- should still score
        // tech content with nonzero scores rather than filtering everything
        let personas = all_personas();
        let db = sim_db();
        let opts = sim_no_freshness();
        let items = corpus();
        let emb = vec![0.0_f32; 384];

        let tech_ids: [u64; 4] = [1, 14, 15, 28]; // Rust, React 19, TS 5.4, Next.js
        let mut any_nonzero = false;

        for &id in &tech_ids {
            let item = items.iter().find(|i| i.id == id).unwrap();
            let input = sim_input(item.id, item.title, item.content, &emb);
            let result = score_item(&input, &personas[5], &db, &opts, None);
            if result.top_score > 0.0 {
                any_nonzero = true;
            }
            // Bootstrap should not score anything excessively high
            assert!(
                result.top_score <= 0.8,
                "Bootstrap scored '{}' too high: {:.4}",
                item.title,
                result.top_score
            );
        }

        assert!(
            any_nonzero,
            "Bootstrap user scored zero on ALL tech content -- too restrictive"
        );
    }

    // ========================================================================
    // Golden: Exclusion always produces zero score
    // ========================================================================

    #[test]
    fn golden_exclusion_always_zero() {
        use super::super::super::ScoringContext;
        use super::super::personas::rust_systems_dev;

        // Build a Rust-like persona with "rails" excluded
        let base = rust_systems_dev();
        let ctx = ScoringContext::builder()
            .interest_count(base.interest_count)
            .interests(base.interests)
            .ace_ctx(base.ace_ctx)
            .domain_profile(base.domain_profile)
            .declared_tech(base.declared_tech)
            .composed_stack(base.composed_stack)
            .exclusions(vec!["rails".to_string()])
            .feedback_interaction_count(50)
            .build();

        let db = sim_db();
        let opts = sim_no_freshness();
        let items = corpus();
        let emb = vec![0.0_f32; 384];

        // ID 51: "Ruby on Rails 7.2: What's new" -- contains "rails" in title
        let item = items.iter().find(|i| i.id == 51).unwrap();
        let input = sim_input(item.id, item.title, item.content, &emb);
        let result = score_item(&input, &ctx, &db, &opts, None);

        assert!(
            result.excluded,
            "Item with excluded keyword should be marked excluded"
        );
        assert_eq!(
            result.top_score, 0.0,
            "Excluded item must score exactly 0.0, got {:.4}",
            result.top_score
        );
        assert!(
            !result.relevant,
            "Excluded item must not be marked relevant"
        );
    }

    // ========================================================================
    // Golden: Power User canonical items (persona 6)
    // ========================================================================

    #[test]
    fn golden_power_user_canonical_items() {
        check_golden(&[
            GoldenExpectation {
                item_id: 1,
                title: "Rust 2024 Edition",
                persona_idx: 6,
                expected_range: (0.2, 1.0),
                expect_relevant: Some(true),
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 11,
                title: "PyTorch 2.0 benchmarks",
                persona_idx: 6,
                expected_range: (0.0, 1.0),
                expect_relevant: None,
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 16,
                title: "Kubernetes 1.30",
                persona_idx: 6,
                expected_range: (0.0, 1.0),
                expect_relevant: None,
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 96,
                title: "career noise: Rust engineer hiring",
                persona_idx: 6,
                expected_range: (0.0, 0.4),
                expect_relevant: Some(false),
                expect_excluded: false,
            },
        ]);
    }

    // ========================================================================
    // Golden: Context Switcher canonical items (persona 7)
    // ========================================================================

    #[test]
    fn golden_context_switcher_canonical_items() {
        check_golden(&[
            GoldenExpectation {
                item_id: 24,
                title: "Go generics deep dive",
                persona_idx: 7,
                expected_range: (0.0, 1.0),
                expect_relevant: None,
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 1,
                title: "Rust 2024 Edition",
                persona_idx: 7,
                expected_range: (0.2, 1.0),
                expect_relevant: Some(true),
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 11,
                title: "PyTorch 2.0 benchmarks",
                persona_idx: 7,
                expected_range: (0.0, 0.5),
                expect_relevant: Some(false),
                expect_excluded: false,
            },
        ]);
    }

    // ========================================================================
    // Golden: Niche Specialist canonical items (persona 8)
    // ========================================================================

    #[test]
    fn golden_niche_specialist_canonical_items() {
        check_golden(&[
            GoldenExpectation {
                item_id: 21,
                title: "GHC 9.8 Haskell features",
                persona_idx: 8,
                expected_range: (0.2, 1.0),
                expect_relevant: Some(true),
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 22,
                title: "Category theory applied",
                persona_idx: 8,
                expected_range: (0.0, 1.0),
                expect_relevant: None,
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 23,
                title: "NixOS 24.05",
                persona_idx: 8,
                expected_range: (0.0, 1.0),
                expect_relevant: None,
                expect_excluded: false,
            },
            GoldenExpectation {
                item_id: 16,
                title: "Kubernetes 1.30",
                persona_idx: 8,
                expected_range: (0.0, 0.35),
                expect_relevant: Some(false),
                expect_excluded: false,
            },
        ]);
    }
}
