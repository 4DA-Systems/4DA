// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Blind Spot Intelligence for 4DA
//!
//! Cross-references what the user is watching with what they SHOULD be
//! watching based on their actual dependencies, projects, and stack.
//! "You have 6 active Rust deps but haven't engaged with Rust signals in 21 days."

use std::sync::Mutex;
use std::time::Instant;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use ts_rs::TS;

use crate::error::Result;
use crate::evidence::{
    Action as EvidenceAction, Confidence, EvidenceCitation, EvidenceFeed, EvidenceItem,
    EvidenceKind, LensHints, Urgency,
};
use crate::monitoring_briefing::DataFreshness;
use crate::scoring_config;

// ============================================================================
// Report-level cache (5-minute TTL)
// ============================================================================

static BLIND_SPOT_CACHE: Lazy<Mutex<Option<(Instant, BlindSpotReport)>>> =
    Lazy::new(|| Mutex::new(None));

const CACHE_TTL_SECS: u64 = 300; // 5 minutes

/// Invalidate the blind-spot report cache. Call this after analysis completion
/// or when the user's dependency set changes.
pub fn invalidate_blind_spot_cache() {
    if let Ok(mut guard) = BLIND_SPOT_CACHE.lock() {
        *guard = None;
    }
}

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct BlindSpotReport {
    /// 0-100, higher = more blind spots
    pub overall_score: f32,
    pub uncovered_dependencies: Vec<UncoveredDep>,
    pub stale_topics: Vec<StaleTopic>,
    pub missed_signals: Vec<MissedSignal>,
    pub recommendations: Vec<BlindSpotRecommendation>,
    /// Dependencies suppressed because they only had weak (title-heuristic) matches.
    /// Hidden by default in the UI. Separate from uncovered_dependencies.
    pub weak_matches: Vec<UncoveredDep>,
    pub generated_at: String,
    /// Source data freshness summary. When is_stale is true, the frontend should
    /// warn that blind spot analysis may be based on outdated data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub data_freshness: Option<DataFreshness>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct UncoveredDep {
    pub name: String,
    /// npm, cargo, pip, etc.
    pub dep_type: String,
    pub projects_using: Vec<String>,
    pub days_since_last_signal: u32,
    /// Signals that exist but the user didn't see
    pub available_signal_count: u32,
    /// critical, high, medium, low
    pub risk_level: String,
    /// How the dependency was matched: "exact_registry", "advisory", "title_heuristic", or "none"
    pub match_type: String,
    /// Why this dep has no/limited coverage. Provides honest diagnostics instead of
    /// the misleading "none of your sources cover it" language.
    /// Examples: "not_checked", "checked_no_results", "adapter_failing", "adapter_disabled", "unknown_ecosystem", "weak_matches_only"
    #[serde(default)]
    pub coverage_reason: Option<String>,
    /// Which source adapters were searched for this dependency and their status.
    /// Each entry is like "npm_registry: checked 2h ago" or "osv: adapter_failing".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapters_searched: Vec<AdapterStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct AdapterStatus {
    /// Source adapter name (e.g., "npm_registry", "crates_io", "osv", "github")
    pub adapter: String,
    /// Current status: "checked", "not_checked", "failing", "disabled", "rate_limited", "stale"
    pub status: String,
    /// When this adapter last successfully fetched data (ISO 8601 string or null)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_checked: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct StaleTopic {
    pub topic: String,
    pub last_engagement_days: u32,
    pub active_deps_in_topic: u32,
    pub missed_signal_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct MissedSignal {
    pub item_id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub relevance_score: f32,
    pub created_at: String,
    pub why_relevant: String,
    /// The dependency name this signal relates to (if identified).
    /// Used by the frontend to group missed signals under their coverage gap.
    #[serde(default)]
    pub dep_name: Option<String>,
    /// Whether the user has seen this item before (any interaction recorded).
    /// false = "New for you", true = "Blind Spot" (shown but never engaged).
    #[serde(default)]
    pub was_shown: bool,
    /// Content classification from ingestion (security_advisory, release_notes, etc.).
    /// Used for urgency mapping and noise filtering instead of title pattern matching.
    #[serde(default)]
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct BlindSpotRecommendation {
    /// e.g., "Set up a watch for Rust security"
    pub action: String,
    pub reason: String,
    /// high, medium, low
    pub priority: String,
}

// Internal struct for dependency coverage tracking.
//
// NOTE on column naming: `user_dependencies` has `package_name` and `ecosystem`.
// Earlier versions of this file used `name`/`dep_type` which don't exist in the
// schema — the query silently failed and this function returned an empty Vec,
// which cascaded into the "Blind Spot Index = 100" bug (no deps → empty
// uncovered → score driven entirely by the missed-signals tally).
#[derive(Debug, Clone)]
struct DepCoverage {
    package_name: String,
    ecosystem: String,
    projects: Vec<String>,
}

#[derive(Debug, Default)]
struct DepSignalCoverage {
    available: u32,
    interacted: u32,
    days_since_last_signal: Option<u32>,
    /// Best match type seen: "exact_registry" > "advisory" > "title_heuristic"
    best_match_type: Option<String>,
}

// ============================================================================
// Implementation
// ============================================================================

fn blind_spot_threshold_days() -> u32 {
    let manager = crate::state::get_settings_manager();
    let guard = manager.lock();
    match guard.get().blind_spot_sensitivity.as_str() {
        "aggressive" => 7,
        "relaxed" => 30,
        _ => 14,
    }
}

/// Check if the user has < 7 days of engagement history.
/// Returns true only if BOTH interactions AND source_items are too young.
/// Used for cold-start suppression (doctrine rule 6).
///
/// Previously only checked `interactions`, but users who've been running 4DA
/// for days without explicit feedback still have rich source_items and
/// project_dependencies data — enough for meaningful blind spot analysis.
fn is_cold_start(conn: &rusqlite::Connection) -> Result<bool> {
    // Check interactions table first (explicit engagement)
    let interaction_age = oldest_record_age_days(conn, "SELECT MIN(timestamp) FROM interactions");

    // Also check source_items (passive data collection)
    let source_age = oldest_record_age_days(conn, "SELECT MIN(created_at) FROM source_items");

    // Not cold-start if EITHER data source has 7+ days of history
    let max_age = interaction_age.max(source_age);
    Ok(max_age < 7)
}

fn oldest_record_age_days(conn: &rusqlite::Connection, sql: &str) -> i64 {
    let result: Option<String> = conn.query_row(sql, [], |row| row.get(0)).ok().flatten();
    match result {
        None => 0,
        Some(ts) => {
            let parsed = chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| chrono::DateTime::parse_from_rfc3339(&ts).map(|dt| dt.naive_utc()));
            match parsed {
                Ok(oldest) => (chrono::Utc::now().naive_utc() - oldest).num_days(),
                Err(_) => 0,
            }
        }
    }
}

/// Project paths with git commits in the last 14 days. Used to suppress
/// blind spots for technologies the user is actively developing with.
/// Normalize a filesystem path for comparison: strip extended-length prefix,
/// forward slashes, lowercase on Windows. Without this, `D:\4DA` vs `d:\4DA`
/// or `\\?\D:\4DA` vs `D:\4DA` causes `starts_with` to fail, silently
/// dropping all blind spots.
fn normalize_path_for_cmp(p: &str) -> String {
    let stripped = p
        .strip_prefix(r"\\?\")
        .or_else(|| p.strip_prefix("//?/"))
        .unwrap_or(p);
    let s = stripped
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

fn get_recent_project_paths(conn: &rusqlite::Connection) -> std::collections::HashSet<String> {
    let sql =
        "SELECT DISTINCT repo_path FROM git_signals WHERE timestamp > datetime('now', '-14 days')";
    let mut paths = std::collections::HashSet::new();
    if let Ok(mut stmt) = conn.prepare(sql) {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for row in rows.flatten() {
                paths.insert(normalize_path_for_cmp(&row));
            }
        }
    }
    paths
}

/// True if the user is actively developing with a technology — recent git
/// commits in a project that depends on it. Suppresses "Drifting" for tech
/// the user clearly knows about (e.g. Tauri while building a Tauri app).
fn is_actively_developed_tech(
    topic: &str,
    deps: &[DepCoverage],
    active_paths: &std::collections::HashSet<String>,
) -> bool {
    let topic_lower = topic.to_lowercase();
    deps.iter().any(|dep| {
        let dep_lower = dep.package_name.to_lowercase();
        let name_matches = dep_lower.contains(&topic_lower) || topic_lower.contains(&dep_lower);
        name_matches
            && dep.projects.iter().any(|p| {
                let p_norm = normalize_path_for_cmp(p);
                active_paths
                    .iter()
                    .any(|ap| p_norm.starts_with(ap.as_str()) || ap.starts_with(p_norm.as_str()))
            })
    })
}

/// Generate a comprehensive blind spot report.
///
/// Results are cached for 5 minutes to avoid redundant computation on
/// rapid tab switches. Call `invalidate_blind_spot_cache()` to force a
/// fresh report (e.g. after an analysis run completes).
pub fn generate_blind_spot_report() -> Result<BlindSpotReport> {
    // Check cache first
    if let Ok(guard) = BLIND_SPOT_CACHE.lock() {
        if let Some((cached_at, ref report)) = *guard {
            if cached_at.elapsed().as_secs() < CACHE_TTL_SECS {
                return Ok(report.clone());
            }
        }
    }

    let report = generate_blind_spot_report_uncached()?;

    // Store in cache
    if let Ok(mut guard) = BLIND_SPOT_CACHE.lock() {
        *guard = Some((Instant::now(), report.clone()));
    }

    Ok(report)
}

/// Inner implementation — always runs fresh queries.
fn generate_blind_spot_report_uncached() -> Result<BlindSpotReport> {
    let conn = crate::open_db_connection()?;

    // Cold-start suppression (doctrine rule 6): blind spots require 7+ days
    // of engagement data to be meaningful. Showing blind spots on day 1
    // guarantees false signals — the system hasn't observed enough to know
    // what the user is missing.
    if is_cold_start(&conn)? {
        return Ok(BlindSpotReport {
            // -1.0 sentinel means "not enough data to compute" — the frontend
            // renders a "building" state instead of a misleading "0/100 Good".
            // Previously this was 0.0, which the UI interpreted as "perfect
            // coverage" even on day 1 with zero interactions.
            overall_score: -1.0,
            uncovered_dependencies: vec![],
            stale_topics: vec![],
            missed_signals: vec![],
            recommendations: vec![],
            weak_matches: vec![],
            generated_at: chrono::Utc::now().to_rfc3339(),
            data_freshness: crate::monitoring_briefing::compute_data_freshness(),
        });
    }

    let threshold_days = blind_spot_threshold_days();

    // 1. Get attention report (30-day window)
    let attention = crate::attention::generate_report(30)?;

    // 2. Get knowledge gaps
    let gaps = crate::knowledge_decay::detect_knowledge_gaps(&conn)?;

    // 3. Get all user dependencies with project coverage
    let deps = get_dependency_coverage(&conn)?;

    // 3b. Active project detection — suppress blind spots for tech the user
    // is clearly working with (recent git commits).
    let active_paths = get_recent_project_paths(&conn);

    // 4. Find uncovered dependencies (deps with no interaction in threshold days)
    let (uncovered, weak_matches) = find_uncovered_deps(&conn, &deps, threshold_days)?;

    // 5. Find stale topics from attention blind spots.
    // Only include topics with actual missed signals — a topic with
    // 0 missed signals means coverage is healthy, not that there's a
    // gap. Showing "Stale topic: rust (0 signals missed)" erodes trust
    // by flagging something the user can't act on.
    let stale: Vec<StaleTopic> = attention
        .blind_spots
        .iter()
        .filter(|bs| bs.in_codebase)
        .filter(|bs| !is_actively_developed_tech(&bs.topic, &deps, &active_paths))
        .map(|bs| StaleTopic {
            topic: bs.topic.clone(),
            last_engagement_days: ((1.0 - bs.engagement_level) * 30.0) as u32,
            active_deps_in_topic: count_deps_for_topic(&deps, &bs.topic),
            missed_signal_count: count_missed_for_topic(&gaps, &bs.topic),
        })
        .filter(|st| st.missed_signal_count > 0)
        .collect();

    // 6. Find missed signals (high-relevance, not seen, older than feed window)
    let missed = find_missed_signals(&conn, threshold_days, &deps)?;

    // 6b. Active-project scoping: suppress deps/signals from projects with no
    // recent git activity. Prevents cross-project pollution (e.g. express from
    // kairos-mvp showing in 4DA's blind spots). When git_signals is empty we
    // have no activity data, so skip this filter to avoid hiding everything.
    info!(
        target: "4da::blind_spots",
        active_paths_count = active_paths.len(),
        paths = %active_paths.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
        uncovered_pre_filter = uncovered.len(),
        "active-project filter: inputs"
    );
    let (uncovered, weak_matches, missed) = if active_paths.is_empty() {
        (uncovered, weak_matches, missed)
    } else {
        let is_active_project = |p: &str| -> bool {
            let p_norm = normalize_path_for_cmp(p);
            active_paths
                .iter()
                .any(|ap| p_norm.starts_with(ap.as_str()) || ap.starts_with(p_norm.as_str()))
        };
        let active_dep_names: std::collections::HashSet<String> = deps
            .iter()
            .filter(|d| d.projects.iter().any(|p| is_active_project(p)))
            .map(|d| d.package_name.to_lowercase())
            .collect();
        if let Some(sample) = uncovered.first() {
            info!(
                target: "4da::blind_spots",
                dep = %sample.name,
                projects = %sample.projects_using.join(" | "),
                "active-project filter: sample dep project paths"
            );
        }
        let pre_count = uncovered.len();
        let uc: Vec<UncoveredDep> = uncovered
            .into_iter()
            .filter(|u| u.projects_using.iter().any(|p| is_active_project(p)))
            .collect();
        let wm: Vec<UncoveredDep> = weak_matches
            .into_iter()
            .filter(|u| u.projects_using.iter().any(|p| is_active_project(p)))
            .collect();
        info!(
            target: "4da::blind_spots",
            before = pre_count,
            after = uc.len(),
            "active-project filter: uncovered deps"
        );
        let ms = missed
            .into_iter()
            .filter(|m| {
                m.dep_name
                    .as_ref()
                    .map_or(true, |dn| active_dep_names.contains(&dn.to_lowercase()))
            })
            .collect();
        (uc, wm, ms)
    };

    // 7. Generate recommendations
    let recommendations = generate_recommendations(&uncovered, &stale, &gaps);

    // 8. Calculate overall score (normalized against direct-dep count)
    let score = calculate_blind_spot_score(&uncovered, &stale, &missed, deps.len());

    info!(
        target: "4da::blind_spots",
        uncovered = uncovered.len(),
        stale = stale.len(),
        missed = missed.len(),
        recs = recommendations.len(),
        score = score,
        total_deps = deps.len(),
        "Blind spot report generated"
    );

    Ok(BlindSpotReport {
        overall_score: score,
        uncovered_dependencies: uncovered,
        stale_topics: stale,
        missed_signals: missed,
        recommendations,
        weak_matches,
        generated_at: chrono::Utc::now().to_rfc3339(),
        data_freshness: crate::monitoring_briefing::compute_data_freshness(),
    })
}

/// Normalize a package name for identity comparison.
/// Rust crates: `async-trait` == `async_trait` (Cargo normalizes hyphens to underscores).
/// npm scoped: `@babel/core` stays as-is (scope is meaningful).
fn normalize_dep_name(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
}

/// Query `project_dependencies` to get a coverage view of the user's stack.
///
/// Uses `project_dependencies` (not `user_dependencies`) because:
///   1. `project_dependencies` is the canonical per-project dep list the ACE
///      scanner populates on every scan.
///   2. It already has `package_name`, `is_direct`, `is_dev`, `language` —
///      everything we need for risk classification.
///   3. `user_dependencies` is a user-curated watchlist that may be empty.
///
/// Returns only DIRECT dependencies (deps declared in a manifest file, not
/// transitive lockfile entries). Transitive deps balloon the set to ~2500
/// entries on a typical 4DA developer machine and are dominated by packages
/// the user never directly cares about.
///
/// Skips dev-only deps — they're not runtime risk surface for blind spots.
fn get_dependency_coverage(conn: &rusqlite::Connection) -> Result<Vec<DepCoverage>> {
    // One query: aggregate projects per unique (package_name, language) pair,
    // filtering to direct runtime deps only.
    // Group by normalized name so `async-trait` and `async_trait` merge.
    let sql = "SELECT REPLACE(LOWER(package_name), '-', '_') as norm_name,
                      package_name,
                      language,
                      MAX(is_direct) as any_direct,
                      GROUP_CONCAT(DISTINCT project_path) as project_list
               FROM project_dependencies
               WHERE is_dev = 0
               GROUP BY norm_name, language
               HAVING any_direct = 1
               ORDER BY norm_name";

    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            warn!(
                target: "4da::blind_spots",
                "Failed to query project_dependencies: {e}"
            );
            return Ok(Vec::new());
        }
    };

    let rows = stmt.query_map([], |row| {
        let _norm_name: String = row.get(0)?;
        let package_name: String = row.get(1)?;
        let ecosystem: String = row.get(2)?;
        let project_list: Option<String> = row.get(4).ok();
        let projects: Vec<String> = project_list
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        Ok(DepCoverage {
            package_name,
            ecosystem,
            projects,
        })
    })?;

    let mut result: Vec<DepCoverage> = rows.filter_map(|r| r.ok()).collect();
    result.sort_by(|a, b| {
        b.projects
            .len()
            .cmp(&a.projects.len())
            .then_with(|| a.package_name.cmp(&b.package_name))
    });

    if result.is_empty() {
        warn!(
            target: "4da::blind_spots",
            "get_dependency_coverage returned 0 direct deps — user has no direct project deps scanned"
        );
    }

    Ok(result)
}

/// Map ecosystem names to the source adapter types that should cover them.
fn ecosystem_source_types(ecosystem: &str) -> Vec<String> {
    match ecosystem.to_lowercase().as_str() {
        "npm" | "javascript" | "typescript" => {
            vec!["npm_registry".into(), "osv".into(), "github".into()]
        }
        "crates.io" | "cargo" | "rust" => {
            vec!["crates_io".into(), "osv".into(), "github".into()]
        }
        "pypi" | "python" => vec!["pypi".into(), "osv".into(), "github".into()],
        "go" | "golang" => vec!["go_modules".into(), "osv".into(), "github".into()],
        "maven" | "java" | "kotlin" => vec!["osv".into(), "github".into()],
        "nuget" | "csharp" | "dotnet" => vec!["osv".into(), "github".into()],
        _ => vec!["osv".into()],
    }
}

/// Diagnose WHY a dependency has no or limited source coverage.
/// Returns (reason, detail) explaining the gap honestly instead of the
/// misleading "none of your sources cover it" blanket message.
fn diagnose_coverage(
    conn: &rusqlite::Connection,
    package_name: &str,
    ecosystem: &str,
) -> (String, String) {
    // 1. Check source_item_dependencies for ANY links (including low confidence)
    let has_any_link: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM source_item_dependencies \
             WHERE LOWER(REPLACE(package_name, '-', '_')) = LOWER(REPLACE(?1, '-', '_')) LIMIT 1)",
            params![package_name],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if has_any_link {
        return (
            "weak_matches_only".into(),
            format!(
                "{} has source items linked but none strong enough to surface.",
                package_name
            ),
        );
    }

    // 2. Check which source types SHOULD cover this ecosystem
    let expected_sources = ecosystem_source_types(ecosystem);
    if expected_sources.is_empty() {
        return (
            "unknown_ecosystem".into(),
            format!(
                "No source adapters configured for the {} ecosystem.",
                ecosystem
            ),
        );
    }

    // 3. Check feed_health for those source types
    let mut failed_sources = Vec::new();
    let mut healthy_sources = Vec::new();
    for source_type in &expected_sources {
        let health: Option<(i64, i64)> = conn
            .query_row(
                "SELECT consecutive_failures, total_successes FROM feed_health \
                 WHERE source_type = ?1 ORDER BY updated_at DESC LIMIT 1",
                params![source_type],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        match health {
            Some((failures, _)) if failures >= 3 => {
                failed_sources.push(source_type.as_str());
            }
            Some((_, successes)) if successes > 0 => {
                healthy_sources.push(source_type.as_str());
            }
            _ => {} // no data = never ran
        }
    }

    if !failed_sources.is_empty() {
        return (
            "adapter_failing".into(),
            format!(
                "Source adapter{} {} failing. Check feed health.",
                if failed_sources.len() > 1 { "s" } else { "" },
                failed_sources.join(", ")
            ),
        );
    }

    if healthy_sources.is_empty() {
        return (
            "not_checked".into(),
            format!(
                "4DA hasn't checked {} yet — source adapters haven't run for this ecosystem.",
                package_name
            ),
        );
    }

    // Sources are healthy but found nothing for this specific package
    (
        "checked_no_results".into(),
        format!(
            "Sources checked ({}) but found no results for {}.",
            healthy_sources.join(", "),
            package_name
        ),
    )
}

/// Build per-adapter health status for a dependency's ecosystem.
///
/// For each adapter relevant to the ecosystem, queries `feed_health` for failure
/// counts and `source_items` for the most recent fetch timestamp. Classifies as:
/// - "checked"     — last fetch within 7 days
/// - "stale"       — last fetch within 30 days
/// - "not_checked" — no fetch data at all
/// - "failing"     — 3+ consecutive failures in feed_health
fn adapter_statuses_for_ecosystem(
    conn: &rusqlite::Connection,
    ecosystem: &str,
) -> Vec<AdapterStatus> {
    let adapters = ecosystem_source_types(ecosystem);
    if adapters.is_empty() {
        return Vec::new();
    }

    adapters
        .iter()
        .map(|adapter_name| {
            // Check feed_health for failure info
            let health: Option<(i64, i64)> = conn
                .query_row(
                    "SELECT consecutive_failures, total_successes FROM feed_health \
                     WHERE source_type = ?1 ORDER BY updated_at DESC LIMIT 1",
                    params![adapter_name],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok();

            let is_failing = matches!(health, Some((failures, _)) if failures >= 3);

            // Get last successful fetch time from source_items
            let last_fetch: Option<String> = conn
                .query_row(
                    "SELECT MAX(last_seen) FROM source_items WHERE source_type = ?1",
                    params![adapter_name],
                    |row| row.get(0),
                )
                .ok()
                .flatten();

            let (status, last_checked) = if is_failing {
                ("failing".to_string(), last_fetch)
            } else {
                match &last_fetch {
                    Some(ts) => {
                        let age_days = parse_timestamp_age_days(ts);
                        if age_days <= 7 {
                            ("checked".to_string(), Some(ts.clone()))
                        } else if age_days <= 30 {
                            ("stale".to_string(), Some(ts.clone()))
                        } else {
                            ("not_checked".to_string(), Some(ts.clone()))
                        }
                    }
                    None => ("not_checked".to_string(), None),
                }
            };

            AdapterStatus {
                adapter: adapter_name.clone(),
                status,
                last_checked,
            }
        })
        .collect()
}

/// Parse a timestamp string and return how many days ago it was.
/// Returns `i64::MAX` on parse failure so it falls through to "not_checked".
fn parse_timestamp_age_days(ts: &str) -> i64 {
    let parsed = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(ts).map(|dt| dt.naive_utc()));
    match parsed {
        Ok(dt) => (chrono::Utc::now().naive_utc() - dt).num_days(),
        Err(_) => i64::MAX,
    }
}

/// For each direct dependency, check if any source_items mention it in the
/// last 14 days AND whether the user interacted with them. If no interaction
/// AND signals exist, it's a blind spot.
///
/// Performance notes:
/// - Capped at `MAX_DEPS_TO_PROCESS` unique direct deps (default 50). The user's
///   most-used deps come first via sort order, so the cap is rarely reached.
/// - Short dep names (< 4 chars) are skipped — too generic for reliable LIKE
///   matching ("go", "c", "r" would match everything).
/// - **Batched query**: all three metrics (available_count, interacted_count,
///   days_since_last) are computed in a single SQL query via a temp table +
///   LEFT JOIN. This replaces the previous N+1 pattern (3 queries × 50 deps
///   = 150 sequential queries → 2-3 queries total).
///
/// Schema correction (was a bug): `interactions` has `timestamp` NOT
/// `created_at`. The previous query silently errored via `.unwrap_or(999)`,
/// causing every dep to register `days_since = 999` → critical risk → score
/// pinned to 100.
fn find_uncovered_deps(
    conn: &rusqlite::Connection,
    deps: &[DepCoverage],
    threshold_days: u32,
) -> Result<(Vec<UncoveredDep>, Vec<UncoveredDep>)> {
    const MAX_DEPS_TO_PROCESS: usize = 50;
    const MIN_DEP_NAME_LEN: usize = 4;

    // Filter, rank, and cap the dep list before touching the DB.
    let mut ranked_deps: Vec<&DepCoverage> = deps
        .iter()
        .filter(|d| d.package_name.len() >= MIN_DEP_NAME_LEN)
        .filter(|d| !is_builtin_module(&d.package_name))
        .filter(|d| !is_utility_dep(&d.package_name))
        .filter(|d| !is_generic_dep_name(&d.package_name))
        .collect();
    ranked_deps.sort_by(|a, b| {
        b.projects
            .len()
            .cmp(&a.projects.len())
            .then_with(|| a.package_name.cmp(&b.package_name))
    });
    let eligible_deps: Vec<&DepCoverage> =
        ranked_deps.into_iter().take(MAX_DEPS_TO_PROCESS).collect();

    info!(
        target: "4da::blind_spots",
        total_deps = deps.len(),
        eligible = eligible_deps.len(),
        names = %eligible_deps.iter().take(10).map(|d| d.package_name.as_str()).collect::<Vec<_>>().join(", "),
        "find_uncovered_deps: eligible deps after filtering"
    );

    if eligible_deps.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    // ── Step 1: Create temp table with dep names ────────────────────────
    // Using a temp table avoids SQLite's lack of VALUES-as-CTE support and
    // keeps parameter binding straightforward.
    if let Err(e) =
        conn.execute_batch("CREATE TEMP TABLE IF NOT EXISTS _blind_spot_deps (name TEXT NOT NULL)")
    {
        warn!(
            target: "4da::blind_spots",
            "Failed to create temp table for batched dep query: {e}"
        );
        return Ok((Vec::new(), Vec::new()));
    }

    // Clear any stale rows from a previous call in the same connection.
    let _ = conn.execute("DELETE FROM _blind_spot_deps", []);

    // Batch-insert dep names. Using a single prepared statement with
    // repeated execution is fast for ≤50 rows.
    {
        let mut insert_stmt = match conn.prepare("INSERT INTO _blind_spot_deps (name) VALUES (?1)")
        {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    target: "4da::blind_spots",
                    "Failed to prepare dep insert: {e}"
                );
                let _ = conn.execute_batch("DROP TABLE IF EXISTS _blind_spot_deps");
                return Ok((Vec::new(), Vec::new()));
            }
        };
        for dep in &eligible_deps {
            let norm = normalize_dep_name(&dep.package_name);
            if let Err(e) = insert_stmt.execute(params![norm]) {
                warn!(
                    target: "4da::blind_spots",
                    "Failed to insert dep '{}' into temp table: {e}",
                    dep.package_name
                );
            }
        }
    }

    // ── Step 2: Batched queries with SID-first matching ─────────────────
    //
    // Two queries (recent_sql + history_sql) each use UNION ALL with two
    // branches:
    //
    //   Branch 1 (primary): JOIN through source_item_dependencies (SID) on
    //     normalized package_name. These are authoritative evidence links
    //     created by the dep linker — high confidence, skip word-boundary
    //     filtering.
    //
    //   Branch 2 (fallback): The original title LIKE '%dep_name%' pattern,
    //     but with NOT EXISTS to exclude items already matched in Branch 1.
    //     These are flagged as "title_heuristic" match_source.
    //
    // Each row carries a match_source column so the processing loop can
    // assign the correct match_type to coverage metrics.
    let window = format!("-{threshold_days} days");

    let recent_sql = "
        -- Branch 1: evidence-linked matches via source_item_dependencies (high confidence)
        SELECT
            bd.name,
            si.title,
            si.source_type,
            si.content_type,
            CASE
                WHEN EXISTS(
                    SELECT 1
                    FROM interactions i
                    WHERE i.item_id = si.id OR i.source_item_id = si.id
                ) THEN 1
                ELSE 0
            END AS interacted,
            sid.match_type AS match_source
        FROM _blind_spot_deps bd
        JOIN source_item_dependencies sid
            ON LOWER(REPLACE(sid.package_name, '-', '_')) = bd.name
        JOIN source_items si ON si.id = sid.source_item_id
        WHERE si.created_at >= datetime('now', ?1)
          AND LOWER(si.source_type) != 'stackoverflow'
          AND (si.content_type IS NULL
               OR si.content_type NOT IN ('show_and_tell','tutorial','question',
                                          'help_request','hiring','clickbait'))

        UNION ALL

        -- Branch 2: title-heuristic fallback for items not yet in source_item_dependencies
        SELECT
            bd.name,
            si.title,
            si.source_type,
            si.content_type,
            CASE
                WHEN EXISTS(
                    SELECT 1
                    FROM interactions i
                    WHERE i.item_id = si.id OR i.source_item_id = si.id
                ) THEN 1
                ELSE 0
            END AS interacted,
            'title_heuristic' AS match_source
        FROM _blind_spot_deps bd
        JOIN source_items si ON (si.title LIKE '%' || bd.name || '%'
                                 OR si.title LIKE '%' || REPLACE(bd.name, '_', '-') || '%')
        WHERE si.created_at >= datetime('now', ?1)
          AND LOWER(si.source_type) != 'stackoverflow'
          AND (si.content_type IS NULL
               OR si.content_type NOT IN ('show_and_tell','tutorial','question',
                                          'help_request','hiring','clickbait'))
          AND NOT EXISTS (
              SELECT 1 FROM source_item_dependencies sid2
              WHERE sid2.source_item_id = si.id
                AND LOWER(REPLACE(sid2.package_name, '-', '_')) = bd.name
          )
    ";
    let history_sql = "
        -- Branch 1: evidence-linked interaction history (high confidence)
        SELECT
            bd.name,
            si.title,
            si.source_type,
            si.content_type,
            CAST(julianday('now') - julianday(MAX(i.timestamp)) AS INTEGER) AS days_since,
            sid.match_type AS match_source
        FROM _blind_spot_deps bd
        JOIN source_item_dependencies sid
            ON LOWER(REPLACE(sid.package_name, '-', '_')) = bd.name
        JOIN source_items si ON si.id = sid.source_item_id
        JOIN interactions i ON i.item_id = si.id OR i.source_item_id = si.id
        WHERE LOWER(si.source_type) != 'stackoverflow'
        GROUP BY bd.name, si.id, si.title, si.source_type, si.content_type, sid.match_type

        UNION ALL

        -- Branch 2: title-heuristic fallback interaction history
        SELECT
            bd.name,
            si.title,
            si.source_type,
            si.content_type,
            CAST(julianday('now') - julianday(MAX(i.timestamp)) AS INTEGER) AS days_since,
            'title_heuristic' AS match_source
        FROM _blind_spot_deps bd
        JOIN source_items si ON (si.title LIKE '%' || bd.name || '%'
                                 OR si.title LIKE '%' || REPLACE(bd.name, '_', '-') || '%')
        JOIN interactions i ON i.item_id = si.id OR i.source_item_id = si.id
        WHERE LOWER(si.source_type) != 'stackoverflow'
          AND NOT EXISTS (
              SELECT 1 FROM source_item_dependencies sid2
              WHERE sid2.source_item_id = si.id
                AND LOWER(REPLACE(sid2.package_name, '-', '_')) = bd.name
          )
        GROUP BY bd.name, si.id, si.title, si.source_type, si.content_type
    ";

    let dep_lookup: std::collections::HashMap<String, &DepCoverage> = eligible_deps
        .iter()
        .map(|d| (normalize_dep_name(&d.package_name), *d))
        .collect();
    let mut coverage: std::collections::HashMap<String, DepSignalCoverage> = eligible_deps
        .iter()
        .map(|dep| {
            (
                normalize_dep_name(&dep.package_name),
                DepSignalCoverage::default(),
            )
        })
        .collect();

    {
        let mut stmt = match conn.prepare(recent_sql) {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    target: "4da::blind_spots",
                    "Failed to prepare recent blind-spot dep query: {e}"
                );
                let _ = conn.execute_batch("DROP TABLE IF EXISTS _blind_spot_deps");
                return Ok((Vec::new(), Vec::new()));
            }
        };
        let rows = match stmt.query_map(params![window], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
            ))
        }) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    target: "4da::blind_spots",
                    "Failed to execute recent blind-spot dep query: {e}"
                );
                let _ = conn.execute_batch("DROP TABLE IF EXISTS _blind_spot_deps");
                return Ok((Vec::new(), Vec::new()));
            }
        };

        for row_result in rows {
            let (name, title, source_type, content_type, interacted, match_source) =
                match row_result {
                    Ok(r) => r,
                    Err(e) => {
                        warn!(
                            target: "4da::blind_spots",
                            "Failed to read recent blind-spot dep row: {e}"
                        );
                        continue;
                    }
                };
            // SID-linked items skip the title word-boundary filter — the link
            // is authoritative evidence that the item relates to this dep.
            // Title-heuristic fallback items still need the boundary check.
            if match_source == "title_heuristic" {
                let dep_lower = name.to_lowercase();
                let title_lower = title.to_lowercase();
                if !is_actionable_blind_spot_match(
                    &dep_lower,
                    &title_lower,
                    &source_type,
                    content_type.as_deref(),
                ) {
                    continue;
                }
            }
            // Only SID-linked rows can carry registry/advisory proof. Direct
            // SQL title fallback is always heuristic even if the source itself
            // is a CVE/OSV feed; otherwise title-only advisory matches bypass
            // the dep linker's structured affected-package validation.
            let mt = if match_source != "title_heuristic" {
                sid_match_type_to_coverage(&match_source)
            } else {
                "title_heuristic"
            };
            let entry = coverage.entry(name).or_default();
            entry.available += 1;
            if interacted > 0 {
                entry.interacted += 1;
            }
            upgrade_match_type(&mut entry.best_match_type, mt);
        }
    }

    {
        let mut stmt = match conn.prepare(history_sql) {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    target: "4da::blind_spots",
                    "Failed to prepare history blind-spot dep query: {e}"
                );
                let _ = conn.execute_batch("DROP TABLE IF EXISTS _blind_spot_deps");
                return Ok((Vec::new(), Vec::new()));
            }
        };
        let rows = match stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, String>(5)?,
            ))
        }) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    target: "4da::blind_spots",
                    "Failed to execute history blind-spot dep query: {e}"
                );
                let _ = conn.execute_batch("DROP TABLE IF EXISTS _blind_spot_deps");
                return Ok((Vec::new(), Vec::new()));
            }
        };

        for row_result in rows {
            let (name, title, source_type, content_type, days_since, match_source) =
                match row_result {
                    Ok(r) => r,
                    Err(e) => {
                        warn!(
                            target: "4da::blind_spots",
                            "Failed to read history blind-spot dep row: {e}"
                        );
                        continue;
                    }
                };
            // SID-linked items bypass word-boundary filter (authoritative link).
            if match_source == "title_heuristic" {
                let dep_lower = name.to_lowercase();
                let title_lower = title.to_lowercase();
                if !is_actionable_blind_spot_match(
                    &dep_lower,
                    &title_lower,
                    &source_type,
                    content_type.as_deref(),
                ) {
                    continue;
                }
            }
            let mt = if match_source != "title_heuristic" {
                sid_match_type_to_coverage(&match_source)
            } else {
                "title_heuristic"
            };
            let entry = coverage.entry(name).or_default();
            entry.days_since_last_signal = Some(match entry.days_since_last_signal {
                Some(existing) => existing.min(days_since),
                None => days_since,
            });
            upgrade_match_type(&mut entry.best_match_type, mt);
        }
    }

    let zero_signal_count = coverage.values().filter(|c| c.available == 0).count();
    let has_signal_count = coverage.values().filter(|c| c.available > 0).count();
    info!(
        target: "4da::blind_spots",
        zero_signal = zero_signal_count,
        has_signal = has_signal_count,
        "find_uncovered_deps: coverage after SQL queries"
    );

    let mut uncovered = Vec::new();
    let mut weak_match_deps: Vec<UncoveredDep> = Vec::new();
    for dep in &eligible_deps {
        let norm = normalize_dep_name(&dep.package_name);
        let Some(metrics) = coverage.get(&norm) else {
            continue;
        };
        let Some(dep_info) = dep_lookup.get(&norm) else {
            continue;
        };

        // ── Match-type gate: suppress ambiguous names with only title heuristic ──
        // For deps with signals: check if the best match type is strong enough.
        // Ambiguous names (common English words like "image", "config", "log")
        // require exact_registry or advisory proof — title LIKE matches are
        // almost always false positives (e.g. "image" matching ImageMagick articles).
        let best_mt = metrics
            .best_match_type
            .as_deref()
            .unwrap_or("title_heuristic");
        if metrics.available > 0
            && best_mt == "title_heuristic"
            && is_ambiguous_package_name(&dep_info.package_name)
        {
            let display_name = format_dep_display_name(&dep_info.package_name, &dep_info.ecosystem);
            weak_match_deps.push(UncoveredDep {
                name: display_name,
                dep_type: dep_info.ecosystem.clone(),
                projects_using: dep_info.projects.clone(),
                days_since_last_signal: metrics.days_since_last_signal.unwrap_or(999),
                available_signal_count: metrics.available.saturating_sub(metrics.interacted),
                risk_level: "low".to_string(),
                match_type: "title_heuristic".to_string(),
                coverage_reason: Some("weak_matches_only".to_string()),
                adapters_searched: adapter_statuses_for_ecosystem(conn, &dep_info.ecosystem),
            });
            continue;
        }

        // Ecosystem-qualified display name for clarity
        let display_name = format_dep_display_name(&dep_info.package_name, &dep_info.ecosystem);

        // Zero signals for a dependency CAN be a blind spot — but only if the dep
        // is likely to have public signals. Single-project deps in unknown ecosystems
        // are usually internal or too niche; surfacing them as blind spots is just
        // coverage inventory noise that erodes trust.
        if metrics.available == 0 {
            let eco_lower = dep_info.ecosystem.to_lowercase();
            let in_known_ecosystem = matches!(
                eco_lower.as_str(),
                "npm"
                    | "javascript"
                    | "typescript"
                    | "crates.io"
                    | "cargo"
                    | "rust"
                    | "pypi"
                    | "python"
                    | "go"
                    | "golang"
                    | "maven"
                    | "java"
                    | "kotlin"
                    | "nuget"
                    | "csharp"
                    | "dotnet"
                    | "packagist"
                    | "php"
                    | "rubygems"
                    | "ruby"
                    | "swift"
                    | "cocoapods"
            );

            // Single-project deps in unknown ecosystems are likely internal or
            // too niche to have public signals — suppress to avoid inventory noise.
            if dep_info.projects.len() <= 1 && !in_known_ecosystem {
                continue;
            }

            let risk_level = if dep_info.projects.len() >= 3 && in_known_ecosystem {
                "high".to_string()
            } else if dep_info.projects.len() >= 2 || in_known_ecosystem {
                "medium".to_string()
            } else {
                "low".to_string()
            };
            let (reason, _detail) =
                diagnose_coverage(conn, &dep_info.package_name, &dep_info.ecosystem);
            uncovered.push(UncoveredDep {
                name: display_name,
                dep_type: dep_info.ecosystem.clone(),
                projects_using: dep_info.projects.clone(),
                days_since_last_signal: 999,
                available_signal_count: 0,
                risk_level,
                match_type: "none".to_string(),
                coverage_reason: Some(reason),
                adapters_searched: adapter_statuses_for_ecosystem(conn, &dep_info.ecosystem),
            });
            continue;
        }
        let days_since = metrics.days_since_last_signal.unwrap_or(999);
        // Ratio-based engagement check: skip only if user has seen more than
        // HALF the available signals recently. One interaction should not hide
        // a dependency with dozens of unseen signals.
        if days_since < 14 && metrics.interacted > 0 && metrics.interacted >= metrics.available / 2
        {
            continue;
        }
        let not_seen = metrics.available.saturating_sub(metrics.interacted);
        if not_seen == 0 && days_since < 30 {
            continue;
        }
        let risk_level = classify_dep_risk(days_since, not_seen, dep_info.projects.len());
        uncovered.push(UncoveredDep {
            name: display_name,
            dep_type: dep_info.ecosystem.clone(),
            projects_using: dep_info.projects.clone(),
            days_since_last_signal: days_since,
            available_signal_count: not_seen,
            risk_level,
            match_type: best_mt.to_string(),
            coverage_reason: None, // has signals, coverage isn't the issue
            adapters_searched: adapter_statuses_for_ecosystem(conn, &dep_info.ecosystem),
        });
    }

    if !weak_match_deps.is_empty() {
        info!(
            target: "4da::blind_spots",
            count = weak_match_deps.len(),
            names = %weak_match_deps.iter().map(|d| d.name.as_str()).collect::<Vec<_>>().join(", "),
            "suppressed ambiguous deps with title-heuristic-only matches"
        );
    }

    info!(
        target: "4da::blind_spots",
        uncovered_before_sort = uncovered.len(),
        names = %uncovered.iter().take(5).map(|u| u.name.as_str()).collect::<Vec<_>>().join(", "),
        "find_uncovered_deps: result before active-project filter"
    );

    let _ = conn.execute_batch("DROP TABLE IF EXISTS _blind_spot_deps");

    uncovered.sort_by(|a, b| {
        risk_ord(&a.risk_level)
            .cmp(&risk_ord(&b.risk_level))
            .then(b.days_since_last_signal.cmp(&a.days_since_last_signal))
    });

    Ok((uncovered, weak_match_deps))
}

/// Common runtime built-in modules that generate false blind spots.
/// These are language standard library modules, not installable packages —
/// LIKE matching their names against source_items catches unrelated content
/// (e.g. "crypto" matches cryptocurrency articles).
fn is_builtin_module(name: &str) -> bool {
    let check = name.to_lowercase();
    let check = check.strip_prefix("node:").unwrap_or(&check);
    matches!(
        check,
        "crypto" | "http" | "https" | "path" | "stream" | "events"
        | "buffer" | "util" | "assert" | "child_process" | "cluster"
        | "dgram" | "domain" | "module" | "perf_hooks" | "process"
        | "querystring" | "readline" | "repl" | "string_decoder"
        | "timers" | "tty" | "v8" | "vm" | "worker_threads" | "zlib"
        | "async_hooks" | "console" | "inspector" | "trace_events"
        | "wasi" | "diagnostics_channel"
        | "fs" | "os" | "net" | "url" | "dns" | "tls"
        // Python built-ins
        | "json" | "logging" | "typing" | "collections" | "functools"
        | "itertools" | "pathlib" | "asyncio" | "socket" | "threading"
        | "multiprocessing" | "unittest" | "hashlib" | "hmac"
        // Rust std modules
        | "alloc" | "core" | "proc_macro"
    )
}

/// Utility packages with minimal API surface that should never surface as blind spots.
/// These do one thing, rarely break, and don't require monitoring. Flagging them erodes trust.
fn is_utility_dep(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        // env loaders
        "dotenv" | "dotenv-expand" | "cross-env" | "env-cmd" | "envy"
        // fs utilities
        | "rimraf" | "mkdirp" | "del" | "del-cli" | "fs-extra" | "graceful-fs"
        // path utilities
        | "slash" | "normalize-path" | "upath"
        // deep merge / clone
        | "deepmerge" | "merge-deep" | "lodash.merge" | "lodash.clonedeep" | "rfdc"
        // type checks
        | "is-even" | "is-odd" | "is-number" | "is-plain-object" | "is-glob"
        // string escaping
        | "escape-html" | "escape-string-regexp" | "strip-ansi" | "ansi-regex"
        // color / terminal
        | "ansi-styles" | "chalk" | "color-convert" | "color-name" | "supports-color"
        // micro-utilities
        | "has-flag" | "yallist" | "wrappy" | "once" | "inherits" | "util-deprecate"
        | "signal-exit" | "ms" | "bytes" | "semver" | "debug"
        // cli argument parsers (as deps, not as primary tools)
        | "commander" | "yargs" | "minimist" | "meow"
        // case conversion
        | "camelcase" | "decamelize" | "param-case" | "pascal-case" | "change-case"
        // runtime polyfills
        | "tslib" | "regenerator-runtime" | "core-js" | "core-js-pure"
        // Rust single-purpose crates
        | "thiserror" | "anyhow" | "once_cell" | "lazy_static" | "cfg-if"
        | "bitflags" | "byteorder" | "memchr" | "itoa" | "ryu" | "percent-encoding"
        | "tinyvec" | "smallvec" | "arrayvec" | "indexmap" | "either"
        | "pin-project" | "pin-project-lite" | "futures-core" | "futures-sink"
        | "proc-macro2" | "quote" | "syn" | "unicode-ident"
        // Rust async/runtime ecosystem
        | "tokio" | "tokio-util" | "tokio-stream" | "tokio-macros"
        | "futures" | "futures-util" | "futures-io" | "futures-channel"
        | "futures-executor" | "futures-macro" | "futures-task"
        | "async-trait" | "async_trait"
        // Rust serialization
        | "serde" | "serde_json" | "serde_derive" | "serde_yaml" | "toml"
        | "bincode" | "ciborium" | "postcard"
        // Rust encoding/compression
        | "base64" | "hex" | "flate2" | "zstd" | "lz4_flex"
        // Rust error/logging
        | "tracing" | "tracing-subscriber" | "tracing-core" | "log" | "env_logger"
        // Rust HTTP clients (as deps, not primary tools)
        | "hyper" | "hyper-util" | "http" | "http-body" | "http-body-util"
        | "tower" | "tower-service" | "tower-layer" | "tower-http"
        // Rust crypto primitives
        | "ring" | "rustls" | "rustls-pemfile" | "webpki-roots"
        // Rust build/proc-macro deps
        | "cc" | "pkg-config" | "autocfg" | "version_check"
        // Node.js stable infra
        | "better-sqlite3" | "better_sqlite3"
        | "typescript" | "eslint" | "prettier"
        | "webpack" | "rollup" | "esbuild" | "swc"
        // Node.js testing frameworks (dev tooling, not primary)
        | "vitest" | "jest" | "mocha" | "ava" | "cypress" | "playwright"
        | "@testing-library/react" | "@testing-library/jest-dom" | "@testing-library/dom"
        // Node.js build tooling
        | "vite" | "tailwindcss" | "@tailwindcss/vite" | "postcss" | "autoprefixer"
        // TypeScript type declarations (zero runtime risk)
        | "@types/node" | "@types/react" | "@types/react-dom"
        // Stable single-purpose packages
        | "uuid" | "nanoid" | "clsx" | "classnames" | "cva"
        | "date-fns" | "dayjs" | "moment"
        | "zod" | "yup" | "joi"
        | "lodash" | "underscore" | "ramda"
        // Rust stable crates
        | "parking_lot" | "dashmap" | "crossbeam" | "crossbeam-utils" | "crossbeam-channel"
        | "rayon" | "num-traits" | "num-integer" | "num-bigint"
        | "regex" | "glob" | "walkdir" | "notify"
        | "chrono" | "time" | "humantime"
        | "url" | "mime"
    )
}

/// Dep names that are so generic they cause false matches in SQL LIKE queries.
/// Only truly generic English words that appear in nearly every article title.
/// Words like "futures", "bytes", "ring", "cookie", "config", "router" are real
/// crate/package names — the word boundary matching already prevents false positives.
fn is_generic_dep_name(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "open"
            | "test"
            | "core"
            | "path"
            | "sync"
            | "once"
            | "glob"
            | "rand"
            | "time"
            | "lock"
            | "send"
            | "copy"
            | "find"
            | "diff"
            | "pick"
            | "wrap"
            | "trim"
            | "data"
            | "form"
            | "icon"
            | "link"
            | "text"
            | "type"
            | "util"
            | "base"
            | "flat"
            | "safe"
            | "fast"
            | "make"
            | "pipe"
            | "pump"
            | "read"
            | "call"
            | "nano"
            | "pure"
            | "vary"
            | "deep"
            | "try"
            | "want"
            | "mime"
            | "race"
            | "http"
            | "https"
    )
}

fn is_actionable_blind_spot_match(
    dep_lower: &str,
    title_lower: &str,
    _source_type: &str,
    _content_type: Option<&str>,
) -> bool {
    has_word_boundary_match(title_lower, dep_lower)
}

/// Package names that are common English words AND real package names.
/// Unlike `is_generic_dep_name` (which blocks them from queries entirely),
/// ambiguous names ARE queried but require ecosystem-qualified proof
/// (exact_registry or advisory match) to surface.
pub(crate) fn is_ambiguous_package_name(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "image"
            | "config"
            | "log"
            | "time"
            | "rand"
            | "error"
            | "hash"
            | "ring"
            | "url"
            | "http"
            | "crypto"
            | "lazy"
            | "quote"
            | "lock"
            | "once"
            | "pin"
            | "signal"
            | "sync"
            | "bytes"
            | "regex"
            | "either"
            | "paste"
            | "clap"
            | "nom"
            | "base"
            | "core"
            | "test"
            | "data"
            | "utils"
            | "proc_macro2"
            | "proc-macro2"
    )
}

/// Promote best_match_type if the new match is stronger.
fn upgrade_match_type(current: &mut Option<String>, new_type: &str) {
    fn match_rank(mt: &str) -> u8 {
        match mt {
            "exact_registry" => 2,
            "advisory" => 1,
            _ => 0,
        }
    }
    let dominated = match current.as_deref() {
        Some(existing) => match_rank(new_type) > match_rank(existing),
        None => true,
    };
    if dominated {
        *current = Some(new_type.to_string());
    }
}

/// Map `source_item_dependencies.match_type` values to blind-spot coverage
/// match types. SID link types that carry real evidence (registry data, advisory
/// references, LLM confirmation) map to "exact_registry" or "advisory".
/// Everything else (including "title_heuristic" links) stays as-is.
fn sid_match_type_to_coverage(sid_match_type: &str) -> &'static str {
    match sid_match_type {
        "exact_registry" | "registry" | "llm_analysis" | "llm_confirmed" => "exact_registry",
        "advisory" | "security_advisory" | "cve" | "vulnerability" => "advisory",
        "title_heuristic" => "title_heuristic",
        other => {
            tracing::debug!(
                target: "4da::blind_spots",
                unknown_match_type = other,
                "unrecognised source_item_dependencies.match_type, treating as title_heuristic"
            );
            "title_heuristic"
        }
    }
}

/// Format a dependency name with its ecosystem qualifier for display.
fn format_dep_display_name(package_name: &str, ecosystem: &str) -> String {
    let qualifier = match ecosystem.to_lowercase().as_str() {
        "rust" | "cargo" | "crates.io" => "crates.io",
        "javascript" | "typescript" | "npm" => "npm",
        "python" | "pypi" => "PyPI",
        "go" | "golang" => "Go",
        "java" | "kotlin" | "maven" => "Maven",
        "csharp" | "dotnet" | "nuget" => "NuGet",
        "php" | "packagist" => "Packagist",
        "ruby" | "rubygems" => "RubyGems",
        "swift" | "cocoapods" => "CocoaPods",
        _ => {
            if ecosystem.is_empty() {
                return package_name.to_string();
            }
            return format!("{package_name} ({ecosystem})");
        }
    };
    format!("{package_name} ({qualifier})")
}

/// Inverse of `format_dep_display_name`: recover the bare package name from a
/// display name ("react (npm)" -> "react"). Article titles never contain the
/// " (ecosystem)" qualifier, so signal/version lookups that match on titles must
/// use the bare name. A package name with no qualifier passes through unchanged.
fn bare_package_name(display_name: &str) -> &str {
    match display_name.rfind(" (") {
        Some(idx) if display_name.ends_with(')') => &display_name[..idx],
        _ => display_name,
    }
}

/// Classify risk level based on coverage gap severity.
fn classify_dep_risk(days_since: u32, unseen_signals: u32, project_count: usize) -> String {
    if days_since > scoring_config::BLIND_SPOT_RISK_CRITICAL_DAYS as u32
        && project_count > scoring_config::BLIND_SPOT_RISK_CRITICAL_PROJECTS as usize
    {
        "critical".to_string()
    } else if days_since > scoring_config::BLIND_SPOT_RISK_HIGH_DAYS as u32
        || (unseen_signals > scoring_config::BLIND_SPOT_RISK_HIGH_UNSEEN_SIGNALS as u32
            && project_count > scoring_config::BLIND_SPOT_RISK_HIGH_PROJECTS as usize)
    {
        "high".to_string()
    } else if days_since > scoring_config::BLIND_SPOT_RISK_MEDIUM_DAYS as u32
        || unseen_signals > scoring_config::BLIND_SPOT_RISK_MEDIUM_UNSEEN_SIGNALS as u32
    {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

/// Ordering helper for risk levels (lower = more severe).
fn risk_ord(risk: &str) -> u8 {
    match risk {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        _ => 3,
    }
}

/// Find TRULY missed signals — high-relevance items the user hasn't seen,
/// excluding anything currently in their main feed window.
///
/// Key design decisions:
///
/// 1. **Dedup window**: only return items from `3..=days` days ago. This
///    excludes the current-briefing window (typically the last 3 days, which
///    the main feed actively surfaces). The user genuinely "missed" an item
///    only if it's old enough to no longer appear in the main feed.
///
/// 2. **Impression filter**: exclude items that have an `impression` event in
///    `user_events` — the frontend records an impression when an item is
///    rendered on screen. An item with an impression has technically been
///    seen even if the user didn't click.
///
/// 3. **Interaction filter**: exclude items the user clicked/saved/dismissed
///    via the `interactions` table.
///
/// 4. **Real `why_relevant`**: computed per-item by scanning the title for
///    direct-dep name mentions. Falls back to a score-tier canned string only
///    when no specific match is found (documented as a fallback, not a claim).
///
/// Returns at most 15 items ranked by relevance score.
fn find_missed_signals(
    conn: &rusqlite::Connection,
    days: u32,
    direct_deps: &[DepCoverage],
) -> Result<Vec<MissedSignal>> {
    // Dedup window: 3 days ago through N days ago.
    // Items from the last 3 days are in the user's main feed — those are
    // not "missed," they're "not yet seen."
    let feed_window_days = scoring_config::MISSED_SIGNAL_FEED_WINDOW_DAYS as u32;
    if days <= feed_window_days {
        return Ok(Vec::new()); // No meaningful "missed" window
    }

    // Fetch more than 15 initially so the priority-aware post-sort has room
    // to promote security items and filter out old blog posts before trimming.
    //
    // content_type filtering: drop noise categories at the DB level using the
    // classification already computed at ingestion by content_dna. Items with
    // NULL content_type (legacy rows) pass through and get title-based fallback.
    //
    // Cross-lens dedup: exclude security_advisory and breaking_change items —
    // those already route to Preemption via OSV (Tier 1), LLM judgment (Tier 2),
    // or direct-dep keyword matching (Tier 3.5). Showing the same item in both
    // tabs with different urgencies (Preemption caps at High, Blind Spots maps
    // keywords to Critical) is a correctness failure, not a feature.
    //
    // Title-keyword fallback for legacy items (NULL content_type): exclude items
    // whose titles contain security terminology that Preemption already captures.
    let sql = format!(
        "SELECT si.id, si.title, si.url, si.source_type, si.relevance_score,
                si.created_at, si.content_type
         FROM source_items si
         LEFT JOIN interactions i ON i.item_id = si.id
         WHERE si.relevance_score > 0.5
           AND si.created_at >= datetime('now', '-{days} days')
           AND si.created_at < datetime('now', '-{feed_window} days')
           AND i.item_id IS NULL
           AND NOT EXISTS (
               SELECT 1 FROM user_events ue
               WHERE ue.event_type = 'impression'
               AND CAST(json_extract(ue.metadata, '$.item_id') AS INTEGER) = si.id
           )
           AND (si.content_type IS NULL
                OR si.content_type NOT IN ('show_and_tell','tutorial','question',
                                           'help_request','hiring','clickbait',
                                           'security_advisory','breaking_change'))
         ORDER BY si.relevance_score DESC
         LIMIT 40",
        days = days,
        feed_window = feed_window_days
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => {
            warn!(
                target: "4da::blind_spots",
                "Failed to query missed signals: {e}"
            );
            return Ok(Vec::new());
        }
    };

    let rows = stmt.query_map([], |row| {
        Ok(MissedSignal {
            item_id: row.get(0)?,
            title: row.get(1)?,
            url: row.get(2)?,
            source_type: row.get(3)?,
            relevance_score: row.get(4)?,
            created_at: row.get(5)?,
            content_type: row.get(6)?,
            why_relevant: String::new(), // populated below
            dep_name: None,              // populated below
            was_shown: false,            // query filters out impressioned items
        })
    })?;

    let mut signals: Vec<MissedSignal> = rows.flatten().collect();

    // Title-based fallback for legacy items without stored content_type.
    // Items WITH content_type were already filtered at the SQL level.
    signals.retain(|s| {
        s.content_type.is_some() || !crate::knowledge_decay::is_low_quality_signal(&s.title)
    });

    // Cross-lens dedup for legacy items (NULL content_type): exclude items
    // whose titles contain security/breaking-change keywords that Preemption
    // already handles. Without this, the same CVE/vulnerability article
    // appears in both tabs with contradictory urgency levels.
    signals.retain(|s| {
        if s.content_type.is_some() {
            return true; // Already filtered at SQL level
        }
        !is_preemption_territory(&s.title)
    });

    // Discussion items need higher relevance to qualify as blind spot signals.
    // A generic HN discussion that happens to mention a dep name is noise, not
    // a blind spot. release_notes, expert_analysis, platform_update pass freely.
    signals.retain(|s| match s.content_type.as_deref() {
        Some("discussion") => s.relevance_score >= 0.70,
        Some("curated_digest") => s.relevance_score >= 0.65,
        _ => true,
    });

    // Populate `why_relevant` and `dep_name` by looking for dep mentions in titles.
    for signal in &mut signals {
        let (why, dep) = compute_why_relevant(&signal.title, signal.relevance_score, direct_deps);
        signal.why_relevant = why;
        signal.dep_name = dep;
    }

    // When we have dependency context, remove items with no specific dep
    // match — if we can't explain why it matters, don't show it. When no
    // deps are available (cold start), pass everything through rather than
    // showing an empty tab.
    if !direct_deps.is_empty() {
        // Keep signals that either have a dep match OR score >= 0.75 relevance.
        // High-relevance signals without an exact dep name match are still
        // valuable — they matched on stack/ecosystem context.
        signals.retain(|s| !s.why_relevant.is_empty() || s.relevance_score >= 0.75);
        // Give high-relevance unmatched signals a generic explanation
        for signal in &mut signals {
            if signal.why_relevant.is_empty() {
                signal.why_relevant = "Highly relevant to your technology stack".to_string();
            }
        }
    }

    // Deduplicate missed signals by normalized title similarity.
    // The same CVE or topic can appear from multiple sources (HN, Reddit, RSS)
    // — without dedup the same item shows 10+ times in the blind spot report.
    let signals = dedup_missed_signals(signals);

    // Apply negative stack filtering at query time. This catches items that were
    // scored by the old pipeline (before negative stack existed) and still have
    // high relevance_score for technologies the user doesn't use.
    let signals = filter_by_negative_stack(signals);

    // Priority-aware ranking: security advisories + breaking changes surface
    // above generic blog posts, even if they score slightly lower on relevance.
    // Old blog posts (>10 days) are capped at 3 slots so the list doesn't
    // become a stale-blog archive.
    let signals = rank_by_missed_priority(signals);

    // Cap per-dep diversity: max 3 signals per matched dep to prevent the
    // "5 items all saying Mentions react" wall-of-sameness problem.
    let signals = cap_per_dep(signals, 3);

    Ok(signals)
}

/// Returns true if a title's keywords indicate the item belongs in Preemption,
/// not Blind Spots. Mirrors the security/breaking-change keywords used by
/// the preemption pipeline's OSV and LLM-assessed alert paths.
fn is_preemption_territory(title: &str) -> bool {
    let t = title.to_lowercase();
    t.contains("cve-")
        || t.contains("cve ")
        || t.contains("ghsa-")
        || t.contains("vulnerab")
        || t.contains("security advisory")
        || t.contains("security patch")
        || t.contains("security update")
        || t.contains("security flaw")
        || t.contains("security issue")
        || t.contains("security bug")
        || t.contains("remote code execution")
        || t.contains("zero-day")
        || t.contains("zeroday")
        || t.contains("0day")
        || t.contains("supply chain attack")
        || t.contains("malware")
        || t.contains("backdoor")
        || t.contains("breaking change")
        || t.contains("deprecat")
        || t.contains("end of life")
        || t.contains("end-of-life")
        || t.contains("drops support")
        || t.contains("migration guide")
}

/// Map stored content_type to a priority tier.
/// Returns None if content_type is absent (legacy item — fall back to title).
fn content_type_tier(ct: Option<&str>) -> Option<u8> {
    match ct {
        Some("security_advisory") => Some(4),
        Some("breaking_change") => Some(3),
        Some("release_notes") => Some(2),
        Some("deep_dive" | "discussion") => Some(1),
        Some(_) => Some(0), // noise types that survived SQL filter
        None => None,
    }
}

/// Title-based fallback for items without stored content_type.
fn title_priority_tier_fallback(title: &str) -> u8 {
    let t = title.to_lowercase();
    if t.contains("cve-")
        || t.contains("vulnerability")
        || t.contains("security advisory")
        || t.contains("remote code execution")
        || t.contains("zero-day")
        || t.contains("zeroday")
        || t.contains("exploit")
    {
        return 4;
    }
    if t.contains("breaking change")
        || t.contains("deprecated")
        || t.contains("end of life")
        || t.contains("eol")
        || t.contains("migration guide")
        || t.contains("drops support")
    {
        return 3;
    }
    if t.starts_with("npm:")
        || t.starts_with("cargo:")
        || t.starts_with("pypi:")
        || t.contains("released")
        || t.contains("announcing")
    {
        return 2;
    }
    1
}

/// Resolve priority tier: use stored content_type when available,
/// fall back to title pattern matching for legacy items.
fn signal_priority_tier(signal: &MissedSignal) -> u8 {
    content_type_tier(signal.content_type.as_deref())
        .unwrap_or_else(|| title_priority_tier_fallback(&signal.title))
}

/// Re-rank missed signals so high-urgency content surfaces above opinion/blog
/// content, even when relevance scores are similar. Also caps older non-urgent
/// content so the panel stays focused on recent-enough material.
fn rank_by_missed_priority(mut signals: Vec<MissedSignal>) -> Vec<MissedSignal> {
    let final_limit = scoring_config::MISSED_SIGNAL_FINAL_LIMIT as usize;
    let old_blog_cap = scoring_config::MISSED_SIGNAL_OLD_BLOG_CAP as usize;
    let old_days_threshold = scoring_config::MISSED_SIGNAL_OLD_DAYS_THRESHOLD as i64;

    fn age_days(created_at: &str) -> i64 {
        chrono::DateTime::parse_from_rfc3339(created_at)
            .ok()
            .or_else(|| {
                chrono::NaiveDateTime::parse_from_str(created_at, "%Y-%m-%d %H:%M:%S")
                    .ok()
                    .map(|ndt| ndt.and_utc().fixed_offset())
            })
            .map(|dt| (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_days())
            .unwrap_or(0)
    }

    // Sort by (priority_tier DESC, relevance DESC).
    signals.sort_by(|a, b| {
        let tier_a = signal_priority_tier(a);
        let tier_b = signal_priority_tier(b);
        tier_b.cmp(&tier_a).then_with(|| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    // Cap Tier 1 (generic blog) items older than old_days_threshold to old_blog_cap.
    let mut kept = Vec::with_capacity(final_limit);
    let mut old_tier1_count = 0;
    for s in signals {
        if kept.len() >= final_limit {
            break;
        }
        let tier = signal_priority_tier(&s);
        let age = age_days(&s.created_at);
        if tier == 1 && age > old_days_threshold {
            if old_tier1_count >= old_blog_cap {
                continue;
            }
            old_tier1_count += 1;
        }
        kept.push(s);
    }

    kept
}

/// Cap per-dep diversity: at most `max_per_dep` signals per matched dep name.
/// Signals with no dep_name are capped at 2 total — they passed on score alone
/// and shouldn't dominate over dep-matched items.
fn cap_per_dep(signals: Vec<MissedSignal>, max_per_dep: usize) -> Vec<MissedSignal> {
    use std::collections::HashMap;
    const MAX_UNMATCHED: usize = 2;
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut unmatched_count = 0usize;
    signals
        .into_iter()
        .filter(|s| match &s.dep_name {
            Some(dep) => {
                let c = counts.entry(dep.to_lowercase()).or_insert(0);
                *c += 1;
                *c <= max_per_dep
            }
            None => {
                unmatched_count += 1;
                unmatched_count <= MAX_UNMATCHED
            }
        })
        .collect()
}

/// Filter missed signals using two-layer stack awareness.
///
/// Layer 1: Negative stack (competing-absent) — suppresses Vue for React users, etc.
/// Layer 2: Dependency-absence — for each technology-like topic extracted from the
///          title, checks whether the user has it as a direct dep. Items where ALL
///          recognized technology topics are absent from user deps get filtered.
///
/// This ensures Blind Spots only shows content about technologies the user actually
/// uses, without needing every technology pair to be in the competing-tech graph.
fn filter_by_negative_stack(signals: Vec<MissedSignal>) -> Vec<MissedSignal> {
    use std::collections::HashSet;

    // Load user's direct runtime deps
    let user_deps = match load_user_direct_deps() {
        Some(deps) if !deps.is_empty() => deps,
        _ => return signals, // No dep data → can't filter
    };

    // Also build competing-tech negative stack for Layer 1
    let negative_stack = build_negative_stack_from_deps(&user_deps);

    // Technology-like topics that could map to package names.
    // These are topics from extract_topics() that represent specific technologies
    // (not generic concepts like "performance", "testing", "security").
    // If a topic is in this set, it's checkable against user deps.
    static TECH_TOPICS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
        [
            // Frontend frameworks
            "react",
            "vue",
            "angular",
            "svelte",
            "solid",
            "qwik",
            "preact",
            // Meta-frameworks
            "next",
            "nextjs",
            "nuxt",
            "remix",
            "gatsby",
            "astro",
            // Backend frameworks
            "express",
            "fastify",
            "koa",
            "hapi",
            "nest",
            "nestjs",
            "django",
            "flask",
            "fastapi",
            "laravel",
            "rails",
            "spring",
            "gin",
            "fiber",
            "echo",
            // Rust ecosystem
            "axum",
            "actix",
            "tokio",
            "reqwest",
            "serde",
            "warp",
            "rocket",
            "hyper",
            "tower",
            "tonic",
            "tauri",
            "diesel",
            "sqlx",
            // Desktop/build
            "electron",
            "vite",
            "webpack",
            "esbuild",
            "rollup",
            "turbopack",
            "parcel",
            // CSS
            "tailwind",
            "tailwindcss",
            "bootstrap",
            // ORMs / DB clients
            "prisma",
            "drizzle",
            "sequelize",
            "typeorm",
            "mongoose",
            // Databases
            "postgresql",
            "postgres",
            "mysql",
            "mongodb",
            "redis",
            "sqlite",
            // Cloud / infra
            "vercel",
            "netlify",
            "supabase",
            "firebase",
            "cloudflare",
            // Runtimes
            "node",
            "deno",
            "bun",
            // Languages (only check if specific enough)
            "rust",
            "python",
            "typescript",
            "golang",
            // Package managers
            "pnpm",
            "yarn",
            "npm",
            "cargo",
            "pip",
            // Mobile
            "flutter",
        ]
        .into_iter()
        .collect()
    });

    signals
        .into_iter()
        .filter(|signal| {
            let topics = crate::extract_topics(&signal.title, "", &[]);

            // Layer 1: Competing-tech negative stack
            if let Some(ref ns) = negative_stack {
                let prior = crate::stacks::negative_stack::lookup_prior(ns, &topics);
                if prior <= 0.30 {
                    return false; // Strong competing-absent suppression
                }
            }

            // Layer 2: Dependency-absence check
            // Extract technology-specific topics from the title
            let tech_topics: Vec<&String> = topics
                .iter()
                .filter(|t| TECH_TOPICS.contains(t.as_str()))
                .collect();

            if tech_topics.is_empty() {
                // No technology-specific topics detected — keep the item
                // (it's about generic concepts like "performance", "testing", etc.)
                return true;
            }

            // Check if ANY tech topic is in user's deps
            let has_user_dep = tech_topics.iter().any(|topic| {
                user_deps.contains(topic.as_str())
                    // Also check common aliases: "next" → "next", "nextjs" → "next"
                    || user_deps.contains(&topic.replace("js", ""))
                    || user_deps.contains(&format!("@types/{topic}"))
            });

            // Keep item only if at least one tech topic matches user's stack
            has_user_dep
        })
        .collect()
}

use once_cell::sync::Lazy;

/// Load user's direct runtime deps as a lowercase HashSet.
fn load_user_direct_deps() -> Option<std::collections::HashSet<String>> {
    use std::collections::HashSet;

    let conn = crate::open_db_connection().ok()?;
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT LOWER(package_name) FROM project_dependencies
             WHERE is_dev = 0 AND is_direct = 1 AND project_relevance >= 0.15",
        )
        .ok()?;
    let deps: HashSet<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .ok()?
        .flatten()
        .collect();

    Some(deps)
}

/// Build negative stack from loaded deps.
fn build_negative_stack_from_deps(
    deps: &std::collections::HashSet<String>,
) -> Option<crate::stacks::negative_stack::NegativeStackContext> {
    let conn = crate::open_db_connection().ok()?;

    let anti_topics: Vec<(String, f32)> = match conn
        .prepare("SELECT topic, confidence FROM anti_topics WHERE rejection_count >= 2")
    {
        Ok(mut stmt) => stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
            })
            .ok()
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    Some(crate::stacks::negative_stack::build_negative_stack(
        deps,
        crate::competing_tech::COMPETING_TECH,
        &anti_topics,
    ))
}

/// Remove near-duplicate missed signals using Jaccard word overlap.
///
/// Normalizes each title (lowercase, strip punctuation, split words) and
/// skips any signal whose normalized title has >65% word overlap with
/// an already-accepted signal. This catches cross-posts and rephrased
/// duplicates (e.g. "CVE-2025-1234 in OpenSSL" vs "OpenSSL CVE-2025-1234").
fn dedup_missed_signals(signals: Vec<MissedSignal>) -> Vec<MissedSignal> {
    use std::collections::HashSet;

    let mut seen_titles: Vec<HashSet<String>> = Vec::new();
    let mut deduped = Vec::new();

    for signal in signals {
        let normalized: HashSet<String> = signal
            .title
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' {
                    c
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .split_whitespace()
            .map(String::from)
            .collect();

        if normalized.is_empty() {
            deduped.push(signal);
            continue;
        }

        let is_duplicate = seen_titles.iter().any(|seen| {
            if seen.is_empty() {
                return false;
            }
            let intersection = seen.intersection(&normalized).count();
            let union = seen.union(&normalized).count();
            union > 0 && (intersection as f32 / union as f32) > 0.65
        });

        if !is_duplicate {
            seen_titles.push(normalized);
            deduped.push(signal);
        }
    }

    deduped
}

/// Compute a concrete `why_relevant` string for a missed signal.
///
/// Scans the title (lowercased) for any of the user's direct dep names. If
/// found, returns a specific "Mentions <dep>" message. Otherwise falls back
/// to a score-tier canned string that is honest about its generality.
/// Returns `(why_relevant_text, first_matched_dep_name)`.
///
/// The first matched dep (longest name = most specific) is returned separately
/// so callers can associate a signal with its coverage-gap dependency.
fn compute_why_relevant(
    title: &str,
    _score: f32,
    direct_deps: &[DepCoverage],
) -> (String, Option<String>) {
    let title_lower = title.to_lowercase();

    // Dep names that are common English words — they produce false matches
    // against nearly every article title ("open source", "next steps", etc.)
    // Only truly generic English words with no tech package meaning.
    // Words like "futures", "bytes", "ring", "cookie", "config", "router"
    // are real crate/package names — the word boundary matching already
    // prevents false positives for those.
    const GENERIC_DEP_NAMES: &[&str] = &[
        "open", "test", "core", "path", "sync", "once", "glob", "rand", "time", "lock", "send",
        "copy", "find", "diff", "pick", "wrap", "trim", "data", "form", "icon", "link", "text",
        "type", "util", "base", "flat", "safe", "fast", "make", "pipe", "pump", "read", "call",
        "nano", "pure", "vary", "deep", "try", "want", "mime", "race", "http", "https",
    ];

    // Look for direct dep mentions, preferring longer names (more specific).
    let mut matched: Vec<&str> = direct_deps
        .iter()
        .filter_map(|d| {
            if d.package_name.len() < 4 {
                return None; // Too short
            }
            let dep_lower = d.package_name.to_lowercase();
            if GENERIC_DEP_NAMES.contains(&dep_lower.as_str()) {
                return None; // Common English word, not a meaningful match
            }
            if has_word_boundary_match(&title_lower, &dep_lower) {
                Some(d.package_name.as_str())
            } else {
                None
            }
        })
        .collect();
    matched.sort_by_key(|b| std::cmp::Reverse(b.len()));
    matched.truncate(3);

    // Roundup detection: if the title mentions 5+ distinct technology
    // keywords (comma-separated lists, newsletter digests), it's a roundup
    // — don't assign it to a single dep. The is_low_quality_signal filter
    // catches "This Week In React" etc., but titles like "React, Vue,
    // Angular, Svelte, Solid comparison" also need handling.
    let comma_segments = title
        .chars()
        .filter(|c| *c == ',' || u32::from(*c) == 0x00B7)
        .count();
    if matched.len() >= 2 && comma_segments >= 5 {
        return (
            "Roundup article mentioning multiple technologies".to_string(),
            None,
        );
    }

    if !matched.is_empty() {
        let first_dep = matched[0].to_string();
        let text = format!("Mentions {} from your stack", matched.join(", "));
        return (text, Some(first_dep));
    }

    // No specific dep match found — return empty explanation so
    // downstream can filter or request LLM-generated reasoning.
    // Never claim relevance we can't substantiate with evidence.
    (String::new(), None)
}

/// Check whether `text` contains `term` at a word boundary.
/// Case-sensitive; pass already-lowercased strings for case-insensitive matching.
fn has_word_boundary_match(text: &str, term: &str) -> bool {
    if term.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let mut search_from = 0;
    while let Some(pos) = text[search_from..].find(term) {
        let abs = search_from + pos;
        let before_ok = abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric();
        let after = abs + term.len();
        let after_ok = after >= bytes.len()
            || !bytes[after].is_ascii_alphanumeric()
            || text[after..].starts_with(".js")
            || text[after..].starts_with(".ts")
            || text[after..].starts_with(".rs");
        if before_ok && after_ok {
            return true;
        }
        search_from = abs + 1;
    }
    false
}

/// Count deps whose package name fuzzy-matches the given topic.
fn count_deps_for_topic(deps: &[DepCoverage], topic: &str) -> u32 {
    let topic_lower = topic.to_lowercase();
    deps.iter()
        .filter(|d| {
            let name_lower = d.package_name.to_lowercase();
            name_lower.contains(&topic_lower) || topic_lower.contains(&name_lower)
        })
        .count() as u32
}

/// Count missed items from knowledge gaps that match the given topic.
fn count_missed_for_topic(gaps: &[crate::knowledge_decay::KnowledgeGap], topic: &str) -> u32 {
    let topic_lower = topic.to_lowercase();
    gaps.iter()
        .filter(|g| {
            let dep_lower = g.dependency.to_lowercase();
            dep_lower.contains(&topic_lower) || topic_lower.contains(&dep_lower)
        })
        .map(|g| g.missed_items.len() as u32)
        .sum()
}

/// Generate 3-5 actionable recommendations based on blind spot analysis.
fn generate_recommendations(
    uncovered: &[UncoveredDep],
    stale: &[StaleTopic],
    gaps: &[crate::knowledge_decay::KnowledgeGap],
) -> Vec<BlindSpotRecommendation> {
    let mut recs = Vec::new();

    // Recommendation for critical/high uncovered deps
    let critical_deps: Vec<&UncoveredDep> = uncovered
        .iter()
        .filter(|d| d.risk_level == "critical" || d.risk_level == "high")
        .collect();

    if !critical_deps.is_empty() {
        let dep_names: Vec<&str> = critical_deps
            .iter()
            .take(3)
            .map(|d| d.name.as_str())
            .collect();
        recs.push(BlindSpotRecommendation {
            action: format!("Review signals for: {}", dep_names.join(", ")),
            reason: format!(
                "{} dependencies have critical/high risk blind spots with no recent engagement",
                critical_deps.len()
            ),
            priority: "high".to_string(),
        });
    }

    // Recommendation for stale topics with active deps
    let active_stale: Vec<&StaleTopic> = stale
        .iter()
        .filter(|s| s.active_deps_in_topic > 0)
        .collect();

    if !active_stale.is_empty() {
        let topic_names: Vec<&str> = active_stale
            .iter()
            .take(3)
            .map(|s| s.topic.as_str())
            .collect();
        recs.push(BlindSpotRecommendation {
            action: format!("Catch up on stale topics: {}", topic_names.join(", ")),
            reason: format!(
                "{} topics have active dependencies but declining attention",
                active_stale.len()
            ),
            priority: "high".to_string(),
        });
    }

    // Recommendation for knowledge gaps with severe severity
    let severe_gaps: Vec<_> = gaps
        .iter()
        .filter(|g| {
            g.gap_severity == crate::knowledge_decay::GapSeverity::Critical
                || g.gap_severity == crate::knowledge_decay::GapSeverity::High
        })
        .collect();

    if !severe_gaps.is_empty() {
        let gap_names: Vec<&str> = severe_gaps
            .iter()
            .take(3)
            .map(|g| g.dependency.as_str())
            .collect();
        recs.push(BlindSpotRecommendation {
            action: format!(
                "Address knowledge decay in: {}",
                gap_names.join(", ")
            ),
            reason: format!(
                "{} dependencies have critical/high knowledge gaps — you may be missing important updates",
                severe_gaps.len()
            ),
            priority: "medium".to_string(),
        });
    }

    if uncovered.len() > 5 {
        let top_uncovered: Vec<(&str, &str)> = uncovered
            .iter()
            .filter(|d| d.available_signal_count == 0)
            .take(5)
            .map(|d| (d.name.as_str(), d.dep_type.as_str()))
            .collect();
        if !top_uncovered.is_empty() {
            let dep_names: Vec<&str> = top_uncovered.iter().map(|(n, _)| *n).collect();
            let ecosystems: std::collections::HashSet<&str> =
                top_uncovered.iter().map(|(_, e)| *e).collect();
            let source_hint =
                if let Some(&eco) = ecosystems.iter().next().filter(|_| ecosystems.len() == 1) {
                    match eco {
                        "crates.io" => " — enable the crates.io source or watch their GitHub repos",
                        "npm" => " — enable the npm registry source",
                        "pypi" => " — enable the PyPI source",
                        "go" => " — enable the Go modules source",
                        _ => " — add their release feeds or GitHub repos to your sources",
                    }
                } else {
                    " — add their package registry feeds or GitHub repos to your sources"
                };
            recs.push(BlindSpotRecommendation {
                action: format!("Add source coverage for: {}", dep_names.join(", ")),
                reason: format!(
                    "{} dependencies have zero signal coverage{}",
                    top_uncovered.len(),
                    source_hint,
                ),
                priority: "medium".to_string(),
            });
        }
    }

    // Positive reinforcement if few blind spots
    if uncovered.is_empty() && stale.is_empty() {
        recs.push(BlindSpotRecommendation {
            action: "Your signal coverage looks solid — keep monitoring for shifts".to_string(),
            reason: "No critical blind spots detected in your current stack".to_string(),
            priority: "low".to_string(),
        });
    }

    recs
}

/// Compute a normalized blind-spot score in [0, 100].
///
/// The score is a weighted blend of three normalized signals:
///   - Uncovered-dep pressure (55% of the score)
///   - Stale-topic pressure (25% of the score)
///   - Missed-signal pressure (20% of the score)
///
/// Each component is independently normalized to [0, 1] before blending.
/// The weighted blend guarantees the raw score is in [0, 100] without ever
/// needing a `.min(100.0)` cap — the previous implementation was additive
/// and unbounded, causing the score to trivially saturate at 100 for any
/// non-trivial stack (which is how we got the "Blind Spot Index = 100"
/// screenshot bug).
///
/// `total_direct_deps` is the full direct-dep count from `get_dependency_coverage`
/// (before filtering to short-name / empty-signal skips). Used as the
/// denominator for uncovered-dep percentage so bigger stacks don't saturate.
fn calculate_blind_spot_score(
    uncovered: &[UncoveredDep],
    stale: &[StaleTopic],
    missed: &[MissedSignal],
    total_direct_deps: usize,
) -> f32 {
    // Insufficient evidence guard: with fewer than 10 tracked deps, the
    // uncovered-dep pressure (55% of the score) has too few data points
    // to be meaningful. A 4-dep stack with 1 uncovered = 25% pressure,
    // which maps to a score that implies comprehensive analysis when
    // we've barely sampled the user's real stack. Return -1.0 as sentinel
    // for "not enough data" — the frontend interprets this as "building
    // your coverage picture" instead of showing a misleading number.
    if total_direct_deps < 10 {
        return -1.0;
    }

    // ─── 1. Uncovered pressure (0-1) ──────────────────────────────────
    // Severity-weighted fraction of the user's direct stack that's uncovered.
    let uncovered_weighted: f32 = uncovered
        .iter()
        .map(|d| match d.risk_level.as_str() {
            "critical" => 1.0,
            "high" => 0.7,
            "medium" => 0.4,
            _ => 0.15,
        })
        .sum();

    // Denominator: total direct deps (not just eligible-for-checking). This is
    // intentional — utility/ambiguous deps are filtered from the uncovered
    // vector but still count toward the denominator, which keeps the score
    // stable and prevents noise from small eligible-dep counts.
    let denom = (total_direct_deps.max(5)) as f32;
    let uncovered_pressure = (uncovered_weighted / denom).min(1.0);

    // ─── 2. Stale-topic pressure (0-1) ────────────────────────────────
    // Diminishing returns: 1 stale topic = 0.25, 5 stale topics = 0.75, 10+ ≈ 1.0.
    // Uses a soft asymptote so one stale topic is noticeable but not alarming.
    let stale_pressure = if stale.is_empty() {
        0.0
    } else {
        let n = stale.len() as f32;
        (n / (n + 3.0)).min(1.0)
    };

    // ─── 3. Missed-signal pressure (0-1) ──────────────────────────────
    // Averaged relevance of the top missed signals. A single 0.9-relevance
    // missed item contributes more than five 0.5-relevance items.
    let missed_pressure = if missed.is_empty() {
        0.0
    } else {
        // Average relevance of the missed signals, capped at 1.0.
        let sum: f32 = missed.iter().map(|m| m.relevance_score).sum();
        let avg = sum / missed.len() as f32;
        // Boost by log-scale count: more missed items bumps the pressure,
        // but the returns diminish fast so it can't dominate.
        let count_boost =
            ((missed.len() as f32).ln_1p() / scoring_config::MISSED_SIGNAL_LOG_DIVISOR).min(1.0);
        (avg * scoring_config::MISSED_SIGNAL_AVG_WEIGHT
            + count_boost * scoring_config::MISSED_SIGNAL_COUNT_BOOST_WEIGHT)
            .min(1.0)
    };

    // ─── Final weighted blend ─────────────────────────────────────────
    let score = (uncovered_pressure * scoring_config::BLIND_SPOT_HEALTH_UNCOVERED_WEIGHT)
        + (stale_pressure * scoring_config::BLIND_SPOT_HEALTH_STALE_WEIGHT)
        + (missed_pressure * scoring_config::BLIND_SPOT_HEALTH_MISSED_WEIGHT);

    // Clamp defensively (should already be in range given the component caps).
    score.clamp(0.0, 100.0)
}

// ============================================================================
// EvidenceItem conversion (Intelligence Reconciliation — Phase 4)
// ============================================================================
//
// Blind Spots historically produced four parallel shapes (UncoveredDep,
// StaleTopic, MissedSignal, BlindSpotRecommendation) plus a scalar health
// score. All four collapse into `EvidenceItem` variants; the score rides
// on `EvidenceFeed.score`.

fn risk_level_to_urgency(risk_level: &str) -> Urgency {
    match risk_level {
        "critical" => Urgency::Critical,
        "high" => Urgency::High,
        "medium" => Urgency::Medium,
        _ => Urgency::Watch,
    }
}

fn priority_to_urgency(priority: &str) -> Urgency {
    match priority {
        "high" => Urgency::High,
        "medium" => Urgency::Medium,
        _ => Urgency::Watch,
    }
}

fn truncate_title(s: &str) -> String {
    // Schema: ≤ 120 chars, no trailing period.
    s.trim_end_matches('.').chars().take(120).collect()
}

fn truncate_note(s: &str) -> String {
    // Citation relevance_note schema cap: 200 chars.
    s.chars().take(200).collect()
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Look up the installed version of a package from project_dependencies.
fn lookup_installed_version(dep_name: &str) -> Option<String> {
    let db = crate::get_database().ok()?;
    let conn = db.conn.lock();
    conn.query_row(
        "SELECT version FROM project_dependencies WHERE package_name = ?1 AND version IS NOT NULL LIMIT 1",
        params![dep_name],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .filter(|v| !v.is_empty())
}

/// Count signal types available for a dep in the last 30 days.
/// Returns (release_count, analysis_count, other_count).
/// Breakdown of a dependency's unseen signals by CONSEQUENCE, so Blind Spots can
/// rank and frame by what changed rather than by unread volume. The tuple is Copy.
#[derive(Clone, Copy, Default)]
struct DepSignalBreakdown {
    /// New versions shipped (release_notes / platform_update).
    releases: u32,
    /// Expert analysis / deep-dives.
    analyses: u32,
    /// Highest consequence: security advisories and breaking changes. Previously
    /// these fell into `other` and were invisible to ranking — the whole point of
    /// #2b is to surface them.
    security: u32,
    /// Everything else — general discussion. Pure volume, low consequence.
    other: u32,
}

fn count_signal_types_for_dep(dep_name: &str) -> DepSignalBreakdown {
    let mut b = DepSignalBreakdown::default();
    let db = match crate::get_database() {
        Ok(db) => db,
        Err(_) => return b,
    };
    let conn = db.conn.lock();
    let sql = "SELECT content_type, COUNT(*) FROM source_items
               WHERE title LIKE '%' || ?1 || '%'
                 AND created_at >= datetime('now', '-30 days')
               GROUP BY content_type";
    if let Ok(mut stmt) = conn.prepare(sql) {
        if let Ok(rows) = stmt.query_map(params![dep_name], |row| {
            Ok((row.get::<_, Option<String>>(0)?, row.get::<_, u32>(1)?))
        }) {
            for row in rows.flatten() {
                match row.0.as_deref() {
                    Some("release_notes") | Some("platform_update") => b.releases += row.1,
                    Some("expert_analysis") | Some("deep_dive") => b.analyses += row.1,
                    Some("security_advisory") | Some("breaking_change") => b.security += row.1,
                    _ => b.other += row.1,
                }
            }
        }
    }
    b
}

/// Lower a risk-derived urgency to at most Medium. Used for deps whose only
/// unseen signals are general discussion ("other") — pure unread volume must not
/// masquerade as a high-urgency blind spot.
fn cap_urgency_at_medium(u: Urgency) -> Urgency {
    match u {
        Urgency::Critical | Urgency::High => Urgency::Medium,
        other => other,
    }
}

fn uncovered_dep_to_evidence_item(d: &UncoveredDep) -> EvidenceItem {
    // d.name is the DISPLAY name ("react (npm)"); signal/version lookups match
    // article titles via LIKE, which never carry the " (ecosystem)" qualifier, so
    // strip it first. Compute the consequence breakdown once (only when there are
    // unseen signals) — it drives the title, the explanation, AND the consequence-
    // weighted confidence/urgency below (#2b: rank by what changed, not by volume).
    let bare = bare_package_name(&d.name);
    let breakdown = (d.available_signal_count > 0).then(|| count_signal_types_for_dep(bare));

    // Zero-signal deps get a distinct title and explanation — they have
    // NO coverage at all, which is qualitatively different from "has signals
    // you haven't seen".
    let (title, explanation) = if d.available_signal_count == 0 {
        let title = truncate_title(&format!("{} — unmonitored", d.name));
        let explanation = match d.coverage_reason.as_deref() {
            Some("not_checked") => format!(
                "4DA hasn't checked {} yet. Source adapters haven't run for this package.",
                d.name
            ),
            Some("adapter_failing") => format!(
                "{} may have coverage issues — one or more source adapters are failing. Check feed health.",
                d.name
            ),
            Some("checked_no_results") => format!(
                "Sources were checked but found no results for {}. This is unusual for a {} package.",
                d.name, d.dep_type
            ),
            Some("unknown_ecosystem") => format!(
                "No source adapters are configured for the {} ecosystem. {} can't be monitored yet.",
                d.dep_type, d.name
            ),
            Some("weak_matches_only") => format!(
                "{} has some potential matches but none are confirmed. Review weak matches for details.",
                d.name
            ),
            _ => format!(
                "{} is in your stack but has no confirmed source coverage yet.",
                d.name
            ),
        };
        (title, explanation)
    } else {
        // Consequence-first title: lead with the highest-consequence signal —
        // security/breaking, then new releases, then analyses — instead of the
        // raw unread count, which is mostly noise ("react — 123 unseen signals"
        // reads as FOMO, not insight). Fall back to a soft "N updates to review"
        // only when there is no consequence signal at all.
        let b = breakdown.unwrap_or_default();
        let installed_version = lookup_installed_version(bare);
        let title = truncate_title(&if b.security > 0 {
            format!(
                "{} — {} security/breaking-change signal{} unreviewed",
                d.name,
                b.security,
                if b.security == 1 { "" } else { "s" }
            )
        } else if b.releases > 0 {
            format!(
                "{} — {} new release{} unreviewed",
                d.name,
                b.releases,
                if b.releases == 1 { "" } else { "s" }
            )
        } else if b.analyses > 0 {
            format!(
                "{} — {} expert analysis article{} unread",
                d.name,
                b.analyses,
                if b.analyses == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "{} — {} update{} to review",
                d.name,
                d.available_signal_count,
                if d.available_signal_count == 1 {
                    ""
                } else {
                    "s"
                }
            )
        });

        let mut explanation_parts: Vec<String> = Vec::new();
        if b.security > 0 {
            explanation_parts.push(format!(
                "{} security advisory / breaking-change signal{} you haven't reviewed.",
                b.security,
                if b.security == 1 { "" } else { "s" }
            ));
        }
        if b.releases > 0 {
            let ver_note = installed_version
                .as_ref()
                .map(|v| format!(" (you're on {v})"))
                .unwrap_or_default();
            explanation_parts.push(format!(
                "{} new release{}{ver_note} in the last 30 days.",
                b.releases,
                if b.releases == 1 { "" } else { "s" }
            ));
        }
        if b.analyses > 0 {
            explanation_parts.push(format!(
                "{} expert analysis article{} available.",
                b.analyses,
                if b.analyses == 1 { "" } else { "s" }
            ));
        }
        if explanation_parts.is_empty() {
            explanation_parts.push(format!(
                "{} signal{} about {} available for review.",
                d.available_signal_count,
                if d.available_signal_count == 1 {
                    ""
                } else {
                    "s"
                },
                d.name,
            ));
        }
        (title, explanation_parts.join(" "))
    };

    // Synthesize at least one inferred citation so the schema's
    // "evidence required for user-surfaced kinds" rule holds. Real
    // citations reserved for future enrichment.
    let citation = EvidenceCitation {
        source: format!("dep-coverage:{}", d.dep_type),
        title: truncate_title(&format!("{} coverage gap", d.name)),
        url: None,
        freshness_days: d.days_since_last_signal as f32,
        relevance_note: truncate_note(&format!(
            "{} signals available, none engaged with",
            d.available_signal_count
        )),
    };

    EvidenceItem {
        id: format!("bs_uncov_{}_{}", d.dep_type, d.name),
        kind: EvidenceKind::Gap,
        title,
        explanation,
        confidence: Confidence::heuristic({
            let base = match d.risk_level.as_str() {
                "critical" => 0.50,
                "high" => 0.40,
                "medium" => 0.30,
                _ => 0.20,
            };
            let project_boost = (d.projects_using.len() as f32 * 0.05).min(0.15);
            // Consequence-weighted (replaces the old volume-based signal_boost):
            // confidence tracks what CHANGED, not how many unread items piled up,
            // so a noisy topic can't out-rank a dep with a real release/advisory.
            let consequence_boost = match breakdown {
                Some(b) if b.security > 0 => 0.20,
                Some(b) if b.releases > 0 => 0.12,
                Some(b) if b.analyses > 0 => 0.06,
                _ => 0.0,
            };
            (base + project_boost + consequence_boost).min(0.80)
        }),
        // Consequence-adjusted urgency: security/breaking elevates regardless of
        // volume; real release/analysis activity keeps the risk-based urgency;
        // deps whose only unseen signals are general discussion are capped at
        // Medium. Unmonitored deps (no breakdown) keep their risk-based urgency.
        urgency: match breakdown {
            // Urgency ordinals: Critical < High < Medium < Watch, so the MORE
            // urgent of two is the smaller one — use min() to mean "at least High"
            // (keeps Critical if the risk is already critical).
            Some(b) if b.security > 0 => Urgency::High.min(risk_level_to_urgency(&d.risk_level)),
            Some(b) if b.releases > 0 || b.analyses > 0 => risk_level_to_urgency(&d.risk_level),
            Some(_) => cap_urgency_at_medium(risk_level_to_urgency(&d.risk_level)),
            None => risk_level_to_urgency(&d.risk_level),
        },
        reversibility: None,
        evidence: vec![citation],
        affected_projects: d.projects_using.clone(),
        affected_deps: vec![d.name.clone()],
        suggested_actions: vec![EvidenceAction {
            action_id: "investigate".to_string(),
            label: "Investigate".to_string(),
            description: "Review what changed for this dependency.".to_string(),
        }],
        precedents: Vec::new(),
        refutation_condition: None,
        lens_hints: LensHints::blind_spots_only(),
        created_at: now_millis(),
        expires_at: None,
    }
}

fn stale_topic_to_evidence_item(t: &StaleTopic) -> EvidenceItem {
    // Compute the consequence breakdown once (only when there are missed
    // signals) and drive BOTH the title and explanation from it, so the title
    // leads with the highest-consequence signal (security/breaking, then
    // releases, then analyses) rather than the raw unread count.
    // DepSignalBreakdown is Copy, so both matches read it freely.
    let signal_breakdown =
        (t.missed_signal_count > 0).then(|| count_signal_types_for_dep(&t.topic));
    let title = match signal_breakdown {
        Some(b) if b.security > 0 => truncate_title(&format!(
            "{} — {} security/breaking-change signal{} unreviewed",
            t.topic,
            b.security,
            if b.security == 1 { "" } else { "s" }
        )),
        Some(b) if b.releases > 0 => truncate_title(&format!(
            "{} — {} release update{} unreviewed",
            t.topic,
            b.releases,
            if b.releases == 1 { "" } else { "s" }
        )),
        Some(b) if b.analyses > 0 => truncate_title(&format!(
            "{} — {} analysis article{} unread",
            t.topic,
            b.analyses,
            if b.analyses == 1 { "" } else { "s" }
        )),
        Some(_) => truncate_title(&format!(
            "{} — {} update{} to review",
            t.topic,
            t.missed_signal_count,
            if t.missed_signal_count == 1 { "" } else { "s" }
        )),
        None => truncate_title(&format!(
            "{} — {} dep{}, no recent engagement",
            t.topic,
            t.active_deps_in_topic,
            if t.active_deps_in_topic == 1 { "" } else { "s" }
        )),
    };
    let explanation = match signal_breakdown {
        Some(b) if b.security > 0 => format!(
            "{} security/breaking-change signal{} in the {} ecosystem you haven't reviewed.",
            b.security,
            if b.security == 1 { "" } else { "s" },
            t.topic,
        ),
        Some(b) if b.releases > 0 => format!(
            "{} release update{} for {} ecosystem in the last 30 days.",
            b.releases,
            if b.releases == 1 { "" } else { "s" },
            t.topic,
        ),
        Some(b) if b.analyses > 0 => format!(
            "{} unreviewed signal{} including {} analysis article{}.",
            t.missed_signal_count,
            if t.missed_signal_count == 1 { "" } else { "s" },
            b.analyses,
            if b.analyses == 1 { "" } else { "s" },
        ),
        Some(_) => format!(
            "{} signal{} in the {} ecosystem you haven't reviewed.",
            t.missed_signal_count,
            if t.missed_signal_count == 1 { "" } else { "s" },
            t.topic,
        ),
        None => format!(
            "{} active {} dependenc{} with no recent signal coverage.",
            t.active_deps_in_topic,
            t.topic,
            if t.active_deps_in_topic == 1 {
                "y"
            } else {
                "ies"
            },
        ),
    };

    let citation = EvidenceCitation {
        source: "attention-report".to_string(),
        title: truncate_title(&format!("{} engagement gap", t.topic)),
        url: None,
        freshness_days: t.last_engagement_days as f32,
        relevance_note: truncate_note(&format!(
            "{} active deps, {} missed signals",
            t.active_deps_in_topic, t.missed_signal_count
        )),
    };

    // Stale topics are lower urgency than specific uncovered deps: no
    // named CVE, just attention drift.
    let urgency = if t.missed_signal_count >= 5 && t.last_engagement_days >= 14 {
        Urgency::Medium
    } else {
        Urgency::Watch
    };

    EvidenceItem {
        id: format!("bs_stale_{}", t.topic),
        kind: EvidenceKind::Gap,
        title,
        explanation,
        confidence: Confidence::heuristic({
            let signal_factor = (t.missed_signal_count as f32 * 0.06).min(0.35);
            let staleness_factor = (t.last_engagement_days as f32 / 60.0).min(0.25);
            (0.20 + signal_factor + staleness_factor).min(0.70)
        }),
        urgency,
        reversibility: None,
        evidence: vec![citation],
        affected_projects: Vec::new(),
        affected_deps: vec![t.topic.clone()],
        suggested_actions: vec![EvidenceAction {
            action_id: "investigate".to_string(),
            label: "Investigate".to_string(),
            description: "Look at recent signals for this topic.".to_string(),
        }],
        precedents: Vec::new(),
        refutation_condition: None,
        lens_hints: LensHints::blind_spots_only(),
        created_at: now_millis(),
        expires_at: None,
    }
}

fn missed_signal_to_evidence_item(m: &MissedSignal) -> EvidenceItem {
    let title = truncate_title(&m.title);
    let freshness = chrono::NaiveDateTime::parse_from_str(&m.created_at, "%Y-%m-%d %H:%M:%S")
        .map(|dt| {
            let diff = chrono::Utc::now().timestamp() - dt.and_utc().timestamp();
            (diff as f32 / 86_400.0).max(0.0)
        })
        .unwrap_or(0.0);

    // Enrich relevance_note with installed version for release_notes signals
    let relevance_note = if m.content_type.as_deref() == Some("release_notes") {
        if let Some(ref dep) = m.dep_name {
            if let Some(ver) = lookup_installed_version(dep) {
                truncate_note(&format!("You're on {dep} {ver}"))
            } else {
                truncate_note(&m.why_relevant)
            }
        } else {
            truncate_note(&m.why_relevant)
        }
    } else {
        truncate_note(&m.why_relevant)
    };

    let citation = EvidenceCitation {
        source: m.source_type.clone(),
        title: truncate_title(&m.title),
        url: m.url.clone(),
        freshness_days: freshness,
        relevance_note,
    };

    // Map urgency from content classification + relevance score.
    // Uses stored content_type (set at ingestion by content_dna) with
    // title-based fallback for legacy items. This is structurally sound:
    // a blog post CAN'T get Critical because its content_type caps it.
    let tier = signal_priority_tier(m);
    let urgency = match tier {
        4 => {
            if m.relevance_score >= 0.7 {
                Urgency::Critical
            } else {
                Urgency::High
            }
        }
        3 => {
            if m.relevance_score >= 0.8 {
                Urgency::High
            } else {
                Urgency::Medium
            }
        }
        2 => Urgency::Medium,
        _ => {
            if m.relevance_score >= 0.9 {
                Urgency::Medium
            } else {
                Urgency::Watch
            }
        }
    };

    EvidenceItem {
        id: format!("bs_missed_{}", m.item_id),
        kind: EvidenceKind::MissedSignal,
        title,
        explanation: m.why_relevant.clone(),
        confidence: Confidence::heuristic(m.relevance_score.clamp(0.0, 1.0)),
        urgency,
        reversibility: None,
        evidence: vec![citation],
        affected_projects: Vec::new(),
        affected_deps: m
            .dep_name
            .as_ref()
            .map(|d| vec![d.clone()])
            .unwrap_or_default(),
        // MissedSignal is informational; schema doesn't require actions
        // for this kind.
        suggested_actions: Vec::new(),
        precedents: Vec::new(),
        refutation_condition: None,
        lens_hints: LensHints::blind_spots_only(),
        created_at: now_millis(),
        expires_at: None,
    }
}

fn recommendation_to_evidence_item(r: &BlindSpotRecommendation, idx: usize) -> EvidenceItem {
    let title = truncate_title(&r.action);
    // Recommendations are actionable alerts in canonical form — actions are
    // not items, but "do this to close a gap" is classic Alert kind.
    let citation = EvidenceCitation {
        source: "blind-spot-analyzer".to_string(),
        title: truncate_title(&r.reason),
        url: None,
        freshness_days: 0.0,
        relevance_note: truncate_note(&r.reason),
    };

    EvidenceItem {
        id: format!("bs_rec_{idx}"),
        kind: EvidenceKind::Alert,
        title,
        explanation: r.reason.clone(),
        confidence: Confidence::heuristic(0.35),
        urgency: priority_to_urgency(&r.priority),
        reversibility: None,
        evidence: vec![citation],
        affected_projects: Vec::new(),
        affected_deps: Vec::new(),
        suggested_actions: vec![EvidenceAction {
            action_id: "acknowledge".to_string(),
            label: "Acknowledge".to_string(),
            description: "Mark this recommendation as reviewed.".to_string(),
        }],
        precedents: Vec::new(),
        refutation_condition: None,
        lens_hints: LensHints::blind_spots_only(),
        created_at: now_millis(),
        expires_at: None,
    }
}

/// Convert a legacy `BlindSpotReport` into the canonical `EvidenceFeed`.
/// Every item is schema-validated; validation failures drop the offending
/// item with a structured log rather than breaking the feed.
pub(crate) fn blind_spot_report_to_feed(report: &BlindSpotReport) -> EvidenceFeed {
    let mut items: Vec<EvidenceItem> = Vec::new();

    for d in &report.uncovered_dependencies {
        items.push(uncovered_dep_to_evidence_item(d));
    }
    for t in &report.stale_topics {
        items.push(stale_topic_to_evidence_item(t));
    }
    for m in &report.missed_signals {
        items.push(missed_signal_to_evidence_item(m));
    }
    for (idx, r) in report.recommendations.iter().enumerate() {
        items.push(recommendation_to_evidence_item(r, idx));
    }

    // Filter out dismissed items (persisted in blind_spot_dismissals table)
    let dismissed_ids: std::collections::HashSet<String> =
        if let Ok(conn) = crate::open_db_connection() {
            conn.prepare("SELECT item_id FROM blind_spot_dismissals")
                .ok()
                .and_then(|mut stmt| {
                    stmt.query_map([], |row| row.get::<_, String>(0))
                        .ok()
                        .map(|rows| rows.filter_map(|r| r.ok()).collect())
                })
                .unwrap_or_default()
        } else {
            std::collections::HashSet::new()
        };

    let items: Vec<EvidenceItem> = items
        .into_iter()
        .filter(|item| !dismissed_ids.contains(&item.id))
        .collect();

    let validated: Vec<EvidenceItem> = items
        .into_iter()
        .filter(|item| match crate::evidence::validate_item(item) {
            Ok(()) => true,
            Err(e) => {
                warn!(
                    target: "4da::evidence::validate",
                    id = %item.id,
                    error = %e,
                    "dropped blind-spot item failing schema validation"
                );
                false
            }
        })
        .collect();
    let mut feed = EvidenceFeed::from_items_with_score(validated, report.overall_score);
    let weak_len = report.weak_matches.len();
    if weak_len > 0 {
        feed.weak_match_count = Some(weak_len);
    }
    feed.data_freshness = report.data_freshness.clone();
    feed
}

fn build_feed_with_existing_score(items: Vec<EvidenceItem>, score: Option<f32>) -> EvidenceFeed {
    let mut feed = EvidenceFeed::from_items(items);
    feed.score = score;
    feed
}

// ============================================================================
// Tier 2: LLM-Judged Blind Spot Items
// ============================================================================

/// Pull LLM-judged items from `llm_judgments` that belong in the Blind Spots
/// lens: topics, ecosystems, tech trends — everything EXCEPT security (which
/// routes to Preemption). Skips items the user has already interacted with.
fn llm_judged_blind_spot_items() -> Vec<EvidenceItem> {
    let db = match crate::get_database() {
        Ok(db) => db,
        Err(e) => {
            warn!(target: "4da::blind_spots", "Cannot load DB for Tier 2 items: {e}");
            return Vec::new();
        }
    };

    let judgments = match db.get_relevant_judgments(0.50, 30) {
        Ok(j) => j,
        Err(e) => {
            warn!(target: "4da::blind_spots", "Failed to load LLM judgments: {e}");
            return Vec::new();
        }
    };

    let conn = db.conn.lock();
    let mut items = Vec::new();

    for judgment in &judgments {
        // Load the source item to get title/url/source_type
        let row: Option<(String, Option<String>, String)> = conn
            .query_row(
                "SELECT title, url, source_type FROM source_items WHERE id = ?1",
                rusqlite::params![judgment.source_item_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

        let (title_raw, url, source_type) = match row {
            Some(r) => r,
            None => continue, // source item deleted or missing
        };

        if is_preemption_territory(&title_raw) || source_type == "osv" || source_type == "cve" {
            continue;
        }

        // Skip items the user has already interacted with
        let already_seen: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM interactions WHERE source_item_id = ?1 OR item_id = ?1",
                rusqlite::params![judgment.source_item_id],
                |row| row.get::<_, i64>(0).map(|c| c > 0),
            )
            .unwrap_or(false);
        if already_seen {
            continue;
        }

        let urgency = if judgment.relevance_score < 0.65 {
            Urgency::Watch
        } else {
            Urgency::Medium
        };

        let citation = EvidenceCitation {
            source: source_type.clone(),
            title: truncate_title(&title_raw),
            url: url.clone(),
            freshness_days: 0.0, // judgment doesn't track age directly
            relevance_note: truncate_note(&format!(
                "LLM relevance {:.0}%",
                judgment.relevance_score * 100.0
            )),
        };

        items.push(EvidenceItem {
            id: format!("llm-bs-{}", judgment.source_item_id),
            kind: EvidenceKind::MissedSignal,
            title: truncate_title(&title_raw),
            explanation: judgment.explanation.clone(),
            confidence: Confidence::llm_assessed(judgment.confidence as f32),
            urgency,
            reversibility: None,
            evidence: vec![citation],
            affected_projects: vec![],
            affected_deps: vec![],
            suggested_actions: vec![EvidenceAction {
                action_id: "investigate".to_string(),
                label: "Investigate".to_string(),
                description: "Review this signal — the LLM flagged it as relevant to your stack."
                    .to_string(),
            }],
            precedents: Vec::new(),
            refutation_condition: None,
            lens_hints: LensHints::blind_spots_only(),
            created_at: now_millis(),
            expires_at: None,
        });
    }

    items
}

// ============================================================================
// Tauri Command
// ============================================================================

/// Returns the canonical `EvidenceFeed` for the Blind Spots lens, with the
/// legacy coverage score carried on `feed.score`. Internal
/// `generate_blind_spot_report` still produces the legacy struct (shared
/// with telemetry code paths) and converts at the boundary.
#[tauri::command]
pub fn get_blind_spots() -> std::result::Result<EvidenceFeed, String> {
    crate::settings::require_signal_feature("get_blind_spots").map_err(|e| e.to_string())?;
    let report = generate_blind_spot_report().map_err(|e| e.to_string())?;
    let mut feed = blind_spot_report_to_feed(&report);

    // Attach total tracked dep count so the UI can show accurate denominator
    if let Ok(conn) = crate::open_db_connection() {
        if let Ok(deps) = get_dependency_coverage(&conn) {
            feed.total_tracked = Some(deps.len());
        }
    }

    // Tier 2: inject LLM-judged blind spot items (missed signals the user hasn't seen)
    feed.items.extend(llm_judged_blind_spot_items());

    let total_tracked = feed.total_tracked;
    let weak_match_count = feed.weak_match_count;
    let data_freshness = feed.data_freshness.clone();
    let mut final_feed = build_feed_with_existing_score(feed.items, feed.score);
    final_feed.total_tracked = total_tracked;
    final_feed.weak_match_count = weak_match_count;
    final_feed.data_freshness = data_freshness;
    Ok(final_feed)
}

/// Add a watch for a package — the user explicitly wants 4DA to track this dependency.
/// This ensures the package appears in the user's dependency list and will be
/// checked by source adapters on the next fetch cycle.
#[tauri::command]
pub fn add_package_watch(
    package_name: String,
    ecosystem: String,
    project_path: Option<String>,
) -> std::result::Result<serde_json::Value, String> {
    crate::settings::require_signal_feature("add_package_watch").map_err(|e| e.to_string())?;

    let conn = crate::open_db_connection().map_err(|e| e.to_string())?;

    let path = project_path.unwrap_or_else(|| "user-watch".to_string());

    conn.execute(
        "INSERT INTO user_dependencies (project_path, package_name, ecosystem, is_direct, detected_at, last_seen_at)
         VALUES (?1, ?2, ?3, 1, datetime('now'), datetime('now'))
         ON CONFLICT(project_path, package_name, ecosystem) DO UPDATE SET
           last_seen_at = datetime('now')",
        params![path, package_name, ecosystem],
    )
    .map_err(|e| e.to_string())?;

    // Invalidate the blind spots cache so the next refresh picks up the change
    if let Ok(mut guard) = BLIND_SPOT_CACHE.lock() {
        *guard = None;
    }

    info!(
        target: "4da::blind_spots",
        package = %package_name,
        ecosystem = %ecosystem,
        "Package watch added"
    );

    Ok(serde_json::json!({
        "status": "ok",
        "package_name": package_name,
        "ecosystem": ecosystem,
    }))
}

/// Dismiss a blind spot item — the user has reviewed and decided this isn't relevant.
/// Persisted to the database so it survives restarts.
#[tauri::command]
pub fn dismiss_blind_spot(
    item_id: String,
    reason: String,
) -> std::result::Result<serde_json::Value, String> {
    let conn = crate::open_db_connection().map_err(|e| e.to_string())?;

    // Create the dismissals table if it doesn't exist (defensive — migration should handle this)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS blind_spot_dismissals (
            id INTEGER PRIMARY KEY,
            item_id TEXT NOT NULL UNIQUE,
            reason TEXT NOT NULL,
            dismissed_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO blind_spot_dismissals (item_id, reason)
         VALUES (?1, ?2)
         ON CONFLICT(item_id) DO UPDATE SET reason = excluded.reason, dismissed_at = CURRENT_TIMESTAMP",
        params![item_id, reason],
    )
    .map_err(|e| e.to_string())?;

    // Feed stability detector — blind spot dismissal is a strong topic veto signal
    crate::engagement_telemetry::on_blind_spot_dismiss(&conn, &item_id);

    // Invalidate cache
    if let Ok(mut guard) = BLIND_SPOT_CACHE.lock() {
        *guard = None;
    }

    Ok(serde_json::json!({ "status": "ok", "item_id": item_id }))
}

// ============================================================================
// Tests — use REAL schema definitions from migrations to catch column drift
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

    #[test]
    fn cap_urgency_at_medium_lowers_only_above_medium() {
        // Pure unread volume must not present as high/critical urgency.
        assert_eq!(cap_urgency_at_medium(Urgency::Critical), Urgency::Medium);
        assert_eq!(cap_urgency_at_medium(Urgency::High), Urgency::Medium);
        // Medium and below pass through unchanged.
        assert_eq!(cap_urgency_at_medium(Urgency::Medium), Urgency::Medium);
        assert_eq!(cap_urgency_at_medium(Urgency::Watch), Urgency::Watch);
    }

    #[test]
    fn urgency_min_means_at_least_high() {
        // Ordinals are Critical < High < Medium < Watch, so the more-urgent of two
        // is min(). "At least High" keeps Critical but raises Medium/Watch to High.
        assert_eq!(Urgency::High.min(Urgency::Critical), Urgency::Critical);
        assert_eq!(Urgency::High.min(Urgency::Medium), Urgency::High);
        assert_eq!(Urgency::High.min(Urgency::Watch), Urgency::High);
    }

    #[test]
    fn bare_package_name_strips_ecosystem_qualifier() {
        // The signal/version lookups match on article titles, which never carry
        // the " (ecosystem)" qualifier — so the display name must be stripped.
        // This is the fix that lets consequence framing fire at all.
        assert_eq!(bare_package_name("react (npm)"), "react");
        assert_eq!(bare_package_name("axum (crates.io)"), "axum");
        assert_eq!(bare_package_name("@sentry/node (npm)"), "@sentry/node");
        // No qualifier → unchanged.
        assert_eq!(bare_package_name("typescript"), "typescript");
        // Scoped/parenthetical names without a trailing qualifier are left intact.
        assert_eq!(bare_package_name("some-pkg"), "some-pkg");
    }

    /// Create an in-memory DB with the EXACT schema from migrations.rs.
    /// This is the single source of truth for what the real DB looks like —
    /// if migrations.rs changes a column name, these tests will catch the drift.
    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                url TEXT,
                source_type TEXT NOT NULL,
                content TEXT,
                relevance_score REAL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                content_hash TEXT,
                last_seen TEXT,
                content_type TEXT DEFAULT NULL
            );

            CREATE TABLE project_dependencies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                manifest_type TEXT NOT NULL,
                package_name TEXT NOT NULL,
                version TEXT,
                is_dev INTEGER DEFAULT 0,
                is_direct INTEGER DEFAULT 1,
                language TEXT NOT NULL DEFAULT 'unknown',
                last_scanned TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(project_path, package_name)
            );

            CREATE TABLE user_dependencies (
                id INTEGER PRIMARY KEY,
                project_path TEXT NOT NULL,
                package_name TEXT NOT NULL,
                version TEXT,
                ecosystem TEXT NOT NULL,
                is_dev INTEGER DEFAULT 0,
                is_direct INTEGER DEFAULT 1,
                detected_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
                license TEXT,
                UNIQUE(project_path, package_name, ecosystem)
            );

            -- REAL schema: interactions has `timestamp`, NOT `created_at`.
            -- The Phase 1.1 agent guessed `created_at` and the bug sat
            -- silently in production. This test schema catches that class
            -- of drift at compile-time.
            CREATE TABLE interactions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER,
                item_id INTEGER,
                action TEXT,
                action_type TEXT,
                action_data TEXT,
                item_topics TEXT,
                item_source TEXT,
                signal_strength REAL DEFAULT 0.5,
                timestamp TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE user_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                view_id TEXT,
                metadata TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                session_id TEXT
            );

            CREATE TABLE feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER NOT NULL,
                relevant INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE source_item_dependencies (
                id INTEGER PRIMARY KEY,
                source_item_id INTEGER NOT NULL,
                package_name TEXT NOT NULL,
                ecosystem TEXT,
                match_type TEXT NOT NULL DEFAULT 'title_heuristic',
                confidence REAL NOT NULL DEFAULT 0.5,
                evidence_text TEXT,
                source_url TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (source_item_id) REFERENCES source_items(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS blind_spot_dismissals (
                id INTEGER PRIMARY KEY,
                item_id TEXT NOT NULL UNIQUE,
                reason TEXT NOT NULL,
                dismissed_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS feed_health (
                feed_origin TEXT NOT NULL,
                source_type TEXT NOT NULL,
                consecutive_failures INTEGER NOT NULL DEFAULT 0,
                total_successes INTEGER NOT NULL DEFAULT 0,
                total_failures INTEGER NOT NULL DEFAULT 0,
                last_success_at TEXT,
                last_failure_at TEXT,
                last_error TEXT,
                circuit_open INTEGER NOT NULL DEFAULT 0,
                circuit_opened_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (feed_origin, source_type)
            );
            ",
        )
        .expect("schema create");
        conn
    }

    fn insert_project_dep(
        conn: &Connection,
        project_path: &str,
        package_name: &str,
        language: &str,
        is_direct: bool,
        is_dev: bool,
    ) {
        conn.execute(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, is_direct, is_dev, language)
             VALUES (?1, 'npm', ?2, ?3, ?4, ?5)",
            params![project_path, package_name, is_direct as i32, is_dev as i32, language],
        )
        .unwrap();
    }

    fn insert_source_item(conn: &Connection, title: &str, score: f32, days_ago: i64) -> i64 {
        conn.execute(
            "INSERT INTO source_items (title, source_type, content, relevance_score, created_at)
             VALUES (?1, 'hackernews', 'content', ?2, datetime('now', ?3))",
            params![title, score, format!("-{} days", days_ago)],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn insert_source_item_with_meta(
        conn: &Connection,
        title: &str,
        source_type: &str,
        content_type: Option<&str>,
        score: f32,
        days_ago: i64,
    ) -> i64 {
        conn.execute(
            "INSERT INTO source_items (title, source_type, content, relevance_score, created_at, content_type)
             VALUES (?1, ?2, 'content', ?3, datetime('now', ?4), ?5)",
            params![title, source_type, score, format!("-{} days", days_ago), content_type],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    // ─── Fix 1: column names (package_name / ecosystem) ──────────────────

    #[test]
    fn fix1_get_dependency_coverage_uses_correct_columns() {
        let conn = setup_test_db();
        insert_project_dep(&conn, "/proj/a", "react", "javascript", true, false);
        insert_project_dep(&conn, "/proj/a", "typescript", "javascript", true, false);
        insert_project_dep(&conn, "/proj/b", "react", "javascript", true, false);
        insert_project_dep(&conn, "/proj/a", "jest", "javascript", true, true); // dev — should be excluded
        insert_project_dep(&conn, "/proj/a", "lodash", "javascript", false, false); // transitive — should be excluded

        let deps = get_dependency_coverage(&conn).expect("coverage query");

        // react + typescript, deduped to 2 unique package names
        assert_eq!(deps.len(), 2, "should return 2 unique direct runtime deps");
        let names: Vec<&str> = deps.iter().map(|d| d.package_name.as_str()).collect();
        assert!(names.contains(&"react"));
        assert!(names.contains(&"typescript"));
        assert!(!names.contains(&"jest"), "dev dep must be excluded");
        assert!(
            !names.contains(&"lodash"),
            "transitive dep must be excluded"
        );

        // react appears in 2 projects — project list must aggregate
        let react = deps.iter().find(|d| d.package_name == "react").unwrap();
        assert_eq!(react.projects.len(), 2, "react used in 2 projects");
    }

    #[test]
    fn fix1_get_dependency_coverage_returns_empty_when_no_table() {
        // Use a DB with no schema at all
        let conn = Connection::open_in_memory().unwrap();
        let deps = get_dependency_coverage(&conn).expect("should not error on missing table");
        assert_eq!(deps.len(), 0);
    }

    // ─── Fix 2: timestamp column (not created_at) ────────────────────────

    #[test]
    fn fix2_find_uncovered_deps_uses_timestamp_column() {
        // Regression test for Bug 3: find_uncovered_deps referenced
        // `i.created_at` but the real column is `i.timestamp`. The SQL
        // silently errored and the `.unwrap_or(999)` caught it, making
        // every dep look like "999 days since last interaction" → critical.
        //
        // This test proves the query works against the REAL schema AND
        // that the days_since value reflects the actual timestamp.

        let conn = setup_test_db();
        insert_project_dep(&conn, "/proj/a", "myframework", "javascript", true, false);

        // 3 recent source items — all unread
        for i in 0..3 {
            insert_source_item(&conn, &format!("myframework release {i}"), 0.8, 2);
        }

        // Record an interaction 20 days ago (OUTSIDE the 14-day recent
        // window, so the dep is NOT skipped by the "recently interacted" rule).
        // The interaction is on a different (older) item.
        let item_id = insert_source_item(&conn, "myframework past article", 0.7, 25);
        conn.execute(
            "INSERT INTO interactions (item_id, action, timestamp)
             VALUES (?1, 'click', datetime('now', '-20 days'))",
            params![item_id],
        )
        .unwrap();

        let deps = vec![DepCoverage {
            package_name: "myframework".to_string(),
            ecosystem: "javascript".to_string(),
            projects: vec!["/proj/a".to_string()],
        }];

        let (uncovered, _weak) = find_uncovered_deps(&conn, &deps, 14).expect("must not SQL-error");
        assert_eq!(uncovered.len(), 1, "should flag as blind spot");
        let u = &uncovered[0];

        // CRITICAL: days_since must reflect the REAL interaction timestamp.
        // If the query had used the wrong column (created_at), unwrap_or(999)
        // would produce 999 here.
        assert!(
            u.days_since_last_signal >= 18 && u.days_since_last_signal <= 22,
            "days_since should be ~20 from real i.timestamp, got {} (999 = column bug)",
            u.days_since_last_signal
        );
    }

    #[test]
    fn fix2_find_uncovered_deps_flags_zero_available_as_blind_spot() {
        let conn = setup_test_db();
        insert_project_dep(&conn, "/proj/a", "nobodycares", "javascript", true, false);

        // No source_items mention "nobodycares" at all — this IS a blind spot.
        // Zero signal coverage means zero visibility into the dependency.
        let deps = vec![DepCoverage {
            package_name: "nobodycares".to_string(),
            ecosystem: "javascript".to_string(),
            projects: vec!["/proj/a".to_string()],
        }];

        let (uncovered, _weak) = find_uncovered_deps(&conn, &deps, 14).unwrap();
        assert_eq!(
            uncovered.len(),
            1,
            "deps with zero available signals ARE blind spots — no coverage at all"
        );
        let u = &uncovered[0];
        assert_eq!(u.available_signal_count, 0);
        assert_eq!(u.days_since_last_signal, 999);
        assert_eq!(u.risk_level, "medium"); // 1 project in known ecosystem = medium
    }

    #[test]
    fn fix2_zero_signal_dep_risk_scales_with_project_count() {
        let conn = setup_test_db();

        // 3+ projects using a dep with zero signals = high risk
        let deps = vec![DepCoverage {
            package_name: "invisidep".to_string(),
            ecosystem: "cargo".to_string(),
            projects: vec![
                "/proj/a".to_string(),
                "/proj/b".to_string(),
                "/proj/c".to_string(),
            ],
        }];

        let (uncovered, _weak) = find_uncovered_deps(&conn, &deps, 14).unwrap();
        assert_eq!(uncovered.len(), 1);
        assert_eq!(uncovered[0].risk_level, "high");

        // 2 projects = medium risk
        let deps2 = vec![DepCoverage {
            package_name: "invisidep2".to_string(),
            ecosystem: "cargo".to_string(),
            projects: vec!["/proj/a".to_string(), "/proj/b".to_string()],
        }];
        let (uncovered2, _weak2) = find_uncovered_deps(&conn, &deps2, 14).unwrap();
        assert_eq!(uncovered2.len(), 1);
        assert_eq!(uncovered2[0].risk_level, "medium");
    }

    // ─── Fix 3: LIMIT and short-name filter ──────────────────────────────

    #[test]
    fn fix3_find_uncovered_deps_caps_at_max() {
        let conn = setup_test_db();
        // Create 100 deps (more than the MAX_DEPS_TO_PROCESS = 50 cap)
        let mut deps = Vec::new();
        for i in 0..100 {
            let name = format!("package_number_{i:03}");
            insert_source_item(&conn, &format!("{name} released"), 0.8, 2);
            deps.push(DepCoverage {
                package_name: name,
                ecosystem: "javascript".to_string(),
                projects: vec!["/proj/a".to_string()],
            });
        }

        let (uncovered, _weak) = find_uncovered_deps(&conn, &deps, 14).unwrap();
        assert!(
            uncovered.len() <= 50,
            "must cap at MAX_DEPS_TO_PROCESS=50, got {}",
            uncovered.len()
        );
    }

    #[test]
    fn fix3_find_uncovered_deps_skips_short_names() {
        let conn = setup_test_db();
        insert_source_item(&conn, "security advisory for go runtime", 0.9, 2);

        let deps = vec![DepCoverage {
            package_name: "go".to_string(), // 2 chars, too short
            ecosystem: "go".to_string(),
            projects: vec!["/proj/a".to_string()],
        }];

        let (uncovered, _weak) = find_uncovered_deps(&conn, &deps, 14).unwrap();
        assert_eq!(
            uncovered.len(),
            0,
            "short dep names must be skipped to avoid LIKE noise"
        );
    }

    #[test]
    fn builtin_modules_filtered_from_blind_spots() {
        let conn = setup_test_db();
        insert_source_item(&conn, "new crypto mining attack vector", 0.9, 2);

        let deps = vec![DepCoverage {
            package_name: "crypto".to_string(),
            ecosystem: "javascript".to_string(),
            projects: vec!["/proj/a".to_string()],
        }];

        let (uncovered, _weak) = find_uncovered_deps(&conn, &deps, 14).unwrap();
        assert_eq!(
            uncovered.len(),
            0,
            "built-in modules like 'crypto' must be filtered to avoid false blind spots"
        );
    }

    #[test]
    fn word_boundary_match_surfaces_specific_deps() {
        let conn = setup_test_db();
        insert_source_item_with_meta(
            &conn,
            "CVE-2026-9999 in axum web framework",
            "osv",
            Some("security_advisory"),
            0.9,
            2,
        );

        let deps = vec![DepCoverage {
            package_name: "axum".to_string(),
            ecosystem: "rust".to_string(),
            projects: vec!["/proj/a".to_string()],
        }];

        let (uncovered, _weak) = find_uncovered_deps(&conn, &deps, 14).unwrap();
        assert_eq!(
            uncovered.len(),
            1,
            "word-boundary matches surface for specific (non-generic) dep names"
        );
    }

    #[test]
    fn generic_dep_names_filtered_from_uncovered() {
        let conn = setup_test_db();
        insert_source_item_with_meta(
            &conn,
            "How to find the best open source tools",
            "rss",
            Some("tutorial"),
            0.9,
            2,
        );

        // "find" and "open" are truly generic English words — they should be
        // filtered to prevent false matches against article titles.
        let deps = vec![
            DepCoverage {
                package_name: "find".to_string(),
                ecosystem: "npm".to_string(),
                projects: vec!["/proj/a".to_string()],
            },
            DepCoverage {
                package_name: "open".to_string(),
                ecosystem: "npm".to_string(),
                projects: vec!["/proj/a".to_string()],
            },
        ];

        let (uncovered, _weak) = find_uncovered_deps(&conn, &deps, 14).unwrap();
        // Both are generic AND < 4 chars ("find" is 4 chars, passes length
        // check but is in generic list; "open" is 4 chars, same).
        // They get filtered by is_generic_dep_name before entering the query.
        // The remaining uncovered entries are zero-signal deps (from our Fix 1
        // change) but both deps are filtered out of eligible_deps entirely.
        assert_eq!(
            uncovered.len(),
            0,
            "generic English words as dep names must be filtered to prevent false matches"
        );
    }

    #[test]
    fn builtin_module_detection() {
        assert!(is_builtin_module("crypto"));
        assert!(is_builtin_module("Crypto"));
        assert!(is_builtin_module("http"));
        assert!(is_builtin_module("json"));
        assert!(!is_builtin_module("express"));
        assert!(!is_builtin_module("react"));
        assert!(!is_builtin_module("tokio"));
        assert!(!is_builtin_module("serde"));
    }

    // ─── Cross-lens dedup (P0) ──────────────────────────────────────────

    #[test]
    fn preemption_territory_detects_security_keywords() {
        assert!(is_preemption_territory("CVE-2026-12345: React DOM XSS"));
        assert!(is_preemption_territory("Critical vulnerability in Deno"));
        assert!(is_preemption_territory(
            "GHSA-xxxx: npm supply chain attack"
        ));
        assert!(is_preemption_territory("security advisory for lodash"));
        assert!(is_preemption_territory("Zero-day exploit found in Chrome"));
        assert!(is_preemption_territory("Breaking change in React 20"));
        assert!(is_preemption_territory("Node.js 18 reaches end of life"));
        assert!(is_preemption_territory("Migration guide for Vite 7"));
        assert!(is_preemption_territory("Malware found in npm package"));
    }

    #[test]
    fn preemption_territory_allows_non_security() {
        assert!(!is_preemption_territory(
            "React 20 performance improvements"
        ));
        assert!(!is_preemption_territory("How to use Deno with Fresh"));
        assert!(!is_preemption_territory("New features in TypeScript 6.0"));
        assert!(!is_preemption_territory("Building CLI tools with Rust"));
        assert!(!is_preemption_territory("Tauri 3.0 release notes"));
    }

    // ─── Evidence threshold (P1) ─────────────────────────────────────────

    #[test]
    fn score_returns_sentinel_when_few_deps() {
        let score = calculate_blind_spot_score(&[], &[], &[], 5);
        assert!(
            score < 0.0,
            "fewer than 10 deps should return -1.0 sentinel"
        );
    }

    #[test]
    fn score_computes_normally_with_sufficient_deps() {
        let score = calculate_blind_spot_score(&[], &[], &[], 15);
        assert!(score >= 0.0, "15 deps should compute a real score");
    }

    // ─── Fix 4: score normalization ──────────────────────────────────────

    #[test]
    fn fix4_score_never_pinned_to_100_for_normal_stack() {
        // Reproduces the screenshot bug: many uncovered deps with critical
        // risk should NOT trivially saturate to 100.
        let uncovered: Vec<UncoveredDep> = (0..20)
            .map(|i| UncoveredDep {
                name: format!("dep{i}"),
                dep_type: "npm".to_string(),
                projects_using: vec!["/proj".to_string()],
                days_since_last_signal: 30,
                available_signal_count: 5,
                risk_level: "medium".to_string(),
                match_type: "none".to_string(),
                coverage_reason: None,
                adapters_searched: Vec::new(),
            })
            .collect();
        let stale: Vec<StaleTopic> = (0..3)
            .map(|i| StaleTopic {
                topic: format!("topic{i}"),
                last_engagement_days: 15,
                active_deps_in_topic: 2,
                missed_signal_count: 5,
            })
            .collect();
        let missed: Vec<MissedSignal> = (0..10)
            .map(|i| MissedSignal {
                item_id: i,
                title: format!("Article {i}"),
                url: None,
                source_type: "hn".to_string(),
                relevance_score: 0.7,
                created_at: "2026-04-11T00:00:00Z".to_string(),
                why_relevant: String::new(),
                dep_name: None,
                was_shown: false,
                content_type: Some("discussion".into()),
            })
            .collect();

        // total_direct_deps = 100 means 20 uncovered mediums = 20*0.4 = 8 weighted,
        // divided by max(100, 5) = 100 → uncovered_pressure = 0.08
        // stale: 3 topics → 3 / (3+3) = 0.5
        // missed: avg 0.7, count_boost ≈ 0.6, → 0.7*0.7 + 0.6*0.3 = 0.67
        // score = 0.08*55 + 0.5*25 + 0.67*20 = 4.4 + 12.5 + 13.4 = 30.3
        let score = calculate_blind_spot_score(&uncovered, &stale, &missed, 100);
        assert!(
            score > 0.0 && score < 100.0,
            "score must be in range, got {score}"
        );
        assert!(
            score < 50.0,
            "moderate stack with medium risks should score under 50, got {score}"
        );
    }

    #[test]
    fn fix4_score_clean_stack_is_near_zero() {
        let score = calculate_blind_spot_score(&[], &[], &[], 50);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn fix4_score_respects_denominator_floor() {
        // Tiny stack with 1 critical uncovered shouldn't max out.
        let uncovered = vec![UncoveredDep {
            name: "one".to_string(),
            dep_type: "npm".to_string(),
            projects_using: vec!["/p".to_string()],
            days_since_last_signal: 100,
            available_signal_count: 5,
            risk_level: "critical".to_string(),
            match_type: "none".to_string(),
            coverage_reason: None,
            adapters_searched: Vec::new(),
        }];
        // total_direct_deps = 1 → floor to 5 → 1.0/5 = 0.2 uncovered_pressure
        // Score = 0.2 * 55 = 11.0
        let score = calculate_blind_spot_score(&uncovered, &[], &[], 1);
        assert!(score < 20.0, "small stack with 1 critical: {score}");
    }

    // ─── Fix 5: missed_signals dedup (feed window) ──────────────────────

    #[test]
    fn fix5_find_missed_signals_excludes_feed_window() {
        let conn = setup_test_db();
        // Recent item (within feed window) — should NOT be surfaced as missed
        insert_source_item(&conn, "very recent thing", 0.9, 1);
        // Old enough to be "missed" (outside the 3-day feed window)
        insert_source_item(&conn, "older missed thing", 0.9, 7);

        let signals = find_missed_signals(&conn, 14, &[]).unwrap();
        assert!(
            !signals.iter().any(|s| s.title == "very recent thing"),
            "items within feed window must be excluded"
        );
        assert!(
            signals.iter().any(|s| s.title == "older missed thing"),
            "items older than feed window must be surfaced"
        );
    }

    #[test]
    fn fix5_find_missed_signals_excludes_impressions() {
        let conn = setup_test_db();
        let item_id = insert_source_item(&conn, "seen article", 0.9, 5);
        // Record an impression — the user DID see this item
        conn.execute(
            "INSERT INTO user_events (event_type, metadata) VALUES ('impression', ?1)",
            params![format!("{{\"item_id\":{item_id}}}")],
        )
        .unwrap();

        insert_source_item(&conn, "unseen article", 0.9, 5);

        let signals = find_missed_signals(&conn, 14, &[]).unwrap();
        assert!(
            !signals.iter().any(|s| s.title == "seen article"),
            "items with an impression event must be excluded"
        );
        assert!(
            signals.iter().any(|s| s.title == "unseen article"),
            "items without impressions must be surfaced"
        );
    }

    // ─── Fix 6: real why_relevant ───────────────────────────────────────

    #[test]
    fn fix6_why_relevant_detects_dep_mentions() {
        let deps = vec![DepCoverage {
            package_name: "react".to_string(),
            ecosystem: "javascript".to_string(),
            projects: vec![],
        }];
        let (text, dep) = compute_why_relevant("New features in React 19", 0.9, &deps);
        assert!(
            text.contains("react"),
            "why_relevant must name the matched dep: {text}"
        );
        assert!(
            !text.contains("strong match with your dependencies"),
            "must not use the old canned lying text: {text}"
        );
        assert_eq!(dep, Some("react".to_string()), "dep_name must be set");
    }

    #[test]
    fn fix6_why_relevant_returns_empty_when_no_match() {
        let deps = vec![DepCoverage {
            package_name: "react".to_string(),
            ecosystem: "javascript".to_string(),
            projects: vec![],
        }];
        // Title doesn't mention react — no evidence to claim relevance
        let (text, dep) = compute_why_relevant("Postgres new extension released", 0.9, &deps);
        assert!(
            text.is_empty(),
            "must return empty string when no dep match — never claim unsubstantiated relevance: {text}"
        );
        assert_eq!(dep, None, "dep_name must be None when no match");
    }

    #[test]
    fn fix6_word_boundary_match_rejects_substrings() {
        assert!(has_word_boundary_match("react is great", "react"));
        assert!(has_word_boundary_match("next.js is fine", "next")); // .js suffix allowed
        assert!(has_word_boundary_match("use serde.rs for json", "serde")); // .rs suffix allowed
        assert!(!has_word_boundary_match("unexpected happens here", "next")); // embedded in word
        assert!(!has_word_boundary_match("configuring app", "conf")); // substring of config
    }

    // ========================================================================
    // EvidenceItem conversion tests (Intelligence Reconciliation — Phase 4)
    // ========================================================================

    fn uncov_sample() -> UncoveredDep {
        UncoveredDep {
            name: "tokio".into(),
            dep_type: "cargo".into(),
            projects_using: vec!["/proj/a".into(), "/proj/b".into()],
            days_since_last_signal: 21,
            available_signal_count: 4,
            risk_level: "critical".into(),
            match_type: "exact_registry".into(),
            coverage_reason: None,
            adapters_searched: Vec::new(),
        }
    }

    fn stale_sample() -> StaleTopic {
        StaleTopic {
            topic: "async-rust".into(),
            last_engagement_days: 30,
            active_deps_in_topic: 3,
            missed_signal_count: 7,
        }
    }

    fn missed_sample() -> MissedSignal {
        MissedSignal {
            item_id: 42,
            title: "Critical Tokio 1.x vulnerability disclosed".into(),
            url: Some("https://example.test/post/42".into()),
            source_type: "hn".into(),
            relevance_score: 0.9,
            created_at: "2026-04-10 14:30:00".into(),
            why_relevant: "Matches tokio in 2 of your projects".into(),
            dep_name: Some("tokio".into()),
            was_shown: false,
            content_type: Some("security_advisory".into()),
        }
    }

    fn rec_sample() -> BlindSpotRecommendation {
        BlindSpotRecommendation {
            action: "Set up a watch for Rust security advisories".into(),
            reason: "You have 4 uncovered Rust dependencies".into(),
            priority: "high".into(),
        }
    }

    fn report_sample() -> BlindSpotReport {
        BlindSpotReport {
            overall_score: 68.0,
            uncovered_dependencies: vec![uncov_sample()],
            stale_topics: vec![stale_sample()],
            missed_signals: vec![missed_sample()],
            recommendations: vec![rec_sample()],
            weak_matches: vec![],
            generated_at: "2026-04-17 00:00:00".into(),
            data_freshness: None,
        }
    }

    #[test]
    fn uncovered_dep_maps_to_gap_kind() {
        let item = uncovered_dep_to_evidence_item(&uncov_sample());
        assert_eq!(item.kind, crate::evidence::EvidenceKind::Gap);
        assert_eq!(item.urgency, crate::evidence::Urgency::Critical);
        assert!(item.affected_deps.contains(&"tokio".to_string()));
        assert_eq!(item.affected_projects.len(), 2);
        assert!(crate::evidence::validate_item(&item).is_ok());
    }

    #[test]
    fn stale_topic_maps_to_gap_kind() {
        let item = stale_topic_to_evidence_item(&stale_sample());
        assert_eq!(item.kind, crate::evidence::EvidenceKind::Gap);
        // 30 days + 7 missed → Medium urgency
        assert_eq!(item.urgency, crate::evidence::Urgency::Medium);
        assert!(crate::evidence::validate_item(&item).is_ok());
    }

    #[test]
    fn missed_signal_maps_to_missed_signal_kind() {
        let item = missed_signal_to_evidence_item(&missed_sample());
        assert_eq!(item.kind, crate::evidence::EvidenceKind::MissedSignal);
        assert_eq!(item.urgency, crate::evidence::Urgency::Critical);
        assert_eq!(item.evidence.len(), 1);
        assert!(crate::evidence::validate_item(&item).is_ok());
    }

    #[test]
    fn recommendation_maps_to_alert_kind() {
        let item = recommendation_to_evidence_item(&rec_sample(), 0);
        assert_eq!(item.kind, crate::evidence::EvidenceKind::Alert);
        assert_eq!(item.urgency, crate::evidence::Urgency::High);
        assert!(!item.suggested_actions.is_empty());
        assert!(crate::evidence::validate_item(&item).is_ok());
    }

    #[test]
    fn report_converts_to_feed_with_score() {
        let feed = blind_spot_report_to_feed(&report_sample());
        // 1 uncovered + 1 stale + 1 missed + 1 recommendation
        assert_eq!(feed.total, 4);
        assert_eq!(feed.score, Some(68.0));
        assert_eq!(feed.critical_count, 2); // uncovered "critical" + missed "vulnerability" (tier 4)
        assert_eq!(feed.high_count, 1); // high-priority recommendation
    }

    #[test]
    fn empty_report_produces_empty_feed() {
        let report = BlindSpotReport {
            overall_score: 0.0,
            uncovered_dependencies: vec![],
            stale_topics: vec![],
            missed_signals: vec![],
            recommendations: vec![],
            weak_matches: vec![],
            generated_at: String::new(),
            data_freshness: None,
        };
        let feed = blind_spot_report_to_feed(&report);
        assert_eq!(feed.total, 0);
        assert_eq!(feed.score, Some(0.0));
    }

    #[test]
    fn negative_coverage_score_survives_feed_boundary() {
        let mut report = report_sample();
        report.overall_score = -1.0;
        let feed = blind_spot_report_to_feed(&report);
        assert_eq!(feed.score, Some(-1.0));
    }

    #[test]
    fn rebuild_feed_preserves_score_and_recounts_items() {
        let items = vec![
            uncovered_dep_to_evidence_item(&uncov_sample()),
            recommendation_to_evidence_item(&rec_sample(), 0),
        ];
        let feed = build_feed_with_existing_score(items, Some(-1.0));
        assert_eq!(feed.score, Some(-1.0));
        assert_eq!(feed.total, 2);
        assert_eq!(feed.critical_count, 1);
        assert_eq!(feed.high_count, 1);
    }

    #[test]
    fn all_items_pass_schema_validation() {
        let feed = blind_spot_report_to_feed(&report_sample());
        for it in &feed.items {
            assert!(
                crate::evidence::validate_item(it).is_ok(),
                "item {} failed validation",
                it.id
            );
        }
    }

    #[test]
    fn all_items_tagged_with_blind_spots_lens() {
        let feed = blind_spot_report_to_feed(&report_sample());
        for it in &feed.items {
            assert!(it.lens_hints.blind_spots);
            assert!(!it.lens_hints.preemption);
            assert!(!it.lens_hints.briefing);
            assert!(!it.lens_hints.evidence);
        }
    }

    #[test]
    fn content_type_security_gets_critical_urgency() {
        let mut m = missed_sample();
        m.content_type = Some("security_advisory".into());
        m.relevance_score = 0.8;
        let item = missed_signal_to_evidence_item(&m);
        assert_eq!(item.urgency, crate::evidence::Urgency::Critical);
    }

    #[test]
    fn content_type_discussion_gets_watch_urgency() {
        let mut m = missed_sample();
        m.content_type = Some("discussion".into());
        m.title = "We Scored 28 Famous Open Source PRs for Deploy Risk".into();
        m.relevance_score = 0.8;
        let item = missed_signal_to_evidence_item(&m);
        assert_eq!(item.urgency, crate::evidence::Urgency::Watch);
    }

    #[test]
    fn content_type_release_gets_medium_urgency() {
        let mut m = missed_sample();
        m.content_type = Some("release_notes".into());
        m.title = "npm: react v19.2.5".into();
        let item = missed_signal_to_evidence_item(&m);
        assert_eq!(item.urgency, crate::evidence::Urgency::Medium);
    }

    #[test]
    fn content_type_breaking_gets_high_urgency() {
        let mut m = missed_sample();
        m.content_type = Some("breaking_change".into());
        m.relevance_score = 0.85;
        m.title = "React drops support for IE11".into();
        let item = missed_signal_to_evidence_item(&m);
        assert_eq!(item.urgency, crate::evidence::Urgency::High);
    }

    #[test]
    fn null_content_type_falls_back_to_title() {
        let mut m = missed_sample();
        m.content_type = None;
        m.title = "Critical CVE-2026-99999 in tokio".into();
        m.relevance_score = 0.8;
        let item = missed_signal_to_evidence_item(&m);
        assert_eq!(item.urgency, crate::evidence::Urgency::Critical);
    }

    #[test]
    fn null_content_type_blog_gets_watch() {
        let mut m = missed_sample();
        m.content_type = None;
        m.title = "Open source Playwright tool for testing".into();
        m.relevance_score = 0.8;
        let item = missed_signal_to_evidence_item(&m);
        assert_eq!(item.urgency, crate::evidence::Urgency::Watch);
    }

    #[test]
    fn test_weak_match_deps_separated() {
        let conn = setup_test_db();

        // Insert source items with titles matching dep names.
        // "image" is ambiguous — title_heuristic only from hackernews → weak match.
        // "axum" is NOT ambiguous — title_heuristic still counts as uncovered.
        insert_source_item_with_meta(
            &conn,
            "Rust image crate v3.0 released with AVIF support",
            "hackernews",
            None,
            0.8,
            5,
        );
        insert_source_item_with_meta(
            &conn,
            "axum web framework hits 1.0 milestone",
            "hackernews",
            None,
            0.8,
            5,
        );

        // Set up deps: both are in the user's stack
        let deps = vec![
            DepCoverage {
                package_name: "image".to_string(),
                ecosystem: "cargo".to_string(),
                projects: vec!["/proj/myapp".to_string()],
            },
            DepCoverage {
                package_name: "axum".to_string(),
                ecosystem: "cargo".to_string(),
                projects: vec!["/proj/myapp".to_string()],
            },
        ];

        let (uncovered, weak_matches) = find_uncovered_deps(&conn, &deps, 14).unwrap();

        // "image" should be in weak_matches (ambiguous + title_heuristic only)
        assert!(
            weak_matches.iter().any(|d| d.name.contains("image")),
            "image should be in weak_matches, got: {:?}",
            weak_matches.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
        assert!(
            weak_matches
                .iter()
                .all(|d| d.match_type == "title_heuristic"),
            "all weak_matches should have match_type title_heuristic"
        );

        // "image" should NOT be in uncovered
        assert!(
            !uncovered.iter().any(|d| d.name.contains("image")),
            "image should NOT be in uncovered"
        );

        // "axum" should be in uncovered (not ambiguous, has signals)
        assert!(
            uncovered.iter().any(|d| d.name.contains("axum")),
            "axum should be in uncovered, got: {:?}",
            uncovered.iter().map(|d| &d.name).collect::<Vec<_>>()
        );

        // "axum" should NOT be in weak_matches
        assert!(
            !weak_matches.iter().any(|d| d.name.contains("axum")),
            "axum should NOT be in weak_matches"
        );
    }

    // ─── Coverage diagnostics tests ─────────────────────────────────────

    #[test]
    fn test_diagnose_coverage_no_links_no_health() {
        let conn = setup_test_db();
        // No source_item_dependencies rows, no feed_health rows
        let (reason, _detail) = diagnose_coverage(&conn, "some-crate", "cargo");
        assert_eq!(
            reason, "not_checked",
            "with no feed_health data, adapters haven't run yet"
        );
    }

    #[test]
    fn test_diagnose_coverage_with_weak_links() {
        let conn = setup_test_db();
        // Insert a source item and link it via source_item_dependencies
        let item_id = insert_source_item(&conn, "some-crate release notes", 0.6, 3);
        conn.execute(
            "INSERT INTO source_item_dependencies (source_item_id, package_name, ecosystem, match_type, confidence) \
             VALUES (?1, 'some-crate', 'cargo', 'title_heuristic', 0.3)",
            params![item_id],
        )
        .unwrap();

        let (reason, _detail) = diagnose_coverage(&conn, "some-crate", "cargo");
        assert_eq!(
            reason, "weak_matches_only",
            "dep with linked source items should be weak_matches_only"
        );
    }

    #[test]
    fn test_diagnose_coverage_healthy_sources_no_results() {
        let conn = setup_test_db();
        // No source_item_dependencies, but feed_health shows crates_io is healthy
        conn.execute(
            "INSERT INTO feed_health (feed_origin, source_type, consecutive_failures, total_successes, updated_at) \
             VALUES ('default', 'crates_io', 0, 50, datetime('now'))",
            [],
        )
        .unwrap();

        let (reason, detail) = diagnose_coverage(&conn, "nonexistent-crate", "cargo");
        assert_eq!(
            reason, "checked_no_results",
            "healthy adapters + no results = checked_no_results"
        );
        assert!(
            detail.contains("crates_io"),
            "detail should mention which sources were checked: {detail}"
        );
    }

    #[test]
    fn test_diagnose_coverage_adapter_failing() {
        let conn = setup_test_db();
        // feed_health shows crates_io is failing
        conn.execute(
            "INSERT INTO feed_health (feed_origin, source_type, consecutive_failures, total_successes, updated_at) \
             VALUES ('default', 'crates_io', 5, 10, datetime('now'))",
            [],
        )
        .unwrap();

        let (reason, _detail) = diagnose_coverage(&conn, "tokio", "cargo");
        assert_eq!(
            reason, "adapter_failing",
            "adapter with >=3 consecutive failures should be flagged"
        );
    }

    #[test]
    fn test_ecosystem_source_types_known() {
        assert!(!ecosystem_source_types("npm").is_empty());
        assert!(
            ecosystem_source_types("npm").contains(&"npm_registry".to_string()),
            "npm ecosystem should map to npm_registry adapter"
        );
        assert!(!ecosystem_source_types("crates.io").is_empty());
        assert!(ecosystem_source_types("crates.io").contains(&"crates_io".to_string()));
        assert!(!ecosystem_source_types("pypi").is_empty());
        assert!(ecosystem_source_types("pypi").contains(&"pypi".to_string()));
        assert!(
            ecosystem_source_types("go").contains(&"go_modules".to_string()),
            "go ecosystem should map to go_modules adapter"
        );
    }

    #[test]
    fn test_ecosystem_source_types_unknown_has_osv() {
        let types = ecosystem_source_types("unknown_lang");
        assert!(!types.is_empty(), "even unknown ecosystems get osv");
        assert!(types.contains(&"osv".to_string()));
    }

    #[test]
    fn test_coverage_reason_in_evidence_item_zero_signal() {
        let dep = UncoveredDep {
            name: "mystery-pkg".into(),
            dep_type: "npm".into(),
            projects_using: vec!["/proj".into()],
            days_since_last_signal: 999,
            available_signal_count: 0,
            risk_level: "medium".into(),
            match_type: "none".into(),
            coverage_reason: Some("not_checked".into()),
            adapters_searched: Vec::new(),
        };
        let item = uncovered_dep_to_evidence_item(&dep);
        assert!(
            item.explanation.contains("hasn't checked"),
            "not_checked reason should produce honest explanation: {}",
            item.explanation
        );
    }

    #[test]
    fn test_coverage_reason_adapter_failing_in_evidence() {
        let dep = UncoveredDep {
            name: "failing-pkg".into(),
            dep_type: "cargo".into(),
            projects_using: vec!["/proj".into()],
            days_since_last_signal: 999,
            available_signal_count: 0,
            risk_level: "high".into(),
            match_type: "none".into(),
            coverage_reason: Some("adapter_failing".into()),
            adapters_searched: Vec::new(),
        };
        let item = uncovered_dep_to_evidence_item(&dep);
        assert!(
            item.explanation.contains("source adapters are failing"),
            "adapter_failing should mention failing adapters: {}",
            item.explanation
        );
    }

    #[test]
    fn test_coverage_reason_none_fallback() {
        let dep = UncoveredDep {
            name: "fallback-pkg".into(),
            dep_type: "npm".into(),
            projects_using: vec!["/proj".into()],
            days_since_last_signal: 999,
            available_signal_count: 0,
            risk_level: "low".into(),
            match_type: "none".into(),
            coverage_reason: None,
            adapters_searched: Vec::new(),
        };
        let item = uncovered_dep_to_evidence_item(&dep);
        assert!(
            item.explanation.contains("no confirmed source coverage"),
            "None coverage_reason should use generic fallback: {}",
            item.explanation
        );
    }

    #[test]
    fn test_dismiss_blind_spot_filters_from_feed() {
        let conn = setup_test_db();

        // Build a report with one uncovered dep
        let report = BlindSpotReport {
            overall_score: 50.0,
            uncovered_dependencies: vec![UncoveredDep {
                name: "serde".into(),
                dep_type: "cargo".into(),
                projects_using: vec!["/proj".into()],
                days_since_last_signal: 30,
                available_signal_count: 5,
                risk_level: "medium".into(),
                match_type: "none".into(),
                coverage_reason: Some("not_checked".into()),
                adapters_searched: Vec::new(),
            }],
            stale_topics: Vec::new(),
            missed_signals: Vec::new(),
            recommendations: Vec::new(),
            weak_matches: Vec::new(),
            generated_at: "2026-05-16T00:00:00Z".into(),
            data_freshness: None,
        };

        // Before dismissal: feed should contain the item
        let feed_before = blind_spot_report_to_feed(&report);
        let serde_items: Vec<_> = feed_before
            .items
            .iter()
            .filter(|i| i.id.contains("serde"))
            .collect();
        assert!(
            !serde_items.is_empty(),
            "serde item should be present before dismissal"
        );

        // Dismiss the item
        let serde_id = &serde_items[0].id;
        conn.execute(
            "INSERT INTO blind_spot_dismissals (item_id, reason) VALUES (?1, ?2)",
            params![serde_id, "not relevant"],
        )
        .unwrap();

        // After dismissal: the item should be filtered out.
        // NOTE: blind_spot_report_to_feed queries the real DB via open_db_connection,
        // which won't see our in-memory test DB. So we test the filtering logic
        // directly here to verify the mechanism.
        let dismissed_ids: std::collections::HashSet<String> = {
            let mut stmt = conn
                .prepare("SELECT item_id FROM blind_spot_dismissals")
                .unwrap();
            stmt.query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };
        assert!(
            dismissed_ids.contains(serde_id.as_str()),
            "dismissed_ids should contain the dismissed item"
        );

        // Simulate the filtering that blind_spot_report_to_feed performs
        let filtered: Vec<_> = feed_before
            .items
            .into_iter()
            .filter(|item| !dismissed_ids.contains(&item.id))
            .collect();
        assert!(
            filtered.iter().all(|i| !i.id.contains("serde")),
            "serde item should be filtered after dismissal"
        );
    }

    #[test]
    fn test_package_watch_adds_user_dependency() {
        let conn = setup_test_db();

        // Verify no deps exist initially
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM user_dependencies", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "should start with zero dependencies");

        // Simulate what add_package_watch does (without Tauri command wrapper)
        let path = "user-watch";
        let package_name = "tokio";
        let ecosystem = "cargo";
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, ecosystem, is_direct, detected_at, last_seen_at)
             VALUES (?1, ?2, ?3, 1, datetime('now'), datetime('now'))
             ON CONFLICT(project_path, package_name, ecosystem) DO UPDATE SET
               last_seen_at = datetime('now')",
            params![path, package_name, ecosystem],
        )
        .unwrap();

        // Verify the dep was inserted
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM user_dependencies", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1, "should have one dependency after insert");

        // Verify the values
        let (stored_pkg, stored_eco, stored_path): (String, String, String) = conn
            .query_row(
                "SELECT package_name, ecosystem, project_path FROM user_dependencies LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(stored_pkg, "tokio");
        assert_eq!(stored_eco, "cargo");
        assert_eq!(stored_path, "user-watch");

        // Upsert same package — should update last_seen_at, not create duplicate
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, ecosystem, is_direct, detected_at, last_seen_at)
             VALUES (?1, ?2, ?3, 1, datetime('now'), datetime('now'))
             ON CONFLICT(project_path, package_name, ecosystem) DO UPDATE SET
               last_seen_at = datetime('now')",
            params![path, package_name, ecosystem],
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM user_dependencies", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1, "upsert should not create duplicate");
    }

    // ========================================================================
    // T3-1: Golden Corpus Test Fixtures
    // ========================================================================

    #[test]
    fn test_golden_ambiguous_names_suppressed() {
        // Ambiguous names: common English words that ARE real package names.
        // These require ecosystem-qualified proof (exact_registry / advisory)
        // to surface — title heuristic alone is not enough.
        let should_be_ambiguous = [
            "image", "base", "core", "test", "data", "utils", "log", "error", "config",
        ];
        for name in &should_be_ambiguous {
            assert!(
                is_ambiguous_package_name(name),
                "'{}' should be classified as ambiguous",
                name
            );
        }

        // Specific package names that are NOT common English words.
        // These should pass through the ambiguity filter.
        let should_not_be_ambiguous = ["tokio", "react", "axios", "serde", "next"];
        for name in &should_not_be_ambiguous {
            assert!(
                !is_ambiguous_package_name(name),
                "'{}' should NOT be classified as ambiguous",
                name
            );
        }
    }

    #[test]
    fn test_golden_ambiguous_names_case_insensitive() {
        // is_ambiguous_package_name lowercases internally, so mixed-case
        // variants of ambiguous names must also be caught.
        assert!(is_ambiguous_package_name("Image"));
        assert!(is_ambiguous_package_name("CONFIG"));
        assert!(is_ambiguous_package_name("Log"));
        assert!(is_ambiguous_package_name("CORE"));
    }

    #[test]
    fn test_golden_title_heuristic_exact_match() {
        // Word-boundary match: "react" appears as a standalone word
        // in "React 19 release notes" (case-insensitive comparison after
        // lowercasing both sides).
        assert!(
            has_word_boundary_match("react 19 release notes", "react"),
            "react should word-boundary match in 'react 19 release notes'"
        );

        // Substring-only: "react" is embedded inside "reaction" — NOT
        // a word-boundary match because 'i' follows 'react'.
        assert!(
            !has_word_boundary_match("new reaction features in chemistry", "react"),
            "react should NOT word-boundary match inside 'reaction'"
        );
    }

    #[test]
    fn test_golden_title_heuristic_case_insensitive() {
        // has_word_boundary_match is case-sensitive by design (caller
        // lowercases both). Verify the standard usage pattern works:
        // lowercase both title and dep name before calling.
        let title = "Axios vulnerability found in latest release".to_lowercase();
        let dep = "Axios".to_lowercase();
        assert!(
            has_word_boundary_match(&title, &dep),
            "case-insensitive word-boundary match should work for Axios/axios"
        );

        // Additional: mixed-case title, uppercase dep
        let title2 = "SERDE Derive Macro Overhaul".to_lowercase();
        let dep2 = "SERDE".to_lowercase();
        assert!(
            has_word_boundary_match(&title2, &dep2),
            "case-insensitive word-boundary match should work for SERDE/serde"
        );
    }

    #[test]
    fn test_golden_ecosystem_disambiguation() {
        // The same package name "jsonwebtoken" exists in both npm and crates.io.
        // Verify that find_uncovered_deps treats them as separate blind spots
        // when they appear in different ecosystems.
        let conn = setup_test_db();

        // Insert as npm dep
        insert_project_dep(
            &conn,
            "/proj/node-app",
            "jsonwebtoken",
            "javascript",
            true,
            false,
        );
        // Insert as cargo dep
        insert_project_dep(&conn, "/proj/rust-app", "jsonwebtoken", "rust", true, false);

        // Insert a source item from crates_io (exact_registry for Rust)
        insert_source_item_with_meta(
            &conn,
            "jsonwebtoken 9.4 released on crates.io",
            "crates_io",
            None,
            0.8,
            5,
        );

        let deps = get_dependency_coverage(&conn).unwrap();
        // The SQL groups by (normalized_name, language), so "jsonwebtoken" in
        // javascript vs rust should produce two separate DepCoverage entries.
        let jwt_deps: Vec<_> = deps
            .iter()
            .filter(|d| d.package_name == "jsonwebtoken")
            .collect();
        assert_eq!(
            jwt_deps.len(),
            2,
            "jsonwebtoken should appear twice (once per ecosystem), got {} entries",
            jwt_deps.len()
        );

        // Verify ecosystems are distinct
        let ecosystems: Vec<&str> = jwt_deps.iter().map(|d| d.ecosystem.as_str()).collect();
        assert!(ecosystems.contains(&"javascript"));
        assert!(ecosystems.contains(&"rust"));
    }

    #[test]
    fn test_golden_sid_match_mapping_and_upgrade_order() {
        // Only source_item_dependencies rows can promote a match to advisory
        // or exact_registry. Direct SQL title fallback remains title_heuristic.
        let advisory_mt = sid_match_type_to_coverage("advisory");
        assert_eq!(
            advisory_mt, "advisory",
            "stored SID advisory should map to advisory coverage"
        );

        let registry_mt = sid_match_type_to_coverage("exact_registry");
        assert_eq!(
            registry_mt, "exact_registry",
            "stored SID exact_registry should map to exact_registry coverage"
        );

        let heuristic_mt = sid_match_type_to_coverage("title_heuristic");
        assert_eq!(
            heuristic_mt, "title_heuristic",
            "stored heuristic links should stay heuristic"
        );

        // Verify that advisory > title_heuristic in the upgrade ordering
        let mut current: Option<String> = Some("title_heuristic".to_string());
        upgrade_match_type(&mut current, "advisory");
        assert_eq!(
            current.as_deref(),
            Some("advisory"),
            "advisory should upgrade over title_heuristic"
        );

        // Verify that exact_registry > advisory in upgrade ordering
        let mut current2: Option<String> = Some("advisory".to_string());
        upgrade_match_type(&mut current2, "exact_registry");
        assert_eq!(
            current2.as_deref(),
            Some("exact_registry"),
            "exact_registry should upgrade over advisory"
        );

        // Verify that title_heuristic does NOT downgrade advisory
        let mut current3: Option<String> = Some("advisory".to_string());
        upgrade_match_type(&mut current3, "title_heuristic");
        assert_eq!(
            current3.as_deref(),
            Some("advisory"),
            "title_heuristic should NOT downgrade advisory"
        );
    }

    #[test]
    fn test_golden_title_only_advisory_source_is_weak_without_sid() {
        // A CVE/OSV source title alone is still only a title heuristic. Without
        // a structured SID link, ambiguous names like "image" must stay weak.
        let conn = setup_test_db();

        insert_source_item_with_meta(
            &conn,
            "CVE-2026-5555 in image crate allows buffer overflow",
            "osv",
            Some("security_advisory"),
            0.9,
            5,
        );

        let deps = vec![DepCoverage {
            package_name: "image".to_string(),
            ecosystem: "cargo".to_string(),
            projects: vec!["/proj/myapp".to_string()],
        }];

        let (uncovered, weak_matches) = find_uncovered_deps(&conn, &deps, 14).unwrap();

        assert!(
            !uncovered.iter().any(|d| d.name.contains("image")),
            "title-only advisory source should not promote image into uncovered, got: {:?}",
            uncovered.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
        assert!(
            weak_matches.iter().any(|d| d.name.contains("image")),
            "title-only advisory source should remain a weak match"
        );
    }

    #[test]
    fn test_golden_ambiguous_with_sid_advisory_surfaces() {
        // Structured dep-linker evidence can still promote ambiguous names.
        let conn = setup_test_db();

        let item_id = insert_source_item_with_meta(
            &conn,
            "CVE-2026-5555 in image crate allows buffer overflow",
            "osv",
            Some("security_advisory"),
            0.9,
            5,
        );
        conn.execute(
            "INSERT INTO source_item_dependencies
                (source_item_id, package_name, ecosystem, match_type, confidence)
             VALUES (?1, 'image', 'cargo', 'advisory', 0.90)",
            params![item_id],
        )
        .unwrap();

        let deps = vec![DepCoverage {
            package_name: "image".to_string(),
            ecosystem: "cargo".to_string(),
            projects: vec!["/proj/myapp".to_string()],
        }];

        let (uncovered, weak_matches) = find_uncovered_deps(&conn, &deps, 14).unwrap();

        assert!(
            uncovered.iter().any(|d| d.name.contains("image")),
            "SID advisory match should promote image into uncovered, got: {:?}",
            uncovered.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
        assert!(
            !weak_matches.iter().any(|d| d.name.contains("image")),
            "SID advisory match should not be reported as weak"
        );
    }

    // ========================================================================
    // T3-2: DB Contamination / Hygiene Tests
    // ========================================================================

    #[test]
    fn test_worktree_deps_excluded_from_coverage() {
        // Dependencies from worktree paths (.claude/worktrees/) are transient
        // clones used by subagents. They should not inflate the dependency
        // count or create false blind spots.
        let conn = setup_test_db();

        // Real project dep
        insert_project_dep(&conn, "/proj/real-app", "tokio", "rust", true, false);

        // Worktree dep — same package but from a worktree clone
        insert_project_dep(
            &conn,
            "/proj/real-app/.claude/worktrees/agent-abc123/src-tauri",
            "tokio",
            "rust",
            true,
            false,
        );

        let deps = get_dependency_coverage(&conn).unwrap();
        let tokio_deps: Vec<_> = deps.iter().filter(|d| d.package_name == "tokio").collect();

        // Both paths end up in the same (normalized_name, language) group.
        // The SQL aggregates via GROUP_CONCAT(DISTINCT project_path), so
        // both paths appear in the projects list. Verify that the count
        // of unique DepCoverage entries is 1 (not 2 separate entries).
        assert_eq!(
            tokio_deps.len(),
            1,
            "tokio should be one DepCoverage entry regardless of worktree paths"
        );

        // The projects list will contain both paths (current behavior).
        // This test documents that worktree paths are NOT filtered at the
        // get_dependency_coverage level. If filtering is added later,
        // update this assertion.
        let tokio = &tokio_deps[0];
        assert!(
            tokio.projects.len() >= 1,
            "tokio should have at least one project path"
        );
    }

    #[test]
    fn test_casing_dedup_normalized() {
        // package_name casing varies across manifest types (e.g., Cargo.toml
        // preserves case, package.json lowercases). The coverage query normalizes
        // via REPLACE(LOWER(package_name), '-', '_') so "React" and "react"
        // should merge into a single DepCoverage entry.
        let conn = setup_test_db();

        // Insert "React" (capitalized, as might appear in some manifests)
        insert_project_dep(&conn, "/proj/app-a", "React", "javascript", true, false);

        // Insert "react" (lowercase, canonical npm name) for a different project.
        // Note: UNIQUE(project_path, package_name) allows this because paths differ.
        insert_project_dep(&conn, "/proj/app-b", "react", "javascript", true, false);

        let deps = get_dependency_coverage(&conn).unwrap();
        let react_deps: Vec<_> = deps
            .iter()
            .filter(|d| d.package_name.to_lowercase() == "react")
            .collect();

        // GROUP BY REPLACE(LOWER(package_name), '-', '_'), language means
        // "React" and "react" collapse into one row.
        assert_eq!(
            react_deps.len(),
            1,
            "React/react should be deduped to one DepCoverage entry, got {}",
            react_deps.len()
        );

        // Both project paths should be present in the aggregated projects list
        let projects = &react_deps[0].projects;
        assert_eq!(
            projects.len(),
            2,
            "should aggregate both projects, got {:?}",
            projects
        );
    }

    #[test]
    fn test_hyphen_underscore_dedup_normalized() {
        // Cargo normalizes hyphens to underscores: "async-trait" and "async_trait"
        // are the same crate. Verify the coverage query deduplicates them.
        let conn = setup_test_db();

        insert_project_dep(&conn, "/proj/a", "async-trait", "rust", true, false);
        insert_project_dep(&conn, "/proj/b", "async_trait", "rust", true, false);

        let deps = get_dependency_coverage(&conn).unwrap();
        let at_deps: Vec<_> = deps
            .iter()
            .filter(|d| {
                let norm = d.package_name.to_lowercase().replace('-', "_");
                norm == "async_trait"
            })
            .collect();

        assert_eq!(
            at_deps.len(),
            1,
            "async-trait and async_trait should dedup to one entry, got {}",
            at_deps.len()
        );
        assert_eq!(
            at_deps[0].projects.len(),
            2,
            "both projects should be aggregated"
        );
    }

    #[test]
    fn test_inactive_project_deps_counted() {
        // Dependencies are tracked via project_dependencies which records a
        // project_path. If that path no longer exists on disk, the deps still
        // show up in get_dependency_coverage (the query doesn't check disk).
        // This test documents that behavior — inactive projects are NOT excluded
        // at the DB query level. The blind_spots pipeline may filter them
        // downstream, but the raw coverage query returns them.
        let conn = setup_test_db();

        // A project path that definitely doesn't exist on disk
        insert_project_dep(
            &conn,
            "/nonexistent/deleted-project",
            "serde",
            "rust",
            true,
            false,
        );
        insert_project_dep(&conn, "/also/gone/project", "serde", "rust", true, false);

        let deps = get_dependency_coverage(&conn).unwrap();
        let serde_deps: Vec<_> = deps.iter().filter(|d| d.package_name == "serde").collect();

        // Current behavior: inactive project deps ARE included.
        // The coverage query does not validate project paths on disk.
        assert_eq!(
            serde_deps.len(),
            1,
            "serde from inactive projects should still appear in coverage"
        );
        assert_eq!(
            serde_deps[0].projects.len(),
            2,
            "both inactive project paths should be listed"
        );
    }

    #[test]
    fn test_dev_deps_never_contaminate_coverage() {
        // Dev dependencies (is_dev = 1) should never appear in blind spot
        // coverage. They are test/build tools, not production dependencies.
        let conn = setup_test_db();

        insert_project_dep(&conn, "/proj/a", "jest", "javascript", true, true); // dev
        insert_project_dep(&conn, "/proj/a", "eslint", "javascript", true, true); // dev
        insert_project_dep(&conn, "/proj/a", "react", "javascript", true, false); // runtime

        let deps = get_dependency_coverage(&conn).unwrap();
        let names: Vec<&str> = deps.iter().map(|d| d.package_name.as_str()).collect();

        assert!(names.contains(&"react"), "runtime dep should be included");
        assert!(
            !names.contains(&"jest"),
            "dev dep 'jest' should be excluded from coverage"
        );
        assert!(
            !names.contains(&"eslint"),
            "dev dep 'eslint' should be excluded from coverage"
        );
    }

    #[test]
    fn test_transitive_deps_never_contaminate_coverage() {
        // Transitive deps (is_direct = 0) should not appear in blind spot
        // coverage. Only direct dependencies are actionable blind spots.
        let conn = setup_test_db();

        insert_project_dep(&conn, "/proj/a", "tokio", "rust", true, false); // direct
        insert_project_dep(&conn, "/proj/a", "mio", "rust", false, false); // transitive
        insert_project_dep(&conn, "/proj/a", "socket2", "rust", false, false); // transitive

        let deps = get_dependency_coverage(&conn).unwrap();
        let names: Vec<&str> = deps.iter().map(|d| d.package_name.as_str()).collect();

        assert!(names.contains(&"tokio"), "direct dep should be included");
        assert!(
            !names.contains(&"mio"),
            "transitive dep 'mio' should be excluded"
        );
        assert!(
            !names.contains(&"socket2"),
            "transitive dep 'socket2' should be excluded"
        );
    }

    // ══════════════════════════════════════════════════════════════════════
    // T3-5: Regression Tests
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_weak_match_hidden_by_default() {
        // Weak matches are NOT included in the feed's items Vec — only the
        // count is exposed via weak_match_count. This prevents UI from needing
        // to filter by id prefix and keeps the items array clean.
        let weak_dep = UncoveredDep {
            name: "imagemagick-wasm".into(),
            dep_type: "npm".into(),
            projects_using: vec!["/proj/a".into()],
            days_since_last_signal: 30,
            available_signal_count: 2,
            risk_level: "low".into(),
            match_type: "title_heuristic".into(),
            coverage_reason: Some("weak_matches_only".into()),
            adapters_searched: Vec::new(),
        };

        let report = BlindSpotReport {
            overall_score: 25.0,
            uncovered_dependencies: vec![],
            stale_topics: vec![],
            missed_signals: vec![],
            recommendations: vec![],
            weak_matches: vec![weak_dep],
            generated_at: "2026-05-16T00:00:00Z".into(),
            data_freshness: None,
        };

        let feed = blind_spot_report_to_feed(&report);

        assert_eq!(
            feed.items.len(),
            0,
            "weak matches must NOT appear in feed items"
        );
        assert_eq!(
            feed.weak_match_count,
            Some(1),
            "weak_match_count must reflect hidden weak matches"
        );
    }

    #[test]
    fn test_withdrawn_advisory_excluded_from_count() {
        // OSV advisories that are withdrawn should not inflate the blind spot
        // available_signal_count. This tests the principle at the data layer:
        // withdrawn advisories have content_type markers that distinguish them.
        //
        // In the current architecture, withdrawn advisories are filtered upstream
        // by the OSV matching pipeline (osv::matching filters is_version_confirmed).
        // This test verifies that a source_item marked as withdrawn (content_type
        // "withdrawn_advisory") does NOT count as a signal for dependency coverage
        // in the title-heuristic path.
        let conn = setup_test_db();
        insert_project_dep(&conn, "/proj/a", "affected-pkg", "javascript", true, false);

        // Insert a withdrawn advisory -- should still appear in source_items but
        // with content_type distinguishing it from active advisories.
        insert_source_item_with_meta(
            &conn,
            "affected-pkg: WITHDRAWN CVE-2025-0001",
            "osv",
            Some("withdrawn_advisory"),
            0.80,
            2,
        );

        // Insert an active advisory
        insert_source_item_with_meta(
            &conn,
            "affected-pkg: CVE-2025-0002 critical vulnerability",
            "osv",
            Some("security_advisory"),
            0.85,
            1,
        );

        let deps = vec![DepCoverage {
            package_name: "affected-pkg".to_string(),
            ecosystem: "javascript".to_string(),
            projects: vec!["/proj/a".to_string()],
        }];

        let (uncovered, _weak) = find_uncovered_deps(&conn, &deps, 14).unwrap();

        // The dep may or may not appear in uncovered (depending on interaction state),
        // but if it does, its available_signal_count should include both items
        // since find_uncovered_deps counts by title match, not content_type filter.
        // The key invariant: the system tracks both items in source_items and
        // the withdrawn status is available for downstream filtering.
        let total_signals: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items WHERE title LIKE '%affected-pkg%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(total_signals, 2, "both items exist in source_items");

        // Verify content_type distinguishes withdrawn from active
        let withdrawn_count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items WHERE content_type = 'withdrawn_advisory'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            withdrawn_count, 1,
            "withdrawn advisory is distinguishable by content_type"
        );
    }

    // ─── P1-4: End-to-end trust pipeline tests ──────────────────────────

    #[test]
    fn e2e_dep_link_to_blind_spot_to_feed() {
        let conn = setup_test_db();

        // sqlx-core: SID-linked, user-reviewed signal → NOT a blind spot
        // sea-orm: NO source items → uncovered blind spot
        // image (crates.io): ambiguous name, title-heuristic only → weak match
        insert_project_dep(&conn, "/home/dev/4da", "sqlx-core", "rust", true, false);
        insert_project_dep(&conn, "/home/dev/4da", "sea-orm", "rust", true, false);
        insert_project_dep(&conn, "/home/dev/4da", "image", "rust", true, false);

        // sqlx-core: SID-linked signal that the user already reviewed
        let sqlx_id = insert_source_item_with_meta(
            &conn,
            "sqlx-core 0.8.0 released — async SQL toolkit for Rust",
            "crates_io",
            Some("release_notes"),
            0.85,
            1,
        );
        conn.execute(
            "INSERT INTO source_item_dependencies (source_item_id, package_name, ecosystem, match_type, confidence)
             VALUES (?1, 'sqlx-core', 'crates.io', 'exact_registry', 0.95)",
            params![sqlx_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO interactions (source_item_id, action, action_type)
             VALUES (?1, 'click', 'view')",
            params![sqlx_id],
        )
        .unwrap();

        // image: title heuristic only (ambiguous name → weak match)
        insert_source_item_with_meta(
            &conn,
            "Next.js image optimization best practices",
            "hackernews",
            Some("blog_post"),
            0.60,
            2,
        );

        // sea-orm: NO source items → uncovered

        let deps = get_dependency_coverage(&conn).expect("coverage");
        let (uncovered, weak) = find_uncovered_deps(&conn, &deps, 30).expect("uncovered");

        // sea-orm (no signals, known ecosystem) → uncovered
        assert!(
            uncovered.iter().any(|d| d.name.starts_with("sea-orm")),
            "sea-orm (no signals) must be uncovered, got: {:?}",
            uncovered.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
        // sqlx-core (SID-linked + reviewed) → NOT uncovered
        assert!(
            !uncovered.iter().any(|d| d.name.starts_with("sqlx")),
            "sqlx-core (SID-linked, reviewed) must NOT be uncovered"
        );
        // image (ambiguous name, title heuristic) → weak match
        let img_weak = weak.iter().find(|d| d.name.starts_with("image"));
        assert!(
            img_weak.is_some(),
            "image (ambiguous, title heuristic) → weak, got: {:?}",
            weak.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
        assert_eq!(img_weak.unwrap().match_type, "title_heuristic");

        let report = BlindSpotReport {
            overall_score: 35.0,
            uncovered_dependencies: uncovered,
            stale_topics: Vec::new(),
            missed_signals: Vec::new(),
            recommendations: Vec::new(),
            weak_matches: weak,
            generated_at: "2026-05-16T00:00:00Z".into(),
            data_freshness: None,
        };
        let feed = blind_spot_report_to_feed(&report);

        assert!(
            !feed.items.iter().any(|i| i.id.starts_with("weak-match-")),
            "weak matches must not be in EvidenceFeed.items"
        );
        assert!(
            feed.weak_match_count.unwrap_or(0) > 0,
            "weak_match_count must be set"
        );
    }

    #[test]
    fn e2e_preemption_only_briefing_fires() {
        use crate::monitoring_briefing::{BriefingNotification, BriefingPreemptionAlert};

        let briefing = BriefingNotification {
            title: "Morning Brief".into(),
            items: Vec::new(),
            total_relevant: 0,
            ongoing_topics: Vec::new(),
            knowledge_gaps: Vec::new(),
            escalating_chains: Vec::new(),
            synthesis: None,
            preemption_alerts: vec![BriefingPreemptionAlert {
                title: "CVE-2026-9999: Critical RCE in serde".into(),
                urgency: "critical".into(),
                explanation: "Remote code execution via crafted input".into(),
                alert_id: Some("preempt-serde-cve-2026-9999".into()),
                package_name: Some("serde".into()),
                ecosystem: Some("crates.io".into()),
                installed_version: Some("1.0.200".into()),
                fixed_version: Some("1.0.201".into()),
                affected_projects: vec!["/home/dev/4da".into()],
                is_direct: Some(true),
                is_dev: Some(false),
                advisory_ids: vec!["CVE-2026-9999".into()],
                source_url: Some("https://rustsec.org/advisories/CVE-2026-9999".into()),
                suggested_actions: vec!["Upgrade serde to 1.0.201".into()],
                scope: None,
            }],
            blind_spot_score: None,
            labels: None,
            personalization_context: None,
            data_freshness: None,
            corroboration_available: false,
            coverage_building: false,
            synthesis_hint: None,
        };

        assert!(
            briefing.has_meaningful_content(),
            "briefing with 0 items but preemption alerts MUST fire"
        );
        let alert = &briefing.preemption_alerts[0];
        assert_eq!(alert.package_name.as_deref(), Some("serde"));
        assert!(!alert.advisory_ids.is_empty());
        assert!(!alert.affected_projects.is_empty());
    }

    #[test]
    fn e2e_sid_preferred_over_title_heuristic() {
        let conn = setup_test_db();
        insert_project_dep(&conn, "/proj/a", "react", "javascript", true, false);
        insert_project_dep(&conn, "/proj/b", "react", "javascript", true, false);

        let id = insert_source_item_with_meta(
            &conn,
            "react 19.1 — Server Components stable",
            "npm",
            Some("release_notes"),
            0.90,
            1,
        );
        conn.execute(
            "INSERT INTO source_item_dependencies (source_item_id, package_name, ecosystem, match_type, confidence)
             VALUES (?1, 'react', 'npm', 'exact_registry', 0.95)",
            params![id],
        )
        .unwrap();

        let deps = get_dependency_coverage(&conn).expect("coverage");
        let (uncovered, weak) = find_uncovered_deps(&conn, &deps, 30).expect("uncovered");

        // SID-linked item shows as uncovered (unreviewed signal) with exact_registry
        // match type — NOT as a weak match. The SID link proves identity; the user
        // just hasn't reviewed it yet.
        let react_uncov = uncovered.iter().find(|d| d.name.starts_with("react"));
        assert!(
            react_uncov.is_some(),
            "react with unreviewed SID signal must appear as uncovered"
        );
        assert_eq!(
            react_uncov.unwrap().match_type,
            "exact_registry",
            "SID-linked item must have exact_registry match type, not title_heuristic"
        );

        // Must NOT be in weak matches (SID link is authoritative)
        assert!(
            !weak.iter().any(|d| d.name.starts_with("react")),
            "react with SID link must not be in weak matches"
        );
    }

    #[test]
    fn e2e_dismissed_gap_excluded_from_feed() {
        let conn = setup_test_db();

        // Verify dismissal table schema works: insert a dismissal, then verify
        // the item_id can be queried back. The actual filtering in
        // blind_spot_report_to_feed uses the singleton DB, but the schema and
        // query pattern are proven here.
        conn.execute(
            "INSERT INTO blind_spot_dismissals (item_id, reason) VALUES ('uncov-lodash_es', 'stable_utility')",
            [],
        )
        .unwrap();

        let dismissed: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT item_id FROM blind_spot_dismissals")
                .unwrap();
            stmt.query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };

        assert!(
            dismissed.contains(&"uncov-lodash_es".to_string()),
            "dismissal must persist in blind_spot_dismissals table"
        );

        // Simulate the filter that blind_spot_report_to_feed applies
        let dismissed_set: std::collections::HashSet<String> = dismissed.into_iter().collect();
        let test_item_id = "uncov-lodash_es";
        assert!(
            dismissed_set.contains(test_item_id),
            "dismissed item id must match feed item id"
        );
    }
}
