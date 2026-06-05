// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the deterministic morning-brief floor (pure rendering — no feed/DB needed).

use super::render_signals_section;
use crate::db::DigestSourceItem;
use std::collections::HashMap;

fn item(id: i64, title: &str, score: f64) -> DigestSourceItem {
    DigestSourceItem {
        id,
        title: title.to_string(),
        url: None,
        source_type: "hackernews".to_string(),
        created_at: chrono::Utc::now(),
        relevance_score: Some(score),
        topics: vec![],
        content_type: None,
    }
}

#[test]
fn empty_items_render_honest_placeholder() {
    let out = render_signals_section(&[], &HashMap::new());
    assert!(out.contains("No relevant items"), "got: {out}");
}

#[test]
fn signals_render_ranked_with_percent_and_why() {
    let items = vec![
        item(1, "Async Rust deep-dive", 0.87),
        item(2, "axum 0.8 release", 0.61),
    ];
    let mut expl = HashMap::new();
    expl.insert(1, "matches your recent tokio work".to_string());
    // id 2 has no explanation → renders without the "— why" suffix.
    let out = render_signals_section(&items, &expl);

    assert!(out.contains("1. **Async Rust deep-dive** (87%) — matches your recent tokio work"));
    assert!(out.contains("2. **axum 0.8 release** (61%)"));
    assert!(
        !out.contains("2. **axum 0.8 release** (61%) —"),
        "no dangling em-dash when why is empty"
    );
}

#[test]
fn placeholder_why_is_suppressed() {
    let items = vec![item(1, "Some item", 0.5)];
    let mut expl = HashMap::new();
    expl.insert(1, "No context match".to_string()); // the placeholder — must not render
    let out = render_signals_section(&items, &expl);
    assert!(out.contains("1. **Some item** (50%)"));
    assert!(
        !out.contains("No context match"),
        "placeholder why must be suppressed"
    );
}

#[test]
fn caps_at_ten_signals() {
    let items: Vec<DigestSourceItem> = (1..=15)
        .map(|i| item(i, &format!("item {i}"), 0.5))
        .collect();
    let out = render_signals_section(&items, &HashMap::new());
    assert!(out.contains("10. **item 10**"));
    assert!(!out.contains("11. **item 11**"), "should cap at 10");
}
