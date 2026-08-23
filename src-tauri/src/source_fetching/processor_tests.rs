// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for `source_fetching::processor` — included from processor.rs via
//! `#[path = "processor_tests.rs"] mod tests;`.

use super::*;
use crate::test_utils::{seed_embedding, test_db};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Helper: replicate the ID hashing logic used in fetch_all_sources and process_source_items
fn hash_source_id(source_type: &str, source_id: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    format!("{}:{}", source_type, source_id).hash(&mut hasher);
    hasher.finish()
}

/// Helper: build a `PendingRow` for tests.
fn pending_row(source_type: &str, source_id: &str, title: &str) -> PendingRow {
    (
        source_type.to_string(),
        source_id.to_string(),
        None,
        title.to_string(),
        format!("{title} body"),
        format!("{title} embed text"),
    )
}

/// Helper: build an `InsertRow` with the given embedding for tests.
fn insert_row(source_type: &str, source_id: &str, title: &str, embedding: Vec<f32>) -> InsertRow {
    (
        source_type.to_string(),
        source_id.to_string(),
        None,
        title.to_string(),
        format!("{title} body"),
        embedding,
        "en".to_string(),
        None,
        None,
        None,
        None,
        None,
    )
}

/// Helper: build a `(PreparedItem, String)` pair for partition tests.
fn prepared_item(source_type: &str, source_id: &str, title: &str) -> (PreparedItem, String) {
    (
        (
            source_type.to_string(),
            source_id.to_string(),
            None,
            title.to_string(),
            format!("{title} body"),
            "en".to_string(),
            None,
            None,
            None,
        ),
        format!("{title} embed text"),
    )
}

// ---------- Test 1: ID hashing is deterministic and collision-resistant ----------

#[test]
fn test_source_id_hashing_deterministic_and_distinct() {
    // Same inputs must produce the same hash (deterministic)
    let id_a = hash_source_id("hackernews", "12345");
    let id_b = hash_source_id("hackernews", "12345");
    assert_eq!(
        id_a, id_b,
        "Same source_type + source_id should yield same hash"
    );

    // Different source_id should produce different hash
    let id_c = hash_source_id("hackernews", "99999");
    assert_ne!(
        id_a, id_c,
        "Different source_ids should produce different hashes"
    );

    // Different source_type with same source_id should produce different hash
    let id_d = hash_source_id("reddit", "12345");
    assert_ne!(
        id_a, id_d,
        "Different source_types should produce different hashes"
    );
}

// ---------- Test 2: process_source_items routes uncached items to embed list ----------

#[test]
fn test_process_source_items_new_items_go_to_embed_list() {
    let db = test_db();
    let mut all_items: Vec<(GenericSourceItem, Vec<f32>)> = Vec::new();
    let mut new_items_to_embed: Vec<(GenericSourceItem, String)> = Vec::new();

    let items = vec![
        sources::SourceItem::new("hackernews", "hn_001", "Rust is great")
            .with_url(Some("https://example.com/rust".to_string()))
            .with_content("Rust offers memory safety without a GC.".to_string()),
        sources::SourceItem::new("hackernews", "hn_002", "TypeScript 6.0 Released")
            .with_content("Major new features in TS 6.".to_string()),
    ];

    process_source_items(
        &db,
        &mut all_items,
        &mut new_items_to_embed,
        items,
        "hackernews",
    );

    // Nothing in DB, so all items should be in the embed list
    assert_eq!(
        all_items.len(),
        0,
        "No cached items should appear in all_items"
    );
    assert_eq!(
        new_items_to_embed.len(),
        2,
        "Both items should need embedding"
    );

    // Verify GenericSourceItem fields
    let (ref item, ref embed_text) = new_items_to_embed[0];
    assert_eq!(item.source_type, "hackernews");
    assert_eq!(item.source_id, "hn_001");
    assert_eq!(item.title, "Rust is great");
    assert_eq!(item.url, Some("https://example.com/rust".to_string()));
    assert_eq!(item.content, "Rust offers memory safety without a GC.");

    // Embedding text should contain both title and content
    assert!(
        embed_text.contains("Rust is great"),
        "Embed text should contain the title"
    );
    assert!(
        embed_text.contains("memory safety"),
        "Embed text should contain the content"
    );
}

// ---------- Test 3: process_source_items assigns correct source_type ----------

#[test]
fn test_process_source_items_tags_with_source_type() {
    let db = test_db();
    let mut all_items: Vec<(GenericSourceItem, Vec<f32>)> = Vec::new();
    let mut embed_list: Vec<(GenericSourceItem, String)> = Vec::new();

    let reddit_items = vec![sources::SourceItem::new(
        "reddit",
        "t3_abc",
        "Show Reddit: My CLI tool",
    )];
    let arxiv_items = vec![sources::SourceItem::new(
        "arxiv",
        "2401.00001",
        "Attention Is Still All You Need",
    )];

    process_source_items(&db, &mut all_items, &mut embed_list, reddit_items, "reddit");
    process_source_items(&db, &mut all_items, &mut embed_list, arxiv_items, "arxiv");

    assert_eq!(embed_list.len(), 2);
    assert_eq!(embed_list[0].0.source_type, "reddit");
    assert_eq!(embed_list[1].0.source_type, "arxiv");

    // IDs should differ because source_type differs
    assert_ne!(embed_list[0].0.id, embed_list[1].0.id);
}

// ---------- Test 4: Fallback zero-vector detection ----------

#[test]
fn test_fallback_zero_vector_detection() {
    // This tests the pattern used to detect fallback embeddings
    let real_embedding = [0.1f32, -0.5, 0.3, 0.0, 0.8];
    let zero_embedding = vec![0.0f32; crate::EMBEDDING_DIMS];
    let empty_embedding: Vec<f32> = vec![];

    let is_fallback_real = real_embedding.iter().all(|&v| v == 0.0);
    let is_fallback_zero = zero_embedding.iter().all(|&v| v == 0.0);
    let is_fallback_empty = empty_embedding.iter().all(|&v| v == 0.0);

    assert!(
        !is_fallback_real,
        "Real embedding should not be detected as fallback"
    );
    assert!(
        is_fallback_zero,
        "All-zeros vector should be detected as fallback"
    );
    assert!(
        is_fallback_empty,
        "Empty vector should satisfy all() vacuously (edge case)"
    );
}

// ---------- Test 5: Retry backoff constants from fetch_with_retry ----------

#[test]
fn test_retry_backoff_delays() {
    use crate::source_fetching::{MAX_RETRY_ATTEMPTS, RETRY_BACKOFF_SECS};

    // fetch_with_retry uses exponential backoff: 1s, 2s, 4s
    assert_eq!(RETRY_BACKOFF_SECS[0], 1, "First retry: 1s");
    assert_eq!(RETRY_BACKOFF_SECS[1], 2, "Second retry: 2s");
    assert_eq!(RETRY_BACKOFF_SECS[2], 4, "Third retry: 4s");
    assert_eq!(MAX_RETRY_ATTEMPTS, 3, "Maximum 3 attempts");

    // Beyond array bounds should fallback to 4
    assert_eq!(
        RETRY_BACKOFF_SECS.get(3).copied().unwrap_or(4),
        4,
        "Out-of-bounds: fallback 4s"
    );
}

// ============================================================================
// Per-source incremental ingestion (2026-08-23 scoring audit)
// ============================================================================

// ---------- One source's embed failure must not lose another source's items ----------

#[test]
fn test_embed_failure_for_one_source_does_not_lose_other_sources_items() {
    let db = test_db();

    // Source A ("hackernews") hits the embed-failure path: ingest_source_batch's
    // Err arm calls persist_pending_rows with the raw batch. The items must land
    // in the embedding-pending retry queue, not vanish.
    let mut counts_a = IngestCounts::default();
    persist_pending_rows(
        &db,
        "hackernews",
        vec![
            pending_row("hackernews", "hn_1", "First failed item"),
            pending_row("hackernews", "hn_2", "Second failed item"),
        ],
        &mut counts_a,
    );
    assert_eq!(counts_a.pending_items, 2, "Both items parked for retry");
    assert_eq!(counts_a.db_errors, 0);

    // Source B ("reddit") ingests normally and must be completely unaffected.
    let mut counts_b = IngestCounts::default();
    persist_source_batch(
        &db,
        "reddit",
        vec![insert_row(
            "reddit",
            "r1",
            "Healthy item",
            seed_embedding("reddit:r1"),
        )],
        Vec::new(),
        &mut counts_b,
    );
    assert_eq!(counts_b.new_items, 1, "Source B's item fully ingested");
    assert_eq!(counts_b.db_errors, 0);

    // Source B's item is complete and visible.
    assert!(
        db.get_source_item("reddit", "r1")
            .expect("query reddit item")
            .is_some(),
        "Source B's item must be fully ingested despite source A's embed failure"
    );

    // Source A's failed batch is queued for re-embedding, not lost.
    let pending = db
        .get_pending_embedding_items(10)
        .expect("query pending items");
    let pending_ids: Vec<&str> = pending.iter().map(|(_, _, sid, _)| sid.as_str()).collect();
    assert!(
        pending_ids.contains(&"hn_1") && pending_ids.contains(&"hn_2"),
        "Source A's items must be in the embedding-pending retry queue, got {pending_ids:?}"
    );
}

// ---------- DB error on the main upsert: counted, and the batch is re-queued ----------

#[test]
fn test_db_error_is_counted_and_batch_requeued_as_pending() {
    let db = test_db();
    // Break the vec-table write path (the failure class that has really
    // happened: see Database::upgrade_pending_to_complete). The batch upsert
    // transaction must fail...
    db.conn
        .lock()
        .execute("DROP TABLE source_vec", [])
        .expect("drop source_vec");

    let mut counts = IngestCounts::default();
    persist_source_batch(
        &db,
        "hackernews",
        vec![insert_row(
            "hackernews",
            "hn_db_err",
            "Item behind a failing vec write",
            seed_embedding("hackernews:hn_db_err"),
        )],
        Vec::new(),
        &mut counts,
    );

    // ...and be SURFACED in the counts, never .ok()-swallowed:
    assert_eq!(counts.db_errors, 1, "DB failure must be counted");
    assert_eq!(
        counts.new_items, 0,
        "A failed upsert must not be counted as new items"
    );
    // ...while the item survives in the pending queue (which does not touch
    // source_vec), so the next cycle retries instead of losing it forever.
    assert_eq!(counts.pending_items, 1, "Failed batch re-queued as pending");
    let pending = db
        .get_pending_embedding_items(10)
        .expect("query pending items");
    assert!(
        pending.iter().any(|(_, _, sid, _)| sid == "hn_db_err"),
        "Item from the failed upsert must be parked in the retry queue"
    );
}

// ---------- DB error on the pending store itself: still counted, not swallowed ----------

#[test]
fn test_pending_store_db_error_is_counted_not_swallowed() {
    let db = test_db();
    db.conn
        .lock()
        .execute("DROP TABLE source_items", [])
        .expect("drop source_items");

    let mut counts = IngestCounts::default();
    persist_pending_rows(
        &db,
        "hackernews",
        vec![pending_row("hackernews", "hn_gone", "Unstorable item")],
        &mut counts,
    );

    assert_eq!(counts.db_errors, 1, "Pending-store failure must be counted");
    assert_eq!(counts.pending_items, 0, "Nothing was actually stored");
}

// ---------- Zero-embedding items are parked as pending, except osv/cve ----------

#[test]
fn test_partition_zero_embedding_routes_to_pending_except_security() {
    let zero = vec![0.0f32; 8];
    let real = vec![0.1f32, -0.5, 0.3, 0.0, 0.8, 0.2, 0.1, 0.4];

    let (insert_rows, pending_rows) = partition_embedded(
        vec![
            prepared_item("hackernews", "hn_zero", "Zero-embedded story"),
            prepared_item("osv", "GHSA-xxxx", "[GHSA-xxxx] pkg: advisory"),
            prepared_item("reddit", "r_real", "Properly embedded post"),
        ],
        vec![zero.clone(), zero, real],
    );

    // The zero-embedded non-security item is parked for retry, NOT dropped.
    assert_eq!(pending_rows.len(), 1);
    assert_eq!(pending_rows[0].0, "hackernews");
    assert_eq!(pending_rows[0].1, "hn_zero");
    assert_eq!(
        pending_rows[0].5, "Zero-embedded story embed text",
        "Pending row must carry the embed text for the repair loop"
    );

    // The security advisory keeps its zero vector (version-grounded), and the
    // properly embedded item inserts normally.
    assert_eq!(insert_rows.len(), 2);
    assert_eq!(insert_rows[0].0, "osv");
    assert!(
        insert_rows[0].5.iter().all(|&v| v == 0.0),
        "osv advisory retains its zero vector"
    );
    assert_eq!(insert_rows[1].0, "reddit");
}

// ---------- Embedder returning fewer vectors than texts must not drop the tail ----------

#[test]
fn test_partition_embedding_shortfall_routes_tail_to_pending() {
    let (insert_rows, pending_rows) = partition_embedded(
        vec![
            prepared_item("hackernews", "hn_a", "Embedded item"),
            prepared_item("hackernews", "hn_b", "Item the embedder never returned"),
        ],
        vec![vec![0.2f32, 0.4, 0.6]],
    );

    assert_eq!(insert_rows.len(), 1);
    assert_eq!(insert_rows[0].1, "hn_a");
    assert_eq!(
        pending_rows.len(),
        1,
        "Shortfall tail parked, not truncated"
    );
    assert_eq!(pending_rows[0].1, "hn_b");
}

// ---------- Language filter: foreign items dropped, security sources exempt ----------

#[test]
fn test_prepare_source_batch_filters_foreign_language_keeps_security() {
    let raw = |st: &str, sid: &str, title: &str, content: &str| -> RawNewItem {
        (
            st.to_string(),
            sid.to_string(),
            None,
            title.to_string(),
            content.to_string(),
            None,
            None,
            None,
        )
    };

    // English user, no auto-translate: the CJK-titled story is dropped, the
    // English one is kept with HTML entities decoded into title and embed text.
    let prepared = prepare_source_batch(
        "hackernews",
        vec![
            raw(
                "hackernews",
                "en1",
                "Rust &amp; Cargo ship a new release",
                "A detailed English write-up about the Rust toolchain.",
            ),
            raw(
                "hackernews",
                "jp1",
                "\u{65e5}\u{672c}\u{8a9e}\u{306e}\u{30bf}\u{30a4}\u{30c8}\u{30eb}\u{3067}\u{3059}",
                "\u{3053}\u{308c}\u{306f}\u{65e5}\u{672c}\u{8a9e}\u{306e}\u{8a18}\u{4e8b}\u{3067}\u{3059}\u{3002}",
            ),
        ],
        "en",
        false,
    );
    assert_eq!(prepared.len(), 1, "Foreign-language item filtered out");
    let ((_, sid, _, title, _, _, _, _, _), embed_text) = &prepared[0];
    assert_eq!(sid, "en1");
    assert_eq!(
        title, "Rust & Cargo ship a new release",
        "HTML entities decoded at ingestion"
    );
    assert!(embed_text.contains("Rust & Cargo"));

    // Security sources are never language-filtered, whatever the title looks like.
    let prepared_osv = prepare_source_batch(
        "osv",
        vec![raw(
            "osv",
            "OSV-2026-1",
            "[OSV-2026-1] pkg: \u{8106}\u{5f31}\u{6027}",
            "Advisory body.",
        )],
        "en",
        false,
    );
    assert_eq!(
        prepared_osv.len(),
        1,
        "osv/cve items bypass the language filter"
    );
}
