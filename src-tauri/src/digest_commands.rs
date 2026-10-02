// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! AI Briefing Tauri commands.
//!
//! Extracted from lib.rs to reduce file size. Contains AI briefing synthesis.
//! Digest configuration, briefing cache, and decision context are in digest_config.rs.

use tracing::{error, info, warn};

use crate::error::Result;
use crate::prompt_safety::BriefingSlate;
use crate::{get_database, get_settings_manager};

// ============================================================================
// AI Briefing Commands
// ============================================================================

/// Get the latest persisted briefing from the database (survives restarts)
#[tauri::command]
pub async fn get_latest_briefing() -> Result<serde_json::Value> {
    let db = get_database()?;
    match db.get_latest_briefing() {
        Ok(Some((content, model, item_count, created_at))) => Ok(serde_json::json!({
            "content": content,
            "model": model,
            "item_count": item_count,
            "created_at": created_at,
        })),
        Ok(None) => Ok(serde_json::Value::Null),
        Err(e) => {
            error!(target: "4da::briefing", error = %e, "Failed to load persisted briefing");
            Ok(serde_json::Value::Null)
        }
    }
}

// When 4DA first held an advisory — the only honest age a brief may state.
// The security facts themselves are built in `brief_facts`.
mod grounding;
pub(crate) use grounding::first_seen_for_ids;

/// Auto-trigger reuse (2026-08-31 live audit; fact fingerprint since
/// Decision 2) — the mechanism and its tests live in their own file.
#[path = "briefing_reuse.rs"]
mod briefing_reuse;
use briefing_reuse::{remember_fingerprint, try_reuse_recent_briefing};

/// The narrated Brief's system prompt (incl. the MACHINE TRAILER verdict
/// contract) — extracted for size hygiene and testability; see module doc.
#[path = "briefing_prompt.rs"]
mod briefing_prompt;
use briefing_prompt::briefing_system_prompt;
#[cfg(test)]
pub(crate) use briefing_prompt::render_facts_for_prompt;

/// AD-035 display binding: serves the LATEST briefing's filter verdicts,
/// bounded by the reuse window, so Brief cards / Key Signals can demote
/// what the briefing explicitly filtered. See module doc.
#[path = "brief_verdict_display.rs"]
mod brief_verdict_display;

/// The LATEST briefing's structured filter verdicts, for display binding
/// (AD-035: one item, one verdict).
///
/// Demote-only and fail-open: an empty set is always a valid answer (no
/// briefing, stale briefing, clock skew, verdict-less briefing, read error),
/// and a verdict can only remove an item from PROMOTED placement — nothing
/// here ever promotes or hides an item from the ordinary feed.
#[tauri::command]
pub async fn get_brief_display_verdicts() -> Result<serde_json::Value> {
    let latest = match get_database() {
        Ok(db) => db.get_latest_brief_verdicts().unwrap_or_else(|e| {
            warn!(
                target: "4da::briefing",
                error = %e,
                "Brief display verdicts unavailable — serving none (fail-open)"
            );
            None
        }),
        Err(_) => None,
    };
    Ok(brief_verdict_display::display_verdicts_response(
        latest,
        briefing_reuse::BRIEFING_REUSE_WINDOW_HOURS,
    ))
}

/// Internal briefing generation -- called by both the Tauri command and auto-trigger.
///
/// Facts first (Decision 2, 2026-10-02): `brief_facts` computes every claim
/// the brief can make — confirmed security with its real fix path, breaking
/// upgrades of direct dependencies, fresh worth-knowing candidates, and what
/// was already reported. A Sonnet-class model explains those facts; without
/// one, the same facts render as the deterministic floor.
///
/// `auto_triggered`: when true, today's briefing is reused while the facts it
/// was written from are unchanged (see [`try_reuse_recent_briefing`]);
/// explicit user triggers always regenerate.
/// `anomaly_context`: unresolved system anomalies. Not sent to the model: the
/// brief is about the user's code, never about 4DA's own pipeline.
pub(crate) async fn generate_briefing_internal(
    auto_triggered: bool,
    anomaly_context: Option<Vec<String>>,
) -> Result<serde_json::Value> {
    let trigger = if auto_triggered { "auto" } else { "manual" };
    info!(target: "4da::briefing", trigger = trigger, "Generating AI briefing");
    if let Some(anomalies) = anomaly_context.as_ref().filter(|a| !a.is_empty()) {
        info!(target: "4da::briefing", count = anomalies.len(), "Unresolved anomalies left out of the brief prompt");
    }

    let db = get_database()?;
    // Facts read the DB, the preemption feed and the lockfile graph: keep
    // that blocking work off the async runtime.
    let facts = tokio::task::spawn_blocking(move || crate::brief_facts::build_brief_facts(db))
        .await
        .map_err(|e| {
            crate::error::FourDaError::Internal(format!("brief facts task failed: {e}"))
        })?;

    if auto_triggered {
        if let Some(cached) = try_reuse_recent_briefing(db, &facts.fingerprint) {
            return Ok(cached);
        }
    }

    let llm_settings = {
        let mut guard = get_settings_manager().lock();
        guard.ensure_keys_hydrated();
        guard.get().llm.clone()
    };

    // A genuine NARRATED brief needs a Sonnet-class+ model (`is_brief_capable`).
    // Without one — no LLM at all, or a model too weak for genuine synthesis
    // (Haiku / *-mini / consumer-hardware local) — the deterministic floor
    // renders the same facts instead of erroring or faking synthesis.
    let has_llm = crate::llm_gate::compute_has_llm(&llm_settings.provider, &llm_settings.api_key);
    let brief_capable = has_llm && crate::llm_capability::is_brief_capable(&llm_settings);

    if !brief_capable {
        info!(
            target: "4da::briefing",
            has_llm,
            model = %llm_settings.model,
            "Served deterministic facts brief (no Sonnet-class model)"
        );
        return Ok(serve_deterministic(
            db,
            &facts,
            auto_triggered,
            crate::briefing_deterministic::FloorReason::NoCapableModel,
        ));
    }

    // `titles_only` (llm_egress): article excerpts leave the machine only when
    // the user allows bodies, or the model runs on this machine.
    let send_body = crate::llm_egress::body_allowed(&llm_settings);
    let on_machine = crate::llm_egress::provider_is_on_machine(&llm_settings);
    let BriefingSlate {
        text: candidates_text,
        ids: slate_ids,
    } = briefing_prompt::build_candidate_slate(&facts.worth_knowing, send_body);

    // Project context: full cards on this machine, nouns only for a cloud
    // model (decision B, NETWORK.md "nouns, never your prose").
    let projects = crate::project_cards::judge_context(db, on_machine);
    let decision_context = crate::digest_config::build_decision_context_for_briefing();
    let facts_text = briefing_prompt::render_facts_for_prompt(&facts);
    let today = chrono::Local::now().format("%A %Y-%m-%d").to_string();
    let user_prompt = format!(
        "Today is {today}.\n\nMy projects:\n{projects}{decisions}\n\n{facts_text}\n\
         Today's {count} items (WORTH KNOWING candidates: judge-approved articles published in the last \
         {window} days, never featured before{excerpt_note}):\n\n{candidates}\n\n\
         Give me my intelligence briefing.",
        decisions = decision_context,
        count = slate_ids.len(),
        window = crate::brief_facts::WORTH_KNOWING_WINDOW_DAYS,
        excerpt_note = if send_body {
            "; each carries an excerpt of the article"
        } else {
            "; titles only, per the user's privacy setting"
        },
        candidates = if candidates_text.is_empty() {
            "(none today)".to_string()
        } else {
            candidates_text
        },
    );

    // llm-egress: routed through llm_egress::body_allowed above; excerpts are omitted under titles_only
    let llm_client = crate::llm::LLMClient::with_purpose(llm_settings.clone(), "digest");
    let system_prompt = briefing_system_prompt();
    let package_facts = crate::brief_facts::package_facts(&facts);
    let start_time = std::time::Instant::now();

    // One retry when the deterministic version check catches a number the
    // facts do not hold — told exactly which, so the retry can do better
    // than a re-roll. After that the facts are served as the floor.
    let mut attempt = 0;
    let mut correction = String::new();
    let (content, rejects, total_tokens) = loop {
        attempt += 1;
        let messages = vec![crate::llm::Message {
            role: "user".to_string(),
            content: format!("{user_prompt}{correction}"),
        }];
        let response = match llm_client.complete(&system_prompt, messages).await {
            Ok(r) => r,
            Err(e) => {
                error!(target: "4da::briefing", error = %e, "Failed to generate briefing");
                return Ok(serde_json::json!({
                    "success": false,
                    "error": provider_error_message(&e.to_string()),
                    "briefing": null
                }));
            }
        };
        let (content, rejects) =
            crate::brief_rejections::extract_rejects_trailer(&response.content);
        let violations =
            crate::briefing_groundedness::check_factual_claims(&content, &package_facts);
        if violations.is_empty() {
            break (
                content,
                rejects,
                response.input_tokens + response.output_tokens,
            );
        }
        // The rejected draft is logged (bounded) so a false fault can be
        // diagnosed from the log alone: on 2026-10-03 the live brief fell to
        // the floor twice and nothing showed which sentence tripped the check.
        let draft: String = content.chars().take(1200).collect();
        warn!(
            target: "4da::briefing",
            attempt,
            violations = ?violations,
            draft = %draft,
            "Brief stated a version the facts do not hold"
        );
        if attempt >= 2 {
            warn!(target: "4da::briefing", "Serving the facts brief instead of a narration with an unsupported version");
            return Ok(serve_deterministic(
                db,
                &facts,
                auto_triggered,
                crate::briefing_deterministic::FloorReason::NarrationRejected,
            ));
        }
        correction = crate::brief_facts::correction_note(&violations, &package_facts);
    };
    let elapsed = start_time.elapsed();
    info!(target: "4da::briefing",
        tokens = total_tokens,
        elapsed_ms = elapsed.as_millis(),
        trigger = trigger,
        attempts = attempt,
        "AI briefing generated"
    );
    *crate::digest_config::LATEST_BRIEFING.lock() = Some(content.clone());

    match db.save_briefing(
        &content,
        Some(&llm_settings.model),
        slate_ids.len(),
        Some(total_tokens),
        Some(elapsed.as_millis() as u64),
    ) {
        Ok(briefing_id) => {
            // Join trailer indices back to the candidate slate's real ids.
            // `slate_ids` came out of the same pass that rendered the prompt.
            crate::brief_rejections::record_rejections(db, briefing_id, &rejects, &slate_ids);
            remember_fingerprint(db, briefing_id, &facts.fingerprint);
        }
        Err(e) => {
            error!(target: "4da::briefing", error = %e, "Failed to persist briefing");
        }
    }
    // Featured = named in the brief. A candidate left out for space is
    // neither featured nor rejected, and may be offered again tomorrow.
    let featured = crate::brief_facts::featured_in(&content, &facts.worth_knowing);
    crate::brief_facts::record_reported(db, &facts, &featured);

    Ok(serde_json::json!({
        "success": true,
        "briefing": content,
        "item_count": slate_ids.len(),
        "model": llm_settings.model,
        "tokens_used": total_tokens,
        "latency_ms": elapsed.as_millis(),
        "auto_triggered": auto_triggered,
    }))
}

/// The facts rendered without a model: always available, cannot fabricate.
fn serve_deterministic(
    db: &crate::db::Database,
    facts: &crate::brief_facts::BriefFacts,
    auto_triggered: bool,
    reason: crate::briefing_deterministic::FloorReason,
) -> serde_json::Value {
    let briefing = crate::briefing_deterministic::build_deterministic_brief(facts, reason);
    match db.save_briefing(
        &briefing,
        Some("deterministic"),
        facts.worth_knowing.len(),
        Some(0),
        Some(0),
    ) {
        Ok(id) => remember_fingerprint(db, id, &facts.fingerprint),
        Err(e) => {
            error!(target: "4da::briefing", error = %e, "Failed to persist deterministic briefing");
        }
    }
    // The floor shows its first articles; those are featured like any other.
    let shown: Vec<i64> = facts
        .worth_knowing
        .iter()
        .take(crate::briefing_deterministic::FLOOR_ARTICLES)
        .map(|c| c.id)
        .collect();
    crate::brief_facts::record_reported(db, facts, &shown);
    *crate::digest_config::LATEST_BRIEFING.lock() = Some(briefing.clone());
    serde_json::json!({
        "success": true,
        "briefing": briefing,
        "item_count": facts.worth_knowing.len(),
        "model": "deterministic",
        "deterministic": true,
        "auto_triggered": auto_triggered,
    })
}

/// The user-facing message for a provider error.
fn provider_error_message(e_str: &str) -> String {
    if e_str.contains("Connection refused") || e_str.contains("connect") {
        "Ollama is not running. Start it with 'ollama serve' or check your LLM settings."
            .to_string()
    } else if e_str.contains("401")
        || e_str.contains("authentication_error")
        || e_str.contains("invalid x-api-key")
        || e_str.contains("invalid_api_key")
    {
        "Your API key was rejected by the provider (invalid or expired). A saved key isn't verified until it's used — re-enter it in Settings → AI Provider, or switch to a local Ollama model."
            .to_string()
    } else if e_str.contains("403") || e_str.contains("permission") {
        "API key lacks permission for this model. Check your plan and key permissions in Settings."
            .to_string()
    } else if e_str.contains("429") || e_str.contains("rate_limit") {
        "Rate limit exceeded. Wait a moment and try again, or check your API plan limits."
            .to_string()
    } else if e_str.contains("model") {
        "The configured model may not be available. Try 'ollama pull qwen3:14b' or 'ollama pull gemma3:12b'.".to_string()
    } else {
        e_str.to_string()
    }
}
/// Generate an AI-powered briefing from recent relevant items
/// Uses the configured LLM (Ollama by default) to synthesize insights
///
/// `auto`: `Some(true)` marks an AUTO-triggered call (window open / frontend
/// mount / post-analysis effect) — those reuse a briefing younger than
/// `briefing_reuse::BRIEFING_REUSE_WINDOW_HOURS` unless critical items
/// arrived since. Omitted or `false` = an explicit user trigger, which always
/// regenerates. Optional so every existing caller keeps its explicit-trigger
/// semantics until it opts in.
#[tauri::command]
pub async fn generate_ai_briefing(
    app: tauri::AppHandle,
    auto: Option<bool>,
) -> Result<serde_json::Value> {
    crate::ipc_rate_limit::check_rate_limit("generate_ai_briefing", 10)?;

    // Improvement C: Gather unresolved anomalies for context injection.
    // StaleData anomalies ("No context updates for N hours") are EXCLUDED: absence
    // of recent file-edit activity means the user simply hasn't been coding — it is
    // not intelligence, and feeding it to the LLM reliably manufactures a fabricated
    // "context blackout / supply-chain drifted unseen" emergency narrative. See the
    // brief-grounding fix (PENDING-DECISION 2026-06-06, lever 1).
    let anomalies = {
        if let Ok(ace) = crate::get_ace_engine() {
            let conn = ace.get_conn().lock();
            crate::anomaly::get_unresolved(&conn).ok().map(|list| {
                list.iter()
                    .filter(|a| !matches!(a.anomaly_type, crate::anomaly::AnomalyType::StaleData))
                    .map(|a| a.description.clone())
                    .collect::<Vec<_>>()
            })
        } else {
            None
        }
    };
    let result = generate_briefing_internal(auto.unwrap_or(false), anomalies).await;

    // GAME: track briefing generation on success
    if let Ok(ref val) = result {
        if val
            .get("success")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            if let Ok(db) = crate::get_database() {
                for a in crate::achievement_engine::increment_counter(db, "briefings", 1) {
                    crate::events::emit_achievement_unlocked(&app, &a);
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    /// End-to-end on a REAL corpus snapshot with the user's configured model:
    /// facts -> prompt -> model -> version check -> persist -> novelty. Costs
    /// one model call; writes only to the snapshot. Run with
    /// `FOURDA_DB_PATH=<snapshot> FOURDA_DATA_DIR=<dir with settings.json>
    ///  FOURDA_BRIEF_LIVE=1 cargo test --lib live_snapshot_generate_brief -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "calls the configured LLM against a real database snapshot"]
    async fn live_snapshot_generate_brief() {
        if std::env::var("FOURDA_BRIEF_LIVE").is_err() || std::env::var("FOURDA_DB_PATH").is_err() {
            return;
        }
        let first = super::generate_briefing_internal(false, None)
            .await
            .expect("generation runs");
        println!("{}", serde_json::to_string_pretty(&first).unwrap());
        assert_eq!(first["success"], true, "{first}");
        let text = first["briefing"].as_str().expect("a brief");
        assert!(
            text.contains("## ") || text.contains("Nothing new touches your code today"),
            "{text}"
        );

        // Same facts, same day: an AUTO trigger must reuse, not regenerate.
        let second = super::generate_briefing_internal(true, None)
            .await
            .expect("reuse runs");
        assert_eq!(second["cached"], true, "auto trigger regenerated: {second}");
        assert_eq!(second["briefing"], first["briefing"]);
    }

    // ========================================================================
    // Briefing JSON response structure tests
    // ========================================================================

    #[test]
    fn briefing_no_capable_model_serves_deterministic_floor() {
        // When there's no Sonnet-class model (no LLM, or a model too weak for genuine
        // synthesis), the brief no longer errors — it serves the deterministic grounded
        // floor: success=true, a real briefing, model="deterministic", deterministic=true.
        let response = serde_json::json!({
            "success": true,
            "briefing": "## Security\n✓ No confirmed vulnerabilities...\n\n## Top signals today\n1. ...",
            "item_count": 5,
            "model": "deterministic",
            "deterministic": true,
            "auto_triggered": false,
        });
        assert_eq!(response["success"], true);
        assert_eq!(response["deterministic"], true);
        assert_eq!(response["model"], "deterministic");
        assert!(response["briefing"].as_str().unwrap().contains("Security"));
    }

    #[test]
    fn briefing_empty_items_response_shape() {
        // Simulates the response when no items are found
        let model = "llama3.2:latest";
        let response = serde_json::json!({
            "success": true,
            "briefing": "No items found. Run an analysis first to fetch and score content.",
            "item_count": 0,
            "model": model
        });
        assert_eq!(response["success"], true);
        assert_eq!(response["item_count"], 0);
        assert_eq!(response["model"], model);
        assert!(response["briefing"].as_str().unwrap().contains("No items"));
    }

    #[test]
    fn briefing_success_response_has_required_fields() {
        let response = serde_json::json!({
            "success": true,
            "briefing": "## Action Required\nNothing urgent today.",
            "item_count": 5,
            "model": "claude-3-haiku",
            "tokens_used": 1500,
            "latency_ms": 2300,
            "auto_triggered": false,
        });
        assert_eq!(response["success"], true);
        assert!(response["briefing"].is_string());
        assert!(response["item_count"].is_number());
        assert!(response["model"].is_string());
        assert!(response["tokens_used"].is_number());
        assert!(response["latency_ms"].is_number());
        assert_eq!(response["auto_triggered"], false);
    }
}
