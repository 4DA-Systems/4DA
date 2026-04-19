// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use once_cell::sync::Lazy;
use std::collections::HashSet;

// ============================================================================
// Topic Extraction
// ============================================================================

/// Single-word topic keywords — O(1) lookup via HashSet
static SINGLE_WORD_TOPICS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "rust",
        "python",
        "javascript",
        "typescript",
        "go",
        "golang",
        "java",
        "cpp",
        "react",
        "vue",
        "angular",
        "svelte",
        "solid",
        "qwik",
        "preact",
        "next",
        "nextjs",
        "nuxt",
        "remix",
        "gatsby",
        "astro",
        "node",
        "deno",
        "bun",
        "express",
        "fastify",
        "koa",
        "hapi",
        "nest",
        "nestjs",
        "ai",
        "ml",
        "neural",
        "gpt",
        "llm",
        "transformer",
        "database",
        "sql",
        "postgresql",
        "postgres",
        "mysql",
        "mongodb",
        "redis",
        "sqlite",
        "tauri",
        "electron",
        "vite",
        "webpack",
        "esbuild",
        "rollup",
        "turbopack",
        "parcel",
        "tailwind",
        "tailwindcss",
        "bootstrap",
        "prisma",
        "drizzle",
        "sequelize",
        "typeorm",
        "mongoose",
        "diesel",
        "sqlx",
        "django",
        "flask",
        "fastapi",
        "laravel",
        "rails",
        "spring",
        "gin",
        "fiber",
        "echo",
        "axum",
        "actix",
        "tokio",
        "reqwest",
        "serde",
        "warp",
        "rocket",
        "hyper",
        "tower",
        "tonic",
        "pnpm",
        "yarn",
        "npm",
        "cargo",
        "pip",
        "kubernetes",
        "k8s",
        "docker",
        "container",
        "terraform",
        "ansible",
        "pulumi",
        "aws",
        "azure",
        "gcp",
        "cloud",
        "vercel",
        "netlify",
        "cloudflare",
        "supabase",
        "firebase",
        "api",
        "rest",
        "graphql",
        "grpc",
        "microservice",
        "crypto",
        "cryptocurrency",
        "bitcoin",
        "ethereum",
        "blockchain",
        "nft",
        "web3",
        "defi",
        "startup",
        "vc",
        "funding",
        "acquisition",
        "oss",
        "github",
        "git",
        "security",
        "vulnerability",
        "hack",
        "breach",
        "performance",
        "optimization",
        "scale",
        "scalability",
        "frontend",
        "backend",
        "fullstack",
        "devops",
        "sre",
        "linux",
        "unix",
        "windows",
        "macos",
        "mobile",
        "ios",
        "android",
        "flutter",
        "game",
        "gaming",
        "gamedev",
        "hardware",
        "chip",
        "semiconductor",
        "cpu",
        "gpu",
        "climate",
        "sustainability",
        "energy",
        "sports",
        "football",
        "basketball",
        "soccer",
        "politics",
        "election",
        "government",
        // Cross-cutting developer concerns (universal — every stack cares about these)
        "architecture",
        "testing",
        "deployment",
        "monitoring",
        "accessibility",
        "debugging",
        "refactoring",
        "caching",
        "authentication",
        "authorization",
        "observability",
        "logging",
        "profiling",
        "benchmarking",
        "migration",
        "concurrency",
        "parallelism",
        "networking",
        "websocket",
        "streaming",
        "compiler",
        "interpreter",
        "documentation",
        "linting",
        "packaging",
    ]
    .into_iter()
    .collect()
});

/// Multi-word topic phrases — small enough for linear scan
static MULTI_WORD_TOPICS: &[&str] = &[
    "c++",
    "machine learning",
    "deep learning",
    "open source",
    "react native",
    "next.js",
    "nuxt.js",
    "vue.js",
    "node.js",
    "styled-components",
    "material-ui",
    "shadcn/ui",
    "ruby on rails",
    // Cross-cutting multi-word concerns
    "unit testing",
    "integration testing",
    "load testing",
    "design patterns",
    "best practices",
    "code review",
    "continuous integration",
    "continuous deployment",
];

/// Stopwords excluded from capitalized-word extraction in titles
const TITLE_STOPWORDS: &[&str] = &[
    // Articles, conjunctions, prepositions
    "the",
    "and",
    "for",
    "how",
    "why",
    "what",
    "show",
    "ask",
    "with",
    "from",
    "into",
    "about",
    "this",
    "that",
    "your",
    "our",
    "their",
    "some",
    "any",
    "all",
    "every",
    "each",
    "more",
    "most",
    "many",
    "much",
    "also",
    "just",
    "very",
    "still",
    "not",
    "but",
    "yet",
    "here",
    "there",
    "when",
    "where",
    "will",
    "can",
    "should",
    "could",
    "would",
    // Generic verbs / gerunds (capitalized in titles, useless as topics)
    "using",
    "building",
    "working",
    "making",
    "getting",
    "running",
    "creating",
    "developing",
    "announcing",
    "introducing",
    "launching",
    "deploying",
    "implementing",
    "understanding",
    "exploring",
    "discussing",
    "comparing",
    "improving",
    "fixing",
    "breaking",
    "starting",
    "looking",
    "moving",
    "keeping",
    "finding",
    "writing",
    "reading",
    "learning",
    "teaching",
    "testing",
    "trying",
    "adding",
    "removing",
    "setting",
    "built",
    "made",
    "released",
    // Generic adjectives / nouns
    "new",
    "best",
    "first",
    "free",
    "fast",
    "easy",
    "simple",
    "better",
    "modern",
    "full",
    "real",
    "good",
    "great",
    "top",
    "key",
    "big",
    "small",
    "open",
    "way",
    "part",
    "time",
    "year",
    "week",
    "month",
    "day",
    "thing",
    "guide",
    "tips",
    "tool",
    "tools",
    "list",
    "need",
    "help",
    "project",
    "projects",
    "update",
    "version",
];

/// Extract topics/keywords from text for context matching
/// Returns lowercase keywords suitable for exclusion/interest matching
/// Optimized: O(1) HashSet lookup for single-word topics, linear scan only for multi-word phrases
pub(crate) fn extract_topics(title: &str, content: &str) -> Vec<String> {
    // Combine title and first part of content
    let text = format!(
        "{} {}",
        title,
        content.chars().take(500).collect::<String>()
    );
    let text_lower = text.to_lowercase();

    let mut topics = Vec::new();
    let mut seen = HashSet::new();

    // O(1) lookup for single-word topics: split into words, check each against HashSet
    for word in text_lower.split(|c: char| !c.is_alphanumeric() && c != '+' && c != '#') {
        if word.len() >= 2 && SINGLE_WORD_TOPICS.contains(word) && seen.insert(word.to_string()) {
            topics.push(word.to_string());
        }
    }

    // Linear scan for multi-word phrases (only ~5 entries)
    for &phrase in MULTI_WORD_TOPICS {
        if text_lower.contains(phrase) && seen.insert(phrase.to_string()) {
            topics.push(phrase.to_string());
        }
    }

    // Also extract capitalized words from title as potential topics
    for word in title.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
        if clean.len() > 2 && clean.chars().next().is_some_and(char::is_uppercase) {
            let lower = clean.to_lowercase();
            if !seen.contains(&lower)
                && !TITLE_STOPWORDS.contains(&lower.as_str())
                && seen.insert(lower.clone())
            {
                topics.push(lower);
            }
        }
    }

    topics
}

/// Detect trending topics from a batch of items.
/// A topic is "trending" when 3+ items in the current batch share it,
/// indicating multiple sources are reporting on it simultaneously.
pub(crate) fn detect_trend_topics<'a>(
    items: impl Iterator<Item = (&'a str, &'a str)>,
) -> Vec<String> {
    let mut topic_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for (title, content) in items {
        let topics = extract_topics(title, content);
        for topic in topics {
            *topic_counts.entry(topic).or_insert(0) += 1;
        }
    }
    topic_counts
        .into_iter()
        .filter(|(_, count)| *count >= 3)
        .map(|(topic, _)| topic)
        .collect()
}

/// Check if an item should be excluded based on user exclusions
/// Returns Some(exclusion) if blocked, None if allowed
pub(crate) fn check_exclusions(topics: &[String], exclusions: &[String]) -> Option<String> {
    for topic in topics {
        let topic_lower = topic.to_lowercase();
        for exclusion in exclusions {
            let exclusion_lower = exclusion.to_lowercase();
            if topic_lower.contains(&exclusion_lower) || exclusion_lower.contains(&topic_lower) {
                return Some(exclusion.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_topics_basic() {
        let topics = extract_topics(
            "Rust async patterns for Tauri apps",
            "A guide to async Rust",
        );
        assert!(!topics.is_empty());
        // Should extract meaningful words, not stopwords
        assert!(topics
            .iter()
            .any(|t| t.contains("rust") || t.contains("tauri") || t.contains("async")));
    }

    #[test]
    fn test_extract_topics_empty() {
        let topics = extract_topics("", "");
        assert!(topics.is_empty());
    }

    #[test]
    fn test_extract_topics_optimized() {
        // Test single-word keyword extraction
        let topics = extract_topics(
            "Building a Rust web server",
            "Using async/await with PostgreSQL database",
        );
        assert!(
            topics.contains(&"rust".to_string()),
            "Should extract 'rust'"
        );
        assert!(
            topics.contains(&"postgresql".to_string()),
            "Should extract 'postgresql'"
        );
        assert!(
            topics.contains(&"database".to_string()),
            "Should extract 'database'"
        );

        // Test multi-word phrase extraction
        let topics2 = extract_topics(
            "Machine Learning with Python",
            "Deep learning and open source tools",
        );
        assert!(
            topics2.contains(&"machine learning".to_string()),
            "Should extract 'machine learning'"
        );
        assert!(
            topics2.contains(&"deep learning".to_string()),
            "Should extract 'deep learning'"
        );
        assert!(
            topics2.contains(&"open source".to_string()),
            "Should extract 'open source'"
        );
        assert!(
            topics2.contains(&"python".to_string()),
            "Should extract 'python'"
        );

        // Test special character handling (c++)
        let topics3 = extract_topics("C++ programming", "Using C++ for systems programming");
        assert!(topics3.contains(&"c++".to_string()), "Should extract 'c++'");

        // Test no duplicates
        let topics4 = extract_topics("Rust Rust Rust", "rust rust rust everywhere");
        let rust_count = topics4.iter().filter(|t| *t == "rust").count();
        assert_eq!(rust_count, 1, "Should not have duplicates");
    }

    #[test]
    fn test_check_exclusions_none() {
        let topics = vec!["rust".to_string(), "webdev".to_string()];
        let exclusions = vec!["crypto".to_string()];
        assert!(check_exclusions(&topics, &exclusions).is_none());
    }

    #[test]
    fn test_check_exclusions_match() {
        let topics = vec!["rust".to_string(), "cryptocurrency".to_string()];
        let exclusions = vec!["crypto".to_string()];
        let result = check_exclusions(&topics, &exclusions);
        assert!(result.is_some(), "Should match 'crypto' substring");
    }

    #[test]
    fn test_extract_topics_multiword_phrases() {
        let topics = extract_topics("Machine Learning with Open Source tools", "");
        assert!(topics.contains(&"machine learning".to_string()));
        assert!(topics.contains(&"open source".to_string()));
    }

    #[test]
    fn test_extract_topics_no_stopwords() {
        let topics = extract_topics("The Best New Way For Your Project", "");
        // None of these stopwords should appear in topics (they're in the exclusion list)
        assert!(!topics.contains(&"the".to_string()));
        assert!(!topics.contains(&"best".to_string()));
        assert!(!topics.contains(&"new".to_string()));
        assert!(!topics.contains(&"way".to_string()));
        assert!(!topics.contains(&"for".to_string()));
        assert!(!topics.contains(&"your".to_string()));
        assert!(!topics.contains(&"project".to_string()));
    }

    #[test]
    fn test_extract_topics_known_single_keywords() {
        let topics = extract_topics("docker kubernetes aws", "");
        assert!(topics.contains(&"docker".to_string()));
        assert!(topics.contains(&"kubernetes".to_string()));
        assert!(topics.contains(&"aws".to_string()));
    }

    #[test]
    fn test_extract_topics_capitalized_words_from_title() {
        let topics = extract_topics("Building Tauri Desktop Apps", "");
        assert!(topics.contains(&"tauri".to_string()));
    }

    #[test]
    fn test_extract_topics_content_truncation() {
        // Content is truncated to first 500 chars
        let long_content = "x ".repeat(300); // 600 chars
        let topics = extract_topics("Rust", &long_content);
        assert!(topics.contains(&"rust".to_string()));
    }

    #[test]
    fn test_check_exclusions_case_insensitive() {
        let topics = vec!["Crypto".to_string()];
        let exclusions = vec!["crypto".to_string()];
        assert!(check_exclusions(&topics, &exclusions).is_some());
    }

    #[test]
    fn test_check_exclusions_partial_match() {
        let topics = vec!["cryptocurrency".to_string()];
        let exclusions = vec!["crypto".to_string()];
        assert!(check_exclusions(&topics, &exclusions).is_some());
    }

    #[test]
    fn test_check_exclusions_empty_topics() {
        let topics: Vec<String> = vec![];
        let exclusions = vec!["crypto".to_string()];
        assert!(check_exclusions(&topics, &exclusions).is_none());
    }

    #[test]
    fn test_check_exclusions_empty_exclusions() {
        let topics = vec!["rust".to_string()];
        let exclusions: Vec<String> = vec![];
        assert!(check_exclusions(&topics, &exclusions).is_none());
    }
}
