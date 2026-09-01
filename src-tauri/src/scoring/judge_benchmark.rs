// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Judge accuracy benchmark — the LLM judge measured against LABELS.
//!
//! ## Why this file exists
//!
//! Every other measurement of the judge compares it to the SCORING PIPELINE.
//! `judge_agreement_live` counts where the two disagree; the Mesh's shadow
//! arena compares one model against another. Those are two UNLABELED opinions.
//! They can prove the judge and the pipeline differ — they can never say which
//! one is right, so "are the LLM judges worth their cost?" had no answerable
//! form at all.
//!
//! Measured on the live instance 2026-09-01, which is why this exists:
//!   * the judge lanes spend 78c of a 150c daily cap (52% of all LLM spend),
//!     and the cap is reached ~6.7h before local midnight — after which every
//!     LLM lane is dead while ingestion keeps running;
//!   * they demote roughly half the curated feed (471 items stamped
//!     `llm_reject` against 392 surviving, over 14 days);
//!   * the only outcome signal that could adjudicate any of it is empty:
//!     `feedback` holds ONE row ever, `interactions` stopped 2026-08-24.
//!
//! `benchmark_scenarios.json` already carried 87 human-labeled scenarios,
//! 40 relevant / 47 not — a BALANCED set, which the live corpus (~95% reject)
//! emphatically is not. Nothing had ever run the judge against them.
//! `judge_scenarios.json` adds 14 more, written from live failures this
//! benchmark exposed: 101 cases, 45 relevant / 56 not.
//!
//! What it has already caught, in its first three runs:
//!   * a truncated element discarding an entire paid-for batch of ten
//!     judgments (fixed — see `parse_batch_response`);
//!   * prompt v5 accepting, with high confidence, the exact classes the RERANK
//!     judge's prompt guards against and the ingest prompt never mentioned —
//!     beginner questions about tech the user ships expertly (0/3) and pre-1.0
//!     packages that merely name-match the stack (0/2). Porting those rules
//!     into v6 moved MCC 0.607 -> 0.728 with recall RISING (0.911 -> 0.932),
//!     so the gain is discrimination, not a stricter threshold.
//!
//! ## What it measures
//!
//! The shipped prompt ([`judge_system_prompt`]), the shipped item rendering
//! ([`format_items_block`]), the shipped parser ([`parse_batch_response`]) and
//! the shipped thresholds — never a copy of any of them. A benchmark that
//! re-implements the prompt measures its own copy: it keeps passing while the
//! live judge changes underneath it, which is precisely how the v3 confidence
//! regression survived two audits.
//!
//! Two verdicts are scored per scenario:
//!
//!   1. **Relevance call** — `relevance >= DEMOTION_RELEVANCE_BELOW` against
//!      the scenario's `should_be_relevant` label, reported as a confusion
//!      matrix plus MCC. MCC, unlike raw agreement, does not inflate when one
//!      class dominates — the exact trap that makes the live judge look "93%
//!      consistent" when its chance-corrected agreement is about 0.2.
//!   2. **The shipped demotion gate** — `relevance < 0.30 AND confidence >=
//!      0.7`. `false_demotions` is the number a user would feel: items the
//!      labels call relevant that the live gate would delete from the feed.
//!
//! Plus the confidence-omission rate — the tripwire for the regression class
//! where a judge stops emitting `confidence` and silently disables the gate.
//!
//! ## Deliberate design choices
//!
//! * **Neutral pipeline score.** Production shows the judge the pipeline's own
//!   `relevance_score`. Feeding the scenario's expected range would leak the
//!   label; feeding a real pipeline score would make judge numbers move
//!   whenever the PIPELINE changed. Every scenario is presented at a fixed
//!   [`NEUTRAL_PIPELINE_SCORE`], so this isolates the judge's own
//!   discrimination and stays comparable across pipeline versions.
//! * **It spends money** (~11 calls, ~5c on the Haiku judge sibling), so it is
//!   opt-in via `FOURDA_JUDGE_BENCHMARK=1` and can never run as a side effect
//!   of `cargo test`.
//! * **Its own process, its own budget.** The daily cost counters are
//!   per-process and seeded only by the app at startup, so this harness does
//!   not consume — or get blocked by — the live app's daily cap. That is
//!   deliberate: a starved day is exactly when you most need to measure.
//!
//! ## Running it
//!
//! ```text
//! FOURDA_JUDGE_BENCHMARK=1 FOURDA_DB_PATH=D:/4DA/data/4da.db \
//!     cargo test --lib judge_benchmark -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;

use serde::Deserialize;

use super::benchmark_scenarios::{load_scenarios, profile_ctx};
use crate::llm::{LLMClient, Message};
use crate::llm_judgments::{
    format_items_block, judge_system_prompt, parse_batch_response, ItemForJudgment, BATCH_SIZE,
    DEMOTION_CONFIDENCE_MIN, DEMOTION_RELEVANCE_BELOW, PROMPT_VERSION,
};

/// The pipeline score every scenario is presented at. See the module docs: a
/// fixed value keeps the benchmark measuring the JUDGE rather than the
/// pipeline, and stops the expected range leaking the label into the prompt.
const NEUTRAL_PIPELINE_SCORE: f64 = 0.50;

/// The judge-only corpus.
///
/// Deliberately NOT merged into `benchmark_scenarios.json`: that file is the
/// PIPELINE's contract — `scenarios_parse_correctly` hard-asserts its exact
/// count and `benchmark_scoring_accuracy` gates on score ranges produced with a
/// ZERO embedding. Judge cases would have to carry pipeline expectations they
/// have no business asserting, and every future judge case would re-tune a
/// pipeline gate. Two corpora, one loop.
///
/// These cases come from MEASURED live failures (2026-09-01), and every
/// negative is paired with a near-twin positive — an obscure `tauri-plugin-*`
/// v0.1.0 against an in-stack `thiserror` release, a beginner React question
/// against a React-internals deep dive, an off-domain ML paper against a rustc
/// soundness paper. Without the twins, a prompt that simply rejects more would
/// score better while getting worse.
const JUDGE_SCENARIOS_JSON: &str = include_str!("judge_scenarios.json");

/// Regression tripwire, NOT a quality target.
///
/// Measured on this corpus: prompt v5 scored 0.607, v6 scored 0.728. The floor
/// sits below both, because one LLM run has real variance and a floor set at
/// the last reading would flap on noise rather than catch regressions. What it
/// catches is a COLLAPSE — a prompt edit that guts discrimination, a model
/// swap to something that cannot do the task, a silent provider-side change.
///
/// Gradual drift is not this assert's job; the appended JSONL trend is, and
/// `pnpm run bench:judge` prints the last ten runs for exactly that reason.
/// Raise this only from measured evidence, never to make a run pass.
const MCC_FLOOR: f64 = 0.55;

#[derive(Deserialize)]
struct JudgeScenario {
    id: String,
    category: String,
    item: JudgeItem,
    profile: String,
    relevant: bool,
}

#[derive(Deserialize)]
struct JudgeItem {
    title: String,
    content: String,
    source_type: String,
}

/// One labeled case, from either corpus.
struct Case {
    id: String,
    category: String,
    title: String,
    content: String,
    source_type: String,
    profile: String,
    truth_relevant: bool,
}

/// Both corpora as one list: the 87 pipeline scenarios (read for their
/// `should_be_relevant` label only) plus the judge-specific cases.
fn load_cases() -> Vec<Case> {
    let mut cases: Vec<Case> = load_scenarios()
        .into_iter()
        .map(|s| Case {
            id: s.id,
            category: s.category,
            title: s.item.title,
            content: s.item.content,
            source_type: s.item.source_type,
            profile: s.profile,
            truth_relevant: s.expected.should_be_relevant,
        })
        .collect();

    let judge: Vec<JudgeScenario> = serde_json::from_str(JUDGE_SCENARIOS_JSON)
        .expect("judge_scenarios.json must be valid JSON");
    cases.extend(judge.into_iter().map(|j| Case {
        id: j.id,
        category: j.category,
        title: j.item.title,
        content: j.item.content,
        source_type: j.item.source_type,
        profile: j.profile,
        truth_relevant: j.relevant,
    }));

    cases
}

/// One scenario's result.
struct Outcome {
    id: String,
    category: String,
    title: String,
    truth_relevant: bool,
    /// `None` when the model dropped this item from its reply entirely.
    judged: Option<(f64, Option<f64>)>,
}

/// Confusion matrix for the relevance call, positive class = "relevant".
#[derive(Default, Debug, Clone, Copy)]
struct Matrix {
    tp: u32,
    fp: u32,
    tn: u32,
    fn_: u32,
}

impl Matrix {
    fn total(self) -> u32 {
        self.tp + self.fp + self.tn + self.fn_
    }
    fn accuracy(self) -> f64 {
        let t = self.total();
        if t == 0 {
            return 0.0;
        }
        f64::from(self.tp + self.tn) / f64::from(t)
    }
    fn precision(self) -> f64 {
        let d = self.tp + self.fp;
        if d == 0 {
            return 0.0;
        }
        f64::from(self.tp) / f64::from(d)
    }
    fn recall(self) -> f64 {
        let d = self.tp + self.fn_;
        if d == 0 {
            return 0.0;
        }
        f64::from(self.tp) / f64::from(d)
    }
    fn f1(self) -> f64 {
        let (p, r) = (self.precision(), self.recall());
        if p + r == 0.0 {
            return 0.0;
        }
        2.0 * p * r / (p + r)
    }
    /// Matthews correlation coefficient. The headline number: it collapses to
    /// ~0 for a judge that just votes the majority class, which raw accuracy
    /// and raw agreement both reward.
    fn mcc(self) -> f64 {
        let (tp, tn) = (f64::from(self.tp), f64::from(self.tn));
        let (fp, fn_) = (f64::from(self.fp), f64::from(self.fn_));
        let denom = ((tp + fp) * (tp + fn_) * (tn + fp) * (tn + fn_)).sqrt();
        if denom == 0.0 {
            return 0.0;
        }
        (tp * tn - fp * fn_) / denom
    }
}

/// The user-context block for a labeled profile, in the SAME two-line shape
/// `adversarial::build_user_context_summary` produces in production (same
/// field order, same 10/5 caps, same fallback string). Built from the
/// benchmark's own `ScoringContext` so a run is reproducible and never depends
/// on whatever the live ACE happens to hold.
fn profile_user_context(profile: &str) -> String {
    let ctx = profile_ctx(profile);
    let mut parts = Vec::new();

    let tech: Vec<&str> = if ctx.declared_tech.is_empty() {
        ctx.ace_ctx
            .detected_tech
            .iter()
            .take(10)
            .map(String::as_str)
            .collect()
    } else {
        ctx.declared_tech
            .iter()
            .take(10)
            .map(String::as_str)
            .collect()
    };
    if !tech.is_empty() {
        parts.push(format!("Tech stack: {}", tech.join(", ")));
    }

    let topics: Vec<&str> = ctx
        .interests
        .iter()
        .take(5)
        .map(|i| i.topic.as_str())
        .collect();
    if !topics.is_empty() {
        parts.push(format!("Active topics: {}", topics.join(", ")));
    }

    if parts.is_empty() {
        "General software developer (no specific tech context available)".to_string()
    } else {
        parts.join("\n")
    }
}

/// Judge one profile's scenarios, in the shipped batch size.
/// Returns the outcomes plus `(input_tokens, output_tokens)`.
async fn judge_profile(
    client: &LLMClient,
    profile: &str,
    scenarios: &[(usize, &Case)],
) -> (Vec<Outcome>, u64, u64) {
    let user_context = profile_user_context(profile);
    let system_prompt = judge_system_prompt(&user_context);
    let mut outcomes = Vec::new();
    let (mut tin, mut tout) = (0u64, 0u64);

    for chunk in scenarios.chunks(BATCH_SIZE) {
        let items: Vec<ItemForJudgment> = chunk
            .iter()
            .map(|(idx, s)| ItemForJudgment {
                // Synthetic numeric id: the shipped prompt and parser are
                // id-keyed on i64. Index+1 keeps them small and stable.
                id: (*idx as i64) + 1,
                title: s.title.clone(),
                content: Some(s.content.clone()),
                source_type: s.source_type.clone(),
                relevance_score: NEUTRAL_PIPELINE_SCORE,
            })
            .collect();

        let user_msg = format!("Evaluate these items:\n\n{}", format_items_block(&items));
        let messages = vec![Message {
            role: "user".to_string(),
            content: user_msg,
        }];

        let parsed = match client.complete(&system_prompt, messages).await {
            Ok(resp) => {
                tin += resp.input_tokens;
                tout += resp.output_tokens;
                match parse_batch_response(&resp.content) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("  !! batch parse failed for {profile}: {e}");
                        Vec::new()
                    }
                }
            }
            Err(e) => {
                eprintln!("  !! batch call failed for {profile}: {e}");
                Vec::new()
            }
        };

        for (idx, s) in chunk {
            let synthetic = (*idx as i64) + 1;
            let judged = parsed
                .iter()
                .find(|(id, _)| *id == synthetic)
                .map(|(_, r)| (r.relevance.unwrap_or(0.0).clamp(0.0, 1.0), r.confidence));
            outcomes.push(Outcome {
                id: s.id.clone(),
                category: s.category.clone(),
                title: s.title.chars().take(64).collect(),
                truth_relevant: s.truth_relevant,
                judged,
            });
        }
    }

    (outcomes, tin, tout)
}

#[tokio::test]
#[ignore = "spends money: set FOURDA_JUDGE_BENCHMARK=1 to run"]
async fn judge_accuracy_benchmark() {
    if std::env::var("FOURDA_JUDGE_BENCHMARK").unwrap_or_default() != "1" {
        eprintln!("FOURDA_JUDGE_BENCHMARK != 1 — not spending money, nothing measured");
        return;
    }

    // Same routing production uses: the configured provider, downgraded to the
    // cheap judge sibling. Measuring the premium model would measure something
    // the user never runs.
    let provider = {
        let mgr = crate::get_settings_manager();
        let mut guard = mgr.lock();
        guard.ensure_keys_hydrated();
        crate::llm_judge::judge_provider(&guard.get().llm)
    };
    if provider.provider != "ollama" && provider.api_key.is_empty() {
        eprintln!("no API key configured — nothing measured");
        return;
    }
    let model = provider.model.clone();
    let client = LLMClient::with_purpose(provider, "judge_benchmark");

    let scenarios = load_cases();
    let mut by_profile: BTreeMap<String, Vec<(usize, &Case)>> = BTreeMap::new();
    for (i, s) in scenarios.iter().enumerate() {
        by_profile
            .entry(s.profile.clone())
            .or_default()
            .push((i, s));
    }

    println!("\n=== JUDGE ACCURACY BENCHMARK ===");
    println!("model          : {model}");
    println!("prompt_version : {PROMPT_VERSION}");
    println!("scenarios      : {}", scenarios.len());
    println!(
        "gate           : relevance < {DEMOTION_RELEVANCE_BELOW} AND confidence >= {DEMOTION_CONFIDENCE_MIN}\n"
    );

    let mut all: Vec<Outcome> = Vec::new();
    let (mut tin, mut tout) = (0u64, 0u64);
    for (profile, list) in &by_profile {
        println!("judging {profile} ({} scenarios)...", list.len());
        let (o, i, ot) = judge_profile(&client, profile, list).await;
        all.extend(o);
        tin += i;
        tout += ot;
    }

    // ── Score ───────────────────────────────────────────────────────────
    let mut m = Matrix::default();
    let mut by_cat: BTreeMap<String, Matrix> = BTreeMap::new();
    let mut missing = 0u32;
    let mut conf_omitted = 0u32;
    let mut true_demotions = 0u32;
    let mut false_demotions: Vec<&Outcome> = Vec::new();
    let mut misses: Vec<&Outcome> = Vec::new();

    for o in &all {
        let Some((relevance, confidence)) = o.judged else {
            missing += 1;
            continue;
        };
        if confidence.is_none() {
            conf_omitted += 1;
        }
        let judge_says_relevant = relevance >= DEMOTION_RELEVANCE_BELOW;
        let cat = by_cat.entry(o.category.clone()).or_default();
        match (o.truth_relevant, judge_says_relevant) {
            (true, true) => {
                m.tp += 1;
                cat.tp += 1;
            }
            (true, false) => {
                m.fn_ += 1;
                cat.fn_ += 1;
                misses.push(o);
            }
            (false, true) => {
                m.fp += 1;
                cat.fp += 1;
            }
            (false, false) => {
                m.tn += 1;
                cat.tn += 1;
            }
        }

        // The SHIPPED gate, exactly as `apply_judgment_demotions` applies it.
        let would_demote = relevance < DEMOTION_RELEVANCE_BELOW
            && confidence.unwrap_or(0.0) >= DEMOTION_CONFIDENCE_MIN;
        if would_demote {
            if o.truth_relevant {
                false_demotions.push(o);
            } else {
                true_demotions += 1;
            }
        }
    }

    // ── Report ──────────────────────────────────────────────────────────
    println!("\n--- relevance call (positive = relevant) ---");
    println!("  scored           : {}", m.total());
    println!("  no judgment      : {missing}");
    println!("  confidence absent: {conf_omitted}");
    println!(
        "  TP {:<4} FP {:<4} TN {:<4} FN {:<4}",
        m.tp, m.fp, m.tn, m.fn_
    );
    println!("  accuracy  : {:.3}", m.accuracy());
    println!("  precision : {:.3}", m.precision());
    println!("  recall    : {:.3}", m.recall());
    println!("  F1        : {:.3}", m.f1());
    println!("  MCC       : {:.3}   <-- headline", m.mcc());

    println!("\n--- shipped demotion gate ---");
    println!("  correct demotions : {true_demotions}");
    println!(
        "  FALSE demotions   : {}   <-- relevant items the live gate would delete",
        false_demotions.len()
    );
    for o in &false_demotions {
        println!("      [{}] {}", o.id, o.title);
    }

    println!("\n--- by category ---");
    println!("  {:<18} {:>5} {:>7} {:>7}", "category", "n", "acc", "MCC");
    for (cat, cm) in &by_cat {
        println!(
            "  {:<18} {:>5} {:>7.3} {:>7.3}",
            cat,
            cm.total(),
            cm.accuracy(),
            cm.mcc()
        );
    }

    if !misses.is_empty() {
        println!("\n--- false negatives (labeled relevant, judge rejected) ---");
        for o in &misses {
            println!("      [{}] {}", o.id, o.title);
        }
    }

    let cost_cents = client.estimate_cost_cents(tin, tout);
    println!("\ntokens: in={tin} out={tout}   cost ~= {cost_cents}c");

    append_result_row(
        &model,
        m,
        missing,
        conf_omitted,
        true_demotions,
        u32::try_from(false_demotions.len()).unwrap_or(u32::MAX),
        tin,
        tout,
    );

    // ── The gate ────────────────────────────────────────────────────────
    // Asserted on MCC, never on accuracy: 56 of 101 cases are labeled NOT
    // relevant, so a judge that rejects everything scores 0.55 accuracy while
    // carrying zero information. MCC 0.0 IS that judge.
    assert!(
        m.total() > 0,
        "no scenario was judged at all — the benchmark measured nothing"
    );
    assert!(
        m.mcc() >= MCC_FLOOR,
        "judge MCC {:.3} is below the {MCC_FLOOR} floor — this judge drives feed \
         demotions it can no longer justify. Compare against the trend in \
         judge-benchmark-results.jsonl before changing the floor.",
        m.mcc()
    );
}

/// Append one JSONL row per run so drift across models and prompt versions is
/// visible without re-running history. Lands next to the database (the data
/// dir is gitignored); silently skipped when the path is unavailable — a
/// bookkeeping failure must never fail the measurement.
#[allow(clippy::too_many_arguments)]
fn append_result_row(
    model: &str,
    m: Matrix,
    missing: u32,
    conf_omitted: u32,
    true_demotions: u32,
    false_demotions: u32,
    tin: u64,
    tout: u64,
) {
    use std::io::Write;
    let db_path = crate::state::get_db_path();
    let Some(dir) = db_path.parent() else {
        return;
    };
    let row = serde_json::json!({
        "at": chrono::Utc::now().to_rfc3339(),
        "model": model,
        "prompt_version": PROMPT_VERSION,
        "gate": {
            "relevance_below": DEMOTION_RELEVANCE_BELOW,
            "confidence_min": DEMOTION_CONFIDENCE_MIN,
        },
        "tp": m.tp, "fp": m.fp, "tn": m.tn, "fn": m.fn_,
        "accuracy": m.accuracy(), "precision": m.precision(),
        "recall": m.recall(), "f1": m.f1(), "mcc": m.mcc(),
        "no_judgment": missing, "confidence_absent": conf_omitted,
        "true_demotions": true_demotions, "false_demotions": false_demotions,
        "tokens_in": tin, "tokens_out": tout,
    });
    let path = dir.join("judge-benchmark-results.jsonl");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{row}");
        println!("appended result row -> {}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The headline metric must punish a majority-class voter. 47 of the 87
    /// scenarios are labeled not-relevant, so "reject everything" scores 0.54
    /// accuracy — this pins that MCC calls it what it is: zero.
    #[test]
    fn mcc_is_zero_for_a_reject_everything_judge() {
        let m = Matrix {
            tp: 0,
            fp: 0,
            tn: 47,
            fn_: 40,
        };
        assert!(m.accuracy() > 0.5, "accuracy flatters the degenerate judge");
        assert!(
            m.mcc().abs() < f64::EPSILON,
            "MCC must be 0 for a judge that carries no information"
        );
    }

    #[test]
    fn mcc_is_one_for_a_perfect_judge() {
        let m = Matrix {
            tp: 40,
            fp: 0,
            tn: 47,
            fn_: 0,
        };
        assert!((m.mcc() - 1.0).abs() < 1e-9);
    }

    /// Every labeled profile must resolve to a real context. A profile that
    /// silently degrades to the "no specific tech context" fallback would
    /// benchmark the judge against a blank user — and still look like a pass.
    #[test]
    fn every_scenario_profile_builds_a_real_context() {
        for s in load_cases() {
            if s.profile == "minimal" {
                continue; // minimal is legitimately sparse
            }
            let ctx = profile_user_context(&s.profile);
            assert!(
                ctx.contains("Tech stack:"),
                "profile {} produced no tech context: {ctx}",
                s.profile
            );
        }
    }
}
