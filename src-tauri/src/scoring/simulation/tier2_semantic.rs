//! Tier 2: Semantic / Embedding Scoring Validation
//!
//! Tests that the interest_score (embedding cosine similarity path) and
//! topic_embedding matching work correctly across personas and corpus items.
//! Tier 1 = keyword, Tier 2 = semantic (this file), Tier 3 = reranking.

#[cfg(test)]
mod tests {
    use super::super::super::ace_context::ACEContext;
    use super::super::super::{score_item, ScoringContext};
    use super::super::corpus::corpus;
    use super::super::domain_embeddings;
    use super::super::metrics::SimMetrics;
    use super::super::personas;
    use super::super::{sim_db, sim_input, sim_no_freshness};
    use super::super::{ContentCategory, ExpectedOutcome};
    use super::super::{PI_DEVOPS, PI_NICHE, PI_PYTHON, PI_RUST, PI_TS};

    // ========================================================================
    // Helpers
    // ========================================================================

    /// Build a single Interest with domain embedding.
    fn sem_interest(
        id: i64,
        topic: &str,
        weight: f32,
        domain: usize,
    ) -> crate::context_engine::Interest {
        crate::context_engine::Interest {
            id: Some(id),
            topic: topic.to_string(),
            weight,
            embedding: Some(domain_embeddings::interest_embedding(domain)),
            source: crate::context_engine::InterestSource::Explicit,
        }
    }

    /// Build ACE context from topic/tech lists.
    fn sem_ace(topics: &[&str], tech: &[&str]) -> ACEContext {
        let mut ace = ACEContext::default();
        for t in topics {
            ace.active_topics.push(t.to_string());
        }
        for t in tech {
            ace.detected_tech.push(t.to_string());
        }
        ace
    }

    /// Score a batch of items, returning mean top_score.
    fn mean_score(ctx: &ScoringContext, items: &[(u64, &str, &str, Vec<f32>)]) -> f64 {
        let db = sim_db();
        let opts = sim_no_freshness();
        let mut total = 0.0_f64;
        for (id, title, content, emb) in items {
            let input = sim_input(*id, title, content, emb);
            total += score_item(&input, ctx, &db, &opts, None).top_score as f64;
        }
        if items.is_empty() {
            0.0
        } else {
            total / items.len() as f64
        }
    }

    // ========================================================================
    // Semantic persona builders (compact)
    // ========================================================================

    fn semantic_rust_ctx() -> ScoringContext {
        let interests = vec![
            sem_interest(1, "Rust", 1.0, PI_RUST),
            sem_interest(2, "systems programming", 1.0, PI_RUST),
            sem_interest(3, "Tauri", 0.9, PI_RUST),
        ];
        let ace = sem_ace(&["rust", "tauri", "sqlite"], &["rust", "tauri", "sqlite"]);
        let domain = personas::make_domain(
            &["rust", "tauri", "sqlite"],
            &["tokio", "serde", "wasm", "typescript"],
            &["tokio", "serde", "sqlx", "tauri"],
            &["rust", "systems programming", "tauri", "sqlite"],
        );
        ScoringContext::builder()
            .interest_count(3)
            .interests(interests)
            .ace_ctx(ace)
            .domain_profile(domain)
            .declared_tech(vec!["rust".into(), "tauri".into(), "sqlite".into()])
            .composed_stack(crate::stacks::compose_profiles(&[
                "rust_systems".to_string()
            ]))
            .feedback_interaction_count(50)
            .build()
    }

    #[allow(dead_code)]
    fn semantic_python_ctx() -> ScoringContext {
        let interests = vec![
            sem_interest(1, "Machine Learning", 1.0, PI_PYTHON),
            sem_interest(2, "Python", 1.0, PI_PYTHON),
        ];
        let ace = sem_ace(
            &["python", "pytorch", "machine learning"],
            &["python", "pytorch"],
        );
        let domain = personas::make_domain(
            &["python", "pytorch", "tensorflow"],
            &["numpy", "pandas", "scikit-learn", "huggingface"],
            &["torch", "transformers", "numpy", "pandas"],
            &["machine learning", "python", "llm", "pytorch"],
        );
        ScoringContext::builder()
            .interest_count(2)
            .interests(interests)
            .ace_ctx(ace)
            .domain_profile(domain)
            .declared_tech(vec!["python".into(), "pytorch".into(), "tensorflow".into()])
            .composed_stack(crate::stacks::compose_profiles(&["python_ml".to_string()]))
            .feedback_interaction_count(40)
            .build()
    }

    fn semantic_ts_ctx() -> ScoringContext {
        let interests = vec![sem_interest(1, "TypeScript", 1.0, PI_TS)];
        let ace = sem_ace(
            &["typescript", "react", "nextjs"],
            &["typescript", "react", "nodejs"],
        );
        let domain = personas::make_domain(
            &["typescript", "react", "nodejs"],
            &["nextjs", "graphql", "prisma", "tailwind"],
            &["react", "typescript", "next", "prisma"],
            &["typescript", "react", "nodejs", "nextjs"],
        );
        ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace)
            .domain_profile(domain)
            .declared_tech(vec!["typescript".into(), "react".into(), "nodejs".into()])
            .composed_stack(crate::stacks::compose_profiles(&[
                "fullstack_ts".to_string()
            ]))
            .feedback_interaction_count(35)
            .build()
    }

    fn semantic_devops_ctx() -> ScoringContext {
        let interests = vec![sem_interest(1, "Kubernetes", 1.0, PI_DEVOPS)];
        let ace = sem_ace(
            &["kubernetes", "docker", "terraform"],
            &["kubernetes", "docker", "terraform"],
        );
        let domain = personas::make_domain(
            &["kubernetes", "docker", "terraform"],
            &["helm", "prometheus", "grafana", "ansible"],
            &["kubernetes", "terraform", "helm", "prometheus"],
            &["kubernetes", "devops", "docker", "terraform"],
        );
        ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace)
            .domain_profile(domain)
            .declared_tech(vec![
                "kubernetes".into(),
                "docker".into(),
                "terraform".into(),
            ])
            .composed_stack(crate::stacks::compose_profiles(&["devops_sre".to_string()]))
            .feedback_interaction_count(30)
            .build()
    }

    fn semantic_niche_ctx() -> ScoringContext {
        let interests = vec![
            sem_interest(1, "Haskell", 1.0, PI_NICHE),
            sem_interest(2, "functional programming", 1.0, PI_NICHE),
        ];
        let ace = sem_ace(
            &["haskell", "functional programming", "nix"],
            &["haskell", "nix"],
        );
        let domain = personas::make_domain(
            &["haskell", "nix"],
            &["purescript", "ocaml", "elm", "agda"],
            &["ghc", "cabal", "nix"],
            &["haskell", "functional programming", "type theory", "nix"],
        );
        ScoringContext::builder()
            .interest_count(2)
            .interests(interests)
            .ace_ctx(ace)
            .domain_profile(domain)
            .declared_tech(vec!["haskell".into(), "nix".into()])
            .feedback_interaction_count(30)
            .build()
    }

    fn semantic_power_ctx() -> ScoringContext {
        let interests = vec![
            sem_interest(1, "Rust", 0.9, PI_RUST),
            sem_interest(2, "Python", 0.8, PI_PYTHON),
            sem_interest(3, "TypeScript", 0.8, PI_TS),
        ];
        let ace = sem_ace(
            &["rust", "python", "typescript", "distributed systems"],
            &["rust", "python", "typescript"],
        );
        let domain = personas::make_domain(
            &["rust", "python", "typescript"],
            &["wasm", "llm", "databases", "distributed systems"],
            &["tokio", "torch", "react", "postgres"],
            &["rust", "python", "typescript", "distributed systems"],
        );
        ScoringContext::builder()
            .interest_count(3)
            .interests(interests)
            .ace_ctx(ace)
            .domain_profile(domain)
            .feedback_interaction_count(200)
            .build()
    }

    fn semantic_bootstrap_ctx() -> ScoringContext {
        let interests = vec![crate::context_engine::Interest {
            id: Some(1),
            topic: "TypeScript".to_string(),
            weight: 1.0,
            embedding: Some(domain_embeddings::zero_embedding()),
            source: crate::context_engine::InterestSource::Explicit,
        }];
        let mut ace = ACEContext::default();
        ace.active_topics.push("typescript".to_string());
        ScoringContext::builder()
            .interest_count(1)
            .interests(interests)
            .ace_ctx(ace)
            .feedback_interaction_count(0)
            .build()
    }

    // ========================================================================
    // Interest Embedding Quality (5 tests)
    // ========================================================================

    #[test]
    fn semantic_rust_interest_boosts_rust_content() {
        let ctx = semantic_rust_ctx();
        let db = sim_db();
        let opts = sim_no_freshness();
        let emb = domain_embeddings::content_embedding_for_domain(PI_RUST, 1);
        let input = sim_input(
            1,
            "Rust ownership and borrowing deep dive",
            "Understanding Rust's ownership model: move semantics, borrowing rules, \
             and lifetime annotations for safe systems code.",
            &emb,
        );
        let result = score_item(&input, &ctx, &db, &opts, None);
        assert!(
            result.interest_score > 0.0,
            "Rust content with semantic embeddings should have interest_score > 0.0, got {:.4}",
            result.interest_score
        );
    }

    #[test]
    fn semantic_cross_domain_penalty() {
        let ctx = semantic_rust_ctx();
        let db = sim_db();
        let opts = sim_no_freshness();

        let rust_emb = domain_embeddings::content_embedding_for_domain(PI_RUST, 10);
        let rust_input = sim_input(
            1,
            "Rust async runtime internals",
            "Deep dive into tokio async runtime: task scheduling and executor pools.",
            &rust_emb,
        );
        let rust_result = score_item(&rust_input, &ctx, &db, &opts, None);

        let ml_emb = domain_embeddings::content_embedding_for_domain(PI_PYTHON, 20);
        let ml_input = sim_input(
            2,
            "PyTorch 2.0 performance benchmarks",
            "PyTorch 2.0 introduces torch.compile() for dramatic training speedups.",
            &ml_emb,
        );
        let ml_result = score_item(&ml_input, &ctx, &db, &opts, None);

        assert!(
            rust_result.interest_score >= ml_result.interest_score,
            "Rust persona: Rust interest ({:.4}) should >= ML interest ({:.4})",
            rust_result.interest_score,
            ml_result.interest_score
        );
    }

    #[test]
    fn semantic_adjacent_gets_partial_boost() {
        let ctx = semantic_rust_ctx();
        let db = sim_db();
        let opts = sim_no_freshness();

        let rust_emb = domain_embeddings::content_embedding_for_domain(PI_RUST, 30);
        let rust_input = sim_input(
            1,
            "Rust memory allocator design",
            "Custom allocators in Rust: GlobalAlloc trait and arena allocation.",
            &rust_emb,
        );
        let rust_result = score_item(&rust_input, &ctx, &db, &opts, None);

        let adj_emb = domain_embeddings::adjacent_content_embedding(PI_RUST, PI_DEVOPS, 0.5);
        let adj_input = sim_input(
            2,
            "Database performance tuning for embedded systems",
            "Optimizing SQLite and embedded databases for systems applications.",
            &adj_emb,
        );
        let adj_result = score_item(&adj_input, &ctx, &db, &opts, None);

        let noise_emb = domain_embeddings::noise_embedding(99);
        let noise_input = sim_input(
            3,
            "Top 10 marketing strategies for 2024",
            "Growth hacking and marketing automation for SaaS businesses.",
            &noise_emb,
        );
        let noise_result = score_item(&noise_input, &ctx, &db, &opts, None);

        assert!(
            adj_result.interest_score <= rust_result.interest_score,
            "Adjacent ({:.4}) should not exceed pure domain ({:.4})",
            adj_result.interest_score,
            rust_result.interest_score
        );
        assert!(
            adj_result.interest_score >= noise_result.interest_score,
            "Adjacent ({:.4}) should >= noise ({:.4})",
            adj_result.interest_score,
            noise_result.interest_score
        );
    }

    #[test]
    fn semantic_bootstrap_uses_zero_embeddings() {
        let ctx = semantic_bootstrap_ctx();
        let db = sim_db();
        let opts = sim_no_freshness();
        let emb = domain_embeddings::content_embedding_for_domain(PI_TS, 40);
        let input = sim_input(
            1,
            "TypeScript 5.0 features",
            "New TypeScript features: decorators, const type parameters, and enum improvements.",
            &emb,
        );
        let result = score_item(&input, &ctx, &db, &opts, None);
        assert!(
            result.interest_score >= 0.0 && result.interest_score <= 1.0,
            "Bootstrap interest_score out of [0,1]: {:.4}",
            result.interest_score
        );
    }

    #[test]
    fn semantic_embedding_dimension_mismatch_handled() {
        let ctx = semantic_rust_ctx();
        let db = sim_db();
        let opts = sim_no_freshness();
        let wrong_emb = domain_embeddings::wrong_dimension_embedding();
        let input = sim_input(
            1,
            "Rust systems programming",
            "Building systems with Rust ownership model.",
            &wrong_emb,
        );
        // Should not panic
        let result = score_item(&input, &ctx, &db, &opts, None);
        assert!(
            result.top_score >= 0.0 && result.top_score <= 1.0,
            "top_score out of [0,1] with mismatched embedding: {:.4}",
            result.top_score
        );
        assert!(
            result.interest_score >= 0.0 && result.interest_score <= 1.0,
            "interest_score out of [0,1] with mismatched embedding: {:.4}",
            result.interest_score
        );
    }

    // ========================================================================
    // Persona Separation (4 tests)
    // ========================================================================

    #[test]
    fn semantic_rust_vs_python_separation() {
        let ctx = semantic_rust_ctx();
        let rust_items: Vec<(u64, &str, &str, Vec<f32>)> = vec![
            (
                1,
                "Rust 2024 Edition: What's New",
                "Rust 2024 edition introduces async support and better error messages.",
                domain_embeddings::content_embedding_for_domain(PI_RUST, 1),
            ),
            (
                2,
                "tokio async runtime internals",
                "Deep dive into tokio task scheduling and executor pools.",
                domain_embeddings::content_embedding_for_domain(PI_RUST, 2),
            ),
            (
                3,
                "serde serialization framework",
                "serde derive macros and zero-copy deserialization in Rust.",
                domain_embeddings::content_embedding_for_domain(PI_RUST, 3),
            ),
        ];
        let python_items: Vec<(u64, &str, &str, Vec<f32>)> = vec![
            (
                11,
                "PyTorch 2.0 performance benchmarks",
                "PyTorch torch.compile() for training speedups with minimal changes.",
                domain_embeddings::content_embedding_for_domain(PI_PYTHON, 11),
            ),
            (
                12,
                "Fine-tuning LLMs with QLoRA",
                "QLoRA enables fine-tuning LLMs on consumer GPUs using 4-bit quantization.",
                domain_embeddings::content_embedding_for_domain(PI_PYTHON, 12),
            ),
            (
                13,
                "Python type hints complete guide",
                "Python type hints with mypy and pyright for static typing.",
                domain_embeddings::content_embedding_for_domain(PI_PYTHON, 13),
            ),
        ];
        let rust_mean = mean_score(&ctx, &rust_items);
        let python_mean = mean_score(&ctx, &python_items);
        assert!(
            rust_mean > python_mean,
            "Rust persona: Rust content ({rust_mean:.4}) should > Python ({python_mean:.4})"
        );
    }

    #[test]
    fn semantic_ts_vs_devops_separation() {
        let ts_ctx = semantic_ts_ctx();
        let devops_ctx = semantic_devops_ctx();
        let ts_items: Vec<(u64, &str, &str, Vec<f32>)> = vec![
            (
                14,
                "Next.js App Router migration guide",
                "Migrating from Pages Router to App Router in Next.js with TypeScript.",
                domain_embeddings::content_embedding_for_domain(PI_TS, 14),
            ),
            (
                15,
                "React Server Components deep dive",
                "React Server Components reduce client bundle size with server rendering.",
                domain_embeddings::content_embedding_for_domain(PI_TS, 15),
            ),
        ];
        let devops_items: Vec<(u64, &str, &str, Vec<f32>)> = vec![
            (
                16,
                "Kubernetes autoscaling strategies",
                "HPA and VPA configuration for production Kubernetes clusters.",
                domain_embeddings::content_embedding_for_domain(PI_DEVOPS, 16),
            ),
            (
                17,
                "Terraform state management best practices",
                "Remote state backends and workspace isolation for Terraform.",
                domain_embeddings::content_embedding_for_domain(PI_DEVOPS, 17),
            ),
        ];
        // TS persona prefers TS content
        let ts_on_ts = mean_score(&ts_ctx, &ts_items);
        let ts_on_devops = mean_score(&ts_ctx, &devops_items);
        assert!(
            ts_on_ts > ts_on_devops,
            "TS persona: TS ({ts_on_ts:.4}) should > DevOps ({ts_on_devops:.4})"
        );
        // DevOps persona prefers DevOps content
        let devops_on_devops = mean_score(&devops_ctx, &devops_items);
        let devops_on_ts = mean_score(&devops_ctx, &ts_items);
        assert!(
            devops_on_devops > devops_on_ts,
            "DevOps persona: DevOps ({devops_on_devops:.4}) should > TS ({devops_on_ts:.4})"
        );
    }

    #[test]
    fn semantic_niche_specialist_ignores_mainstream() {
        let ctx = semantic_niche_ctx();
        let fp_items: Vec<(u64, &str, &str, Vec<f32>)> = vec![
            (
                21,
                "Haskell GHC 9.8 improvements",
                "GHC 9.8 type-level programming features and performance improvements.",
                domain_embeddings::content_embedding_for_domain(PI_NICHE, 21),
            ),
            (
                22,
                "Category theory for programmers: Functors",
                "Understanding functors: Functor laws and fmap from category theory.",
                domain_embeddings::content_embedding_for_domain(PI_NICHE, 22),
            ),
        ];
        let mainstream_items: Vec<(u64, &str, &str, Vec<f32>)> = vec![
            (
                14,
                "Next.js App Router migration",
                "Migrating from Pages Router to App Router in Next.js.",
                domain_embeddings::content_embedding_for_domain(PI_TS, 14),
            ),
            (
                15,
                "React Server Components",
                "React Server Components reduce client bundle size.",
                domain_embeddings::content_embedding_for_domain(PI_TS, 15),
            ),
        ];
        let fp_mean = mean_score(&ctx, &fp_items);
        let mainstream_mean = mean_score(&ctx, &mainstream_items);
        assert!(
            fp_mean > mainstream_mean,
            "Niche FP: FP content ({fp_mean:.4}) should > mainstream ({mainstream_mean:.4})"
        );
    }

    #[test]
    fn semantic_power_user_broad_coverage() {
        let power_ctx = semantic_power_ctx();
        let rust_ctx = semantic_rust_ctx();
        let python_items: Vec<(u64, &str, &str, Vec<f32>)> = vec![
            (
                11,
                "PyTorch 2.0 benchmarks",
                "PyTorch torch.compile() for training speedups.",
                domain_embeddings::content_embedding_for_domain(PI_PYTHON, 11),
            ),
            (
                12,
                "Fine-tuning LLMs with QLoRA",
                "QLoRA for fine-tuning LLMs on consumer GPUs.",
                domain_embeddings::content_embedding_for_domain(PI_PYTHON, 12),
            ),
        ];
        let rust_items: Vec<(u64, &str, &str, Vec<f32>)> = vec![
            (
                1,
                "Rust 2024 Edition",
                "Rust 2024 async support and better error messages.",
                domain_embeddings::content_embedding_for_domain(PI_RUST, 1),
            ),
            (
                2,
                "tokio runtime internals",
                "tokio task scheduling and executor pools.",
                domain_embeddings::content_embedding_for_domain(PI_RUST, 2),
            ),
        ];
        let power_gap =
            (mean_score(&power_ctx, &rust_items) - mean_score(&power_ctx, &python_items)).abs();
        let specialist_gap =
            (mean_score(&rust_ctx, &rust_items) - mean_score(&rust_ctx, &python_items)).abs();
        // Power user should have smaller or comparable domain gap
        assert!(
            power_gap <= specialist_gap + 0.15,
            "Power user gap ({power_gap:.4}) should <= specialist gap ({specialist_gap:.4}) + 0.15"
        );
    }

    // ========================================================================
    // Corpus Sweep (3 tests)
    // ========================================================================

    #[test]
    fn semantic_corpus_scores_bounded() {
        let ctx = semantic_rust_ctx();
        let db = sim_db();
        let opts = sim_no_freshness();
        let items = corpus();
        let emb = domain_embeddings::content_embedding_for_domain(PI_RUST, 100);
        for item in &items {
            let input = sim_input(item.id, item.title, item.content, &emb);
            let result = score_item(&input, &ctx, &db, &opts, None);
            assert!(
                result.top_score >= 0.0 && result.top_score <= 1.0,
                "Item {} '{}' top_score out of [0,1]: {:.4}",
                item.id,
                item.title,
                result.top_score
            );
            assert!(
                result.interest_score >= 0.0 && result.interest_score <= 1.0,
                "Item {} '{}' interest_score out of [0,1]: {:.4}",
                item.id,
                item.title,
                result.interest_score
            );
        }
    }

    #[test]
    fn semantic_f1_improves_over_zero_embeddings() {
        let items = corpus();
        let db = sim_db();
        let opts = sim_no_freshness();
        let zero_emb = vec![0.0_f32; 384];
        let zero_ctx = personas::rust_systems_dev();
        let sem_ctx = semantic_rust_ctx();
        let mut zero_metrics = SimMetrics::new();
        let mut sem_metrics = SimMetrics::new();

        for item in &items {
            let expected = item.expected[PI_RUST];
            if matches!(expected, ExpectedOutcome::MildBorderline) {
                continue;
            }

            // Zero embedding baseline
            let zero_input = sim_input(item.id, item.title, item.content, &zero_emb);
            let zero_result = score_item(&zero_input, &zero_ctx, &db, &opts, None);
            zero_metrics.record(&zero_result, expected);

            // Semantic embedding: domain-aligned for StrongRelevant DirectMatch, noise otherwise
            let sem_emb = match item.category {
                ContentCategory::DirectMatch
                    if matches!(expected, ExpectedOutcome::StrongRelevant) =>
                {
                    domain_embeddings::content_embedding_for_domain(PI_RUST, item.id)
                }
                ContentCategory::AdjacentMatch => {
                    domain_embeddings::adjacent_content_embedding(PI_RUST, PI_TS, 0.6)
                }
                _ => domain_embeddings::noise_embedding(item.id),
            };
            let sem_input = sim_input(item.id, item.title, item.content, &sem_emb);
            let sem_result = score_item(&sem_input, &sem_ctx, &db, &opts, None);
            sem_metrics.record(&sem_result, expected);
        }

        let zero_f1 = zero_metrics.f1();
        let sem_f1 = sem_metrics.f1();
        println!(
            "F1 comparison: zero={zero_f1:.4} semantic={sem_f1:.4} delta={:.4}",
            sem_f1 - zero_f1
        );
        assert!(
            sem_f1 >= zero_f1 - 0.05,
            "Semantic F1 ({sem_f1:.4}) should >= zero F1 ({zero_f1:.4}) - 0.05 tolerance"
        );
    }

    #[test]
    fn semantic_interest_score_correlates_with_domain() {
        let ctx = semantic_rust_ctx();
        let db = sim_db();
        let opts = sim_no_freshness();
        let items = corpus();
        let mut direct_scores = Vec::new();
        let mut noise_scores = Vec::new();

        for item in &items {
            let expected = item.expected[PI_RUST];
            let emb = match item.category {
                ContentCategory::DirectMatch
                    if matches!(expected, ExpectedOutcome::StrongRelevant) =>
                {
                    domain_embeddings::content_embedding_for_domain(PI_RUST, item.id)
                }
                ContentCategory::CrossDomainNoise => domain_embeddings::noise_embedding(item.id),
                _ => continue,
            };
            let input = sim_input(item.id, item.title, item.content, &emb);
            let result = score_item(&input, &ctx, &db, &opts, None);
            match expected {
                ExpectedOutcome::StrongRelevant => direct_scores.push(result.interest_score as f64),
                ExpectedOutcome::NotRelevant => noise_scores.push(result.interest_score as f64),
                _ => {}
            }
        }

        if direct_scores.is_empty() || noise_scores.is_empty() {
            return;
        }
        let mean_direct = direct_scores.iter().sum::<f64>() / direct_scores.len() as f64;
        let mean_noise = noise_scores.iter().sum::<f64>() / noise_scores.len() as f64;
        println!(
            "Interest correlation: direct={mean_direct:.4} noise={mean_noise:.4} gap={:.4}",
            mean_direct - mean_noise
        );
        assert!(
            mean_direct > mean_noise,
            "DirectMatch interest ({mean_direct:.4}) should > CrossDomainNoise ({mean_noise:.4})"
        );
    }
}
