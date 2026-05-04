// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Content sophistication scoring — detects content complexity/depth level
//! independent of topic relevance. Used to filter beginner tutorials for
//! experienced developers and surface advanced content appropriately.

use crate::domain_profile::DomainProfile;

/// Result of sophistication analysis
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SophisticationScore {
    pub title_complexity: f32,
    pub content_depth: f32,
    pub audience_is_senior: bool,
    pub multiplier: f32,
}

/// Compute content sophistication and return an audience-aware multiplier.
///
/// Multiplier range: [0.60, 1.15]
/// - Advanced content (>= 0.5 sophistication) + senior audience: 1.15 (boost)
/// - Moderate content (0.3..0.5): 1.00 (neutral)
/// - Beginner content (< 0.3) + senior audience: 0.60 (strong penalty)
///
/// `detected_tech_count` is the number of ACE-detected technologies (from ACEContext).
pub fn compute_sophistication(
    title: &str,
    content: &str,
    detected_tech_count: usize,
    domain_profile: &DomainProfile,
) -> SophisticationScore {
    let title_complexity = assess_title_complexity(title);
    let content_depth = assess_content_depth_signals(content);
    let audience_is_senior = infer_senior_audience(detected_tech_count, domain_profile);

    // Combined sophistication: title has more weight (content is often truncated/missing)
    let sophistication = title_complexity * 0.6 + content_depth * 0.4;

    let multiplier = compute_multiplier(sophistication, audience_is_senior);

    SophisticationScore {
        title_complexity,
        content_depth,
        audience_is_senior,
        multiplier,
    }
}

fn compute_multiplier(sophistication: f32, is_senior: bool) -> f32 {
    if sophistication >= 0.5 {
        if is_senior {
            1.15
        } else {
            1.05
        }
    } else if sophistication < 0.3 {
        if is_senior {
            0.60
        } else {
            0.90
        }
    } else {
        1.0
    }
}

// --- Title Complexity ---------------------------------------------------------

fn assess_title_complexity(title: &str) -> f32 {
    let lower = title.to_lowercase();
    let mut score: f32 = 0.0;

    // Advanced technical terms (high-signal terms only)
    let advanced_hits = count_term_hits(&lower, ADVANCED_TERMS);
    score += (advanced_hits as f32 * 0.20).min(0.50);

    // Moderate technical terms (intermediate-signal — indicate non-beginner content)
    let moderate_hits = count_term_hits(&lower, MODERATE_TERMS);
    score += (moderate_hits as f32 * 0.10).min(0.25);

    // Compound/hyphenated terms indicate technical depth
    let hyphen_count = title.matches('-').count();
    let technical_hyphens = hyphen_count.saturating_sub(1);
    score += (technical_hyphens as f32 * 0.08).min(0.16);

    // Version specificity (RFC, v4.2, etc.)
    if has_version_specificity(&lower) {
        score += 0.15;
    }

    // Abstract/architectural framing
    if has_abstract_framing(&lower) {
        score += 0.15;
    }

    // Word count + average word length
    let words: Vec<&str> = title.split_whitespace().filter(|w| w.len() >= 2).collect();
    if words.len() >= 7 {
        score += 0.10;
    }
    let avg_word_len = if words.is_empty() {
        0.0
    } else {
        words.iter().map(|w| w.len() as f32).sum::<f32>() / words.len() as f32
    };
    if avg_word_len >= 6.0 {
        score += 0.10;
    }

    // Beginner counter-indicators
    let beginner_hits = count_term_hits(&lower, BEGINNER_TITLE_TERMS);
    score -= beginner_hits as f32 * 0.15;

    score.clamp(0.0, 1.0)
}

// --- Content Depth ------------------------------------------------------------

fn assess_content_depth_signals(content: &str) -> f32 {
    if content.is_empty() {
        return 0.5; // neutral when no content available
    }
    let lower = content.to_lowercase();
    let mut score: f32 = 0.0;

    // Code sophistication: error handling, generics, async patterns
    let code_sophistication_hits = count_term_hits(&lower, CODE_SOPHISTICATION_MARKERS);
    score += (code_sophistication_hits as f32 * 0.10).min(0.30);

    // Structural depth markers
    let depth_hits = count_term_hits(&lower, DEPTH_MARKERS);
    score += (depth_hits as f32 * 0.08).min(0.25);

    // Reference density
    let ref_hits = count_term_hits(&lower, REFERENCE_MARKERS);
    score += (ref_hits as f32 * 0.10).min(0.20);

    // Counter-indicators: step-by-step walkthroughs
    let walkthrough_hits = count_term_hits(&lower, WALKTHROUGH_MARKERS);
    score -= walkthrough_hits as f32 * 0.10;

    // Code block length heuristic
    let code_block_lines = count_code_block_lines(content);
    if code_block_lines > 30 {
        score += 0.15;
    } else if code_block_lines > 10 {
        score += 0.08;
    }

    score.clamp(0.0, 1.0)
}

// --- Audience Inference -------------------------------------------------------

fn infer_senior_audience(detected_tech_count: usize, domain_profile: &DomainProfile) -> bool {
    let dependency_count = domain_profile.dependency_names.len();
    let all_tech_count = domain_profile.all_tech.len();

    // Original threshold: proven-senior (lots of deps and tech)
    if detected_tech_count > 15 && dependency_count > 50 && all_tech_count > 15 {
        return true;
    }

    // Bootstrapping-senior: multiple distinct tech families detected.
    // Multi-stack developers are experienced regardless of dep count.
    if detected_tech_count >= 5 {
        let distinct_families = count_tech_families(&domain_profile.all_tech);
        if distinct_families >= 3 {
            return true;
        }
    }

    false
}

/// Count distinct technology families present in the user's detected tech.
/// Maps individual technologies to high-level families.
/// 3+ families = multi-stack developer (experienced).
fn count_tech_families(all_tech: &std::collections::HashSet<String>) -> usize {
    let mut families = std::collections::HashSet::new();

    for tech in all_tech {
        let tech_lower = tech.to_lowercase();
        let family = match tech_lower.as_str() {
            // Frontend
            "react" | "vue" | "angular" | "svelte" | "next" | "nuxt" | "nextjs" | "next.js"
            | "gatsby" | "remix" | "solid" | "preact" | "lit" | "astro" | "vite" | "webpack"
            | "tailwind" | "tailwindcss" | "css" | "sass" | "html" => Some("frontend"),

            // Backend
            "express" | "django" | "flask" | "rails" | "spring" | "actix" | "axum" | "rocket"
            | "fastapi" | "gin" | "echo" | "fiber" | "koa" | "nest" | "nestjs" | "hono"
            | "node" | "nodejs" | "deno" | "bun" => Some("backend"),

            // Systems
            "rust" | "go" | "golang" | "c" | "cpp" | "c++" | "zig" | "assembly" | "wasm"
            | "webassembly" | "tauri" | "electron" => Some("systems"),

            // Data
            "postgresql" | "postgres" | "mysql" | "sqlite" | "mongodb" | "redis"
            | "elasticsearch" | "cassandra" | "dynamodb" | "supabase" | "firebase" | "prisma"
            | "drizzle" | "sqlalchemy" | "diesel" | "sea-orm" => Some("data"),

            // DevOps
            "docker" | "kubernetes" | "k8s" | "terraform" | "ansible" | "github-actions"
            | "gitlab-ci" | "jenkins" | "circleci" | "nginx" | "caddy" | "traefik" | "aws"
            | "gcp" | "azure" => Some("devops"),

            // Mobile
            "swift" | "kotlin" | "react-native" | "flutter" | "dart" | "ios" | "android"
            | "expo" | "capacitor" => Some("mobile"),

            // Languages (counted only if not already covered by framework families)
            "typescript" | "javascript" => Some("frontend"),
            "python" => Some("backend"),
            "java" => Some("backend"),
            "ruby" => Some("backend"),
            "php" => Some("backend"),
            "elixir" | "erlang" => Some("backend"),

            _ => None,
        };

        if let Some(f) = family {
            families.insert(f);
        }
    }

    families.len()
}

// --- Helpers ------------------------------------------------------------------

fn count_term_hits(text: &str, terms: &[&str]) -> usize {
    terms.iter().filter(|&&term| text.contains(term)).count()
}

fn has_version_specificity(lower: &str) -> bool {
    lower.contains("rfc ")
        || lower.contains("rfc-")
        || (lower.contains(" v") && lower.chars().any(|c| c.is_ascii_digit()))
        || lower.contains("cve-")
        || lower.contains("spec ")
        || lower.contains("specification")
}

fn has_abstract_framing(lower: &str) -> bool {
    ABSTRACT_FRAMING.iter().any(|&pat| lower.contains(pat))
}

fn count_code_block_lines(content: &str) -> usize {
    let mut in_block = false;
    let mut count = 0;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_block = !in_block;
        } else if in_block {
            count += 1;
        }
    }
    count
}

// --- Term Lists ---------------------------------------------------------------

pub(crate) const ADVANCED_TERMS: &[&str] = &[
    "allocator",
    "monomorphization",
    "vtable",
    "zero-copy",
    "lock-free",
    "wait-free",
    "linearizability",
    "consensus",
    "raft",
    "paxos",
    "sharding",
    "partitioning",
    "replication",
    "backpressure",
    "circuit breaker",
    "saga pattern",
    "event sourcing",
    "cqrs",
    "merkle",
    "b-tree",
    "lsm",
    "bloom filter",
    "skip list",
    "futex",
    "spinlock",
    "rwlock",
    "atomic",
    "memory ordering",
    "cache coherence",
    "false sharing",
    "simd",
    "vectorization",
    "branch prediction",
    "prefetch",
    "memory-mapped",
    "io_uring",
    "epoll",
    "kqueue",
    "completion port",
    "dependent type",
    "higher-kinded",
    "existential type",
    "phantom type",
    "variance",
    "covariance",
    "contravariance",
    "type erasure",
    "monomorphize",
    "trait object",
    "dyn dispatch",
    "static dispatch",
    "lifetime elision",
    "borrow checker",
    "affine type",
    "linear type",
    "crdt",
    "vector clock",
    "lamport",
    "causal consistency",
    "eventual consistency",
    "strong consistency",
    "two-phase commit",
    "three-phase commit",
    "gossip protocol",
    "anti-entropy",
    "timing attack",
    "side channel",
    "constant-time",
    "memory safety",
    "use-after-free",
    "buffer overflow",
    "stack canary",
    "aslr",
    "sandboxing",
    "capability-based",
    "zero-knowledge",
    "eigenvalue",
    "gradient descent",
    "backpropagation",
    "transformer",
    "attention mechanism",
    "embedding space",
    "dimensionality reduction",
    "kernel trick",
    "regularization",
    "overfitting",
    "quic",
    "http/3",
    "tls 1.3",
    "mtls",
    "service mesh",
    "grpc",
    "protobuf",
    "flatbuffers",
    "cap theorem",
    "write-ahead log",
    "wal",
    "mvcc",
    "snapshot isolation",
    "serializable",
    "vacuum",
    "compaction",
    "tombstone",
];

const MODERATE_TERMS: &[&str] = &[
    "plugin",
    "system",
    "engine",
    "compiler",
    "runtime",
    "framework",
    "middleware",
    "pipeline",
    "scheduler",
    "resolver",
    "renderer",
    "parser",
    "generator",
    "daemon",
    "proxy",
    "gateway",
    "adapter",
    "driver",
    "dispatcher",
    "orchestrat",
    "migration",
    "refactor",
    "serialization",
    "deserialization",
];

const BEGINNER_TITLE_TERMS: &[&str] = &[
    "getting started",
    "for beginners",
    "beginner's guide",
    "beginners guide",
    "your first",
    "hello world",
    "from scratch",
    "made easy",
    "simplified",
    "for dummies",
    "crash course",
    "in 5 minutes",
    "in 10 minutes",
    "in 15 minutes",
    "quick start",
    "quickstart",
    "learn to",
    "learn how to",
    " 101",
    "basics of",
    "fundamentals of",
    "what is ",
    "intro to ",
    "introduction to ",
    "complete beginner",
    "absolute beginner",
    "step by step",
    "step-by-step",
    "full project breakdown",
];

const ABSTRACT_FRAMING: &[&str] = &[
    "principles of",
    "patterns in",
    "architecture of",
    "design of",
    "internals of",
    "under the hood",
    "deep dive",
    "in depth",
    "in-depth",
    "trade-offs",
    "tradeoffs",
    "trade offs",
    "beyond the basics",
    "advanced ",
    "production-grade",
    "production grade",
    "at scale",
    "high-performance",
    "high performance",
];

const CODE_SOPHISTICATION_MARKERS: &[&str] = &[
    "result<",
    "-> result",
    "anyhow::",
    "thiserror",
    "unwrap_or",
    "unwrap_or_else",
    "map_err",
    "async fn",
    "async move",
    ".await",
    "impl ",
    "trait ",
    "where ",
    "<t>",
    "<t,",
    "generic",
    "lifetime",
    "pin<",
    "arc<",
    "mutex<",
    "unsafe ",
    "try {",
    "try!(",
    "catch (",
    "promise<",
    "async/await",
    "observable",
];

const DEPTH_MARKERS: &[&str] = &[
    "trade-off",
    "tradeoff",
    "limitation",
    "edge case",
    "edge-case",
    "anti-pattern",
    "antipattern",
    "gotcha",
    "caveat",
    "pitfall",
    "when not to",
    "when to avoid",
    "downside",
    "drawback",
    "benchmark",
    "profiling",
    "flame graph",
    "perf ",
    "regression",
    "migration path",
];

const REFERENCE_MARKERS: &[&str] = &[
    "rfc ",
    "arxiv",
    "doi.org",
    "ieee",
    "acm.org",
    "spec ",
    "specification",
    "official doc",
    "documentation",
    "changelog",
    "release note",
    "github.com/",
];

const WALKTHROUGH_MARKERS: &[&str] = &[
    "step 1",
    "step 2",
    "step 3",
    "step one",
    "step two",
    "first, install",
    "first install",
    "npm install",
    "pip install",
    "let's create",
    "let's build",
    "let's start",
    "we'll create",
    "we will create",
    "open your terminal",
    "open your editor",
    "create a new file",
    "create a new project",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const SENIOR_TECH_COUNT: usize = 25;
    const JUNIOR_TECH_COUNT: usize = 2;

    fn mock_domain_senior() -> DomainProfile {
        let mut dp = DomainProfile::default();
        dp.all_tech = (0..20).map(|i| format!("tech_{i}")).collect();
        dp.dependency_names = (0..60).map(|i| format!("dep_{i}")).collect();
        dp
    }

    fn mock_domain_junior() -> DomainProfile {
        let mut dp = DomainProfile::default();
        dp.all_tech = ["react".to_string()].into_iter().collect();
        dp.dependency_names = ["react".to_string(), "vite".to_string()]
            .into_iter()
            .collect();
        dp
    }

    #[test]
    fn beginner_title_scores_low() {
        let score = assess_title_complexity("Getting Started with React - A Beginner's Guide");
        assert!(score < 0.3, "Expected <0.3, got {score}");
    }

    #[test]
    fn advanced_title_scores_high() {
        let score = assess_title_complexity(
            "Lock-Free CRDT Consensus in Distributed Systems with Causal Consistency",
        );
        assert!(score > 0.6, "Expected >0.6, got {score}");
    }

    #[test]
    fn moderate_title_scores_middle() {
        let score = assess_title_complexity("Building a Plugin System for a Tauri App");
        assert!(
            (0.2..=0.7).contains(&score),
            "Expected 0.2-0.7, got {score}"
        );
    }

    #[test]
    fn senior_audience_penalizes_beginner_content() {
        let dp = mock_domain_senior();
        let result =
            compute_sophistication("Getting Started with React", "", SENIOR_TECH_COUNT, &dp);
        assert!(
            result.multiplier <= 0.60,
            "Expected <=0.60, got {}",
            result.multiplier
        );
        assert!(result.audience_is_senior);
    }

    #[test]
    fn senior_audience_boosts_advanced_content() {
        let dp = mock_domain_senior();
        let result = compute_sophistication(
            "Zero-Copy Deserialization with Lifetime Elision in Rust",
            "impl<'a> Deserialize<'a> for MyStruct { fn deserialize() -> Result<Self> { ... } }",
            SENIOR_TECH_COUNT,
            &dp,
        );
        assert!(
            result.multiplier >= 1.10,
            "Expected >=1.10, got {}",
            result.multiplier
        );
    }

    #[test]
    fn junior_audience_mild_beginner_penalty() {
        let dp = mock_domain_junior();
        let result =
            compute_sophistication("Getting Started with React", "", JUNIOR_TECH_COUNT, &dp);
        assert!(
            result.multiplier >= 0.85,
            "Junior should get mild penalty, got {}",
            result.multiplier
        );
    }

    #[test]
    fn code_depth_with_error_handling_scores_higher() {
        let simple = assess_content_depth_signals("console.log('hello world');");
        let complex = assess_content_depth_signals(
            "async fn process() -> Result<Data> { let conn = pool.get().await.map_err(|e| ...); }",
        );
        assert!(
            complex > simple,
            "Complex ({complex}) should > simple ({simple})"
        );
    }

    #[test]
    fn multiplier_range_is_valid() {
        for soph in [0.0, 0.15, 0.3, 0.5, 0.7, 0.85, 1.0] {
            for senior in [true, false] {
                let m = compute_multiplier(soph, senior);
                assert!(
                    (0.60..=1.15).contains(&m),
                    "Out of range: soph={soph} senior={senior} m={m}"
                );
            }
        }
    }

    #[test]
    fn typing_master_tutorial_scores_low() {
        let result = compute_sophistication(
            "Typing Master Web App React + Vite -- Full Project Breakdown",
            "Step 1: Create a new Vite project. npm install. Step 2: Create components.",
            SENIOR_TECH_COUNT,
            &mock_domain_senior(),
        );
        assert!(
            result.multiplier <= 0.60,
            "Tutorial penalized for senior, got {}",
            result.multiplier
        );
    }

    #[test]
    fn bootstrapping_senior_multi_stack() {
        let profile = DomainProfile {
            all_tech: HashSet::from([
                "react".to_string(),
                "express".to_string(),
                "rust".to_string(),
                "sqlite".to_string(),
            ]),
            dependency_names: HashSet::new(),
            ..Default::default()
        };
        // 4 families: frontend, backend, systems, data
        assert!(infer_senior_audience(5, &profile));
    }

    #[test]
    fn single_family_not_senior() {
        let profile = DomainProfile {
            all_tech: HashSet::from([
                "react".to_string(),
                "typescript".to_string(),
                "next".to_string(),
                "tailwind".to_string(),
            ]),
            dependency_names: HashSet::new(),
            ..Default::default()
        };
        // All frontend = 1 family
        assert!(!infer_senior_audience(5, &profile));
    }

    #[test]
    fn three_families_is_senior() {
        let profile = DomainProfile {
            all_tech: HashSet::from([
                "python".to_string(),
                "django".to_string(),
                "postgresql".to_string(),
                "docker".to_string(),
            ]),
            dependency_names: HashSet::new(),
            ..Default::default()
        };
        // backend, data, devops = 3 families
        assert!(infer_senior_audience(5, &profile));
    }

    #[test]
    fn too_few_detected_tech_not_senior() {
        let profile = DomainProfile {
            all_tech: HashSet::from([
                "react".to_string(),
                "rust".to_string(),
                "sqlite".to_string(),
            ]),
            dependency_names: HashSet::new(),
            ..Default::default()
        };
        // 3 families but only 3 detected_tech (< 5 minimum)
        assert!(!infer_senior_audience(3, &profile));
    }
}
