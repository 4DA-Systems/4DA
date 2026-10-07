// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for hybrid search: FTS5 sanitizing, the exact-title lane, RRF fusion and
//! the merge that keeps exact and top keyword hits from being truncated away.

use super::exact::{exact_title_queries, exact_title_terms, title_has_exact_phrase};
use super::*;
use crate::test_utils::{seed_embedding, test_db};

fn cand(id: i64, title: &str) -> Candidate {
    Candidate {
        id,
        title: title.to_string(),
        content: String::new(),
        source_type: "test".to_string(),
        url: None,
        created_at: None,
        distance: None,
    }
}

fn terms(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_string()).collect()
}

// ---------------------------------------------------------------- sanitizer

#[test]
fn fts5_query_sanitization() {
    assert_eq!(sanitize_fts5_query("tokio async"), "\"tokio\"* \"async\"*");
    assert_eq!(sanitize_fts5_query("react-dom"), "\"react-dom\"*");
    assert_eq!(sanitize_fts5_query(""), "");
    assert_eq!(sanitize_fts5_query("   "), "");
    assert_eq!(
        sanitize_fts5_query("OR AND NOT"),
        "\"OR\"* \"AND\"* \"NOT\"*"
    );
}

#[test]
fn fts5_sanitizer_escapes_quotes_and_keeps_package_names_whole() {
    // An embedded quote is doubled, never left to close the string early.
    assert_eq!(sanitize_fts5_query("say\"hi"), "\"say\"\"hi\"*");
    // Scoped npm names used to lose `@` and `/`, gluing "tauri-appsapi" together.
    assert_eq!(
        sanitize_fts5_query("@tauri-apps/api"),
        "\"@tauri-apps/api\"*"
    );
    // Punctuation-only tokens carry no searchable text and are dropped.
    assert_eq!(sanitize_fts5_query("\" * ( )"), "");
}

#[test]
fn exact_lane_query_quotes_and_column_filters() {
    assert_eq!(
        exact_title_queries("rusqlite"),
        vec![("title : \"rusqlite\"".to_string(), terms(&["rusqlite"]))]
    );
    assert_eq!(
        exact_title_queries("\"@scope/name\"?"),
        vec![(
            "title : \"@scope/name\"".to_string(),
            terms(&["@scope/name"])
        )]
    );
    // Stop words drop out; the phrase keeps query order.
    assert_eq!(
        exact_title_queries("show me the tokio runtime"),
        vec![(
            "title : \"tokio runtime\"".to_string(),
            terms(&["tokio", "runtime"])
        )]
    );
    // Multi-word: the whole phrase first, then each package-like term alone.
    let q = exact_title_queries("react-dom hydration");
    assert_eq!(q.len(), 2);
    assert_eq!(
        q[1],
        ("title : \"react-dom\"".to_string(), terms(&["react-dom"]))
    );
    assert!(exact_title_queries("").is_empty());
    assert!(exact_title_queries("\"\" ??").is_empty());
    assert!(exact_title_terms("what is the").is_empty());
}

#[test]
fn exact_title_requires_word_boundaries() {
    let rusqlite = terms(&["rusqlite"]);
    assert!(title_has_exact_phrase(
        "crates.io: rusqlite v0.40.2",
        &rusqlite
    ));
    assert!(title_has_exact_phrase("Rusqlite", &rusqlite));
    assert!(title_has_exact_phrase("rusqlite.rs released", &rusqlite));
    assert!(!title_has_exact_phrase("rusqlite-migration 1.0", &rusqlite));
    assert!(!title_has_exact_phrase("myrusqlite wrapper", &rusqlite));
    assert!(!title_has_exact_phrase("", &rusqlite));
    assert!(title_has_exact_phrase(
        "The  Tokio   Runtime explained",
        &terms(&["tokio", "runtime"])
    ));
    assert!(title_has_exact_phrase(
        "npm: @tauri-apps/api 2.1",
        &terms(&["@tauri-apps/api"])
    ));
    assert!(!title_has_exact_phrase("anything", &[]));
}

// ---------------------------------------------------------------- fusion + merge

#[test]
fn rrf_fusion_logic() {
    let score_rank1 = 1.0 / (RRF_K + 1.0);
    let score_rank10 = 1.0 / (RRF_K + 10.0);
    assert!(score_rank1 > score_rank10);
    assert!(score_rank1 < 0.02); // rank 1 ~= 0.0164
}

/// The 2026-10-07 defect, reproduced: 90 vector candidates, 7 FTS candidates, and
/// the exact title (`crates.io: rusqlite v0.40.2`) found ONLY by FTS at rank 6.
#[test]
fn fts_only_exact_title_survives_truncation_and_ranks_top_three() {
    let vector: Vec<Candidate> = (1..=90).map(|i| cand(i, &format!("rus-ish {i}"))).collect();
    let mut bm25: Vec<Candidate> = (1001..=1005)
        .map(|i| cand(i, &format!("rusqlite-pool {i}")))
        .collect();
    let exact = cand(1006, "crates.io: rusqlite v0.40.2");
    bm25.push(exact.clone());
    bm25.push(cand(1007, "rusqlite_helpers"));

    let fused = fuse_rrf(&bm25, &vector, 0.4, 0.6);
    // Pure RRF: a keyword-only rank-1 hit scores below vector rank 30.
    let fts_rank1 = fused
        .iter()
        .find(|r| r.item_id == 1001)
        .map(|r| r.rrf_score);
    let vec_rank30 = fused.iter().find(|r| r.item_id == 30).map(|r| r.rrf_score);
    assert!(
        fts_rank1 < vec_rank30,
        "precondition: RRF buries keyword-only hits"
    );
    let plain: Vec<i64> = fused.iter().take(30).map(|r| r.item_id).collect();
    assert!(
        !plain.contains(&1006),
        "precondition: plain truncate drops the exact title"
    );

    let merged = merge_with_exact_lane(&[exact], fused, 30);
    assert_eq!(merged.len(), 30);
    let pos = merged.iter().position(|r| r.item_id == 1006);
    assert!(
        pos.is_some_and(|p| p < 3),
        "exact title must rank top 3, got {pos:?}"
    );
    assert!(merged[0].exact_title);
    assert_eq!(
        merged[0].bm25_rank,
        Some(6),
        "pinned item keeps its fused ranks"
    );
    // The top keyword hits are guaranteed a slot even without an exact title.
    for id in [1001, 1002, 1003] {
        assert!(
            merged.iter().any(|r| r.item_id == id),
            "FTS top hit {id} must survive"
        );
    }
    let ids: std::collections::HashSet<i64> = merged.iter().map(|r| r.item_id).collect();
    assert_eq!(ids.len(), merged.len(), "no duplicates");
}

#[test]
fn merge_without_exact_matches_still_guarantees_top_fts_hits() {
    let vector: Vec<Candidate> = (1..=90).map(|i| cand(i, "v")).collect();
    let bm25 = vec![cand(500, "kw-only"), cand(5, "both legs")];
    let merged = merge_with_exact_lane(&[], fuse_rrf(&bm25, &vector, 0.4, 0.6), 10);
    assert_eq!(merged.len(), 10);
    assert!(merged.iter().any(|r| r.item_id == 500));
    assert!(merged.iter().all(|r| !r.exact_title));
}

#[test]
fn merge_respects_pin_cap_and_tiny_limits() {
    let exact: Vec<Candidate> = (1..=8).map(|i| cand(i, "x")).collect();
    let merged = merge_with_exact_lane(&exact, Vec::new(), 30);
    assert_eq!(merged.len(), MAX_EXACT_PINS);
    let merged = merge_with_exact_lane(&exact, Vec::new(), 2);
    assert_eq!(merged.len(), 2);
    assert!(merge_with_exact_lane(&exact, Vec::new(), 0).is_empty());
}

// ---------------------------------------------------------------- live SQLite

/// End to end on a real FTS5 + sqlite-vec database: the exact title is first even
/// though the query embedding points at a crowd of unrelated items, and a
/// hyphen-continued title (`rusqlite-migration`) is not treated as exact.
#[test]
fn hybrid_search_pins_exact_title_on_real_index() {
    let db = test_db();
    let probe = seed_embedding("crowd");
    for i in 0..100 {
        db.upsert_source_item(
            "rss",
            &format!("crowd-{i}"),
            None,
            &format!("rumdl rsconstruct {i}"),
            "rust tooling",
            &probe,
        )
        .expect("upsert crowd");
    }
    let near_miss = db
        .upsert_source_item(
            "rss",
            "nm",
            None,
            "rusqlite-migration 1.3",
            "rusqlite migrations",
            &seed_embedding("nm"),
        )
        .expect("upsert near miss");
    let target = db
        .upsert_source_item(
            "cratesio",
            "rq",
            None,
            "crates.io: rusqlite v0.40.2",
            "Ergonomic SQLite bindings",
            &seed_embedding("rq"),
        )
        .expect("upsert target");

    let results = db.hybrid_search("rusqlite", &probe, 30, 0.4, 0.6);
    assert_eq!(results.len(), 30);
    assert_eq!(results[0].item_id, target);
    assert!(results[0].exact_title);
    let near = results.iter().find(|r| r.item_id == near_miss);
    assert!(
        near.is_some_and(|r| !r.exact_title),
        "keyword hit kept, but not pinned as exact"
    );

    // FTS syntax in user input must not error the search away.
    for hostile in ["\"", "NEAR(", "title:", "@scope/name", "a OR", "*"] {
        let _ = db.hybrid_search(hostile, &probe, 5, 0.4, 0.6);
    }
}
