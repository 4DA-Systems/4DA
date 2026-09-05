// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Live-sample judge — the shipped judge over a caller-chosen slice of the
//! REAL corpus, reported per source class.
//!
//! `judge_benchmark` answers "is the judge any good?" against labels. This
//! answers a different question with the same instrument: "is the PIPELINE's
//! precision different for one source class than another?" — using the judge
//! as a PROXY label (MCC 0.728 against human labels at prompt v6, so read
//! every number below as a judge's opinion, never as ground truth). Built for
//! the 2026-09-06 question "dev.to is 39% of the Signal feed; is it earning
//! it?" (`.claude/plans/HANDOFF-2026-09-06-scoring-v30-followthrough.md`,
//! task 4), which the doctrine answers with a measurement before any rule.
//!
//! What it does, for every item id the caller passes:
//!   * loads the row through the production loader
//!     (`llm_judgments::load_items_for_judgment`), renders it through the
//!     shipped `format_items_block`, judges it under the shipped
//!     `judge_system_prompt` built from the REAL ACE context
//!     (`adversarial::build_user_context_summary`, asserted non-generic) and
//!     parses the reply with the shipped parser — never a copy of any of them;
//!   * presents every item at the neutral pipeline score, as the benchmark
//!     does, so the judge is not anchored on the verdict being measured;
//!   * prints one line per item and, per `source_type` and per stratum
//!     (`devto` against everything else), the confusion matrix of the
//!     PIPELINE's verdict (`feed_relevant`) scored against the judge's
//!     (`relevance >= DEMOTION_RELEVANCE_BELOW`): precision, recall, MCC and
//!     the raw counts.
//!
//! The caller chooses the slice — a stratified, seeded sample drawn from a
//! SNAPSHOT of the corpus, never the live database — so a run is reproducible
//! and never writes to the corpus. It spends money (roughly one cent per ten
//! items on the Haiku judge sibling), so it is opt-in through
//! `FOURDA_JUDGE_SAMPLE_IDS` and can never run as a side effect of `cargo test`.
//!
//! ```text
//! FOURDA_JUDGE_SAMPLE_IDS="12,345,678" FOURDA_DB_PATH=<snapshot>/4da.db \
//!     cargo test --lib judge_live_sample -- --ignored --nocapture
//! ```
//! `<snapshot>/settings.json` supplies the provider key; it is only read.

use std::collections::BTreeMap;

use super::judge_benchmark::{Matrix, NEUTRAL_PIPELINE_SCORE};
use crate::llm::{LLMClient, Message};
use crate::llm_judgments::{
    format_items_block, judge_system_prompt, load_items_for_judgment, parse_batch_response,
    BATCH_SIZE, DEMOTION_RELEVANCE_BELOW, PROMPT_VERSION,
};

/// One sampled row as the corpus holds it, plus the judge's answer.
struct Sampled {
    id: i64,
    source_type: String,
    feed_relevant: bool,
    pipeline_score: f64,
    title: String,
    /// `None` when the model dropped the item from its reply.
    judged: Option<(f64, Option<f64>)>,
}

/// The two-way split the 2026-09-06 question is about.
fn stratum(source_type: &str) -> &'static str {
    if source_type == "devto" {
        "devto"
    } else {
        "other"
    }
}

fn print_table(entries: &[(String, Matrix)]) {
    println!(
        "  {:<14} {:>4} {:>6} {:>7} {:>7} {:>7} {:>7}   TP/FP/FN/TN",
        "class", "n", "feed_n", "prec", "recall", "acc", "MCC"
    );
    for (name, m) in entries {
        println!(
            "  {:<14} {:>4} {:>6} {:>7.3} {:>7.3} {:>7.3} {:>7.3}   {}/{}/{}/{}",
            name,
            m.total(),
            m.tp + m.fp,
            m.precision(),
            m.recall(),
            m.accuracy(),
            m.mcc(),
            m.tp,
            m.fp,
            m.fn_,
            m.tn
        );
    }
}

#[tokio::test]
#[ignore = "spends money: set FOURDA_JUDGE_SAMPLE_IDS to run"]
async fn judge_live_sample() {
    let Ok(raw) = std::env::var("FOURDA_JUDGE_SAMPLE_IDS") else {
        eprintln!("FOURDA_JUDGE_SAMPLE_IDS unset — not spending money, nothing measured");
        return;
    };
    let ids: Vec<i64> = raw
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    assert!(
        !ids.is_empty(),
        "FOURDA_JUDGE_SAMPLE_IDS carried no numeric ids"
    );

    // Same routing production uses: the configured provider, downgraded to the
    // cheap judge sibling.
    let provider = {
        let mgr = crate::get_settings_manager();
        let mut guard = mgr.lock();
        guard.ensure_keys_hydrated();
        crate::llm_judge::judge_provider(&guard.get().llm)
    };
    assert!(
        crate::llm_gate::compute_has_llm(&provider.provider, &provider.api_key),
        "no LLM provider configured — nothing measured"
    );
    let model = provider.model.clone();
    let client = LLMClient::with_purpose(provider, "judge_sample");

    let db_path = crate::state::get_db_path();
    let db = crate::db::Database::new(&db_path).expect("open the sample database");

    // The REAL user context — and loudly not the generic fallback, which would
    // judge the sample against a blank user and still print numbers.
    let user_context = crate::adversarial::build_user_context_summary();
    assert!(
        user_context.contains("Tech stack:"),
        "ACE context did not resolve from {}: {user_context}",
        db_path.display()
    );
    let system_prompt = judge_system_prompt(&user_context);

    // The pipeline's verdict and stored score per id, straight from the corpus.
    let mut meta: BTreeMap<i64, (String, bool, f64, String)> = BTreeMap::new();
    {
        let conn = db.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT source_type, COALESCE(feed_relevant, 0), COALESCE(relevance_score, 0.0), title
                 FROM source_items WHERE id = ?1",
            )
            .expect("prepare the metadata query");
        for id in &ids {
            let row = stmt.query_row(rusqlite::params![id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)? != 0,
                    r.get::<_, f64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            });
            if let Ok(row) = row {
                meta.insert(*id, row);
            }
        }
    }

    println!("\n=== JUDGE LIVE SAMPLE ===");
    println!("model          : {model}");
    println!("prompt_version : {PROMPT_VERSION}");
    println!("database       : {}", db_path.display());
    println!("ids requested  : {}   found: {}", ids.len(), meta.len());
    println!("user context   : {}", user_context.replace('\n', " | "));

    let mut rows: Vec<Sampled> = Vec::new();
    let (mut tin, mut tout) = (0u64, 0u64);
    for chunk in ids.chunks(BATCH_SIZE) {
        let mut items = load_items_for_judgment(&db, chunk).expect("load the sampled items");
        if items.is_empty() {
            continue;
        }
        for item in &mut items {
            item.relevance_score = NEUTRAL_PIPELINE_SCORE;
        }
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
                        eprintln!("  !! batch parse failed: {e}");
                        Vec::new()
                    }
                }
            }
            Err(e) => {
                eprintln!("  !! batch call failed: {e}");
                Vec::new()
            }
        };
        for item in &items {
            let Some((source_type, feed_relevant, pipeline_score, title)) = meta.get(&item.id)
            else {
                continue;
            };
            let judged = parsed
                .iter()
                .find(|(id, _)| *id == item.id)
                .map(|(_, r)| (r.relevance.unwrap_or(0.0).clamp(0.0, 1.0), r.confidence));
            rows.push(Sampled {
                id: item.id,
                source_type: source_type.clone(),
                feed_relevant: *feed_relevant,
                pipeline_score: *pipeline_score,
                title: title.chars().take(64).collect(),
                judged,
            });
        }
    }

    // ── Per item ────────────────────────────────────────────────────────
    println!(
        "\n--- items (pipe = feed verdict / stored score; judge = relevance / confidence) ---"
    );
    for r in &rows {
        let (jr, jc) = match r.judged {
            Some((rel, conf)) => (
                format!("{rel:.2}"),
                conf.map_or_else(|| "-".to_string(), |c| format!("{c:.2}")),
            ),
            None => ("none".to_string(), "-".to_string()),
        };
        println!(
            "  {:>6} {:<12} pipe {}/{:.3}  judge {}/{}  {}",
            r.id,
            r.source_type,
            u8::from(r.feed_relevant),
            r.pipeline_score,
            jr,
            jc,
            r.title
        );
    }

    // ── Per source class ───────────────────────────────────────────────
    // Rows are the PIPELINE's verdict, the judge is the label. Positive =
    // relevant, so `precision` is the share of feed items the judge agrees
    // with — the proxy for "is this source earning its feed share" — and
    // `recall` is over the caller's near-line non-feed rows, a deliberately
    // hard negative sample, not the corpus.
    let mut by_source: BTreeMap<String, Matrix> = BTreeMap::new();
    let mut by_stratum: BTreeMap<&'static str, Matrix> = BTreeMap::new();
    let mut missing = 0usize;
    for r in &rows {
        let Some((rel, _)) = r.judged else {
            missing += 1;
            continue;
        };
        let judge_relevant = rel >= DEMOTION_RELEVANCE_BELOW;
        let source = by_source.entry(r.source_type.clone()).or_default();
        let strat = by_stratum.entry(stratum(&r.source_type)).or_default();
        for m in [source, strat] {
            match (r.feed_relevant, judge_relevant) {
                (true, true) => m.tp += 1,
                (true, false) => m.fp += 1,
                (false, true) => m.fn_ += 1,
                (false, false) => m.tn += 1,
            }
        }
    }

    println!(
        "\n--- by stratum (pipeline verdict scored against the judge; positive = relevant) ---"
    );
    let strata: Vec<(String, Matrix)> = by_stratum
        .iter()
        .map(|(k, v)| ((*k).to_string(), *v))
        .collect();
    print_table(&strata);
    println!("\n--- by source_type ---");
    let sources: Vec<(String, Matrix)> = by_source.iter().map(|(k, v)| (k.clone(), *v)).collect();
    print_table(&sources);
    println!("\n  no judgment: {missing}");
    let cost_cents = client.estimate_cost_cents(tin, tout);
    println!("tokens: in={tin} out={tout}   cost ~= {cost_cents}c");

    assert!(
        !rows.is_empty() && missing < rows.len(),
        "the judge returned nothing — measured nothing"
    );
}
