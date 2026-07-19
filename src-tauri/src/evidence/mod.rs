// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Evidence — the canonical intelligence type for 4DA.
//!
//! Five parallel intelligence systems (Preemption, Blind Spots, Knowledge
//! Decay, Signal Chains, Evidence) historically shipped five parallel type systems. Each
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
mod upgrade_plan;
mod validate;

#[cfg(test)]
mod tests;

// Phase 1 dependency Upgrade Plan brain. `_with_drops` returns the ranked plan
// plus the validation-drop canary the persisted snapshot records.
pub use upgrade_plan::build_upgrade_plan_with_drops;

// These are published for consumption by Phases 3-5 (where existing
// Preemption / BlindSpots / KnowledgeDecay / SignalChains producers will
// implement `EvidenceMaterializer`). The unused-warnings are intentional
// while those phases are pending.
#[allow(unused_imports)]
pub use materializer::{EvidenceMaterializer, MaterializeContext};
#[allow(unused_imports)]
pub use types::{
    Action, Confidence, ConfidenceProvenance, EvidenceCitation, EvidenceFeed, EvidenceItem,
    EvidenceKind, LensHints, PrecedentOutcome, PrecedentRef, TierScope, UpgradePlanSnapshot,
    Urgency, ACTION_IDS,
};

// Phase 2 (D-1, DB-as-interface): persist the ranked plan for out-of-process
// readers (the MCP server / a future CLI). `read_upgrade_plan_snapshot` is not
// yet called in-process (the app reads the plan live from the feed) — the MCP
// read path is a later, operator-gated distribution stone.
#[allow(unused_imports)]
pub use upgrade_plan::{persist_upgrade_plan, read_upgrade_plan_snapshot};
#[allow(unused_imports)]
pub use validate::{validate_item, ValidationError};
