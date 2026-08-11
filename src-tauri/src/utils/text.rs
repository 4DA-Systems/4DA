// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

// ============================================================================
// String Utilities & Content Preprocessing
// ============================================================================

/// Safely truncate a string to a maximum number of characters (UTF-8 aware)
/// This avoids panics when slicing multi-byte characters like Cyrillic, Chinese, etc.
///
/// This is the *machine* truncator — a hard character cap for embedding text,
/// LLM prompt budgets and log lines, where a mid-word cut costs nothing. Never
/// use it for a string a human will read: use [`truncate_display`] instead.
pub(crate) fn truncate_utf8(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

/// Characters that must never be the last thing before an ellipsis — cutting
/// after them reads as a broken sentence ("…the PRs with their,…").
const DANGLING: &[char] = &[
    ' ', '\t', '\n', ',', ';', ':', '.', '-', '–', '—', '(', '[', '{', '/', '&', '+', '·', '|',
    '\u{2026}',
];

/// Truncate a string for **display** to at most `max_chars` characters,
/// breaking on a word boundary and marking the cut with a single ellipsis.
///
/// The invariant every caller depends on: the returned string is never longer
/// than `max_chars` *including* the ellipsis, so schema caps (`EvidenceItem`
/// title ≤ 120, `relevance_note` ≤ 200) hold without the caller doing arithmetic.
///
/// Rules:
/// - Short enough already → returned untouched, with no ellipsis added. A title
///   the source already ellipsised ("…Attack A large-scale…") keeps its single
///   marker rather than gaining a second.
/// - Otherwise cut at the last whitespace inside the budget, strip any dangling
///   punctuation, and append `…` (U+2026, one char — not three ASCII dots).
/// - No whitespace in the budget → hard cut at the boundary. Scripts that do
///   not space their words (Chinese, Japanese, Thai) have no word boundary to
///   find, and a locale that renders 12 translated strings must not degenerate
///   to a lone ellipsis. The same fallback covers a single unbroken token.
///
/// Origin: 2026-08-12. `signals.rs` cut headlines at a flat 60 chars mid-word,
/// so the Key Signals card read "…Infects More Than 400 npm Pa" — 58% of the
/// live corpus was long enough to hit it.
pub(crate) fn truncate_display(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    // Reserve one char for the ellipsis so the result honours the caller's cap.
    let budget = max_chars - 1;
    let head: String = s.chars().take(budget).collect();

    // `rfind` returns a byte offset at a real char boundary (whitespace is
    // single-byte ASCII here), so the slice below cannot split a code point.
    let cut = match head.rfind(char::is_whitespace) {
        Some(i) if i > 0 => &head[..i],
        _ => head.as_str(),
    };
    let trimmed = cut.trim_end_matches(DANGLING);
    // An all-punctuation head would trim to nothing — keep the hard cut instead
    // of returning a bare ellipsis.
    let body = if trimmed.is_empty() { cut } else { trimmed };

    format!("{body}\u{2026}")
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
pub(crate) fn strip_html_tags(text: &str) -> String {
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

/// Minimum characters for a chunk to carry any distinctive signal. A one-line
/// shebang or a stray heading embeds into a near-generic vector that matches
/// everything. Mirrors the README indexer's inline 50-char floor.
const MIN_CHUNK_CHARS: usize = 50;

/// License/generated-file phrases that mark a chunk as boilerplate. Lowercase;
/// matched with `contains` against the lowercased chunk.
const BOILERPLATE_MARKERS: &[&str] = &[
    "spdx-license-identifier",
    "licensed under the",
    "permission is hereby granted",
    "all rights reserved",
    "www.apache.org/licenses",
    "warranties of merchantability",
    "@generated",
    "do not edit this file",
    "autogenerated file",
];

/// Is this chunk indexing-worthless boilerplate? Boilerplate chunks are the
/// live source of phantom "Similar to your code" evidence — a shebang chunk
/// from a script matched 6+ unrelated feed items as their TOP context evidence
/// (2026-07-13 audit). Rules, precision-first:
/// - shorter than [`MIN_CHUNK_CHARS`] → no distinctive signal;
/// - first line is a shebang and the chunk is a short preamble (< 200 chars);
/// - carries a license/generated-file marker within a short chunk (< 400
///   chars — a real document that merely QUOTES a license phrase runs longer).
pub(crate) fn is_boilerplate_chunk(chunk: &str) -> bool {
    let trimmed = chunk.trim();
    if trimmed.len() < MIN_CHUNK_CHARS {
        return true;
    }
    if trimmed.starts_with("#!") && trimmed.len() < 200 {
        return true;
    }
    if trimmed.len() < 400 {
        let lower = trimmed.to_lowercase();
        if BOILERPLATE_MARKERS.iter().any(|m| lower.contains(m)) {
            return true;
        }
    }
    false
}

/// Strip a leading shebang line — it is pure noise even when the rest of the
/// chunk is real content (the chunker packs the shebang paragraph together
/// with whatever follows it).
fn strip_shebang_line(chunk: &str) -> &str {
    let t = chunk.trim();
    if t.starts_with("#!") {
        t.split_once('\n')
            .map(|(_, rest)| rest.trim())
            .unwrap_or("")
    } else {
        t
    }
}

/// Split text into chunks for embedding. Boilerplate chunks (shebangs,
/// license headers, sub-minimum fragments) are dropped — they carry no user
/// context and their embeddings match everything.
pub(crate) fn chunk_text(text: &str, source_file: &str) -> Vec<(String, String)> {
    let mut chunks = Vec::new();
    let paragraphs: Vec<&str> = text.split("\n\n").collect();

    let mut current_chunk = String::new();

    let push_chunk = |chunks: &mut Vec<(String, String)>, chunk: String| {
        let cleaned = strip_shebang_line(&chunk);
        if !is_boilerplate_chunk(cleaned) {
            chunks.push((source_file.to_string(), cleaned.to_string()));
        }
    };

    for para in paragraphs {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }

        if current_chunk.len() + para.len() > MAX_CHUNK_SIZE && !current_chunk.is_empty() {
            push_chunk(&mut chunks, std::mem::take(&mut current_chunk));
        }

        if !current_chunk.is_empty() {
            current_chunk.push_str("\n\n");
        }
        current_chunk.push_str(para);
    }

    if !current_chunk.is_empty() {
        push_chunk(&mut chunks, current_chunk);
    }

    // If no chunks were created, use the whole text — unless the whole file is
    // itself boilerplate (a bare shebang script stub, a license stub).
    if chunks.is_empty() && !text.trim().is_empty() {
        let cleaned = strip_shebang_line(text);
        if !is_boilerplate_chunk(cleaned) {
            chunks.push((source_file.to_string(), cleaned.to_string()));
        }
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

    // ------------------------------------------------------------------
    // truncate_display — the human-facing truncator
    // ------------------------------------------------------------------

    #[test]
    fn truncate_display_leaves_short_strings_alone() {
        assert_eq!(truncate_display("hello world", 20), "hello world");
        assert_eq!(truncate_display("hello", 5), "hello");
        assert_eq!(truncate_display("", 10), "");
    }

    #[test]
    fn truncate_display_never_cuts_mid_word() {
        // The live regression: a flat 60-char cut produced "…400 npm Pa".
        let title = "Self-Propagating ChainDrop Worm Infects More Than 400 npm Packages in Major Software Supply Chain Attack";
        let out = truncate_display(title, 60);
        assert_eq!(
            out,
            "Self-Propagating ChainDrop Worm Infects More Than 400 npm…"
        );
        assert!(!out.contains("Pa\u{2026}"), "cut mid-word: {out}");
    }

    #[test]
    fn truncate_display_honours_the_cap_including_the_ellipsis() {
        // EvidenceItem title ≤ 120 / relevance_note ≤ 200 are schema-enforced;
        // the ellipsis must fit *inside* the budget, never push past it.
        for max in [1, 2, 8, 30, 60, 119, 120, 200] {
            let long = "alpha beta gamma delta epsilon zeta eta theta iota kappa ".repeat(12);
            let out = truncate_display(&long, max);
            assert!(
                out.chars().count() <= max,
                "max={max} produced {} chars: {out:?}",
                out.chars().count()
            );
        }
    }

    #[test]
    fn truncate_display_strips_dangling_punctuation() {
        // "…with their," / "…numbers:" read as broken sentences.
        assert_eq!(
            truncate_display("alpha beta, gamma", 13),
            "alpha beta\u{2026}"
        );
        assert_eq!(
            truncate_display("alpha beta: gamma", 13),
            "alpha beta\u{2026}"
        );
        assert_eq!(
            truncate_display("alpha beta - gamma", 14),
            "alpha beta\u{2026}"
        );
    }

    #[test]
    fn truncate_display_does_not_double_ellipsis() {
        // 3,401 live rows already end with an ellipsis from upstream truncation.
        let already = "Supply Chain Attack A large-scale\u{2026}";
        assert_eq!(truncate_display(already, 120), already);
        assert_eq!(already.matches('\u{2026}').count(), 1);
        // And when we *do* cut such a string, the old marker goes with the tail.
        let out = truncate_display(already, 20);
        assert_eq!(out.matches('\u{2026}').count(), 1, "{out}");
    }

    #[test]
    fn truncate_display_handles_unspaced_scripts() {
        // Chinese has no word boundary to find — hard cut beats a bare ellipsis.
        let chinese = "你好世界你好世界你好世界";
        let out = truncate_display(chinese, 5);
        assert_eq!(out, "你好世界\u{2026}");
        assert_eq!(out.chars().count(), 5);
    }

    #[test]
    fn truncate_display_handles_one_giant_token() {
        let token = "a".repeat(200);
        let out = truncate_display(&token, 10);
        assert_eq!(out, format!("{}\u{2026}", "a".repeat(9)));
    }

    #[test]
    fn truncate_display_is_multibyte_safe() {
        // Cyrillic (2 bytes/char) and emoji (4 bytes) must not panic or split.
        let out = truncate_display("Привет мир как дела сегодня", 12);
        assert!(out.ends_with('\u{2026}'), "{out}");
        assert!(out.chars().count() <= 12);
        let emoji = truncate_display("🦀 Rust 🦀 systems 🦀 programming", 12);
        assert!(emoji.chars().count() <= 12, "{emoji}");
    }

    #[test]
    fn truncate_display_never_returns_a_bare_ellipsis() {
        // An all-punctuation head would trim to nothing.
        let out = truncate_display("--------- tail", 6);
        assert_ne!(out, "\u{2026}");
        assert!(out.chars().count() <= 6);
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
        // Sub-minimum fragments carry no distinctive signal — filtered
        // (2026-07-14 boilerplate gate). A short-but-real sentence above the
        // floor still chunks.
        let chunks = chunk_text("Short text.", "test.txt");
        assert!(chunks.is_empty(), "sub-50-char text is indexing noise");

        let text = "A short but meaningful sentence about the project architecture.";
        let chunks = chunk_text(text, "test.txt");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].0, "test.txt");
        assert_eq!(chunks[0].1, text);
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
        let text = "A single paragraph of moderate length that clears the minimum chunk floor.";
        let chunks = chunk_text(text, "src.rs");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].0, "src.rs");
        assert_eq!(chunks[0].1, text);
    }

    #[test]
    fn test_chunk_text_drops_boilerplate() {
        // The live phantom class: a shebang preamble chunk matched 6+ feed
        // items as their TOP "Similar to your code" evidence.
        assert!(is_boilerplate_chunk("#!/usr/bin/env node"));
        assert!(is_boilerplate_chunk(
            "#!/usr/bin/env bash
# helper script for local dev"
        ));
        assert!(is_boilerplate_chunk(
            "// SPDX-License-Identifier: FSL-1.1-Apache-2.0 licensed under the terms below"
        ));
        assert!(is_boilerplate_chunk(
            "Permission is hereby granted, free of charge, to any person obtaining a copy of this software."
        ));
        // Real prose of similar length is NOT boilerplate.
        assert!(!is_boilerplate_chunk(
            "The scoring pipeline grounds registry items by their subject package and version-checks advisories."
        ));

        // A script whose only paragraph is its shebang preamble yields nothing.
        let chunks = chunk_text("#!/usr/bin/env node", "run.js");
        assert!(chunks.is_empty(), "bare shebang stub must not be indexed");

        // A script with a real body keeps the body, drops the shebang paragraph.
        let text = "#!/usr/bin/env node

This module reconciles the vector index against the chunk table and repairs drift between them.";
        let chunks = chunk_text(text, "run.js");
        assert_eq!(chunks.len(), 1);
        assert!(!chunks[0].1.contains("#!/usr/bin/env"));
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
