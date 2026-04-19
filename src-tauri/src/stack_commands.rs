// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tauri commands for the Stack Intelligence System.
//!
//! 5 commands for managing stack profiles from the frontend:
//! - List available profiles
//! - Get/set user selections
//! - Auto-detect from ACE context
//! - Get composed stack summary (for debugging/UI)

use serde::Serialize;

use crate::error::Result;
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
            core_tech: p
                .core_tech
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            companions: p
                .companions
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            competing: p
                .competing
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            pain_point_count: p.pain_points.len(),
            ecosystem_shift_count: p.ecosystem_shifts.len(),
        })
        .collect()
}

/// Get the user's currently selected stack profile IDs.
#[tauri::command]
pub fn get_selected_stacks() -> Result<Vec<String>> {
    let conn = crate::open_db_connection()?;
    Ok(stacks::load_selected_stacks(&conn))
}

/// Set the user's selected stack profiles (replaces existing selections).
#[tauri::command]
pub fn set_selected_stacks(profile_ids: Vec<String>) -> Result<()> {
    // Validate all IDs exist
    for id in &profile_ids {
        if stacks::get_profile(id).is_none() {
            return Err(format!("Unknown stack profile: {id}").into());
        }
    }
    let conn = crate::open_db_connection()?;
    stacks::save_selected_stacks(&conn, &profile_ids)?;
    Ok(())
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
pub fn get_composed_stack() -> Result<ComposedStackSummary> {
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
        all_tech: composed
            .all_tech
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
        competing: composed
            .competing
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_profile_summary_construction() {
        let summary = StackProfileSummary {
            id: "nextjs".to_string(),
            name: "Next.js Full-Stack".to_string(),
            core_tech: vec!["next.js".to_string(), "react".to_string()],
            companions: vec!["tailwind".to_string()],
            competing: vec!["remix".to_string()],
            pain_point_count: 3,
            ecosystem_shift_count: 1,
        };
        assert_eq!(summary.id, "nextjs");
        assert_eq!(summary.core_tech.len(), 2);
        assert_eq!(summary.pain_point_count, 3);
    }

    #[test]
    fn test_stack_detection_result_serialization() {
        let result = StackDetectionResult {
            profile_id: "rust-backend".to_string(),
            profile_name: "Rust Backend".to_string(),
            confidence: 0.87,
            matched_tech: vec!["rust".to_string(), "tokio".to_string()],
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("\"confidence\":0.87"));
        assert!(json.contains("rust-backend"));
    }

    #[test]
    fn test_composed_stack_summary_serialization() {
        let summary = ComposedStackSummary {
            active: true,
            pain_point_count: 5,
            ecosystem_shift_count: 2,
            keyword_boost_count: 10,
            source_preferences: vec![("hackernews".to_string(), 1.2)],
            all_tech: vec!["rust".to_string(), "typescript".to_string()],
            competing: vec!["go".to_string()],
        };
        let json = serde_json::to_string(&summary).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed["active"], true);
        assert_eq!(parsed["pain_point_count"], 5);
        assert_eq!(parsed["all_tech"].as_array().expect("array").len(), 2);
    }
}
