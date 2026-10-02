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

mod dormant_notice;
// Pinned is not installed (AD-046): a live read of what `node_modules` holds.
// `pub(crate)`: Preemption appends its rows and projects them for the brief.
pub(crate) mod install_drift;
mod list_transport;
mod liveness;
mod materializer;
mod plan_snapshot;
mod types;
mod upgrade_plan;
mod upgrade_steps;
mod validate;

#[cfg(test)]
mod tests;

// Liveness policy (2026-08-31 audit): dormant-project and unverified-
// provenance urgency caps applied at materializer output. Cap-and-annotate,
// never drop.
pub use liveness::{
    cap_dormant_items, cap_unverified_item_urgency, dormant_projects_note, inactive_label,
    load_user_dependency_names, provenance_is_unverified, ProjectLiveness,
};

// Dormant-project visibility (2026-09-07 audit): a repo the user still owns
// but has not touched is named ONCE, quietly, instead of being silently
// absent (below the relevance floor) or shouting N rows.
pub use dormant_notice::collapse_dormant_alerts;

// Phase 1 dependency Upgrade Plan brain: the ranked plan, its machine-readable
// work order (AD-049) and the validation-drop canary the snapshot records.
pub use upgrade_plan::{build_upgrade_plan, BuiltPlan};

// Preemption LIST transport (AD-036): the single visibility filter (returned
// counts == rendered cards) plus the list-payload trim, applied only in
// `get_preemption_alerts`' response mapping.
pub use list_transport::present_preemption_list;

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
// readers (the `4da plan` CLI and the MCP `upgrade_planner`, which read the
// kv_store key themselves). `read_upgrade_plan_snapshot` is not yet called
// in-process (the app reads the plan live from the feed).
#[allow(unused_imports)]
pub use plan_snapshot::{persist_upgrade_plan, read_upgrade_plan_snapshot};
#[allow(unused_imports)]
pub use validate::{validate_item, ValidationError};
