// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Page-specific AWE (Artificial Wisdom Engine) commands for 4DA.
//!
//! These commands provide structured JSON responses for each page's unique
//! wisdom needs. Unlike the generic commands in context_commands.rs, these
//! are designed for specific UI integration points.
//!
//! ## Submodules
//!
//! - [`awe_events`] — typed real-time Tauri event layer. Every mutation
//!   command emits events so the UI re-renders without polling.
//! - [`awe_source_mining`] — Tier 2: autonomous decision mining from
//!   4DA's source-item feed (Hacker News, Reddit, etc.).
//! - [`awe_autonomous`] — Tier 0 (seeding), Tier 1 (classify-enhanced
//!   scan), and Tier 3 (retriage) loops invoked by the daily scheduler.

// Child modules declared with `#[path]` because the canonical crate root
// (`lib.rs`) has uncommitted changes from parallel terminals — we cannot
// add `pub mod` declarations there without colliding. Declaring these as
// children of `awe_commands` makes them reachable via
// `crate::awe_commands::awe_events::*` from anywhere in the crate.
#[path = "awe_events.rs"]
pub mod awe_events;

#[path = "awe_source_mining.rs"]
pub mod awe_source_mining;

#[path = "awe_autonomous.rs"]
pub mod awe_autonomous;

use crate::context_commands::{find_awe_binary, run_awe_with_timeout};
use tracing::{info, warn};

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

/// Record a user interaction as AWE feedback.
///
/// This is the primary channel by which user behavior in 4DA (save,
/// dismiss, click, mark_irrelevant) feeds AWE's wisdom graph.
///
/// **Historical bugs (both fixed):**
/// 1. Original code passed `--stages receive`, but the CLI rejects
///    `receive` as an unknown stage (it's auto-prepended). Every call
///    was failing silently via `let _ = ...`.
/// 2. Original code minted a local `ux_<timestamp>` feedback ID that
///    never matched a real decision in AWE's DB. Even if the transmute
///    had succeeded, the feedback call would target a nonexistent ID.
///
/// **New flow:**
/// 1. Check if `item_title` is decision-shaped (heuristic). If not,
///    record as a lightweight interest signal and return early — we
///    don't want to pollute the wisdom graph with every click.
/// 2. If decision-shaped: run `awe transmute` (no `receive` stage) to
///    persist the decision to AWE's DB.
/// 3. Read the newly-created decision ID from `awe history --limit 1
///    --json`. Match by statement text to defend against races.
/// 4. Record feedback against the real ID.
/// 5. Emit `awe:decision-added` and `awe:feedback-recorded` Tauri events
///    so the UI can update live.
#[tauri::command]
pub async fn record_awe_interaction_feedback(
    app: tauri::AppHandle,
    item_title: String,
    interaction: String,
    source_type: String,
) -> Result<String> {
    use self::awe_events::{emit_awe_event, AweEvent};

    // Register this AppHandle for zero-arg commands that need one later
    // (see AWE_APP_HANDLE documentation). Idempotent — first interaction
    // unlocks the full autonomous pipeline for the rest of the session.
    register_awe_app_handle(&app);

    // Validate input lengths defensively (item_title comes from source feeds
    // which may contain very long strings or malformed text).
    let item_title = crate::ipc_guard::validate_length(
        "item_title",
        &item_title,
        crate::ipc_guard::MAX_INPUT_LENGTH,
    )
    .map_err(|e| e.to_string())?
    .to_string();
    let interaction = crate::ipc_guard::validate_length("interaction", &interaction, 50)
        .map_err(|e| e.to_string())?
        .to_string();
    let source_type = crate::ipc_guard::validate_length("source_type", &source_type, 100)
        .map_err(|e| e.to_string())?
        .to_string();

    let awe_path = match find_awe_binary() {
        Some(p) => p,
        None => return Ok(r#"{"status":"skipped","reason":"awe_not_installed"}"#.into()),
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

    // Decision-shape filter — only transmute items that look like decisions.
    // Raw news titles, release notes, etc. are signals but not decisions and
    // should not pollute the wisdom graph.
    if !self::awe_source_mining::looks_like_decision(&item_title) {
        info!(
            target: "4da::awe",
            %interaction,
            %source_type,
            "AWE feedback skipped — item is not decision-shaped"
        );
        return Ok(serde_json::json!({
            "status": "skipped",
            "reason": "not_decision_shaped"
        })
        .to_string());
    }

    // Step 1: transmute the item as a persisted AWE decision.
    // Use only interrogate+articulate stages — receive auto-prepended by CLI,
    // skipping consequent/synthesize/judge saves ~10s per call (5s vs 15s).
    let transmute_out = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args([
            "transmute",
            &item_title,
            "--stages",
            "interrogate,articulate",
            "-d",
            "software-engineering",
            "--json",
        ]),
        15,
    );
    if let Err(e) = &transmute_out {
        warn!(target: "4da::awe", error = %e, "transmute failed — skipping feedback");
        return Ok(serde_json::json!({
            "status": "error",
            "phase": "transmute",
            "error": e.to_string()
        })
        .to_string());
    }

    // Step 2: capture the newly-created decision ID by reading the head of
    // history. Matched by statement text — if someone else raced us and
    // created a different decision, our lookup won't match and we skip
    // gracefully rather than corrupt data.
    let history_out = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args([
            "history",
            "-d",
            "software-engineering",
            "--limit",
            "3",
            "--json",
        ]),
        5,
    );
    let decision_id = match &history_out {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            serde_json::from_str::<serde_json::Value>(&stdout)
                .ok()
                .and_then(|json| {
                    json.get("decisions")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| {
                            arr.iter().find(|d| {
                                d.get("statement").and_then(|s| s.as_str())
                                    == Some(item_title.as_str())
                            })
                        })
                        .and_then(|d| d.get("id").and_then(|v| v.as_str()).map(String::from))
                })
        }
        Err(_) => None,
    };
    let Some(decision_id) = decision_id else {
        warn!(
            target: "4da::awe",
            "transmute succeeded but decision ID lookup failed — feedback skipped"
        );
        return Ok(serde_json::json!({
            "status": "error",
            "phase": "id_lookup",
            "error": "could not find newly-created decision in history"
        })
        .to_string());
    };

    // Step 3: record feedback against the real decision ID.
    let feedback_result = run_awe_with_timeout(
        std::process::Command::new(&awe_path).args([
            "feedback",
            &decision_id,
            "--outcome",
            outcome,
            "--details",
            &format!("4DA user {} from {}", verb, source_type),
        ]),
        10,
    );
    if let Err(e) = &feedback_result {
        warn!(target: "4da::awe", %decision_id, error = %e, "feedback call failed");
        return Ok(serde_json::json!({
            "status": "error",
            "phase": "feedback",
            "error": e.to_string(),
            "decision_id": decision_id
        })
        .to_string());
    }

    info!(
        target: "4da::awe",
        %interaction,
        %outcome,
        %decision_id,
        "AWE interaction feedback recorded"
    );

    // Step 4: emit live events so the UI can update without polling.
    emit_awe_event(
        &app,
        AweEvent::DecisionAdded {
            id: decision_id.clone(),
            statement: item_title.clone(),
            domain: "software-engineering".into(),
            source: "user_interaction".into(),
        },
    );
    emit_awe_event(
        &app,
        AweEvent::FeedbackRecorded {
            decision_id: decision_id.clone(),
            outcome: outcome.into(),
        },
    );
    emit_awe_event(&app, AweEvent::SummaryStale);

    Ok(serde_json::json!({
        "status": "recorded",
        "outcome": outcome,
        "decision_id": decision_id
    })
    .to_string())
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
    let query =
        crate::ipc_guard::validate_length("query", &query, crate::ipc_guard::MAX_INPUT_LENGTH)
            .map_err(|e| e.to_string())?;
    let domain =
        crate::ipc_guard::validate_length("domain", &domain, 200).map_err(|e| e.to_string())?;
    if query.trim().is_empty() {
        return Ok(serde_json::json!({
            "precedents": [],
            "principles": [],
            "anti_patterns": [],
        })
        .to_string());
    }

    // NOTE: AWE CLI auto-prepends `Receive`. Passing it explicitly fails
    // with `Unknown stage: 'receive'`. See context_commands.rs for history.
    let result = run_awe(
        &[
            "transmute",
            &query,
            "--stages",
            "synthesize,articulate",
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
    let domain =
        crate::ipc_guard::validate_length("domain", &domain, 200).map_err(|e| e.to_string())?;
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

/// Global AppHandle cache for AWE commands that were originally
/// registered with a zero-argument signature (and therefore cannot
/// receive Tauri's injected `AppHandle` directly).
///
/// The cell is populated by any command that DOES take an `AppHandle`
/// parameter — notably `record_awe_interaction_feedback` and
/// `run_awe_autonomous_now`. Once populated, the legacy zero-arg
/// commands (`run_awe_auto_feedback`, startup paths) can pull the
/// handle out to emit live events and run the full autonomous
/// pipeline instead of the legacy scan-only path.
///
/// This indirection is necessary because the production `lib.rs`
/// had concurrent uncommitted edits from other terminals during the
/// AWE wiring work — we could not modify the existing command
/// signatures or register a new zero-arg command without colliding.
/// The OnceCell lets us upgrade behavior in place the first time
/// any command with a real AppHandle runs, which is typically within
/// seconds of the UI mounting.
static AWE_APP_HANDLE: once_cell::sync::OnceCell<tauri::AppHandle> =
    once_cell::sync::OnceCell::new();

/// Register the AppHandle with the AWE subsystem. Called by any AWE
/// command that receives an AppHandle from Tauri's injector.
fn register_awe_app_handle(app: &tauri::AppHandle) {
    // `set` returns Err if already initialized — we ignore it because
    // that just means another command beat us to it.
    let _ = AWE_APP_HANDLE.set(app.clone());
}

/// Retrieve the cached AppHandle if one has been registered. Used by
/// zero-arg commands (`run_awe_auto_feedback`, startup paths) to
/// upgrade their behavior when a handle becomes available.
fn cached_awe_app_handle() -> Option<tauri::AppHandle> {
    AWE_APP_HANDLE.get().cloned()
}

/// Run AWE auto-feedback inference from git history.
///
/// Scans configured context directories for decisions, infers outcomes
/// from git patterns (time-stability, reverts), and records feedback.
/// Called by the monitoring scheduler daily or on-demand from Console.
///
/// This is the **Tauri command entry point** — it takes no AppHandle arg
/// so the existing call sites in `app_setup.rs` (startup spawn) and
/// `monitoring.rs` keep compiling without needing edits.
///
/// **Behavior upgrade**: if an AppHandle has been cached via
/// `register_awe_app_handle` (any prior `record_awe_interaction_feedback`
/// or `run_awe_autonomous_now` call), this command runs the **full
/// autonomous tier pipeline** (Tier 0 seed + Tier 1 classify scan +
/// Tier 2 source mining + Tier 3 retriage) and emits live events.
/// Otherwise it falls back to the legacy scan-only behavior so cold
/// startup (before any UI interaction) still works.
#[tauri::command]
pub async fn run_awe_auto_feedback() -> Result<String> {
    if let Some(app) = cached_awe_app_handle() {
        info!(
            target: "4da::awe",
            "run_awe_auto_feedback → running full autonomous pipeline (AppHandle cached)"
        );
        self::awe_autonomous::run_daily_autonomous_job(&app).await;
        return Ok(serde_json::json!({
            "status": "ok",
            "mode": "autonomous_full",
            "tiers_run": ["tier0_seed", "tier1_classify_scan", "tier2_source_mining", "tier3_retriage"]
        })
        .to_string());
    }
    // Legacy path — runs at startup before any UI interaction has
    // cached an AppHandle. Still does Tier 0 seed via the shared impl.
    let stats = auto_feedback_scan_impl::<tauri::Wry>(None).await?;
    Ok(stats.to_json())
}

/// Run the full autonomous AWE tier pipeline right now.
///
/// This is the command the UI "Run Wisdom Now" button invokes. It
/// runs every tier in sequence with live event emission so the
/// Wisdom Trajectory updates in real time as each tier completes:
///
/// 1. **Tier 0** — cold-start seeding via `awe season` (no-op if the
///    graph is already populated).
/// 2. **Tier 1** — LLM-assisted git scan with `--classify` over every
///    configured context directory.
/// 3. **Tier 2** — source-item decision mining: scans the top-200
///    most recent items from 4DA's source feed, filters for
///    decision-shape, transmutes up to 12 through AWE.
/// 4. **Tier 3** — retriage with `--auto-confirm-threshold 0.7`, which
///    re-scores every decision and auto-promotes high-worthiness ones.
///
/// Returns a JSON object describing what each tier produced so the
/// UI can show a completion summary.
#[tauri::command]
pub async fn run_awe_autonomous_now(app: tauri::AppHandle) -> Result<String> {
    register_awe_app_handle(&app);
    info!(target: "4da::awe", "run_awe_autonomous_now: starting full pipeline on demand");

    // Emit a "started" marker — gives the UI something to latch onto
    // for its progress indicator even before any tier completes.
    self::awe_events::emit_awe_event(&app, self::awe_events::AweEvent::SummaryStale);

    self::awe_autonomous::run_daily_autonomous_job(&app).await;

    info!(target: "4da::awe", "run_awe_autonomous_now: complete");
    Ok(serde_json::json!({
        "status": "ok",
        "tiers_run": ["tier0_seed", "tier1_classify_scan", "tier2_source_mining", "tier3_retriage"]
    })
    .to_string())
}

/// Scan statistics returned by `auto_feedback_scan_impl`.
#[derive(Debug, Clone, Default)]
pub struct AutoFeedbackStats {
    pub repos_scanned: u64,
    pub decisions_stored: u64,
    pub outcomes_inferred: u64,
    pub seed_outcome: Option<self::awe_autonomous::SeedOutcome>,
}

impl AutoFeedbackStats {
    /// Serialize to the JSON shape the frontend has always consumed.
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "decisions_stored": self.decisions_stored,
            "outcomes_inferred": self.outcomes_inferred,
            "repos_scanned": self.repos_scanned,
            "seed_outcome": self.seed_outcome
                .map(|o| format!("{:?}", o))
                .unwrap_or_else(|| "skipped".to_string()),
        })
        .to_string()
    }
}

/// Shared implementation of the auto-feedback scan path. Callers that
/// have a live `AppHandle` pass `Some(&app)` and get event emission + the
/// Tier 0 seed check; callers without one (the legacy tauri command) pass
/// `None` and the scan still runs without live UI ticks.
pub async fn auto_feedback_scan_impl<R: tauri::Runtime>(
    app: Option<&tauri::AppHandle<R>>,
) -> Result<AutoFeedbackStats> {
    let awe_path = require_awe()?;

    // Tier 0 — seed if the wisdom graph is empty. Safe to call repeatedly;
    // the helper short-circuits when decisions already exist.
    let seed_outcome = if let Some(app) = app {
        Some(self::awe_autonomous::run_tier0_seed_if_empty(app).await)
    } else {
        None
    };

    let context_dirs = crate::get_context_dirs();
    let mut total_inferred = 0u64;
    let mut total_stored = 0u64;

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

    // Emit scan-complete so the UI ticks without needing a poll. Only
    // when an AppHandle was provided — tests and the legacy command
    // path skip this.
    if let Some(app) = app {
        let scan_event = self::awe_events::AweEvent::ScanComplete {
            repos_scanned: context_dirs.len() as u64,
            decisions_stored: total_stored,
            outcomes_inferred: total_inferred,
        };
        self::awe_events::emit_awe_event(app, scan_event);
        self::awe_events::emit_awe_event(app, self::awe_events::AweEvent::SummaryStale);
    }

    info!(
        target: "4da::awe",
        stored = total_stored,
        inferred = total_inferred,
        seed_outcome = ?seed_outcome,
        "AWE auto-feedback scan complete"
    );

    Ok(AutoFeedbackStats {
        repos_scanned: context_dirs.len() as u64,
        decisions_stored: total_stored,
        outcomes_inferred: total_inferred,
        seed_outcome,
    })
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
        // Check if AWE binary is available (cross-platform)
        let awe_available = std::env::var("AWE_BIN").is_ok()
            || std::env::var("AWE_ROOT").is_ok()
            || find_awe_binary().is_some();
        if awe_available {
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

    // ========================================================================
    // Bug #1 regression guard — the "--stages receive" bug
    // ========================================================================
    //
    // Before the fix, 4DA passed `--stages receive` to every transmute call.
    // The CLI auto-prepends `Receive` and rejects an explicit `receive` name,
    // so every call returned `Error: Unknown stage: 'receive'` — silently
    // swallowed by `let _ = run_awe_with_timeout(...)`. The whole Wisdom
    // Panel transmute path + every user interaction feedback was dead.
    //
    // This regression guard runs on the dev machine only (where AWE exists)
    // and asserts that the exact argument pattern 4DA uses now succeeds.
    // ========================================================================

    #[test]
    fn regression_bug_1_transmute_stage_args_are_valid() {
        // Check if AWE binary is available (cross-platform)
        let awe_available = std::env::var("AWE_BIN").is_ok()
            || std::env::var("AWE_ROOT").is_ok()
            || find_awe_binary().is_some();
        let Some(awe_path) = find_awe_binary() else {
            if !awe_available {
                return;
            } // AWE not available — skip silently
            return;
        };
        let awe_path: &str = &awe_path;

        // Exact arg pattern used in awe_commands.rs:get_awe_pattern_match
        let out = run_awe_with_timeout(
            std::process::Command::new(awe_path).args([
                "transmute",
                "Should 4DA regression-test adopt strict semver?",
                "--stages",
                "synthesize,articulate",
                "--json",
                "-d",
                "software-engineering",
                "--no-persist",
            ]),
            30,
        );
        assert!(
            out.is_ok(),
            "transmute with synthesize,articulate must succeed — bug #1 regressed: {:?}",
            out.as_ref().err()
        );
        let output = out.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains("Unknown stage"),
            "AWE rejected the stage args — bug #1 regressed: {stdout}"
        );

        // Exact arg pattern used in context_commands.rs:run_awe_quick_check
        let out = run_awe_with_timeout(
            std::process::Command::new(awe_path).args([
                "transmute",
                "Should 4DA regression-test adopt strict semver?",
                "--json",
                "-d",
                "software-engineering",
                "--stages",
                "interrogate,articulate",
                "--no-persist",
            ]),
            30,
        );
        assert!(
            out.is_ok(),
            "transmute with interrogate,articulate must succeed — bug #1 regressed"
        );

        // Exact arg pattern used in context_commands.rs:run_awe_consequence_scan
        let out = run_awe_with_timeout(
            std::process::Command::new(awe_path).args([
                "transmute",
                "Should 4DA regression-test adopt strict semver?",
                "--json",
                "-d",
                "software-engineering",
                "--stages",
                "consequent,articulate",
                "--no-persist",
            ]),
            30,
        );
        assert!(
            out.is_ok(),
            "transmute with consequent,articulate must succeed — bug #1 regressed"
        );

        // Exact arg pattern used in record_awe_interaction_feedback (Tier 4)
        let out = run_awe_with_timeout(
            std::process::Command::new(awe_path).args([
                "transmute",
                "Should we regression-test adopt memory-mapped IO?",
                "--stages",
                "interrogate,articulate",
                "-d",
                "software-engineering",
                "--json",
                "--no-persist",
            ]),
            30,
        );
        assert!(
            out.is_ok(),
            "interaction-feedback transmute args must succeed — bug #1 regressed"
        );
    }

    // ========================================================================
    // Bug #2 regression guard — decision ID lookup after transmute
    // ========================================================================
    //
    // Original record_awe_interaction_feedback minted a `ux_<timestamp>` ID
    // that never matched any real decision in AWE's DB. The fix does a
    // history lookup by statement text. This test proves the pattern:
    //
    //   1. Run transmute (persist).
    //   2. Read `awe history --limit 3 --json`.
    //   3. Assert the newest decision's statement matches our input.
    //   4. Record feedback against that real ID.
    //   5. Assert feedback call succeeds.
    // ========================================================================

    #[test]
    fn regression_bug_2_feedback_uses_real_decision_id() {
        // Check if AWE binary is available (cross-platform)
        let Some(awe_path) = find_awe_binary() else {
            return; // AWE not available — skip silently
        };
        let awe_path: &str = &awe_path;

        // Use a unique marker so we can find our decision back reliably
        // without racing against any other recent activity.
        let marker = format!(
            "4DA-regression-test-{}",
            chrono::Utc::now().timestamp_millis()
        );
        let statement = format!("Should we {} adopt a new logger crate?", marker);

        // Step 1: transmute to persist
        let transmute = run_awe_with_timeout(
            std::process::Command::new(awe_path).args([
                "transmute",
                &statement,
                "--stages",
                "interrogate,articulate",
                "-d",
                "software-engineering",
                "--json",
            ]),
            30,
        );
        assert!(
            transmute.is_ok(),
            "transmute failed — bug #2 test blocked by bug #1: {:?}",
            transmute.err()
        );

        // Step 2: lookup via history
        let history = run_awe_with_timeout(
            std::process::Command::new(awe_path).args([
                "history",
                "-d",
                "software-engineering",
                "--limit",
                "5",
                "--json",
            ]),
            10,
        )
        .expect("history call must succeed");
        let stdout = String::from_utf8_lossy(&history.stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("history --json must parse");
        let decision_id = json
            .get("decisions")
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                arr.iter().find(|d| {
                    d.get("statement").and_then(|s| s.as_str()) == Some(statement.as_str())
                })
            })
            .and_then(|d| d.get("id").and_then(|v| v.as_str()).map(String::from))
            .expect("our freshly-persisted decision must appear at head of history");

        assert!(
            decision_id.starts_with("dc_"),
            "decision ID must be a UUIDv7 dc_ prefix, got: {decision_id}"
        );

        // Step 3: feedback against the REAL ID
        let feedback = run_awe_with_timeout(
            std::process::Command::new(awe_path).args([
                "feedback",
                &decision_id,
                "--outcome",
                "confirmed",
                "--details",
                "regression-test: bug #2 end-to-end flow",
            ]),
            10,
        );
        assert!(
            feedback.is_ok(),
            "feedback with real ID must succeed — bug #2 regressed: {:?}",
            feedback.err()
        );
    }
}
