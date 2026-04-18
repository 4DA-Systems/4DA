// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! EvidenceMaterializer — the single trait every intelligence system
//! implements after Phases 3-5 of the Intelligence Reconciliation.
//!
//! Rule 7 of the Intelligence Doctrine: "Materializer trait is the only entry
//! point — no tab queries the DB directly for intelligence data."

use async_trait::async_trait;

use super::types::EvidenceItem;
use crate::error::Result;

/// Context handed to every materializer on each call. Carries the user's
/// situation so materializers never query state independently.
#[derive(Debug, Clone, Default)]
pub struct MaterializeContext {
    /// The user's primary tech stack (from ACE).
    pub primary_stack: Vec<String>,

    /// Adjacent technologies the user touches.
    pub adjacent_tech: Vec<String>,

    /// Absolute paths of currently-active project directories.
    pub active_projects: Vec<String>,

    /// Optional time-window cap. Items older than this should not be
    /// surfaced. Expressed as Unix millis lower bound (inclusive).
    pub since_ms: Option<i64>,

    /// Cap on how many items the materializer should return. Lenses do
    /// their own final ranking/dedup but materializers should not dump
    /// megabytes when a small N suffices.
    pub max_items: usize,
}

impl MaterializeContext {
    pub fn new() -> Self {
        Self {
            max_items: 50,
            ..Default::default()
        }
    }
}

#[async_trait]
pub trait EvidenceMaterializer: Send + Sync {
    /// Materializer name, for logging/provenance.
    fn name(&self) -> &'static str;

    /// Produce evidence items from the materializer's data source.
    ///
    /// Contract:
    /// - Every returned item must pass `validate_item`.
    /// - `explanation` may be empty before Phase 9; must be populated after.
    /// - Items should be pre-ranked by the materializer; the caller may
    ///   further re-rank across materializers.
    /// - Errors propagate via `Result`; a materializer that fails does not
    ///   bring down other materializers — the caller is expected to tolerate
    ///   partial failure.
    async fn materialize(&self, ctx: &MaterializeContext) -> Result<Vec<EvidenceItem>>;
}
