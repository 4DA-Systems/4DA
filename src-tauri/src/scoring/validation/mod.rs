// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Scoring Validation — Persona-based precision testing
//!
//! Validates the scoring engine across 10 developer profiles by scoring
//! content items and auto-judging relevance via topic overlap.

#[cfg(test)]
pub mod personas;
pub mod runner;
