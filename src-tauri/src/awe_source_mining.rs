// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Tier 2 — Autonomous source-item decision mining.
//!
//! 4DA already fetches ~33k items per week from Hacker News, Reddit, Lobsters,
//! Dev.to, StackOverflow, CVE, OSV, npm/pypi/crates, arXiv, and several other
//! sources. Of those, a small fraction look decision-shaped — titles like
//! "Should we adopt X?", "Tauri vs Electron for desktop apps", "Why I switched
//! from Y to Z", etc. Before this module, none of them reached AWE's wisdom
//! graph. That was the biggest untapped signal source in the whole system.
//!
//! This module:
//!
//! 1. Exposes `looks_like_decision()` — a fast regex-based heuristic the
//!    rest of the codebase uses to decide whether to transmute an item.
//!    It's deliberately conservative: false negatives are fine (we don't
//!    pollute the graph), false positives are the risk (we waste LLM cost).
//! 2. Exposes `run_source_mining_batch()` — the top-of-day batch that
//!    scans the most recent relevant items, picks the top decision-shaped
//!    ones, transmutes them through AWE with rate limiting, and emits a
//!    `SourceMiningComplete` event when done.
//!
//! The batch is invoked from `monitoring.rs` once per day alongside the
//! other autonomous AWE jobs. It reuses `run_awe_with_timeout` so all
//! configuration (LLM key injection, timeouts, error logging) is shared
//! with the rest of the AWE command surface.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tracing::{info, warn};

use crate::awe_commands::awe_events::{emit_awe_event, AweEvent};

// ============================================================================
// Decision-shape heuristic
// ============================================================================

/// Tiny fast heuristic: does this title look like a decision?
///
/// A "decision" in the AWE sense is a question the developer (or the
/// community) is weighing — "should we", "X vs Y", "migrating from",
/// "why I chose", "trade-off", etc. Plain news ("Rust 1.80 released"),
/// pure tutorials ("Guide to X"), and non-decision discussion are
/// deliberately excluded.
///
/// **False-negative bias**: we'd rather miss a borderline case than
/// pollute the wisdom graph with non-decision content. Precision > recall.
///
/// Design notes:
/// - Lowercase comparison — robust to title case variance.
/// - Keyword-matching (not regex) so the hot path stays O(n) per title
///   and avoids a regex engine dependency.
/// - Minimum 20-character title length filters out IDs and stubs that
///   slipped past the source quality gate.
pub fn looks_like_decision(title: &str) -> bool {
    if title.len() < 20 {
        return false;
    }
    let lower = title.to_lowercase();
    let trimmed = lower.trim();

    // --- Strong signals (any one triggers) --------------------------------
    const QUESTION_STARTERS: &[&str] = &[
        "should i",
        "should we",
        "should you",
        "is it worth",
        "is it time to",
        "why i ",
        "why we ",
        "why not ",
        "do you ",
        "when to ",
        "when should",
        "how i stopped",
        "how we stopped",
        "what i learned",
        "what we learned",
    ];
    for starter in QUESTION_STARTERS {
        if trimmed.starts_with(starter) {
            return true;
        }
    }

    // --- X vs Y / comparison --------------------------------------------------
    // "python vs rust", "postgres vs mysql", "sqlite vs postgres"
    if trimmed.contains(" vs ") || trimmed.contains(" vs. ") {
        return true;
    }

    // --- Migration / switching signals --------------------------------------
    const MIGRATION_MARKERS: &[&str] = &[
        "migrated from",
        "migrating from",
        "moving from",
        "switched from",
        "switching from",
        "replacing ",
        "we replaced ",
        "ditching ",
        "abandoning ",
        "giving up on",
    ];
    for m in MIGRATION_MARKERS {
        if trimmed.contains(m) {
            return true;
        }
    }

    // --- Trade-off / choice language -----------------------------------------
    const CHOICE_MARKERS: &[&str] = &[
        "trade-off",
        "tradeoff",
        "pros and cons",
        "choosing between",
        "picking the right",
        "when not to use",
        "when to use",
        "to use or not to use",
    ];
    for m in CHOICE_MARKERS {
        if trimmed.contains(m) {
            return true;
        }
    }

    // --- "Is X worth it?" / "X considered harmful" pattern -------------------
    if trimmed.contains("worth it")
        || trimmed.contains("considered harmful")
        || trimmed.contains("is dead")
        || trimmed.contains("is over")
    {
        return true;
    }

    false
}

// ============================================================================
// Batch mining job
// ============================================================================

/// Outcome of a single source-mining batch run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMiningStats {
    pub candidates_considered: u64,
    pub decision_shaped: u64,
    pub decisions_created: u64,
    pub rate_limited: u64,
    pub errors: u64,
}

impl Default for SourceMiningStats {
    fn default() -> Self {
        Self {
            candidates_considered: 0,
            decision_shaped: 0,
            decisions_created: 0,
            rate_limited: 0,
            errors: 0,
        }
    }
}

/// Default per-run budget. Conservative — full-pipeline transmute with
/// local Ollama is ~5-10s per item, so 12 items is ~1-2 minutes of
/// compute per day, which is a very reasonable cost for a background job.
const DEFAULT_MAX_TRANSMUTES_PER_RUN: usize = 12;

/// Run a single source-mining batch.
///
/// Reads the latest N source items from the DB, filters for decision-shape,
/// transmutes up to `max_transmutes_per_run` of them through AWE, and emits
/// a `SourceMiningComplete` event when done. Safe to call repeatedly — AWE's
/// own wisdom delta will deduplicate semantically-similar decisions.
///
/// Returns `SourceMiningStats` so callers (monitoring.rs, integration tests)
/// can assert on the outcome.
pub async fn run_source_mining_batch<R: Runtime>(
    app: &AppHandle<R>,
    max_transmutes_per_run: usize,
) -> Result<SourceMiningStats, String> {
    let awe_path = match crate::context_commands::find_awe_binary() {
        Some(p) => p,
        None => {
            info!(target: "4da::awe_source_mining", "AWE binary missing — skipping batch");
            return Ok(SourceMiningStats::default());
        }
    };

    // Pull the 200 most-recent items from the DB. We deliberately query the
    // raw source_items table rather than relevance_results — a decision-shaped
    // item that scored LOW is still valuable wisdom input (maybe even more so,
    // because "community is debating this" signals a real trade-off).
    let db = crate::get_database().map_err(|e| format!("db unavailable: {e}"))?;
    let items = db
        .get_items_tiered(168, 200)
        .map_err(|e| format!("get_items_tiered failed: {e}"))?;

    let mut stats = SourceMiningStats {
        candidates_considered: items.len() as u64,
        ..Default::default()
    };

    // Filter for decision-shape. Also dedupe by content hash via title — same
    // headline reposted to multiple sources shouldn't consume the budget twice.
    let mut seen_titles: std::collections::HashSet<String> = std::collections::HashSet::new();
    let decision_candidates: Vec<_> = items
        .into_iter()
        .filter(|item| looks_like_decision(&item.title))
        .filter(|item| seen_titles.insert(item.title.to_lowercase()))
        .collect();
    stats.decision_shaped = decision_candidates.len() as u64;

    // Rate-limit: take only the N newest unique decision-shaped items.
    let to_process: Vec<_> = decision_candidates
        .into_iter()
        .take(max_transmutes_per_run)
        .collect();
    stats.rate_limited = stats
        .decision_shaped
        .saturating_sub(to_process.len() as u64);

    info!(
        target: "4da::awe_source_mining",
        candidates = stats.candidates_considered,
        decision_shaped = stats.decision_shaped,
        will_transmute = to_process.len(),
        rate_limited = stats.rate_limited,
        "Starting source mining batch"
    );

    for item in &to_process {
        // Transmute with a modest pipeline: interrogate + articulate is enough
        // for wisdom-graph population. Full 7-stage would be overkill for
        // background mining and ~3x more expensive.
        let transmute_result = crate::context_commands::run_awe_with_timeout(
            std::process::Command::new(&awe_path).args([
                "transmute",
                &item.title,
                "--stages",
                "interrogate,articulate",
                "-d",
                "software-engineering",
                "--json",
            ]),
            20,
        );

        match transmute_result {
            Ok(_) => {
                stats.decisions_created += 1;

                // Look up the real decision ID and emit DecisionAdded so the
                // UI can tick up its counter live.
                if let Ok(history_out) = crate::context_commands::run_awe_with_timeout(
                    std::process::Command::new(&awe_path).args([
                        "history",
                        "-d",
                        "software-engineering",
                        "--limit",
                        "3",
                        "--json",
                    ]),
                    5,
                ) {
                    let stdout = String::from_utf8_lossy(&history_out.stdout);
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                        if let Some(id) = json
                            .get("decisions")
                            .and_then(|v| v.as_array())
                            .and_then(|arr| {
                                arr.iter().find(|d| {
                                    d.get("statement").and_then(|s| s.as_str())
                                        == Some(item.title.as_str())
                                })
                            })
                            .and_then(|d| d.get("id").and_then(|v| v.as_str()))
                        {
                            emit_awe_event(
                                app,
                                AweEvent::DecisionAdded {
                                    id: id.to_string(),
                                    statement: item.title.clone(),
                                    domain: "software-engineering".into(),
                                    source: format!("source_mining:{}", item.source_type),
                                },
                            );
                        }
                    }
                }
            }
            Err(e) => {
                stats.errors += 1;
                warn!(
                    target: "4da::awe_source_mining",
                    title = %item.title,
                    error = %e,
                    "transmute failed in batch"
                );
            }
        }
    }

    emit_awe_event(
        app,
        AweEvent::SourceMiningComplete {
            candidates_considered: stats.candidates_considered,
            decisions_created: stats.decisions_created,
            rate_limited: stats.rate_limited,
        },
    );
    emit_awe_event(app, AweEvent::SummaryStale);

    info!(
        target: "4da::awe_source_mining",
        decisions_created = stats.decisions_created,
        errors = stats.errors,
        "Source mining batch complete"
    );

    Ok(stats)
}

/// Convenience wrapper with the default budget. This is what the daily
/// scheduler calls — the lower-arity signature keeps the monitoring code
/// clean and gives us a single place to tune the rate limit.
pub async fn run_daily_source_mining<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<SourceMiningStats, String> {
    run_source_mining_batch(app, DEFAULT_MAX_TRANSMUTES_PER_RUN).await
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Decision-shape heuristic: positive cases ---------------------------

    #[test]
    fn detects_should_we_question() {
        assert!(looks_like_decision(
            "Should we adopt Tauri 2.0 for our desktop apps?"
        ));
    }

    #[test]
    fn detects_vs_comparison() {
        assert!(looks_like_decision(
            "Postgres vs MySQL for a multi-tenant SaaS"
        ));
    }

    #[test]
    fn detects_migration_marker() {
        assert!(looks_like_decision(
            "We migrated from MongoDB to Postgres — here is what we learned"
        ));
    }

    #[test]
    fn detects_trade_off_language() {
        assert!(looks_like_decision(
            "The real trade-off of serverless at scale"
        ));
    }

    #[test]
    fn detects_considered_harmful() {
        assert!(looks_like_decision(
            "Dependency injection frameworks considered harmful"
        ));
    }

    #[test]
    fn detects_when_to_use() {
        assert!(looks_like_decision(
            "When to use gRPC instead of REST for internal services"
        ));
    }

    #[test]
    fn detects_why_we_switched() {
        assert!(looks_like_decision(
            "Why we switched from Kubernetes to systemd units"
        ));
    }

    #[test]
    fn detects_worth_it_pattern() {
        assert!(looks_like_decision(
            "Is a monorepo actually worth it for a 4-person team?"
        ));
    }

    // ---- Decision-shape heuristic: negative cases ---------------------------

    #[test]
    fn rejects_release_announcement() {
        assert!(!looks_like_decision(
            "Rust 1.80 released with const generics improvements"
        ));
    }

    #[test]
    fn rejects_tutorial() {
        assert!(!looks_like_decision(
            "A beginners guide to Docker networking"
        ));
    }

    #[test]
    fn rejects_too_short() {
        assert!(!looks_like_decision("gRPC is fast"));
    }

    #[test]
    fn rejects_bug_report() {
        assert!(!looks_like_decision(
            "Memory leak in the HTTP/2 connection pool"
        ));
    }

    #[test]
    fn rejects_news_headline() {
        assert!(!looks_like_decision(
            "Major CVE disclosed in xz-utils affecting millions of servers"
        ));
    }

    #[test]
    fn rejects_empty_title() {
        assert!(!looks_like_decision(""));
    }

    #[test]
    fn rejects_whitespace_only() {
        assert!(!looks_like_decision("                              "));
    }

    // ---- Stats defaults -----------------------------------------------------

    #[test]
    fn default_stats_all_zero() {
        let stats = SourceMiningStats::default();
        assert_eq!(stats.candidates_considered, 0);
        assert_eq!(stats.decision_shaped, 0);
        assert_eq!(stats.decisions_created, 0);
        assert_eq!(stats.rate_limited, 0);
        assert_eq!(stats.errors, 0);
    }

    // ---- Case-insensitive matching ------------------------------------------

    #[test]
    fn case_insensitive_starters() {
        assert!(looks_like_decision(
            "SHOULD WE MIGRATE from Python to Rust for the ingest service?"
        ));
    }

    #[test]
    fn vs_separator_with_period() {
        assert!(looks_like_decision("Rust vs. Go for systems programming"));
    }
}
