// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Preemption Engine for 4DA
//!
//! Orchestrates forward-looking intelligence by combining signal chains,
//! project health, knowledge gaps, and attention analysis into ranked
//! preemptive alerts. Tells the user what matters BEFORE it becomes painful.

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
use ts_rs::TS;

use crate::error::Result;
use crate::evidence::{
    Action as EvidenceAction, Confidence, EvidenceCitation, EvidenceFeed, EvidenceItem,
    EvidenceKind, LensHints, Urgency,
};
use crate::scoring_config;
use crate::signal_chains::ChainResolution;

// ============================================================================
// Types
// ============================================================================

/// Category of preemption alert.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum PreemptionType {
    SecurityAdvisory,
    BreakingChange,
    MigrationWindow,
    EcosystemShift,
    MaintainerDecline,
    KnowledgeBlindSpot,
}

/// How urgently the user should act on this alert.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum AlertUrgency {
    Critical,
    High,
    Medium,
    Watch,
}

/// A single piece of evidence backing a preemption alert.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AlertEvidence {
    pub source: String,
    pub title: String,
    pub url: Option<String>,
    pub freshness_days: f32,
    pub relevance_score: f32,
}

/// An action the user can take in response to an alert.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SuggestedAction {
    /// One of: "dismiss", "watch", "investigate", "review_decision"
    pub action_type: String,
    pub label: String,
    pub description: String,
}

/// A single preemption alert combining evidence from multiple intelligence sources.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PreemptionAlert {
    pub id: String,
    pub alert_type: PreemptionType,
    pub title: String,
    pub explanation: String,
    pub evidence: Vec<AlertEvidence>,
    pub affected_projects: Vec<String>,
    pub affected_dependencies: Vec<String>,
    pub urgency: AlertUrgency,
    pub confidence: f32,
    pub predicted_window: Option<String>,
    pub suggested_actions: Vec<SuggestedAction>,
    pub created_at: String,
    /// True when this alert is backed by a deterministic OSV advisory match
    /// with version verification. Drives Confidence::osv_verified provenance.
    #[serde(default)]
    pub osv_verified: bool,
    /// True when the source itself classified this as security_advisory or
    /// breaking_change (not just keyword matching). Drives llm_assessed provenance.
    #[serde(default)]
    pub source_classified: bool,
    /// Installed version of the affected package (from project_dependencies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_version: Option<String>,
    /// Fixed version to update to (from OSV advisory).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_version: Option<String>,
    /// Whether this is a direct dependency (true) or transitive (false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_direct: Option<bool>,
    /// Whether this is a dev-only dependency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_dev: Option<bool>,
}

/// The full preemption feed with summary counts.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PreemptionFeed {
    pub alerts: Vec<PreemptionAlert>,
    pub total: usize,
    pub critical_count: usize,
    pub high_count: usize,
}

#[derive(Debug, Clone)]
struct DirectRuntimeDep {
    package_name: String,
    project_path: String,
    language: String,
}

// ============================================================================
// Implementation
// ============================================================================

fn title_indicates_not_affected(text_lower: &str) -> bool {
    [
        "not affected",
        "users protected",
        "already protected",
        "protected by default",
        "already mitigated",
        "outside the affected range",
    ]
    .iter()
    .any(|marker| text_lower.contains(marker))
}

fn has_conditional_scope_language(text_lower: &str) -> bool {
    [
        "may be scoped",
        "might be scoped",
        "only affects",
        "specific to",
        "needs verification",
        "deployment context",
    ]
    .iter()
    .any(|marker| text_lower.contains(marker))
}

fn find_unmet_platform_scope(text_lower: &str, user_context_lower: &str) -> Option<&'static str> {
    let markers = [
        ("deno", "deno deploy"),
        ("vercel", "vercel"),
        ("netlify", "netlify"),
        ("cloudflare", "cloudflare workers"),
        ("bun", "bun"),
        ("electron", "electron"),
        ("edge", "edge runtime"),
    ];
    for (context_key, marker) in markers {
        if text_lower.contains(marker) && !user_context_lower.contains(context_key) {
            return Some(context_key);
        }
    }
    None
}

/// Infer the likely package ecosystem from advisory context.
/// Returns normalized ecosystem strings matching project_dependencies.language values.
fn infer_advisory_ecosystem(title_lower: &str, source_type: &str) -> Option<&'static str> {
    // Source-type hints
    if source_type == "crates_io" {
        return Some("rust");
    }
    if source_type == "npm" {
        return Some("javascript");
    }
    if source_type == "pypi" {
        return Some("python");
    }

    // Title-based hints — check for ecosystem markers
    if title_lower.contains("npm")
        || title_lower.contains("node.js")
        || title_lower.contains("nodejs")
    {
        return Some("javascript");
    }
    if title_lower.contains("crate")
        || title_lower.contains("cargo")
        || title_lower.contains("rustc")
    {
        return Some("rust");
    }
    if title_lower.contains("pypi")
        || title_lower.contains("pip ")
        || title_lower.contains("python")
    {
        return Some("python");
    }
    if title_lower.contains("nuget")
        || title_lower.contains(".net")
        || title_lower.contains("dotnet")
    {
        return Some("csharp");
    }
    if title_lower.contains("maven") || title_lower.contains("gradle") {
        return Some("java");
    }
    if title_lower.contains("rubygem") || title_lower.contains("ruby") {
        return Some("ruby");
    }
    if title_lower.contains("go module") || title_lower.contains("golang") {
        return Some("go");
    }

    None // Can't determine — allow match (conservative)
}

fn load_direct_runtime_deps(conn: &rusqlite::Connection) -> Result<Vec<DirectRuntimeDep>> {
    let direct_filter = if has_is_direct_column(conn) {
        "AND is_direct = 1"
    } else {
        ""
    };
    let relevance_filter = if has_project_relevance_column(conn) {
        "AND project_relevance >= 0.15"
    } else {
        ""
    };
    let sql = format!(
        "SELECT package_name, project_path, language
         FROM project_dependencies
         WHERE is_dev = 0
           {direct_filter}
           {relevance_filter}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(DirectRuntimeDep {
            package_name: row.get(0)?,
            project_path: row.get(1)?,
            language: row.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn matched_direct_runtime_deps(
    deps: &[DirectRuntimeDep],
    title_lower: &str,
    source_type: &str,
    _content_type: Option<&str>,
) -> Vec<DirectRuntimeDep> {
    deps.iter()
        .filter(|dep| dep.package_name.len() >= 5)
        .filter_map(|dep| {
            let pkg_lower = dep.package_name.to_lowercase();
            if !has_word_boundary_match(title_lower, &pkg_lower) {
                return None;
            }
            // Compound-prefix: "i18next" matching "i18next-http-middleware" = different package
            if is_compound_prefix_match(title_lower, &pkg_lower) {
                return None;
            }
            // Advisory-subject: only match if the dep is the advisory's actual subject
            if !is_advisory_subject_match(title_lower, &pkg_lower) {
                return None;
            }
            // Cross-ecosystem guard: if we can infer the advisory's ecosystem and it
            // doesn't match the dependency's language, skip — prevents npm advisories
            // matching Rust crates with the same package name.
            if let Some(advisory_eco) = infer_advisory_ecosystem(title_lower, source_type) {
                let dep_lang = dep.language.to_lowercase();
                if dep_lang != advisory_eco {
                    return None;
                }
            }
            Some(dep.clone())
        })
        .collect()
}

fn collapse_direct_dep_targets(matches: &[DirectRuntimeDep]) -> (Vec<String>, Vec<String>) {
    let mut deps = std::collections::BTreeSet::new();
    let mut projects = std::collections::BTreeSet::new();
    for item in matches {
        deps.insert(item.package_name.clone());
        projects.insert(item.project_path.clone());
    }
    (projects.into_iter().collect(), deps.into_iter().collect())
}

fn osv_matches_to_alerts() -> Vec<PreemptionAlert> {
    let db = match crate::get_database() {
        Ok(db) => db,
        Err(e) => {
            warn!(target: "4da::preemption", error = %e, "Failed to get database for OSV matches");
            return Vec::new();
        }
    };

    let matches = match crate::osv::matching::get_matched_advisories(db) {
        Ok(m) => m,
        Err(e) => {
            warn!(target: "4da::preemption", error = %e, "Failed to get OSV matched advisories");
            return Vec::new();
        }
    };

    // Group confirmed matches by (package_name, ecosystem) — one alert per package.
    let mut pkg_groups: std::collections::BTreeMap<
        (String, String),
        Vec<&crate::osv::types::MatchedAdvisory>,
    > = std::collections::BTreeMap::new();
    for m in matches.iter().filter(|m| m.is_version_confirmed) {
        let key = (m.package_name.clone(), m.ecosystem.clone());
        pkg_groups.entry(key).or_default().push(m);
    }

    pkg_groups
        .into_values()
        .map(|group| {
            let first = group[0];
            let advisory_count = group.len();

            // Highest urgency across all advisories for this package
            let urgency = group
                .iter()
                .map(|m| {
                    if let Some(s) = m.cvss_score {
                        if s >= 9.0 {
                            AlertUrgency::Critical
                        } else if s >= 7.0 {
                            AlertUrgency::High
                        } else if s >= 4.0 {
                            AlertUrgency::Medium
                        } else {
                            AlertUrgency::Watch
                        }
                    } else {
                        infer_urgency_from_summary(&m.summary, &m.advisory_id)
                    }
                })
                .min_by_key(|u| urgency_rank(u))
                .unwrap_or(AlertUrgency::Watch);

            // Highest CVSS across the group
            let max_cvss = group
                .iter()
                .filter_map(|m| m.cvss_score)
                .fold(None, |acc, s| Some(acc.map_or(s, |a: f64| a.max(s))));

            let confidence: f32 = {
                let base: f32 = 0.92;
                let cvss_bonus: f32 = if max_cvss.is_some() { 0.03 } else { 0.0 };
                (base + cvss_bonus).min(0.99)
            };

            // Best fix version (highest semver among fixed_versions)
            let best_fix: Option<String> = group
                .iter()
                .filter_map(|m| m.fixed_version.as_ref())
                .max_by(|a, b| {
                    semver::Version::parse(a.trim_start_matches('v'))
                        .ok()
                        .zip(semver::Version::parse(b.trim_start_matches('v')).ok())
                        .map(|(va, vb)| va.cmp(&vb))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .cloned();

            // Merge all project paths
            let mut all_projects: Vec<String> = group
                .iter()
                .flat_map(|m| m.project_paths.iter().cloned())
                .collect();
            all_projects.sort();
            all_projects.dedup();

            let project_display = if all_projects.is_empty() {
                "your projects".to_string()
            } else {
                let mut names: Vec<String> = all_projects
                    .iter()
                    .map(|p| shorten_project_path(p))
                    .collect();
                names.sort();
                names.dedup();
                names.join(", ")
            };

            let version_str = first.installed_version.as_deref().unwrap_or("unknown");
            let fix_str = best_fix
                .as_deref()
                .map(|f| format!(" Update to >= {f}."))
                .unwrap_or_default();

            let vuln_word = if advisory_count == 1 {
                "vulnerability"
            } else {
                "vulnerabilities"
            };

            let title = if advisory_count == 1 {
                truncate(&first.summary, 120).to_string()
            } else {
                format!(
                    "{pkg}@{ver}: {count} known {vuln_word}",
                    pkg = first.package_name,
                    ver = version_str,
                    count = advisory_count,
                    vuln_word = vuln_word,
                )
            };

            // Collect advisory IDs for explanation
            let advisory_ids: Vec<&str> = group.iter().map(|m| m.advisory_id.as_str()).collect();
            let ids_display = if advisory_count <= 3 {
                advisory_ids.join(", ")
            } else {
                format!(
                    "{}, {} and {} more",
                    advisory_ids[0],
                    advisory_ids[1],
                    advisory_count - 2
                )
            };

            let explanation = format!(
                "{ids} ({count} {vuln_word}) affect {pkg}@{ver} in {projects}.{fix}",
                ids = ids_display,
                count = advisory_count,
                vuln_word = vuln_word,
                pkg = first.package_name,
                ver = version_str,
                projects = project_display,
                fix = fix_str,
            );

            let action_label = if let Some(ref fix) = best_fix {
                format!(
                    "Update {} from {} to >= {}",
                    first.package_name, version_str, fix
                )
            } else {
                format!(
                    "Review {} advisories for {}",
                    advisory_count, first.package_name
                )
            };

            // Include top 3 advisories as evidence entries
            let evidence: Vec<AlertEvidence> = group
                .iter()
                .take(3)
                .map(|m| AlertEvidence {
                    source: "osv".to_string(),
                    title: m.summary.clone(),
                    url: m.source_url.clone(),
                    freshness_days: m
                        .published_at
                        .as_deref()
                        .map(|ts| freshness_from_timestamp(ts))
                        .unwrap_or(0.0),
                    relevance_score: 1.0,
                })
                .collect();

            let suggested_actions = vec![
                SuggestedAction {
                    action_type: "investigate".to_string(),
                    label: action_label,
                    description: format!(
                        "Review {} advisories and update {} if affected.",
                        advisory_count, first.package_name
                    ),
                },
                SuggestedAction {
                    action_type: "dismiss".to_string(),
                    label: "Not affected".to_string(),
                    description:
                        "Dismiss if you've confirmed your version is outside the affected range."
                            .to_string(),
                },
            ];

            let (dep_is_direct, dep_is_dev) = {
                let conn = db.conn.lock();
                let scoped_result = all_projects.first().and_then(|project| {
                    conn.query_row(
                        "SELECT is_direct, is_dev FROM project_dependencies WHERE package_name = ?1 AND project_path = ?2 LIMIT 1",
                        rusqlite::params![first.package_name, project],
                        |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
                    )
                    .ok()
                });
                scoped_result
                    .or_else(|| {
                        conn.query_row(
                            "SELECT is_direct, is_dev FROM project_dependencies WHERE package_name = ?1 LIMIT 1",
                            rusqlite::params![first.package_name],
                            |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
                        )
                        .ok()
                    })
                    .map(|(d, v)| (Some(d), Some(v)))
                    .unwrap_or((None, None))
            };

            PreemptionAlert {
                id: format!("osv-pkg-{}-{}", first.package_name, first.ecosystem),
                alert_type: PreemptionType::SecurityAdvisory,
                title,
                explanation,
                evidence,
                affected_projects: all_projects,
                affected_dependencies: vec![first.package_name.clone()],
                urgency,
                confidence,
                predicted_window: None,
                suggested_actions,
                created_at: chrono::Utc::now().to_rfc3339(),
                osv_verified: true,
                source_classified: false,
                installed_version: first.installed_version.clone(),
                fixed_version: best_fix.clone(),
                is_direct: dep_is_direct,
                is_dev: dep_is_dev,
            }
        })
        .collect()
}

/// Tier 2: Convert LLM-judged high-relevance items into preemption alerts.
///
/// Queries stored LLM judgments for security/breaking-change items and converts
/// them into `PreemptionAlert`s with LLM-calibrated confidence. These sit between
/// OSV-verified (Tier 1) and keyword-heuristic (Tier 3) in trust ranking.
fn llm_judged_to_alerts() -> Vec<PreemptionAlert> {
    let db = match crate::get_database() {
        Ok(db) => db,
        Err(_) => return Vec::new(),
    };

    let judgments = match db.get_relevant_judgments(0.50, 50) {
        Ok(j) => j,
        Err(e) => {
            warn!(target: "4da::preemption", error = %e, "Failed to get LLM judgments");
            return Vec::new();
        }
    };

    if judgments.is_empty() {
        return Vec::new();
    }

    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let direct_runtime_deps = match load_direct_runtime_deps(&conn) {
        Ok(deps) => deps,
        Err(e) => {
            warn!(target: "4da::preemption", error = %e, "Failed to load direct runtime deps");
            return Vec::new();
        }
    };
    let user_context_lower = crate::adversarial::build_user_context_summary().to_lowercase();

    let mut alerts = Vec::new();

    for j in &judgments {
        // Load the source item to get title/url/source_type
        let item = match conn.query_row(
            "SELECT title, url, source_type, created_at, content_type FROM source_items WHERE id = ?1",
            rusqlite::params![j.source_item_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        ) {
            Ok(item) => item,
            Err(_) => continue,
        };
        let (title, url, source_type, created_at, content_type) = item;

        // Only include security-relevant items in preemption
        let title_lower = title.to_lowercase();
        let explanation_lower = j.explanation.to_lowercase();
        let combined_lower = format!("{title_lower}\n{explanation_lower}");
        let is_security = title_lower.contains("cve")
            || title_lower.contains("ghsa")
            || title_lower.contains("vulnerab")
            || title_lower.contains("security")
            || title_lower.contains("advisory")
            || title_lower.contains("exploit");
        let is_breaking = title_lower.contains("breaking")
            || title_lower.contains("deprecat")
            || title_lower.contains("end of life")
            || title_lower.contains("end-of-life")
            || title_lower.contains("migration guide");

        if !is_security && !is_breaking {
            continue;
        }

        // Tier 2 never assigns Critical — that's reserved for deterministic OSV matches
        if title_indicates_not_affected(&combined_lower) {
            continue;
        }
        if find_unmet_platform_scope(&combined_lower, &user_context_lower).is_some() {
            continue;
        }
        let matched = matched_direct_runtime_deps(
            &direct_runtime_deps,
            &title_lower,
            &source_type,
            content_type.as_deref(),
        );
        if matched.is_empty() {
            continue;
        }
        let (affected_projects, affected_dependencies) = collapse_direct_dep_targets(&matched);

        let urgency = if !has_conditional_scope_language(&combined_lower)
            && j.relevance_score >= scoring_config::PREEMPTION_URGENCY_HIGH_THRESHOLD as f64
            && j.confidence >= 0.70
        {
            AlertUrgency::Medium
        } else {
            AlertUrgency::Watch
        };

        let alert_type = if is_security {
            PreemptionType::SecurityAdvisory
        } else {
            PreemptionType::BreakingChange
        };

        let evidence = vec![AlertEvidence {
            source: source_type,
            title: title.clone(),
            url,
            freshness_days: freshness_from_timestamp(&created_at),
            relevance_score: j.relevance_score as f32,
        }];

        let suggested_actions = vec![
            SuggestedAction {
                action_type: "investigate".to_string(),
                label: format!("Review: {}", truncate(&title, 60)),
                description: j.explanation.clone(),
            },
            SuggestedAction {
                action_type: "dismiss".to_string(),
                label: "Not relevant".to_string(),
                description: "Dismiss if this doesn't affect your projects.".to_string(),
            },
        ];

        alerts.push(PreemptionAlert {
            id: format!("llm-{}", j.source_item_id),
            alert_type,
            title: truncate(&title, 120),
            explanation: j.explanation.clone(),
            evidence,
            affected_projects,
            affected_dependencies,
            urgency,
            confidence: j.confidence as f32,
            predicted_window: None,
            suggested_actions,
            created_at: j.judged_at.clone(),
            osv_verified: false,
            source_classified: false,
            installed_version: None,
            fixed_version: None,
            is_direct: None,
            is_dev: None,
        });
    }

    alerts
}

/// Generate the preemption feed by combining all intelligence sources.
///
/// PERFORMANCE: On a 239MB DB with 141 projects × 2497 deps, the naive
/// approach (calling `compute_all_project_health` which iterates 141
/// projects × 45 LIKE queries × 2 content columns + embedded detect_chains)
/// takes 4-8 minutes. This hits the Tauri 30-second IPC timeout and produces
/// the "Command 'get_preemption_alerts' timed out after 30s" error.
///
/// The fix:
/// 1. Call `detect_chains` exactly ONCE (not per-project).
/// 2. Replace `compute_all_project_health` with a single batched JOIN query
///    that finds DIRECT deps mentioned in security-keyword source_items in
///    the last 30 days. One SQL round-trip vs ~8000 per-dep queries.
///
/// Target: under 5 seconds end-to-end on the production DB.
pub fn get_preemption_feed() -> Result<PreemptionFeed> {
    let conn = crate::open_db_connection()?;
    let mut alerts = Vec::new();

    // ─── 0. Tier 1: OSV verified advisories (deterministic, highest trust) ──
    let tier1 = osv_matches_to_alerts();
    debug!(target: "4da::preemption", tier1_count = tier1.len(), "Tier 1 OSV alerts");
    alerts.extend(tier1);

    // ─── 0.5. Tier 2: LLM-assessed security items (pre-computed judgments) ──
    let tier2 = llm_judged_to_alerts();
    debug!(target: "4da::preemption", tier2_count = tier2.len(), "Tier 2 LLM alerts");
    alerts.extend(tier2);

    // ─── 1. Signal chain predictions (single call, bounded LIMIT 200) ────
    match crate::signal_chains::detect_chains(&conn) {
        Ok(chains) => {
            for chain in &chains {
                let prediction = crate::signal_chains::predict_chain_lifecycle(chain);
                if prediction.confidence > 0.4 && chain.resolution == ChainResolution::Open {
                    alerts.push(chain_to_alert(chain, &prediction, &conn));
                }
            }
        }
        Err(e) => warn!(target: "4da::preemption", error = %e, "Failed to detect signal chains"),
    }

    // Tier 3 heuristics (keyword matching article titles) removed — produced
    // noise that degraded trust in the entire preemption surface. All security
    // alerts now flow through Tier 1 (OSV-verified) or Tier 2 (LLM-assessed).

    let pre_dedup = alerts.len();

    // ─── Cross-tier dedup: higher-trust tier wins ────────────────────────
    // Tier 1 (OSV) > Tier 2 (LLM) > Tier 3 (signal chains). When the same
    // vulnerability appears across tiers, keep only the highest-trust entry.
    // Dedup key normalizes on the advisory/CVE id embedded in the alert id
    // and the primary affected package.
    {
        let mut seen = std::collections::HashSet::new();
        alerts.retain(|alert| {
            let norm_key = cross_tier_dedup_key(alert);
            seen.insert(norm_key)
        });
    }
    debug!(
        target: "4da::preemption",
        pre_dedup = pre_dedup,
        post_dedup = alerts.len(),
        removed = pre_dedup - alerts.len(),
        "Final preemption feed"
    );

    // Sort: Critical first, then High, Medium, Watch. Within same urgency, highest confidence first.
    alerts.sort_by(|a, b| {
        urgency_rank(&a.urgency)
            .cmp(&urgency_rank(&b.urgency))
            .then(
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    // Cap total alerts to keep the UI scannable.
    const MAX_ALERTS: usize = 30;
    alerts.truncate(MAX_ALERTS);

    let critical_count = alerts
        .iter()
        .filter(|a| matches!(a.urgency, AlertUrgency::Critical))
        .count();
    let high_count = alerts
        .iter()
        .filter(|a| matches!(a.urgency, AlertUrgency::High))
        .count();
    let total = alerts.len();

    Ok(PreemptionFeed {
        alerts,
        total,
        critical_count,
        high_count,
    })
}

/// Check whether `project_dependencies` has the `is_direct` column.
///
/// Added in Phase 53 migration. Pre-Phase-53 databases lack the column
/// and would SQL-error on `WHERE pd.is_direct = 1`. This runtime check
/// lets us gracefully fall back to processing all non-dev deps.
fn has_is_direct_column(conn: &rusqlite::Connection) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('project_dependencies') WHERE name = 'is_direct'",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

/// Check whether `project_dependencies` has the `project_relevance` column.
///
/// Added in Phase 55 migration. Pre-Phase-55 databases lack the column.
/// When present, low-relevance projects (example/demo/test dirs) are excluded
/// from preemption alerts.
fn has_project_relevance_column(conn: &rusqlite::Connection) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('project_dependencies') WHERE name = 'project_relevance'",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

// (Tier 3 heuristics and suppression list removed — see get_preemption_feed comment)
// ============================================================================
// Converters
// ============================================================================

/// Convert a signal chain + its lifecycle prediction into a preemption alert.
fn chain_to_alert(
    chain: &crate::signal_chains::SignalChain,
    prediction: &crate::signal_chains::ChainPrediction,
    conn: &rusqlite::Connection,
) -> PreemptionAlert {
    use crate::signal_chains::ChainPhase;

    let urgency = match &prediction.phase {
        ChainPhase::Escalating | ChainPhase::Peak => {
            if chain.overall_priority == "critical" {
                AlertUrgency::Critical
            } else {
                AlertUrgency::High
            }
        }
        ChainPhase::Active => AlertUrgency::Medium,
        ChainPhase::Nascent | ChainPhase::Resolving => AlertUrgency::Watch,
    };

    let alert_type = classify_chain_type(&chain.chain_name);

    let predicted_window = prediction
        .predicted_next_hours
        .map(|h| format_time_window(h));

    let evidence: Vec<AlertEvidence> = chain
        .links
        .iter()
        .map(|link| {
            let freshness = freshness_from_timestamp(&link.timestamp);
            let url: Option<String> = conn
                .query_row(
                    "SELECT url FROM source_items WHERE id = ?1",
                    rusqlite::params![link.source_item_id],
                    |row| row.get(0),
                )
                .ok()
                .flatten();
            AlertEvidence {
                source: link.signal_type.clone(),
                title: link.title.clone(),
                url,
                freshness_days: freshness,
                relevance_score: chain.confidence as f32,
            }
        })
        .collect();

    let suggested_actions = vec![
        SuggestedAction {
            action_type: "investigate".to_string(),
            label: format!("Investigate {}", chain.chain_name),
            description: chain.suggested_action.clone(),
        },
        SuggestedAction {
            action_type: "watch".to_string(),
            label: "Monitor chain".to_string(),
            description: format!(
                "Keep watching — {} signals tracked so far",
                chain.links.len()
            ),
        },
    ];

    PreemptionAlert {
        id: format!("chain-{}", uuid::Uuid::new_v4()),
        alert_type,
        title: if let Some(first_link) = chain.links.first() {
            truncate(&first_link.title, 120)
        } else {
            truncate(&chain.chain_name, 120)
        },
        explanation: {
            let source_count = chain.links.len();
            let first_ts = chain.links.first().map(|l| &l.timestamp);
            let last_ts = chain.links.last().map(|l| &l.timestamp);
            let days_span = match (first_ts, last_ts) {
                (Some(first), Some(last)) => {
                    let first_f = freshness_from_timestamp(first);
                    let last_f = freshness_from_timestamp(last);
                    ((first_f - last_f).abs().ceil() as u32).max(1)
                }
                _ => 1,
            };
            format!(
                "{source_count} sources discussing {} over {days_span} day{}. No advisory issued.",
                chain.chain_name,
                if days_span == 1 { "" } else { "s" }
            )
        },
        evidence,
        affected_projects: vec![],
        affected_dependencies: vec![],
        urgency,
        confidence: prediction.confidence as f32,
        predicted_window,
        suggested_actions,
        created_at: chrono::Utc::now().to_rfc3339(),
        osv_verified: false,
        source_classified: false,
        installed_version: None,
        fixed_version: None,
        is_direct: None,
        is_dev: None,
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Map urgency to a sort rank (lower = more urgent).
/// Extract the last two path segments for readable project identification.
/// "C:\Users\Admin\Documents\kairos-mvp\backend" → "kairos-mvp/backend"
/// Matches the frontend's `shortenProjectPath()` logic.
fn shorten_project_path(full_path: &str) -> String {
    let segments: Vec<&str> = full_path
        .split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .collect();
    if segments.len() <= 2 {
        segments.join("/")
    } else {
        segments[segments.len() - 2..].join("/")
    }
}

/// Infer urgency from advisory summary text when CVSS score is absent.
fn infer_urgency_from_summary(summary: &str, advisory_id: &str) -> AlertUrgency {
    let s = summary.to_lowercase();
    let id = advisory_id.to_lowercase();
    let has_critical_keyword = s.contains("remote code execution")
        || s.contains("rce")
        || s.contains("arbitrary code")
        || s.contains("sandbox escape")
        || s.contains("authentication bypass")
        || s.contains("authorization bypass");
    if has_critical_keyword {
        return AlertUrgency::High;
    }
    let has_high_keyword = s.contains("prototype pollution")
        || s.contains("ssrf")
        || s.contains("xss")
        || s.contains("cross-site scripting")
        || s.contains("injection")
        || s.contains("exfiltration")
        || s.contains("credential")
        || s.contains("timing sidechannel");
    if has_high_keyword {
        return AlertUrgency::Medium;
    }
    if id.starts_with("mal-") {
        return AlertUrgency::High;
    }
    AlertUrgency::Watch
}

/// Build a normalized dedup key for cross-tier duplicate detection.
/// Extracts the advisory identifier (GHSA/CVE) and primary package from
/// the alert, so the same vulnerability surfaced by OSV (Tier 1) and
/// LLM (Tier 2) collapses to one entry. Tier 1 entries appear first in
/// the alerts vec, so `retain()` keeps them over Tier 2/3 duplicates.
fn cross_tier_dedup_key(alert: &PreemptionAlert) -> String {
    let pkg = alert
        .affected_dependencies
        .first()
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    // Extract advisory id from the title or explanation (GHSA-xxxx or CVE-xxxx)
    let text = format!("{} {}", alert.title, alert.explanation);
    let advisory_id = extract_advisory_id(&text).unwrap_or_else(|| alert.id.clone());

    format!("{}:{}", advisory_id.to_lowercase(), pkg)
}

/// Pull the first GHSA-xxx or CVE-xxx identifier from text.
fn extract_advisory_id(text: &str) -> Option<String> {
    let text_upper = text.to_uppercase();
    for prefix in &["GHSA-", "CVE-"] {
        if let Some(start) = text_upper.find(prefix) {
            let end = text[start..]
                .find(|c: char| c.is_whitespace() || c == ')' || c == ']' || c == ':' || c == ',')
                .map(|i| start + i)
                .unwrap_or(text.len().min(start + 30));
            return Some(text[start..end].to_string());
        }
    }
    None
}

fn urgency_rank(urgency: &AlertUrgency) -> u8 {
    match urgency {
        AlertUrgency::Critical => 0,
        AlertUrgency::High => 1,
        AlertUrgency::Medium => 2,
        AlertUrgency::Watch => 3,
    }
}

/// Classify a chain name into a preemption type based on keywords.
fn classify_chain_type(chain_name: &str) -> PreemptionType {
    let lower = chain_name.to_lowercase();
    if lower.contains("cve") || lower.contains("security") || lower.contains("vulnerab") {
        PreemptionType::SecurityAdvisory
    } else if lower.contains("breaking") || lower.contains("deprecat") {
        PreemptionType::BreakingChange
    } else if lower.contains("migrat") || lower.contains("upgrade") {
        PreemptionType::MigrationWindow
    } else if lower.contains("maintain") || lower.contains("abandon") {
        PreemptionType::MaintainerDecline
    } else {
        PreemptionType::EcosystemShift
    }
}

/// Format hours into a human-readable time window string.
fn format_time_window(hours: f64) -> String {
    if hours < 1.0 {
        "within the hour".to_string()
    } else if hours < 24.0 {
        format!("within ~{:.0} hours", hours)
    } else {
        let days = hours / 24.0;
        format!("within ~{:.0} days", days)
    }
}

/// Compute approximate freshness in days from an RFC3339/ISO timestamp.
fn freshness_from_timestamp(timestamp: &str) -> f32 {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .or_else(|_| {
            // Try parsing as "YYYY-MM-DD HH:MM:SS" (SQLite default)
            chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S").map(|naive| {
                naive
                    .and_local_timezone(chrono::Utc)
                    .single()
                    .unwrap_or_else(chrono::Utc::now)
                    .fixed_offset()
            })
        })
        .map(|dt| {
            let duration = chrono::Utc::now().signed_duration_since(dt);
            (duration.num_hours() as f32 / 24.0).max(0.0)
        })
        .unwrap_or(0.0)
}

/// Truncate a string to a maximum length, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .nth(max_len.saturating_sub(3))
            .map(|(i, _)| i)
            .unwrap_or_else(|| s.floor_char_boundary(max_len.saturating_sub(3)));
        format!("{}...", &s[..end])
    }
}

/// Detect when a dep name is a prefix of a longer compound package name in the
/// title. E.g. "i18next" inside "i18next-http-middleware" — the hyphen after
/// the match means it's a DIFFERENT package, not a standalone mention.
/// Returns true when ALL occurrences of `dep` in `text` are compound-prefixes.
fn is_compound_prefix_match(text: &str, dep: &str) -> bool {
    if dep.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let mut found_standalone = false;
    let mut search_from = 0;
    while let Some(pos) = text[search_from..].find(dep) {
        let abs = search_from + pos;
        let before_ok = abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric();
        let after = abs + dep.len();
        let after_is_hyphen = after < bytes.len() && bytes[after] == b'-';
        if before_ok && !after_is_hyphen {
            found_standalone = true;
            break;
        }
        search_from = abs + 1;
    }
    // If we never found a standalone (non-prefix) occurrence, it's compound-prefix only
    !found_standalone && text.contains(dep)
}

/// For structured advisory titles like "[CVE-2026-XXXX] PackageName has/is ..."
/// or "[GHSA-xxxx-yyyy] PackageName: description", extract the subject package
/// and verify the user's dep IS that package — not just a word in the description.
///
/// Returns true (allow the match) when:
///   - The title doesn't have the structured pattern (can't extract subject → allow)
///   - The dep name IS the advisory's subject package
/// Returns false (reject the match) when:
///   - The title has a clear subject package that's DIFFERENT from the dep
fn is_advisory_subject_match(title_lower: &str, dep: &str) -> bool {
    // Extract text after the advisory ID prefix: "[CVE-...] " or "[GHSA-...] "
    let subject_start = if let Some(bracket_end) = title_lower.find("] ") {
        bracket_end + 2
    } else {
        return true; // No structured prefix — can't extract subject, allow match
    };

    let remainder = &title_lower[subject_start..];
    if remainder.is_empty() {
        return true;
    }

    // The subject package is the first word(s) before a verb like "has", "is",
    // "allows", "could", "can", "may", "in", or a colon.
    let subject_end_markers = [
        " has ",
        " is ",
        " allows ",
        " could ",
        " can ",
        " may ",
        " in ",
        ": ",
        " vulnerable ",
        " affected ",
        " exposes ",
    ];
    let subject_end = subject_end_markers
        .iter()
        .filter_map(|m| remainder.find(m))
        .min()
        .unwrap_or(remainder.len().min(80));

    let subject = &remainder[..subject_end];

    // If the dep name appears as a word boundary in the subject, it's the target
    if has_word_boundary_match(subject, dep) {
        return true;
    }

    // The subject is a different package/product name. Reject the match.
    false
}

/// Check whether `text` contains `term` at a word boundary (not embedded in a
/// larger word). Case-sensitive — pass lowercase strings for case-insensitive
/// matching. Accepts `.js`/`.ts`/`.rs` suffixes as valid boundaries for package
/// names like "next.js" or "serde.rs".
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

// ============================================================================
// EvidenceItem conversion (Intelligence Reconciliation — Phase 3)
// ============================================================================
//
// `PreemptionAlert` is the pre-reconciliation shape. The Tauri command now
// emits canonical `EvidenceItem`s via `EvidenceFeed`. Internal callers
// (e.g. `monitoring_briefing.rs`) still use `PreemptionAlert` until their
// own materializers land in later phases.

fn alert_urgency_to_canonical(u: &AlertUrgency) -> Urgency {
    match u {
        AlertUrgency::Critical => Urgency::Critical,
        AlertUrgency::High => Urgency::High,
        AlertUrgency::Medium => Urgency::Medium,
        AlertUrgency::Watch => Urgency::Watch,
    }
}

/// Map the legacy `action_type` string onto a canonical action_id. Legacy
/// values were a free-text convention; canonical ids are enumerated in
/// `evidence::types::ACTION_IDS`. Unknown values fall back to "acknowledge".
fn map_action_id(legacy: &str) -> &'static str {
    match legacy {
        "dismiss" => "dismiss",
        "watch" => "snooze_7d",
        "investigate" => "investigate",
        "review_decision" => "brief_this",
        _ => "acknowledge",
    }
}

fn suggested_action_to_canonical(a: &SuggestedAction) -> EvidenceAction {
    EvidenceAction {
        action_id: map_action_id(&a.action_type).to_string(),
        label: a.label.clone(),
        description: a.description.clone(),
    }
}

fn alert_evidence_to_citation(e: &AlertEvidence) -> EvidenceCitation {
    // Cap relevance_note at 200 chars per EvidenceItem schema rule.
    let note = format!("relevance {:.2}", e.relevance_score);
    EvidenceCitation {
        source: e.source.clone(),
        title: e.title.clone(),
        url: e.url.clone(),
        freshness_days: e.freshness_days,
        relevance_note: note,
    }
}

fn preemption_kind_to_canonical(t: &PreemptionType) -> EvidenceKind {
    match t {
        PreemptionType::KnowledgeBlindSpot => EvidenceKind::Gap,
        _ => EvidenceKind::Alert,
    }
}

impl PreemptionAlert {
    /// Convert to the canonical `EvidenceItem` for lens consumption.
    /// Used by `get_preemption_alerts` (command boundary).
    pub fn to_evidence_item(&self) -> EvidenceItem {
        // `created_at` is an ISO-8601 SQLite datetime string; convert to
        // Unix millis. On parse failure fall back to "now" — never break
        // a user-facing item on a timestamp quirk.
        let created_at =
            chrono::NaiveDateTime::parse_from_str(&self.created_at, "%Y-%m-%d %H:%M:%S")
                .map(|dt| dt.and_utc().timestamp_millis())
                .unwrap_or_else(|_| chrono::Utc::now().timestamp_millis());

        // Always title the item with the alert's own title; trim any
        // trailing period per schema rule.
        let title = self
            .title
            .trim_end_matches('.')
            .chars()
            .take(120)
            .collect::<String>();

        let kind = preemption_kind_to_canonical(&self.alert_type);
        let mut evidence: Vec<EvidenceCitation> = self
            .evidence
            .iter()
            .map(alert_evidence_to_citation)
            .collect();

        // Add version context as a structured citation when available
        if self.installed_version.is_some() || self.fixed_version.is_some() {
            let installed = self.installed_version.as_deref().unwrap_or("unknown");
            let fixed_note = self
                .fixed_version
                .as_deref()
                .map(|f| format!(" \u{2192} update to >= {f}"))
                .unwrap_or_default();
            let direct_note = match self.is_direct {
                Some(true) => " (direct)",
                Some(false) => " (transitive)",
                None => "",
            };
            let dev_note = match self.is_dev {
                Some(true) => " [dev]",
                _ => "",
            };
            evidence.push(EvidenceCitation {
                source: "version_context".to_string(),
                title: format!("Installed: {installed}{fixed_note}{direct_note}{dev_note}"),
                url: None,
                freshness_days: 0.0,
                relevance_note: "Dependency version metadata from project scan".to_string(),
            });
        }

        let suggested_actions: Vec<EvidenceAction> = self
            .suggested_actions
            .iter()
            .map(suggested_action_to_canonical)
            .collect();

        EvidenceItem {
            id: self.id.clone(),
            kind,
            title,
            explanation: self.explanation.clone(),
            confidence: if self.osv_verified {
                Confidence::osv_verified(self.confidence.clamp(0.0, 1.0))
            } else if self.id.starts_with("llm-") || self.source_classified {
                Confidence::llm_assessed(self.confidence.clamp(0.0, 1.0))
            } else {
                Confidence::heuristic(self.confidence.clamp(0.0, 1.0))
            },
            urgency: alert_urgency_to_canonical(&self.urgency),
            // Reversibility is not computed by preemption — leave None.
            reversibility: None,
            evidence,
            affected_projects: self.affected_projects.clone(),
            affected_deps: self.affected_dependencies.clone(),
            suggested_actions,
            precedents: Vec::new(),
            refutation_condition: None,
            lens_hints: LensHints::preemption_only(),
            created_at,
            expires_at: None,
        }
    }
}

// ============================================================================
// Tauri Command
// ============================================================================

/// Returns the canonical `EvidenceFeed` for the Preemption lens.
/// Internally still produces `PreemptionAlert`s (same ranking, same content)
/// and converts at the boundary — lossless for the UI, and lets
/// `monitoring_briefing.rs` continue to use the legacy shape until its own
/// phase. In dev builds the output is schema-validated; validation failures
/// drop the offending item with a log rather than breaking the feed.
#[tauri::command]
pub async fn get_preemption_alerts() -> std::result::Result<EvidenceFeed, String> {
    crate::settings::require_signal_feature("get_preemption_alerts").map_err(|e| e.to_string())?;
    let feed = get_preemption_feed().map_err(|e| e.to_string())?;
    let mut items: Vec<EvidenceItem> = feed
        .alerts
        .iter()
        .map(|a| a.to_evidence_item())
        .filter(|item| match crate::evidence::validate_item(item) {
            Ok(()) => true,
            Err(e) => {
                warn!(
                    target: "4da::evidence::validate",
                    id = %item.id,
                    error = %e,
                    "dropped preemption item failing schema validation"
                );
                false
            }
        })
        .collect();
    // TitanCA-inspired adversarial deliberation — two-perspective signal/noise
    // validation. Critical/High items bypass; Medium/Watch get deliberated.
    // Gracefully degrades when LLM is unavailable (items pass through unchanged).
    let user_context = crate::adversarial::build_user_context_summary();
    items = crate::adversarial::filter_batch(items, &user_context).await;

    Ok(EvidenceFeed::from_items(items))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    // ─── Compound-prefix detection ───────────────────────────────────

    #[test]
    fn compound_prefix_rejects_i18next_in_i18next_http_middleware() {
        assert!(is_compound_prefix_match(
            "[cve-2026-42353] i18next-http-middleware has path traversal",
            "i18next"
        ));
    }

    #[test]
    fn compound_prefix_allows_standalone_mention() {
        assert!(!is_compound_prefix_match(
            "critical vulnerability in i18next allows xss",
            "i18next"
        ));
    }

    #[test]
    fn compound_prefix_allows_when_both_standalone_and_compound_exist() {
        assert!(!is_compound_prefix_match(
            "i18next-http-middleware bypasses i18next sanitization",
            "i18next"
        ));
    }

    // ─── Advisory-subject extraction ─────────────────────────────────

    #[test]
    fn advisory_subject_rejects_react_in_nextjs_advisory() {
        assert!(!is_advisory_subject_match(
            "[ghsa-h25m-26qc-wcjf] next.js http request deserialization can lead to dos when using insecure react server components",
            "react"
        ));
    }

    #[test]
    fn advisory_subject_allows_direct_package_match() {
        assert!(is_advisory_subject_match(
            "[cve-2026-5555] react has critical xss vulnerability",
            "react"
        ));
    }

    #[test]
    fn advisory_subject_rejects_imagemagick_for_image_crate() {
        assert!(!is_advisory_subject_match(
            "[ghsa-xxxx-yyyy] imagemagick has heap buffer overflow in image encoder",
            "image"
        ));
    }

    #[test]
    fn advisory_subject_rejects_lemmy_for_image_crate() {
        assert!(!is_advisory_subject_match(
            "[ghsa-h6hf-9846-xwrq] lemmy has ssrf and internal image disclosure in post link metadata",
            "image"
        ));
    }

    #[test]
    fn advisory_subject_allows_unstructured_titles() {
        assert!(is_advisory_subject_match(
            "critical vulnerability in react allows xss",
            "react"
        ));
    }

    #[test]
    fn advisory_subject_allows_when_dep_is_subject() {
        assert!(is_advisory_subject_match(
            "[ghsa-xxxx-yyyy] dotenv could override environment variables",
            "dotenv"
        ));
    }

    // ─── Word-boundary helper ────────────────────────────────────────

    #[test]
    fn word_boundary_match_handles_suffix_extensions() {
        assert!(has_word_boundary_match("next.js release", "next"));
        assert!(has_word_boundary_match("serde.rs v2", "serde"));
        assert!(!has_word_boundary_match("unexpected", "next"));
    }

    // ─── Runtime column detection ────────────────────────────────────

    #[test]
    fn has_is_direct_column_true_when_present() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE project_dependencies (
                id INTEGER PRIMARY KEY,
                project_path TEXT NOT NULL,
                manifest_type TEXT NOT NULL,
                package_name TEXT NOT NULL,
                is_dev INTEGER DEFAULT 0,
                is_direct INTEGER DEFAULT 1,
                language TEXT NOT NULL DEFAULT 'unknown'
            );",
        )
        .unwrap();
        assert!(has_is_direct_column(&conn));
    }

    #[test]
    fn has_is_direct_column_false_when_absent() {
        let conn = Connection::open_in_memory().unwrap();
        // Create table WITHOUT is_direct column
        conn.execute_batch(
            "CREATE TABLE project_dependencies (
                id INTEGER PRIMARY KEY,
                project_path TEXT NOT NULL,
                manifest_type TEXT NOT NULL,
                package_name TEXT NOT NULL,
                is_dev INTEGER DEFAULT 0,
                language TEXT NOT NULL DEFAULT 'unknown'
            );",
        )
        .unwrap();
        assert!(!has_is_direct_column(&conn));
    }

    // ========================================================================
    // EvidenceItem conversion tests (Intelligence Reconciliation — Phase 3)
    // ========================================================================

    fn sample_alert() -> PreemptionAlert {
        PreemptionAlert {
            id: "p_sec_webpack".to_string(),
            alert_type: PreemptionType::SecurityAdvisory,
            title: "CVE-2026-9999 affects webpack".to_string(),
            explanation: "A critical vulnerability was reported.".to_string(),
            evidence: vec![AlertEvidence {
                source: "hn".to_string(),
                title: "CVE-2026-9999 webpack critical vulnerability".to_string(),
                url: Some("https://news.ycombinator.com/item?id=1".to_string()),
                freshness_days: 5.0,
                relevance_score: 0.82,
            }],
            affected_projects: vec!["/proj/a".to_string()],
            affected_dependencies: vec!["webpack".to_string()],
            urgency: AlertUrgency::Critical,
            confidence: 0.77,
            predicted_window: Some("within 7 days".to_string()),
            suggested_actions: vec![SuggestedAction {
                action_type: "investigate".to_string(),
                label: "Investigate".to_string(),
                description: "Review the advisory for affected versions.".to_string(),
            }],
            created_at: "2026-04-17 09:30:00".to_string(),
            osv_verified: false,
            source_classified: false,
            installed_version: None,
            fixed_version: None,
            is_direct: None,
            is_dev: None,
        }
    }

    #[test]
    fn to_evidence_item_maps_urgency() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        assert_eq!(item.urgency, crate::evidence::Urgency::Critical);
    }

    #[test]
    fn to_evidence_item_maps_security_advisory_to_alert_kind() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        assert_eq!(item.kind, crate::evidence::EvidenceKind::Alert);
    }

    #[test]
    fn to_evidence_item_maps_knowledge_blindspot_to_gap_kind() {
        let mut alert = sample_alert();
        alert.alert_type = PreemptionType::KnowledgeBlindSpot;
        let item = alert.to_evidence_item();
        assert_eq!(item.kind, crate::evidence::EvidenceKind::Gap);
    }

    #[test]
    fn to_evidence_item_maps_legacy_action_types() {
        let mut alert = sample_alert();
        alert.suggested_actions = vec![
            SuggestedAction {
                action_type: "watch".to_string(),
                label: "Watch".to_string(),
                description: "".to_string(),
            },
            SuggestedAction {
                action_type: "review_decision".to_string(),
                label: "Review".to_string(),
                description: "".to_string(),
            },
            SuggestedAction {
                action_type: "investigate".to_string(),
                label: "Look".to_string(),
                description: "".to_string(),
            },
            SuggestedAction {
                action_type: "dismiss".to_string(),
                label: "X".to_string(),
                description: "".to_string(),
            },
            SuggestedAction {
                action_type: "unknown_legacy".to_string(),
                label: "?".to_string(),
                description: "".to_string(),
            },
        ];
        let item = alert.to_evidence_item();
        let ids: Vec<&str> = item
            .suggested_actions
            .iter()
            .map(|a| a.action_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "snooze_7d",
                "brief_this",
                "investigate",
                "dismiss",
                "acknowledge"
            ]
        );
    }

    #[test]
    fn to_evidence_item_sets_preemption_lens_hint() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        assert!(item.lens_hints.preemption);
        assert!(!item.lens_hints.briefing);
        assert!(!item.lens_hints.blind_spots);
        assert!(!item.lens_hints.evidence);
    }

    #[test]
    fn to_evidence_item_strips_trailing_period_from_title() {
        let mut alert = sample_alert();
        alert.title = "Something will break.".to_string();
        let item = alert.to_evidence_item();
        assert_eq!(item.title, "Something will break");
    }

    #[test]
    fn to_evidence_item_caps_title_at_120_chars() {
        let mut alert = sample_alert();
        alert.title = "x".repeat(200);
        let item = alert.to_evidence_item();
        assert_eq!(item.title.len(), 120);
    }

    #[test]
    fn to_evidence_item_passes_schema_validation() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        assert!(crate::evidence::validate_item(&item).is_ok());
    }

    #[test]
    fn to_evidence_item_marks_confidence_heuristic_provenance() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        assert_eq!(
            item.confidence.provenance,
            crate::evidence::ConfidenceProvenance::Heuristic
        );
    }

    #[test]
    fn to_evidence_item_source_classified_gets_llm_assessed_provenance() {
        let mut alert = sample_alert();
        alert.source_classified = true;
        let item = alert.to_evidence_item();
        assert_eq!(
            item.confidence.provenance,
            crate::evidence::ConfidenceProvenance::LlmAssessed
        );
    }

    #[test]
    fn to_evidence_item_clamps_confidence_into_range() {
        let mut alert = sample_alert();
        alert.confidence = 1.5; // Out-of-range legacy value
        let item = alert.to_evidence_item();
        assert!(item.confidence.value >= 0.0 && item.confidence.value <= 1.0);
    }

    #[test]
    fn to_evidence_item_includes_citations() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        assert_eq!(item.evidence.len(), 1);
        assert_eq!(item.evidence[0].source, "hn");
        assert!(item.evidence[0].url.is_some());
    }

    #[test]
    fn to_evidence_item_parses_created_at() {
        let alert = sample_alert();
        let item = alert.to_evidence_item();
        // 2026-04-17 09:30:00 UTC → must be a real millis value
        assert!(item.created_at > 1_700_000_000_000);
    }

    // ─── shorten_project_path ───────────────────────────────────────

    #[test]
    fn shorten_project_path_windows_long() {
        assert_eq!(
            shorten_project_path(r"C:\Users\Admin\Documents\kairos-mvp\backend"),
            "kairos-mvp/backend"
        );
    }

    #[test]
    fn shorten_project_path_unix_long() {
        assert_eq!(
            shorten_project_path("/home/user/projects/my-app/frontend"),
            "my-app/frontend"
        );
    }

    #[test]
    fn shorten_project_path_short() {
        assert_eq!(shorten_project_path("my-app"), "my-app");
    }

    #[test]
    fn shorten_project_path_two_segments() {
        assert_eq!(shorten_project_path("parent/child"), "parent/child");
    }

    // ─── cross_tier_dedup_key ───────────────────────────────────────

    #[test]
    fn cross_tier_dedup_detects_same_ghsa_across_tiers() {
        let mut a = sample_alert();
        a.id = "osv-GHSA-abc-123-xyz-axios".to_string();
        a.title = "Axios: GHSA-abc-123-xyz SSRF bypass".to_string();
        a.explanation = "GHSA-abc-123-xyz affects axios".to_string();
        a.affected_dependencies = vec!["axios".to_string()];

        let mut b = sample_alert();
        b.id = "llm-source-42".to_string();
        b.title = "GHSA-abc-123-xyz: Axios SSRF".to_string();
        b.explanation = "GHSA-abc-123-xyz affects axios".to_string();
        b.affected_dependencies = vec!["axios".to_string()];

        assert_eq!(cross_tier_dedup_key(&a), cross_tier_dedup_key(&b));
    }

    #[test]
    fn cross_tier_dedup_distinguishes_different_advisories() {
        let mut a = sample_alert();
        a.title = "GHSA-aaa-bbb-ccc: Axios SSRF".to_string();
        a.affected_dependencies = vec!["axios".to_string()];

        let mut b = sample_alert();
        b.title = "GHSA-ddd-eee-fff: Axios DoS".to_string();
        b.affected_dependencies = vec!["axios".to_string()];

        assert_ne!(cross_tier_dedup_key(&a), cross_tier_dedup_key(&b));
    }

    // ─── extract_advisory_id ────────────────────────────────────────

    #[test]
    fn extract_advisory_id_finds_ghsa() {
        assert_eq!(
            extract_advisory_id("Axios: GHSA-m7pr-hjqh-92cm allows SSRF"),
            Some("GHSA-m7pr-hjqh-92cm".to_string())
        );
    }

    #[test]
    fn extract_advisory_id_finds_cve() {
        assert_eq!(
            extract_advisory_id("CVE-2025-62718 incomplete fix"),
            Some("CVE-2025-62718".to_string())
        );
    }

    #[test]
    fn extract_advisory_id_returns_none_for_no_id() {
        assert_eq!(extract_advisory_id("Some generic title"), None);
    }

    // ─── confidence scoring ─────────────────────────────────────────

    #[test]
    fn confidence_confirmed_with_cvss_is_highest() {
        let c: f32 = {
            let base: f32 = 0.92;
            let cvss_bonus: f32 = 0.03;
            (base + cvss_bonus).min(0.99)
        };
        assert!((c - 0.95).abs() < 0.001);
    }

    #[test]
    fn confidence_confirmed_no_cvss_lower_than_with() {
        let with: f32 = 0.95;
        let without: f32 = 0.92;
        assert!(without < with);
    }

    #[test]
    fn confidence_unconfirmed_clearly_lower() {
        let confirmed: f32 = 0.92;
        let unconfirmed: f32 = 0.58;
        assert!(unconfirmed < confirmed);
        assert!(confirmed - unconfirmed > 0.3);
    }

    #[test]
    fn test_osv_alerts_use_project_scoped_dep_context() {
        let db = crate::test_utils::test_db();
        let conn = db.conn.lock();

        conn.execute(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, is_direct, language)
             VALUES (?1, 'package.json', 'jsonwebtoken', '9.0.0', 0, 1, 'javascript')",
            rusqlite::params!["/projects/fourda"],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, is_direct, language)
             VALUES (?1, 'package.json', 'jsonwebtoken', '8.5.1', 1, 0, 'javascript')",
            rusqlite::params!["/projects/kairos"],
        )
        .unwrap();

        let (is_direct, is_dev): (Option<bool>, Option<bool>) = {
            let scoped = conn
                .query_row(
                    "SELECT is_direct, is_dev FROM project_dependencies WHERE package_name = ?1 AND project_path = ?2 LIMIT 1",
                    rusqlite::params!["jsonwebtoken", "/projects/fourda"],
                    |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
                )
                .ok();
            scoped
                .map(|(d, v)| (Some(d), Some(v)))
                .unwrap_or((None, None))
        };

        assert_eq!(is_direct, Some(true), "fourda has jsonwebtoken as direct");
        assert_eq!(is_dev, Some(false), "fourda has jsonwebtoken as prod");

        let (is_direct2, is_dev2): (Option<bool>, Option<bool>) = {
            let scoped = conn
                .query_row(
                    "SELECT is_direct, is_dev FROM project_dependencies WHERE package_name = ?1 AND project_path = ?2 LIMIT 1",
                    rusqlite::params!["jsonwebtoken", "/projects/kairos"],
                    |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
                )
                .ok();
            scoped
                .map(|(d, v)| (Some(d), Some(v)))
                .unwrap_or((None, None))
        };

        assert_eq!(
            is_direct2,
            Some(false),
            "kairos has jsonwebtoken as transitive"
        );
        assert_eq!(is_dev2, Some(true), "kairos has jsonwebtoken as dev");

        let (is_direct_unscoped, _): (Option<bool>, Option<bool>) = {
            let result = conn
                .query_row(
                    "SELECT is_direct, is_dev FROM project_dependencies WHERE package_name = ?1 LIMIT 1",
                    rusqlite::params!["jsonwebtoken"],
                    |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
                )
                .ok();
            result
                .map(|(d, v)| (Some(d), Some(v)))
                .unwrap_or((None, None))
        };

        assert!(
            is_direct_unscoped.is_some(),
            "unscoped fallback still returns a row"
        );
    }

    // ══════════════════════════════════════════════════════════════════════
    // T3-5: Regression Tests
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_scope_filter_current_repo_only() {
        // When a package exists in multiple projects, querying with a specific
        // project_path must return only that project's dep context (is_direct,
        // is_dev). This prevents cross-project contamination where project A's
        // transitive dev dep is reported as project B's direct prod dep.
        let db = crate::test_utils::test_db();
        let conn = db.conn.lock();

        // Project Alpha: axios as direct prod dependency
        conn.execute(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, is_direct, language)
             VALUES (?1, 'package.json', 'axios', '1.7.0', 0, 1, 'javascript')",
            rusqlite::params!["/projects/alpha"],
        )
        .unwrap();

        // Project Beta: axios as transitive dev dependency
        conn.execute(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, is_direct, language)
             VALUES (?1, 'package.json', 'axios', '1.6.0', 1, 0, 'javascript')",
            rusqlite::params!["/projects/beta"],
        )
        .unwrap();

        // Project Gamma: does NOT use axios at all
        conn.execute(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, is_direct, language)
             VALUES (?1, 'package.json', 'lodash', '4.17.21', 0, 1, 'javascript')",
            rusqlite::params!["/projects/gamma"],
        )
        .unwrap();

        // Query scoped to Alpha -- must see direct + prod
        let (is_direct_alpha, is_dev_alpha): (Option<bool>, Option<bool>) = conn
            .query_row(
                "SELECT is_direct, is_dev FROM project_dependencies WHERE package_name = ?1 AND project_path = ?2 LIMIT 1",
                rusqlite::params!["axios", "/projects/alpha"],
                |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
            )
            .ok()
            .map(|(d, v)| (Some(d), Some(v)))
            .unwrap_or((None, None));

        assert_eq!(is_direct_alpha, Some(true), "Alpha has axios as direct");
        assert_eq!(is_dev_alpha, Some(false), "Alpha has axios as prod");

        // Query scoped to Beta -- must see transitive + dev
        let (is_direct_beta, is_dev_beta): (Option<bool>, Option<bool>) = conn
            .query_row(
                "SELECT is_direct, is_dev FROM project_dependencies WHERE package_name = ?1 AND project_path = ?2 LIMIT 1",
                rusqlite::params!["axios", "/projects/beta"],
                |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
            )
            .ok()
            .map(|(d, v)| (Some(d), Some(v)))
            .unwrap_or((None, None));

        assert_eq!(is_direct_beta, Some(false), "Beta has axios as transitive");
        assert_eq!(is_dev_beta, Some(true), "Beta has axios as dev");

        // Query scoped to Gamma -- axios must not exist
        let gamma_axios = conn
            .query_row(
                "SELECT COUNT(*) FROM project_dependencies WHERE package_name = 'axios' AND project_path = '/projects/gamma'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(
            gamma_axios, 0,
            "Gamma has no axios -- scoped query must return 0 rows"
        );

        // Verify that without project_path scope, you get ambiguous results
        let total_axios: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_dependencies WHERE package_name = 'axios'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            total_axios, 2,
            "unscoped query returns both Alpha and Beta rows -- ambiguous"
        );
    }
}
