// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Sovereign Content Engine — remnants of the personalized STREETS lesson pipeline.
//!
//! The five-level personalization pipeline (interpolation, conditionals, insight
//! cards, sovereign connections, temporal evolution) was retired with the STREETS
//! tab. What survives is the cache janitor for the tables the pipeline left behind
//! and the crate-wide LLM-availability gate in `context`.

pub mod cache;
pub mod commands;
pub mod context;
