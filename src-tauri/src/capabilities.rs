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

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
