// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Shared package-name ambiguity guards.
//!
//! User dependency names like "os", "http", or "config" are common English
//! words (or fragments of them), so raw substring matching against article
//! titles/content mints false dependency matches -- and downstream, false
//! `preemption_wins` rows. These guards are the single source of truth for
//! "does this text actually talk about this package?" and are used by
//! blind_spots, decision_advantage window detection, and win validation.

/// Check whether `text` contains `term` at a word boundary.
/// Case-sensitive; pass already-lowercased strings for case-insensitive matching.
pub(crate) fn has_word_boundary_match(text: &str, term: &str) -> bool {
    if term.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let mut search_from = 0;
    while let Some(pos) = text[search_from..].find(term) {
        let abs = search_from + pos;
        let before_ok = abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric();
        let after = abs + term.len();
        let after_ok = after >= bytes.len()
            || !bytes[after].is_ascii_alphanumeric()
            || text[after..].starts_with(".js")
            || text[after..].starts_with(".ts")
            || text[after..].starts_with(".rs");
        if before_ok && after_ok {
            return true;
        }
        search_from = abs + 1;
    }
    false
}

/// Package names that are common English words AND real package names.
/// Unlike `is_generic_dep_name` (which blocks them from queries entirely),
/// ambiguous names ARE queried but require ecosystem-qualified proof
/// (exact_registry or advisory match) to surface.
pub(crate) fn is_ambiguous_package_name(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "image"
            | "config"
            | "log"
            | "time"
            | "rand"
            | "error"
            | "hash"
            | "ring"
            | "url"
            | "http"
            | "crypto"
            | "lazy"
            | "quote"
            | "lock"
            | "once"
            | "pin"
            | "signal"
            | "sync"
            | "bytes"
            | "regex"
            | "either"
            | "paste"
            | "clap"
            | "nom"
            | "base"
            | "core"
            | "test"
            | "data"
            | "utils"
            | "proc_macro2"
            | "proc-macro2"
    )
}

/// True when a package name is too word-like to trust a bare text match:
/// either it is on the ambiguous list, or it is so short ("os", "fs", "js",
/// npm shims) that it collides with fragments of ordinary prose.
pub(crate) fn requires_strict_proof(name: &str) -> bool {
    is_ambiguous_package_name(name) || name.len() < 4
}

/// Ecosystem context words that indicate the text is actually discussing a
/// software package rather than using the dep name as an English word.
/// Mirrors the CONTEXT_WORDS idea in scoring/pipeline_v2.rs.
const ECOSYSTEM_CONTEXT_WORDS: &[&str] = &[
    "npm",
    "cargo",
    "crate",
    "crates",
    "pip",
    "pypi",
    "gem",
    "rubygems",
    "maven",
    "nuget",
    "composer",
    "package",
    "library",
    "dependency",
    "module",
    "sbom",
    "lockfile",
];

/// Whether the (lowercased) text contains at least one ecosystem context word.
/// Short terms use word-boundary matching ("gem" must not fire on "judgement");
/// longer terms match as substrings so plurals like "packages" still count.
fn has_ecosystem_context(text: &str) -> bool {
    ECOSYSTEM_CONTEXT_WORDS.iter().any(|w| {
        if w.len() <= 4 {
            has_word_boundary_match(text, w)
        } else {
            text.contains(w)
        }
    })
}

/// Decide whether an item (lowercased title + content) is genuinely about the
/// given (lowercased) dependency.
///
/// Policy:
/// - Normal names: word-boundary match in title OR content.
/// - Strict-proof names (ambiguous or <4 chars): word-boundary match in the
///   TITLE (content alone never qualifies) AND at least one ecosystem context
///   word anywhere in title+content.
pub(crate) fn dep_grounded_match(title_lower: &str, content_lower: &str, dep_lower: &str) -> bool {
    if dep_lower.is_empty() {
        return false;
    }
    if !requires_strict_proof(dep_lower) {
        return has_word_boundary_match(title_lower, dep_lower)
            || has_word_boundary_match(content_lower, dep_lower);
    }
    if !has_word_boundary_match(title_lower, dep_lower) {
        return false;
    }
    has_ecosystem_context(title_lower) || has_ecosystem_context(content_lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- has_word_boundary_match --

    #[test]
    fn word_boundary_basics() {
        assert!(has_word_boundary_match("react is great", "react"));
        assert!(has_word_boundary_match("next.js is fine", "next")); // .js suffix allowed
        assert!(has_word_boundary_match("use serde.rs for json", "serde")); // .rs suffix allowed
        assert!(!has_word_boundary_match("unexpected happens here", "next")); // embedded in word
        assert!(!has_word_boundary_match("configuring app", "conf")); // substring of config
        assert!(!has_word_boundary_match("anything", ""));
    }

    // -- requires_strict_proof --

    #[test]
    fn strict_proof_short_and_ambiguous_names() {
        // Short npm shims collide with prose fragments.
        assert!(requires_strict_proof("os"));
        assert!(requires_strict_proof("fs"));
        assert!(requires_strict_proof("js"));
        // Ambiguous-list members of any length.
        assert!(requires_strict_proof("http"));
        assert!(requires_strict_proof("config"));
        // Distinctive names need no extra proof.
        assert!(!requires_strict_proof("axios"));
        assert!(!requires_strict_proof("lodash"));
    }

    // -- dep_grounded_match: real-world regression cases --

    #[test]
    fn regression_os_does_not_match_bugsink_advisory() {
        // Real false win: dep "os" matched as a substring of "close"/"macos"
        // in an unrelated Bugsink CVE. Must NOT match.
        let title = "bugsink: issue event views can show an event from another project";
        let content = "the issue is close to resolution and affects macos users";
        assert!(!dep_grounded_match(title, content, "os"));
    }

    #[test]
    fn regression_http_does_not_match_tinymce_advisory() {
        // Real false win: dep "http" matched "https://..." URLs in an
        // unrelated TinyMCE XSS advisory. Must NOT match.
        let title = "tinymce cross-site scripting (xss) vulnerability using sanitization";
        let content = "see https://example.com for details";
        assert!(!dep_grounded_match(title, content, "http"));
    }

    #[test]
    fn strict_name_matches_with_title_hit_and_context() {
        let title = "npm package http 1.0 security advisory";
        let content = "the http package on npm";
        assert!(dep_grounded_match(title, content, "http"));
    }

    #[test]
    fn strict_name_os_matches_with_ecosystem_context() {
        let title = "os package vulnerability in npm registry";
        assert!(dep_grounded_match(title, "", "os"));
    }

    #[test]
    fn strict_name_content_only_never_qualifies() {
        // Even with context words, a strict-proof name needs a TITLE hit.
        let title = "weekly security roundup";
        let content = "the http package on npm has a vulnerability";
        assert!(!dep_grounded_match(title, content, "http"));
    }

    #[test]
    fn normal_name_matches_in_title() {
        assert!(dep_grounded_match(
            "axios 1.12 security advisory",
            "",
            "axios"
        ));
    }

    #[test]
    fn normal_name_matches_in_content_alone() {
        assert!(dep_grounded_match(
            "security alert",
            "axios has a cve",
            "axios"
        ));
    }

    #[test]
    fn normal_name_no_match_without_word_boundary() {
        assert!(!dep_grounded_match(
            "unrelated title",
            "unrelated content",
            "axios"
        ));
    }

    #[test]
    fn empty_dep_never_matches() {
        assert!(!dep_grounded_match("anything", "anything", ""));
    }

    // -- has_ecosystem_context --

    #[test]
    fn ecosystem_context_short_terms_need_word_boundary() {
        assert!(has_ecosystem_context("install via npm today"));
        assert!(!has_ecosystem_context("final judgement rendered")); // "gem" embedded
        assert!(has_ecosystem_context("all packages updated")); // substring of plural
        assert!(!has_ecosystem_context("nothing relevant here"));
    }
}
