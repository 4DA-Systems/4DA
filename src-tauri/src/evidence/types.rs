// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Canonical EvidenceItem type and all supporting enums/structs.
//!
//! The contract is pinned by `docs/strategy/EVIDENCE-ITEM-SCHEMA.md`.
//! Any divergence between code and schema updates the code, not the schema.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ============================================================================
// EvidenceItem — the canonical unit
// ============================================================================

/// A single unit of actionable intelligence surfaced to the user.
/// Produced by any `EvidenceMaterializer`. Consumed by any lens.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "bindings/")]
pub struct EvidenceItem {
    /// Stable identifier derived from content hash + source. Survives restart.
    pub id: String,

    /// The category of evidence. Determines default rendering hints.
    pub kind: EvidenceKind,

    /// One-line summary. ≤ 120 chars. No trailing period.
    pub title: String,

    /// Full explanation. Produced by AWE.articulate after Phase 9; may be
    /// empty during transition phases but never after the AWE spine is
    /// wired.
    pub explanation: String,

    /// Calibrated confidence with provenance.
    pub confidence: Confidence,

    /// Shared urgency scale.
    pub urgency: Urgency,

    /// 0.0 = fully reversible, 1.0 = irreversible. `None` only when
    /// reversibility is conceptually N/A for this kind.
    pub reversibility: Option<f32>,

    /// Citations backing the claim. Non-empty for all user-surfaced kinds
    /// except `Retrospective`.
    pub evidence: Vec<EvidenceCitation>,

    /// Projects this touches (empty if not project-scoped).
    pub affected_projects: Vec<String>,

    /// Dependencies this touches (empty if not dep-scoped).
    pub affected_deps: Vec<String>,

    /// Actions the user can take. Required for actionable kinds.
    pub suggested_actions: Vec<Action>,

    /// Precedents from the Wisdom Graph. Empty allowed on cold-start;
    /// should populate after Phase 8.
    pub precedents: Vec<PrecedentRef>,

    /// User-set refutation condition. Only populated for accepted
    /// `Decision` items tracked by the commitment-contract watcher.
    pub refutation_condition: Option<String>,

    /// Which lenses this item is a candidate for.
    pub lens_hints: LensHints,

    /// Unix timestamp in millis.
    pub created_at: i64,

    /// Unix timestamp in millis. `None` for durable items (decisions,
    /// retrospectives).
    pub expires_at: Option<i64>,
}

// ============================================================================
// EvidenceKind
// ============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum EvidenceKind {
    /// Forward-looking alert. Security advisory, breaking change, migration.
    Alert,
    /// Coverage gap. Dependency or topic the user is not watching.
    Gap,
    /// Missed signal. Item that was relevant but the user did not see.
    MissedSignal,
    /// Connected signals forming a pattern over time.
    Chain,
    /// A decision the user is weighing (inferred or typed).
    Decision,
    /// A retrospective on a past decision with new signal.
    Retrospective,
    /// A refutation condition has been met.
    Refutation,
    /// A precedent relevant to the user's context (informational).
    Precedent,
}

// ============================================================================
// Urgency
// ============================================================================

/// Shared urgency scale. Replaces AlertUrgency / GapSeverity / risk_level /
/// priority. Ordered from most to least urgent.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "bindings/")]
pub enum Urgency {
    /// Act within 24 hours.
    Critical,
    /// Act within the week.
    High,
    /// Act within the month.
    Medium,
    /// Informational.
    Watch,
}

// ============================================================================
// Confidence
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "bindings/")]
pub struct Confidence {
    /// 0.0–1.0.
    pub value: f32,

    /// Where this number came from.
    pub provenance: ConfidenceProvenance,

    /// If provenance is `Calibrated`, the N of samples. `None` for others.
    pub sample_size: Option<u32>,
}

impl Confidence {
    /// Constructor for keyword/pattern-matched confidence.
    pub fn checklist(value: f32) -> Self {
        Self {
            value,
            provenance: ConfidenceProvenance::Checklist,
            sample_size: None,
        }
    }

    /// Constructor for weighted-formula confidence.
    pub fn heuristic(value: f32) -> Self {
        Self {
            value,
            provenance: ConfidenceProvenance::Heuristic,
            sample_size: None,
        }
    }

    /// Constructor for Bayesian-calibrated confidence.
    /// `n` must be ≥ 10 per schema rules (enforced by `validate_item`).
    pub fn calibrated(value: f32, n: u32) -> Self {
        Self {
            value,
            provenance: ConfidenceProvenance::Calibrated,
            sample_size: Some(n),
        }
    }

    /// Constructor for LLM-assessed confidence (AWE.calibrate output).
    pub fn llm_assessed(value: f32) -> Self {
        Self {
            value,
            provenance: ConfidenceProvenance::LlmAssessed,
            sample_size: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum ConfidenceProvenance {
    /// Keyword/pattern matching. Fast, deterministic, limited.
    Checklist,
    /// Weighted formula.
    Heuristic,
    /// Bayesian posterior with ≥ 10 feedback samples.
    Calibrated,
    /// LLM judgment from AWE.calibrate.
    LlmAssessed,
}

// ============================================================================
// EvidenceCitation
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "bindings/")]
pub struct EvidenceCitation {
    /// Source (e.g. "hackernews", "github-advisory", "git-history",
    /// "curated-corpus").
    pub source: String,

    /// Human-readable title.
    pub title: String,

    /// URL if available. `None` for inferred signals (git-history).
    pub url: Option<String>,

    /// Age in days. 0.0 = today.
    pub freshness_days: f32,

    /// Why this was selected as evidence. ≤ 200 chars.
    pub relevance_note: String,
}

// ============================================================================
// Action
// ============================================================================

/// Allowed canonical action ids. Frontend dispatches by these values.
pub const ACTION_IDS: &[&str] = &[
    "dismiss",
    "acknowledge",
    "snooze_7d",
    "brief_this",
    "view_source",
    "investigate",
    "accept_decision",
    "reject_decision",
    "set_refutation",
];

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "bindings/")]
pub struct Action {
    /// Canonical id. Must be in `ACTION_IDS`.
    pub action_id: String,
    pub label: String,
    pub description: String,
}

// ============================================================================
// PrecedentRef
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "bindings/")]
pub struct PrecedentRef {
    pub decision_id: String,
    pub statement: String,
    pub outcome: Option<PrecedentOutcome>,
    /// Origin: "user-history" / "curated-corpus" / "public-corpus".
    pub origin: String,
    /// Similarity to the current situation, 0.0–1.0.
    pub similarity: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "bindings/")]
pub enum PrecedentOutcome {
    Confirmed,
    Refuted,
    Partial,
    Pending,
}

// ============================================================================
// LensHints
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Default)]
#[ts(export, export_to = "bindings/")]
pub struct LensHints {
    pub briefing: bool,
    pub preemption: bool,
    pub blind_spots: bool,
    pub evidence: bool,
}

impl LensHints {
    /// Convenience: hint only preemption (forward-looking alert).
    pub fn preemption_only() -> Self {
        Self {
            preemption: true,
            ..Default::default()
        }
    }

    /// Convenience: hint only blind-spots (coverage gap / missed signal).
    pub fn blind_spots_only() -> Self {
        Self {
            blind_spots: true,
            ..Default::default()
        }
    }

    /// Convenience: hint only the evidence lens (decisions, retrospectives).
    pub fn evidence_only() -> Self {
        Self {
            evidence: true,
            ..Default::default()
        }
    }
}

// ============================================================================
// EvidenceFeed — feed-level envelope emitted by lens-backing commands
// ============================================================================

/// Standard envelope every lens-backing command returns. Carries the items
/// plus precomputed summary counts (so the UI can render a summary bar
/// without traversing the items list). Emitted by `get_preemption_alerts`,
/// `get_blind_spots`, and Phase 12's Evidence lens command.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "bindings/")]
pub struct EvidenceFeed {
    pub items: Vec<EvidenceItem>,
    pub total: usize,
    pub critical_count: usize,
    pub high_count: usize,

    /// Optional 0–100 lens-specific health score. Populated by lenses that
    /// have a meaningful aggregate state — e.g. Blind Spots uses this for
    /// the coverage index. `None` for lenses where a single number is
    /// meaningless (e.g. Preemption: alerts are individual, not an
    /// aggregate). UIs that show a score MUST tooltip its definition.
    pub score: Option<f32>,
}

impl EvidenceFeed {
    /// Build a feed from items, computing the summary counts. No score.
    pub fn from_items(items: Vec<EvidenceItem>) -> Self {
        let total = items.len();
        let critical_count = items
            .iter()
            .filter(|i| i.urgency == Urgency::Critical)
            .count();
        let high_count = items.iter().filter(|i| i.urgency == Urgency::High).count();
        Self {
            items,
            total,
            critical_count,
            high_count,
            score: None,
        }
    }

    /// Build a feed from items with a lens-specific 0–100 aggregate score.
    pub fn from_items_with_score(items: Vec<EvidenceItem>, score: f32) -> Self {
        let mut feed = Self::from_items(items);
        feed.score = Some(score.clamp(0.0, 100.0));
        feed
    }
}
