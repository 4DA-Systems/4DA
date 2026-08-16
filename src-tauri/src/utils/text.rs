// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

// ============================================================================
// String Utilities & Content Preprocessing
// ============================================================================

// UTF-8 safety gate (see the `clippy::string_slice` note in Cargo.toml).
// Byte-slicing a `str` panics on any index that is not a char boundary. This
// module was hardened against that class, so the lint is denied here to keep it
// at zero: every future slice must carry an explicit char-boundary proof
// (`floor_char_boundary`, an offset from `find` of an ASCII needle, or one of
// the `utils::text` helpers) or an `#[allow]` that states why it is safe.
#![deny(clippy::string_slice)]

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
// Word-boundary matching (UTF-8 safe)
// ============================================================================
//
// Eight near-identical `has_word_boundary` helpers existed across the tree
// (signals, scoring/dependencies, package_ambiguity, knowledge_decay,
// dep_linker, preemption, competing_tech, stacks/scoring) and exactly one —
// `scoring::utils` — was UTF-8 safe. #422 fixed 17 byte-slicing sites and
// reached none of these, because a fix cannot find seven copies it does not
// know about. They all now delegate here.
//
// The shared defect was the cursor advance:
//
// ```ignore
// let mut search_from = 0;
// while let Some(rel) = text[search_from..].find(term) {
//     let abs = search_from + rel;
//     ...
//     search_from = abs + 1;   // next iteration slices text[search_from..]
// }
// ```
//
// `abs` is the START of a match, so `abs + 1` is a char boundary only when the
// needle's first char is one byte. That advance is reached only when the
// word-boundary test FAILS — i.e. when the match abuts an alphanumeric char —
// so a non-ASCII search term glued to a letter panicked the caller. In
// `signals.rs` the term is the user's onboarding tech stack and the caller runs
// on every item of every scoring pass.

/// Byte offsets where `term` starts in `text`, as **non-overlapping** matches.
///
/// Always lands on a char boundary (it advances by `term.len()`, which is what
/// `str::match_indices` does). Use this instead of a hand-rolled
/// `search_from = pos + 1` cursor — see the module comment above for why that
/// form panics.
pub(crate) fn match_offsets<'a>(text: &'a str, term: &'a str) -> impl Iterator<Item = usize> + 'a {
    text.match_indices(term).map(|(i, _)| i)
}

/// The char immediately before byte offset `at`; `None` at the string start.
///
/// Total: an `at` that is out of range or not on a char boundary yields `None`
/// rather than panicking, so a caller that miscomputes an offset degrades to
/// "treat as a boundary" instead of unwinding.
pub(crate) fn char_before(text: &str, at: usize) -> Option<char> {
    text.get(..at).and_then(|s| s.chars().next_back())
}

/// The char that starts at byte offset `at`; `None` at or past the end.
/// Total in the same sense as [`char_before`].
pub(crate) fn char_at(text: &str, at: usize) -> Option<char> {
    text.get(at..).and_then(|s| s.chars().next())
}

/// Does `term` occur in `text` bounded on both sides by a char that is NOT a
/// word char per `is_word_char`?
///
/// The predicate is a parameter because the call sites genuinely disagree on
/// what a word char is, and collapsing them onto one policy would be a
/// behaviour change, not a refactor:
///
/// - most callers want plain alphanumeric ([`has_word_boundary_match`]);
/// - `dep_linker` / `scoring::dependencies` treat `-`, `_`, `.` and `@` as
///   package-name-internal, so "react" must not match inside "react-router";
/// - `stacks::scoring` treats `-` and `_` as word chars for the same reason.
///
/// An empty `term` is never a match (it would otherwise "occur" at every char
/// boundary and report a spurious hit at the start of an empty string).
pub(crate) fn has_bounded_match<F>(text: &str, term: &str, is_word_char: F) -> bool
where
    F: Fn(char) -> bool,
{
    if term.is_empty() {
        return false;
    }
    match_offsets(text, term).any(|pos| {
        char_before(text, pos).is_none_or(|c| !is_word_char(c))
            && char_at(text, pos + term.len()).is_none_or(|c| !is_word_char(c))
    })
}

/// Whole-word containment: `term` bounded by non-alphanumeric chars.
///
/// Case-sensitive — lowercase both sides first for case-insensitive matching.
///
/// Boundaries are tested on CHARS, not bytes. `as_bytes()[i - 1]
/// .is_ascii_alphanumeric()` is false for every UTF-8 continuation byte, so a
/// non-ASCII letter glued to the term ("иgo") read as a word boundary and "go"
/// matched (bug E).
pub(crate) fn has_word_boundary_match(text: &str, term: &str) -> bool {
    has_bounded_match(text, term, char::is_alphanumeric)
}

/// File-extension suffixes that read as a right word boundary rather than a
/// continuation: "next.js" corroborates the `next` package, "serde.rs" the
/// `serde` crate.
const NAME_EXT_SUFFIXES: [&str; 3] = [".js", ".ts", ".rs"];

/// [`has_word_boundary_match`], plus [`NAME_EXT_SUFFIXES`] accepted as a right
/// boundary. Used by the package-name matchers, where "next.js" is the same
/// package as "next".
pub(crate) fn has_word_boundary_match_with_ext(text: &str, term: &str) -> bool {
    if term.is_empty() {
        return false;
    }
    match_offsets(text, term).any(|pos| {
        let before_ok = char_before(text, pos).is_none_or(|c| !c.is_alphanumeric());
        let rest = text.get(pos + term.len()..).unwrap_or("");
        let after_ok = rest.chars().next().is_none_or(|c| !c.is_alphanumeric())
            || NAME_EXT_SUFFIXES.iter().any(|s| rest.starts_with(s));
        before_ok && after_ok
    })
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

    // ------------------------------------------------------------------
    // Word-boundary matching
    // ------------------------------------------------------------------

    /// THE panic condition, and it is narrower than "any non-ASCII term".
    ///
    /// The old advance was `search_from = abs + 1` where `abs` is the START of
    /// a match, so it lands on a boundary iff the term's FIRST char is one
    /// byte — "café" is safe, "éclair" is not. And the advance is reached only
    /// when the boundary test FAILS, i.e. when the match abuts an alphanumeric
    /// char. Both conditions must hold, which is why every ASCII test in the
    /// tree passed over this for as long as it existed.
    ///
    /// Pre-fix each of these aborts the process; the assertions are secondary
    /// to the calls returning at all.
    #[test]
    fn word_boundary_multibyte_term_abutting_alnum_does_not_panic() {
        // 'é' is 2 bytes, so `abs + 1` splits it.
        assert!(!has_word_boundary_match("éclair2 release", "éclair"));
        // Cyrillic (2), Chinese (3), emoji (4) — same shape, wider chars.
        assert!(!has_word_boundary_match("привет9", "привет"));
        assert!(!has_word_boundary_match("你好1 world", "你好"));
        assert!(!has_word_boundary_match("🦀x", "🦀"));
        // A genuine bounded occurrence after a failed one still matches.
        assert!(has_word_boundary_match("éclair2 and éclair here", "éclair"));
        // An ASCII-first-char term is unaffected either way — kept so the
        // narrowness of the trigger stays documented.
        assert!(!has_word_boundary_match("café2 release", "café"));
    }

    /// Same condition through the ext-suffix variant and the custom-predicate
    /// variant, which are separate code paths.
    #[test]
    fn word_boundary_variants_survive_multibyte_terms() {
        assert!(!has_word_boundary_match_with_ext("éclair2", "éclair"));
        assert!(has_word_boundary_match_with_ext(
            "éclair.js rocks",
            "éclair"
        ));
        let word = |c: char| c.is_alphanumeric() || c == '-';
        assert!(!has_bounded_match("éclair2", "éclair", word));
        assert!(!has_bounded_match("éclair-x", "éclair", word));
        assert!(has_bounded_match("éclair x", "éclair", word));
    }

    /// An empty term must never match. Without the guard the cursor walks the
    /// string one byte at a time and panics at the first ASCII-letter-followed-
    /// by-multibyte position — `signals.rs` had no such guard, and its term is
    /// the user's onboarding tech stack.
    #[test]
    fn word_boundary_empty_term_never_matches() {
        assert!(!has_word_boundary_match("anything", ""));
        assert!(!has_word_boundary_match("", ""));
        assert!(!has_word_boundary_match("aé", ""));
        assert!(!has_word_boundary_match_with_ext("aé", ""));
        assert!(!has_bounded_match("aé", "", char::is_alphanumeric));
    }

    #[test]
    fn word_boundary_ascii_semantics_preserved() {
        assert!(has_word_boundary_match("rust 1.80", "rust"));
        assert!(has_word_boundary_match("learn rust today", "rust"));
        assert!(!has_word_boundary_match("frustrating bug", "rust"));
        assert!(!has_word_boundary_match("entrust your data", "rust"));
        assert!(!has_word_boundary_match("argo", "go"));
        assert!(has_word_boundary_match("go", "go"));
        // Bug E: a non-ASCII letter glued to the term is NOT a boundary.
        assert!(!has_word_boundary_match("иgo", "go"));
        assert!(!has_word_boundary_match("goи", "go"));
    }

    #[test]
    fn word_boundary_ext_suffix_accepted() {
        assert!(has_word_boundary_match_with_ext("next.js is fine", "next"));
        assert!(has_word_boundary_match_with_ext(
            "use serde.rs here",
            "serde"
        ));
        assert!(has_word_boundary_match_with_ext("vue.ts types", "vue"));
        // Any other dotted continuation is still package-internal.
        assert!(!has_word_boundary_match_with_ext(
            "axios.get(url)",
            "axios.g"
        ));
        assert!(!has_word_boundary_match_with_ext("configuring app", "conf"));
    }

    #[test]
    fn char_accessors_are_total() {
        let s = "aé";
        assert_eq!(char_before(s, 0), None);
        assert_eq!(char_before(s, 1), Some('a'));
        assert_eq!(char_before(s, 2), None, "byte 2 splits 'é'");
        assert_eq!(char_at(s, 1), Some('é'));
        assert_eq!(char_at(s, 2), None, "byte 2 splits 'é'");
        assert_eq!(char_at(s, 99), None);
    }

    #[test]
    fn match_offsets_are_non_overlapping_and_on_boundaries() {
        let offsets: Vec<usize> = match_offsets("aéaéa", "aé").collect();
        assert_eq!(offsets, vec![0, 3]);
        for o in offsets {
            assert!("aéaéa".is_char_boundary(o));
        }
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
