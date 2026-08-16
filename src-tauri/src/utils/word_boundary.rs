// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Whole-word matching over `str`, without byte arithmetic.
//!
//! Split out of `utils/text.rs`: that module is about preprocessing content for
//! embedding (strip, decode, collapse, chunk), this one is about locating a
//! token inside text. They were only ever neighbours because both handle
//! strings, and keeping them together pushed the file past the size limit.
//!
//! This is the single home of a helper that had been written EIGHT times across
//! the tree, seven of them with a byte-arithmetic cursor that panicked on
//! multi-byte input. Every copy now delegates here. See the module comment
//! below for the exact defect.

// UTF-8 safety gate (see the `clippy::string_slice` note in Cargo.toml).
// Byte-slicing a `str` panics on any index that is not a char boundary. This
// module exists to make that impossible for its callers, so the lint is denied
// here: it must stay at zero.
#![deny(clippy::string_slice)]

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
