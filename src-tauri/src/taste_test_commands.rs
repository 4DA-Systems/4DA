// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tauri commands for the taste test calibration flow.

use std::sync::{Mutex, OnceLock};

use tauri::AppHandle;

use crate::error::{Result, ResultExt};
use crate::state::open_db_connection;
use crate::taste_test::inference::InferenceState;
use crate::taste_test::sprint;
use crate::taste_test::{TasteProfileSummary, TasteResponse, TasteTestStep};

/// Global inference state (lives for the duration of one taste test session).
static INFERENCE_STATE: OnceLock<Mutex<Option<InferenceState>>> = OnceLock::new();

fn get_state() -> &'static Mutex<Option<InferenceState>> {
    INFERENCE_STATE.get_or_init(|| Mutex::new(None))
}

/// Start a new taste test session. Returns the first card to show.
#[tauri::command]
pub async fn taste_test_start() -> Result<TasteTestStep> {
    let state = InferenceState::new();
    let step = state.next_step();

    let mutex = get_state();
    let mut guard = mutex.lock().context("Lock error")?;
    *guard = Some(state);

    Ok(step)
}

/// Process a user response and return the next step.
#[tauri::command]
pub async fn taste_test_respond(
    item_slot: usize,
    response: String,
    response_time_ms: Option<u64>,
) -> Result<TasteTestStep> {
    let taste_response = match response.as_str() {
        "interested" => TasteResponse::Interested,
        "not_interested" => TasteResponse::NotInterested,
        "strong_interest" => TasteResponse::StrongInterest,
        _ => return Err(format!("Invalid response: {response}").into()),
    };

    let mutex = get_state();
    let mut guard = mutex.lock().context("Lock error")?;
    let state = guard.as_mut().ok_or("No active taste test session")?;

    state.update_with_latency(item_slot, &taste_response, response_time_ms);
    let step = state.next_step();

    Ok(step)
}

/// Finalize the taste test: save to DB, apply to context, return summary.
#[tauri::command]
pub async fn taste_test_finalize(app: AppHandle) -> Result<TasteProfileSummary> {
    let (profile, responses, latencies, summary) = {
        let mutex = get_state();
        let mut guard = mutex.lock().context("Lock error")?;
        let state = guard.take().ok_or("No active taste test session")?;

        let profile = state.finalize();
        let responses: Vec<(usize, TasteResponse)> = state.responses().to_vec();
        let latencies: Vec<Option<u64>> = state.response_latencies().to_vec();
        let summary = state.build_summary();

        (profile, responses, latencies, summary)
    };

    // Save to database
    let conn = open_db_connection()?;

    crate::taste_test::db::save_taste_result(&conn, &profile, &responses, &latencies)?;
    crate::taste_test::db::apply_taste_to_context(&conn, &profile)?;

    // Embed the inferred interests so SEMANTIC interest-scoring works from the
    // first analysis. apply_taste_to_context stores them via raw INSERT with no
    // embedding column, which left a taste-test-onboarded user with a DEAD feed:
    // interest_score (the dominant signal when no projects are detected) collapses
    // to 0, every item tops out at the 1-signal ceiling (~0.23), and nothing clears
    // the 0.4 relevance gate (cold-start integrity run, 2026-06-13). The context
    // engine's add_interest upserts the same rows WITH the embedding BLOB, so this
    // is idempotent and just fills in what the sync path could not.
    if !profile.inferred_interests.is_empty() {
        let topics: Vec<String> = profile
            .inferred_interests
            .iter()
            .map(|(t, _)| t.clone())
            .collect();
        match crate::embed_texts(&topics).await {
            Ok(embeddings) => {
                if let Ok(engine) = crate::get_context_engine() {
                    for ((topic, weight), emb) in
                        profile.inferred_interests.iter().zip(embeddings.iter())
                    {
                        if let Err(e) = engine.add_interest(
                            topic,
                            *weight,
                            Some(emb.as_slice()),
                            crate::context_engine::InterestSource::Inferred,
                        ) {
                            tracing::warn!(target: "taste_test", topic = %topic, error = %e, "Failed to store inferred-interest embedding");
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(target: "taste_test", error = %e, "Failed to embed inferred interests — feed will rely on keyword matching until re-added");
            }
        }
    }

    // Bridge taste test responses into synthetic interactions so the scoring
    // pipeline's feedback_interaction_count sees real signal from day one.
    // Written to the main DB (same as context engine) because that's where
    // build_scoring_context() counts implicit interactions for bootstrap mode.
    if let Err(e) = crate::taste_test::db::generate_synthetic_feedback(&conn, &responses) {
        tracing::warn!(target: "taste_test", error = %e, "Failed to generate synthetic feedback");
    }

    // Seed continuous posterior in ACE DB from taste test persona weights
    if let Ok(ace) = crate::state::get_ace_engine() {
        let ace_conn = ace.get_conn().lock();
        if let Err(e) =
            crate::taste_test::continuous::seed_from_taste_test(&ace_conn, &profile.persona_weights)
        {
            tracing::warn!(target: "taste_test", error = %e, "Failed to seed continuous posterior");
        }
    }

    // Invalidate context engine so scoring picks up new data
    crate::invalidate_context_engine();

    // GAME: track taste test completion
    if let Ok(db) = crate::get_database() {
        for a in crate::achievement_engine::increment_counter(db, "taste_tests", 1) {
            crate::events::emit_achievement_unlocked(&app, &a);
        }
    }

    Ok(summary)
}

/// Check if any taste test has been completed.
#[tauri::command]
pub async fn taste_test_is_calibrated() -> Result<bool> {
    let conn = open_db_connection()?;
    Ok(crate::taste_test::db::is_calibrated(&conn))
}

/// Get the latest taste test profile summary.
#[tauri::command]
pub async fn taste_test_get_profile() -> Result<Option<TasteProfileSummary>> {
    let conn = open_db_connection()?;
    Ok(crate::taste_test::db::load_latest_taste_result(&conn))
}

// ============================================================================
// Review Sprint — explicit labels on real corpus items (FREE tier;
// calibration is the product promise, never gated). See taste_test/sprint.rs.
// ============================================================================

/// Sample up to 24 stratified review-sprint cards from items that have
/// unprocessed calibration samples. Returns fewer (or none) when the
/// corpus is thin — the frontend degrades honestly.
#[tauri::command]
pub async fn get_calibration_sprint_items() -> Result<Vec<sprint::CalibrationSprintCard>> {
    let conn = open_db_connection()?;
    sprint::sprint_items(&conn)
}

/// Record one sprint judgment.
///
/// - `relevant` / `not_relevant` write a `feedback` row through the
///   SAME path as `record_item_feedback` (`Database::record_feedback`)
///   — the calibration fitter treats it as unconditional ground truth.
///   A non-learning ACE interaction row (`probe_` source prefix, see
///   ace/behavior/tracking.rs) records the mechanics WITHOUT shifting
///   topic affinities / source prefs / the persona posterior, so sprint
///   labels feed the FITTER without double-counting into taste learning.
/// - `skip` writes nothing: an unsure user must not pollute ground truth.
#[tauri::command]
pub async fn record_calibration_sprint_response(
    source_item_id: i64,
    response: String,
) -> Result<()> {
    let parsed = sprint::parse_response(&response)
        .ok_or_else(|| format!("Invalid sprint response: {response}"))?;

    let relevant = match parsed {
        sprint::SprintResponse::Relevant => true,
        sprint::SprintResponse::NotRelevant => false,
        sprint::SprintResponse::Skip => return Ok(()),
    };

    let db = crate::get_database()?;
    db.record_feedback(source_item_id, relevant)
        .context("Failed to record sprint feedback")?;

    // Best-effort mechanics row; the feedback row above is the label.
    if let Ok(ace) = crate::state::get_ace_engine() {
        let action = if relevant {
            crate::ace::BehaviorAction::Save
        } else {
            crate::ace::BehaviorAction::MarkIrrelevant
        };
        if let Err(e) = ace.record_interaction(
            source_item_id,
            action,
            Vec::new(),
            "probe_calibration_sprint".to_string(),
        ) {
            tracing::warn!(
                target: "4da::taste_test::sprint",
                error = %e,
                "Sprint interaction row not recorded (feedback label still written)"
            );
        }
    }

    Ok(())
}

/// Honest progress toward the first calibration fit: distinct labeled
/// items, the fitter's real MIN_FIT_SAMPLES floor, and whether a curve
/// already exists on disk.
#[tauri::command]
pub async fn get_calibration_sprint_status() -> Result<sprint::CalibrationSprintStatus> {
    let conn = open_db_connection()?;
    let calibration_dir = crate::runtime_paths::RuntimePaths::get()
        .data_dir
        .join("calibrations");
    sprint::sprint_status(&conn, &calibration_dir)
}
