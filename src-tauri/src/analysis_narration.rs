//! Analysis Narration — human-readable summaries of scoring decisions.
//!
//! Provides narrative explanations for why items were scored the way they were,
//! making the PASIFA algorithm transparent to the user.

use serde::Serialize;
use tauri::Emitter;

#[derive(Serialize, Clone, Debug)]
pub struct NarrationEvent {
    pub narration_type: String, // "discovery" | "match" | "insight"
    pub message: String,
    pub source: Option<String>,
    pub relevance: Option<f32>,
}

pub fn emit_narration(app: &tauri::AppHandle, event: NarrationEvent) {
    if let Err(e) = app.emit("analysis-narration", &event) {
        tracing::warn!("Failed to emit 'analysis-narration': {e}");
    }
}
