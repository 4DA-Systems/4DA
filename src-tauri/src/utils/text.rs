// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

// ============================================================================
// String Utilities & Content Preprocessing
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
        format!("{clean_title}\n\n{clean_title}\n\n{clean_content}")
    }
}

/// Preprocess content for embedding: strip noise, normalize whitespace, cap length.
/// Goal: maximize signal density in the text sent to the embedding model.
pub(crate) fn preprocess_content(content: &str) -> String {
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
// Text Chunking
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_truncate_utf8_zero() {
        assert_eq!(truncate_utf8("hello", 0), "");
    }

    #[test]
    fn test_truncate_utf8_exact_length() {
        assert_eq!(truncate_utf8("hello", 5), "hello");
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
    fn test_build_embedding_text() {
        let result = build_embedding_text("My Title", "Some content here");
        assert!(result.contains("My Title"));
        assert!(result.contains("Some content here"));
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
