//! Tauri commands for the Stack Intelligence System.
//!
//! 5 commands for managing stack profiles from the frontend:
//! - List available profiles
//! - Get/set user selections
//! - Auto-detect from ACE context
//! - Get composed stack summary (for debugging/UI)

use serde::Serialize;

use crate::stacks;

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct StackProfileSummary {
    pub id: String,
    pub name: String,
    pub core_tech: Vec<String>,
    pub companions: Vec<String>,
    pub competing: Vec<String>,
    pub pain_point_count: usize,
    pub ecosystem_shift_count: usize,
}

#[derive(Debug, Serialize)]
pub struct StackDetectionResult {
    pub profile_id: String,
    pub profile_name: String,
    pub confidence: f32,
    pub matched_tech: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ComposedStackSummary {
    pub active: bool,
    pub pain_point_count: usize,
    pub ecosystem_shift_count: usize,
    pub keyword_boost_count: usize,
    pub source_preferences: Vec<(String, f32)>,
    pub all_tech: Vec<String>,
    pub competing: Vec<String>,
}

// ============================================================================
// Commands
// ============================================================================

/// List all available stack profiles.
#[tauri::command]
pub fn get_stack_profiles() -> Vec<StackProfileSummary> {
    stacks::list_profiles()
        .iter()
        .map(|p| StackProfileSummary {
            id: p.id.to_string(),
            name: p.name.to_string(),
            core_tech: p.core_tech.iter().map(|s| s.to_string()).collect(),
            companions: p.companions.iter().map(|s| s.to_string()).collect(),
            competing: p.competing.iter().map(|s| s.to_string()).collect(),
            pain_point_count: p.pain_points.len(),
            ecosystem_shift_count: p.ecosystem_shifts.len(),
        })
        .collect()
}

/// Get the user's currently selected stack profile IDs.
#[tauri::command]
pub fn get_selected_stacks() -> Result<Vec<String>, String> {
    let conn = crate::open_db_connection()?;
    Ok(stacks::load_selected_stacks(&conn))
}

/// Set the user's selected stack profiles (replaces existing selections).
#[tauri::command]
pub fn set_selected_stacks(profile_ids: Vec<String>) -> Result<(), String> {
    // Validate all IDs exist
    for id in &profile_ids {
        if stacks::get_profile(id).is_none() {
            return Err(format!("Unknown stack profile: {}", id));
        }
    }
    let conn = crate::open_db_connection()?;
    stacks::save_selected_stacks(&conn, &profile_ids).map_err(|e| e.to_string())
}

/// Auto-detect matching stack profiles from ACE context.
#[tauri::command]
pub fn detect_stack_profiles() -> Vec<StackDetectionResult> {
    let ace_ctx = crate::scoring::get_ace_context();
    stacks::detection::detect_matching_profiles(&ace_ctx)
        .into_iter()
        .map(|d| StackDetectionResult {
            profile_id: d.profile_id,
            profile_name: d.profile_name,
            confidence: d.confidence,
            matched_tech: d.matched_tech,
        })
        .collect()
}

/// Get the composed (merged) stack summary for debugging/UI display.
#[tauri::command]
pub fn get_composed_stack() -> Result<ComposedStackSummary, String> {
    let conn = crate::open_db_connection()?;
    let composed = stacks::load_composed_stack(&conn);
    Ok(ComposedStackSummary {
        active: composed.active,
        pain_point_count: composed.pain_points.len(),
        ecosystem_shift_count: composed.ecosystem_shifts.len(),
        keyword_boost_count: composed.keyword_boosts.len(),
        source_preferences: composed
            .source_preferences
            .iter()
            .map(|(&s, &v)| (s.to_string(), v))
            .collect(),
        all_tech: composed.all_tech.iter().map(|s| s.to_string()).collect(),
        competing: composed.competing.iter().map(|s| s.to_string()).collect(),
    })
}
