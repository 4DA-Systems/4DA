// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Ranking expectations for named-package queries (audit L8) and the whole-word
//! stack boost. The fixture relevances are the live "sharp vulnerability" numbers
//! measured on 2026-10-26 before the change.

use super::*;

#[path = "natural_language_search_rank_live.rs"]
mod live;

fn item(id: i64, relevance: f64, title: &str, preview: &str) -> QueryResultItem {
    QueryResultItem {
        id,
        file_path: None,
        file_name: Some(title.to_string()),
        preview: preview.to_string(),
        relevance,
        source_type: "cve".to_string(),
        timestamp: None,
        match_reason: "keyword + semantic".to_string(),
        exact_match: false,
        affects_package: None,
        foreign_ecosystem: false,
    }
}

fn affects(mut i: QueryResultItem, pkg: &str) -> QueryResultItem {
    i.affects_package = Some(pkg.to_string());
    i
}

fn foreign(mut i: QueryResultItem) -> QueryResultItem {
    i.foreign_ecosystem = true;
    i
}

fn stack(names: &[&str]) -> Vec<StackContextEntry> {
    names
        .iter()
        .map(|n| StackContextEntry {
            name: (*n).to_string(),
            category: "lib".to_string(),
            relevant: false,
        })
        .collect()
}

fn ids(items: &[QueryResultItem]) -> Vec<i64> {
    items.iter().map(|i| i.id).collect()
}

/// The L8 page: Next.js advisories that mention sharp (boosted by the user's Next
/// stack) sat above sharp's own advisories, and .NET advisories filled the tail.
#[test]
fn package_advisories_outrank_mentions_and_foreign_ecosystems_sink() {
    let mut items = vec![
        item(
            84996,
            0.62,
            "Next.js: RCE in Image Optimization API",
            "libheif used by `sharp`",
        ),
        item(53326, 0.60, "next: Next.js Content Injection", ""),
        affects(
            item(154544, 0.686, "sharp : Vulnerability in librsvg", ""),
            "sharp",
        ),
        affects(
            item(84990, 0.680, "sharp: Vulnerabilities in libheif", ""),
            "sharp",
        ),
        foreign(item(
            346,
            0.642,
            ".NET Elevation of Privilege Vulnerability",
            "",
        )),
        foreign(item(
            349,
            0.639,
            ".NET Information Disclosure Vulnerability",
            "",
        )),
        item(
            84988,
            0.639,
            "Astro: RCE through AVIF image optimization",
            "Sharp image service",
        ),
    ];
    boost_for_stack(&mut items, &stack(&["next.js", "next"]));
    rank_named_packages(&mut items);
    sort_items(&mut items);

    let order = ids(&items);
    assert_eq!(
        &order[..2],
        &[154544, 84990],
        "sharp's own advisories first"
    );
    assert_eq!(&order[5..], &[346, 349], ".NET advisories last");
    assert!(items[0].match_reason.starts_with("advisory for sharp"));
    assert!(items[5]
        .match_reason
        .contains("ecosystem not in your projects"));
    for pair in items.windows(2) {
        assert!(
            pair[0].relevance >= pair[1].relevance,
            "displayed relevance must read monotonically"
        );
    }
    assert!(items.iter().all(|i| i.relevance <= NON_EXACT_RELEVANCE_CAP));
}

#[test]
fn exact_title_matches_stay_above_package_advisories() {
    let mut exact = item(1, 0.999, "npm: sharp v0.35.5", "");
    exact.exact_match = true;
    let mut items = vec![
        affects(item(2, 0.95, "sharp: advisory", ""), "sharp"),
        exact,
    ];
    rank_named_packages(&mut items);
    sort_items(&mut items);
    assert_eq!(ids(&items), vec![1, 2]);
    assert!(
        (items[0].relevance - 0.999).abs() < f64::EPSILON,
        "exact untouched"
    );
}

/// A topic query carries no package marks, so its ranking is unchanged.
#[test]
fn topic_queries_are_untouched() {
    let mut items = vec![
        item(1, 0.72, "A mini async runtime for Rust", ""),
        item(2, 0.68, "Deterministic testing harness for async Rust", ""),
        item(3, 0.65, "Async HTTP framework", ""),
    ];
    let before: Vec<(i64, f64)> = items.iter().map(|i| (i.id, i.relevance)).collect();
    rank_named_packages(&mut items);
    sort_items(&mut items);
    let after: Vec<(i64, f64)> = items.iter().map(|i| (i.id, i.relevance)).collect();
    assert_eq!(before, after);
}

#[test]
fn foreign_ecosystem_demotion_applies_without_a_package_hit() {
    // The query named an installed package that has no advisories in the corpus.
    let mut items = vec![
        foreign(item(1, 0.64, ".NET Denial of Service Vulnerability", "")),
        item(2, 0.60, "zod: memory exhaustion in large collections", ""),
    ];
    rank_named_packages(&mut items);
    sort_items(&mut items);
    assert_eq!(ids(&items), vec![2, 1]);
    assert!((items[1].relevance - 0.32).abs() < 1e-9);
}

/// The stack boost used a substring test: "rust" boosted every RUSTSEC advisory
/// and crates such as "rustyroad"; "next" boosted prose saying "next to".
#[test]
fn stack_boost_matches_whole_words_only() {
    let mut items = vec![
        item(
            1,
            0.60,
            "[RUSTSEC-2026-0367] zip: SUID bits preserved",
            "Affected: zip (crates.io)",
        ),
        item(
            2,
            0.60,
            "crates.io: rustyroad v1.8.2",
            "A framework based on Ruby on Rails",
        ),
        item(
            3,
            0.60,
            "crates.io: nezuko v0.2.0",
            "A mini async runtime for Rust.",
        ),
        item(4, 0.60, "Next.js: Image Optimization", ""),
        item(5, 0.60, "nextest 0.9 released", ""),
    ];
    boost_for_stack(&mut items, &stack(&["rust", "next"]));
    let boosted: Vec<i64> = items
        .iter()
        .filter(|i| i.relevance > 0.60)
        .map(|i| i.id)
        .collect();
    assert_eq!(boosted, vec![3, 4]);
}
