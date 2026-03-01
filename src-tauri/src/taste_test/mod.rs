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
