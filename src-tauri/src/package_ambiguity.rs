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
            // Live audit 2026-07-19: title-only matches on these three were
            // 6/6 false in the 7-day window ("Defense Express" news outlet,
            // "Tower Bridge"/"Tower of Hanoi", "Tracing the voyage of a
            // plastic bottle cap") — they painted the gold "touches your
            // stack" ring on war-news items. Registry/advisory-proofed
            // matches still surface them.
            | "tower"
            | "express"
            | "tracing"
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
            // Live signal-chain audit 2026-07-28: these exact dependency
            // names are real packages in local manifests, but bare topic
            // matches produced personal critical chains from generic prose
            // ("arbitrary code execution", router hardware/networking,
            // testing discourse, async articles, clone/read verbs). Later live
            // signal-chain probes also caught "next" as ordinary English, motion
            // sensors, and profiling articles binding to transitive packages. They
            // remain valid when package-adjacent ecosystem proof exists.
            | "arbitrary"
            | "router"
            | "testing"
            | "async"
            | "clone"
            | "read"
            | "windows"
            | "next"
            | "motion"
            | "profiling"
    )
}

/// Dep names that are so generic they cause false matches in SQL LIKE queries.
/// Only truly generic English words that appear in nearly every article title.
/// Words like "futures", "bytes", "ring", "cookie", "config", "router" are real
/// crate/package names -- the word boundary matching already prevents false positives.
pub(crate) fn is_generic_dep_name(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "open"
            | "test"
            | "core"
            | "path"
            | "sync"
            | "once"
            | "glob"
            | "rand"
            | "time"
            | "lock"
            | "send"
            | "copy"
            | "find"
            | "diff"
            | "pick"
            | "wrap"
            | "trim"
            | "data"
            | "form"
            | "icon"
            | "link"
            | "text"
            | "type"
            | "util"
            | "base"
            | "flat"
            | "safe"
            | "fast"
            | "make"
            | "pipe"
            | "pump"
            | "read"
            | "call"
            | "nano"
            | "pure"
            | "vary"
            | "deep"
            | "try"
            | "want"
            | "mime"
            | "race"
            | "http"
            | "https"
    )
}

/// True when a package name is too word-like to trust a bare text match:
/// on the ambiguous list, on the generic-English-word list ("path", "open",
/// "data" appear in ordinary prose at word boundaries), or so short
/// ("os", "fs", "js" npm shims) that it collides with prose fragments.
pub(crate) fn requires_strict_proof(name: &str) -> bool {
    is_ambiguous_package_name(name) || is_generic_dep_name(name) || name.len() < 4
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
    "packages",
    "library",
    "libraries",
    "dependency",
    "dependencies",
    "module",
    "modules",
    "sbom",
    "lockfile",
];

/// Whether the (lowercased) text contains at least one ecosystem context word.
/// Short terms use word-boundary matching ("gem" must not fire on "judgement");
/// longer terms match as substrings so plurals like "packages" still count.
#[cfg(test)]
fn has_ecosystem_context(text: &str) -> bool {
    ECOSYSTEM_CONTEXT_WORDS.iter().any(|w| {
        if w.len() <= 4 {
            has_word_boundary_match(text, w)
        } else {
            text.contains(w)
        }
    })
}

fn has_context_phrase(text: &str, dep_lower: &str, context_word: &str) -> bool {
    for separator in [" ", "-", "/", "."] {
        let dep_first = format!("{dep_lower}{separator}{context_word}");
        if has_word_boundary_match(text, &dep_first) {
            return true;
        }
        let context_first = format!("{context_word}{separator}{dep_lower}");
        if has_word_boundary_match(text, &context_first) {
            return true;
        }
    }
    false
}

fn has_adjacent_ecosystem_context(text: &str, dep_lower: &str, context_words: &[&str]) -> bool {
    context_words
        .iter()
        .any(|word| has_context_phrase(text, dep_lower, word))
}

fn has_strict_dep_context(
    title_lower: &str,
    content_lower: &str,
    dep_lower: &str,
    context_words: &[&str],
) -> bool {
    has_word_boundary_match(title_lower, dep_lower)
        && (has_adjacent_ecosystem_context(title_lower, dep_lower, context_words)
            || has_adjacent_ecosystem_context(content_lower, dep_lower, context_words))
}

/// Decide whether an item (lowercased title + content) is genuinely about the
/// given (lowercased) dependency.
///
/// Policy:
/// - Normal names: word-boundary match in title OR content.
/// - Strict-proof names (ambiguous or <4 chars): word-boundary match in the
///   TITLE (content alone never qualifies) AND package/ecosystem context adjacent
///   to the dependency name in title or content.
pub(crate) fn dep_grounded_match(title_lower: &str, content_lower: &str, dep_lower: &str) -> bool {
    if dep_lower.is_empty() {
        return false;
    }
    if !requires_strict_proof(dep_lower) {
        return has_word_boundary_match(title_lower, dep_lower)
            || has_word_boundary_match(content_lower, dep_lower);
    }
    has_strict_dep_context(
        title_lower,
        content_lower,
        dep_lower,
        ECOSYSTEM_CONTEXT_WORDS,
    )
}

/// Ecosystem-SPECIFIC context tokens keyed by the dependency's manifest
/// language (as recorded in `project_dependencies.language`). The generic
/// [`ECOSYSTEM_CONTEXT_WORDS`] list proves "this text discusses a software
/// package" — but for ambiguous names that is not enough: live 2026-07-21,
/// the user's CARGO crate `tracing` minted 7 open "Security: tracing" windows
/// from dd-trace-{java,py,go,js,rb,dotnet} advisories, because a Java tracing
/// library legitimately says "library". An ambiguous name must be discussed
/// in ITS OWN ecosystem's vocabulary to bind to the user's dependency.
fn ecosystem_specific_context_words(language: &str) -> &'static [&'static str] {
    match language {
        "rust" => &["cargo", "crate", "crates", "crates.io", "rust"],
        "javascript" | "typescript" | "node" => &[
            "npm",
            "javascript",
            "typescript",
            "yarn",
            "pnpm",
            "js",
            "package",
            "packages",
            "dependency",
            "dependencies",
            "module",
            "modules",
        ],
        "python" => &["pip", "pypi", "python"],
        "go" => &["golang", "go.mod", "go module", "go modules"],
        "ruby" => &["gem", "rubygems", "ruby", "bundler"],
        "java" | "kotlin" => &["maven", "gradle", "java", "kotlin"],
        "csharp" | "dotnet" => &["nuget", "dotnet", ".net"],
        _ => &[],
    }
}

/// [`dep_grounded_match`] with the dependency's manifest language. For
/// strict-proof names with a KNOWN language, the context requirement narrows
/// from "any package vocabulary" to that ecosystem's own tokens (short tokens
/// word-boundary matched, longer ones as substrings — same rule as the
/// generic list). Unknown language, or any non-strict name, falls back to
/// [`dep_grounded_match`] unchanged.
pub(crate) fn dep_grounded_match_for_ecosystem(
    title_lower: &str,
    content_lower: &str,
    dep_lower: &str,
    language: Option<&str>,
) -> bool {
    if dep_lower.is_empty() {
        return false;
    }
    if !requires_strict_proof(dep_lower) {
        return dep_grounded_match(title_lower, content_lower, dep_lower);
    }
    let words = language
        .map(|l| ecosystem_specific_context_words(&l.to_lowercase()))
        .unwrap_or(&[]);
    if words.is_empty() {
        return dep_grounded_match(title_lower, content_lower, dep_lower);
    }
    has_strict_dep_context(title_lower, content_lower, dep_lower, words)
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
        // Generic English words that are also real package names.
        assert!(requires_strict_proof("path"));
        assert!(requires_strict_proof("open"));
        assert!(requires_strict_proof("router"));
        assert!(requires_strict_proof("clone"));
        assert!(requires_strict_proof("read"));
        assert!(requires_strict_proof("windows"));
        // Distinctive names need no extra proof.
        assert!(!requires_strict_proof("axios"));
        assert!(!requires_strict_proof("lodash"));
    }

    #[test]
    fn regression_path_does_not_match_singularity_advisory() {
        // Real false win: dep "path" (npm) bound to a Singularity CVE whose
        // title uses "path" as an ordinary English word. Must NOT match.
        let title = "singluarity: incorrect path matching for 'limit container paths'";
        assert!(!dep_grounded_match(title, "", "path"));
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

    // -- dep_grounded_match_for_ecosystem --

    #[test]
    fn regression_rust_tracing_does_not_match_dd_trace_java_advisory() {
        // Live 2026-07-21: 7 open "Security: tracing" windows minted from
        // dd-trace-{java,py,go,js,rb,dotnet} CVEs against the user's CARGO
        // `tracing` crate — "library"/"package" vocabulary passed the generic
        // context check. Ecosystem-specific context must refuse them.
        let title = "[cve-2026-50270] dd-trace-java: improper parsing of w3c \
                     baggage headers in the distributed tracing library";
        let content = "the datadog tracing library for java applications improperly parses headers";
        // Generic matcher passes (title has word-boundary "tracing", content
        // says "library") — the exact hole.
        assert!(dep_grounded_match(title, content, "tracing"));
        // Ecosystem-aware matcher with the dep's real language refuses.
        assert!(!dep_grounded_match_for_ecosystem(
            title,
            content,
            "tracing",
            Some("rust")
        ));
    }

    #[test]
    fn rust_tracing_still_matches_cargo_context() {
        let title = "tracing 0.2 released with structured spans";
        let content = "the tracing crate for rust adds cargo feature flags";
        assert!(dep_grounded_match_for_ecosystem(
            title,
            content,
            "tracing",
            Some("rust")
        ));
    }

    #[test]
    fn ecosystem_matcher_falls_back_when_language_unknown_or_name_distinct() {
        // Unknown language → generic strict-proof behavior, still requiring
        // package context adjacent to the ambiguous dependency name.
        assert!(!dep_grounded_match_for_ecosystem(
            "tracing improvements in the new library",
            "",
            "tracing",
            None
        ));
        assert!(dep_grounded_match_for_ecosystem(
            "tracing library improvements",
            "",
            "tracing",
            None
        ));
        // Distinctive names never need ecosystem proof.
        assert!(dep_grounded_match_for_ecosystem(
            "axios 2.0 released",
            "",
            "axios",
            Some("rust")
        ));
        // Empty dep never matches.
        assert!(!dep_grounded_match_for_ecosystem(
            "x",
            "x",
            "",
            Some("rust")
        ));
    }

    #[test]
    fn npm_ambiguous_name_needs_js_context() {
        // "image" (ambiguous) as a JS dep: a photography article must not
        // bind; an npm-context article does.
        assert!(!dep_grounded_match_for_ecosystem(
            "image compression tips for photographers",
            "shoot raw and edit later",
            "image",
            Some("javascript")
        ));
        assert!(dep_grounded_match_for_ecosystem(
            "image package 3.0 on npm adds avif",
            "install via npm",
            "image",
            Some("javascript")
        ));
    }

    #[test]
    fn javascript_router_does_not_match_nextjs_app_router_context() {
        let title =
            "[ghsa-m99w-x7hq-7vfj] next.js: denial of service in app router using server actions";
        let content = "vulnerable package next on npm";
        assert!(!dep_grounded_match_for_ecosystem(
            title,
            content,
            "router",
            Some("javascript")
        ));
    }

    #[test]
    fn javascript_clone_does_not_match_git_clone_node_context() {
        let title = "[cve-2026-65598] n8n: race condition in git clone node allows rce";
        let content = "the affected package is n8n, not clone";
        assert!(!dep_grounded_match_for_ecosystem(
            title,
            content,
            "clone",
            Some("javascript")
        ));
    }

    #[test]
    fn javascript_next_matches_nextjs_but_not_plain_english_next() {
        assert!(!dep_grounded_match_for_ecosystem(
            "what comes next for web apps",
            "javascript developers discuss future architecture",
            "next",
            Some("javascript")
        ));
        assert!(dep_grounded_match_for_ecosystem(
            "next.js 16 released",
            "next.js package update on npm",
            "next",
            Some("javascript")
        ));
    }

    #[test]
    fn javascript_motion_does_not_match_motion_sensor_articles() {
        assert!(!dep_grounded_match_for_ecosystem(
            "motion sensors and home security gadgets",
            "home automation hardware with no npm package context",
            "motion",
            Some("javascript")
        ));
    }

    #[test]
    fn rust_profiling_needs_crate_context() {
        assert!(!dep_grounded_match_for_ecosystem(
            "python 3.15 ultra-low overhead interpreter profiling mode",
            "profiling mode for cpython",
            "profiling",
            Some("rust")
        ));
        assert!(dep_grounded_match_for_ecosystem(
            "profiling crate released",
            "the profiling crate for rust adds cargo features",
            "profiling",
            Some("rust")
        ));
    }

    #[test]
    fn rust_windows_does_not_match_linux_rust_article() {
        let title = "linux-native tui written in rust";
        let content = "mentions windows only as an operating system comparison";
        assert!(!dep_grounded_match_for_ecosystem(
            title,
            content,
            "windows",
            Some("rust")
        ));
    }
}
