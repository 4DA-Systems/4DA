// SPDX-License-Identifier: FSL-1.1-Apache-2.0

//! Query-text handling for hybrid search: FTS5 query sanitizing, exact-title lane
//! queries, and the word-boundary title check that decides what "exact" means.
//!
//! Every user-supplied token reaches FTS5 inside a double-quoted string, so FTS5
//! operators (`OR`, `NOT`, `NEAR`, `:`, `^`, parentheses) are inert and package names
//! such as `@tauri-apps/api` or `react-dom` cannot raise a syntax error. A quoted
//! string is tokenized by the table's tokenizer (`porter unicode61`), so
//! `"@tauri-apps/api"` becomes the phrase `tauri apps api`.

use crate::natural_language_search::STOP_WORDS;

/// Characters that continue a word for the exact-title boundary check. Hyphen and
/// underscore count, so `rusqlite` does not exactly match `rusqlite-migration`.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_'
}

/// Escape a token for use inside an FTS5 double-quoted string.
fn fts5_quote(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// Sanitize a query string for the BM25 leg: every token becomes a quoted,
/// prefix-matched phrase. Tokens with no letters or digits are dropped (an empty
/// phrase is meaningless to FTS5).
pub(crate) fn sanitize_fts5_query(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|t| t.chars().any(char::is_alphanumeric))
        .map(|t| format!("{}*", fts5_quote(t)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Normalize one query token for the exact lane: lowercase, strip quotes, trim
/// surrounding punctuation (a leading `@` scope marker is kept). `None` when nothing
/// alphanumeric remains.
fn normalize_term(raw: &str) -> Option<String> {
    let lower = raw.to_lowercase().replace('"', "");
    let trimmed = lower
        .trim_end_matches(|c: char| !c.is_alphanumeric())
        .trim_start_matches(|c: char| !c.is_alphanumeric() && c != '@');
    if trimmed.chars().any(char::is_alphanumeric) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// The query terms the exact lane matches titles against: normalized tokens with
/// stop words removed, in query order.
pub(crate) fn exact_title_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .filter_map(normalize_term)
        .filter(|t| !STOP_WORDS.contains(&t.as_str()))
        .collect()
}

/// A token shaped like a package or crate name (`react-dom`, `serde_json`,
/// `socket.io`, `@scope/name`). In a multi-word query these are also tried alone,
/// so "rusqlite-migration changelog" can still pin the crate.
fn is_package_like(term: &str) -> bool {
    term.contains(['-', '_', '.', '@', '/'])
}

/// The exact-lane FTS queries to run, in priority order, each paired with the
/// terms its hits must contain at word boundaries: first the whole query as one
/// title phrase, then (for multi-word queries) each package-like term alone.
pub(crate) fn exact_title_queries(query: &str) -> Vec<(String, Vec<String>)> {
    let terms = exact_title_terms(query);
    if terms.is_empty() {
        return Vec::new();
    }
    let mut queries = vec![(
        format!("title : {}", fts5_quote(&terms.join(" "))),
        terms.clone(),
    )];
    if terms.len() > 1 {
        for term in terms.iter().filter(|t| is_package_like(t)) {
            queries.push((format!("title : {}", fts5_quote(term)), vec![term.clone()]));
        }
    }
    queries
}

/// Whether `title` contains `terms` (joined by single spaces) as a whole word or
/// phrase — bounded on both sides by a non-word character or the string edge.
/// Case-insensitive; runs of whitespace in the title compare as one space.
pub(crate) fn title_has_exact_phrase(title: &str, terms: &[String]) -> bool {
    let needle = terms.join(" ");
    if needle.is_empty() {
        return false;
    }
    let hay = title
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    hay.match_indices(needle.as_str()).any(|(start, matched)| {
        let before_ok = hay[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word_char(c));
        let after_ok = hay[start + matched.len()..]
            .chars()
            .next()
            .is_none_or(|c| !is_word_char(c));
        before_ok && after_ok
    })
}
