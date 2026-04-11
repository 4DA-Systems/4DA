//! Per-persona enrichment data for full-fidelity simulation.
//!
//! Provides realistic values for the 11 ScoringContext fields and 4 ACEContext
//! fields that base personas leave at defaults. Each persona gets a
//! `PersonaEnrichment` struct with domain-appropriate values.

use std::collections::{HashMap, HashSet};

use super::domain_embeddings::topic_embedding;

// ============================================================================
// Types
// ============================================================================

/// Simplified decision window spec for enrichment data.
pub(super) struct WindowSpec {
    pub window_type: &'static str,
    pub title: &'static str,
    pub urgency: f32,
    pub relevance: f32,
    pub dependency: Option<&'static str>,
}

/// Simplified dependency info spec for enrichment data.
pub(super) struct DepInfoSpec {
    pub package_name: &'static str,
    pub version: Option<&'static str>,
    pub is_dev: bool,
    pub is_direct: bool,
    pub search_terms: Vec<&'static str>,
}

/// Full enrichment data for a single persona.
/// Covers all fields that base persona builders leave at defaults.
pub(super) struct PersonaEnrichment {
    // ACEContext enrichment (4 missing fields)
    pub topic_confidence: HashMap<String, f32>,
    pub topic_affinities: HashMap<String, (f32, f32)>,
    pub anti_topics: Vec<String>,
    pub anti_topic_confidence: HashMap<String, f32>,

    // ScoringContext enrichment (11 missing fields)
    pub topic_embeddings: HashMap<String, Vec<f32>>,
    pub source_quality: HashMap<String, f32>,
    pub work_topics: Vec<String>,
    pub calibration_deltas: HashMap<String, f32>,
    pub taste_embedding: Option<Vec<f32>>,
    pub topic_half_lives: HashMap<String, f32>,
    pub exclusions: Vec<String>,
    pub open_windows: Vec<WindowSpec>,
    pub skill_gaps: Vec<(&'static str, &'static str)>,
    pub dependency_info: Vec<DepInfoSpec>,
    pub dependency_names: HashSet<String>,
}

// ============================================================================
// Helpers
// ============================================================================

fn confidence_map(pairs: &[(&str, f32)]) -> HashMap<String, f32> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn affinity_map(pairs: &[(&str, f32, f32)]) -> HashMap<String, (f32, f32)> {
    pairs
        .iter()
        .map(|(k, a, c)| (k.to_string(), (*a, *c)))
        .collect()
}

fn string_vec(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn string_set(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn half_life_map(pairs: &[(&str, f32)]) -> HashMap<String, f32> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn source_quality_map(pairs: &[(&str, f32)]) -> HashMap<String, f32> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn topic_embedding_map(topics: &[&str]) -> HashMap<String, Vec<f32>> {
    topics
        .iter()
        .map(|t| (t.to_string(), topic_embedding(t)))
        .collect()
}

fn taste_from_topics(topics: &[&str]) -> Vec<f32> {
    if topics.is_empty() {
        return vec![0.0; 384];
    }
    let embeddings: Vec<Vec<f32>> = topics.iter().map(|t| topic_embedding(t)).collect();
    let mut avg = vec![0.0_f32; 384];
    for emb in &embeddings {
        for (i, v) in emb.iter().enumerate() {
            avg[i] += v;
        }
    }
    let n = embeddings.len() as f32;
    let norm: f32 = avg.iter().map(|v| (v / n).powi(2)).sum::<f32>().sqrt();
    if norm > 0.0 {
        avg.iter().map(|v| v / n / norm).collect()
    } else {
        avg.iter().map(|v| v / n).collect()
    }
}

// ============================================================================
// Persona 0: Rust Systems Developer
// ============================================================================

fn rust_enrichment() -> PersonaEnrichment {
    PersonaEnrichment {
        topic_confidence: confidence_map(&[
            ("rust", 0.95),
            ("tauri", 0.90),
            ("sqlite", 0.85),
            ("wasm", 0.75),
            ("systems programming", 0.90),
        ]),
        topic_affinities: affinity_map(&[
            ("rust", 0.7, 0.9),
            ("systems programming", 0.5, 0.8),
            ("memory safety", 0.4, 0.7),
            ("performance", 0.3, 0.6),
        ]),
        anti_topics: string_vec(&["python", "java"]),
        anti_topic_confidence: confidence_map(&[("python", 0.6), ("java", 0.5)]),
        topic_embeddings: topic_embedding_map(&[
            "rust",
            "tauri",
            "sqlite",
            "wasm",
            "systems programming",
        ]),
        source_quality: source_quality_map(&[
            ("hackernews", 0.8),
            ("reddit", 0.6),
            ("github_trending", 0.9),
            ("rss", 0.7),
        ]),
        work_topics: string_vec(&["rust", "tauri"]),
        calibration_deltas: confidence_map(&[("rust", 0.05), ("tauri", 0.03)]),
        taste_embedding: Some(taste_from_topics(&["rust", "systems programming", "tauri"])),
        topic_half_lives: half_life_map(&[
            ("rust", 168.0),
            ("tauri", 72.0),
            ("sqlite", 336.0),
            ("wasm", 168.0),
        ]),
        exclusions: string_vec(&["crypto", "nft", "blockchain"]),
        open_windows: vec![],
        skill_gaps: vec![("wasm-bindgen", "WebAssembly integration gap")],
        dependency_info: vec![
            DepInfoSpec {
                package_name: "tokio",
                version: Some("1.35"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["tokio", "async runtime"],
            },
            DepInfoSpec {
                package_name: "serde",
                version: Some("1.0"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["serde", "serialization"],
            },
            DepInfoSpec {
                package_name: "sqlx",
                version: Some("0.7"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["sqlx", "sql"],
            },
        ],
        dependency_names: string_set(&["tokio", "serde", "sqlx", "tauri", "anyhow", "thiserror"]),
    }
}

// ============================================================================
// Persona 1: Python ML Engineer
// ============================================================================

fn python_enrichment() -> PersonaEnrichment {
    PersonaEnrichment {
        topic_confidence: confidence_map(&[
            ("python", 0.95),
            ("pytorch", 0.90),
            ("machine learning", 0.92),
            ("llm", 0.85),
            ("data science", 0.75),
        ]),
        topic_affinities: affinity_map(&[
            ("machine learning", 0.8, 0.9),
            ("python", 0.6, 0.85),
            ("transformers", 0.5, 0.7),
            ("gpu", 0.3, 0.6),
        ]),
        anti_topics: string_vec(&["rust", "go"]),
        anti_topic_confidence: confidence_map(&[("rust", 0.5), ("go", 0.4)]),
        topic_embeddings: topic_embedding_map(&[
            "python",
            "pytorch",
            "machine learning",
            "llm",
            "data science",
        ]),
        source_quality: source_quality_map(&[
            ("hackernews", 0.7),
            ("reddit", 0.7),
            ("github_trending", 0.8),
            ("rss", 0.8),
        ]),
        work_topics: string_vec(&["pytorch", "transformers"]),
        calibration_deltas: confidence_map(&[("python", 0.04), ("pytorch", 0.06)]),
        taste_embedding: Some(taste_from_topics(&[
            "python",
            "machine learning",
            "pytorch",
        ])),
        topic_half_lives: half_life_map(&[
            ("python", 336.0),
            ("pytorch", 168.0),
            ("machine learning", 336.0),
            ("llm", 72.0),
        ]),
        exclusions: string_vec(&["crypto", "web3"]),
        open_windows: vec![],
        skill_gaps: vec![("triton", "GPU kernel optimization gap")],
        dependency_info: vec![
            DepInfoSpec {
                package_name: "torch",
                version: Some("2.1"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["torch", "pytorch"],
            },
            DepInfoSpec {
                package_name: "transformers",
                version: Some("4.36"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["transformers", "huggingface"],
            },
            DepInfoSpec {
                package_name: "numpy",
                version: Some("1.26"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["numpy"],
            },
        ],
        dependency_names: string_set(&["torch", "transformers", "numpy", "pandas", "scikit-learn"]),
    }
}

// ============================================================================
// Persona 2: Fullstack TypeScript Developer
// ============================================================================

fn fullstack_ts_enrichment() -> PersonaEnrichment {
    PersonaEnrichment {
        topic_confidence: confidence_map(&[
            ("typescript", 0.90),
            ("react", 0.90),
            ("nextjs", 0.85),
            ("nodejs", 0.80),
            ("graphql", 0.70),
        ]),
        topic_affinities: affinity_map(&[
            ("react", 0.7, 0.85),
            ("nextjs", 0.5, 0.7),
            ("typescript", 0.6, 0.8),
            ("tailwind", 0.3, 0.5),
        ]),
        anti_topics: string_vec(&["java", "c++"]),
        anti_topic_confidence: confidence_map(&[("java", 0.5), ("c++", 0.4)]),
        topic_embeddings: topic_embedding_map(&[
            "typescript",
            "react",
            "nextjs",
            "nodejs",
            "graphql",
        ]),
        source_quality: source_quality_map(&[
            ("hackernews", 0.7),
            ("reddit", 0.7),
            ("github_trending", 0.8),
            ("rss", 0.8),
        ]),
        work_topics: string_vec(&["react", "nextjs"]),
        calibration_deltas: confidence_map(&[("typescript", 0.03), ("react", 0.04)]),
        taste_embedding: Some(taste_from_topics(&["typescript", "react", "nextjs"])),
        topic_half_lives: half_life_map(&[
            ("typescript", 336.0),
            ("react", 168.0),
            ("nextjs", 72.0),
            ("nodejs", 336.0),
        ]),
        exclusions: string_vec(&["crypto"]),
        open_windows: vec![],
        skill_gaps: vec![("prisma", "ORM integration depth")],
        dependency_info: vec![
            DepInfoSpec {
                package_name: "react",
                version: Some("18.2"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["react"],
            },
            DepInfoSpec {
                package_name: "next",
                version: Some("14.0"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["next", "nextjs"],
            },
            DepInfoSpec {
                package_name: "prisma",
                version: Some("5.7"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["prisma", "orm"],
            },
        ],
        dependency_names: string_set(&["react", "next", "prisma", "typescript", "tailwindcss"]),
    }
}

// ============================================================================
// Persona 3: DevOps/SRE
// ============================================================================

fn devops_enrichment() -> PersonaEnrichment {
    PersonaEnrichment {
        topic_confidence: confidence_map(&[
            ("kubernetes", 0.95),
            ("docker", 0.85),
            ("terraform", 0.85),
            ("prometheus", 0.80),
            ("ci/cd", 0.80),
            ("observability", 0.75),
        ]),
        topic_affinities: affinity_map(&[
            ("kubernetes", 0.8, 0.9),
            ("terraform", 0.6, 0.8),
            ("observability", 0.5, 0.7),
            ("ebpf", 0.4, 0.6),
        ]),
        anti_topics: string_vec(&["frontend", "react"]),
        anti_topic_confidence: confidence_map(&[("frontend", 0.6), ("react", 0.5)]),
        topic_embeddings: topic_embedding_map(&[
            "kubernetes",
            "docker",
            "terraform",
            "prometheus",
            "observability",
        ]),
        source_quality: source_quality_map(&[
            ("hackernews", 0.7),
            ("reddit", 0.6),
            ("github_trending", 0.8),
            ("rss", 0.8),
        ]),
        work_topics: string_vec(&["kubernetes", "terraform"]),
        calibration_deltas: confidence_map(&[("kubernetes", 0.05), ("terraform", 0.03)]),
        taste_embedding: Some(taste_from_topics(&[
            "kubernetes",
            "docker",
            "terraform",
            "observability",
        ])),
        topic_half_lives: half_life_map(&[
            ("kubernetes", 168.0),
            ("docker", 336.0),
            ("terraform", 168.0),
            ("prometheus", 336.0),
        ]),
        exclusions: string_vec(&["crypto", "nft"]),
        open_windows: vec![],
        skill_gaps: vec![("cilium", "eBPF networking gap")],
        dependency_info: vec![
            DepInfoSpec {
                package_name: "helm",
                version: Some("3.13"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["helm", "kubernetes helm"],
            },
            DepInfoSpec {
                package_name: "prometheus",
                version: Some("2.48"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["prometheus", "monitoring"],
            },
        ],
        dependency_names: string_set(&["helm", "prometheus", "grafana", "terraform", "kubernetes"]),
    }
}

// ============================================================================
// Persona 4: Mobile Developer
// ============================================================================

fn mobile_enrichment() -> PersonaEnrichment {
    PersonaEnrichment {
        topic_confidence: confidence_map(&[
            ("react native", 0.90),
            ("expo", 0.85),
            ("mobile", 0.85),
            ("ios", 0.70),
            ("android", 0.70),
        ]),
        topic_affinities: affinity_map(&[
            ("mobile", 0.7, 0.85),
            ("expo", 0.5, 0.7),
            ("react native", 0.6, 0.8),
        ]),
        anti_topics: string_vec(&["backend", "devops"]),
        anti_topic_confidence: confidence_map(&[("backend", 0.4), ("devops", 0.4)]),
        topic_embeddings: topic_embedding_map(&[
            "react native",
            "expo",
            "mobile",
            "ios",
            "android",
        ]),
        source_quality: source_quality_map(&[
            ("hackernews", 0.6),
            ("reddit", 0.7),
            ("github_trending", 0.8),
            ("rss", 0.7),
        ]),
        work_topics: string_vec(&["react native", "expo"]),
        calibration_deltas: confidence_map(&[("react native", 0.04), ("expo", 0.03)]),
        taste_embedding: Some(taste_from_topics(&["react native", "mobile", "expo"])),
        topic_half_lives: half_life_map(&[
            ("react native", 168.0),
            ("expo", 72.0),
            ("ios", 336.0),
            ("android", 336.0),
        ]),
        exclusions: string_vec(&["crypto"]),
        open_windows: vec![],
        skill_gaps: vec![("reanimated", "Animation performance gap")],
        dependency_info: vec![
            DepInfoSpec {
                package_name: "react-native",
                version: Some("0.73"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["react-native", "react native"],
            },
            DepInfoSpec {
                package_name: "expo",
                version: Some("50.0"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["expo"],
            },
        ],
        dependency_names: string_set(&["react-native", "expo", "typescript", "metro"]),
    }
}

// ============================================================================
// Persona 5: Bootstrap / First-Run User
// ============================================================================

fn bootstrap_enrichment() -> PersonaEnrichment {
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

fn power_user_enrichment() -> PersonaEnrichment {
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
            },
            DepInfoSpec {
                package_name: "torch",
                version: Some("2.1"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["torch", "pytorch"],
            },
            DepInfoSpec {
                package_name: "react",
                version: Some("18.2"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["react"],
            },
        ],
        dependency_names: string_set(&["tokio", "serde", "torch", "react", "postgres", "redis"]),
    }
}

// ============================================================================
// Persona 7: Context Switcher
// ============================================================================

fn context_switcher_enrichment() -> PersonaEnrichment {
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
            },
            DepInfoSpec {
                package_name: "gin",
                version: Some("1.9"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["gin", "go web"],
            },
        ],
        dependency_names: string_set(&["tokio", "axum", "gin", "grpc", "kafka"]),
    }
}

// ============================================================================
// Persona 8: Niche Specialist (Haskell/FP)
// ============================================================================

fn niche_specialist_enrichment() -> PersonaEnrichment {
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
            },
            DepInfoSpec {
                package_name: "cabal",
                version: Some("3.10"),
                is_dev: false,
                is_direct: true,
                search_terms: vec!["cabal", "haskell build"],
            },
        ],
        dependency_names: string_set(&["ghc", "cabal", "nix", "stack"]),
    }
}

// ============================================================================
// Collector
// ============================================================================

/// Returns enrichment data for all 9 personas in canonical order.
pub(super) fn all_enrichments() -> Vec<PersonaEnrichment> {
    vec![
        rust_enrichment(),
        python_enrichment(),
        fullstack_ts_enrichment(),
        devops_enrichment(),
        mobile_enrichment(),
        bootstrap_enrichment(),
        power_user_enrichment(),
        context_switcher_enrichment(),
        niche_specialist_enrichment(),
    ]
}
