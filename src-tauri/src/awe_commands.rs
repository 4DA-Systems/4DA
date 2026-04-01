// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Page-specific AWE (Artificial Wisdom Engine) commands for 4DA.
//!
//! These commands provide structured JSON responses for each page's unique
//! wisdom needs. Unlike the generic commands in context_commands.rs, these
//! are designed for specific UI integration points.

use crate::context_commands::{find_awe_binary, run_awe_with_timeout};
use tracing::info;

type Result<T> = std::result::Result<T, String>;

/// Helper: require AWE binary or return error.
fn require_awe() -> Result<String> {
    find_awe_binary()
        .ok_or_else(|| "AWE binary not found. Build with: cargo build --release -p awe-cli".into())
}

/// Helper: run AWE command and return stdout as string.
fn run_awe(args: &[&str], timeout_secs: u64) -> Result<String> {
    let awe_path = require_awe()?;
    let output = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args(args),
        timeout_secs,
    )?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ============================================================================
// Candidate Principles — "Patterns are forming"
// ============================================================================

/// Get candidate principle counts from AWE wisdom report.
#[tauri::command]
pub async fn get_awe_candidates(domain: String) -> Result<String> {
    let result = run_awe(&["wisdom", "-d", &domain], 15);
    let (mut cand, mut anti, mut analyzed, mut cov) = (0u64, 0u64, 0u64, 0u64);
    if let Ok(stdout) = result {
        for line in stdout.lines() {
            let t = line.trim();
            if t.starts_with("Candidate principles:") {
                cand = t
                    .rsplit_once(':')
                    .and_then(|(_, v)| v.trim().parse().ok())
                    .unwrap_or(0);
            } else if t.starts_with("Candidate anti-patterns:") {
                anti = t
                    .rsplit_once(':')
                    .and_then(|(_, v)| v.trim().parse().ok())
                    .unwrap_or(0);
            } else if t.starts_with("Decisions analyzed:") {
                analyzed = t
                    .rsplit_once(':')
                    .and_then(|(_, v)| v.trim().parse().ok())
                    .unwrap_or(0);
            } else if t.starts_with("Feedback coverage:") {
                cov = t
                    .rsplit_once(':')
                    .and_then(|(_, v)| v.trim().trim_end_matches('%').parse().ok())
                    .unwrap_or(0);
            }
        }
    }
    Ok(serde_json::json!({
        "candidates": cand,
        "anti_patterns": anti,
        "decisions_analyzed": analyzed,
        "coverage_pct": cov,
    })
    .to_string())
}

// ============================================================================
// Interaction Feedback — User behavior feeds AWE
// ============================================================================

/// Record user interaction as AWE feedback. Non-blocking.
#[tauri::command]
pub async fn record_awe_interaction_feedback(
    item_title: String,
    interaction: String,
    source_type: String,
) -> Result<String> {
    let awe_path = match find_awe_binary() {
        Some(p) => p,
        None => return Ok(r#"{"status":"skipped"}"#.into()),
    };
    let outcome = match interaction.as_str() {
        "save" => "confirmed",
        "dismiss" | "mark_irrelevant" => "refuted",
        _ => "partial",
    };
    let verb = match interaction.as_str() {
        "save" => "saved",
        "dismiss" => "dismissed",
        "click" => "clicked",
        o => o,
    };
    let stmt = format!("User {} '{}' from {}", verb, item_title, source_type);
    let _ = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args([
            "transmute",
            &stmt,
            "--stages",
            "receive",
            "-d",
            "software-engineering",
        ]),
        10,
    );
    let id = format!("ux_{}", chrono::Utc::now().timestamp_millis());
    let _ = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args([
            "feedback",
            &id,
            "--outcome",
            outcome,
            "--details",
            &format!("Auto: user {} from {}", verb, source_type),
        ]),
        10,
    );
    info!(target: "4da::awe", %interaction, %outcome, "AWE interaction feedback");
    Ok(serde_json::json!({"status": "recorded", "outcome": outcome}).to_string())
}

// ============================================================================
// Pattern Matching — "I've seen this pattern before" (Briefing page)
// ============================================================================

/// Pattern-match a query against AWE's decision history.
///
/// Runs a partial transmutation (receive → synthesize → articulate) to find
/// precedents, relevant principles, and anti-patterns for the given topics.
/// Used by the Briefing page to annotate signals with historical context.
#[tauri::command]
pub async fn get_awe_pattern_match(query: String, domain: String) -> Result<String> {
    if query.trim().is_empty() {
        return Ok(serde_json::json!({
            "precedents": [],
            "principles": [],
            "anti_patterns": [],
        })
        .to_string());
    }

    let result = run_awe(
        &[
            "transmute",
            &query,
            "--stages",
            "receive,synthesize,articulate",
            "--json",
            "-d",
            &domain,
        ],
        15,
    );

    match result {
        Ok(stdout) => {
            // Extract the Synthesized stage from the JSON output
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                // Navigate to stages array and find Synthesized
                if let Some(stages) = json.get("stages").and_then(|s| s.as_array()) {
                    for stage in stages {
                        if let Some(arr) = stage.as_array() {
                            if arr.len() == 2 && arr[0].as_str() == Some("Synthesized") {
                                return Ok(arr[1].to_string());
                            }
                        }
                    }
                }
            }
            // Fallback: return raw output
            Ok(stdout)
        }
        Err(_) => Ok(serde_json::json!({
            "precedents": [],
            "principles": [],
            "anti_patterns": [],
        })
        .to_string()),
    }
}

// ============================================================================
// Decision History — "Here's what happened last time" (Results page)
// ============================================================================

/// Get AWE decision history for a domain.
///
/// Returns structured decision records with feedback status.
/// Used by the Results page to show decision backlinks on items.
#[tauri::command]
pub async fn get_awe_decision_history(domain: String, limit: usize) -> Result<String> {
    let limit_str = limit.to_string();
    let result = run_awe(
        &["history", "-d", &domain, "--limit", &limit_str, "--json"],
        15,
    );

    match result {
        Ok(stdout) => Ok(stdout),
        Err(_) => Ok(serde_json::json!({ "decisions": [] }).to_string()),
    }
}

// ============================================================================
// Pending Decisions — Feedback Queue (Console page)
// ============================================================================

/// Get pending decisions awaiting feedback.
///
/// Returns structured list of decisions without recorded outcomes.
/// Used by the Console's Feedback Queue panel.
#[tauri::command]
pub async fn get_awe_pending_decisions(limit: usize) -> Result<String> {
    let limit_str = limit.to_string();
    let result = run_awe(&["pending", "--limit", &limit_str, "--json"], 15);

    match result {
        Ok(stdout) => Ok(stdout),
        Err(_) => Ok(serde_json::json!({ "pending": [], "total": 0 }).to_string()),
    }
}

// ============================================================================
// Wisdom Well — Depth Spectrum (Console page)
// ============================================================================

/// Get the Wisdom Well depth visualization data.
///
/// Returns structured depth spectrum from Surface → Universal.
/// Used by the Console's Wisdom Well panel.
#[tauri::command]
pub async fn get_awe_wisdom_well(domain: String) -> Result<String> {
    let result = run_awe(&["well", "-d", &domain, "--json"], 15);

    match result {
        Ok(stdout) => Ok(stdout),
        Err(_) => Ok(serde_json::json!({
            "surface": [], "pattern": [], "principle": [],
            "causal": [], "meta": [], "universal": [],
        })
        .to_string()),
    }
}

// ============================================================================
// Growth Trajectory — "This is who you're becoming" (Profile page)
// ============================================================================

/// Compute AWE growth trajectory from calibration data.
///
/// Returns composite metrics: decisions/week trend, coverage trend,
/// principle count, and growth phase classification.
#[tauri::command]
pub async fn get_awe_growth_trajectory(domain: String) -> Result<String> {
    let awe_path = require_awe()?;

    // Get calibration data for metrics
    let cal_output = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args(["calibration", "-d", &domain]),
        15,
    );

    let mut decisions = 0u64;
    let mut feedback = 0u64;
    let mut principles = 0u64;
    let mut coverage = 0u64;

    if let Ok(output) = cal_output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let t = line.trim();
            if t.contains("decisions tracked") || t.contains("Decisions tracked") {
                if let Some(n) = t.split_whitespace().find(|w| w.parse::<u64>().is_ok()) {
                    decisions = n.parse().unwrap_or(0);
                }
            } else if t.contains("feedback") && t.contains("recorded") {
                if let Some(n) = t.split_whitespace().find(|w| w.parse::<u64>().is_ok()) {
                    feedback = n.parse().unwrap_or(0);
                }
            } else if t.contains("coverage") || t.contains("Coverage") {
                if let Some(pct) = t.split_whitespace().find(|w| w.ends_with('%')) {
                    coverage = pct.trim_end_matches('%').parse().unwrap_or(0);
                }
            } else if t.contains("principles") && t.contains("Validated") {
                if let Some(n) = t.split_whitespace().find(|w| w.parse::<u64>().is_ok()) {
                    principles = n.parse().unwrap_or(0);
                }
            }
        }
    }

    // Classify growth phase
    let growth_phase = if decisions < 10 {
        "cold_start"
    } else if coverage < 50 {
        "accumulating"
    } else if principles > 0 {
        "compounding"
    } else {
        "accumulating"
    };

    info!(
        target: "4da::awe",
        decisions, feedback, principles, coverage, growth_phase,
        "AWE growth trajectory computed"
    );

    Ok(serde_json::json!({
        "decisions": decisions,
        "feedback_count": feedback,
        "feedback_coverage": coverage,
        "principles_formed": principles,
        "anti_patterns_detected": 0,
        "growth_phase": growth_phase,
    })
    .to_string())
}

// ============================================================================
// Batch Feedback — Proactive feedback submission (Console page)
// ============================================================================

/// Submit batch feedback for multiple decisions.
///
/// Each feedback entry is a {decision_id, outcome, details} triple.
/// Used by the Console's Feedback Queue "Resolve All" feature.
#[tauri::command]
pub async fn submit_awe_batch_feedback(feedbacks: Vec<serde_json::Value>) -> Result<String> {
    let awe_path = require_awe()?;
    let mut success_count = 0;
    let mut error_count = 0;

    for fb in &feedbacks {
        let decision_id = fb.get("decision_id").and_then(|v| v.as_str()).unwrap_or("");
        let outcome = fb.get("outcome").and_then(|v| v.as_str()).unwrap_or("");
        let details = fb
            .get("details")
            .and_then(|v| v.as_str())
            .unwrap_or("Batch feedback from 4DA");

        if decision_id.is_empty() || outcome.is_empty() {
            error_count += 1;
            continue;
        }

        let result = run_awe_with_timeout(
            std::process::Command::new(&awe_path).args([
                "feedback",
                decision_id,
                "--outcome",
                outcome,
                "--details",
                details,
            ]),
            10,
        );

        if result.is_ok() {
            success_count += 1;
        } else {
            error_count += 1;
        }
    }

    info!(
        target: "4da::awe",
        success = success_count,
        errors = error_count,
        total = feedbacks.len(),
        "Batch feedback submitted"
    );

    Ok(serde_json::json!({
        "submitted": success_count,
        "errors": error_count,
        "total": feedbacks.len(),
    })
    .to_string())
}

// ============================================================================
// Auto-Feedback — Push coverage from git history (scheduled)
// ============================================================================

/// Run AWE auto-feedback inference from git history.
///
/// Scans configured context directories for decisions, infers outcomes
/// from git patterns (time-stability, reverts), and records feedback.
/// Called by the monitoring scheduler daily or on-demand from Console.
#[tauri::command]
pub async fn run_awe_auto_feedback() -> Result<String> {
    let awe_path = require_awe()?;

    let context_dirs = crate::get_context_dirs();
    let mut total_inferred = 0;
    let mut total_stored = 0;

    for dir in &context_dirs {
        let dir_str = dir.to_string_lossy();
        let result = run_awe_with_timeout(
            std::process::Command::new(&awe_path).args([
                "scan",
                "--repo",
                &dir_str,
                "--domain",
                "software-engineering",
                "--infer",
                "--limit",
                "200",
                "--json",
            ]),
            60,
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
        target: "4da::awe",
        stored = total_stored,
        inferred = total_inferred,
        "AWE auto-feedback scan complete"
    );

    Ok(serde_json::json!({
        "decisions_stored": total_stored,
        "outcomes_inferred": total_inferred,
        "repos_scanned": context_dirs.len(),
    })
    .to_string())
}

// ============================================================================
// Purge — Remove stale principles (on-demand from Console)
// ============================================================================

/// Run AWE principle purge to remove below-threshold principles.
///
/// Returns count of purged vs kept principles.
#[tauri::command]
pub async fn run_awe_purge(dry_run: bool) -> Result<String> {
    let mut args = vec!["purge", "--json"];
    if dry_run {
        args.push("--dry-run");
    }
    run_awe(&args, 15)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_awe_returns_some_on_this_machine() {
        // This test only passes on the dev machine with AWE built
        if std::path::Path::new("D:\\runyourempire\\awe\\target\\release\\awe.exe").exists() {
            assert!(require_awe().is_ok());
        }
    }

    #[test]
    fn run_awe_handles_bad_args() {
        // Invalid subcommand should return error or empty
        let result = run_awe(&["nonexistent_command"], 5);
        // Either fails or returns something — just shouldn't panic
        let _ = result;
    }
}
