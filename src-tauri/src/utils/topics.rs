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
    // Sentence openers and adverbs (capitalised by position, never names)
    "actually",
    "act",
    "action",
    "actions",
    "really",
    "finally",
    "today",
    "inside",
    "beyond",
    "observation",
    "observations",
    "reminder",
    "note",
    "notes",
    "updates",
    "psa",
    "til",
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

/// Extract topics/keywords from text and structured source tags for context matching.
/// Returns lowercase keywords suitable for exclusion/interest matching.
/// Optimized: O(1) HashSet lookup for single-word topics, linear scan only for multi-word phrases.
///
/// `source_tags` are structured tags from source metadata (SO tags, Dev.to tags, arXiv categories
/// mapped to topic vocabulary, etc.). These are trusted — they bypass text scanning and are
/// accepted directly if they match our topic vocabulary, ensuring all sources get fair signal
/// generation regardless of their content format.
pub(crate) fn extract_topics(title: &str, content: &str, source_tags: &[String]) -> Vec<String> {
    let mut topics = Vec::new();
    let mut seen = HashSet::new();

    // Phase 1: Structured source tags (highest priority — pre-validated by source community)
    for tag in source_tags {
        let lower = tag.to_lowercase();
        // Accept tags that match our vocabulary directly
        if SINGLE_WORD_TOPICS.contains(lower.as_str()) && seen.insert(lower.clone()) {
            topics.push(lower);
            continue;
        }
        // Check multi-word phrases
        for &phrase in MULTI_WORD_TOPICS {
            if lower == phrase && seen.insert(phrase.to_string()) {
                topics.push(phrase.to_string());
            }
        }
        // Accept hyphenated compound tags by checking each component
        // e.g., "async-await" → check "async" and "await"
        if lower.contains('-') {
            for part in lower.split('-') {
                if part.len() >= 2
                    && SINGLE_WORD_TOPICS.contains(part)
                    && seen.insert(part.to_string())
                {
                    topics.push(part.to_string());
                }
            }
        }
    }

    // Phase 2: Text-based extraction from title + content (same as before)
    let text = format!(
        "{} {}",
        title,
        content.chars().take(500).collect::<String>()
    );
    let text_lower = text.to_lowercase();

    // O(1) lookup for single-word topics: split into words, check each against HashSet
    for word in text_lower.split(|c: char| !c.is_alphanumeric() && c != '+' && c != '#') {
        if word.len() >= 2 && SINGLE_WORD_TOPICS.contains(word) && seen.insert(word.to_string()) {
            topics.push(word.to_string());
        }
    }

    // Linear scan for multi-word phrases (only ~19 entries)
    for &phrase in MULTI_WORD_TOPICS {
        if text_lower.contains(phrase) && seen.insert(phrase.to_string()) {
            topics.push(phrase.to_string());
        }
    }

    // Phase 3: Capitalized words from title as potential topics. A Title
    // Case headline ("What Actually Becomes the Moat", "Privacy Act
    // Reforms", "Quick thoughts on GitHub Actions") capitalises every word,
    // so capitalisation carries no name signal there — live 2026-09-08 the
    // brief's escalating section listed `act`, `action` and `actually`
    // signal chains. In a Title Case title only a word something else marks
    // as a name counts (an interior capital or a digit: TypeScript, GitHub,
    // S3); in a sentence-case title any capitalised word does, minus the
    // stopwords.
    let words: Vec<&str> = title
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() > 2)
        .collect();
    let title_case = is_title_case(&words);
    for clean in words {
        if !clean.chars().next().is_some_and(char::is_uppercase) {
            continue;
        }
        if title_case && !is_marked_name(clean) {
            continue;
        }
        let lower = clean.to_lowercase();
        if !seen.contains(&lower)
            && !TITLE_STOPWORDS.contains(&lower.as_str())
            && seen.insert(lower.clone())
        {
            topics.push(lower);
        }
    }

    topics
}

/// A headline styled in Title Case: at least four significant words (four
/// letters or more) of which at most one is lowercase-initial — "Getting
/// Started with Rust" still qualifies, "Toasty is an async ORM for Rust"
/// does not. There, capitalisation is typography, not naming.
fn is_title_case(words: &[&str]) -> bool {
    let significant: Vec<&&str> = words.iter().filter(|w| w.len() >= 4).collect();
    if significant.len() < 4 {
        return false;
    }
    let lowercase = significant
        .iter()
        .filter(|w| w.chars().next().is_some_and(char::is_lowercase))
        .count();
    lowercase <= 1
}

/// An interior capital or a digit marks a name regardless of typography
/// (TypeScript, GitHub, S3, ZenHub).
fn is_marked_name(word: &str) -> bool {
    word.chars().skip(1).any(char::is_uppercase) || word.chars().any(|c| c.is_ascii_digit())
}

/// Detect trending topics from a batch of items.
/// A topic is "trending" when 3+ items in the current batch share it,
/// indicating multiple sources are reporting on it simultaneously.
pub(crate) fn detect_trend_topics<'a>(
    items: impl Iterator<Item = (&'a str, &'a str)>,
) -> Vec<String> {
    let mut topic_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut item_count: u32 = 0;
    for (title, content) in items {
        item_count += 1;
        let topics = extract_topics(title, content, &[]);
        for topic in topics {
            *topic_counts.entry(topic).or_insert(0) += 1;
        }
    }
    // A topic carried by more than `max_topic_share` of a large batch is the
    // batch's SUBJECT, not a trend: the 500-item drain chunks made
    // rust/react/javascript "trend" every cycle and handed every on-domain
    // item a blanket +0.08 (measured live 2026-09-04). The share cap only
    // applies once the batch is big enough for a share to mean anything.
    let share_cap = if item_count >= crate::scoring_config::TREND_BOOST_MIN_BATCH_ITEMS as u32 {
        Some((item_count as f32 * crate::scoring_config::TREND_BOOST_MAX_TOPIC_SHARE).ceil() as u32)
    } else {
        None
    };
    topic_counts
        .into_iter()
        .filter(|(_, count)| *count >= 3 && share_cap.is_none_or(|cap| *count <= cap))
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
            &[],
        );
        assert!(!topics.is_empty());
        // Should extract meaningful words, not stopwords
        assert!(topics
            .iter()
            .any(|t| t.contains("rust") || t.contains("tauri") || t.contains("async")));
    }

    #[test]
    fn test_extract_topics_empty() {
        let topics = extract_topics("", "", &[]);
        assert!(topics.is_empty());
    }

    #[test]
    fn test_extract_topics_optimized() {
        // Test single-word keyword extraction
        let topics = extract_topics(
            "Building a Rust web server",
            "Using async/await with PostgreSQL database",
            &[],
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
            &[],
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
        let topics3 = extract_topics("C++ programming", "Using C++ for systems programming", &[]);
        assert!(topics3.contains(&"c++".to_string()), "Should extract 'c++'");

        // Test no duplicates
        let topics4 = extract_topics("Rust Rust Rust", "rust rust rust everywhere", &[]);
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
        let topics = extract_topics("Machine Learning with Open Source tools", "", &[]);
        assert!(topics.contains(&"machine learning".to_string()));
        assert!(topics.contains(&"open source".to_string()));
    }

    #[test]
    fn test_extract_topics_no_stopwords() {
        let topics = extract_topics("The Best New Way For Your Project", "", &[]);
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
        let topics = extract_topics("docker kubernetes aws", "", &[]);
        assert!(topics.contains(&"docker".to_string()));
        assert!(topics.contains(&"kubernetes".to_string()));
        assert!(topics.contains(&"aws".to_string()));
    }

    #[test]
    fn test_extract_topics_capitalized_words_from_title() {
        let topics = extract_topics("Building Tauri Desktop Apps", "", &[]);
        assert!(topics.contains(&"tauri".to_string()));
    }

    /// Live 2026-09-08: the brief's "escalating" section listed "act signal
    /// chain (5 events)", "action …" and "actually …" — Title Case headline
    /// words minted as topics ("What Actually Becomes the Moat", "Privacy
    /// Act Reforms", "GitHub Actions"). In a Title Case headline
    /// capitalisation is typography; sentence openers are stopwords anyway.
    #[test]
    fn title_case_headlines_mint_no_bare_capitalised_topics() {
        for (title, words) in [
            (
                "Why Code Is About to Get Cheap — and What Actually Becomes the Moat",
                &["actually", "moat", "code", "cheap"][..],
            ),
            (
                "How AI Actually Changed My QA Workflow (Not the Sales Pitch Version)",
                &["actually", "workflow", "sales", "pitch"][..],
            ),
            (
                "Quick thoughts on GitHub Actions Aug 26 incident",
                &["actions"][..],
            ),
            (
                "Privacy Act Reforms: Government unveils tranche 2 proposals",
                &["act"][..],
            ),
            (
                "Actually, the borrow checker is your friend",
                &["actually"][..],
            ),
        ] {
            let topics = extract_topics(title, "", &[]);
            for word in words {
                assert!(
                    !topics.contains(&(*word).to_string()),
                    "{title:?} must not mint {word:?} (got {topics:?})"
                );
            }
        }
        // Vocabulary and marked names survive Title Case.
        let topics = extract_topics("Quick thoughts on GitHub Actions Aug 26 incident", "", &[]);
        assert!(topics.contains(&"github".to_string()));
        let topics = extract_topics("How ZenHub Made My Board Fast Again", "", &[]);
        assert!(
            topics.contains(&"zenhub".to_string()),
            "interior capital marks a name"
        );
        let topics = extract_topics("Why Deno2 Is Still The Default Runtime", "", &[]);
        assert!(
            topics.contains(&"deno2".to_string()),
            "a digit marks a name"
        );
    }

    /// Negative test for the gate: a sentence-case title still mints its
    /// capitalised names, first word included.
    #[test]
    fn sentence_case_titles_still_mint_names() {
        for (title, word) in [
            ("Toasty is an async ORM for Rust", "toasty"),
            ("Announcing Toasty, an async ORM", "toasty"),
            (
                "Rustls 0.23.44 released with ML-DSA certificates enabled by default",
                "rustls",
            ),
            ("Image crate patch released", "image"),
            (
                "Privacy Act Reforms: Government unveils tranche 2 proposals",
                "reforms",
            ),
        ] {
            let topics = extract_topics(title, "", &[]);
            assert!(
                topics.contains(&word.to_string()),
                "{title:?} must mint {word:?} (got {topics:?})"
            );
        }
        assert!(!is_title_case(&["Toasty", "async", "ORM", "for", "Rust"]));
        assert!(is_title_case(&[
            "What", "Actually", "Becomes", "Moat", "Cheap"
        ]));
        assert!(
            is_title_case(&["Getting", "Started", "with", "Rust", "Axum"]),
            "one lowercase function word is still a headline"
        );
        assert!(
            !is_title_case(&["Building", "Tauri", "Apps"]),
            "three words are not a headline"
        );
    }

    #[test]
    fn test_extract_topics_content_truncation() {
        // Content is truncated to first 500 chars
        let long_content = "x ".repeat(300); // 600 chars
        let topics = extract_topics("Rust", &long_content, &[]);
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

    // v29 (2026-09-04): the batch's subject is not a trend.
    #[test]
    fn test_trend_detection_excludes_the_batch_subject() {
        let mut items: Vec<(String, String)> = (0..40)
            .map(|i| (format!("Rust crate roundup number {i}"), String::new()))
            .collect();
        for _ in 0..3 {
            items.push(("Turbopack bundler rewrite lands".to_string(), String::new()));
        }
        let trends = detect_trend_topics(items.iter().map(|(t, c)| (t.as_str(), c.as_str())));
        assert!(
            !trends.iter().any(|t| t == "rust"),
            "a topic on 40 of 43 items is the batch subject, not a trend: {trends:?}"
        );
        assert!(
            trends.iter().any(|t| t == "turbopack"),
            "a three-item story is still a trend: {trends:?}"
        );
    }

    #[test]
    fn test_trend_share_cap_waits_for_a_real_batch() {
        let items: Vec<(String, String)> = (0..5)
            .map(|i| (format!("Rust crate roundup number {i}"), String::new()))
            .collect();
        let trends = detect_trend_topics(items.iter().map(|(t, c)| (t.as_str(), c.as_str())));
        assert!(
            trends.iter().any(|t| t == "rust"),
            "below the minimum batch size a share means nothing: {trends:?}"
        );
    }
}
