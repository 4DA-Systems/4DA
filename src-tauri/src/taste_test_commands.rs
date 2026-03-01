//! Tauri commands for the taste test calibration flow.

use std::sync::{Mutex, OnceLock};

use crate::state::open_db_connection;
use crate::taste_test::inference::InferenceState;
use crate::taste_test::{TasteProfileSummary, TasteResponse, TasteTestStep};

type Result<T> = std::result::Result<T, String>;

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
    let mut guard = mutex.lock().map_err(|e| format!("Lock error: {e}"))?;
    *guard = Some(state);

    Ok(step)
}

/// Process a user response and return the next step.
#[tauri::command]
pub async fn taste_test_respond(item_slot: usize, response: String) -> Result<TasteTestStep> {
    let taste_response = match response.as_str() {
        "interested" => TasteResponse::Interested,
        "not_interested" => TasteResponse::NotInterested,
        "strong_interest" => TasteResponse::StrongInterest,
        _ => return Err(format!("Invalid response: {response}")),
    };

    let mutex = get_state();
    let mut guard = mutex.lock().map_err(|e| format!("Lock error: {e}"))?;
    let state = guard.as_mut().ok_or("No active taste test session")?;

    state.update(item_slot, &taste_response);
    let step = state.next_step();

    Ok(step)
}

/// Finalize the taste test: save to DB, apply to context, return summary.
#[tauri::command]
pub async fn taste_test_finalize() -> Result<TasteProfileSummary> {
    let (profile, responses, summary) = {
        let mutex = get_state();
        let mut guard = mutex.lock().map_err(|e| format!("Lock error: {e}"))?;
        let state = guard.take().ok_or("No active taste test session")?;

        let profile = state.finalize();
        let responses: Vec<(usize, TasteResponse)> = state.responses().to_vec();
        let summary = state.build_summary();

        (profile, responses, summary)
    };

    // Save to database
    let conn = open_db_connection()?;

    crate::taste_test::db::save_taste_result(&conn, &profile, &responses)?;
    crate::taste_test::db::apply_taste_to_context(&conn, &profile)?;

    // Invalidate context engine so scoring picks up new data
    crate::invalidate_context_engine();

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
