// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Feature tier gating and trial management.
//!
//! Defines which features require the Signal tier, checks trial status,
//! and provides the `require_signal_feature` gate used by Tauri commands.

use crate::error::Result;
use serde::{Deserialize, Serialize};

use super::revalidation::maybe_revalidate_license;
use super::LicenseConfig;

/// Signal-gated features list.
///
/// Registry / inventory of every Tauri command that requires Signal tier.
/// The enforcement itself happens via `require_signal_feature("name")` at the
/// top of each command — the name passed in is only used for error messaging
/// and auditing; tier checking is independent of this list. Keeping the list
/// accurate lets the frontend query gating status up-front (via
/// `is_signal_feature_available`) and lets the license audit compare intent
/// vs enforcement.
///
/// When adding a gate to a new command, append its name here.
/// See `docs/strategy/LICENSE-GATING-AUDIT-2026-04-15.md` for the full audit.
pub const SIGNAL_FEATURES: &[&str] = &[
    // Intelligence panels (original)
    "get_attention_report",
    "get_knowledge_gaps",
    "get_signal_chains",
    "get_signal_chains_predicted",
    "get_project_health",
    // Developer DNA un-gated (AD-026): free tier viral sharing of DNA cards
    // natural_language_query removed — BYOK: runs on user's API key at zero cost (AD-025)
    "get_semantic_shifts",
    "synthesize_search",
    "standing_queries",
    // Additional panels added by LICENSE-GATING-AUDIT-2026-04-15
    "get_blind_spots",
    "get_preemption_alerts",
    "resolve_signal_chain",
    "get_decision_health_report",
    // Cross-project intelligence
    "get_tech_convergence",
    "get_project_health_comparison",
    "get_cross_project_dependencies",
    // Accuracy / intelligence reporting
    "get_accuracy_report",
    "get_intelligence_report",
    // Trust ledger analytics
    "get_domain_precision_report",
    "get_false_positive_analysis",
];

/// Check if the current user has Signal (or Team/Enterprise) tier access.
/// Returns true for "signal", "team", "enterprise", legacy "pro", an active trial,
/// or dev_unlock_all in debug builds.
/// Triggers periodic re-validation to catch settings.json manipulation.
pub fn is_signal() -> bool {
    maybe_revalidate_license();
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let license = &guard.get().license;
    if cfg!(debug_assertions) && license.dev_unlock_all {
        return true;
    }
    is_paid_tier(license.tier.as_str()) || is_trial_active(license)
}

/// Check if a tier string represents a paid tier.
/// Accepts legacy "pro" for backwards compatibility with existing settings.json files.
pub(crate) fn is_paid_tier(tier: &str) -> bool {
    matches!(tier, "signal" | "team" | "enterprise" | "pro")
}

/// Check if a feature is available for the given tier, including trial period
pub fn is_signal_feature_available(feature: &str, license: &LicenseConfig) -> bool {
    if is_paid_tier(license.tier.as_str()) {
        return true;
    }
    if is_trial_active(license) {
        return true;
    }
    !SIGNAL_FEATURES.contains(&feature)
}

/// Gate a Signal feature — returns Ok(()) if allowed, Err if not
/// Call at the top of any Signal-gated Tauri command.
/// Triggers periodic re-validation to catch settings.json manipulation.
pub fn require_signal_feature(feature: &str) -> Result<()> {
    maybe_revalidate_license();
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let license = &guard.get().license;
    // Dev unlock: `"dev_unlock_all": true` in settings.json bypasses all gates.
    // Only effective in debug builds — release builds ignore this field.
    if cfg!(debug_assertions) && license.dev_unlock_all {
        return Ok(());
    }
    if is_signal_feature_available(feature, license) {
        Ok(())
    } else {
        Err(format!(
            "{} requires 4DA Signal — start your free trial or upgrade to unlock it.",
            signal_feature_label(feature)
        )
        .into())
    }
}

/// Human-readable label for a Signal-gated feature, so error messages never
/// leak raw backend command names (e.g. "get_preemption_alerts") to the UI.
fn signal_feature_label(feature: &str) -> &'static str {
    match feature {
        "get_preemption_alerts" => "Preemption Radar",
        "get_blind_spots" => "Blind Spots",
        "get_knowledge_gaps" => "Knowledge Gaps",
        "get_signal_chains" | "get_signal_chains_predicted" | "resolve_signal_chain" => {
            "Signal Chains"
        }
        "get_attention_report" => "Attention Report",
        "synthesize_search" => "Search Synthesis",
        "standing_queries" => "Standing Queries",
        "get_semantic_shifts" => "Semantic Shifts",
        "get_project_health" | "get_project_health_comparison" => "Project Health",
        "get_decision_health_report" => "Decision Health",
        "get_tech_convergence" => "Tech Convergence",
        "get_cross_project_dependencies" => "Cross-Project Dependencies",
        "get_accuracy_report" | "get_intelligence_report" => "Intelligence Report",
        "get_domain_precision_report" | "get_false_positive_analysis" => "Trust Ledger",
        _ => "This feature",
    }
}

/// Trial duration in days. Reverse trial: auto-starts on first launch,
/// giving users enough time for compound intelligence effects to demonstrate value.
const TRIAL_DURATION_DAYS: i64 = 14;

/// Check if the free trial is still active (14 days from trial_started_at)
pub fn is_trial_active(license: &LicenseConfig) -> bool {
    if is_paid_tier(license.tier.as_str()) {
        return false; // Not on trial, has a real license
    }
    match &license.trial_started_at {
        Some(started) => {
            if let Ok(start_date) = chrono::DateTime::parse_from_rfc3339(started) {
                let elapsed = chrono::Utc::now().signed_duration_since(start_date);
                elapsed.num_days() < TRIAL_DURATION_DAYS
            } else {
                false
            }
        }
        None => false, // Trial not started yet
    }
}

/// Get trial status info
pub fn get_trial_status(license: &LicenseConfig) -> TrialStatus {
    if is_paid_tier(license.tier.as_str()) {
        return TrialStatus {
            active: false,
            days_remaining: 0,
            started_at: None,
            has_license: true,
        };
    }
    match &license.trial_started_at {
        Some(started) => {
            if let Ok(start_date) = chrono::DateTime::parse_from_rfc3339(started) {
                let elapsed = chrono::Utc::now().signed_duration_since(start_date);
                let remaining = TRIAL_DURATION_DAYS - elapsed.num_days();
                TrialStatus {
                    active: remaining > 0,
                    days_remaining: remaining.max(0) as i32,
                    started_at: Some(started.clone()),
                    has_license: false,
                }
            } else {
                TrialStatus {
                    active: false,
                    days_remaining: 0,
                    started_at: Some(started.clone()),
                    has_license: false,
                }
            }
        }
        None => TrialStatus {
            active: false,
            days_remaining: 0,
            started_at: None,
            has_license: false,
        },
    }
}

/// Trial status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialStatus {
    pub active: bool,
    pub days_remaining: i32,
    pub started_at: Option<String>,
    pub has_license: bool,
}
