//! Simulation Personas — 9 developer archetypes
//!
//! Canonical order: [rust, python, typescript, devops, mobile, bootstrap, power, switcher, niche]

use std::collections::{HashMap, HashSet};

use super::super::ace_context::ACEContext;
use super::super::ScoringContext;

pub(super) fn emb() -> Vec<f32> {
    vec![0.5_f32; 384]
}

pub(super) fn make_interests(topics: &[(&str, f32)]) -> Vec<crate::context_engine::Interest> {
    let e = emb();
    topics
        .iter()
        .enumerate()
        .map(|(i, (t, w))| crate::context_engine::Interest {
            id: Some((i + 1) as i64),
            topic: t.to_string(),
            weight: *w,
            embedding: Some(e.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        })
        .collect()
}

/// Calibrated variant: uses per-persona domain embeddings instead of uniform vectors.
/// Each interest gets an embedding aligned with the persona's domain block, enabling
/// cosine-similarity-based interest scoring to discriminate in-domain vs off-domain.
#[cfg(feature = "calibrated-sim")]
pub(super) fn make_interests_calibrated(
    topics: &[(&str, f32)],
    persona_idx: usize,
) -> Vec<crate::context_engine::Interest> {
    let e = super::domain_embeddings::interest_embedding(persona_idx);
    topics
        .iter()
        .enumerate()
        .map(|(i, (t, w))| crate::context_engine::Interest {
            id: Some((i + 1) as i64),
            topic: t.to_string(),
            weight: *w,
            embedding: Some(e.clone()),
            source: crate::context_engine::InterestSource::Explicit,
        })
        .collect()
}

pub(super) fn make_domain(
    primary: &[&str],
    adjacent: &[&str],
    deps: &[&str],
    interest_topics: &[&str],
) -> crate::domain_profile::DomainProfile {
    let ps: HashSet<String> = primary.iter().map(|s| s.to_string()).collect();
    let adj: HashSet<String> = adjacent.iter().map(|s| s.to_string()).collect();
    let mut all = ps.clone();
    all.extend(adj.clone());
    crate::domain_profile::DomainProfile {
        primary_stack: ps,
        adjacent_tech: adj,
        all_tech: all,
        dependency_names: deps.iter().map(|s| s.to_string()).collect(),
        interest_topics: interest_topics.iter().map(|s| s.to_string()).collect(),
        domain_concerns: HashSet::new(),
    }
}

// ============================================================================
// Persona 0: Rust Systems Developer
// ============================================================================

pub(super) fn rust_systems_dev() -> ScoringContext {
    #[cfg(feature = "calibrated-sim")]
    let interests = make_interests_calibrated(
        &[("Rust", 1.0), ("systems programming", 1.0), ("Tauri", 0.9), ("SQLite", 0.8), ("WebAssembly", 0.7)],
        0,
    );
    #[cfg(not(feature = "calibrated-sim"))]
    let interests = make_interests(&[
        ("Rust", 1.0),
        ("systems programming", 1.0),
        ("Tauri", 0.9),
        ("SQLite", 0.8),
        ("WebAssembly", 0.7),
    ]);
    let mut ace = ACEContext::default();
    for t in ["rust", "tauri", "sqlite"] {
        ace.active_topics.push(t.to_string());
        ace.detected_tech.push(t.to_string());
    }
    let domain = make_domain(
        &["rust", "tauri", "sqlite"],
        &["tokio", "serde", "wasm", "typescript"],
        &["tokio", "serde", "sqlx", "tauri"],
        &["rust", "systems programming", "tauri", "sqlite"],
    );
    let stack = crate::stacks::compose_profiles(&["rust_systems".to_string()]);
    ScoringContext::builder()
        .interest_count(5)
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(vec![
            "rust".to_string(),
            "tauri".to_string(),
            "sqlite".to_string(),
        ])
        .composed_stack(stack)
        .feedback_interaction_count(50)
        .build()
}

// ============================================================================
// Persona 1: Python ML Engineer
// ============================================================================

pub(super) fn python_ml_engineer() -> ScoringContext {
    #[cfg(feature = "calibrated-sim")]
    let interests = make_interests_calibrated(
        &[("Machine Learning", 1.0), ("Python", 1.0), ("LLM", 0.9), ("PyTorch", 0.9), ("data science", 0.7)],
        1,
    );
    #[cfg(not(feature = "calibrated-sim"))]
    let interests = make_interests(&[
        ("Machine Learning", 1.0),
        ("Python", 1.0),
        ("LLM", 0.9),
        ("PyTorch", 0.9),
        ("data science", 0.7),
    ]);
    let mut ace = ACEContext::default();
    for t in ["python", "pytorch", "machine learning"] {
        ace.active_topics.push(t.to_string());
    }
    for t in ["python", "pytorch"] {
        ace.detected_tech.push(t.to_string());
    }
    let domain = make_domain(
        &["python", "pytorch", "tensorflow"],
        &["numpy", "pandas", "scikit-learn", "huggingface"],
        &["torch", "transformers", "numpy", "pandas"],
        &["machine learning", "python", "llm", "pytorch"],
    );
    let stack = crate::stacks::compose_profiles(&["python_ml".to_string()]);
    ScoringContext::builder()
        .interest_count(5)
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(vec![
            "python".to_string(),
            "pytorch".to_string(),
            "tensorflow".to_string(),
        ])
        .composed_stack(stack)
        .feedback_interaction_count(40)
        .build()
}

// ============================================================================
// Persona 2: Fullstack TypeScript Developer
// ============================================================================

pub(super) fn fullstack_typescript() -> ScoringContext {
    #[cfg(feature = "calibrated-sim")]
    let interests = make_interests_calibrated(
        &[("TypeScript", 1.0), ("React", 1.0), ("Node.js", 0.9), ("Next.js", 0.8), ("GraphQL", 0.7)],
        2,
    );
    #[cfg(not(feature = "calibrated-sim"))]
    let interests = make_interests(&[
        ("TypeScript", 1.0),
        ("React", 1.0),
        ("Node.js", 0.9),
        ("Next.js", 0.8),
        ("GraphQL", 0.7),
    ]);
    let mut ace = ACEContext::default();
    for t in ["typescript", "react", "nextjs"] {
        ace.active_topics.push(t.to_string());
    }
    for t in ["typescript", "react", "nodejs"] {
        ace.detected_tech.push(t.to_string());
    }
    let domain = make_domain(
        &["typescript", "react", "nodejs"],
        &["nextjs", "graphql", "prisma", "tailwind"],
        &["react", "typescript", "next", "prisma"],
        &["typescript", "react", "nodejs", "nextjs"],
    );
    let stack = crate::stacks::compose_profiles(&["fullstack_ts".to_string()]);
    ScoringContext::builder()
        .interest_count(5)
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(vec![
            "typescript".to_string(),
            "react".to_string(),
            "nodejs".to_string(),
        ])
        .composed_stack(stack)
        .feedback_interaction_count(35)
        .build()
}

// ============================================================================
// Persona 3: DevOps/SRE
// ============================================================================

pub(super) fn devops_sre() -> ScoringContext {
    // Phase 2A: Replace broad "DevOps" with specific multi-word terms that bypass
    // BROAD_INTEREST_TERMS specificity penalty (calibration.rs:88). Multi-word terms
    // get 1.0x specificity instead of 0.40x.
    #[cfg(feature = "calibrated-sim")]
    let interests = make_interests_calibrated(
        &[
            ("Kubernetes", 1.0),
            ("kubernetes operator", 0.9),
            ("Docker", 0.9),
            ("Terraform", 0.8),
            ("observability stack", 0.8),
            ("eBPF tracing", 0.7),
            ("Prometheus metrics", 0.7),
            ("SRE", 0.8),
        ],
        3,
    );
    #[cfg(not(feature = "calibrated-sim"))]
    let interests = make_interests(&[
        ("Kubernetes", 1.0),
        ("kubernetes operator", 0.9),
        ("Docker", 0.9),
        ("Terraform", 0.8),
        ("observability stack", 0.8),
        ("eBPF tracing", 0.7),
        ("Prometheus metrics", 0.7),
        ("SRE", 0.8),
    ]);
    let mut ace = ACEContext::default();
    for t in ["kubernetes", "docker", "terraform", "prometheus", "ci/cd", "observability"] {
        ace.active_topics.push(t.to_string());
    }
    for t in ["kubernetes", "docker", "terraform", "prometheus", "ci/cd", "observability"] {
        ace.detected_tech.push(t.to_string());
    }
    let domain = make_domain(
        &["kubernetes", "docker", "terraform"],
        &["helm", "prometheus", "grafana", "ansible", "ebpf"],
        &["kubernetes", "terraform", "helm", "prometheus", "grafana"],
        &[
            "kubernetes",
            "kubernetes operator",
            "docker",
            "terraform",
            "observability stack",
            "ebpf tracing",
            "prometheus metrics",
            "sre",
        ],
    );
    let stack = crate::stacks::compose_profiles(&["devops_sre".to_string()]);
    ScoringContext::builder()
        .interest_count(8)
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(vec![
            "kubernetes".to_string(),
            "docker".to_string(),
            "terraform".to_string(),
            "sre".to_string(),
        ])
        .composed_stack(stack)
        .feedback_interaction_count(30)
        .build()
}

// ============================================================================
// Persona 4: Mobile Developer
// ============================================================================

pub(super) fn mobile_dev() -> ScoringContext {
    #[cfg(feature = "calibrated-sim")]
    let interests = make_interests_calibrated(
        &[("React Native", 1.0), ("mobile development", 1.0), ("Expo", 0.9), ("iOS", 0.7), ("Android", 0.7)],
        4,
    );
    #[cfg(not(feature = "calibrated-sim"))]
    let interests = make_interests(&[
        ("React Native", 1.0),
        ("mobile development", 1.0),
        ("Expo", 0.9),
        ("iOS", 0.7),
        ("Android", 0.7),
    ]);
    let mut ace = ACEContext::default();
    for t in ["react native", "expo", "mobile"] {
        ace.active_topics.push(t.to_string());
    }
    for t in ["react native", "expo", "typescript"] {
        ace.detected_tech.push(t.to_string());
    }
    let domain = make_domain(
        &["react native", "expo", "typescript"],
        &["ios", "android", "fastlane", "metro"],
        &["react-native", "expo", "typescript"],
        &["react native", "mobile development", "expo"],
    );
    let stack = crate::stacks::compose_profiles(&["mobile_rn".to_string()]);
    ScoringContext::builder()
        .interest_count(5)
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(vec![
            "react native".to_string(),
            "expo".to_string(),
            "typescript".to_string(),
        ])
        .composed_stack(stack)
        .feedback_interaction_count(25)
        .build()
}

// ============================================================================
// Persona 5: Bootstrap / First-Run User
// ============================================================================

pub(super) fn bootstrap_user() -> ScoringContext {
    #[cfg(feature = "calibrated-sim")]
    let interests = make_interests_calibrated(&[("TypeScript", 1.0)], 5);
    #[cfg(not(feature = "calibrated-sim"))]
    let interests = make_interests(&[("TypeScript", 1.0)]);
    let mut ace = ACEContext::default();
    ace.active_topics.push("typescript".to_string());
    ScoringContext::builder()
        .interest_count(1)
        .interests(interests)
        .ace_ctx(ace)
        .feedback_interaction_count(0)
        .build()
}

// ============================================================================
// Persona 6: Power User
// ============================================================================

pub(super) fn power_user() -> ScoringContext {
    #[cfg(feature = "calibrated-sim")]
    let interests = make_interests_calibrated(
        &[
            ("Rust", 0.9), ("Python", 0.8), ("TypeScript", 0.8), ("distributed systems", 0.9),
            ("AI", 0.7), ("WebAssembly", 0.7), ("databases", 0.8),
        ],
        6,
    );
    #[cfg(not(feature = "calibrated-sim"))]
    let interests = make_interests(&[
        ("Rust", 0.9),
        ("Python", 0.8),
        ("TypeScript", 0.8),
        ("distributed systems", 0.9),
        ("AI", 0.7),
        ("WebAssembly", 0.7),
        ("databases", 0.8),
    ]);
    let mut ace = ACEContext::default();
    for t in ["rust", "python", "typescript", "distributed systems"] {
        ace.active_topics.push(t.to_string());
    }
    for t in ["rust", "python", "typescript"] {
        ace.detected_tech.push(t.to_string());
    }
    let domain = make_domain(
        &["rust", "python", "typescript"],
        &["wasm", "llm", "databases", "distributed systems"],
        &["tokio", "torch", "react", "postgres"],
        &["rust", "python", "typescript", "distributed systems", "ai"],
    );
    let mut boosts: HashMap<String, f64> = HashMap::new();
    boosts.insert("performance".to_string(), 0.3);
    boosts.insert("architecture".to_string(), 0.3);
    ScoringContext::builder()
        .interest_count(7)
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .feedback_boosts(boosts)
        .feedback_interaction_count(200)
        .build()
}

// ============================================================================
// Persona 7: Context Switcher
// ============================================================================

pub(super) fn context_switcher() -> ScoringContext {
    #[cfg(feature = "calibrated-sim")]
    let interests = make_interests_calibrated(
        &[("Rust", 0.9), ("Go", 0.9), ("backend", 0.8), ("microservices", 0.7)],
        7,
    );
    #[cfg(not(feature = "calibrated-sim"))]
    let interests = make_interests(&[
        ("Rust", 0.9),
        ("Go", 0.9),
        ("backend", 0.8),
        ("microservices", 0.7),
    ]);
    let mut ace = ACEContext::default();
    for t in ["rust", "go", "backend"] {
        ace.active_topics.push(t.to_string());
    }
    for t in ["rust", "go", "grpc"] {
        ace.detected_tech.push(t.to_string());
    }
    let domain = make_domain(
        &["rust", "go"],
        &["grpc", "kafka", "redis", "postgres"],
        &["tokio", "axum", "gin", "grpc"],
        &["rust", "go", "backend", "microservices"],
    );
    ScoringContext::builder()
        .interest_count(4)
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(vec!["rust".to_string(), "go".to_string()])
        .feedback_interaction_count(60)
        .build()
}

// ============================================================================
// Persona 8: Niche Specialist (Haskell/FP)
// ============================================================================

pub(super) fn niche_specialist() -> ScoringContext {
    #[cfg(feature = "calibrated-sim")]
    let interests = make_interests_calibrated(
        &[("Haskell", 1.0), ("functional programming", 1.0), ("type theory", 0.9), ("category theory", 0.7), ("Nix", 0.7), ("monad", 0.6), ("type system", 0.6)],
        8,
    );
    #[cfg(not(feature = "calibrated-sim"))]
    let interests = make_interests(&[
        ("Haskell", 1.0),
        ("functional programming", 1.0),
        ("type theory", 0.9),
        ("category theory", 0.7),
        ("Nix", 0.7),
        ("monad", 0.6),
        ("type system", 0.6),
    ]);
    let mut ace = ACEContext::default();
    for t in ["haskell", "functional programming", "nix", "type theory"] {
        ace.active_topics.push(t.to_string());
    }
    for t in ["haskell", "nix", "ghc", "cabal"] {
        ace.detected_tech.push(t.to_string());
    }
    let domain = make_domain(
        &["haskell", "nix"],
        &["purescript", "ocaml", "elm", "agda"],
        &["ghc", "cabal", "nix"],
        &["haskell", "functional programming", "type theory", "nix"],
    );
    let stack = crate::stacks::compose_profiles(&["haskell".to_string()]);
    ScoringContext::builder()
        .interest_count(7)
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(vec!["haskell".to_string(), "nix".to_string()])
        .composed_stack(stack)
        .feedback_interaction_count(30)
        .build()
}

// ============================================================================
// All Personas
// ============================================================================

pub(super) fn all_personas() -> Vec<ScoringContext> {
    vec![
        rust_systems_dev(),
        python_ml_engineer(),
        fullstack_typescript(),
        devops_sre(),
        mobile_dev(),
        bootstrap_user(),
        power_user(),
        context_switcher(),
        niche_specialist(),
    ]
}
