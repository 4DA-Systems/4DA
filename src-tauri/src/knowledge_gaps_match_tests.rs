// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The token index must agree with the substring scan it replaced, on every
//! fixture title the knowledge-gap suite already uses plus the edge shapes
//! (punctuated names, ext suffixes, multibyte text).

use super::super::{keyword_misses_from, misses_among, GapCandidate, MissedItem};
use super::*;

fn cand(id: i64, title: &str, source_type: &str, linked: &[&str]) -> GapCandidate {
    GapCandidate {
        title_lower: title.to_lowercase(),
        item: MissedItem {
            item_id: id,
            title: title.to_string(),
            url: None,
            source_type: source_type.to_string(),
            created_at: "2026-09-27 13:03:00".to_string(),
        },
        source_id: String::new(),
        content_type: Some("discussion".to_string()),
        linked_packages: linked.iter().map(|s| s.to_string()).collect(),
        version_affected: None,
    }
}

/// Titles from the existing knowledge-gap fixtures, verbatim, plus the
/// shapes a token index could plausibly get wrong.
const TITLES: &[&str] = &[
    "Tokio async runtime v1.36 released",
    "CVE-2026-1234 affects tokio 1.x",
    "Why we chose chatter-3 for our side project",
    "[GHSA-h395-gr6q-cpjc] jsonwebtoken: Type Confusion leads to authorization bypass",
    "[RUSTSEC-2026-0007] jsonwebtoken: header parsing panic",
    "npm: stripe v22.6.1",
    "crates.io: jsonwebtoken v11.0.0",
    "📢 New updates · 3 Sept #15.1 · onecli v2.5.0 — Added Slack Stripe AWS billing fixes",
    "Announcing axum 0.8.0",
    "npm: @modelcontextprotocol/node v2.1.0",
    "Announcing @modelcontextprotocol/node 1.9.0",
    "Why @modelcontextprotocol/node matters",
    "TypeScript: the practical guide for JS developers",
    "crates.io: code-split-plugin-typescript v1.0.0-alpha.4",
    "[CVE-2026-54736] Phalcon: Non-constant-time HMAC verification",
    "[RUSTSEC-2026-0100] hmac: timing leak in verify_slice",
    "[GHSA-x] @hono/oauth-providers: token leak",
    "[CVE-2026-46428] lettre: header injection",
    "Your AI builder shipped the Stripe code in an afternoon",
    "How I built an AI Line Art SaaS with Next.js and Stripe credits",
    "Critical vulnerability found in axum",
    "[CVE-2026-71850] Hono: memo() retains SSR output across requests",
    "Phonology of consonant clusters in synthesis",
    "Building a phonograph simulator in Rust",
    "Unexpected panic in the parser",
    "Next.js 15 release notes",
    "hono security advisory number 0",
    "Onefold + Hono + Cloudflare edge rendering",
    "crates.io: chrono v0.4.45",
    "crates.io: serde_json v1.0.140 and serde-json-ish fork",
    "react-dom 19 drops legacy APIs; react-router v7",
    "éclair2 and éclair here",
    "привет9 world",
    "serde.rs guide to serde",
    "tauri-plugin-notification v2 for Tauri",
];

const NAMES: &[&str] = &[
    "tokio",
    "jsonwebtoken",
    "stripe",
    "axum",
    "@modelcontextprotocol/node",
    "typescript",
    "hmac",
    "hono",
    "lettre",
    "next",
    "chrono",
    "serde_json",
    "serde-json",
    "react-dom",
    "react",
    "react-router",
    "éclair",
    "serde",
    "tauri",
    "notification",
    "chatter-3",
    "@",
    "missingname",
];

fn fixtures() -> Vec<GapCandidate> {
    TITLES
        .iter()
        .enumerate()
        .map(|(i, t)| cand(i as i64, t, "hackernews", &[]))
        .collect()
}

#[test]
fn the_index_agrees_with_the_substring_scan_on_every_fixture() {
    let candidates = fixtures();
    let index = CandidateIndex::build(&candidates);
    for name in NAMES {
        let dep_lower = name.to_lowercase();
        let scanned: Vec<i64> = candidates
            .iter()
            .filter(|c| title_names(&c.title_lower, &dep_lower))
            .map(|c| c.item.item_id)
            .collect();
        let indexed: Vec<i64> = index
            .matching(&candidates, &dep_lower)
            .iter()
            .map(|c| c.item.item_id)
            .collect();
        assert_eq!(indexed, scanned, "matcher disagreement for {name:?}");
    }
}

#[test]
fn indexed_misses_equal_the_original_keyword_misses() {
    let candidates = fixtures();
    let index = CandidateIndex::build(&candidates);
    let no_live = |_: &GapCandidate| -> Option<bool> { None };
    for name in NAMES {
        let old: Vec<i64> = keyword_misses_from(&candidates, name, &no_live)
            .iter()
            .map(|m| m.item_id)
            .collect();
        let hits = index.matching(&candidates, &name.to_lowercase());
        let new: Vec<i64> = misses_among(hits.into_iter(), name, &no_live)
            .iter()
            .map(|m| m.item_id)
            .collect();
        assert_eq!(new, old, "keyword misses differ for {name:?}");
    }
}

#[test]
fn the_index_is_not_vacuous() {
    // Guard on the premise of the equivalence tests: the fixtures DO match.
    let candidates = fixtures();
    let index = CandidateIndex::build(&candidates);
    assert!(index.matching(&candidates, "hono").len() >= 3);
    assert_eq!(index.matching(&candidates, "next").len(), 2);
    assert_eq!(index.matching(&candidates, "serde_json").len(), 1);
    assert!(index.matching(&candidates, "missingname").is_empty());
}

#[test]
fn engagement_counts_clicks_and_hoists_detected_tech() {
    let engagement = Engagement::from_parts(
        &[7],
        &[("crates.io: chrono v0.4.45", "2000-01-01 00:00:00")],
        &["Tauri"],
    );
    assert!(engagement.is_engaged(7), "a clicked item is read");
    assert!(!engagement.is_engaged(8));
    assert!(engagement.days_since("chrono") > 30, "engaged long ago");
    assert_eq!(
        engagement.days_since("tauri"),
        0,
        "ACE-detected tech is current"
    );
    assert_eq!(engagement.days_since("hono"), 999, "never engaged");
}

#[test]
fn engagement_loads_feedback_and_engagement_interactions_only() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE source_items (id INTEGER PRIMARY KEY, title TEXT);
         CREATE TABLE feedback (id INTEGER PRIMARY KEY, source_item_id INTEGER,
                                relevant INTEGER, created_at TEXT);
         CREATE TABLE interactions (id INTEGER PRIMARY KEY, source_item_id INTEGER,
                                    item_id INTEGER, action TEXT, action_type TEXT,
                                    timestamp TEXT);
         INSERT INTO source_items VALUES (1, 'a'), (2, 'b'), (3, 'c'), (4, 'd');
         INSERT INTO feedback VALUES (1, 1, 1, '2026-10-01 00:00:00');
         INSERT INTO interactions VALUES (1, NULL, 2, NULL, 'click', '2026-10-01 00:00:00');
         INSERT INTO interactions VALUES (2, NULL, 3, NULL, 'scroll', '2026-10-01 00:00:00');
         INSERT INTO interactions VALUES (3, 4, NULL, 'save', NULL, '2026-10-01 00:00:00');",
    )
    .unwrap();
    let engagement = Engagement::load(&conn);
    assert!(engagement.is_engaged(1), "feedback");
    assert!(engagement.is_engaged(2), "ACE click (item_id/action_type)");
    assert!(!engagement.is_engaged(3), "a scroll-past is not a read");
    assert!(
        engagement.is_engaged(4),
        "ContextEngine save (source_item_id/action)"
    );
}
