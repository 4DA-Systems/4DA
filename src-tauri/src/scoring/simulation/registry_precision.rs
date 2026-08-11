// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Registry-release & federated-social precision fixtures.
//!
//! Production-derived failure modes (signal-precision investigation,
//! 2026-07-23, live-DB §2 evidence — see
//! `.claude/plans/signal-precision-investigation-handoff.md` in the repo
//! plans): the live feed was ~81% crates_io releases, with **non-dependency
//! look-alike crates** (`forge-plugin-sdk-rust` 0.947, `axum-connect-rpc`
//! 0.926, `serde_v8` 0.700 — all `dep_links=0`) out-scoring the user's real
//! dependency releases, and federated social noise (mastodon/lemmy promo
//! posts at 0.9+) riding the neutral community-signal arm into the feed.
//!
//! The main 9-persona corpus cannot see these classes: `sim_input` hardcodes
//! `source_type: "hackernews"`, so no registry or UGC-cap path is ever
//! exercised there. This module scores production-shaped registry/social
//! inputs (real `source_type`, real `source_id` subject encoding, aged
//! `created_at`) through the full `score_item` pipeline with the enriched
//! rust_systems persona (which declares tokio/serde/sqlx as direct deps).
//!
//! Every item deliberately carries the persona's own interest embedding —
//! maximal embedding similarity — because that IS the production trap:
//! relevance driven by embedding look-alikeness with zero dependency
//! grounding. The only discriminator left is the grounding itself.

use chrono::{Duration, Utc};

use super::super::{score_item, ScoringContext, ScoringInput};
use super::enrichment::{enrich_persona, EnrichmentConfig};
use super::persona_data::all_enrichments;
use super::personas::all_personas;
use super::{sim_db, sim_no_freshness};

/// Enriched rust_systems persona: base persona 0 + full enrichment
/// (dependency_info: tokio/serde/sqlx as direct rust deps).
fn enriched_rust_persona() -> ScoringContext {
    let base = all_personas().remove(0);
    let enrichments = all_enrichments();
    enrich_persona(base, &enrichments[0], &EnrichmentConfig::all())
}

/// A registry-release fixture shaped exactly like a stored crates_io item:
/// `source_id = crate-{name}`, `title = "crates.io: {name} v{ver}"`.
struct RegistryFixture {
    name: &'static str,
    version: &'static str,
    description: &'static str,
}

const LOOKALIKE_CRATES: &[RegistryFixture] = &[
    // §2.2 top-scored non-dep, 0.947 in production, dep_links=0.
    RegistryFixture {
        name: "forge-plugin-sdk-rust",
        version: "1.0.7",
        description: "Rust SDK for building Forge plugins with async support",
    },
    // Look-alike embedding a real dep name as a compound ("serde_v8").
    RegistryFixture {
        name: "serde_v8",
        version: "0.311.0",
        description: "V8 serialization and deserialization for the deno runtime",
    },
    // Compound look-alike on another real dep (production 0.492).
    RegistryFixture {
        name: "cobol_rust_serde",
        version: "0.1.0",
        description: "COBOL data layout serialization helpers",
    },
    // The phantom-text-grounding trap: subject is NOT a dependency, but the
    // description name-drops the user's real deps standalone. Text-derived
    // dep confidence must not count as grounding for a registry release.
    RegistryFixture {
        name: "sonar-flows-core",
        version: "0.3.1",
        description: "Flow orchestration core built on tokio and serde for streaming pipelines",
    },
];

const REAL_DEP_CRATES: &[RegistryFixture] = &[
    // The user's actual direct dependencies (§2.2 ✅ rows) — the product's
    // best signal; these MUST stay relevant (recall guard).
    RegistryFixture {
        name: "tokio",
        version: "1.52.3",
        description:
            "An event-driven, non-blocking I/O platform for writing asynchronous applications",
    },
    RegistryFixture {
        name: "serde",
        version: "1.0.228",
        description: "A generic serialization/deserialization framework",
    },
    RegistryFixture {
        name: "sqlx",
        version: "0.9.0-alpha.1",
        description: "An async, pure Rust SQL toolkit with compile-time checked queries",
    },
];

fn score_registry_fixture(
    fx: &RegistryFixture,
    ctx: &ScoringContext,
    id: u64,
) -> crate::SourceRelevance {
    let db = sim_db();
    let opts = sim_no_freshness();
    // Persona's own interest embedding — maximal similarity (the trap).
    let emb = super::domain_embeddings::interest_embedding(0);
    let title = format!("crates.io: {} v{}", fx.name, fx.version);
    let source_id = format!("crate-{}", fx.name);
    // Old enough that the community-signal fresh-item grace does not apply.
    let created = Utc::now() - Duration::hours(8);
    let input = ScoringInput {
        id,
        title: &title,
        url: Some("https://crates.io"),
        content: fx.description,
        source_type: "crates_io",
        embedding: &emb,
        created_at: Some(&created),
        detected_lang: "en",
        source_tags: &[],
        tags_json: None,
        feed_origin: None,
        source_id: Some(&source_id),
    };
    score_item(&input, ctx, &db, &opts, None)
}

/// Diagnostic (run with `--nocapture`): dump the axis breakdown of every
/// registry fixture so a gate/boost leak is visible instead of guessed at.
/// Measurement only — never asserts.
#[test]
fn diagnose_registry_fixture_breakdown() {
    let ctx = enriched_rust_persona();
    println!("\n=== registry fixture breakdown ===");
    for (label, set) in [
        ("LOOKALIKE", LOOKALIKE_CRATES),
        ("REAL_DEP", REAL_DEP_CRATES),
    ] {
        for (i, fx) in set.iter().enumerate() {
            let r = score_registry_fixture(fx, &ctx, 10_901 + i as u64);
            let bd = r.score_breakdown.as_ref();
            println!(
                "  [{label:<9}] {:<24} score={:.3} rel={} sig={} [{}] ctx={:.2} int={:.2} kw={:.2} ace={:.2} dep={:.2} dna={:.2} intent={:.2} stack={:.2}",
                fx.name,
                r.top_score,
                r.relevant,
                bd.map(|b| b.signal_count).unwrap_or(0),
                bd.map(|b| b.confirmed_signals.join("+")).unwrap_or_default(),
                bd.map(|b| b.context_score).unwrap_or(0.0),
                bd.map(|b| b.interest_score).unwrap_or(0.0),
                bd.map(|b| b.keyword_score).unwrap_or(0.0),
                bd.map(|b| b.ace_boost).unwrap_or(0.0),
                bd.map(|b| b.dep_match_score).unwrap_or(0.0),
                bd.map(|b| b.content_dna_mult).unwrap_or(0.0),
                bd.map(|b| b.intent_boost).unwrap_or(0.0),
                bd.map(|b| b.stack_boost).unwrap_or(0.0),
            );
        }
    }
    println!("=== end registry breakdown ===\n");
}

/// Look-alike (non-dependency) registry releases must NOT be feed-relevant,
/// no matter how stack-shaped their names/descriptions are. Production
/// evidence: these exact classes scored 0.49–0.95 and flooded the feed.
#[test]
fn lookalike_registry_releases_not_relevant() {
    let ctx = enriched_rust_persona();
    let mut failures = Vec::new();
    for (i, fx) in LOOKALIKE_CRATES.iter().enumerate() {
        let r = score_registry_fixture(fx, &ctx, 10_001 + i as u64);
        if r.relevant {
            failures.push(format!(
                "  {} v{} scored RELEVANT ({:.3}) with zero dependency grounding",
                fx.name, fx.version, r.top_score
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "non-dependency registry releases marked relevant:\n{}",
        failures.join("\n")
    );
}

/// v18 regression (verdict) + v19 guarantee (score).
///
/// History: the +0.05 topic-attention-gap boost and `normalize_score_offset`
/// (+0.02) ran AFTER `apply_final_adjustments`, so a 0.35-capped look-alike
/// landed at 0.42 — over the 0.40 threshold (live corpus 2026-07-26: 84
/// crates_io items at exactly 0.42, 26 already `feed_relevant = 1`). v18
/// made the VERDICT categorical; v19 removed the attention-gap boost
/// entirely (AD-029) and added `score_ceiling` re-assertion in
/// `finalize_scores` so post-pipeline writers (cross-encoder, dedup boost,
/// source-tier normalize, LLM reconciler) cannot re-inflate a capped item's
/// SCORE either.
///
/// This test asserts both invariants end-to-end: a look-alike is never
/// relevant, and even a post-pipeline writer inflating its `top_score`
/// gets clamped back to ceiling+offset by `finalize_scores`.
#[test]
fn lookalike_never_relevant_and_score_ceiling_survives_post_writers() {
    let ctx = enriched_rust_persona();
    let mut failures = Vec::new();
    let ceiling = crate::scoring_config::COMMODITY_CEILING_REGISTRY_RELEASE_UNGROUNDED
        + crate::scoring_config::SCORE_OFFSET_NEGATIVE_FLOOR;
    for (i, fx) in LOOKALIKE_CRATES.iter().enumerate() {
        let mut r = score_registry_fixture(fx, &ctx, 10_701 + i as u64);
        if r.relevant {
            failures.push(format!(
                "  {} v{} RELEVANT at {:.3} — the categorical gate failed",
                fx.name, fx.version, r.top_score
            ));
        }
        // Simulate a post-pipeline writer (reconciler/rerank class) inflating
        // the score, then run the canonical final pass.
        r.top_score = 0.95;
        let mut batch = vec![r];
        crate::scoring::finalize_scores(&mut batch);
        let r = &batch[0];
        if r.top_score > ceiling + 1e-4 {
            failures.push(format!(
                "  {} v{} score {:.3} exceeds categorical ceiling {:.3} after \
                 finalize_scores — a post-pipeline writer can re-inflate capped items",
                fx.name, fx.version, r.top_score, ceiling
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "look-alike registry release invariants violated:\n{}",
        failures.join("\n")
    );
}

/// Real direct-dependency releases MUST stay relevant (recall guard —
/// suppressing these would destroy the product's core registry signal).
#[test]
fn real_dep_registry_releases_stay_relevant() {
    let ctx = enriched_rust_persona();
    let mut failures = Vec::new();
    for (i, fx) in REAL_DEP_CRATES.iter().enumerate() {
        let r = score_registry_fixture(fx, &ctx, 10_101 + i as u64);
        if !r.relevant {
            failures.push(format!(
                "  {} v{} scored NOT relevant ({:.3}) despite being a direct dependency",
                fx.name, fx.version, r.top_score
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "real dependency releases dropped (recall regression):\n{}",
        failures.join("\n")
    );
}

/// Grounded releases must out-score ungrounded look-alikes — the feed's
/// ordering invariant. In production the single top-scored crate was a
/// non-dependency (`forge-plugin-sdk-rust` 0.947 above tokio 0.873).
#[test]
fn real_dep_releases_outscore_lookalikes() {
    let ctx = enriched_rust_persona();
    let min_real = REAL_DEP_CRATES
        .iter()
        .enumerate()
        .map(|(i, fx)| score_registry_fixture(fx, &ctx, 10_201 + i as u64).top_score)
        .fold(f32::INFINITY, f32::min);
    let max_lookalike = LOOKALIKE_CRATES
        .iter()
        .enumerate()
        .map(|(i, fx)| score_registry_fixture(fx, &ctx, 10_301 + i as u64).top_score)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        min_real > max_lookalike,
        "ordering inverted: weakest real dep ({min_real:.3}) does not out-score \
         strongest look-alike ({max_lookalike:.3})"
    );
}

/// Federated social noise: aged posts with NO community engagement metadata
/// (live DB: mastodon/lemmy/bluesky tags are empty across the board) must be
/// hard-capped like every other UGC source, not ride the neutral arm to 0.9+.
/// Production evidence (§2.3): "Ditch Java bloat!…" mastodon promo at 0.91,
/// "💀Share some of your collection💀" lemmy at 0.609 — all feed_relevant.
#[test]
fn federated_social_noise_capped() {
    let ctx = enriched_rust_persona();
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = super::domain_embeddings::interest_embedding(0);
    let created = Utc::now() - Duration::hours(8);

    let fixtures: &[(&str, &str, &str)] = &[
        (
            "mastodon",
            "Ditch Java bloat! Switch to native Rust Meilisearch for instant search",
            "Promotional post: our search product is faster than your JVM stack. Try it now!",
        ),
        (
            "lemmy",
            "Share some of your collection",
            "Show off what you have been hoarding this month, any topic welcome.",
        ),
        (
            "bluesky",
            "sketch-a-day tip jar — support my art",
            "Daily art practice, tips appreciated, follow for more sketches.",
        ),
    ];

    let mut failures = Vec::new();
    for (i, (source, title, content)) in fixtures.iter().enumerate() {
        let input = ScoringInput {
            id: 10_401 + i as u64,
            title,
            url: Some("https://example.social"),
            content,
            source_type: source,
            embedding: &emb,
            created_at: Some(&created),
            detected_lang: "en",
            source_tags: &[],
            tags_json: None,
            feed_origin: None,
            source_id: None,
        };
        let r = score_item(&input, &ctx, &db, &opts, None);
        if r.top_score > 0.50 {
            failures.push(format!(
                "  [{source}] \"{title}\" scored {:.3} > 0.50 UGC cap with zero engagement",
                r.top_score
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "federated social posts escaped the UGC low-community cap:\n{}",
        failures.join("\n")
    );
}
