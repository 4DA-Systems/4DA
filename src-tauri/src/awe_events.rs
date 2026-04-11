// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Typed real-time event layer for the AWE (Artificial Wisdom Engine) subsystem.
//!
//! Before this module existed, the only `awe-*` event 4DA ever emitted was
//! `awe-wisdom-synthesis` at startup — and nothing in the frontend listened
//! to it. The UI had to poll via `loadAweSummary()` on component mount,
//! which meant users saw a stale snapshot until they reopened a tab.
//!
//! This module provides:
//!
//! 1. A typed `AweEvent` enum covering every wisdom-graph mutation.
//! 2. A single `emit_awe_event()` helper that serializes and dispatches to
//!    the frontend via `tauri::AppHandle::emit`.
//! 3. Event name constants so the frontend hook and any documentation
//!    share one source of truth (`awe::EVENT_*`).
//!
//! The frontend subscribes via `src/hooks/use-awe-live-events.ts` which
//! maps each event into a `zustand` store mutation so the `MomentumWisdomTrajectory`
//! (and any other consumer) re-renders incrementally without re-fetching.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};
use tracing::warn;

// ============================================================================
// Event name constants — shared with the frontend hook.
// ============================================================================

pub const EVENT_DECISION_ADDED: &str = "awe:decision-added";
pub const EVENT_FEEDBACK_RECORDED: &str = "awe:feedback-recorded";
pub const EVENT_PRINCIPLE_VALIDATED: &str = "awe:principle-validated";
pub const EVENT_COVERAGE_CHANGED: &str = "awe:coverage-changed";
pub const EVENT_SCAN_COMPLETE: &str = "awe:scan-complete";
pub const EVENT_RETRIAGE_COMPLETE: &str = "awe:retriage-complete";
pub const EVENT_SEED_COMPLETE: &str = "awe:seed-complete";
pub const EVENT_SOURCE_MINING_COMPLETE: &str = "awe:source-mining-complete";
/// Emitted when any AWE mutation happens — lets the UI cheaply invalidate
/// its cached summary without needing to know the specific change type.
pub const EVENT_SUMMARY_STALE: &str = "awe:summary-stale";

// ============================================================================
// Event payloads
// ============================================================================

/// Every AWE mutation in 4DA maps to one variant of this enum. The variant
/// name determines the emitted event name (see `event_name()` below).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AweEvent {
    /// A new decision was persisted to AWE's Wisdom Graph.
    DecisionAdded {
        id: String,
        statement: String,
        domain: String,
        /// Where the decision came from — "user_interaction" / "git_scan" /
        /// "source_mining" / "seeding" / "manual".
        source: String,
    },
    /// Outcome feedback was recorded for an existing decision.
    FeedbackRecorded {
        decision_id: String,
        /// One of "confirmed" / "refuted" / "partial".
        outcome: String,
    },
    /// A principle crossed the validation threshold and joined the
    /// validated set. This is the biggest event — the user went from
    /// "collecting signals" to "compound wisdom available."
    PrincipleValidated {
        statement: String,
        confidence: f32,
        evidence_count: u32,
        domain: String,
    },
    /// Aggregate counters changed (decisions / feedback_count / coverage_pct).
    /// Useful when many mutations happen in a burst and we just want a
    /// single "re-render the hero metrics" tick.
    CoverageChanged {
        decisions: u64,
        feedback_count: u64,
        coverage_pct: u64,
    },
    /// A git-repository scan finished (Tier 1).
    ScanComplete {
        repos_scanned: u64,
        decisions_stored: u64,
        outcomes_inferred: u64,
    },
    /// A retriage run finished (Tier 3).
    RetriageComplete {
        auto_confirmed: u64,
        demoted: u64,
        unchanged: u64,
    },
    /// Tier 0 cold-start seeding completed.
    SeedComplete { decisions_loaded: u64 },
    /// Tier 2 source-item mining finished a batch.
    SourceMiningComplete {
        candidates_considered: u64,
        decisions_created: u64,
        rate_limited: u64,
    },
    /// Cheap fallback — frontend should invalidate its AweSummary cache.
    SummaryStale,
}

impl AweEvent {
    /// Map variant → Tauri event name constant.
    #[inline]
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::DecisionAdded { .. } => EVENT_DECISION_ADDED,
            Self::FeedbackRecorded { .. } => EVENT_FEEDBACK_RECORDED,
            Self::PrincipleValidated { .. } => EVENT_PRINCIPLE_VALIDATED,
            Self::CoverageChanged { .. } => EVENT_COVERAGE_CHANGED,
            Self::ScanComplete { .. } => EVENT_SCAN_COMPLETE,
            Self::RetriageComplete { .. } => EVENT_RETRIAGE_COMPLETE,
            Self::SeedComplete { .. } => EVENT_SEED_COMPLETE,
            Self::SourceMiningComplete { .. } => EVENT_SOURCE_MINING_COMPLETE,
            Self::SummaryStale => EVENT_SUMMARY_STALE,
        }
    }
}

// ============================================================================
// Emit helper
// ============================================================================

/// Emit a single AWE event to the frontend. Failures are logged but never
/// propagated — event delivery is best-effort, the UI always has the poll
/// safety net as a backup.
///
/// Generic over `Runtime` so this helper works from both the production
/// Tauri runtime (Wry) and from monitoring tasks that take a generic
/// `AppHandle<R>` — matching the signature used by `monitoring::start_scheduler`.
pub fn emit_awe_event<R: Runtime>(app: &AppHandle<R>, event: AweEvent) {
    let name = event.event_name();
    if let Err(e) = app.emit(name, &event) {
        warn!(target: "4da::awe_events", event = %name, error = %e, "Failed to emit AWE event");
    }
}

/// Emit multiple events in quick succession. Equivalent to calling
/// `emit_awe_event` in a loop but makes the intent explicit.
pub fn emit_awe_events<R: Runtime>(app: &AppHandle<R>, events: impl IntoIterator<Item = AweEvent>) {
    for event in events {
        emit_awe_event(app, event);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_name_matches_constant_for_every_variant() {
        // Safeguard: if someone adds a new AweEvent variant without wiring
        // its constant, this test forces them to update event_name().
        let cases = vec![
            (
                AweEvent::DecisionAdded {
                    id: "dc_test".into(),
                    statement: "test".into(),
                    domain: "sw".into(),
                    source: "test".into(),
                },
                EVENT_DECISION_ADDED,
            ),
            (
                AweEvent::FeedbackRecorded {
                    decision_id: "dc_test".into(),
                    outcome: "confirmed".into(),
                },
                EVENT_FEEDBACK_RECORDED,
            ),
            (
                AweEvent::PrincipleValidated {
                    statement: "test".into(),
                    confidence: 0.85,
                    evidence_count: 7,
                    domain: "sw".into(),
                },
                EVENT_PRINCIPLE_VALIDATED,
            ),
            (
                AweEvent::CoverageChanged {
                    decisions: 100,
                    feedback_count: 90,
                    coverage_pct: 90,
                },
                EVENT_COVERAGE_CHANGED,
            ),
            (
                AweEvent::ScanComplete {
                    repos_scanned: 3,
                    decisions_stored: 12,
                    outcomes_inferred: 5,
                },
                EVENT_SCAN_COMPLETE,
            ),
            (
                AweEvent::RetriageComplete {
                    auto_confirmed: 4,
                    demoted: 1,
                    unchanged: 30,
                },
                EVENT_RETRIAGE_COMPLETE,
            ),
            (
                AweEvent::SeedComplete {
                    decisions_loaded: 30,
                },
                EVENT_SEED_COMPLETE,
            ),
            (
                AweEvent::SourceMiningComplete {
                    candidates_considered: 100,
                    decisions_created: 7,
                    rate_limited: 3,
                },
                EVENT_SOURCE_MINING_COMPLETE,
            ),
            (AweEvent::SummaryStale, EVENT_SUMMARY_STALE),
        ];
        for (ev, expected) in cases {
            assert_eq!(
                ev.event_name(),
                expected,
                "event_name() drift for variant {ev:?}"
            );
        }
    }

    #[test]
    fn event_serializes_with_kind_tag() {
        let ev = AweEvent::DecisionAdded {
            id: "dc_123".into(),
            statement: "Should we adopt X?".into(),
            domain: "software-engineering".into(),
            source: "user_interaction".into(),
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["kind"], "decision_added");
        assert_eq!(json["id"], "dc_123");
        assert_eq!(json["statement"], "Should we adopt X?");
    }

    #[test]
    fn feedback_event_serialization_roundtrip() {
        let ev = AweEvent::FeedbackRecorded {
            decision_id: "dc_abc".into(),
            outcome: "confirmed".into(),
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: AweEvent = serde_json::from_str(&json).unwrap();
        match back {
            AweEvent::FeedbackRecorded {
                decision_id,
                outcome,
            } => {
                assert_eq!(decision_id, "dc_abc");
                assert_eq!(outcome, "confirmed");
            }
            _ => panic!("wrong variant after roundtrip"),
        }
    }

    #[test]
    fn summary_stale_event_has_no_payload() {
        let ev = AweEvent::SummaryStale;
        let json = serde_json::to_value(&ev).unwrap();
        // Serde tags only — just the kind discriminator, no extra fields.
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 1);
        assert_eq!(obj["kind"], "summary_stale");
    }
}
