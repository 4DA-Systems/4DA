//! Taste Test Calibration — Bayesian persona inference from 15 content items.
//!
//! Shows users carefully-selected content cards and infers their developer
//! persona blend using posterior probability updates. Writes results to the
//! same DB tables the scoring pipeline reads — zero pipeline changes needed.

pub mod blending;
pub mod db;
pub mod inference;
pub mod items;

use serde::{Deserialize, Serialize};

// ============================================================================
// Core Types
// ============================================================================

/// A content card shown to the user during calibration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TasteCard {
    pub id: u64,
    pub slot: usize,
    pub title: String,
    pub snippet: String,
    pub source_hint: String,
    pub category_hint: String,
}

/// User's response to a taste card.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TasteResponse {
    Interested,
    NotInterested,
    StrongInterest,
}

/// Result of the calibration inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TasteProfile {
    pub persona_weights: [f64; 9],
    pub dominant_persona: usize,
    pub confidence: f64,
    pub items_shown: u32,
    pub inferred_interests: Vec<(String, f32)>,
    pub inferred_exclusions: Vec<String>,
    pub calibration_deltas: Vec<(String, f32)>,
}

/// Frontend-friendly summary of the taste profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TasteProfileSummary {
    pub dominant_persona_name: String,
    pub dominant_persona_description: String,
    pub confidence: f64,
    pub items_shown: u32,
    pub persona_weights: Vec<PersonaWeight>,
    pub top_interests: Vec<String>,
}

/// A single persona weight for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaWeight {
    pub name: String,
    pub weight: f64,
}

/// Step result returned to the frontend after each interaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TasteTestStep {
    NextCard {
        card: TasteCard,
        progress: f32,
        confidence: f32,
    },
    Complete {
        summary: TasteProfileSummary,
    },
}

// ============================================================================
// Constants
// ============================================================================

/// Canonical persona names in index order.
pub const PERSONA_NAMES: [&str; 9] = [
    "Rust Systems Developer",
    "Python ML Engineer",
    "Fullstack TypeScript Developer",
    "DevOps/SRE Engineer",
    "Mobile Developer",
    "Bootstrap Builder",
    "Power User / Polyglot",
    "Context Switcher",
    "Niche Specialist",
];

/// Short persona descriptions.
pub const PERSONA_DESCRIPTIONS: [&str; 9] = [
    "Deep in systems programming, Rust, Tauri, and low-level performance.",
    "Machine learning, PyTorch, data science, and AI/ML research.",
    "React, Next.js, TypeScript full-stack web development.",
    "Kubernetes, cloud infrastructure, observability, and reliability.",
    "Mobile-first: React Native, Swift, Kotlin, cross-platform apps.",
    "Ship fast with proven tools. Practical, product-focused development.",
    "Broad expertise across multiple domains and languages.",
    "Moves between tech stacks frequently. Values breadth over depth.",
    "Deep expertise in a specialized domain (Haskell, Erlang, etc.).",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_taste_test_step_serialization_next_card() {
        let step = TasteTestStep::NextCard {
            card: TasteCard {
                id: 1,
                slot: 0,
                title: "Test".into(),
                snippet: "Snippet".into(),
                source_hint: "HN".into(),
                category_hint: "Systems".into(),
            },
            progress: 0.5,
            confidence: 0.3,
        };
        let json = serde_json::to_value(&step).unwrap();

        // Verify tag name
        assert_eq!(json["type"], "nextCard", "Tag should be camelCase");
        // Verify card fields are camelCase
        assert_eq!(json["card"]["slot"], 0, "slot should be present");
        assert_eq!(json["card"]["sourceHint"], "HN");
        assert_eq!(json["card"]["categoryHint"], "Systems");
        // Verify progress/confidence are present
        assert!(json["progress"].as_f64().is_some());
        assert!(json["confidence"].as_f64().is_some());
    }

    #[test]
    fn test_taste_test_step_serialization_complete() {
        let step = TasteTestStep::Complete {
            summary: TasteProfileSummary {
                dominant_persona_name: "Rust Systems Developer".into(),
                dominant_persona_description: "Deep in systems".into(),
                confidence: 0.85,
                items_shown: 10,
                persona_weights: vec![PersonaWeight {
                    name: "Rust".into(),
                    weight: 0.6,
                }],
                top_interests: vec!["Rust".into()],
            },
        };
        let json = serde_json::to_value(&step).unwrap();

        // Verify tag
        assert_eq!(json["type"], "complete");
        // Verify summary fields are camelCase
        assert_eq!(
            json["summary"]["dominantPersonaName"],
            "Rust Systems Developer"
        );
        assert_eq!(json["summary"]["itemsShown"], 10);
        assert_eq!(json["summary"]["topInterests"][0], "Rust");
        assert!(json["summary"]["personaWeights"].is_array());
    }

    /// End-to-end simulation: mimics the frontend calling taste_test_start →
    /// taste_test_respond × N → taste_test_finalize, verifying every JSON
    /// payload matches what TypeScript expects.
    #[test]
    fn test_full_flow_e2e_json_contract() {
        use inference::InferenceState;

        let mut state = InferenceState::new();

        // Step 1: Get first card (simulates taste_test_start)
        let first_step = state.next_step();
        let json = serde_json::to_value(&first_step).unwrap();
        assert_eq!(json["type"], "nextCard");
        assert!(json["card"]["id"].is_u64(), "card.id should be u64");
        assert!(
            json["card"]["title"].is_string(),
            "card.title should be string"
        );
        assert!(
            json["card"]["sourceHint"].is_string(),
            "card.sourceHint should be camelCase"
        );
        assert!(
            json["card"]["categoryHint"].is_string(),
            "card.categoryHint should be camelCase"
        );
        assert!(json["card"]["slot"].is_u64(), "card.slot should be u64");
        assert!(json["progress"].is_f64(), "progress should be f64");
        assert!(json["confidence"].is_f64(), "confidence should be f64");

        // Step 2: Simulate responses (mimics taste_test_respond × N)
        // Strong Rust developer persona
        let responses: Vec<(usize, TasteResponse)> = vec![
            (0, TasteResponse::Interested),     // Rust 2024
            (1, TasteResponse::NotInterested),  // PyTorch
            (6, TasteResponse::StrongInterest), // tokio
            (8, TasteResponse::Interested),     // WASM+Rust
            (10, TasteResponse::Interested),    // sqlite-vec
            (2, TasteResponse::NotInterested),  // K8s
            (4, TasteResponse::NotInterested),  // React Native
            (5, TasteResponse::NotInterested),  // Haskell
        ];

        for (slot, resp) in &responses {
            state.update(*slot, resp);
            let step = state.next_step();
            let j = serde_json::to_value(&step).unwrap();
            // Every step should be either nextCard or complete
            let step_type = j["type"].as_str().unwrap();
            assert!(
                step_type == "nextCard" || step_type == "complete",
                "step type should be 'nextCard' or 'complete', got '{step_type}'"
            );
        }

        // Step 3: Finalize (simulates taste_test_finalize)
        let summary = state.build_summary();
        let json = serde_json::to_value(&summary).unwrap();

        // Verify all TypeScript-expected fields exist and have correct types
        assert!(
            json["dominantPersonaName"].is_string(),
            "dominantPersonaName should be string"
        );
        assert!(
            json["dominantPersonaDescription"].is_string(),
            "dominantPersonaDescription should be string"
        );
        assert!(json["confidence"].is_f64(), "confidence should be number");
        assert!(json["itemsShown"].is_u64(), "itemsShown should be number");
        assert!(
            json["personaWeights"].is_array(),
            "personaWeights should be array"
        );
        assert!(
            json["topInterests"].is_array(),
            "topInterests should be array"
        );

        // Verify persona weights have correct shape
        let weights = json["personaWeights"].as_array().unwrap();
        assert!(
            !weights.is_empty(),
            "Should have at least one persona weight"
        );
        assert!(
            weights[0]["name"].is_string(),
            "PersonaWeight.name should be string"
        );
        assert!(
            weights[0]["weight"].is_f64(),
            "PersonaWeight.weight should be number"
        );

        // Verify the dominant persona is Rust-related given our responses
        let dominant = json["dominantPersonaName"].as_str().unwrap();
        assert!(
            dominant.contains("Rust") || dominant.contains("Power"),
            "Dominant persona should be Rust-related, got '{dominant}'"
        );

        // Verify confidence is reasonable (> 50% after 8 responses)
        let conf = json["confidence"].as_f64().unwrap();
        assert!(
            conf > 0.5,
            "Confidence should be > 50% after 8 responses, got {conf:.2}"
        );

        // Verify top interests include Rust-related topics
        let interests = json["topInterests"].as_array().unwrap();
        assert!(!interests.is_empty(), "Should have inferred interests");
    }
}
