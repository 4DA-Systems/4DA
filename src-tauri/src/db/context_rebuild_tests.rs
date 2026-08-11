// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the atomic grounding-corpus rebuild ([`Database::rebuild_contexts`])
//! and the collapse-detection baseline plumbing.
//!
//! Why this exists (2026-07-15): the previous indexing path cleared the corpus
//! FIRST and re-embedded for ~10 minutes into the emptiness. A scheduled engine
//! cycle scored 701 items against the empty table (relevant collapsed 24 -> 3)
//! and the health check blessed the wipe as "healthy total=0". These tests pin
//! the three guarantees that close that class: (1) the swap is atomic and the
//! old corpus survives a refused/unusable replacement, (2) the rebuild cannot
//! bypass the admission chokepoint, (3) a collapse is detected against the
//! last sound baseline.

use super::NewContextChunk;
use crate::context_admission::{CORPUS_BASELINE_KV_KEY, MAX_DOC_CHUNKS_PER_SOURCE};
use crate::test_utils::{seed_embedding, test_db};

fn chunk(source_file: &str, text: &str) -> NewContextChunk {
    NewContextChunk {
        source_file: source_file.to_string(),
        text: text.to_string(),
        embedding: seed_embedding(text),
        weight: 1.0,
    }
}

#[test]
fn rebuild_replaces_previous_corpus_atomically() {
    let db = test_db();
    for i in 0..3 {
        db.upsert_context(
            &format!("old/file_{i}.rs"),
            &format!("old chunk {i}"),
            &seed_embedding(&format!("old {i}")),
        )
        .unwrap();
    }
    assert_eq!(db.context_count().unwrap(), 3);

    let entries = vec![
        chunk("new/alpha.rs", "fn alpha() {}"),
        chunk("new/beta.rs", "fn beta() {}"),
    ];
    let stats = db.rebuild_contexts(&entries).unwrap();
    assert_eq!(stats.refused, None);
    assert_eq!(stats.previous_count, 3);
    assert_eq!(stats.attempted, 2);
    assert_eq!(stats.admitted, 2);

    let contexts = db.get_all_contexts().unwrap();
    assert_eq!(contexts.len(), 2);
    assert!(
        contexts.iter().all(|c| c.source_file.starts_with("new/")),
        "old corpus must be fully replaced: {:?}",
        contexts.iter().map(|c| &c.source_file).collect::<Vec<_>>()
    );
}

#[test]
fn rebuild_enforces_the_admission_chokepoint() {
    let db = test_db();
    let mut entries = vec![
        chunk("src/main.rs", "fn main() {}"),
        // Empty source name -> Reject class -> dropped.
        chunk("", "rejected provenance chunk"),
        // Exact duplicate text of the first -> content-hash dedupe.
        chunk("src/other.rs", "fn main() {}"),
    ];
    // One doc file contributing far beyond the per-source cap.
    for i in 0..(MAX_DOC_CHUNKS_PER_SOURCE + 5) {
        entries.push(chunk(
            "docs/course.md",
            &format!("course prose paragraph {i}"),
        ));
    }

    let stats = db.rebuild_contexts(&entries).unwrap();
    assert_eq!(stats.refused, None);
    assert_eq!(stats.skipped_reject, 1, "empty-name chunk must be rejected");
    assert_eq!(stats.deduped, 1, "duplicate text must dedupe");
    assert_eq!(
        stats.skipped_doc_cap, 5,
        "doc chunks beyond the per-source cap must be dropped"
    );
    assert_eq!(stats.admitted, 1 + MAX_DOC_CHUNKS_PER_SOURCE);

    // Verify persisted classes and the doc weight multiplier survived the swap.
    let conn = db.read_conn();
    let code_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM context_chunks WHERE source_type = 'code'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let doc_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM context_chunks WHERE source_type = 'doc'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let doc_weight: f64 = conn
        .query_row(
            "SELECT weight FROM context_chunks WHERE source_type = 'doc' LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(code_count, 1);
    assert_eq!(doc_count, MAX_DOC_CHUNKS_PER_SOURCE as i64);
    assert!(
        (doc_weight - 0.5).abs() < 1e-6,
        "doc admission weight multiplier must apply, got {doc_weight}"
    );
}

#[test]
fn rebuild_refuses_an_empty_entry_set_and_preserves_the_corpus() {
    let db = test_db();
    db.upsert_context("keep/me.rs", "fn keep() {}", &seed_embedding("keep"))
        .unwrap();

    let stats = db.rebuild_contexts(&[]).unwrap();
    assert_eq!(stats.refused, Some("empty-entry-set"));
    assert_eq!(stats.previous_count, 1);
    assert_eq!(
        db.context_count().unwrap(),
        1,
        "an empty replacement set must never wipe the corpus"
    );
}

#[test]
fn rebuild_refuses_a_fully_inadmissible_set_and_preserves_the_corpus() {
    let db = test_db();
    db.upsert_context("keep/me.rs", "fn keep() {}", &seed_embedding("keep"))
        .unwrap();

    // Every entry Reject-classed -> zero admitted -> rollback, corpus intact.
    let entries = vec![chunk("", "junk one"), chunk("", "junk two")];
    let stats = db.rebuild_contexts(&entries).unwrap();
    assert_eq!(stats.refused, Some("zero-admitted"));
    assert_eq!(stats.admitted, 0);
    assert_eq!(
        db.context_count().unwrap(),
        1,
        "a fully-inadmissible replacement must never wipe the corpus"
    );
    let survivors = db.get_all_contexts().unwrap();
    assert_eq!(survivors[0].source_file, "keep/me.rs");
}

#[test]
fn rebuilt_corpus_serves_knn_grounding_reads() {
    let db = test_db();
    let entries = vec![
        chunk("src/query.rs", "async fn run_query(pool: &Pool) {}"),
        chunk("docs/readme.md", "installation instructions prose"),
    ];
    db.rebuild_contexts(&entries).unwrap();

    // The vec-table shadow rows must exist and grounding must be code-only.
    let results = db
        .find_similar_contexts(&seed_embedding("async fn run_query(pool: &Pool) {}"), 5)
        .unwrap();
    assert!(
        !results.is_empty(),
        "rebuilt corpus must be KNN-searchable (context_vec rows written)"
    );
    assert!(
        results.iter().all(|r| r.source_file.ends_with(".rs")),
        "grounding reads must stay code-only after a rebuild: {:?}",
        results.iter().map(|r| &r.source_file).collect::<Vec<_>>()
    );
}

#[test]
fn kv_roundtrip_and_corpus_baseline_recording() {
    let db = test_db();
    assert_eq!(db.get_kv("no_such_key").unwrap(), None);
    db.set_kv("some_key", "v1").unwrap();
    assert_eq!(db.get_kv("some_key").unwrap().as_deref(), Some("v1"));
    db.set_kv("some_key", "v2").unwrap();
    assert_eq!(db.get_kv("some_key").unwrap().as_deref(), Some("v2"));

    // Regression: the installed-base kv_store has REAL column affinity (ACE
    // schema), so the flag string '2' was stored as REAL 2.0 and the old
    // String read failed -> None -> the "one-time" corpus-wiping rebuild
    // re-ran every boot. get_kv must normalize a REAL back to its integer
    // string so version flags and numeric baselines round-trip.
    {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value) VALUES ('real_flag', 2.0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value) VALUES ('real_baseline', 24113.0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value) VALUES ('int_flag', 1)",
            [],
        )
        .unwrap();
    }
    assert_eq!(db.get_kv("real_flag").unwrap().as_deref(), Some("2"));
    assert_eq!(
        db.get_kv("real_baseline")
            .unwrap()
            .and_then(|v| v.parse::<usize>().ok()),
        Some(24113)
    );
    assert_eq!(db.get_kv("int_flag").unwrap().as_deref(), Some("1"));

    // A sound (healthy + grounded) corpus records its size as the baseline.
    db.upsert_context("src/a.rs", "fn a() {}", &seed_embedding("a"))
        .unwrap();
    db.upsert_context("src/b.rs", "fn b() {}", &seed_embedding("b"))
        .unwrap();
    let health = db.context_health().unwrap();
    assert!(health.healthy);
    assert_eq!(health.grounding_chunks, 2);
    db.record_corpus_baseline(&health).unwrap();
    assert_eq!(
        db.get_kv(CORPUS_BASELINE_KV_KEY).unwrap().as_deref(),
        Some("2")
    );
}

#[test]
fn context_health_detects_a_collapse_against_the_recorded_baseline() {
    let db = test_db();
    // Simulate the 2026-07-15 shape: the last sound corpus was 24,113 chunks;
    // the table now holds a 126-chunk partial rebuild.
    db.set_kv(CORPUS_BASELINE_KV_KEY, "24113").unwrap();
    for i in 0..5 {
        db.upsert_context(
            &format!("README.md#Section{i}"),
            &format!("readme section {i}"),
            &seed_embedding(&format!("s{i}")),
        )
        .unwrap();
    }
    let health = db.context_health().unwrap();
    assert!(health.collapsed, "issues: {:?}", health.issues);
    assert!(!health.healthy);
    assert!(health.issues.iter().any(|i| i.contains("COLLAPSED")));
}

// ── Test-code grounding exclusion (2026-07-21 fixture-grounding audit) ──────

#[test]
fn upsert_demotes_test_files_and_marker_chunks_to_test_code() {
    let db = test_db();
    // Test-named file: demoted by path signal.
    db.upsert_context(
        "version-resolution.test.ts",
        "const resolved = resolve(dir);",
        &seed_embedding("t1"),
    )
    .unwrap();
    // Prod-named file, test-marker chunk: demoted by content signal (the
    // live `context.rs` #[cfg(test)] fixture class).
    db.upsert_context(
        "context.rs",
        "#[cfg(test)]\nmod tests { // Content carefully chosen }",
        &seed_embedding("t2"),
    )
    .unwrap();
    // Prod chunk stays code.
    db.upsert_context(
        "hardware_detect.rs",
        "let gpu = detect_gpu();",
        &seed_embedding("t3"),
    )
    .unwrap();

    let conn = db.conn.lock();
    let type_of = |file: &str| -> String {
        conn.query_row(
            "SELECT source_type FROM context_chunks WHERE source_file = ?1",
            rusqlite::params![file],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(type_of("version-resolution.test.ts"), "test_code");
    assert_eq!(type_of("context.rs"), "test_code");
    assert_eq!(type_of("hardware_detect.rs"), "code");
}

#[test]
fn grounding_reads_exclude_test_code() {
    let db = test_db();
    let text = "async fn run_query(pool: &Pool) {}";
    // Same text in a prod file and (different hash via suffix) a test file.
    db.upsert_context("src/query.rs", text, &seed_embedding(text))
        .unwrap();
    db.upsert_context(
        "src/query.test.ts",
        &format!("{text} // fixture copy"),
        &seed_embedding(text),
    )
    .unwrap();
    let results = db.find_similar_contexts(&seed_embedding(text), 5).unwrap();
    assert!(
        results.iter().any(|r| r.source_file == "src/query.rs"),
        "prod chunk grounds"
    );
    assert!(
        !results.iter().any(|r| r.source_file == "src/query.test.ts"),
        "test chunk must never ground: {:?}",
        results.iter().map(|r| &r.source_file).collect::<Vec<_>>()
    );
}

#[test]
fn reconcile_demotes_legacy_test_chunks_including_file_majority() {
    let db = test_db();
    // Simulate the installed base: rows persisted as 'code' before the
    // TestCode class existed. `stack_simulation.rs` is the live shape — a
    // tests/-dir file stored as a BARE basename whose fixture chunks carry
    // no marker; the file-majority rule must demote those too.
    {
        let conn = db.conn.lock();
        let insert = |file: &str, text: &str| {
            let emb = super::embedding_to_blob(&seed_embedding(text));
            conn.execute(
                "INSERT INTO context_chunks (source_file, content_hash, text, embedding, weight, source_type)
                 VALUES (?1, ?2, ?3, ?4, 1.0, 'code')",
                rusqlite::params![file, super::hash_content(text), text, emb],
            )
            .unwrap();
        };
        // 3 marker chunks + 1 bare fixture chunk = 75% majority.
        insert("stack_simulation.rs", "#[tokio::test]\nasync fn a() {}");
        insert("stack_simulation.rs", "#[test]\nfn b() {}");
        insert("stack_simulation.rs", "#[cfg(test)]\nmod harness {}");
        insert(
            "stack_simulation.rs",
            "// rust: must not match trust, frustrated, rustic, crusty\nlet rust = 1;",
        );
        // Prod file with one inline test chunk: only that chunk demotes.
        insert("scanner.rs", "fn scan(dir: &Path) {}");
        insert("scanner.rs", "fn walk(dir: &Path) {}");
        insert("scanner.rs", "fn parse(dir: &Path) {}");
        insert("scanner.rs", "#[cfg(test)]\nmod tests { use super::*; }");
    }
    db.reconcile_context_provenance().unwrap();
    let conn = db.conn.lock();
    let count = |file: &str, st: &str| -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM context_chunks WHERE source_file = ?1 AND source_type = ?2",
            rusqlite::params![file, st],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(
        count("stack_simulation.rs", "test_code"),
        4,
        "file-majority demotes the markerless fixture chunk too"
    );
    assert_eq!(count("scanner.rs", "code"), 3, "prod chunks keep grounding");
    assert_eq!(
        count("scanner.rs", "test_code"),
        1,
        "the inline test module chunk is demoted"
    );
}
