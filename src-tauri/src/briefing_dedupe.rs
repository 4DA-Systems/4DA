// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Intra-batch fuzzy dedupe for morning-briefing items.
//!
//! Before this module existed, items from the fetched corpus were
//! collected and shipped straight into the display list. When HN and
//! Reddit both carried "React 19.2.3 released" we would render both,
//! wasting a slot; worse, the LLM synthesis would see both and
//! double-count the signal when building its PRIORITY section.
//!
//! This module collapses near-duplicate titles to their highest-scoring
//! representative before the LLM and the UI see the list. Two titles
//! count as near-duplicates when they share enough word-bigram overlap
//! (Jaccard ≥ 0.55) OR when one is a prefix of the other. Both are
//! tuned against production data — see tests for exemplars.
//!
//! Extracted from `monitoring_briefing.rs` to keep that file under its
//! 1500-line hard error threshold.

use crate::monitoring_briefing::BriefingItem;

/// Collapse near-duplicate briefing items to their highest-scoring
/// representative. Returns items in score-descending order.
pub fn dedupe_briefing_items(items: Vec<BriefingItem>) -> Vec<BriefingItem> {
    // Sort by score descending so `kept` contains the best representative
    // and later iterations compare against already-chosen winners.
    let mut sorted = items;
    sorted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut kept: Vec<BriefingItem> = Vec::with_capacity(sorted.len());
    'outer: for candidate in sorted {
        let cand_norm = normalize_title(&candidate.title);
        let cand_bigrams = title_bigrams(&cand_norm);
        for existing in &kept {
            let ex_norm = normalize_title(&existing.title);
            if is_prefix_duplicate(&cand_norm, &ex_norm) {
                continue 'outer;
            }
            // Version-series: "TypeScript 5.9 Beta" vs "TypeScript 7.0 Beta"
            if is_version_series_duplicate(&cand_norm, &ex_norm) {
                continue 'outer;
            }
            let ex_bigrams = title_bigrams(&ex_norm);
            // Threshold tuned against production data. 0.55 merges items
            // that differ by 1-2 filler words ("Rust 1.80 released" vs
            // "Rust 1.80 released with fixes"), while keeping distinct
            // signals apart ("Rust 1.80" vs "Rust 1.37"). Raise to 0.70
            // if you see legitimate items being wrongly merged.
            if jaccard(&cand_bigrams, &ex_bigrams) >= 0.55 {
                continue 'outer;
            }
        }
        kept.push(candidate);
    }
    kept
}

/// Lowercase, strip non-alphanumeric except spaces, collapse whitespace.
pub(crate) fn normalize_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut prev_space = false;
    for c in title.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            prev_space = false;
        } else if c.is_whitespace() || c == '-' || c == '_' {
            if !prev_space && !out.is_empty() {
                out.push(' ');
                prev_space = true;
            }
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

/// "one is a prefix of the other" — tolerates trailing punctuation like
/// "React 19.2.3" vs "React 19.2.3 released with concurrent rendering".
pub(crate) fn is_prefix_duplicate(a: &str, b: &str) -> bool {
    if a.len() < 10 || b.len() < 10 {
        return false;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    long.starts_with(short) && (long.len() - short.len() <= 200)
}

/// Strip version numbers from a normalized title to detect version-series duplicates.
/// "announcing typescript 59 beta" → "announcing typescript beta"
/// "react 1923 released" → "react released"
/// "mikro orm 71 lazy ref" → "mikro orm lazy ref"
fn strip_version_numbers(normalized: &str) -> String {
    normalized
        .split_whitespace()
        .filter(|word| {
            // Remove tokens that are purely numeric (version fragments)
            // After normalize_title, "5.9" becomes "59", "19.2.3" becomes "1923"
            !word.chars().all(|c| c.is_ascii_digit())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Detect version-series duplicates: titles that are identical except for version numbers.
/// "announcing typescript 59 beta" vs "announcing typescript 70 beta" → true
/// "rust 180 released" vs "postgres 17 released" → false
pub(crate) fn is_version_series_duplicate(a: &str, b: &str) -> bool {
    let a_stripped = strip_version_numbers(a);
    let b_stripped = strip_version_numbers(b);
    // Both must have meaningful content after stripping (at least 2 words)
    if a_stripped.split_whitespace().count() < 2 || b_stripped.split_whitespace().count() < 2 {
        return false;
    }
    // Must be identical after version stripping
    a_stripped == b_stripped
}

/// Word-bigram set for Jaccard similarity.
pub(crate) fn title_bigrams(norm: &str) -> std::collections::HashSet<String> {
    let words: Vec<&str> = norm.split_whitespace().collect();
    let mut set = std::collections::HashSet::with_capacity(words.len());
    if words.len() < 2 {
        if let Some(w) = words.first() {
            set.insert((*w).to_string());
        }
        return set;
    }
    for pair in words.windows(2) {
        set.insert(format!("{} {}", pair[0], pair[1]));
    }
    set
}

pub(crate) fn jaccard(
    a: &std::collections::HashSet<String>,
    b: &std::collections::HashSet<String>,
) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(b).count() as f32;
    let union = a.union(b).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_item(title: &str, source: &str, score: f32) -> BriefingItem {
        BriefingItem {
            title: title.to_string(),
            source_type: source.to_string(),
            score,
            signal_type: None,
            url: None,
            item_id: None,
            signal_priority: None,
            description: None,
            matched_deps: vec![],
            content_type: None,
            corroboration_count: 0,
            alt_sources: vec![],
            section: None,
            triage_reason: None,
        }
    }

    // ---- normalize_title ----------------------------------------------------

    #[test]
    fn normalize_title_strips_punctuation_and_lowercases() {
        assert_eq!(
            normalize_title("React 19.2.3 — Released!"),
            "react 1923 released"
        );
    }

    #[test]
    fn normalize_title_collapses_whitespace() {
        assert_eq!(
            normalize_title("  React   \t  19.2   released  "),
            "react 192 released"
        );
    }

    #[test]
    fn normalize_title_empty_input() {
        assert_eq!(normalize_title(""), "");
    }

    #[test]
    fn normalize_title_only_punctuation() {
        assert_eq!(normalize_title("!!! --- >>> "), "");
    }

    // ---- jaccard ------------------------------------------------------------

    #[test]
    fn jaccard_identical_sets_is_one() {
        let mut a = std::collections::HashSet::new();
        a.insert("react 19".into());
        a.insert("19 released".into());
        let b = a.clone();
        assert!((jaccard(&a, &b) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn jaccard_disjoint_sets_is_zero() {
        let mut a = std::collections::HashSet::new();
        a.insert("rust 180".into());
        let mut b = std::collections::HashSet::new();
        b.insert("postgres 16".into());
        assert_eq!(jaccard(&a, &b), 0.0);
    }

    #[test]
    fn jaccard_empty_sets_is_zero() {
        let a = std::collections::HashSet::new();
        let b = std::collections::HashSet::new();
        assert_eq!(jaccard(&a, &b), 0.0);
    }

    // ---- prefix duplicate ---------------------------------------------------

    #[test]
    fn is_prefix_duplicate_detects_prefix() {
        assert!(is_prefix_duplicate(
            "react 192 released",
            "react 192 released with concurrent rendering"
        ));
    }

    #[test]
    fn is_prefix_duplicate_rejects_unrelated() {
        assert!(!is_prefix_duplicate(
            "rust 180 released",
            "postgres 17 released"
        ));
    }

    #[test]
    fn is_prefix_duplicate_rejects_short_inputs() {
        assert!(!is_prefix_duplicate("rust", "rust 180 released"));
    }

    // ---- dedupe_briefing_items (end-to-end) ---------------------------------

    #[test]
    fn dedupe_keeps_highest_score_representative() {
        let items = vec![
            mk_item(
                "React 19.2.3 released with concurrent rendering",
                "reddit",
                0.42,
            ),
            mk_item(
                "React 19.2.3 released with concurrent rendering",
                "hackernews",
                0.78,
            ),
            mk_item("Postgres 17 ships pg_logical v2", "hackernews", 0.61),
        ];
        let out = dedupe_briefing_items(items);
        assert_eq!(out.len(), 2);
        let react = out.iter().find(|i| i.title.contains("React")).unwrap();
        assert_eq!(react.source_type, "hackernews");
        assert!((react.score - 0.78).abs() < 0.001);
    }

    #[test]
    fn dedupe_collapses_near_duplicate_titles() {
        let items = vec![
            mk_item(
                "Rust 1.80 released with const generics improvements",
                "hn",
                0.85,
            ),
            mk_item(
                "Rust 1.80 released with const generics improvements!",
                "rss",
                0.60,
            ),
            mk_item(
                "Rust 1.80 released const generics improvements",
                "devto",
                0.55,
            ),
        ];
        let out = dedupe_briefing_items(items);
        assert_eq!(out.len(), 1, "all three are near-duplicates");
        assert!((out[0].score - 0.85).abs() < 0.001);
    }

    #[test]
    fn dedupe_does_not_collapse_unrelated_titles() {
        let items = vec![
            mk_item(
                "Rust 1.80 released with const generics improvements",
                "hn",
                0.85,
            ),
            mk_item("Postgres 17 ships pg_logical v2 with streaming", "hn", 0.70),
            mk_item("TypeScript 5.6 brings iterator helpers", "hn", 0.65),
        ];
        let out = dedupe_briefing_items(items);
        assert_eq!(out.len(), 3, "none of these overlap");
    }

    #[test]
    fn dedupe_collapses_prefix_relationships() {
        let items = vec![
            mk_item(
                "React 19.2.3 released with concurrent rendering fixes",
                "hn",
                0.70,
            ),
            mk_item(
                "React 19.2.3 released with concurrent rendering",
                "rss",
                0.50,
            ),
        ];
        let out = dedupe_briefing_items(items);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn dedupe_empty_input() {
        let out = dedupe_briefing_items(vec![]);
        assert!(out.is_empty());
    }

    #[test]
    fn dedupe_single_item() {
        let items = vec![mk_item("Rust 1.80 released", "hn", 0.5)];
        let out = dedupe_briefing_items(items);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn dedupe_preserves_score_ordering() {
        let items = vec![
            mk_item("TypeScript 5.6 iterator helpers", "hn", 0.3),
            mk_item("Rust 1.80 const generics", "hn", 0.9),
            mk_item("Postgres 17 pg_logical", "hn", 0.6),
        ];
        let out = dedupe_briefing_items(items);
        assert_eq!(out.len(), 3);
        assert!(out[0].score >= out[1].score);
        assert!(out[1].score >= out[2].score);
    }

    // ---- version-series dedup -------------------------------------------------

    #[test]
    fn strip_version_numbers_removes_numeric_tokens() {
        assert_eq!(
            strip_version_numbers("announcing typescript 59 beta"),
            "announcing typescript beta"
        );
        assert_eq!(
            strip_version_numbers("react 1923 released"),
            "react released"
        );
        assert_eq!(strip_version_numbers("rust 180 released"), "rust released");
    }

    #[test]
    fn strip_version_numbers_preserves_non_version_numbers() {
        // Words with mixed alpha+digit should survive
        assert_eq!(
            strip_version_numbers("i18next released"),
            "i18next released"
        );
        assert_eq!(
            strip_version_numbers("react19 released"),
            "react19 released"
        );
    }

    #[test]
    fn version_series_detects_typescript_betas() {
        let a = normalize_title("Announcing TypeScript 5.9 Beta");
        let b = normalize_title("Announcing TypeScript 6.0 Beta");
        let c = normalize_title("Announcing TypeScript 7.0 Beta");
        assert!(is_version_series_duplicate(&a, &b));
        assert!(is_version_series_duplicate(&b, &c));
        assert!(is_version_series_duplicate(&a, &c));
    }

    #[test]
    fn version_series_rejects_unrelated_titles() {
        let a = normalize_title("Announcing TypeScript 5.9 Beta");
        let b = normalize_title("Announcing React 19.2 Release");
        assert!(!is_version_series_duplicate(&a, &b));
    }

    #[test]
    fn version_series_rejects_short_titles() {
        let a = normalize_title("v5.9");
        let b = normalize_title("v6.0");
        assert!(!is_version_series_duplicate(&a, &b));
    }

    #[test]
    fn dedupe_collapses_version_series() {
        let items = vec![
            mk_item("Announcing TypeScript 5.9 Beta", "hackernews", 0.75),
            mk_item("Announcing TypeScript 6.0 Beta", "reddit", 0.70),
            mk_item("Announcing TypeScript 7.0 Beta", "devto", 0.65),
            mk_item("Postgres 17 ships pg_logical v2", "hackernews", 0.60),
        ];
        let out = dedupe_briefing_items(items);
        assert_eq!(
            out.len(),
            2,
            "three TS betas collapse to one, Postgres stays"
        );
        let ts = out.iter().find(|i| i.title.contains("TypeScript")).unwrap();
        assert!(
            (ts.score - 0.75).abs() < 0.001,
            "highest-scoring TS beta kept"
        );
    }
}
