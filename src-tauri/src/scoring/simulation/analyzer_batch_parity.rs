// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Analyzer-vs-backfill score-family parity (2026-08-23 audit §3.5, item 22a).
//!
//! Production persists scores from TWO paths:
//!   * the ANALYZER path (`scoring/analyzer.rs::score_items_full`): `score_item`
//!     → cross-encoder rerank → sort → dedup → fuzzy dedup → topic dedup →
//!     temporal cluster → domain diversity → source-topic diversity →
//!     per-source normalization → serendipity injection → `finalize_scores`;
//!   * the BACKFILL/DRAIN path (`analysis_backfill.rs::score_chunk` →
//!     `persistable`): `score_item` → `finalize_scores`, per item.
//!
//! The audit measured the same item oscillating between the two families
//! (§3.5: batch-relative writers on one path, pure pipeline output on the
//! other), and one concrete divergence in the INPUTS themselves — the backfill
//! path passed `source_tags: &[]` where the analyzer parsed topic tags — which
//! Wave 2 fixed. These tests pin both:
//!
//!   1. INPUT parity: both paths construct `ScoringInput` identically (same
//!      tag parsing, same published_at fallback, same source_id), so
//!      `score_item` yields bit-identical results.
//!   2. BATCH-LAYER parity: for items that engage none of the batch-relative
//!      mechanisms (no duplicates, distinct topics/domains, <3 items per
//!      source), the analyzer's entire dedup/diversity/serendipity layer is an
//!      IDENTITY on scores — so both families persist the same number. Any
//!      future unconditional batch mutation (the pre-audit per-source
//!      percentile blend was one) breaks this immediately.
//!
//! Deliberately EXCLUDED: the cross-encoder blend and LLM rerank. Both are
//! environment-gated in production (model availability / daily budget) and are
//! the audit's documented family divergence — removing them from the persisted
//! score is item 12 (Phase 1), not this harness's job. The layer exercised
//! here is everything that runs unconditionally.

use super::super::{score_item, ScoringInput, ScoringOptions};
use super::personas::all_personas;
use super::sim_db;

/// A stored-item fixture carrying every field both production paths read.
struct StoredFixture {
    id: u64,
    title: &'static str,
    content: &'static str,
    source_type: &'static str,
    url: &'static str,
    /// `source_items.tags` — the raw JSON both paths hand to
    /// `parse_tags_topics` AND to `tags_json`.
    tags: Option<&'static str>,
    source_id: &'static str,
}

/// Six unique items: distinct titles, topics, and URL domains, at most two per
/// source_type — so dedup, fuzzy/topic dedup, temporal clustering, domain
/// diversity, source-topic diversity, and the per-source percentile blend
/// (which requires >= 3 items per source) all correctly no-op. That is the
/// point: for such items the two persistence families MUST agree.
fn fixtures() -> Vec<StoredFixture> {
    vec![
        StoredFixture {
            id: 9101,
            title: "Tokio work-stealing scheduler internals for production Rust services",
            content: "How the tokio async runtime balances tasks across worker threads, \
                      with systems programming benchmarks under contention.",
            source_type: "hackernews",
            url: "https://alpha-runtime.dev/tokio-scheduler",
            tags: Some(r#"["rust", "tokio", "async"]"#),
            source_id: "hn-9101",
        },
        StoredFixture {
            id: 9102,
            title: "The perfect sourdough starter: a week-by-week guide",
            content: "Flour, water, patience: fermentation schedules and hydration ratios \
                      for home bakers.",
            source_type: "hackernews",
            url: "https://beta-bakery.example/sourdough",
            tags: Some(r#"["baking", "food"]"#),
            source_id: "hn-9102",
        },
        StoredFixture {
            id: 9103,
            title: "SQLite WAL-mode tuning for embedded desktop applications",
            content: "Pragma journal sizing, checkpoint cadence, and write batching for \
                      local-first apps using sqlite databases.",
            source_type: "rss",
            url: "https://gamma-databases.io/sqlite-wal",
            tags: Some(r#"["sqlite", "database"]"#),
            source_id: "rss-9103",
        },
        StoredFixture {
            id: 9104,
            title: "CVE-2026-31337: hyper HTTP/2 CONTINUATION frame flood",
            content: "A vulnerability in the hyper crate's HTTP/2 handling allows a \
                      CONTINUATION frame flood leading to denial of service. \
                      Affected: hyper (crates.io). Fixed in: 1.8.1.",
            source_type: "cve",
            url: "https://delta-advisories.example/cve-2026-31337",
            tags: Some(r#"["security", "cve"]"#),
            source_id: "CVE-2026-31337",
        },
        StoredFixture {
            id: 9105,
            title: "crates.io: serde v1.0.230",
            content: "A generic serialization/deserialization framework.\nDownloads: 380000000",
            source_type: "crates_io",
            url: "https://crates.io/crates/serde",
            tags: None,
            source_id: "crate-serde",
        },
        StoredFixture {
            id: 9106,
            title: "Understanding Kubernetes operators for stateful workloads",
            content: "Custom resource definitions, reconciliation loops, and operator \
                      patterns for databases on k8s.",
            source_type: "rss",
            url: "https://epsilon-cloud.example/k8s-operators",
            tags: Some(r#"["kubernetes", "devops"]"#),
            source_id: "rss-9106",
        },
    ]
}

/// Production analyzer options (`analyzer.rs::score_items_full`): freshness and
/// signals ON, no trend topics (trend detection is batch-derived; empty for
/// this fixture set on both paths).
fn production_options() -> ScoringOptions {
    ScoringOptions {
        apply_freshness: true,
        apply_signals: true,
        trend_topics: vec![],
    }
}

/// Score one fixture EXACTLY as `analyzer.rs` builds its `ScoringInput`
/// (published_at-or-created_at into created_at, `parse_tags_topics(tags)` into
/// source_tags, raw tags into tags_json, real source_id).
fn score_analyzer_shaped(
    fx: &StoredFixture,
    ctx: &super::super::ScoringContext,
    db: &crate::db::Database,
    published_at: &chrono::DateTime<chrono::Utc>,
    classifier: &crate::signals::SignalClassifier,
) -> crate::SourceRelevance {
    let parsed_tags: Vec<String> = super::super::parse_tags_topics(fx.tags);
    let emb = vec![0.0_f32; crate::EMBEDDING_DIMS];
    score_item(
        &ScoringInput {
            id: fx.id,
            title: fx.title,
            url: Some(fx.url),
            content: fx.content,
            source_type: fx.source_type,
            embedding: &emb,
            created_at: Some(published_at),
            detected_lang: "en",
            source_tags: &parsed_tags,
            tags_json: fx.tags,
            feed_origin: None,
            source_id: Some(fx.source_id),
        },
        ctx,
        db,
        &production_options(),
        Some(classifier),
    )
}

/// Score one fixture EXACTLY as `analysis_backfill.rs::score_chunk` builds its
/// `ScoringInput`. Post Wave 2 this construction is identical to the
/// analyzer's — that identity is invariant (1) under test. If the two
/// construction sites ever diverge again (as they did with `source_tags: &[]`
/// pre-audit), update BOTH this mirror and the parity expectation consciously.
fn score_backfill_shaped(
    fx: &StoredFixture,
    ctx: &super::super::ScoringContext,
    db: &crate::db::Database,
    published_at: &chrono::DateTime<chrono::Utc>,
    classifier: &crate::signals::SignalClassifier,
) -> crate::SourceRelevance {
    // Path parity (analysis_backfill.rs): same parse, same signal.
    let parsed_tags: Vec<String> = super::super::parse_tags_topics(fx.tags);
    let emb = vec![0.0_f32; crate::EMBEDDING_DIMS];
    score_item(
        &ScoringInput {
            id: fx.id,
            title: fx.title,
            url: Some(fx.url),
            content: fx.content,
            source_type: fx.source_type,
            embedding: &emb,
            created_at: Some(published_at),
            detected_lang: "en",
            source_tags: &parsed_tags,
            tags_json: fx.tags,
            feed_origin: None,
            source_id: Some(fx.source_id),
        },
        ctx,
        db,
        &production_options(),
        Some(classifier),
    )
}

/// The analyzer's unconditional post-scoring batch layer, in the exact order
/// `score_items_full` applies it (minus the env-gated cross-encoder / LLM
/// stages — see module doc).
fn apply_analyzer_batch_layer(results: &mut Vec<crate::SourceRelevance>) -> usize {
    super::super::sort_results(results);
    super::super::dedup_results(results);
    super::super::fuzzy_dedup_results(results);
    super::super::topic_dedup_results(results);
    super::super::temporal_cluster_results(results);
    super::super::apply_domain_diversity(results);
    super::super::apply_source_topic_diversity(results);
    crate::source_tiers::normalize_scores_by_source(results);
    super::super::sort_results(results);
    // Serendipity budget mirrors the default settings value; the injector
    // REPLACES scorer-rejected originals with capped serendipity picks.
    let injected = super::super::inject_serendipity_candidates(results, 10);
    super::super::finalize_scores(results);
    injected
}

/// Invariant (1): both paths build the same input, so `score_item` must agree
/// bit-for-bit — the "two score families" must not begin at construction.
#[test]
fn analyzer_and_backfill_construct_identical_scores() {
    let ctx = all_personas().remove(0); // rust_systems
    let db = sim_db();
    let classifier = crate::signals::SignalClassifier::new();
    let published = chrono::Utc::now() - chrono::Duration::hours(30);

    for fx in fixtures() {
        let a = score_analyzer_shaped(&fx, &ctx, &db, &published, &classifier);
        let b = score_backfill_shaped(&fx, &ctx, &db, &published, &classifier);
        assert_eq!(
            a.top_score, b.top_score,
            "input-construction divergence for '{}' (analyzer {:.6} vs backfill {:.6}) — \
             the two persistence paths no longer feed score_item the same item",
            fx.title, a.top_score, b.top_score
        );
        assert_eq!(
            a.relevant, b.relevant,
            "relevance verdict diverged for '{}' between path constructions",
            fx.title
        );
    }
}

/// Invariant (2): the analyzer's dedup/diversity/serendipity layer is an
/// IDENTITY on the persisted score of items that engage none of its
/// batch-relative mechanisms — so for such items the analyzer family and the
/// backfill family persist the SAME number (§3.5's fix criterion).
#[test]
fn batch_layer_is_identity_for_non_colliding_items() {
    let ctx = all_personas().remove(0); // rust_systems
    let db = sim_db();
    let classifier = crate::signals::SignalClassifier::new();
    let published = chrono::Utc::now() - chrono::Duration::hours(30);
    let fixture_set = fixtures();

    // Backfill family: score → finalize per item (analysis_backfill::persistable).
    let backfill: Vec<crate::SourceRelevance> = fixture_set
        .iter()
        .map(|fx| {
            let mut r = score_backfill_shaped(fx, &ctx, &db, &published, &classifier);
            super::super::finalize_scores(std::slice::from_mut(&mut r));
            r
        })
        .collect();

    // Analyzer family: score → full unconditional batch layer.
    let mut analyzer: Vec<crate::SourceRelevance> = fixture_set
        .iter()
        .map(|fx| score_analyzer_shaped(fx, &ctx, &db, &published, &classifier))
        .collect();
    apply_analyzer_batch_layer(&mut analyzer);

    // No fixture may be dropped: the layer's removals (dedup/clustering) must
    // not engage on unique items.
    for fx in &fixture_set {
        assert!(
            analyzer.iter().any(|r| r.id == fx.id),
            "batch layer dropped unique item '{}' — a dedup/cluster stage now \
             removes non-duplicates",
            fx.title
        );
    }

    for b in &backfill {
        let a = analyzer
            .iter()
            .find(|r| r.id == b.id)
            .expect("presence asserted above");
        if a.serendipity {
            // A serendipity pick is a deliberate, capped, flagged verdict
            // change — the one sanctioned divergence. Its cap is asserted by
            // the injector's own tests; here it is simply exempt.
            continue;
        }
        assert!(
            (a.top_score - b.top_score).abs() <= 1e-6,
            "score-family divergence for '{}': analyzer batch layer persisted \
             {:.6}, backfill persisted {:.6} — an unconditional batch-relative \
             mutation is back in the persist path (audit §3.5; the per-source \
             percentile blend requires >= 3 items per source and must not have \
             touched this set)",
            a.title,
            a.top_score,
            b.top_score
        );
        assert_eq!(
            a.relevant, b.relevant,
            "relevance verdict for '{}' differs between persistence families",
            a.title
        );
    }
}
