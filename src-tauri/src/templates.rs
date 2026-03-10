// Copyright (c) 2025-2026 Antony Lawrence Kiddie Pasifa. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Pre-built markdown templates for STREETS playbook (free for all users).
//!
//! Provides actionable templates for launch planning, revenue tracking,
//! automation blueprints, competitive analysis, and pricing strategy.

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::Result;
use crate::template_data::*;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoachTemplate {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub content: String,
}

// ============================================================================
// Template Registry
// ============================================================================

fn get_all_templates() -> Vec<CoachTemplate> {
    vec![
        CoachTemplate {
            id: "30-day-launch".into(),
            title: "30-Day Engine Launch Plan".into(),
            description: "Step-by-step plan to launch your first revenue engine in 30 days".into(),
            category: "launch".into(),
            content: TEMPLATE_30DAY_LAUNCH.to_string(),
        },
        CoachTemplate {
            id: "revenue-tracker".into(),
            title: "Revenue Tracking Spreadsheet".into(),
            description: "Track revenue, costs, and key metrics across your engines".into(),
            category: "tracking".into(),
            content: TEMPLATE_REVENUE_TRACKER.to_string(),
        },
        CoachTemplate {
            id: "automation-blueprint".into(),
            title: "Automation Blueprint".into(),
            description: "Audit manual processes and build a 4-week automation plan".into(),
            category: "automation".into(),
            content: TEMPLATE_AUTOMATION_BLUEPRINT.to_string(),
        },
        CoachTemplate {
            id: "competitive-analysis".into(),
            title: "Competitive Analysis Framework".into(),
            description: "Analyze competitors, find gaps, and define your moat".into(),
            category: "analysis".into(),
            content: TEMPLATE_COMPETITIVE_ANALYSIS.to_string(),
        },
        CoachTemplate {
            id: "pricing-calculator".into(),
            title: "Pricing Calculator Worksheet".into(),
            description: "Calculate costs, model pricing tiers, and plan revenue milestones".into(),
            category: "pricing".into(),
            content: TEMPLATE_PRICING_CALCULATOR.to_string(),
        },
    ]
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get all available coach templates.
/// Free for all users — part of the STREETS playbook.
#[tauri::command]
pub fn get_templates() -> Result<Vec<CoachTemplate>> {
    let templates = get_all_templates();
    debug!(
        target: "4da::coach",
        count = templates.len(),
        "Returning coach templates"
    );

    Ok(templates)
}

/// Get a specific template by ID.
/// Free for all users — part of the STREETS playbook.
#[tauri::command]
pub fn get_template_content(template_id: String) -> Result<CoachTemplate> {
    let templates = get_all_templates();
    let template = templates
        .into_iter()
        .find(|t| t.id == template_id)
        .ok_or_else(|| format!("Template not found: {}", template_id))?;

    debug!(
        target: "4da::coach",
        template_id = %template_id,
        "Returning template content"
    );

    Ok(template)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- get_all_templates registry -------------------------------------------

    #[test]
    fn test_get_all_templates_returns_five() {
        let templates = get_all_templates();
        assert_eq!(
            templates.len(),
            5,
            "registry should contain exactly 5 templates"
        );
    }

    #[test]
    fn test_template_ids_are_unique() {
        let templates = get_all_templates();
        let mut ids: Vec<&str> = templates.iter().map(|t| t.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 5, "all template IDs must be unique");
    }

    #[test]
    fn test_expected_template_ids_present() {
        let templates = get_all_templates();
        let ids: Vec<&str> = templates.iter().map(|t| t.id.as_str()).collect();
        let expected = [
            "30-day-launch",
            "revenue-tracker",
            "automation-blueprint",
            "competitive-analysis",
            "pricing-calculator",
        ];
        for id in &expected {
            assert!(ids.contains(id), "expected template id '{}' missing", id);
        }
    }

    #[test]
    fn test_template_categories_are_distinct() {
        let templates = get_all_templates();
        let mut cats: Vec<&str> = templates.iter().map(|t| t.category.as_str()).collect();
        cats.sort();
        cats.dedup();
        assert_eq!(cats.len(), 5, "each template should have a unique category");
    }

    #[test]
    fn test_no_template_has_empty_fields() {
        for t in get_all_templates() {
            assert!(!t.id.is_empty(), "template id must not be empty");
            assert!(!t.title.is_empty(), "template title must not be empty");
            assert!(
                !t.description.is_empty(),
                "template '{}' description must not be empty",
                t.id
            );
            assert!(
                !t.category.is_empty(),
                "template '{}' category must not be empty",
                t.id
            );
            assert!(
                !t.content.is_empty(),
                "template '{}' content must not be empty",
                t.id
            );
        }
    }

    // -- Template content validation ------------------------------------------

    #[test]
    fn test_template_content_starts_with_markdown_heading() {
        for t in get_all_templates() {
            assert!(
                t.content.starts_with("# "),
                "template '{}' content should start with a markdown heading, got: {:?}",
                t.id,
                &t.content[..t.content.len().min(40)]
            );
        }
    }

    #[test]
    fn test_template_content_contains_checkboxes() {
        for t in get_all_templates() {
            assert!(
                t.content.contains("- [ ]"),
                "template '{}' should contain at least one markdown checkbox",
                t.id
            );
        }
    }

    #[test]
    fn test_launch_template_has_four_weeks() {
        let templates = get_all_templates();
        let launch = templates.iter().find(|t| t.id == "30-day-launch").unwrap();
        assert!(launch.content.contains("## Week 1"));
        assert!(launch.content.contains("## Week 2"));
        assert!(launch.content.contains("## Week 3"));
        assert!(launch.content.contains("## Week 4"));
    }

    #[test]
    fn test_revenue_tracker_has_expense_table() {
        let templates = get_all_templates();
        let tracker = templates
            .iter()
            .find(|t| t.id == "revenue-tracker")
            .unwrap();
        assert!(
            tracker.content.contains("## Expense Categories"),
            "revenue tracker should include expense categories section"
        );
        assert!(
            tracker.content.contains("| Category"),
            "revenue tracker should include expense table header"
        );
    }

    // -- Lookup by ID (mirrors get_template_content logic) --------------------

    #[test]
    fn test_find_template_by_valid_id() {
        let templates = get_all_templates();
        let found = templates.into_iter().find(|t| t.id == "pricing-calculator");
        assert!(found.is_some(), "should find pricing-calculator template");
        let tmpl = found.unwrap();
        assert_eq!(tmpl.category, "pricing");
        assert!(tmpl.content.contains("Pricing Calculator Worksheet"));
    }

    #[test]
    fn test_find_template_by_invalid_id_returns_none() {
        let templates = get_all_templates();
        let found = templates
            .into_iter()
            .find(|t| t.id == "nonexistent-template");
        assert!(
            found.is_none(),
            "should return None for a nonexistent template ID"
        );
    }

    // -- Serde roundtrip ------------------------------------------------------

    #[test]
    fn test_coach_template_serde_roundtrip() {
        let original = CoachTemplate {
            id: "test-id".into(),
            title: "Test Title".into(),
            description: "Test description".into(),
            category: "testing".into(),
            content: "# Test\n\n- [ ] Item".into(),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: CoachTemplate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.title, original.title);
        assert_eq!(deserialized.description, original.description);
        assert_eq!(deserialized.category, original.category);
        assert_eq!(deserialized.content, original.content);
    }

    #[test]
    fn test_coach_template_json_field_names() {
        let tmpl = CoachTemplate {
            id: "abc".into(),
            title: "T".into(),
            description: "D".into(),
            category: "C".into(),
            content: "X".into(),
        };
        let val = serde_json::to_value(&tmpl).expect("to_value");
        assert!(val.get("id").is_some(), "JSON must include 'id'");
        assert!(val.get("title").is_some(), "JSON must include 'title'");
        assert!(
            val.get("description").is_some(),
            "JSON must include 'description'"
        );
        assert!(
            val.get("category").is_some(),
            "JSON must include 'category'"
        );
        assert!(val.get("content").is_some(), "JSON must include 'content'");
        // Ensure no extra fields
        let obj = val.as_object().unwrap();
        assert_eq!(
            obj.len(),
            5,
            "CoachTemplate should serialize to exactly 5 fields"
        );
    }

    #[test]
    fn test_all_templates_serialize_successfully() {
        for t in get_all_templates() {
            let result = serde_json::to_value(&t);
            assert!(
                result.is_ok(),
                "template '{}' should serialize to JSON without error",
                t.id
            );
        }
    }
}
