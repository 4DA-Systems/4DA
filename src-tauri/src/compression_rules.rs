// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.

//! Per-Source Compression Rules — reduce LLM/embedding token usage by applying
//! source-specific noise removal before content enters the intelligence pipeline.
//!
//! Applied AFTER `preprocess_content()` (HTML stripping, entity decoding, URL removal)
//! but BEFORE embedding generation or LLM analysis calls.

use std::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// Global Metrics
// ============================================================================

static TOTAL_CHARS_SAVED: AtomicU64 = AtomicU64::new(0);

pub fn chars_saved() -> u64 {
    TOTAL_CHARS_SAVED.load(Ordering::Relaxed)
}

pub fn reset_chars_saved() {
    TOTAL_CHARS_SAVED.store(0, Ordering::Relaxed);
}

// ============================================================================
// Rule Definitions
// ============================================================================

struct CompressionRule {
    max_chars: usize,
    extract_lead: bool,
    lead_paragraphs: usize,
    strip_quoted: bool,
    strip_markdown_formatting: bool,
    keep_section: Option<&'static str>,
}

fn rule_for(source_type: &str) -> CompressionRule {
    match source_type {
        "hn" => CompressionRule {
            max_chars: 1500,
            extract_lead: false,
            lead_paragraphs: 0,
            strip_quoted: true,
            strip_markdown_formatting: false,
            keep_section: None,
        },
        "reddit" => CompressionRule {
            max_chars: 1500,
            extract_lead: false,
            lead_paragraphs: 0,
            strip_quoted: true,
            strip_markdown_formatting: true,
            keep_section: None,
        },
        "rss" | "blog" | "devto" | "lobsters" => CompressionRule {
            max_chars: 2000,
            extract_lead: true,
            lead_paragraphs: 5,
            strip_quoted: false,
            strip_markdown_formatting: false,
            keep_section: None,
        },
        "github" | "github_releases" => CompressionRule {
            max_chars: 1000,
            extract_lead: false,
            lead_paragraphs: 0,
            strip_quoted: false,
            strip_markdown_formatting: true,
            keep_section: Some("changelog"),
        },
        "arxiv" | "papers_with_code" => CompressionRule {
            max_chars: 1500,
            extract_lead: true,
            lead_paragraphs: 3,
            strip_quoted: false,
            strip_markdown_formatting: false,
            keep_section: None,
        },
        "cve" | "osv" | "advisory" => CompressionRule {
            max_chars: 800,
            extract_lead: true,
            lead_paragraphs: 2,
            strip_quoted: false,
            strip_markdown_formatting: false,
            keep_section: None,
        },
        "npm" | "crates" | "pypi" | "go_modules" => CompressionRule {
            max_chars: 600,
            extract_lead: true,
            lead_paragraphs: 2,
            strip_quoted: false,
            strip_markdown_formatting: true,
            keep_section: None,
        },
        _ => CompressionRule {
            max_chars: 2000,
            extract_lead: false,
            lead_paragraphs: 0,
            strip_quoted: false,
            strip_markdown_formatting: false,
            keep_section: None,
        },
    }
}

// ============================================================================
// Compression
// ============================================================================

pub fn compress(source_type: &str, content: &str) -> String {
    if content.is_empty() {
        return String::new();
    }

    let rule = rule_for(source_type);
    let original_len = content.len();
    let mut text = content.to_string();

    if rule.strip_quoted {
        text = strip_quoted_text(&text);
    }

    if rule.strip_markdown_formatting {
        text = strip_markdown(&text);
    }

    if let Some(section) = rule.keep_section {
        if let Some(extracted) = extract_section(&text, section) {
            text = extracted;
        }
    }

    if rule.extract_lead && rule.lead_paragraphs > 0 {
        text = extract_lead_paragraphs(&text, rule.lead_paragraphs);
    }

    // Dedup consecutive identical paragraphs
    text = dedup_paragraphs(&text);

    // Final cap
    text = crate::truncate_utf8(&text, rule.max_chars);

    let saved = original_len.saturating_sub(text.len());
    if saved > 0 {
        TOTAL_CHARS_SAVED.fetch_add(saved as u64, Ordering::Relaxed);
    }

    text
}

// ============================================================================
// Compression Strategies
// ============================================================================

fn strip_quoted_text(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with('>')
                && !trimmed.starts_with("&gt;")
                && !trimmed
                    .starts_with("On ")
                    .then_some(())
                    .and_then(|_| {
                        if trimmed.contains(" wrote:") {
                            Some(())
                        } else {
                            None
                        }
                    })
                    .is_none()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_markdown(text: &str) -> String {
    let mut result = text.to_string();
    // Strip heading markers
    result = result
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                trimmed.trim_start_matches('#').trim_start()
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Strip bold/italic markers
    result = result.replace("**", "").replace("__", "");
    result = result.replace("*", "").replace("_", "");

    // Strip link syntax [text](url) → text
    let mut clean = String::with_capacity(result.len());
    let chars: Vec<char> = result.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            if let Some(close_bracket) = chars[i..].iter().position(|&c| c == ']') {
                let link_text: String = chars[i + 1..i + close_bracket].iter().collect();
                let after = i + close_bracket + 1;
                if after < chars.len() && chars[after] == '(' {
                    if let Some(close_paren) = chars[after..].iter().position(|&c| c == ')') {
                        clean.push_str(&link_text);
                        i = after + close_paren + 1;
                        continue;
                    }
                }
            }
        }
        clean.push(chars[i]);
        i += 1;
    }

    // Strip code block fences
    clean
        .lines()
        .filter(|line| !line.trim_start().starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_section(text: &str, section_name: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let needle = section_name.to_lowercase();

    // Find a heading or section marker containing the keyword
    let lines: Vec<&str> = text.lines().collect();
    let mut start = None;
    let mut end = None;

    for (i, line) in lines.iter().enumerate() {
        let line_lower = line.to_lowercase();
        if start.is_none()
            && (line_lower.contains(&needle)
                && (line_lower.starts_with('#') || line_lower.starts_with("##")))
        {
            start = Some(i + 1);
        } else if start.is_some() && line.starts_with('#') {
            end = Some(i);
            break;
        }
    }

    if let Some(s) = start {
        let e = end.unwrap_or(lines.len());
        let extracted: String = lines[s..e].join("\n");
        if !extracted.trim().is_empty() {
            return Some(extracted);
        }
    }

    // Fallback: check for the keyword anywhere in the text
    if lower.contains(&needle) {
        return None; // Content exists but not in a clean section
    }
    None
}

fn extract_lead_paragraphs(text: &str, count: usize) -> String {
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    paragraphs
        .into_iter()
        .take(count)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn dedup_paragraphs(text: &str) -> String {
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();

    for para in paragraphs {
        let normalized = para.trim().to_lowercase();
        if normalized.len() < 20 || seen.insert(normalized) {
            deduped.push(para);
        }
    }

    deduped.join("\n\n")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_hn_strips_quotes() {
        let content = "This is the main point.\n> Quoted reply text\n> More quoted text\nAnother real comment.";
        let result = compress("hn", content);
        assert!(result.contains("main point"));
        assert!(
            !result.contains("Quoted reply"),
            "Quotes should be stripped"
        );
        assert!(result.contains("Another real comment"));
    }

    #[test]
    fn test_compress_rss_extract_lead() {
        let paragraphs: Vec<String> = (0..10)
            .map(|i| format!("Paragraph {i} with some content about the topic."))
            .collect();
        let content = paragraphs.join("\n\n");
        let result = compress("rss", &content);
        assert!(result.contains("Paragraph 0"));
        assert!(result.contains("Paragraph 4"));
        assert!(
            !result.contains("Paragraph 5"),
            "Only 5 lead paragraphs for RSS"
        );
    }

    #[test]
    fn test_compress_npm_short_cap() {
        let content = "x".repeat(1000);
        let result = compress("npm", &content);
        assert!(result.len() <= 600);
    }

    #[test]
    fn test_compress_unknown_source_passthrough() {
        let content = "Short content.";
        let result = compress("unknown_source", content);
        assert_eq!(result, content);
    }

    #[test]
    fn test_compress_preserves_cve_ids() {
        let content = "CVE-2024-1234 affects versions 1.0-2.0 of libfoo. Upgrade to 2.1.";
        let result = compress("cve", content);
        assert!(result.contains("CVE-2024-1234"));
        assert!(result.contains("2.1"));
    }

    #[test]
    fn test_compress_github_strips_markdown() {
        let content = "## Changelog\n\n**Breaking:** removed deprecated API\n\n## Contributors\n\nThanks everyone";
        let result = compress("github", &content);
        assert!(result.contains("removed deprecated API"));
        // Markdown formatting stripped
        assert!(!result.contains("**"));
    }

    #[test]
    fn test_dedup_paragraphs() {
        let content = "First paragraph.\n\nDuplicate paragraph content here.\n\nDuplicate paragraph content here.\n\nLast paragraph.";
        let result = dedup_paragraphs(content);
        let count = result.matches("Duplicate paragraph").count();
        assert_eq!(count, 1, "Duplicate paragraphs should be removed");
    }

    #[test]
    fn test_strip_markdown_links() {
        let text = "Check out [Rust](https://rust-lang.org) and [Tauri](https://tauri.app)";
        let result = strip_markdown(text);
        assert!(result.contains("Rust"));
        assert!(result.contains("Tauri"));
        assert!(!result.contains("https://"));
    }

    #[test]
    fn test_chars_saved_metric() {
        reset_chars_saved();
        let long_content = "x".repeat(3000);
        compress("npm", &long_content);
        assert!(chars_saved() > 0);
    }

    #[test]
    fn test_empty_content() {
        let result = compress("hn", "");
        assert!(result.is_empty());
    }
}
