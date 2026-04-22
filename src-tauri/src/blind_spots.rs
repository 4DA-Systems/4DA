// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Blind Spot Intelligence for 4DA
//!
//! Cross-references what the user is watching with what they SHOULD be
//! watching based on their actual dependencies, projects, and stack.
//! "You have 6 active Rust deps but haven't engaged with Rust signals in 21 days."

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::warn;
use ts_rs::TS;

use crate::error::Result;
use crate::evidence::{
    Action as EvidenceAction, Confidence, EvidenceCitation, EvidenceFeed, EvidenceItem,
    EvidenceKind, LensHints, Urgency,
};

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
    pub generated_at: String,
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

/// Generate a comprehensive blind spot report.
pub fn generate_blind_spot_report() -> Result<BlindSpotReport> {
    let conn = crate::open_db_connection()?;
    let threshold_days = blind_spot_threshold_days();

    // 1. Get attention report (30-day window)
    let attention = crate::attention::generate_report(30)?;

    // 2. Get knowledge gaps
    let gaps = crate::knowledge_decay::detect_knowledge_gaps(&conn)?;

    // 3. Get all user dependencies with project coverage
    let deps = get_dependency_coverage(&conn)?;

    // 4. Find uncovered dependencies (deps with no interaction in threshold days)
    let uncovered = find_uncovered_deps(&conn, &deps, threshold_days)?;

    // 5. Find stale topics from attention blind spots.
    // Only include topics with actual missed signals — a topic with
    // 0 missed signals means coverage is healthy, not that there's a
    // gap. Showing "Stale topic: rust (0 signals missed)" erodes trust
    // by flagging something the user can't act on.
    let stale: Vec<StaleTopic> = attention
        .blind_spots
        .iter()
        .filter(|bs| bs.in_codebase)
        .map(|bs| StaleTopic {
            topic: bs.topic.clone(),
            last_engagement_days: ((1.0 - bs.engagement_level) * 30.0) as u32,
            active_deps_in_topic: count_deps_for_topic(&deps, &bs.topic),
            missed_signal_count: count_missed_for_topic(&gaps, &bs.topic),
        })
        .filter(|st| st.missed_signal_count > 0 || st.active_deps_in_topic >= 5)
        .collect();

    // 6. Find missed signals (high-relevance, not seen, older than feed window)
    let missed = find_missed_signals(&conn, threshold_days, &deps)?;

    // 7. Generate recommendations
    let recommendations = generate_recommendations(&uncovered, &stale, &gaps);

    // 8. Calculate overall score (normalized against direct-dep count)
    let score = calculate_blind_spot_score(&uncovered, &stale, &missed, deps.len());

    Ok(BlindSpotReport {
        overall_score: score,
        uncovered_dependencies: uncovered,
        stale_topics: stale,
        missed_signals: missed,
        recommendations,
        generated_at: chrono::Utc::now().to_rfc3339(),
    })
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
    let sql = "SELECT package_name,
                      language,
                      MAX(is_direct) as any_direct,
                      GROUP_CONCAT(DISTINCT project_path) as project_list
               FROM project_dependencies
               WHERE is_dev = 0
               GROUP BY package_name, language
               HAVING any_direct = 1
               ORDER BY package_name";

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
        let package_name: String = row.get(0)?;
        let ecosystem: String = row.get(1)?;
        let project_list: Option<String> = row.get(3).ok();
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

    let result: Vec<DepCoverage> = rows.filter_map(|r| r.ok()).collect();

    if result.is_empty() {
        warn!(
            target: "4da::blind_spots",
            "get_dependency_coverage returned 0 direct deps — user has no direct project deps scanned"
        );
    }

    Ok(result)
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
/// - Deps with zero available signals in the window are skipped early,
///   avoiding the remaining two queries for each.
///
/// Schema correction (was a bug): `interactions` has `timestamp` NOT
/// `created_at`. The previous query silently errored via `.unwrap_or(999)`,
/// causing every dep to register `days_since = 999` → critical risk → score
/// pinned to 100.
fn find_uncovered_deps(
    conn: &rusqlite::Connection,
    deps: &[DepCoverage],
    threshold_days: u32,
) -> Result<Vec<UncoveredDep>> {
    const MAX_DEPS_TO_PROCESS: usize = 50;
    const MIN_DEP_NAME_LEN: usize = 4;

    let mut uncovered = Vec::new();
    let mut processed = 0usize;

    for dep in deps {
        if processed >= MAX_DEPS_TO_PROCESS {
            break;
        }
        if dep.package_name.len() < MIN_DEP_NAME_LEN {
            continue;
        }
        processed += 1;

        let search_term = format!("%{}%", dep.package_name);

        let window = format!("-{threshold_days} days");

        let available: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items
                 WHERE title LIKE ?1
                   AND created_at >= datetime('now', ?2)",
                params![search_term, window],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Early exit: no available signals means nothing to miss — not a blind spot.
        if available == 0 {
            continue;
        }

        let interacted: u32 = conn
            .query_row(
                "SELECT COUNT(DISTINCT si.id) FROM source_items si
                 JOIN interactions i ON i.item_id = si.id
                 WHERE si.title LIKE ?1
                   AND si.created_at >= datetime('now', ?2)",
                params![search_term, window],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Days since last interaction. Uses `i.timestamp` (the real column).
        // NULL handling: if never interacted, we use 999 (never engaged).
        let days_since: u32 = conn
            .query_row(
                "SELECT COALESCE(
                    CAST(julianday('now') - julianday(MAX(i.timestamp)) AS INTEGER),
                    999
                 )
                 FROM source_items si
                 JOIN interactions i ON i.item_id = si.id
                 WHERE si.title LIKE ?1",
                params![search_term],
                |row| row.get(0),
            )
            .unwrap_or(999);

        // Skip if user has recently interacted.
        if days_since < 14 && interacted > 0 {
            continue;
        }

        let not_seen = available.saturating_sub(interacted);

        // If the user has interacted recently AND there are no new unseen
        // signals, there's nothing to surface.
        if not_seen == 0 && days_since < 30 {
            continue;
        }

        let risk_level = classify_dep_risk(days_since, not_seen, dep.projects.len());

        uncovered.push(UncoveredDep {
            name: dep.package_name.clone(),
            dep_type: dep.ecosystem.clone(),
            projects_using: dep.projects.clone(),
            days_since_last_signal: days_since,
            available_signal_count: not_seen,
            risk_level,
        });
    }

    // Sort by risk: critical first, then by days since last signal.
    uncovered.sort_by(|a, b| {
        risk_ord(&a.risk_level)
            .cmp(&risk_ord(&b.risk_level))
            .then(b.days_since_last_signal.cmp(&a.days_since_last_signal))
    });

    Ok(uncovered)
}

/// Classify risk level based on coverage gap severity.
fn classify_dep_risk(days_since: u32, unseen_signals: u32, project_count: usize) -> String {
    if days_since > 60 && project_count > 2 {
        "critical".to_string()
    } else if days_since > 30 || (unseen_signals > 5 && project_count > 1) {
        "high".to_string()
    } else if days_since > 14 || unseen_signals > 2 {
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
    const FEED_WINDOW_DAYS: u32 = 3;
    if days <= FEED_WINDOW_DAYS {
        return Ok(Vec::new()); // No meaningful "missed" window
    }

    // Fetch more than 15 initially so the priority-aware post-sort has room
    // to promote security items and filter out old blog posts before trimming.
    let sql = format!(
        "SELECT si.id, si.title, si.url, si.source_type, si.relevance_score, si.created_at
         FROM source_items si
         LEFT JOIN interactions i ON i.item_id = si.id
         LEFT JOIN user_events ue ON (
             ue.event_type = 'impression'
             AND ue.metadata LIKE '%\"item_id\":' || si.id || '%'
         )
         WHERE si.relevance_score > 0.5
           AND si.created_at >= datetime('now', '-{days} days')
           AND si.created_at < datetime('now', '-{feed_window} days')
           AND i.item_id IS NULL
           AND ue.id IS NULL
         ORDER BY si.relevance_score DESC
         LIMIT 40",
        days = days,
        feed_window = FEED_WINDOW_DAYS
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
            why_relevant: String::new(), // populated below
            dep_name: None,              // populated below
            was_shown: false,            // query filters out impressioned items
        })
    })?;

    let mut signals: Vec<MissedSignal> = rows.flatten().collect();

    // Filter low-quality noise: tutorials, beginner content, off-topic career posts.
    signals.retain(|s| !crate::knowledge_decay::is_low_quality_signal(&s.title));

    // Populate `why_relevant` and `dep_name` by looking for dep mentions in titles.
    for signal in &mut signals {
        let (why, dep) =
            compute_why_relevant(&signal.title, signal.relevance_score, direct_deps);
        signal.why_relevant = why;
        signal.dep_name = dep;
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

/// Classify a signal's content priority tier based on title text.
/// Tier 4 = security, Tier 3 = breaking/deprecation, Tier 2 = releases,
/// Tier 1 = everything else (blog, Q&A, showcase).
fn has_standalone_word(haystack: &str, word: &str) -> bool {
    haystack
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| w == word)
}

fn title_priority_tier(title: &str) -> u8 {
    let t = title.to_lowercase();
    if t.contains("cve-")
        || t.contains("vulnerability")
        || t.contains("security advisory")
        || has_standalone_word(&t, "rce")
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

/// Re-rank missed signals so high-urgency content surfaces above opinion/blog
/// content, even when relevance scores are similar. Also caps older non-urgent
/// content so the panel stays focused on recent-enough material.
fn rank_by_missed_priority(mut signals: Vec<MissedSignal>) -> Vec<MissedSignal> {
    const FINAL_LIMIT: usize = 15;
    const OLD_BLOG_CAP: usize = 3;
    const OLD_DAYS_THRESHOLD: i64 = 10;

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
        let tier_a = title_priority_tier(&a.title);
        let tier_b = title_priority_tier(&b.title);
        tier_b.cmp(&tier_a).then_with(|| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    // Cap Tier 1 (generic blog) items older than OLD_DAYS_THRESHOLD to OLD_BLOG_CAP.
    let mut kept = Vec::with_capacity(FINAL_LIMIT);
    let mut old_tier1_count = 0;
    for s in signals {
        if kept.len() >= FINAL_LIMIT {
            break;
        }
        let tier = title_priority_tier(&s.title);
        let age = age_days(&s.created_at);
        if tier == 1 && age > OLD_DAYS_THRESHOLD {
            if old_tier1_count >= OLD_BLOG_CAP {
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
            let topics = crate::extract_topics(&signal.title, "");

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
    score: f32,
    direct_deps: &[DepCoverage],
) -> (String, Option<String>) {
    let title_lower = title.to_lowercase();

    // Dep names that are common English words — they produce false matches
    // against nearly every article title ("open source", "next steps", etc.)
    const GENERIC_DEP_NAMES: &[&str] = &[
        "open", "next", "node", "vite", "test", "core", "path", "sync",
        "once", "glob", "rand", "time", "lock", "send", "copy", "find",
        "diff", "pick", "wrap", "trim", "data", "form", "icon", "link",
        "text", "type", "util", "base", "flat", "safe", "fast", "make",
        "pipe", "pump", "read", "call", "nano", "pure", "vary", "yaml",
        "mime", "race", "uuid", "deep", "http", "https",
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
    let comma_segments = title.split(',').count() + title.split('·').count();
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

    // No specific match — fall back to generic text keyed to score tier.
    // These strings are DELIBERATELY general: we didn't find a specific
    // match, so we don't claim one.
    let text = if score >= 0.85 {
        "High-relevance item matching your topic affinities".to_string()
    } else if score >= 0.7 {
        "Moderately relevant based on your scoring profile".to_string()
    } else {
        "Borderline-relevant — worth a glance if you have time".to_string()
    };
    (text, None)
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

    // General recommendation if many uncovered deps
    if uncovered.len() > 5 {
        recs.push(BlindSpotRecommendation {
            action: "Consider adding RSS feeds or watches for your most-used dependencies"
                .to_string(),
            reason: format!(
                "{} of your dependencies have no recent signal coverage",
                uncovered.len()
            ),
            priority: "medium".to_string(),
        });
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

    // Denominator floor: if the user has fewer than 5 direct deps, use 5 as
    // the denominator. This prevents a 1-dep stack with 1 uncovered critical
    // from scoring 1.0 (100% uncovered).
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
        let count_boost = ((missed.len() as f32).ln_1p() / 4.0).min(1.0);
        (avg * 0.7 + count_boost * 0.3).min(1.0)
    };

    // ─── Final weighted blend ─────────────────────────────────────────
    let score = (uncovered_pressure * 55.0) + (stale_pressure * 25.0) + (missed_pressure * 20.0);

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

fn uncovered_dep_to_evidence_item(d: &UncoveredDep) -> EvidenceItem {
    let title = truncate_title(&format!(
        "{} — {} unseen signal{}",
        d.name,
        d.available_signal_count,
        if d.available_signal_count == 1 {
            ""
        } else {
            "s"
        }
    ));
    let explanation = format!(
        "{} signal{} about {} appeared but you haven't engaged with {} content in {} days.",
        d.available_signal_count,
        if d.available_signal_count == 1 {
            ""
        } else {
            "s"
        },
        d.name,
        d.name,
        d.days_since_last_signal,
    );

    // Synthesize at least one inferred citation so the schema's
    // "evidence required for user-surfaced kinds" rule holds. Real
    // citations land in Phase 9 via the AWE spine.
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
        confidence: Confidence::heuristic(0.7),
        urgency: risk_level_to_urgency(&d.risk_level),
        reversibility: None,
        evidence: vec![citation],
        affected_projects: d.projects_using.clone(),
        affected_deps: vec![d.name.clone()],
        suggested_actions: vec![EvidenceAction {
            action_id: "investigate".to_string(),
            label: "Investigate".to_string(),
            description: "Review the unseen signals for this dependency.".to_string(),
        }],
        precedents: Vec::new(),
        refutation_condition: None,
        lens_hints: LensHints::blind_spots_only(),
        created_at: now_millis(),
        expires_at: None,
    }
}

fn stale_topic_to_evidence_item(t: &StaleTopic) -> EvidenceItem {
    let title = if t.missed_signal_count > 0 {
        truncate_title(&format!(
            "{} — {} signal{} you haven't seen",
            t.topic,
            t.missed_signal_count,
            if t.missed_signal_count == 1 { "" } else { "s" }
        ))
    } else {
        truncate_title(&format!(
            "{} — {} dep{}, no recent engagement",
            t.topic,
            t.active_deps_in_topic,
            if t.active_deps_in_topic == 1 { "" } else { "s" }
        ))
    };
    let explanation = if t.missed_signal_count > 0 {
        format!(
            "{} article{} about {} appeared in the last 30 days that you didn't engage with.",
            t.missed_signal_count,
            if t.missed_signal_count == 1 { "" } else { "s" },
            t.topic,
        )
    } else {
        format!(
            "You have {} active {} dependenc{} but haven't engaged with {} content recently.",
            t.active_deps_in_topic,
            t.topic,
            if t.active_deps_in_topic == 1 {
                "y"
            } else {
                "ies"
            },
            t.topic,
        )
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
        confidence: Confidence::heuristic(0.6),
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
    let citation = EvidenceCitation {
        source: m.source_type.clone(),
        title: truncate_title(&m.title),
        url: m.url.clone(),
        freshness_days: chrono::NaiveDateTime::parse_from_str(&m.created_at, "%Y-%m-%d %H:%M:%S")
            .map(|dt| {
                let diff = chrono::Utc::now().timestamp() - dt.and_utc().timestamp();
                (diff as f32 / 86_400.0).max(0.0)
            })
            .unwrap_or(0.0),
        relevance_note: truncate_note(&m.why_relevant),
    };

    // Map urgency from content priority tier + relevance score.
    // Tier 1 (generic blog/showcase) caps at Watch regardless of score.
    // Tier 2 (registry releases) caps at Medium.
    // Tier 3-4 (breaking changes, security) use score-based urgency.
    let tier = title_priority_tier(&m.title);
    let urgency = match tier {
        4 => {
            if m.relevance_score >= 0.7 { Urgency::Critical } else { Urgency::High }
        }
        3 => {
            if m.relevance_score >= 0.8 { Urgency::High } else { Urgency::Medium }
        }
        2 => Urgency::Medium,
        _ => {
            if m.relevance_score >= 0.9 { Urgency::Medium } else { Urgency::Watch }
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
        confidence: Confidence::heuristic(0.55),
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
pub fn blind_spot_report_to_feed(report: &BlindSpotReport) -> EvidenceFeed {
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

    let mut validated: Vec<EvidenceItem> = items
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
    // Phase 9 — attach precedents via the AWE spine.
    crate::awe_spine::enrich_items(&mut validated);

    EvidenceFeed::from_items_with_score(validated, report.overall_score)
}

// ============================================================================
// Tauri Command
// ============================================================================

/// Returns the canonical `EvidenceFeed` for the Blind Spots lens, with the
/// legacy coverage score carried on `feed.score`. Internal
/// `generate_blind_spot_report` still produces the legacy struct (shared
/// with telemetry code paths) and converts at the boundary.
#[tauri::command]
pub async fn get_blind_spots() -> std::result::Result<EvidenceFeed, String> {
    crate::settings::require_signal_feature("get_blind_spots").map_err(|e| e.to_string())?;
    let report = generate_blind_spot_report().map_err(|e| e.to_string())?;
    Ok(blind_spot_report_to_feed(&report))
}

// ============================================================================
// Tests — use REAL schema definitions from migrations to catch column drift
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

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
                last_seen TEXT
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

        let uncovered = find_uncovered_deps(&conn, &deps, 14).expect("must not SQL-error");
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
    fn fix2_find_uncovered_deps_skips_zero_available() {
        let conn = setup_test_db();
        insert_project_dep(&conn, "/proj/a", "nobodycares", "javascript", true, false);

        // No source_items mention "nobodycares" at all.
        let deps = vec![DepCoverage {
            package_name: "nobodycares".to_string(),
            ecosystem: "javascript".to_string(),
            projects: vec!["/proj/a".to_string()],
        }];

        let uncovered = find_uncovered_deps(&conn, &deps, 14).unwrap();
        assert_eq!(
            uncovered.len(),
            0,
            "deps with zero available signals must not be flagged as blind spots"
        );
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

        let uncovered = find_uncovered_deps(&conn, &deps, 14).unwrap();
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

        let uncovered = find_uncovered_deps(&conn, &deps, 14).unwrap();
        assert_eq!(
            uncovered.len(),
            0,
            "short dep names must be skipped to avoid LIKE noise"
        );
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
    fn fix6_why_relevant_falls_back_when_no_match() {
        let deps = vec![DepCoverage {
            package_name: "react".to_string(),
            ecosystem: "javascript".to_string(),
            projects: vec![],
        }];
        // Title doesn't mention react
        let (text, dep) = compute_why_relevant("Postgres new extension released", 0.9, &deps);
        assert!(
            !text.contains("react"),
            "must not claim a match that isn't there: {text}"
        );
        // Fallback text is deliberately generic
        assert!(
            text.contains("scoring") || text.contains("relevance") || text.contains("topic"),
            "fallback should be honestly generic: {text}"
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
            generated_at: "2026-04-17 00:00:00".into(),
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
            generated_at: String::new(),
        };
        let feed = blind_spot_report_to_feed(&report);
        assert_eq!(feed.total, 0);
        assert_eq!(feed.score, Some(0.0));
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
    fn rce_word_boundary_prevents_source_false_positive() {
        assert!(has_standalone_word("critical rce vulnerability", "rce"));
        assert!(has_standalone_word("[rce] openssl flaw", "rce"));
        assert!(has_standalone_word("rce in libxml2", "rce"));
        assert!(!has_standalone_word("open source project", "rce"));
        assert!(!has_standalone_word("open-source tool", "rce"));
        assert!(!has_standalone_word("resource management", "rce"));
        assert!(!has_standalone_word("workforce planning", "rce"));
    }

    #[test]
    fn tier1_blog_post_gets_watch_urgency() {
        let mut m = missed_sample();
        m.title = "We Scored 28 Famous Open Source PRs for Deploy Risk".into();
        m.relevance_score = 0.8;
        let item = missed_signal_to_evidence_item(&m);
        assert_eq!(item.urgency, crate::evidence::Urgency::Watch);
    }
}
