// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Extended persona enrichment data — personas 5–8.
//!
//! Split from `persona_data.rs` for file-size hygiene.

use std::collections::{HashMap, HashSet};

use super::*;

// ============================================================================
// Persona 5: Bootstrap / First-Run User
// ============================================================================

pub(super) fn bootstrap_enrichment() -> PersonaEnrichment {
    // MINIMAL enrichment — tests bootstrap path fidelity
    PersonaEnrichment {
        topic_confidence: confidence_map(&[("typescript", 0.60)]),
        topic_affinities: HashMap::new(),
        anti_topics: vec![],
        anti_topic_confidence: HashMap::new(),
        topic_embeddings: topic_embedding_map(&["typescript"]),
        source_quality: HashMap::new(),
        work_topics: vec![],
        calibration_deltas: HashMap::new(),
        taste_embedding: None,
        topic_half_lives: HashMap::new(),
        exclusions: vec![],
        open_windows: vec![],
        skill_gaps: vec![],
        dependency_info: vec![],
        dependency_names: HashSet::new(),
    }
}

// ============================================================================
// Persona 6: Power User
// ============================================================================

pub(super) fn power_user_enrichment() -> PersonaEnrichment {
    PersonaEnrichment {
        topic_confidence: confidence_map(&[
            ("rust", 0.90),
            ("python", 0.85),
            ("typescript", 0.80),
            ("distributed systems", 0.85),
            ("ai", 0.75),
            ("wasm", 0.70),
            ("databases", 0.80),
        ]),
        topic_affinities: affinity_map(&[
            ("rust", 0.6, 0.85),
            ("python", 0.5, 0.8),
            ("distributed systems", 0.7, 0.85),
            ("performance", 0.4, 0.7),
            ("architecture", 0.5, 0.75),
        ]),
        anti_topics: string_vec(&["crypto", "nft", "web3"]),
        anti_topic_confidence: confidence_map(&[("crypto", 0.7), ("nft", 0.8), ("web3", 0.6)]),
        topic_embeddings: topic_embedding_map(&[
            "rust",
            "python",
            "typescript",
            "distributed systems",
            "ai",
            "wasm",
            "databases",
        ]),
        source_quality: source_quality_map(&[
            ("hackernews", 0.85),
            ("reddit", 0.7),
            ("github_trending", 0.9),
            ("rss", 0.8),
        ]),
        work_topics: string_vec(&["rust", "python", "distributed systems"]),
        calibration_deltas: confidence_map(&[
            ("rust", 0.03),
            ("python", -0.02),
            ("typescript", 0.01),
        ]),
        taste_embedding: Some(taste_from_topics(&[
            "rust",
            "python",
            "distributed systems",
        ])),
        topic_half_lives: half_life_map(&[
            ("rust", 168.0),
            ("python", 168.0),
            ("typescript", 168.0),
            ("distributed systems", 336.0),
            ("ai", 72.0),
        ]),
        exclusions: string_vec(&["crypto", "nft", "blockchain"]),
        open_windows: vec![WindowSpec {
            window_type: "adoption",
            title: "Evaluate distributed tracing solution",
            urgency: 0.5,
            relevance: 0.8,
            dependency: Some("opentelemetry"),
        }],
        skill_gaps: vec![
            ("opentelemetry", "Observability integration gap"),
            ("wasm-bindgen", "WASM deployment gap"),
        ],
        dependency_info: vec![
            DepInfoSpec {
                package_name: "tokio",
                version: Some("1.35"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["tokio", "async runtime"],
                ecosystem: "rust",
            },
            DepInfoSpec {
                package_name: "torch",
                version: Some("2.1"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["torch", "pytorch"],
                ecosystem: "python",
            },
            DepInfoSpec {
                package_name: "react",
                version: Some("18.2"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["react"],
                ecosystem: "javascript",
            },
        ],
        dependency_names: string_set(&["tokio", "serde", "torch", "react", "postgres", "redis"]),
    }
}

// ============================================================================
// Persona 7: Context Switcher
// ============================================================================

pub(super) fn context_switcher_enrichment() -> PersonaEnrichment {
    PersonaEnrichment {
        topic_confidence: confidence_map(&[
            ("rust", 0.85),
            ("go", 0.85),
            ("backend", 0.80),
            ("microservices", 0.75),
            ("grpc", 0.70),
        ]),
        topic_affinities: affinity_map(&[
            ("rust", 0.5, 0.8),
            ("go", 0.5, 0.8),
            ("microservices", 0.4, 0.7),
            ("grpc", 0.3, 0.6),
        ]),
        anti_topics: string_vec(&["frontend", "css"]),
        anti_topic_confidence: confidence_map(&[("frontend", 0.5), ("css", 0.5)]),
        topic_embeddings: topic_embedding_map(&["rust", "go", "backend", "microservices", "grpc"]),
        source_quality: source_quality_map(&[
            ("hackernews", 0.8),
            ("reddit", 0.6),
            ("github_trending", 0.85),
            ("rss", 0.7),
        ]),
        work_topics: string_vec(&["rust", "go"]),
        calibration_deltas: confidence_map(&[("rust", 0.02), ("go", -0.01)]),
        taste_embedding: Some(taste_from_topics(&["rust", "go", "microservices"])),
        // Different half-lives: Go ecosystem moves faster than Rust
        topic_half_lives: half_life_map(&[
            ("rust", 336.0),
            ("go", 168.0),
            ("backend", 336.0),
            ("microservices", 168.0),
        ]),
        exclusions: string_vec(&["crypto"]),
        open_windows: vec![],
        skill_gaps: vec![("tonic", "gRPC Rust integration gap")],
        dependency_info: vec![
            DepInfoSpec {
                package_name: "tokio",
                version: Some("1.35"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["tokio", "async runtime"],
                ecosystem: "rust",
            },
            DepInfoSpec {
                package_name: "gin",
                version: Some("1.9"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["gin", "go web"],
                ecosystem: "go",
            },
        ],
        dependency_names: string_set(&["tokio", "axum", "gin", "grpc", "kafka"]),
    }
}

// ============================================================================
// Persona 8: Niche Specialist (Haskell/FP)
// ============================================================================

pub(super) fn niche_specialist_enrichment() -> PersonaEnrichment {
    PersonaEnrichment {
        topic_confidence: confidence_map(&[
            ("haskell", 0.95),
            ("functional programming", 0.95),
            ("type theory", 0.90),
            ("category theory", 0.80),
            ("nix", 0.85),
        ]),
        topic_affinities: affinity_map(&[
            ("haskell", 0.9, 0.95),
            ("functional programming", 0.8, 0.9),
            ("type theory", 0.7, 0.85),
            ("nix", 0.5, 0.7),
        ]),
        anti_topics: string_vec(&["javascript", "python", "java"]),
        anti_topic_confidence: confidence_map(&[
            ("javascript", 0.7),
            ("python", 0.6),
            ("java", 0.7),
        ]),
        topic_embeddings: topic_embedding_map(&[
            "haskell",
            "functional programming",
            "type theory",
            "nix",
        ]),
        source_quality: source_quality_map(&[
            ("hackernews", 0.7),
            ("reddit", 0.6),
            ("github_trending", 0.7),
            ("rss", 0.9),
        ]),
        work_topics: string_vec(&["haskell", "nix"]),
        calibration_deltas: confidence_map(&[("haskell", 0.06), ("functional programming", 0.04)]),
        taste_embedding: Some(taste_from_topics(&[
            "haskell",
            "functional programming",
            "type theory",
        ])),
        topic_half_lives: half_life_map(&[
            ("haskell", 672.0),
            ("functional programming", 672.0),
            ("type theory", 672.0),
            ("nix", 336.0),
        ]),
        exclusions: string_vec(&["javascript", "web development", "career"]),
        open_windows: vec![],
        skill_gaps: vec![
            ("purescript", "PureScript migration path"),
            ("idris", "Dependent types gap"),
        ],
        dependency_info: vec![
            DepInfoSpec {
                package_name: "ghc",
                version: Some("9.6"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["ghc", "haskell compiler"],
                ecosystem: "haskell",
            },
            DepInfoSpec {
                package_name: "cabal",
                version: Some("3.10"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["cabal", "haskell build"],
                ecosystem: "haskell",
            },
        ],
        dependency_names: string_set(&["ghc", "cabal", "nix", "stack"]),
    }
}
