// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the LLM judgment engine's parse + store + demote layers.
//!
//! `LLMClient` is a concrete HTTP client (no trait seam), so these tests
//! exercise everything AROUND the network call: prompt-response parsing with
//! canned JSON, the store layer (judgments + content-analysis upserts) against
//! a real in-memory DB, the demote-only verdict feedback, and the budget/BYOK
//! gates via the gate-injectable inner pass — no test ever reaches the network
//! (asserted structurally: every async path either short-circuits on a gate or
//! sees an empty unjudged set before a client exists).

use super::*;
use crate::db::VerdictSource;
use crate::test_utils::{insert_test_item, test_db};

// ============================================================================
// Helpers
// ============================================================================

fn feed_relevant_of(db: &Database, id: i64) -> i64 {
    db.conn
        .lock()
        .query_row(
            "SELECT feed_relevant FROM source_items WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .unwrap()
}

fn seed_feed_item(db: &Database, source_id: &str, title: &str) -> i64 {
    let id = insert_test_item(db, "hackernews", source_id, title, "body");
    db.persist_feed_verdicts(
        &[(id, true, VerdictSource::Score)],
        crate::scoring::PIPELINE_VERSION,
    )
    .unwrap();
    id
}

fn response(technical_depth: Option<f64>, novelty: Option<f64>) -> JudgmentResponse {
    JudgmentResponse {
        relevance: Some(0.5),
        explanation: Some("e".into()),
        actions: None,
        confidence: Some(0.8),
        technical_depth,
        novelty,
        audience_level: None,
        key_insight: None,
    }
}

fn test_provider() -> crate::settings::LLMProvider {
    // No functional-update syntax: LLMProvider implements Drop (key
    // zeroization), which forbids moving out of a default instance.
    let mut p = crate::settings::LLMProvider::default();
    p.provider = "anthropic".into();
    p.api_key = "test-key".into();
    p.model = "claude-test".into();
    p
}

// ============================================================================
// Parse layer (prompt v2)
// ============================================================================

#[test]
fn parse_v2_batch_response_reads_analysis_fields() {
    let json = "```json\n[\n  {\"id\": 41, \"relevance\": 0.82, \"explanation\": \"e\", \"actions\": [\"investigate\"], \"confidence\": 0.9, \"technical_depth\": 4, \"novelty\": 3, \"audience_level\": \"Advanced\", \"key_insight\": \"k\"},\n  {\"id\": 42, \"relevance\": 0.2, \"explanation\": \"thin\", \"confidence\": 0.4}\n]\n```";
    let parsed = parse_batch_response(json).unwrap();
    assert_eq!(parsed.len(), 2);

    let (id, r) = &parsed[0];
    assert_eq!(*id, 41);
    assert_eq!(r.relevance, Some(0.82));
    assert_eq!(r.technical_depth, Some(4.0));
    assert_eq!(r.novelty, Some(3.0));
    assert_eq!(r.audience_level.as_deref(), Some("Advanced"));
    assert_eq!(r.key_insight.as_deref(), Some("k"));

    // Model omitted the analysis fields (instructed when content is thin) —
    // parsing must stay graceful, never unwrap.
    let (_, r2) = &parsed[1];
    assert!(r2.technical_depth.is_none());
    assert!(r2.novelty.is_none());
    assert!(r2.audience_level.is_none());
    assert!(r2.key_insight.is_none());
}

#[test]
fn parse_drops_elements_without_id_and_rejects_non_json() {
    let json = r#"[{"relevance": 0.9}, {"id": 7, "relevance": 0.5}]"#;
    let parsed = parse_batch_response(json).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].0, 7);

    assert!(parse_batch_response("I refuse to answer in JSON").is_err());
}

#[test]
fn parse_accepts_string_ids_and_quoted_numbers() {
    // Live regression 2026-08-31, first Haiku cycle after #553: the cheap
    // judge model quoted the id ("60146") and the strict i64 field failed
    // the ENTIRE batch — budget spent, judged=0, nothing stored. Both id and
    // numeric judgment fields must accept number-or-numeric-string.
    let json = r#"[
      {"id": "60146", "relevance": "0.3", "explanation": "e", "confidence": "0.8"},
      {"id": 60147, "relevance": 0.9, "explanation": "e2", "confidence": 0.7, "technical_depth": "4"}
    ]"#;
    let parsed = parse_batch_response(json).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].0, 60146);
    assert_eq!(parsed[0].1.relevance, Some(0.3));
    assert_eq!(parsed[0].1.confidence, Some(0.8));
    assert_eq!(parsed[1].0, 60147);
    assert_eq!(parsed[1].1.technical_depth, Some(4.0));
}

#[test]
fn parse_treats_garbage_numeric_fields_as_omitted_not_fatal() {
    // One malformed field must not discard the rest of the batch.
    let json = r#"[
      {"id": "not-a-number", "relevance": 0.9},
      {"id": 8, "relevance": "high", "confidence": 0.6, "explanation": "ok"}
    ]"#;
    let parsed = parse_batch_response(json).unwrap();
    // Element with an unparseable id is dropped (nothing to attach to);
    // the unparseable relevance string becomes None, the element survives.
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].0, 8);
    assert_eq!(parsed[0].1.relevance, None);
    assert_eq!(parsed[0].1.confidence, Some(0.6));
}

#[test]
fn prompt_pins_confidence_semantics_and_mandatory_field() {
    // The demotion gate reads `confidence >= 0.7` as "the judge is sure".
    // Two distinct ways that premise has already been broken in production:
    //   1. semantics — rejection expressed AS low confidence (the v4 fix), and
    //   2. ABSENCE — the model omitting the field altogether, which the store
    //      layer used to paper over with a fabricated 0.5 (the v5 fix; 93% of
    //      the v3 cohort sat at exactly 0.5).
    // Both instructions are load-bearing: losing either silently kills
    // demotions again, which is invisible without this test.
    let prompt = judge_system_prompt("CTX");
    assert!(
        prompt.contains("A clear rejection carries HIGH confidence"),
        "confidence-semantics instruction missing from the judge prompt"
    );
    assert!(
        prompt.contains("never that the item is irrelevant"),
        "low-confidence-means-unsure clarification missing from the judge prompt"
    );
    assert!(
        prompt.contains("REQUIRED on every element, never omitted"),
        "confidence must be marked mandatory — an omitted field disables the gate"
    );
    assert!(
        prompt.contains("omission rule NEVER applies to id, relevance, explanation, or confidence"),
        "the low-relevance omission rule must exempt confidence explicitly"
    );
    assert!(prompt.contains("CTX"), "user context must be embedded");
    assert_eq!(
        PROMPT_VERSION, "v5",
        "the no-fabrication fix must ride its own cohort"
    );
}

#[test]
fn omitted_confidence_is_stored_as_unknown_never_fabricated() {
    // Regression pin for the 2026-08-31 dead-gate root cause. `unwrap_or(0.5)`
    // turned an ABSENT confidence into "moderately sure" — a reading the judge
    // never gave — and 0.5 sits just below DEMOTION_CONFIDENCE_MIN, so every
    // such judgment silently became undemotable. Unknown must land at 0.0:
    // it cannot satisfy `>= threshold` in the demotion gate or in
    // judge_agreement_live, which is the honest outcome when nothing was said.
    let db = test_db();
    let id = insert_test_item(&db, "hackernews", "j-omit", "No confidence given", "body");
    let items = load_items_for_judgment(&db, &[id]).unwrap();

    let results = vec![(
        id,
        JudgmentResponse {
            relevance: Some(0.05),
            explanation: Some("plainly outside the user's stack".into()),
            actions: None,
            confidence: None, // the live Haiku failure shape
            technical_depth: None,
            novelty: None,
            audience_level: None,
            key_insight: None,
        },
    )];

    let (judged, _) = store_batch_results(&db, &items, results, "m");
    assert_eq!(judged, 1);

    let j = db.get_llm_judgment(id).unwrap().unwrap();
    assert!(
        j.confidence.abs() < 1e-6,
        "an omitted confidence must store as 0.0 (unknown), got {}",
        j.confidence
    );
    assert!(
        (j.confidence - 0.5).abs() > 1e-6,
        "0.5 is the fabricated value that disabled the gate — it must never be invented"
    );
    assert!(
        j.confidence < DEMOTION_CONFIDENCE_MIN,
        "unknown confidence must never satisfy the demotion gate"
    );
}

#[test]
fn to_scale_clamps_and_rejects_nonfinite() {
    assert_eq!(to_scale_1_5(Some(7.0)), Some(5));
    assert_eq!(to_scale_1_5(Some(0.4)), Some(1));
    assert_eq!(to_scale_1_5(Some(3.4)), Some(3));
    assert_eq!(to_scale_1_5(Some(f64::NAN)), None);
    assert_eq!(to_scale_1_5(Some(f64::INFINITY)), None);
    assert_eq!(to_scale_1_5(None), None);
}

#[test]
fn analysis_from_response_requires_depth_novelty_and_content() {
    let items = vec![
        ItemForJudgment {
            id: 1,
            title: "T".into(),
            content: Some("real body".into()),
            source_type: "hackernews".into(),
            relevance_score: 0.5,
        },
        ItemForJudgment {
            id: 2,
            title: "T".into(),
            content: Some("   ".into()),
            source_type: "hackernews".into(),
            relevance_score: 0.5,
        },
    ];

    let mut full = response(Some(4.0), Some(2.0));
    full.key_insight = Some("  ".into());
    let a = analysis_from_response(&items, 1, &full).expect("analysis");
    assert_eq!(a.technical_depth, 4);
    assert_eq!(a.novelty, 2);
    assert_eq!(
        a.audience_level,
        crate::content_analysis::AudienceLevel::Intermediate,
        "absent audience defaults to the neutral level (multiplier 1.0)"
    );
    assert!(a.key_insight.is_none(), "whitespace insight is dropped");
    assert_eq!(
        a.content_hash,
        crate::content_analysis::content_hash("real body"),
        "hash must be over the FULL stored content — the pipeline read side's key"
    );

    // Whitespace-only content → no analysis (shared-digest protection).
    assert!(analysis_from_response(&items, 2, &full).is_none());
    // Missing either scale field → no analysis (omit, never fabricate).
    assert!(analysis_from_response(&items, 1, &response(Some(4.0), None)).is_none());
    assert!(analysis_from_response(&items, 1, &response(None, Some(3.0))).is_none());
    // Unknown item id → no analysis.
    assert!(analysis_from_response(&items, 99, &response(Some(3.0), Some(3.0))).is_none());
}

// ============================================================================
// Store layer (judgment + content_analyses upserts)
// ============================================================================

#[test]
fn store_batch_results_upserts_judgment_and_content_analysis() {
    let db = test_db();
    let body = "long body about tokio scheduler internals";
    let full = insert_test_item(&db, "hackernews", "j1", "Deep dive into tokio", body);
    let empty = insert_test_item(&db, "hackernews", "j2", "Empty body item", "");

    let items = load_items_for_judgment(&db, &[full, empty]).unwrap();
    let results = vec![
        (
            full,
            JudgmentResponse {
                relevance: Some(0.82),
                explanation: Some("tokio is in your Cargo.lock".into()),
                actions: Some(vec!["investigate".into()]),
                confidence: Some(0.9),
                technical_depth: Some(5.0),
                novelty: Some(4.0),
                audience_level: Some("expert".into()),
                key_insight: Some("Work-stealing details".into()),
            },
        ),
        (
            empty,
            JudgmentResponse {
                relevance: Some(0.3),
                explanation: Some("thin".into()),
                actions: None,
                confidence: Some(0.6),
                technical_depth: Some(3.0),
                novelty: Some(3.0),
                audience_level: None,
                key_insight: None,
            },
        ),
    ];

    let (judged, analyses) = store_batch_results(&db, &items, results, "test-model");
    assert_eq!(judged, 2);
    assert_eq!(
        analyses, 1,
        "empty-content item must not write an analysis row"
    );

    let j = db.get_llm_judgment(full).unwrap().unwrap();
    assert_eq!(j.prompt_version, PROMPT_VERSION);
    assert!((j.relevance_score - 0.82).abs() < 1e-6);
    assert_eq!(j.model, "test-model");
    assert_eq!(j.actions.as_deref(), Some("[\"investigate\"]"));

    let hash = crate::content_analysis::content_hash(body);
    let a = crate::content_analysis::get_cached_analysis(&db, &hash)
        .unwrap()
        .expect("analysis row keyed by the item's content hash");
    assert_eq!(a.technical_depth, 5);
    assert_eq!(a.novelty, 4);
    assert_eq!(
        a.audience_level,
        crate::content_analysis::AudienceLevel::Expert,
        "lowercase LLM output parses to the canonical level"
    );
    assert_eq!(a.key_insight.as_deref(), Some("Work-stealing details"));

    let empty_hash = crate::content_analysis::content_hash("");
    assert!(
        crate::content_analysis::get_cached_analysis(&db, &empty_hash)
            .unwrap()
            .is_none(),
        "the shared empty-content digest must never be written"
    );
}

#[test]
fn store_batch_results_clamps_out_of_range_values() {
    let db = test_db();
    let id = insert_test_item(&db, "hackernews", "j3", "Clamp me", "body text");
    let items = load_items_for_judgment(&db, &[id]).unwrap();

    let results = vec![(
        id,
        JudgmentResponse {
            relevance: Some(1.7),
            explanation: None,
            actions: None,
            confidence: Some(-0.2),
            technical_depth: Some(9.0),
            novelty: Some(0.0),
            audience_level: Some("galactic".into()),
            key_insight: None,
        },
    )];

    let (judged, analyses) = store_batch_results(&db, &items, results, "m");
    assert_eq!((judged, analyses), (1, 1));

    let j = db.get_llm_judgment(id).unwrap().unwrap();
    assert!((j.relevance_score - 1.0).abs() < 1e-6);
    assert!(j.confidence.abs() < 1e-6);

    let hash = crate::content_analysis::content_hash("body text");
    let a = crate::content_analysis::get_cached_analysis(&db, &hash)
        .unwrap()
        .unwrap();
    assert_eq!(a.technical_depth, 5);
    assert_eq!(a.novelty, 1);
    assert_eq!(
        a.audience_level,
        crate::content_analysis::AudienceLevel::Intermediate,
        "unknown audience string falls back to neutral"
    );
}

// ============================================================================
// Demote-only verdict feedback
// ============================================================================

#[test]
fn low_relevance_high_confidence_judgment_demotes_curated_item() {
    let db = test_db();
    let id = seed_feed_item(
        &db,
        "d1",
        "Beginner tutorial that name-drops your whole stack",
    );
    db.upsert_llm_judgment(
        id,
        0.10,
        "It is a beginner tutorial",
        None,
        0.9,
        "m",
        PROMPT_VERSION,
    )
    .unwrap();

    let demoted = apply_judgment_demotions(&db, DEMOTION_CAP_PER_RUN).unwrap();
    assert_eq!(demoted, 1);

    let (feed_relevant, reason): (i64, Option<String>) = db
        .conn
        .lock()
        .query_row(
            "SELECT feed_relevant, feed_verdict_reason FROM source_items WHERE id = ?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(feed_relevant, 0);
    assert_eq!(reason.as_deref(), Some("llm_reject"));
}

#[test]
fn demotion_spares_serendipity_relevant_unconfident_and_old_prompt() {
    let db = test_db();

    // Serendipity pick judged low+confident — immune: anti-bubble picks are
    // SUPPOSED to look irrelevant to a relevance judge.
    let lucky = insert_test_item(&db, "hackernews", "d2", "Anti-bubble pick", "body");
    db.persist_feed_verdicts(
        &[(lucky, true, VerdictSource::Serendipity)],
        crate::scoring::PIPELINE_VERSION,
    )
    .unwrap();
    db.upsert_llm_judgment(
        lucky,
        0.05,
        "looks off-topic (by design)",
        None,
        0.95,
        "m",
        PROMPT_VERSION,
    )
    .unwrap();

    // Judged relevant — immune.
    let good = seed_feed_item(&db, "d3", "Genuinely relevant");
    db.upsert_llm_judgment(good, 0.85, "matches stack", None, 0.9, "m", PROMPT_VERSION)
        .unwrap();

    // Judged low but unconfident — immune.
    let unsure = seed_feed_item(&db, "d4", "Judge unsure");
    db.upsert_llm_judgment(
        unsure,
        0.10,
        "maybe irrelevant",
        None,
        0.5,
        "m",
        PROMPT_VERSION,
    )
    .unwrap();

    // Prior prompt version — immune (only the current judge brain demotes).
    let old = seed_feed_item(&db, "d5", "Old brain judgment");
    db.upsert_llm_judgment(old, 0.05, "old", None, 0.95, "m", "v1")
        .unwrap();

    assert_eq!(
        apply_judgment_demotions(&db, DEMOTION_CAP_PER_RUN).unwrap(),
        0
    );
    for id in [lucky, good, unsure, old] {
        assert_eq!(feed_relevant_of(&db, id), 1, "item {id} must stay curated");
    }
}

#[test]
fn demotion_ignores_stale_judgments() {
    let db = test_db();
    let id = seed_feed_item(&db, "d6", "Judged long ago");
    db.upsert_llm_judgment(id, 0.05, "old read", None, 0.95, "m", PROMPT_VERSION)
        .unwrap();
    db.conn
        .lock()
        .execute(
            "UPDATE llm_judgments SET judged_at = datetime('now', '-10 days')
             WHERE source_item_id = ?1",
            rusqlite::params![id],
        )
        .unwrap();

    assert_eq!(
        apply_judgment_demotions(&db, DEMOTION_CAP_PER_RUN).unwrap(),
        0
    );
    assert_eq!(feed_relevant_of(&db, id), 1);
}

#[test]
fn demotion_respects_per_run_cap_and_converges() {
    let db = test_db();
    for i in 0..12 {
        let id = seed_feed_item(&db, &format!("cap{i}"), &format!("Junk {i}"));
        db.upsert_llm_judgment(id, 0.05, "junk", None, 0.95, "m", PROMPT_VERSION)
            .unwrap();
    }

    assert_eq!(apply_judgment_demotions(&db, 10).unwrap(), 10);
    // Demoted rows leave the working set (feed_relevant = 0), so the next
    // pass drains the remainder instead of re-picking the same items.
    assert_eq!(apply_judgment_demotions(&db, 10).unwrap(), 2);
    assert_eq!(apply_judgment_demotions(&db, 10).unwrap(), 0);
}

// ============================================================================
// Post-cycle pass gates (budget + BYOK)
// ============================================================================

#[tokio::test]
async fn post_cycle_no_op_when_budget_reached() {
    let db = test_db();
    let id = seed_feed_item(&db, "g1", "Would-be demotion");
    db.upsert_llm_judgment(id, 0.05, "junk", None, 0.95, "m", PROMPT_VERSION)
        .unwrap();

    let summary = run_post_cycle_with(&db, true, Some(test_provider())).await;
    assert_eq!(summary.skipped, Some("llm_budget_reached"));
    assert_eq!(summary.judged, 0);
    assert_eq!(summary.analyses_stored, 0);
    assert_eq!(summary.demoted, 0);
    assert_eq!(
        feed_relevant_of(&db, id),
        1,
        "a budget no-op must not touch verdicts"
    );
}

#[tokio::test]
async fn post_cycle_no_op_without_provider() {
    let db = test_db();
    let id = seed_feed_item(&db, "g2", "Would-be demotion");
    db.upsert_llm_judgment(id, 0.05, "junk", None, 0.95, "m", PROMPT_VERSION)
        .unwrap();

    let summary = run_post_cycle_with(&db, false, None).await;
    assert_eq!(summary.skipped, Some("no_llm_provider"));
    assert_eq!(summary.judged, 0);
    assert_eq!(summary.demoted, 0);
    assert_eq!(
        feed_relevant_of(&db, id),
        1,
        "a BYOK no-op must not touch verdicts"
    );
}

#[tokio::test]
async fn post_cycle_runs_demotions_without_llm_calls() {
    // Provider present, budget fine — but nothing is unjudged (the seeded item
    // already has a judgment and its relevance_score is NULL), so the judge
    // lane makes ZERO network calls; the demotion lane still fires.
    let db = test_db();
    let id = seed_feed_item(&db, "g3", "Confident reject");
    db.upsert_llm_judgment(id, 0.05, "junk", None, 0.95, "m", PROMPT_VERSION)
        .unwrap();

    let summary = run_post_cycle_with(&db, false, Some(test_provider())).await;
    assert_eq!(summary.skipped, None);
    assert_eq!(summary.judged, 0);
    assert_eq!(summary.demoted, 1);
    assert_eq!(feed_relevant_of(&db, id), 0);
}

// ============================================================================
// Drain carve-out (2026-08-31 audit: the fresh lane cedes the drain's slice)
// ============================================================================

/// The fresh judge lane's selection shrinks by exactly the drain's slice —
/// bounded by the real backlog, never more than DRAIN_SLICE, and zero when
/// nothing is pending (a healthy instance keeps its full selection).
#[test]
fn drain_reserve_is_backlog_bounded_and_capped() {
    assert_eq!(drain_reserve(0), 0, "no backlog, no carve-out");
    assert_eq!(drain_reserve(-1), 0, "defensive: negative counts");
    assert_eq!(drain_reserve(3), 3, "small backlog reserves only itself");
    assert_eq!(
        drain_reserve(175),
        crate::llm_judge::drain::DRAIN_SLICE,
        "the live-audit backlog caps at the 20% slice"
    );
}

// ============================================================================
// Truncated-batch salvage
//
// Regression cover for the 2026-09-01 finding: a single truncated element
// discarded all ten judgments in a paid-for batch. Surfaced by
// `scoring::judge_benchmark`, which lost 11 of 87 scenarios to exactly one
// such reply.
// ============================================================================

#[test]
fn truncated_batch_salvages_the_complete_elements() {
    // Element 3 is cut off mid-object, exactly as a max_tokens truncation
    // leaves it. The two complete elements before it must survive.
    let truncated = r#"[
      {"id": 101, "relevance": 0.8, "explanation": "tokio is in their stack", "confidence": 0.9},
      {"id": 102, "relevance": 0.1, "explanation": "unrelated ecosystem", "confidence": 0.85},
      {"id": 103, "relevance": 0.7, "explan"#;

    let parsed = parse_batch_response(truncated).expect("salvage must not error");
    assert_eq!(parsed.len(), 2, "both complete elements must be recovered");
    assert_eq!(parsed[0].0, 101);
    assert_eq!(parsed[1].0, 102);
    assert!((parsed[0].1.relevance.unwrap() - 0.8).abs() < f64::EPSILON);
    assert!((parsed[1].1.confidence.unwrap() - 0.85).abs() < f64::EPSILON);
}

#[test]
fn salvage_is_not_confused_by_braces_inside_strings() {
    // A brace inside an explanation string must not open or close an element,
    // or the scanner would split one judgment into two malformed halves.
    let text = r#"[
      {"id": 7, "relevance": 0.9, "explanation": "use Handler{} for }this{ case", "confidence": 0.8},
      {"id": 8, "relevance": 0.2, "explanation": "escaped quote \" and a { brace", "confidence": 0.75},
      {"id": 9, "relevance": 0.5, "expl"#;

    let parsed = parse_batch_response(text).expect("salvage must not error");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].0, 7);
    assert_eq!(parsed[1].0, 8);
    assert!(parsed[0]
        .1
        .explanation
        .as_deref()
        .unwrap()
        .contains("}this{"));
}

#[test]
fn a_reply_with_no_recoverable_element_is_still_an_error() {
    // Salvage must not convert a genuinely broken reply into a silent empty
    // success — that would look like "the judge rejected nothing" instead of
    // "the judge did not answer", the exact ambiguity the WARN paths exist to
    // remove.
    let junk = "I'm sorry, I cannot evaluate these items.";
    assert!(
        parse_batch_response(junk).is_err(),
        "a non-JSON reply must stay a hard error"
    );
}

#[test]
fn salvage_drops_elements_that_have_no_id() {
    // An element without an id has nothing to attach a judgment to; it is
    // dropped, and must not shift the ids of its neighbours.
    let text = r#"[
      {"relevance": 0.9, "explanation": "no id here", "confidence": 0.8},
      {"id": 42, "relevance": 0.3, "explanation": "keeps its id", "confidence": 0.7}
    ]"#;
    let parsed = parse_batch_response(text).expect("valid JSON");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].0, 42);
}
