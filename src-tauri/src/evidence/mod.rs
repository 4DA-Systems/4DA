// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Evidence — the canonical intelligence type for 4DA.
//!
//! Five parallel intelligence systems (AWE, Preemption, Blind Spots, Knowledge
//! Decay, Signal Chains) historically shipped five parallel type systems. Each
//! duplicated the same fields with different names, had a different confidence
//! scale, and hand-wrote its own "why this matters" text. Consumers could not
//! compare, deduplicate, or route items across systems.
//!
//! `EvidenceItem` is the single type every lens consumes. Producers (existing
//! systems now implementing `EvidenceMaterializer`) differ in how they produce
//! items. Consumers (Briefing, Preemption, Blind Spots, Evidence, Results
//! lenses) differ in which `EvidenceKind` they render. Everything else is
//! shared.
//!
//! Contract: `docs/strategy/EVIDENCE-ITEM-SCHEMA.md`.
//! Plan: `docs/strategy/INTELLIGENCE-RECONCILIATION.md`.
//! Doctrine: `.claude/rules/intelligence-doctrine.md`.

mod materializer;
mod types;
mod validate;

#[cfg(test)]
mod tests;

// These are published for consumption by Phases 3-5 (where existing
// Preemption / BlindSpots / KnowledgeDecay / SignalChains producers will
// implement `EvidenceMaterializer`) and Phase 9 (AWE spine wiring). The
// unused-warnings are intentional while those phases are pending.
#[allow(unused_imports)]
pub use materializer::{EvidenceMaterializer, MaterializeContext};
#[allow(unused_imports)]
pub use types::{
    Action, Confidence, ConfidenceProvenance, EvidenceCitation, EvidenceFeed, EvidenceItem,
    EvidenceKind, LensHints, PrecedentOutcome, PrecedentRef, Urgency, ACTION_IDS,
};
#[allow(unused_imports)]
pub use validate::{validate_item, ValidationError};
