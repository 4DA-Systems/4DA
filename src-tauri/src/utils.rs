// Copyright (c) 2025-2026 4DA Systems. All rights reserved.
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
    let clean_content = preprocess_content(content);
    if clean_content.is_empty() {
        clean_title
    } else {
        // Title repeated for emphasis — embedding models weight earlier text more heavily
        format!("{}\n\n{}\n\n{}", clean_title, clean_title, clean_content)
    }
}

/// Preprocess content for embedding: strip noise, normalize whitespace, cap length.
/// Goal: maximize signal density in the text sent to the embedding model.
fn preprocess_content(content: &str) -> String {
    // Order matters: strip tags FIRST (raw HTML), THEN decode entities.
    // This prevents &lt;word&gt; from being decoded to <word> and then stripped as a tag.
    let text = strip_html_tags(content);

    let text = decode_html_entities(&text);

    // Strip URLs (raw URLs don't embed well)
    let text = strip_urls(&text);

    // Collapse whitespace: multiple spaces/newlines → single space
    let text = collapse_whitespace(&text);

    // Cap at 2000 chars to prevent embedding model truncation artifacts
    truncate_utf8(&text, 2000)
}

/// Remove HTML tags while preserving text content.
fn strip_html_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                result.push(' '); // Replace tag with space to prevent word merging
            }
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

/// Remove URLs (http/https) from text — they add noise to embeddings.
fn strip_urls(text: &str) -> String {
    // Simple but effective: find http(s):// and consume until whitespace
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == 'h' {
            // Check for http:// or https://
            let rest: String = std::iter::once(ch).chain(chars.clone().take(8)).collect();
            if rest.starts_with("https://") || rest.starts_with("http://") {
                // Skip until whitespace
                for c in chars.by_ref() {
                    if c.is_whitespace() {
                        result.push(' ');
                        break;
                    }
                }
                continue;
            }
        }
        result.push(ch);
    }
    result
}

/// Collapse runs of whitespace into single spaces, trim edges.
fn collapse_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_was_space = true; // Treat start as space to trim leading
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(ch);
            prev_was_space = false;
        }
    }
    // Trim trailing space
    if result.ends_with(' ') {
        result.pop();
    }
    result
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

    // ====================================================================
    // Additional Utils Tests
    // ====================================================================

    #[test]
    fn test_vector_norm_unit_vector() {
        let v = vec![1.0, 0.0, 0.0];
        assert!((vector_norm(&v) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_vector_norm_zero_vector() {
        let v = vec![0.0, 0.0, 0.0];
        assert!((vector_norm(&v) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_vector_norm_3_4_5() {
        let v = vec![3.0, 4.0];
        assert!((vector_norm(&v) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_with_norm_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let a_norm = vector_norm(&a);
        let sim = cosine_similarity_with_norm(&a, a_norm, &a);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_with_norm_zero_a() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity_with_norm(&a, 0.0, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_with_norm_zero_b() {
        let a = vec![1.0, 2.0, 3.0];
        let a_norm = vector_norm(&a);
        let b = vec![0.0, 0.0, 0.0];
        let sim = cosine_similarity_with_norm(&a, a_norm, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_with_norm_mismatched_length() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity_with_norm(&a, vector_norm(&a), &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_with_norm_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        let sim = cosine_similarity_with_norm(&a, 0.0, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_empty_vectors() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_mismatched_length() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_decode_html_entities_all() {
        assert_eq!(decode_html_entities("&amp;"), "&");
        assert_eq!(decode_html_entities("&lt;"), "<");
        assert_eq!(decode_html_entities("&gt;"), ">");
        assert_eq!(decode_html_entities("&quot;"), "\"");
        assert_eq!(decode_html_entities("&apos;"), "'");
        assert_eq!(decode_html_entities("&#39;"), "'");
        assert_eq!(decode_html_entities("&#x27;"), "'");
        assert_eq!(decode_html_entities("&nbsp;"), " ");
    }

    #[test]
    fn test_decode_html_entities_multiple() {
        assert_eq!(decode_html_entities("A &amp; B &lt; C"), "A & B < C");
    }

    #[test]
    fn test_decode_html_entities_no_entities() {
        assert_eq!(decode_html_entities("plain text"), "plain text");
    }

    #[test]
    fn test_build_embedding_text_with_content() {
        let result = build_embedding_text("Title", "Content");
        // Title repeated for emphasis, content preprocessed
        assert_eq!(result, "Title\n\nTitle\n\nContent");
    }

    #[test]
    fn test_build_embedding_text_empty_content() {
        let result = build_embedding_text("Title Only", "");
        assert_eq!(result, "Title Only");
    }

    #[test]
    fn test_build_embedding_text_html_entities() {
        let result = build_embedding_text("Rust &amp; Go", "Compare &lt;languages&gt;");
        // HTML entities decoded in title; content goes through preprocess_content
        // which decodes entities then strips HTML tags (< and > from decoded &lt;/&gt;
        // are treated as tag delimiters by strip_html_tags)
        assert!(result.starts_with("Rust & Go\n\nRust & Go\n\n"));
        assert!(result.contains("Compare"));
        assert!(result.contains("languages"));
    }

    #[test]
    fn test_preprocess_content_strips_html() {
        let result = preprocess_content("<p>Hello <b>world</b></p>");
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_preprocess_content_strips_urls() {
        let result = preprocess_content("Check out https://example.com for more info");
        assert_eq!(result, "Check out for more info");
    }

    #[test]
    fn test_preprocess_content_collapses_whitespace() {
        let result = preprocess_content("hello    world\n\n\nfoo   bar");
        assert_eq!(result, "hello world foo bar");
    }

    #[test]
    fn test_preprocess_content_truncates() {
        let long = "a".repeat(3000);
        let result = preprocess_content(&long);
        assert_eq!(result.len(), 2000);
    }

    #[test]
    fn test_strip_html_tags_nested() {
        let result = strip_html_tags("<div><p>nested</p></div>");
        // Tags replaced with spaces, then result has extra spaces
        assert!(result.contains("nested"));
        assert!(!result.contains('<'));
    }

    #[test]
    fn test_strip_urls_http() {
        let result = strip_urls("visit http://example.com today");
        assert_eq!(result, "visit  today");
    }

    #[test]
    fn test_strip_urls_no_url() {
        let result = strip_urls("no urls here");
        assert_eq!(result, "no urls here");
    }

    #[test]
    fn test_collapse_whitespace_edges() {
        let result = collapse_whitespace("  hello  ");
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_utf8_zero() {
        assert_eq!(truncate_utf8("hello", 0), "");
    }

    #[test]
    fn test_truncate_utf8_exact_length() {
        assert_eq!(truncate_utf8("hello", 5), "hello");
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

    #[test]
    fn test_chunk_text_empty() {
        let chunks = chunk_text("", "test.txt");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_text_whitespace_only() {
        let chunks = chunk_text("   \n\n   ", "test.txt");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_text_single_paragraph() {
        let text = "A single paragraph of moderate length.";
        let chunks = chunk_text(text, "src.rs");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].0, "src.rs");
        assert_eq!(chunks[0].1, text);
    }

    #[test]
    fn test_chunk_text_respects_paragraph_breaks() {
        let p1 = "A".repeat(400);
        let p2 = "B".repeat(400);
        let text = format!("{}\n\n{}", p1, p2);
        let chunks = chunk_text(&text, "test.txt");
        // Each paragraph is >400 chars, max chunk = 500, so they should split
        assert!(
            chunks.len() >= 2,
            "Long paragraphs should split into multiple chunks"
        );
    }
}
