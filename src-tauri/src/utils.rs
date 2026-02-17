// Copyright (c) 2025-2026 Antony Lawrence Kiddie Pasifa. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use once_cell::sync::Lazy;
use std::collections::HashSet;

// ============================================================================
// String Utilities
// ============================================================================

/// Safely truncate a string to a maximum number of characters (UTF-8 aware)
/// This avoids panics when slicing multi-byte characters like Cyrillic, Chinese, etc.
pub(crate) fn truncate_utf8(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

/// Decode common HTML entities that sources may include in titles/content.
/// Applied to all text before embedding and display to prevent `&amp;` literals.
pub(crate) fn decode_html_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
}

pub(crate) fn build_embedding_text(title: &str, content: &str) -> String {
    let clean_title = decode_html_entities(title);
    let clean_content = decode_html_entities(content);
    if clean_content.is_empty() {
        clean_title
    } else {
        format!("{}\n\n{}", clean_title, clean_content)
    }
}

// ============================================================================
// Vector Math
// ============================================================================

/// Compute L2 norm of a vector
#[inline]
pub(crate) fn vector_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Cosine similarity with precomputed norm for vector `a`
/// Use this in hot loops where you compare the same vector `a` against many `b` vectors
#[inline]
pub(crate) fn cosine_similarity_with_norm(a: &[f32], a_norm: f32, b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_b: f32 = vector_norm(b);
    if a_norm == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (a_norm * norm_b)
}

/// Cosine similarity between two vectors (used by tests; hot path uses cosine_similarity_with_norm)
#[allow(dead_code)]
pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = vector_norm(a);
    let norm_b: f32 = vector_norm(b);

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

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
        "node",
        "deno",
        "bun",
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
        "kubernetes",
        "k8s",
        "docker",
        "container",
        "aws",
        "azure",
        "gcp",
        "cloud",
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
        if clean.len() > 2
            && clean
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
        {
            let lower = clean.to_lowercase();
            if !seen.contains(&lower)
                && ![
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
                ]
                .contains(&lower.as_str())
                && seen.insert(lower.clone())
            {
                topics.push(lower);
            }
        }
    }

    topics
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

// ============================================================================
// Text Processing
// ============================================================================

/// Maximum content length for embedding (roughly 1000 words)
pub(crate) const MAX_CONTENT_LENGTH: usize = 5000;

/// Maximum chunk size in characters (roughly 100-150 words)
const MAX_CHUNK_SIZE: usize = 500;

/// Split text into chunks for embedding
pub(crate) fn chunk_text(text: &str, source_file: &str) -> Vec<(String, String)> {
    let mut chunks = Vec::new();
    let paragraphs: Vec<&str> = text.split("\n\n").collect();

    let mut current_chunk = String::new();

    for para in paragraphs {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }

        if current_chunk.len() + para.len() > MAX_CHUNK_SIZE && !current_chunk.is_empty() {
            chunks.push((source_file.to_string(), current_chunk.clone()));
            current_chunk.clear();
        }

        if !current_chunk.is_empty() {
            current_chunk.push_str("\n\n");
        }
        current_chunk.push_str(para);
    }

    if !current_chunk.is_empty() {
        chunks.push((source_file.to_string(), current_chunk));
    }

    // If no chunks were created, use the whole text
    if chunks.is_empty() && !text.trim().is_empty() {
        chunks.push((source_file.to_string(), text.trim().to_string()));
    }

    chunks
}

// ============================================================================
// Content Scraping
// ============================================================================

/// Scrape article content from a URL
pub(crate) async fn scrape_article_content(url: &str) -> Option<String> {
    use scraper::{Html, Selector};

    // Skip non-HTTP URLs and known problematic domains
    if !url.starts_with("http") {
        return None;
    }

    // Use shared client with per-request timeout
    let client = crate::sources::shared_client();

    // Fetch the page
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let html = response.text().await.ok()?;
    let document = Html::parse_document(&html);

    // Try multiple content selectors in order of preference
    let selectors = [
        "article",
        "main",
        "[role='main']",
        ".post-content",
        ".article-content",
        ".entry-content",
        ".content",
        "body",
    ];

    for selector_str in selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(element) = document.select(&selector).next() {
                let text: String = element
                    .text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");

                // Only use if we got meaningful content (at least 100 chars)
                if text.len() > 100 {
                    // Truncate to max length
                    let truncated = if text.len() > MAX_CONTENT_LENGTH {
                        text.chars().take(MAX_CONTENT_LENGTH).collect()
                    } else {
                        text
                    };
                    return Some(truncated);
                }
            }
        }
    }

    None
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test cosine similarity helper
    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim - 1.0).abs() < 0.001,
            "Identical vectors should have similarity 1.0"
        );

        let c = vec![0.0, 1.0, 0.0];
        let sim_orth = cosine_similarity(&a, &c);
        assert!(
            sim_orth.abs() < 0.001,
            "Orthogonal vectors should have similarity 0.0"
        );
    }

    // ====================================================================
    // Utility Function Tests
    // ====================================================================

    #[test]
    fn test_truncate_utf8_ascii() {
        assert_eq!(truncate_utf8("hello world", 5), "hello");
        assert_eq!(truncate_utf8("hello", 10), "hello");
        assert_eq!(truncate_utf8("", 5), "");
    }

    #[test]
    fn test_truncate_utf8_multibyte() {
        // Cyrillic: each char is 2 bytes
        let cyrillic = "Привет мир";
        let result = truncate_utf8(cyrillic, 6);
        assert_eq!(result, "Привет");

        // Chinese: each char is 3 bytes
        let chinese = "你好世界";
        let result = truncate_utf8(chinese, 2);
        assert_eq!(result, "你好");
    }

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
    fn test_chunk_text_short() {
        let text = "Short text.";
        let chunks = chunk_text(text, "test.txt");
        assert_eq!(chunks.len(), 1);
        // Tuple is (source_file, content)
        assert_eq!(chunks[0].0, "test.txt");
        assert_eq!(chunks[0].1, "Short text.");
    }

    #[test]
    fn test_chunk_text_multi_paragraph() {
        // Create text with multiple paragraphs
        let mut paragraphs = Vec::new();
        for i in 0..20 {
            paragraphs.push(format!("Paragraph {} with some meaningful content about software development and engineering principles.", i));
        }
        let text = paragraphs.join("\n\n");
        let chunks = chunk_text(&text, "test.md");
        assert!(!chunks.is_empty());
        // Each chunk: (source_file, content)
        for (source, _content) in &chunks {
            assert_eq!(source, "test.md");
        }
    }

    #[test]
    fn test_build_embedding_text() {
        let result = build_embedding_text("My Title", "Some content here");
        assert!(result.contains("My Title"));
        assert!(result.contains("Some content here"));
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 0.001);
    }
}
