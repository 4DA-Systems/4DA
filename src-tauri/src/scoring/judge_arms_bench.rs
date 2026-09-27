// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Judge arms bench: the SHIPPED ingest judge (prompt v7, project cards) over
//! a caller-chosen slice of a corpus snapshot, once per judge the product can
//! route to. Built for the 2026-09-27 question: the judge gate (#752) admits
//! low-yield sources on a card-aware judgment >= 0.5, and was validated only
//! with gemma4:26b and full cards. A user without a big GPU is judged by the
//! cloud sibling on nouns-only cards. Does the gate keep their useful items?
//!
//! Per item it runs production's path, never a copy: `load_items_for_judgment`,
//! `withhold_judgment_bodies` under the provider's egress rule,
//! `project_cards::judge_context` (full cards on the machine, nouns in the
//! cloud), `judge_system_prompt`, `format_items_block` with the stored pipeline
//! score, `parse_batch_response`, and `judge_items_per_call` for the batch size.
//!
//! Arms (`FOURDA_ARM`):
//! - `cloud_nouns`: `llm_judge::judge_provider` of the configured model (the
//!   cloud sibling), nouns-only cards, as a user without a local judge gets.
//! - `cloud_tax`: the same plus a `Kind:` line per project, closed-vocabulary
//!   terms from `FOURDA_TAX_TERMS` (a JSON map of project name to terms).
//! - `local:<model>`: that Ollama model, full cards, one item per call.
//! - `print_ctx`: print both contexts and stop (spends nothing).
//!
//! ```text
//! FOURDA_DATA_DIR=<snapshot> FOURDA_DB_PATH=<snapshot>/4da.db \
//! FOURDA_JUDGE_SAMPLE_IDS_FILE=ids.txt FOURDA_ARM=cloud_nouns FOURDA_ARM_OUT=out.json \
//!     cargo test --lib judge_arms_bench -- --ignored --nocapture
//! ```
//! Resumable: ids already in `FOURDA_ARM_OUT` are skipped. Writes only that file.

use std::collections::BTreeMap;

use crate::llm::{LLMClient, Message};
use crate::llm_judgments::{
    format_items_block, judge_items_per_call, judge_system_prompt, load_items_for_judgment,
    parse_batch_response,
};

/// Insert a `Kind:` line under each project header the terms map names.
fn with_kinds(context: &str, terms: &BTreeMap<String, Vec<String>>) -> String {
    let mut out = String::new();
    for line in context.lines() {
        out.push_str(line);
        out.push('\n');
        if let Some(name) = line.strip_prefix("Project ") {
            let name = name.split(':').next().unwrap_or(name).trim();
            if let Some(kinds) = terms.get(name).filter(|k| !k.is_empty()) {
                out.push_str(&format!("  Kind: {}.\n", kinds.join(", ")));
            }
        }
    }
    out
}

fn arm_provider(arm: &str) -> crate::settings::LLMProvider {
    let base = {
        let mgr = crate::get_settings_manager();
        let mut guard = mgr.lock();
        guard.ensure_keys_hydrated();
        guard.get().llm.clone()
    };
    match arm.strip_prefix("local:") {
        Some(model) => {
            let mut p = base;
            p.provider = "ollama".into();
            p.api_key = String::new();
            p.model = model.into();
            p.base_url = Some(crate::local_judge::DEFAULT_OLLAMA_URL.into());
            p
        }
        None => {
            let p = crate::llm_judge::judge_provider(&base);
            assert_ne!(
                p.provider, "ollama",
                "a cloud arm must route to the cloud sibling"
            );
            p
        }
    }
}

#[tokio::test]
#[ignore = "spends money or GPU time: set FOURDA_ARM to run"]
async fn judge_arms_bench() {
    let Ok(arm) = std::env::var("FOURDA_ARM") else {
        eprintln!("FOURDA_ARM unset: nothing measured");
        return;
    };
    let db = crate::db::Database::new(&crate::state::get_db_path()).expect("open the snapshot");

    if arm == "print_ctx" {
        println!(
            "=== CLOUD CONTEXT ===\n{}",
            crate::project_cards::judge_context(&db, false)
        );
        println!(
            "=== LOCAL CONTEXT ===\n{}",
            crate::project_cards::judge_context(&db, true)
        );
        return;
    }

    let ids: Vec<i64> = std::fs::read_to_string(
        std::env::var("FOURDA_JUDGE_SAMPLE_IDS_FILE").expect("FOURDA_JUDGE_SAMPLE_IDS_FILE"),
    )
    .expect("read the id file")
    .split(|c: char| c == ',' || c.is_whitespace())
    .filter_map(|s| s.trim().parse().ok())
    .collect();
    let out_path = std::env::var("FOURDA_ARM_OUT").expect("FOURDA_ARM_OUT");
    let mut out: BTreeMap<String, serde_json::Value> = std::fs::read_to_string(&out_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let provider = arm_provider(&arm);
    let per_call = judge_items_per_call(&provider).expect("the arm's model holds the judging bar");
    let on_machine = crate::llm_egress::provider_is_on_machine(&provider);
    let send_body = crate::llm_egress::body_allowed(&provider);
    let mut context = crate::project_cards::judge_context(&db, on_machine);
    if arm == "cloud_tax" {
        let terms: BTreeMap<String, Vec<String>> = serde_json::from_str(
            &std::fs::read_to_string(std::env::var("FOURDA_TAX_TERMS").expect("FOURDA_TAX_TERMS"))
                .expect("read the terms file"),
        )
        .expect("terms JSON");
        context = with_kinds(&context, &terms);
    }
    let system_prompt = judge_system_prompt(&context);
    println!(
        "arm={arm} model={} per_call={per_call} on_machine={on_machine} bodies={send_body} ids={}",
        provider.model,
        ids.len()
    );
    println!("context:\n{context}");
    // llm-egress: exempt developer measurement on a corpus snapshot; it applies the shipped egress rule itself
    let client = LLMClient::with_purpose(provider, "judge_arms_bench");

    let todo: Vec<i64> = ids
        .into_iter()
        .filter(|id| !out.contains_key(&id.to_string()))
        .collect();
    let started = std::time::Instant::now();
    for chunk in todo.chunks(per_call) {
        let mut items = load_items_for_judgment(&db, chunk).expect("load items");
        if items.is_empty() {
            continue;
        }
        crate::llm_egress::withhold_judgment_bodies(send_body, &mut items);
        let messages = vec![Message {
            role: "user".to_string(),
            content: format!("Evaluate these items:\n\n{}", format_items_block(&items)),
        }];
        let parsed = match client.complete(&system_prompt, messages).await {
            Ok(resp) => parse_batch_response(&resp.content).unwrap_or_default(),
            Err(e) => {
                eprintln!("  !! call failed: {e}");
                continue;
            }
        };
        for (id, r) in parsed {
            out.insert(
                id.to_string(),
                serde_json::json!({
                    "relevance": r.relevance,
                    "confidence": r.confidence,
                    "explanation": r.explanation,
                }),
            );
        }
        std::fs::write(&out_path, serde_json::to_string(&out).expect("json")).expect("write out");
    }
    println!(
        "done: {} judged in total, {:.0} s this run",
        out.len(),
        started.elapsed().as_secs_f64()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_follow_their_project_header() {
        let ctx = "The developer works on these projects:\n\nProject 4da\n  Languages: Rust.\n\nProject navcal: A calendar.\n  Languages: TS.\n";
        let terms = BTreeMap::from([
            (
                "4da".to_string(),
                vec!["desktop app".to_string(), "news feed".to_string()],
            ),
            ("navcal".to_string(), vec!["web app".to_string()]),
        ]);
        let out = with_kinds(ctx, &terms);
        assert!(out.contains("Project 4da\n  Kind: desktop app, news feed.\n  Languages: Rust."));
        assert!(out.contains("Project navcal: A calendar.\n  Kind: web app.\n"));
    }
}
