// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Autonomous AWE tiers 0, 1, and 3.
//!
//! The audit in `docs/strategy/AWE-ASSESSMENT.md` identified five autonomous
//! capabilities in AWE that were never wired:
//!
//! - **Tier 0 — Seeding**: `awe season` loads a battle-tested developer
//!   wisdom corpus. New users start with a populated graph instead of a
//!   blank slate. Cold-start → warm.
//! - **Tier 1 — LLM-assisted classification**: the existing daily git scan
//!   ran with algorithmic-only classification. Adding `--classify` gives
//!   the LLM a vote on which candidates are high-worthiness.
//! - **Tier 3 — Retriage loop**: `awe retriage --auto-confirm-threshold 0.7`
//!   re-scores every detected decision with AWE's worthiness system and
//!   auto-promotes the high-scorers. This is how the graph self-prunes.
//!
//! Tier 2 (source-item mining) lives in `awe_source_mining.rs`. Tier 5
//! (reactive principle crystallization) lives in `awe_commands.rs` —
//! `sync_awe_wisdom` runs whenever the event layer reports a coverage change.
//!
//! All three tiers emit events via `awe_events::emit_awe_event` so the UI
//! updates live without polling.

use std::process::Command;
use tauri::{AppHandle, Runtime};
use tracing::{info, warn};

use crate::awe_commands::awe_events::{emit_awe_event, AweEvent};
use crate::context_commands::{find_awe_binary, run_awe_with_timeout};

// ============================================================================
// Tier 0 — Cold-start seeding
// ============================================================================

/// Outcome of a tier-0 seed check.
#[derive(Debug, Clone, Copy)]
pub enum SeedOutcome {
    /// AWE already had decisions — no seeding needed.
    NotNeeded,
    /// Seeding ran and loaded decisions.
    Seeded(()),
    /// AWE binary is missing.
    AweUnavailable,
    /// Seeding failed (logged, non-fatal).
    Failed,
}

/// Check whether AWE has any decisions. If zero, run `awe season` to load
/// the built-in wisdom corpus. Safe to call repeatedly — if the graph is
/// already populated this is a no-op.
///
/// This is the "first launch" experience fix: previously a new user would
/// see "Seeding phase — 0 decisions tracked" for days until git scans
/// accumulated enough data. Now they get a warm graph at startup.
pub async fn run_tier0_seed_if_empty<R: Runtime>(app: &AppHandle<R>) -> SeedOutcome {
    let Some(awe_path) = find_awe_binary() else {
        return SeedOutcome::AweUnavailable;
    };

    // Check decision count via calibration output.
    let cal_out = run_awe_with_timeout(Command::new(&awe_path).args(["calibration"]), 10);
    let decisions = match cal_out {
        Ok(output) => parse_total_decisions(&String::from_utf8_lossy(&output.stdout)),
        Err(e) => {
            warn!(target: "4da::awe_autonomous", error = %e, "tier0: calibration read failed");
            return SeedOutcome::Failed;
        }
    };

    if decisions > 0 {
        info!(
            target: "4da::awe_autonomous",
            decisions,
            "tier0: graph already populated, seeding not needed"
        );
        return SeedOutcome::NotNeeded;
    }

    info!(target: "4da::awe_autonomous", "tier0: empty graph detected — running awe season");
    let seed_out = run_awe_with_timeout(Command::new(&awe_path).args(["season"]), 60);
    let loaded = match seed_out {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            parse_seed_loaded_count(&stdout)
        }
        Err(e) => {
            warn!(target: "4da::awe_autonomous", error = %e, "tier0: seed failed");
            return SeedOutcome::Failed;
        }
    };

    info!(target: "4da::awe_autonomous", loaded, "tier0: seeding complete");
    emit_awe_event(
        app,
        AweEvent::SeedComplete {
            decisions_loaded: loaded,
        },
    );
    emit_awe_event(app, AweEvent::SummaryStale);
    SeedOutcome::Seeded(())
}

/// Parse the decision count from `awe calibration` stdout. Format:
///     Total decisions tracked:  106
fn parse_total_decisions(stdout: &str) -> u64 {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Total decisions tracked:")
            || trimmed.starts_with("Decisions tracked:")
        {
            return trimmed
                .split_whitespace()
                .find_map(|w| w.parse::<u64>().ok())
                .unwrap_or(0);
        }
    }
    0
}

/// Parse the "N decisions loaded" line from `awe season` stdout. The CLI
/// prints either "Loaded N decisions" or a JSON summary — we scan for both
/// to stay forward-compatible with CLI formatting changes.
fn parse_seed_loaded_count(stdout: &str) -> u64 {
    // JSON form — look for `"loaded": N` or `"decisions_loaded": N`.
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout) {
        if let Some(n) = json
            .get("loaded")
            .or_else(|| json.get("decisions_loaded"))
            .and_then(|v| v.as_u64())
        {
            return n;
        }
    }
    // Plain-text form — look for "Loaded N" or similar at the start of any line.
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.to_lowercase().starts_with("loaded ") {
            if let Some(n) = trimmed
                .split_whitespace()
                .find_map(|w| w.parse::<u64>().ok())
            {
                return n;
            }
        }
    }
    0
}

// ============================================================================
// Tier 1 — LLM-assisted scan wrapper
// ============================================================================

/// Run `awe scan --repo <dir> --infer --classify --limit 200 --json` on
/// every configured context directory. The `--classify` flag is the
/// upgrade over `run_awe_auto_feedback` — it tells AWE to consult the LLM
/// for worthiness classification of ambiguous candidates, producing
/// higher-precision decision detection.
///
/// Returns a `(decisions_stored, outcomes_inferred)` tuple aggregated
/// across all scanned repos.
pub async fn run_tier1_classify_scan<R: Runtime>(app: &AppHandle<R>) -> (u64, u64) {
    let Some(awe_path) = find_awe_binary() else {
        return (0, 0);
    };

    let context_dirs = crate::get_context_dirs();
    let mut total_stored = 0u64;
    let mut total_inferred = 0u64;

    for dir in &context_dirs {
        let dir_str = dir.to_string_lossy();
        let result = run_awe_with_timeout(
            Command::new(&awe_path).args([
                "scan",
                "--repo",
                &dir_str,
                "--domain",
                "software-engineering",
                "--infer",
                "--classify",
                "--limit",
                "200",
                "--json",
            ]),
            120,
        );
        if let Ok(output) = result {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                total_stored += json
                    .get("decisions_stored")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                total_inferred += json
                    .get("outcomes_inferred")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
            }
        }
    }

    info!(
        target: "4da::awe_autonomous",
        stored = total_stored,
        inferred = total_inferred,
        repos = context_dirs.len(),
        "tier1: classify-scan complete"
    );

    emit_awe_event(
        app,
        AweEvent::ScanComplete {
            repos_scanned: context_dirs.len() as u64,
            decisions_stored: total_stored,
            outcomes_inferred: total_inferred,
        },
    );
    emit_awe_event(app, AweEvent::SummaryStale);

    (total_stored, total_inferred)
}

// ============================================================================
// Tier 3 — Retriage loop
// ============================================================================

/// Retriage outcome — what `awe retriage` reports.
#[derive(Debug, Clone, Default)]
pub struct RetriageOutcome {
    pub auto_confirmed: u64,
    pub demoted: u64,
    pub unchanged: u64,
}

/// Run `awe retriage --auto-confirm-threshold 0.7 --json`. This re-scores
/// every detected decision with the worthiness system and promotes the
/// high-scoring ones. It's the graph's self-pruning mechanism: low-signal
/// decisions get demoted, high-signal ones fast-track to principle
/// extraction.
///
/// This is called once per day from `monitoring.rs`.
pub async fn run_tier3_retriage<R: Runtime>(app: &AppHandle<R>) -> Result<RetriageOutcome, String> {
    let Some(awe_path) = find_awe_binary() else {
        return Err("AWE binary not found".into());
    };

    let result = run_awe_with_timeout(
        Command::new(&awe_path).args(["retriage", "--auto-confirm-threshold", "0.7", "--json"]),
        60,
    )
    .map_err(|e| format!("retriage failed: {e}"))?;

    let stdout = String::from_utf8_lossy(&result.stdout);
    let outcome = parse_retriage_outcome(&stdout);

    info!(
        target: "4da::awe_autonomous",
        auto_confirmed = outcome.auto_confirmed,
        demoted = outcome.demoted,
        unchanged = outcome.unchanged,
        "tier3: retriage complete"
    );

    emit_awe_event(
        app,
        AweEvent::RetriageComplete {
            auto_confirmed: outcome.auto_confirmed,
            demoted: outcome.demoted,
            unchanged: outcome.unchanged,
        },
    );
    if outcome.auto_confirmed > 0 || outcome.demoted > 0 {
        emit_awe_event(app, AweEvent::SummaryStale);
    }

    Ok(outcome)
}

/// Parse `awe retriage --json` output into counters. Accepts multiple
/// possible field names for forward-compatibility across AWE CLI versions.
fn parse_retriage_outcome(stdout: &str) -> RetriageOutcome {
    let mut out = RetriageOutcome::default();
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout) {
        out.auto_confirmed = json
            .get("auto_confirmed")
            .or_else(|| json.get("promoted"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        out.demoted = json
            .get("demoted")
            .or_else(|| json.get("purged"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        out.unchanged = json
            .get("unchanged")
            .or_else(|| json.get("stable"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
    }
    out
}

// ============================================================================
// Combined daily job
// ============================================================================

/// Run every autonomous tier in sequence. Called once per day from
/// `monitoring.rs`. Each tier is independent — a failure in one does not
/// abort the others.
pub async fn run_daily_autonomous_job<R: Runtime>(app: &AppHandle<R>) {
    // Tier 0 — seed only if the graph is empty. Cheap to check, cheap to skip.
    match run_tier0_seed_if_empty(app).await {
        SeedOutcome::Failed => {
            warn!(target: "4da::awe", "tier0: AWE seeding failed (non-fatal, continuing)");
        }
        other => {
            info!(target: "4da::awe", outcome = ?other, "tier0: seed check complete");
        }
    }

    // Tier 1 — classify-enhanced git scan. Replaces the old auto-feedback
    // scan for the daily path (startup still runs the old scan for speed).
    let (stored, inferred) = run_tier1_classify_scan(app).await;
    if stored == 0 && inferred == 0 {
        info!(target: "4da::awe", "tier1: classify-scan produced no new decisions");
    }

    // Tier 2 — source-item decision mining. Lives in its own module.
    if let Err(e) = crate::awe_commands::awe_source_mining::run_daily_source_mining(app).await {
        warn!(target: "4da::awe_autonomous", error = %e, "tier2: daily source mining failed");
    }

    // Tier 3 — retriage loop. Runs last so it catches everything tiers 1+2
    // created in the same daily cycle.
    if let Err(e) = run_tier3_retriage(app).await {
        warn!(target: "4da::awe_autonomous", error = %e, "tier3: retriage failed");
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_total_decisions ---------------------------------------------

    #[test]
    fn parses_total_decisions_tracked_line() {
        let stdout = "\
AWE Calibration Report
============================================================

  Total decisions tracked:  106
  Total feedback recorded:  97
  Feedback coverage:        92%";
        assert_eq!(parse_total_decisions(stdout), 106);
    }

    #[test]
    fn parses_short_form_decisions_tracked() {
        let stdout = "\
  Decisions tracked:     100
  With feedback:         88";
        assert_eq!(parse_total_decisions(stdout), 100);
    }

    #[test]
    fn parse_total_decisions_empty_returns_zero() {
        assert_eq!(parse_total_decisions(""), 0);
    }

    #[test]
    fn parse_total_decisions_missing_line_returns_zero() {
        assert_eq!(parse_total_decisions("some other output"), 0);
    }

    // ---- parse_seed_loaded_count -------------------------------------------

    #[test]
    fn parses_seed_json_loaded_field() {
        let stdout = r#"{"loaded": 42, "skipped": 3}"#;
        assert_eq!(parse_seed_loaded_count(stdout), 42);
    }

    #[test]
    fn parses_seed_json_decisions_loaded_field() {
        let stdout = r#"{"decisions_loaded": 30}"#;
        assert_eq!(parse_seed_loaded_count(stdout), 30);
    }

    #[test]
    fn parses_seed_plaintext_loaded_line() {
        let stdout = "Loaded 25 decisions from built-in corpus\nDone.";
        assert_eq!(parse_seed_loaded_count(stdout), 25);
    }

    #[test]
    fn parse_seed_missing_returns_zero() {
        assert_eq!(parse_seed_loaded_count("nothing useful here"), 0);
    }

    // ---- parse_retriage_outcome --------------------------------------------

    #[test]
    fn parses_retriage_json_standard() {
        let stdout = r#"{"auto_confirmed": 5, "demoted": 2, "unchanged": 30}"#;
        let out = parse_retriage_outcome(stdout);
        assert_eq!(out.auto_confirmed, 5);
        assert_eq!(out.demoted, 2);
        assert_eq!(out.unchanged, 30);
    }

    #[test]
    fn parses_retriage_legacy_field_names() {
        let stdout = r#"{"promoted": 3, "purged": 1, "stable": 20}"#;
        let out = parse_retriage_outcome(stdout);
        assert_eq!(out.auto_confirmed, 3);
        assert_eq!(out.demoted, 1);
        assert_eq!(out.unchanged, 20);
    }

    #[test]
    fn parse_retriage_empty_returns_defaults() {
        let out = parse_retriage_outcome("");
        assert_eq!(out.auto_confirmed, 0);
        assert_eq!(out.demoted, 0);
        assert_eq!(out.unchanged, 0);
    }

    #[test]
    fn parse_retriage_invalid_json_returns_defaults() {
        let out = parse_retriage_outcome("not json at all");
        assert_eq!(out.auto_confirmed, 0);
    }
}
