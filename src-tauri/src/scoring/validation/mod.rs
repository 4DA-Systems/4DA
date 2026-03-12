//! Scoring Validation — Persona-based precision testing
//!
//! Validates the scoring engine across 10 developer profiles by scoring
//! content items and auto-judging relevance via topic overlap.
//!
//! Entry point: `run_scoring_validation()` returns a `ValidationReport`
//! with per-persona precision@20, separation scores, and recommendations.
//!
//! Command registration is handled separately — these exports are consumed
//! by the Tauri command layer.

pub mod personas;
pub mod validation;

pub use validation::{run_scoring_validation, ValidationReport, ValidationResult};
