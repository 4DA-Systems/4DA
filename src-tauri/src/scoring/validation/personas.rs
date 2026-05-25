// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Validation Personas — 10 developer archetypes for scoring validation
//!
//! Each persona defines interests, tech stack, dependencies, expected topics,
//! and anti-topics. These are used to build ScoringContexts and auto-judge
//! whether scored items are truly relevant to the persona.

use std::collections::HashSet;

use super::super::ace_context::ACEContext;
use super::super::ScoringContext;

// ============================================================================
// Persona Definition
// ============================================================================

/// A simulated developer profile for validation.
pub struct SimulatedPersona {
    pub name: &'static str,
    pub interests: Vec<(&'static str, f32)>,
    pub tech_stack: Vec<&'static str>,
    pub dependencies: Vec<&'static str>,
    pub role: &'static str,
    pub expected_topics: Vec<&'static str>,
    pub anti_topics: Vec<&'static str>,
}

// ============================================================================
// Persona Builders → ScoringContext
// ============================================================================

fn make_interests(topics: &[(&str, f32)]) -> Vec<crate::context_engine::Interest> {
    let emb = vec![0.5_f32; crate::EMBEDDING_DIMS];
    topics
        .iter()
        .enumerate()
        .map(|(i, (t, w))| crate::context_engine::Interest {
            id: Some((i + 1) as i64),
            topic: t.to_string(),
            weight: *w,
            embedding: Some(emb.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        })
        .collect()
}

fn make_domain(
    primary: &[&str],
    adjacent: &[&str],
    deps: &[&str],
    interest_topics: &[&str],
) -> crate::domain_profile::DomainProfile {
    let ps: HashSet<String> = primary
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    let adj: HashSet<String> = adjacent
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    let mut all = ps.clone();
    all.extend(adj.clone());
    crate::domain_profile::DomainProfile {
        primary_stack: ps,
        adjacent_tech: adj,
        all_tech: all,
        dependency_names: deps.iter().map(std::string::ToString::to_string).collect(),
        interest_topics: interest_topics
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
        domain_concerns: HashSet::new(),
        ace_promoted_tech: HashSet::new(),
    }
}

/// Build a ScoringContext from a SimulatedPersona.
pub fn persona_to_context(persona: &SimulatedPersona) -> ScoringContext {
    let interests = make_interests(&persona.interests);

    let mut ace = ACEContext::default();
    for t in &persona.tech_stack {
        ace.active_topics.push(t.to_lowercase());
        ace.detected_tech.push(t.to_lowercase());
    }
    // Add expected topics as active topics (broader coverage)
    for t in &persona.expected_topics {
        let lower = t.to_lowercase();
        if !ace.active_topics.contains(&lower) {
            ace.active_topics.push(lower);
        }
    }

    let primary: Vec<&str> = persona.tech_stack.clone();
    let adjacent: Vec<&str> = persona
        .expected_topics
        .iter()
        .filter(|t| !persona.tech_stack.contains(t))
        .take(5)
        .copied()
        .collect();
    let interest_topics: Vec<&str> = persona.interests.iter().map(|(t, _)| *t).collect();

    let domain = make_domain(&primary, &adjacent, &persona.dependencies, &interest_topics);

    let declared_tech: Vec<String> = persona
        .tech_stack
        .iter()
        .take(4)
        .map(|s| s.to_lowercase())
        .collect();

    let exclusions: Vec<String> = persona
        .anti_topics
        .iter()
        .map(std::string::ToString::to_string)
        .collect();

    ScoringContext::builder()
        .interest_count(persona.interests.len())
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(declared_tech)
        .exclusions(exclusions)
        .feedback_interaction_count(50)
        .build()
}

// ============================================================================
// 10 Validation Personas
// ============================================================================

/// Persona 1: Rust Systems Developer
fn rust_systems_dev() -> SimulatedPersona {
    SimulatedPersona {
        name: "Rust Systems Dev",
        interests: vec![
            ("Rust", 1.0),
            ("systems programming", 1.0),
            ("async runtime", 0.9),
            ("memory safety", 0.8),
            ("performance optimization", 0.7),
        ],
        tech_stack: vec!["rust", "tokio", "serde", "wasm"],
        dependencies: vec!["tokio", "serde", "axum", "sqlx", "tracing"],
        role: "systems",
        expected_topics: vec![
            "rust",
            "cargo",
            "tokio",
            "async",
            "memory safety",
            "ownership",
            "borrow checker",
            "wasm",
            "systems programming",
            "performance",
            "concurrency",
            "serde",
            "trait",
            "lifetime",
        ],
        anti_topics: vec![
            "javascript",
            "css",
            "react",
            "marketing",
            "seo",
            "cryptocurrency",
            "nft",
            "blockchain",
        ],
    }
}

/// Persona 2: React Frontend Developer
fn react_frontend_dev() -> SimulatedPersona {
    SimulatedPersona {
        name: "React Frontend Dev",
        interests: vec![
            ("React", 1.0),
            ("TypeScript", 1.0),
            ("CSS", 0.8),
            ("UI/UX", 0.7),
            ("web performance", 0.8),
        ],
        tech_stack: vec!["react", "typescript", "nextjs", "tailwind"],
        dependencies: vec!["react", "next", "typescript", "tailwindcss", "prisma"],
        role: "frontend",
        expected_topics: vec![
            "react",
            "typescript",
            "nextjs",
            "css",
            "tailwind",
            "hooks",
            "components",
            "rendering",
            "ssr",
            "web vitals",
            "accessibility",
            "ui",
            "ux",
            "frontend",
            "javascript",
        ],
        anti_topics: vec![
            "kernel",
            "cuda",
            "kubernetes",
            "terraform",
            "cryptocurrency",
            "blockchain",
        ],
    }
}

/// Persona 3: ML Engineer
fn ml_engineer() -> SimulatedPersona {
    SimulatedPersona {
        name: "ML Engineer",
        interests: vec![
            ("machine learning", 1.0),
            ("Python", 1.0),
            ("PyTorch", 0.9),
            ("transformers", 0.9),
            ("MLOps", 0.7),
        ],
        tech_stack: vec!["python", "pytorch", "cuda", "huggingface"],
        dependencies: vec!["torch", "transformers", "numpy", "pandas", "scikit-learn"],
        role: "ml",
        expected_topics: vec![
            "python",
            "pytorch",
            "machine learning",
            "deep learning",
            "llm",
            "transformer",
            "gpu",
            "cuda",
            "training",
            "inference",
            "model",
            "neural network",
            "fine-tuning",
            "embedding",
            "mlops",
        ],
        anti_topics: vec![
            "css",
            "react",
            "frontend",
            "mobile",
            "ios",
            "cryptocurrency",
            "nft",
        ],
    }
}

/// Persona 4: DevOps/Platform Engineer
fn devops_platform() -> SimulatedPersona {
    SimulatedPersona {
        name: "DevOps/Platform Engineer",
        interests: vec![
            ("Kubernetes", 1.0),
            ("Terraform", 0.9),
            ("CI/CD", 0.9),
            ("monitoring", 0.8),
            ("cloud infrastructure", 0.8),
        ],
        tech_stack: vec!["kubernetes", "docker", "terraform", "prometheus"],
        dependencies: vec!["helm", "terraform", "prometheus", "grafana", "ansible"],
        role: "devops",
        expected_topics: vec![
            "kubernetes",
            "docker",
            "terraform",
            "ci/cd",
            "monitoring",
            "observability",
            "prometheus",
            "grafana",
            "helm",
            "aws",
            "gcp",
            "cloud",
            "infrastructure",
            "deployment",
            "container",
        ],
        anti_topics: vec!["css", "react", "ui", "game", "unity", "cryptocurrency"],
    }
}

/// Persona 5: Mobile Developer
fn mobile_dev() -> SimulatedPersona {
    SimulatedPersona {
        name: "Mobile Developer",
        interests: vec![
            ("Swift", 0.9),
            ("Kotlin", 0.9),
            ("React Native", 1.0),
            ("Flutter", 0.8),
            ("mobile UX", 0.7),
        ],
        tech_stack: vec!["react native", "swift", "kotlin", "flutter"],
        dependencies: vec!["react-native", "expo", "swift", "kotlin"],
        role: "mobile",
        expected_topics: vec![
            "mobile",
            "ios",
            "android",
            "swift",
            "kotlin",
            "react native",
            "flutter",
            "expo",
            "app store",
            "push notification",
            "gesture",
            "navigation",
            "responsive",
        ],
        anti_topics: vec![
            "kernel",
            "cuda",
            "kubernetes",
            "terraform",
            "cryptocurrency",
            "blockchain",
            "backend",
        ],
    }
}

/// Persona 6: Security Engineer
fn security_engineer() -> SimulatedPersona {
    SimulatedPersona {
        name: "Security Engineer",
        interests: vec![
            ("vulnerability", 1.0),
            ("cryptography", 0.9),
            ("penetration testing", 0.8),
            ("supply chain security", 0.9),
            ("zero trust", 0.7),
        ],
        tech_stack: vec!["security", "cryptography", "pentest", "sast"],
        dependencies: vec!["openssl", "ring", "rustls", "nmap"],
        role: "security",
        expected_topics: vec![
            "security",
            "vulnerability",
            "cve",
            "exploit",
            "authentication",
            "encryption",
            "tls",
            "pentest",
            "supply chain",
            "zero trust",
            "audit",
            "compliance",
            "malware",
            "xss",
            "injection",
        ],
        anti_topics: vec!["css", "ui", "design", "marketing", "game", "pricing"],
    }
}

/// Persona 7: Data Engineer
fn data_engineer() -> SimulatedPersona {
    SimulatedPersona {
        name: "Data Engineer",
        interests: vec![
            ("SQL", 1.0),
            ("data pipelines", 1.0),
            ("Spark", 0.8),
            ("Kafka", 0.9),
            ("dbt", 0.8),
        ],
        tech_stack: vec!["sql", "spark", "kafka", "dbt"],
        dependencies: vec!["pyspark", "kafka", "dbt-core", "airflow", "snowflake"],
        role: "data",
        expected_topics: vec![
            "sql",
            "data pipeline",
            "etl",
            "spark",
            "kafka",
            "dbt",
            "data warehouse",
            "streaming",
            "batch processing",
            "schema",
            "parquet",
            "airflow",
            "snowflake",
            "data lake",
        ],
        anti_topics: vec![
            "css",
            "react",
            "frontend",
            "mobile",
            "game",
            "cryptocurrency",
        ],
    }
}

/// Persona 8: Indie Hacker / Solo SaaS Developer
fn indie_hacker() -> SimulatedPersona {
    SimulatedPersona {
        name: "Indie Hacker",
        interests: vec![
            ("SaaS", 1.0),
            ("pricing strategy", 0.9),
            ("growth hacking", 0.8),
            ("marketing", 0.8),
            ("revenue", 0.9),
        ],
        tech_stack: vec!["typescript", "stripe", "vercel", "postgres"],
        dependencies: vec!["stripe", "next", "supabase", "resend"],
        role: "indie",
        expected_topics: vec![
            "saas",
            "pricing",
            "revenue",
            "growth",
            "marketing",
            "launch",
            "conversion",
            "churn",
            "mrr",
            "startup",
            "bootstrapped",
            "stripe",
            "landing page",
        ],
        anti_topics: vec![
            "kernel",
            "cuda",
            "kubernetes",
            "terraform",
            "compiler",
            "assembly",
        ],
    }
}

/// Persona 9: Game Developer
fn game_dev() -> SimulatedPersona {
    SimulatedPersona {
        name: "Game Developer",
        interests: vec![
            ("Unity", 0.9),
            ("Unreal Engine", 0.8),
            ("Godot", 0.9),
            ("graphics programming", 1.0),
            ("physics simulation", 0.7),
        ],
        tech_stack: vec!["unity", "godot", "opengl", "vulkan"],
        dependencies: vec!["unity", "godot", "glfw", "sdl2"],
        role: "gamedev",
        expected_topics: vec![
            "game",
            "unity",
            "unreal",
            "godot",
            "graphics",
            "shader",
            "rendering",
            "physics",
            "game engine",
            "opengl",
            "vulkan",
            "3d",
            "animation",
            "game design",
        ],
        anti_topics: vec![
            "kubernetes",
            "terraform",
            "ci/cd",
            "data pipeline",
            "marketing",
            "seo",
        ],
    }
}

/// Persona 10: Backend Java/Go Developer
fn backend_java_go() -> SimulatedPersona {
    SimulatedPersona {
        name: "Backend Java/Go Dev",
        interests: vec![
            ("microservices", 1.0),
            ("gRPC", 0.9),
            ("databases", 0.9),
            ("Go", 0.9),
            ("Java", 0.8),
        ],
        tech_stack: vec!["go", "java", "grpc", "postgres"],
        dependencies: vec!["gin", "spring", "grpc", "postgres", "redis"],
        role: "backend",
        expected_topics: vec![
            "go",
            "golang",
            "java",
            "spring",
            "microservices",
            "grpc",
            "api",
            "database",
            "postgres",
            "redis",
            "kafka",
            "backend",
            "rest",
            "distributed",
        ],
        anti_topics: vec!["css", "react", "frontend", "mobile", "ios", "game", "unity"],
    }
}

// ============================================================================
// Collector
// ============================================================================

/// Returns all 10 validation personas.
pub fn all_validation_personas() -> Vec<SimulatedPersona> {
    vec![
        rust_systems_dev(),
        react_frontend_dev(),
        ml_engineer(),
        devops_platform(),
        mobile_dev(),
        security_engineer(),
        data_engineer(),
        indie_hacker(),
        game_dev(),
        backend_java_go(),
    ]
}

/// Returns all 10 personas as (SimulatedPersona, ScoringContext) pairs.
pub fn all_validation_contexts() -> Vec<(SimulatedPersona, ScoringContext)> {
    all_validation_personas()
        .into_iter()
        .map(|p| {
            let ctx = persona_to_context(&p);
            (p, ctx)
        })
        .collect()
}

// ============================================================================
// Tests — validate persona construction and topic-based scoring judgments
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::pipeline::{score_item, ScoringInput, ScoringOptions};
    use crate::test_utils::{seed_embedding, test_db};

    fn no_opts() -> ScoringOptions {
        ScoringOptions {
            apply_freshness: false,
            apply_signals: false,
            trend_topics: vec![],
        }
    }

    #[test]
    fn all_personas_produce_valid_contexts() {
        let contexts = all_validation_contexts();
        assert_eq!(contexts.len(), 10, "should have exactly 10 personas");

        for (persona, ctx) in &contexts {
            assert!(!persona.name.is_empty(), "persona name must not be empty");
            assert!(
                ctx.interest_count > 0,
                "{}: must have interests",
                persona.name
            );
            assert!(
                !ctx.interests.is_empty(),
                "{}: interests vec must not be empty",
                persona.name
            );
            assert!(
                !ctx.ace_ctx.active_topics.is_empty(),
                "{}: ACE active_topics must not be empty",
                persona.name
            );
            assert!(
                !ctx.domain_profile.primary_stack.is_empty(),
                "{}: domain primary_stack must not be empty",
                persona.name
            );
        }
    }

    #[test]
    fn personas_have_distinct_tech_stacks() {
        let contexts = all_validation_contexts();
        let stacks: Vec<&HashSet<String>> = contexts
            .iter()
            .map(|(_, ctx)| &ctx.domain_profile.primary_stack)
            .collect();

        for i in 0..stacks.len() {
            for j in (i + 1)..stacks.len() {
                assert_ne!(
                    stacks[i], stacks[j],
                    "personas {} and {} have identical primary stacks",
                    contexts[i].0.name, contexts[j].0.name
                );
            }
        }
    }

    #[test]
    fn rust_persona_scores_rust_content_high() {
        let db = test_db();
        let persona = rust_systems_dev();
        let ctx = persona_to_context(&persona);
        let emb = seed_embedding("rust:tokio-async-runtime");

        let input = ScoringInput {
            id: 1,
            title: "Tokio 2.0: major async runtime redesign for Rust",
            url: None,
            content: "The Rust async ecosystem gets a major upgrade with Tokio 2.0, \
                      featuring improved task scheduling, better memory safety guarantees, \
                      and zero-cost abstractions for concurrent programming.",
            source_type: "hackernews",
            embedding: &emb,
            created_at: None,
            detected_lang: "en",
            source_tags: &[],
            tags_json: None,
            feed_origin: None,
        };

        let result = score_item(&input, &ctx, &db, &no_opts(), None);
        assert!(
            result.score > 0.3,
            "Rust persona should score Rust/Tokio content high, got {}",
            result.score
        );
    }

    #[test]
    fn rust_persona_rejects_anti_topics() {
        let db = test_db();
        let persona = rust_systems_dev();
        let ctx = persona_to_context(&persona);
        let emb = seed_embedding("crypto:nft-marketplace");

        let input = ScoringInput {
            id: 2,
            title: "Building an NFT Marketplace with Solidity and React",
            url: None,
            content: "Learn how to create a full-stack NFT marketplace using \
                      Ethereum smart contracts, Solidity, React frontend, \
                      and cryptocurrency wallet integration.",
            source_type: "hackernews",
            embedding: &emb,
            created_at: None,
            detected_lang: "en",
            source_tags: &[],
            tags_json: None,
            feed_origin: None,
        };

        let result = score_item(&input, &ctx, &db, &no_opts(), None);
        assert!(
            result.score < 0.3,
            "Rust persona should reject crypto/NFT content, got {}",
            result.score
        );
    }

    #[test]
    fn ml_engineer_scores_ml_content_high() {
        let db = test_db();
        let persona = ml_engineer();
        let ctx = persona_to_context(&persona);
        let emb = seed_embedding("ml:transformer-attention");

        let input = ScoringInput {
            id: 3,
            title: "Scaling Transformer Models: Attention Mechanism Optimizations",
            url: None,
            content: "New research on efficient attention patterns for large language \
                      models, including flash attention, multi-query attention, and \
                      PyTorch implementation with CUDA kernel optimizations.",
            source_type: "arxiv",
            embedding: &emb,
            created_at: None,
            detected_lang: "en",
            source_tags: &[],
            tags_json: None,
            feed_origin: None,
        };

        let result = score_item(&input, &ctx, &db, &no_opts(), None);
        assert!(
            result.score > 0.3,
            "ML engineer should score transformer/PyTorch content high, got {}",
            result.score
        );
    }

    #[test]
    fn cross_persona_discrimination() {
        let db = test_db();
        let emb = seed_embedding("react:server-components");

        let react_item = ScoringInput {
            id: 4,
            title: "React Server Components: A Deep Dive into Streaming SSR",
            url: None,
            content: "Understanding Next.js App Router, React Server Components, \
                      Suspense boundaries, and TypeScript integration for modern \
                      full-stack web applications.",
            source_type: "devto",
            embedding: &emb,
            created_at: None,
            detected_lang: "en",
            source_tags: &[],
            tags_json: None,
            feed_origin: None,
        };

        let react_persona = react_frontend_dev();
        let react_ctx = persona_to_context(&react_persona);
        let react_score = score_item(&react_item, &react_ctx, &db, &no_opts(), None).score;

        let security_persona = security_engineer();
        let security_ctx = persona_to_context(&security_persona);
        let security_score = score_item(&react_item, &security_ctx, &db, &no_opts(), None).score;

        assert!(
            react_score > security_score,
            "React content should score higher for React dev ({}) than Security eng ({})",
            react_score,
            security_score
        );
    }
}
