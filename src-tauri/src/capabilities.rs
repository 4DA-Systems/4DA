// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Graceful Degradation Framework — centralized capability state tracking.
//!
//! Every major subsystem in 4DA registers as a [`Capability`]. When a subsystem
//! encounters an error (missing API key, Ollama offline, sqlite-vec missing, etc.)
//! it reports its state via [`report_degraded`] or [`report_unavailable`]. When the
//! problem is resolved, it calls [`report_restored`].
//!
//! The frontend reads the registry via the `get_capability_states` and
//! `get_capability_summary` Tauri commands to render a live health dashboard.
//!
//! # Design Principles
//!
//! - **Optimistic default** — all capabilities start as `Full`.
//! - **Transition logging** — state changes are logged at the appropriate level
//!   (warn for degraded, error for unavailable, info for restored).
//! - **Lock-free reads** — uses `parking_lot::RwLock` so reads never block each other.
//! - **Idempotent** — redundant reports for the same state do not re-log.

use std::collections::HashMap;

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::Serialize;
use ts_rs::TS;

// ============================================================================
// Capability Enum
// ============================================================================

/// Every discrete subsystem that can independently degrade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Capability {
    /// Local embedding via Ollama — degrades to zero-vector fallback.
    EmbeddingSearch,
    /// LLM-based re-ranking of search results.
    LlmReranking,
    /// Morning / on-demand intelligence briefing generation.
    BriefingGeneration,
    /// Network fetching from content sources (HN, Reddit, RSS, GitHub, etc.).
    SourceFetching,
    /// ACE — Autonomous Context Engine project scanning.
    AceContext,
    /// System tray icon and menu.
    SystemTray,
    /// Desktop notification delivery.
    Notifications,
    /// OS keychain / credential storage (keyring crate).
    CredentialStorage,
    /// sqlite-vec vector similarity search.
    VectorSearch,
}

impl Capability {
    /// All known capabilities in declaration order.
    pub fn all() -> &'static [Capability] {
        &[
            Capability::EmbeddingSearch,
            Capability::LlmReranking,
            Capability::BriefingGeneration,
            Capability::SourceFetching,
            Capability::AceContext,
            Capability::SystemTray,
            Capability::Notifications,
            Capability::CredentialStorage,
            Capability::VectorSearch,
        ]
    }

    /// Human-readable name for UI display.
    pub fn display_name(&self) -> &'static str {
        match self {
            Capability::EmbeddingSearch => "Semantic Search",
            Capability::LlmReranking => "AI Re-ranking",
            Capability::BriefingGeneration => "Intelligence Briefing",
            Capability::SourceFetching => "Content Sources",
            Capability::AceContext => "Project Context",
            Capability::SystemTray => "System Tray",
            Capability::Notifications => "Notifications",
            Capability::CredentialStorage => "Secure Storage",
            Capability::VectorSearch => "Vector Database",
        }
    }
}

// ============================================================================
// Capability State
// ============================================================================

/// The runtime state of a single capability.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "state")]
#[ts(export)]
pub enum CapabilityState {
    /// Operating normally — no issues detected.
    #[serde(rename = "full")]
    Full,

    /// Partially functional — using a fallback path.
    #[serde(rename = "degraded")]
    Degraded {
        /// Why the capability degraded (e.g. "Ollama not reachable").
        reason: String,
        /// ISO-8601 timestamp of when the degradation was first reported.
        since: String,
        /// Description of the fallback behavior in use.
        fallback: String,
    },

    /// Completely non-functional.
    #[serde(rename = "unavailable")]
    Unavailable {
        /// Why the capability is unavailable.
        reason: String,
        /// User-actionable remediation step.
        remediation: String,
    },
}

// ============================================================================
// Summary
// ============================================================================

/// Aggregate counts of capability states — used by the frontend status bar.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct CapabilitySummary {
    pub full: u32,
    pub degraded: u32,
    pub unavailable: u32,
    pub total: u32,
}

// ============================================================================
// Global Registry
// ============================================================================

static CAPABILITY_REGISTRY: Lazy<RwLock<HashMap<Capability, CapabilityState>>> = Lazy::new(|| {
    let mut map = HashMap::with_capacity(Capability::all().len());
    for &cap in Capability::all() {
        map.insert(cap, CapabilityState::Full);
    }
    RwLock::new(map)
});

// ============================================================================
// Public API
// ============================================================================

/// Report that a capability has degraded to a fallback path.
///
/// Only logs the transition if the capability was **not** already degraded.
pub fn report_degraded(cap: Capability, reason: &str, fallback: &str) {
    let mut registry = CAPABILITY_REGISTRY.write();
    let prev = registry.get(&cap);
    if !matches!(prev, Some(CapabilityState::Degraded { .. })) {
        tracing::warn!(
            target: "4da::capabilities",
            capability = ?cap,
            reason = reason,
            fallback = fallback,
            "Capability degraded"
        );
    }
    registry.insert(
        cap,
        CapabilityState::Degraded {
            reason: reason.to_string(),
            since: chrono::Utc::now().to_rfc3339(),
            fallback: fallback.to_string(),
        },
    );
}

/// Report that a capability is completely unavailable.
///
/// Only logs the transition if the capability was **not** already unavailable.
pub fn report_unavailable(cap: Capability, reason: &str, remediation: &str) {
    let mut registry = CAPABILITY_REGISTRY.write();
    let prev = registry.get(&cap);
    if !matches!(prev, Some(CapabilityState::Unavailable { .. })) {
        tracing::error!(
            target: "4da::capabilities",
            capability = ?cap,
            reason = reason,
            "Capability unavailable"
        );
    }
    registry.insert(
        cap,
        CapabilityState::Unavailable {
            reason: reason.to_string(),
            remediation: remediation.to_string(),
        },
    );
}

/// Report that a previously degraded/unavailable capability has been restored.
///
/// Only logs if the capability was **not** already at full capacity.
pub fn report_restored(cap: Capability) {
    let mut registry = CAPABILITY_REGISTRY.write();
    let prev = registry.get(&cap);
    if !matches!(prev, Some(CapabilityState::Full)) {
        tracing::info!(
            target: "4da::capabilities",
            capability = ?cap,
            "Capability restored to full"
        );
    }
    registry.insert(cap, CapabilityState::Full);
}

/// Returns `true` if the capability is operational (Full **or** Degraded).
pub fn is_available(cap: Capability) -> bool {
    let registry = CAPABILITY_REGISTRY.read();
    matches!(
        registry.get(&cap),
        Some(CapabilityState::Full) | Some(CapabilityState::Degraded { .. })
    )
}

/// Returns `true` only if the capability is at full capacity.
pub fn is_full(cap: Capability) -> bool {
    let registry = CAPABILITY_REGISTRY.read();
    matches!(registry.get(&cap), Some(CapabilityState::Full))
}

/// Snapshot of every capability and its current state.
///
/// Used by the frontend health dashboard.
pub fn get_all_states() -> HashMap<Capability, CapabilityState> {
    CAPABILITY_REGISTRY.read().clone()
}

/// Aggregate summary of how many capabilities are full, degraded, or unavailable.
pub fn get_summary() -> CapabilitySummary {
    let registry = CAPABILITY_REGISTRY.read();
    let mut full = 0u32;
    let mut degraded = 0u32;
    let mut unavailable = 0u32;
    for state in registry.values() {
        match state {
            CapabilityState::Full => full += 1,
            CapabilityState::Degraded { .. } => degraded += 1,
            CapabilityState::Unavailable { .. } => unavailable += 1,
        }
    }
    CapabilitySummary {
        full,
        degraded,
        unavailable,
        total: registry.len() as u32,
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get capability states for the frontend health dashboard.
#[tauri::command]
pub fn get_capability_states() -> HashMap<Capability, CapabilityState> {
    get_all_states()
}

/// Get capability summary counts.
#[tauri::command]
pub fn get_capability_summary() -> CapabilitySummary {
    get_summary()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helper: inline state check (avoids releasing and re-acquiring the lock)
    // -----------------------------------------------------------------------

    fn is_full_in(reg: &HashMap<Capability, CapabilityState>, cap: Capability) -> bool {
        matches!(reg.get(&cap), Some(CapabilityState::Full))
    }

    fn is_available_in(reg: &HashMap<Capability, CapabilityState>, cap: Capability) -> bool {
        matches!(
            reg.get(&cap),
            Some(CapabilityState::Full) | Some(CapabilityState::Degraded { .. })
        )
    }

    fn reset(reg: &mut HashMap<Capability, CapabilityState>) {
        for &cap in Capability::all() {
            reg.insert(cap, CapabilityState::Full);
        }
    }

    fn make_degraded(reason: &str, fallback: &str) -> CapabilityState {
        CapabilityState::Degraded {
            reason: reason.to_string(),
            since: "2026-01-01T00:00:00Z".to_string(),
            fallback: fallback.to_string(),
        }
    }

    fn make_unavailable(reason: &str, remediation: &str) -> CapabilityState {
        CapabilityState::Unavailable {
            reason: reason.to_string(),
            remediation: remediation.to_string(),
        }
    }

    fn count_states(reg: &HashMap<Capability, CapabilityState>) -> (u32, u32, u32) {
        let (mut f, mut d, mut u) = (0u32, 0u32, 0u32);
        for state in reg.values() {
            match state {
                CapabilityState::Full => f += 1,
                CapabilityState::Degraded { .. } => d += 1,
                CapabilityState::Unavailable { .. } => u += 1,
            }
        }
        (f, d, u)
    }

    // -----------------------------------------------------------------------
    // Tests — each holds the write lock to prevent parallel interference.
    // -----------------------------------------------------------------------

    #[test]
    fn registry_contains_all_capabilities() {
        let registry = CAPABILITY_REGISTRY.read();
        assert_eq!(registry.len(), Capability::all().len());
        for &cap in Capability::all() {
            assert!(
                registry.contains_key(&cap),
                "Expected {:?} to be present in registry",
                cap
            );
        }
    }

    #[test]
    fn reset_sets_all_to_full() {
        let mut registry = CAPABILITY_REGISTRY.write();
        registry.insert(
            Capability::EmbeddingSearch,
            make_unavailable("test", "test"),
        );
        reset(&mut registry);
        for &cap in Capability::all() {
            assert!(
                is_full_in(&registry, cap),
                "Expected {:?} to be Full after reset",
                cap
            );
        }
    }

    #[test]
    fn degradation_transition() {
        let mut registry = CAPABILITY_REGISTRY.write();
        reset(&mut registry);
        let cap = Capability::EmbeddingSearch;

        registry.insert(
            cap,
            make_degraded("Ollama offline", "Using zero-vector fallback"),
        );

        match registry.get(&cap) {
            Some(CapabilityState::Degraded {
                reason,
                fallback,
                since,
            }) => {
                assert_eq!(reason, "Ollama offline");
                assert_eq!(fallback, "Using zero-vector fallback");
                assert!(!since.is_empty(), "since timestamp must be populated");
            }
            other => panic!("Expected Degraded, got {:?}", other),
        }
        reset(&mut registry);
    }

    #[test]
    fn unavailable_transition() {
        let mut registry = CAPABILITY_REGISTRY.write();
        reset(&mut registry);
        let cap = Capability::VectorSearch;

        registry.insert(
            cap,
            make_unavailable(
                "sqlite-vec extension failed to load",
                "Reinstall 4DA or check sqlite-vec binary",
            ),
        );

        match registry.get(&cap) {
            Some(CapabilityState::Unavailable {
                reason,
                remediation,
            }) => {
                assert_eq!(reason, "sqlite-vec extension failed to load");
                assert_eq!(remediation, "Reinstall 4DA or check sqlite-vec binary");
            }
            other => panic!("Expected Unavailable, got {:?}", other),
        }
        reset(&mut registry);
    }

    #[test]
    fn restoration_after_degradation() {
        let mut registry = CAPABILITY_REGISTRY.write();
        reset(&mut registry);
        let cap = Capability::LlmReranking;

        registry.insert(
            cap,
            make_degraded("API rate limited", "Skipping reranking pass"),
        );
        assert!(!is_full_in(&registry, cap));
        assert!(is_available_in(&registry, cap));

        registry.insert(cap, CapabilityState::Full);
        assert!(is_full_in(&registry, cap));
        assert!(is_available_in(&registry, cap));
        reset(&mut registry);
    }

    #[test]
    fn restoration_after_unavailable() {
        let mut registry = CAPABILITY_REGISTRY.write();
        reset(&mut registry);
        let cap = Capability::CredentialStorage;

        registry.insert(
            cap,
            make_unavailable("Keyring daemon not running", "Start keyring service"),
        );
        assert!(!is_full_in(&registry, cap));
        assert!(!is_available_in(&registry, cap));

        registry.insert(cap, CapabilityState::Full);
        assert!(is_full_in(&registry, cap));
        assert!(is_available_in(&registry, cap));
        reset(&mut registry);
    }

    #[test]
    fn is_available_for_full_and_degraded_not_unavailable() {
        let mut registry = CAPABILITY_REGISTRY.write();
        reset(&mut registry);
        let cap = Capability::SourceFetching;

        // Full -> available
        assert!(is_available_in(&registry, cap));

        // Degraded -> still available
        registry.insert(
            cap,
            make_degraded("Partial network failure", "Retrying failed sources"),
        );
        assert!(is_available_in(&registry, cap));

        // Unavailable -> not available
        registry.insert(
            cap,
            make_unavailable("No network", "Check internet connection"),
        );
        assert!(!is_available_in(&registry, cap));
        reset(&mut registry);
    }

    #[test]
    fn is_full_only_for_full_state() {
        let mut registry = CAPABILITY_REGISTRY.write();
        reset(&mut registry);
        let cap = Capability::AceContext;

        assert!(is_full_in(&registry, cap));

        registry.insert(
            cap,
            make_degraded("Scan incomplete", "Using cached context"),
        );
        assert!(!is_full_in(&registry, cap));

        registry.insert(cap, CapabilityState::Full);
        assert!(is_full_in(&registry, cap));
        reset(&mut registry);
    }

    #[test]
    fn summary_counts_are_correct() {
        let mut registry = CAPABILITY_REGISTRY.write();
        reset(&mut registry);

        let total = Capability::all().len() as u32;
        let (f, d, u) = count_states(&registry);
        assert_eq!(f, total);
        assert_eq!(d, 0);
        assert_eq!(u, 0);

        // Degrade two
        registry.insert(
            Capability::EmbeddingSearch,
            make_degraded("Ollama slow", "Zero-vector fallback"),
        );
        registry.insert(
            Capability::LlmReranking,
            make_degraded("Rate limited", "Skip reranking"),
        );

        // Make one unavailable
        registry.insert(
            Capability::SystemTray,
            make_unavailable("No tray support", "Run in windowed mode"),
        );

        let (f, d, u) = count_states(&registry);
        assert_eq!(f, total - 3);
        assert_eq!(d, 2);
        assert_eq!(u, 1);

        reset(&mut registry);
    }

    #[test]
    fn redundant_degraded_report_updates_fields() {
        let mut registry = CAPABILITY_REGISTRY.write();
        reset(&mut registry);
        let cap = Capability::Notifications;

        registry.insert(cap, make_degraded("First reason", "First fallback"));
        registry.insert(cap, make_degraded("Updated reason", "Updated fallback"));

        match registry.get(&cap) {
            Some(CapabilityState::Degraded {
                reason, fallback, ..
            }) => {
                assert_eq!(reason, "Updated reason");
                assert_eq!(fallback, "Updated fallback");
            }
            other => panic!("Expected Degraded, got {:?}", other),
        }
        reset(&mut registry);
    }

    #[test]
    fn redundant_unavailable_report_updates_fields() {
        let mut registry = CAPABILITY_REGISTRY.write();
        reset(&mut registry);
        let cap = Capability::VectorSearch;

        registry.insert(cap, make_unavailable("First reason", "First fix"));
        registry.insert(cap, make_unavailable("Updated reason", "Updated fix"));

        match registry.get(&cap) {
            Some(CapabilityState::Unavailable {
                reason,
                remediation,
            }) => {
                assert_eq!(reason, "Updated reason");
                assert_eq!(remediation, "Updated fix");
            }
            other => panic!("Expected Unavailable, got {:?}", other),
        }
        reset(&mut registry);
    }

    #[test]
    fn redundant_restored_on_full_is_noop() {
        let mut registry = CAPABILITY_REGISTRY.write();
        reset(&mut registry);
        let cap = Capability::BriefingGeneration;

        // Already full — inserting Full again should not change anything
        registry.insert(cap, CapabilityState::Full);
        assert!(is_full_in(&registry, cap));
    }

    #[test]
    fn transition_from_degraded_to_unavailable() {
        let mut registry = CAPABILITY_REGISTRY.write();
        reset(&mut registry);
        let cap = Capability::SourceFetching;

        registry.insert(cap, make_degraded("Partial failure", "Retrying"));
        assert!(is_available_in(&registry, cap));
        assert!(!is_full_in(&registry, cap));

        registry.insert(cap, make_unavailable("Total failure", "Check network"));
        assert!(!is_available_in(&registry, cap));
        assert!(!is_full_in(&registry, cap));
        reset(&mut registry);
    }

    #[test]
    fn transition_from_unavailable_to_degraded() {
        let mut registry = CAPABILITY_REGISTRY.write();
        reset(&mut registry);
        let cap = Capability::AceContext;

        registry.insert(
            cap,
            make_unavailable("No projects", "Add a project directory"),
        );
        assert!(!is_available_in(&registry, cap));

        registry.insert(cap, make_degraded("Partial scan", "Using stale cache"));
        assert!(is_available_in(&registry, cap));
        assert!(!is_full_in(&registry, cap));
        reset(&mut registry);
    }

    // -----------------------------------------------------------------------
    // Public API integration tests — test the actual report_*/is_*/get_*
    // functions through the global registry. These use the public API which
    // acquires its own locks, so they must NOT hold the write lock.
    // -----------------------------------------------------------------------

    #[test]
    fn public_api_report_degraded_then_restored() {
        // Use a capability unlikely to conflict with other tests
        let cap = Capability::BriefingGeneration;
        report_degraded(cap, "LLM offline", "Using cached briefing");
        assert!(is_available(cap));
        assert!(!is_full(cap));
        report_restored(cap);
        assert!(is_full(cap));
    }

    #[test]
    fn public_api_report_unavailable_then_restored() {
        let cap = Capability::Notifications;
        report_unavailable(cap, "Permission denied", "Grant notification permission");
        assert!(!is_available(cap));
        report_restored(cap);
        assert!(is_full(cap));
    }

    #[test]
    fn public_api_get_all_states_returns_map() {
        let states = get_all_states();
        assert_eq!(states.len(), Capability::all().len());
    }

    #[test]
    fn public_api_get_summary_returns_correct_total() {
        let summary = get_summary();
        assert_eq!(summary.total, Capability::all().len() as u32);
        assert_eq!(
            summary.full + summary.degraded + summary.unavailable,
            summary.total
        );
    }

    // -----------------------------------------------------------------------
    // Pure tests (no global state) — serialization and enum properties.
    // -----------------------------------------------------------------------

    #[test]
    fn display_names_are_unique_and_nonempty() {
        let names: Vec<&str> = Capability::all().iter().map(|c| c.display_name()).collect();
        for name in &names {
            assert!(!name.is_empty(), "Display name must not be empty");
        }
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "Display names must be unique");
    }

    #[test]
    fn all_returns_every_variant() {
        assert!(
            Capability::all().len() >= 9,
            "Expected at least 9 capabilities"
        );
    }

    #[test]
    fn serialization_full_state() {
        let state = CapabilityState::Full;
        let json = serde_json::to_string(&state).expect("serialize Full");
        assert_eq!(json, r#"{"state":"full"}"#);
    }

    #[test]
    fn serialization_degraded_state() {
        let state = CapabilityState::Degraded {
            reason: "test".to_string(),
            since: "2026-01-01T00:00:00Z".to_string(),
            fallback: "none".to_string(),
        };
        let json = serde_json::to_string(&state).expect("serialize Degraded");
        assert!(json.contains(r#""state":"degraded""#));
        assert!(json.contains(r#""reason":"test""#));
        assert!(json.contains(r#""fallback":"none""#));
        assert!(json.contains(r#""since":"2026-01-01T00:00:00Z""#));
    }

    #[test]
    fn serialization_unavailable_state() {
        let state = CapabilityState::Unavailable {
            reason: "broken".to_string(),
            remediation: "fix it".to_string(),
        };
        let json = serde_json::to_string(&state).expect("serialize Unavailable");
        assert!(json.contains(r#""state":"unavailable""#));
        assert!(json.contains(r#""reason":"broken""#));
        assert!(json.contains(r#""remediation":"fix it""#));
    }

    #[test]
    fn serialization_capability_enum() {
        let cap = Capability::EmbeddingSearch;
        let json = serde_json::to_string(&cap).expect("serialize Capability");
        assert_eq!(json, r#""embedding_search""#);
    }

    #[test]
    fn serialization_summary() {
        let summary = CapabilitySummary {
            full: 7,
            degraded: 1,
            unavailable: 1,
            total: 9,
        };
        let json = serde_json::to_string(&summary).expect("serialize CapabilitySummary");
        assert!(json.contains(r#""full":7"#));
        assert!(json.contains(r#""degraded":1"#));
        assert!(json.contains(r#""unavailable":1"#));
        assert!(json.contains(r#""total":9"#));
    }
}
