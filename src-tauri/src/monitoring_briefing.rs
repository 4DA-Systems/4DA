// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Morning briefing generation for 4DA monitoring.
//!
//! Generates formatted briefing text for CLI output and frontend display.
//! Handles morning briefing scheduling, date tracking, and notification content.

use chrono::Timelike;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
#[cfg(not(target_os = "windows"))]
use tauri_plugin_notification::NotificationExt;
use tracing::{info, warn};

use crate::monitoring::MonitoringState;
use crate::monitoring_notifications::truncate_safe;

// ============================================================================
// Data Freshness
// ============================================================================

/// Summarizes the staleness of source data. Attached to briefings and blind spot
/// reports so the frontend can distinguish "nothing happening" from "data pipeline
/// is broken / sources are offline."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ts_rs::TS)]
#[ts(export, export_to = "bindings/")]
pub struct DataFreshness {
    /// Hours since the newest item was first discovered (None if table is empty).
    pub newest_item_age_hours: Option<f64>,
    /// Number of source items first discovered in the last 24 hours.
    pub items_last_24h: u32,
    /// Number of source items first discovered in the last 72 hours.
    pub items_last_72h: u32,
    /// Hours since any source adapter last succeeded (None if health has never run).
    #[serde(default)]
    pub newest_source_check_age_hours: Option<f64>,
    /// Number of successful source checks in the last 24 hours.
    #[serde(default)]
    pub source_checks_last_24h: u32,
    /// Number of successful source checks in the last 72 hours.
    #[serde(default)]
    pub source_checks_last_72h: u32,
    /// Source adapters currently reporting failures or open circuits.
    #[serde(default)]
    pub failing_sources: u32,
    /// Source adapters with no successful check in more than seven days.
    #[serde(default)]
    pub stale_sources: u32,
    /// Total registered source adapters (for computing health percentage).
    #[serde(default)]
    pub total_sources: u32,
    /// True when neither source items nor successful source checks have appeared in 3 days.
    pub is_stale: bool,
    /// True when no source adapter has succeeded in the last 24 hours,
    /// even if the system is not fully stale. Signals degraded freshness.
    pub no_recent_fetches: bool,
    /// Items scored in the most recent fetch+score cycle, read from the latest
    /// `engine_runs` receipt (external-verifier ground truth). Written by BOTH the in-app
    /// background scheduler AND the headless engine — so it is populated even when
    /// the external verifier / headless task is disabled. None only when no
    /// cycle has ever recorded a receipt (brand-new install); the freshness line
    /// then falls back to the `newest_item_age_hours` watermark.
    #[serde(default)]
    pub last_run_items_scored: Option<u32>,
    /// Source adapters that succeeded in the most recent recorded cycle.
    #[serde(default)]
    pub last_run_sources_succeeded: Option<u32>,
    /// Source adapters that failed in the most recent recorded cycle.
    #[serde(default)]
    pub last_run_sources_failed: Option<u32>,
    /// Minutes since the most recent recorded cycle completed. None with no receipt.
    #[serde(default)]
    pub last_run_age_minutes: Option<f64>,
}

/// Query source_items to compute a DataFreshness snapshot.
/// Returns None only if the database is unreachable.
pub(crate) fn compute_data_freshness() -> Option<DataFreshness> {
    let conn = crate::open_db_connection().ok()?;
    Some(compute_data_freshness_from_conn(&conn))
}

pub(crate) fn compute_data_freshness_from_conn(conn: &rusqlite::Connection) -> DataFreshness {
    let freshest_item_expr = "created_at";
    let newest_age: Option<f64> = conn
        .query_row(
            &format!(
                "SELECT (julianday('now') - julianday(MAX({freshest_item_expr}))) * 24.0 FROM source_items"
            ),
            [],
            |row| row.get(0),
        )
        .ok();
    let items_24h: u32 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM source_items WHERE {freshest_item_expr} >= datetime('now', '-1 day')"
            ),
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let items_72h: u32 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM source_items WHERE {freshest_item_expr} >= datetime('now', '-3 days')"
            ),
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let newest_source_check_age: Option<f64> = conn
        .query_row(
            "SELECT (julianday('now') - julianday(MAX(last_success_at))) * 24.0 FROM feed_health",
            [],
            |row| row.get(0),
        )
        .ok();
    let source_checks_24h: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM feed_health WHERE last_success_at >= datetime('now', '-1 day')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let source_checks_72h: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM feed_health WHERE last_success_at >= datetime('now', '-3 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let failing_sources: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM feed_health WHERE consecutive_failures > 0 OR circuit_open = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let stale_sources: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM feed_health
             WHERE COALESCE(consecutive_failures, 0) = 0
               AND COALESCE(circuit_open, 0) = 0
               AND (last_success_at IS NULL OR last_success_at < datetime('now', '-7 days'))",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let total_sources: u32 = conn
        .query_row("SELECT COUNT(*) FROM feed_health", [], |row| row.get(0))
        .unwrap_or(0);

    // External-verifier freshness receipt — the most recent engine_runs row records what the last
    // fetch+score cycle actually did (ground truth, written by the GUI scheduler and the
    // headless engine alike). Powers the brief's "Scanned N · X/Y sources · Zm ago"
    // provenance line. The table may not exist on a brand-new DB; the query then errors
    // and we leave the fields None (the line falls back to the item watermark).
    let (
        last_run_items_scored,
        last_run_sources_succeeded,
        last_run_sources_failed,
        last_run_age_minutes,
    ): (Option<u32>, Option<u32>, Option<u32>, Option<f64>) = conn
        .query_row(
            "SELECT items_scored, sources_succeeded, sources_failed,
                    (julianday('now') - julianday(completed_at)) * 24.0 * 60.0
             FROM engine_runs ORDER BY id DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    Some(row.get::<_, i64>(0)?.max(0) as u32),
                    Some(row.get::<_, i64>(1)?.max(0) as u32),
                    Some(row.get::<_, i64>(2)?.max(0) as u32),
                    row.get::<_, Option<f64>>(3)?.map(|m| m.max(0.0)),
                ))
            },
        )
        .unwrap_or((None, None, None, None));

    DataFreshness {
        newest_item_age_hours: newest_age,
        items_last_24h: items_24h,
        items_last_72h: items_72h,
        newest_source_check_age_hours: newest_source_check_age,
        source_checks_last_24h: source_checks_24h,
        source_checks_last_72h: source_checks_72h,
        failing_sources,
        stale_sources,
        total_sources,
        is_stale: (items_72h == 0 && source_checks_72h == 0)
            || (total_sources > 0 && failing_sources * 100 / total_sources >= 50)
            || (total_sources > 0 && source_checks_72h == 0),
        no_recent_fetches: total_sources > 0 && source_checks_24h == 0,
        last_run_items_scored,
        last_run_sources_succeeded,
        last_run_sources_failed,
        last_run_age_minutes,
    }
}

/// Minimum relevance score for an item to appear in the morning briefing.
/// The briefing is a flagship surface — every bad signal erodes trust.
/// 0.50 keeps genuinely relevant content while cutting noise that scored
/// on weak keyword matches. Critical/alert priority items bypass this
/// via the signal classifier's own 0.30 threshold.
pub(crate) const BRIEFING_SCORE_FLOOR: f32 = 0.50;

// ============================================================================
// Morning Briefing Types
// ============================================================================

/// Item for morning briefing notification.
/// Enriched with url, item_id, matched_deps for the center-screen briefing window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingItem {
    pub title: String,
    pub source_type: String,
    pub score: f32,
    pub signal_type: Option<String>,
    /// URL for opening in system browser
    #[serde(default)]
    pub url: Option<String>,
    /// Database ID for deep-linking to item in main window
    #[serde(default)]
    pub item_id: Option<i64>,
    /// Signal priority tier (watch, advisory, alert, critical)
    #[serde(default)]
    pub signal_priority: Option<String>,
    /// Short description or action text
    #[serde(default)]
    pub description: Option<String>,
    /// Matched dependency names (why this matters to the user)
    #[serde(default)]
    pub matched_deps: Vec<String>,
    /// Content DNA classification (tutorial, security_advisory, etc.)
    #[serde(default)]
    pub content_type: Option<String>,
    /// Number of distinct sources corroborating this item (from topic clustering)
    #[serde(default)]
    pub corroboration_count: usize,
    /// Alternative sources that reported the same topic (cluster members)
    #[serde(default)]
    pub alt_sources: Vec<crate::topic_clustering::AltSource>,
    /// Which section of the brief this item belongs to: "action", "watch", or "reading"
    #[serde(default)]
    pub section: Option<String>,
    /// Why this item was triaged into its section (human-readable reason)
    #[serde(default)]
    pub triage_reason: Option<String>,
}

pub(crate) fn matched_deps_from_signal_triggers(triggers: Option<&[String]>) -> Vec<String> {
    let mut deps = Vec::new();
    for trigger in triggers.into_iter().flatten() {
        let Some(dep) = trigger.strip_prefix("dep:") else {
            continue;
        };
        let dep = dep.trim();
        if !dep.is_empty() && !deps.iter().any(|existing| existing == dep) {
            deps.push(dep.to_string());
        }
    }
    deps
}

pub(crate) fn load_briefing_matched_deps(
    conn: &rusqlite::Connection,
    source_item_id: i64,
) -> Vec<String> {
    conn.prepare_cached(
        "SELECT DISTINCT package_name
         FROM source_item_dependencies
         WHERE source_item_id = ?1
           AND match_type IN (
               'exact_registry', 'registry',
               'advisory', 'security_advisory', 'cve', 'vulnerability',
               'llm_analysis', 'llm_confirmed'
           )
         ORDER BY package_name",
    )
    .ok()
    .and_then(|mut stmt| {
        stmt.query_map(rusqlite::params![source_item_id], |row| {
            row.get::<_, String>(0)
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
    })
    .unwrap_or_default()
}

/// A topic the user hasn't seen intelligence about recently
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGap {
    pub topic: String,
    pub days_since_last: i64,
}

/// Summary of an escalating signal chain for inclusion in the briefing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainSummary {
    pub name: String,
    pub phase: String,
    pub link_count: usize,
    pub action: String,
    pub confidence: f64,
}

/// One underlying package's real versions inside an advisory-merged alert row.
///
/// When several packages share a single advisory they collapse into one display
/// row (see `merge_alert_packages`), at which point `package_name` becomes a
/// lossy comma-joined label ("@clerk/clerk-react, @clerk/shared") and the single
/// `installed_version`/`fixed_version` fields can only hold one package's pair.
/// The deterministic factual-version check needs every package's own versions,
/// so the merge preserves them here.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MergedPackageVersion {
    pub package_name: String,
    #[serde(default)]
    pub installed_version: Option<String>,
    #[serde(default)]
    pub fixed_version: Option<String>,
}

/// A preemption alert included in the morning briefing (critical/high only).
///
/// Carries the rich evidence fields from the backend `PreemptionAlert` so the
/// frontend can render actionable checklist cards (package, versions, projects,
/// advisory IDs) instead of opaque title/explanation blobs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BriefingPreemptionAlert {
    pub title: String,
    pub urgency: String,
    pub explanation: String,
    /// Unique alert identifier (e.g. "osv-pkg-axios-npm")
    #[serde(default)]
    pub alert_id: Option<String>,
    /// Primary affected package name (first entry from affected_dependencies)
    #[serde(default)]
    pub package_name: Option<String>,
    /// Package ecosystem (extracted from alert id, e.g. "npm", "crates.io")
    #[serde(default)]
    pub ecosystem: Option<String>,
    /// Currently installed version of the affected package
    #[serde(default)]
    pub installed_version: Option<String>,
    /// Version that fixes the vulnerability
    #[serde(default)]
    pub fixed_version: Option<String>,
    /// Project paths affected by this alert
    #[serde(default)]
    pub affected_projects: Vec<String>,
    /// Whether the affected package is a direct dependency
    #[serde(default)]
    pub is_direct: Option<bool>,
    /// Whether the affected package is a dev-only dependency
    #[serde(default)]
    pub is_dev: Option<bool>,
    /// Advisory identifiers extracted from evidence (e.g. GHSA-xxxx, CVE-xxxx)
    #[serde(default)]
    pub advisory_ids: Vec<String>,
    /// URL to the primary advisory source
    #[serde(default)]
    pub source_url: Option<String>,
    /// Human-readable action labels from suggested_actions
    #[serde(default)]
    pub suggested_actions: Vec<String>,
    /// Where this vulnerability lives relative to the primary project.
    /// "primary" = the 4DA app itself, "external" = a side project, "dev" = dev-only dependency.
    #[serde(default)]
    pub scope: Option<String>,
    /// Per-package versions for advisory-merged rows. Empty for a single-package
    /// alert (its own `package_name`/`installed_version`/`fixed_version` are
    /// faithful); populated by `merge_alert_packages` when multiple packages share
    /// one advisory, so the factual-version check can validate each package's real
    /// (installed, fix) pair rather than only the first merged package's.
    #[serde(default)]
    pub merged_package_versions: Vec<MergedPackageVersion>,
}

impl BriefingPreemptionAlert {
    /// Build a briefing alert from a full `PreemptionAlert`, carrying through
    /// all evidence fields the frontend needs for checklist cards.
    pub fn from_preemption_alert(a: &crate::preemption::PreemptionAlert) -> Self {
        let package_name = a.affected_dependencies.first().cloned();
        let ecosystem = extract_ecosystem_from_alert_id(&a.id);
        let source_url = a.evidence.iter().find_map(|e| e.url.clone());
        let advisory_ids = extract_advisory_ids_from_evidence(&a.evidence);
        let suggested_actions = a
            .suggested_actions
            .iter()
            .map(|sa| sa.label.clone())
            .collect();

        Self {
            title: a.title.clone(),
            urgency: format!("{:?}", a.urgency).to_lowercase(),
            explanation: a.explanation.clone(),
            alert_id: Some(a.id.clone()),
            package_name,
            ecosystem,
            installed_version: a.installed_version.clone(),
            fixed_version: a.fixed_version.clone(),
            affected_projects: a.affected_projects.clone(),
            is_direct: a.is_direct,
            is_dev: a.is_dev,
            advisory_ids,
            source_url,
            suggested_actions,
            scope: None,
            merged_package_versions: Vec::new(),
        }
    }
}

/// Extract ecosystem from alert id like "osv-pkg-axios-npm" -> Some("npm").
///
/// The id format is `osv-pkg-{package_name}-{ecosystem}`. Because the package
/// name may contain hyphens (e.g. "json-web-token"), we split on "osv-pkg-"
/// and take everything after the last hyphen as the ecosystem.
fn extract_ecosystem_from_alert_id(id: &str) -> Option<String> {
    let suffix = id.strip_prefix("osv-pkg-")?;
    let last_hyphen = suffix.rfind('-')?;
    let eco = &suffix[last_hyphen + 1..];
    if eco.is_empty() {
        None
    } else {
        Some(eco.to_string())
    }
}

/// Extract advisory identifiers (GHSA-xxxx-xxxx-xxxx, CVE-yyyy-nnnnn) from
/// evidence titles and URLs. Uses simple string scanning to avoid adding a
/// regex dependency.
fn extract_advisory_ids_from_evidence(
    evidence: &[crate::preemption::AlertEvidence],
) -> Vec<String> {
    let mut ids = Vec::new();
    for ev in evidence {
        let texts: Vec<&str> = std::iter::once(ev.title.as_str())
            .chain(std::iter::once(ev.source.as_str()))
            .chain(ev.url.iter().map(|s| s.as_str()))
            .collect();
        for text in texts {
            extract_ids_from_text(text, &mut ids);
        }
    }
    ids
}

/// Collapse briefing preemption alerts that describe the SAME advisory hitting
/// different packages into a single entry that lists the affected packages.
///
/// A single CVE (e.g. GHSA-w24r-5266-9c3c) can flag several installed packages
/// (@clerk/clerk-react AND @clerk/shared). Surfaced raw, that is two
/// near-identical rows consuming two of the brief's five slots. Keyed by the
/// sorted advisory-id set; alerts with no advisory id pass through untouched
/// (we can't prove two unidentified alerts are the same). Order is preserved,
/// so the critical-first ranking of the caller is retained.
fn dedupe_alerts_by_advisory(alerts: Vec<BriefingPreemptionAlert>) -> Vec<BriefingPreemptionAlert> {
    use std::collections::HashMap;
    let mut out: Vec<BriefingPreemptionAlert> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for alert in alerts {
        if alert.advisory_ids.is_empty() {
            out.push(alert);
            continue;
        }
        let mut ids = alert.advisory_ids.clone();
        ids.sort();
        let key = ids.join("|");
        if let Some(&i) = index.get(&key) {
            merge_alert_packages(&mut out[i], &alert);
        } else {
            index.insert(key, out.len());
            out.push(alert);
        }
    }
    out
}

/// Fold a duplicate advisory alert's package + projects into the kept alert so
/// the single row reflects every affected package (e.g. package_name becomes
/// "@clerk/clerk-react, @clerk/shared").
fn merge_alert_packages(keep: &mut BriefingPreemptionAlert, dup: &BriefingPreemptionAlert) {
    // Before the display name is lossily comma-joined, capture the kept alert's
    // OWN package + versions so the per-package fact list stays complete. Seed
    // once, on the first merge, while `keep.package_name` is still a single name.
    if keep.merged_package_versions.is_empty() {
        if let Some(p) = keep.package_name.as_deref().filter(|p| !p.is_empty()) {
            keep.merged_package_versions.push(MergedPackageVersion {
                package_name: p.to_string(),
                installed_version: keep.installed_version.clone(),
                fixed_version: keep.fixed_version.clone(),
            });
        }
    }
    if let Some(dup_pkg) = dup.package_name.as_deref().filter(|p| !p.is_empty()) {
        match keep.package_name.clone() {
            Some(existing) if !existing.split(", ").any(|p| p == dup_pkg) => {
                keep.package_name = Some(format!("{existing}, {dup_pkg}"));
            }
            None => keep.package_name = Some(dup_pkg.to_string()),
            _ => {}
        }
        // Preserve the merged package's real versions for the factual check.
        if !keep
            .merged_package_versions
            .iter()
            .any(|m| m.package_name == dup_pkg)
        {
            keep.merged_package_versions.push(MergedPackageVersion {
                package_name: dup_pkg.to_string(),
                installed_version: dup.installed_version.clone(),
                fixed_version: dup.fixed_version.clone(),
            });
        }
    }
    for proj in &dup.affected_projects {
        if !keep.affected_projects.contains(proj) {
            keep.affected_projects.push(proj.clone());
        }
    }
    // A direct hit anywhere makes the merged row direct.
    if dup.is_direct == Some(true) {
        keep.is_direct = Some(true);
    }
}

/// Scan `text` for GHSA-xxxx-xxxx-xxxx and CVE-YYYY-NNNNN patterns,
/// appending unique matches to `out`.
fn extract_ids_from_text(text: &str, out: &mut Vec<String>) {
    // GHSA pattern: "GHSA-" followed by three groups of 4 alphanumeric chars separated by hyphens
    let mut start = 0;
    while let Some(pos) = text[start..].find("GHSA-") {
        let abs = start + pos;
        // GHSA-xxxx-xxxx-xxxx = 19 chars total
        if abs + 19 <= text.len() {
            let candidate = &text[abs..abs + 19];
            if is_valid_ghsa(candidate) {
                let id = candidate.to_string();
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        start = abs + 5;
    }

    // CVE pattern: "CVE-" followed by 4-digit year, hyphen, 4+ digit sequence
    start = 0;
    while let Some(pos) = text[start..].find("CVE-") {
        let abs = start + pos;
        let rest = &text[abs + 4..];
        if let Some(id_suffix) = parse_cve_suffix(rest) {
            let full_id = format!("CVE-{id_suffix}");
            if !out.contains(&full_id) {
                out.push(full_id);
            }
        }
        start = abs + 4;
    }
}

/// Check if a 19-char string matches GHSA-[a-z0-9]{4}-[a-z0-9]{4}-[a-z0-9]{4}
fn is_valid_ghsa(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 19 {
        return false;
    }
    if bytes[9] != b'-' || bytes[14] != b'-' {
        return false;
    }
    for &i in &[5, 6, 7, 8, 10, 11, 12, 13, 15, 16, 17, 18] {
        if !bytes[i].is_ascii_alphanumeric() {
            return false;
        }
    }
    true
}

/// Parse a CVE id suffix: "YYYY-NNNNN..." returning "YYYY-NNNNN" portion.
fn parse_cve_suffix(rest: &str) -> Option<&str> {
    if rest.len() < 9 {
        return None;
    }
    if !rest[..4].chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if rest.as_bytes()[4] != b'-' {
        return None;
    }
    let num_start = 5;
    let num_end = rest[num_start..]
        .find(|c: char| !c.is_ascii_digit())
        .map(|p| num_start + p)
        .unwrap_or(rest.len());
    if num_end - num_start < 4 {
        return None;
    }
    Some(&rest[..num_end])
}

/// Translated labels for the briefing window UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingLabels {
    pub header: String,
    pub escalating: String,
    pub signals: String,
    pub blind_spots: String,
    pub tracking: String,
    pub gap_days_suffix: String,
    pub signals_today_suffix: String,
    pub empty_state: String,
    pub preemption: String,
    pub blind_spots_score: String,
}

/// Morning briefing notification content.
/// Enriched for center-screen briefing window with knowledge gaps, ongoing topics,
/// and escalating signal chains.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingNotification {
    pub title: String,
    pub items: Vec<BriefingItem>,
    pub total_relevant: usize,
    /// Topics that appeared in recent briefings (ongoing, not novel)
    #[serde(default)]
    pub ongoing_topics: Vec<String>,
    /// Declared tech topics with no recent signals
    #[serde(default)]
    pub knowledge_gaps: Vec<KnowledgeGap>,
    /// Signal chains in escalating or peak phase — top-level briefing section
    #[serde(default)]
    pub escalating_chains: Vec<ChainSummary>,
    /// LLM-synthesized intelligence narrative (populated async after initial delivery)
    #[serde(default)]
    pub synthesis: Option<String>,
    /// Preemption alerts — critical/high urgency items from the preemption engine
    #[serde(default)]
    pub preemption_alerts: Vec<BriefingPreemptionAlert>,
    /// Blind spot summary — quick overview of coverage gaps (0-100, higher = more gaps)
    #[serde(default)]
    pub blind_spot_score: Option<f32>,
    /// Translated labels for the briefing window (i18n)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<BriefingLabels>,
    /// First-briefing personalization: natural-language context line proving the
    /// system understood the user's profile. Only set on the very first briefing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personalization_context: Option<String>,
    /// Source data freshness summary. When is_stale is true, the frontend should
    /// show a staleness warning instead of treating silence as "all clear."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_freshness: Option<DataFreshness>,
    /// Whether corroboration detection ran successfully (embeddings available).
    /// When false, the frontend should not interpret absence of corroboration badges
    /// as "uncorroborated" — the system simply couldn't check.
    #[serde(default)]
    pub corroboration_available: bool,
    /// True when the system has <7 days of briefing history. Signals the frontend
    /// to show "building coverage model" instead of implying complete coverage.
    #[serde(default)]
    pub coverage_building: bool,
    /// Hint message shown when LLM synthesis is unavailable. Tells the user
    /// what to configure so the briefing can include intelligence narrative.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synthesis_hint: Option<String>,
}

const KNOWLEDGE_GAP_HIGH_URGENCY_DAYS: i64 = 7;

impl BriefingNotification {
    /// Returns true if any section has meaningful intelligence worth showing.
    /// Used instead of `items.is_empty()` so preemption-only briefings still fire.
    pub fn has_meaningful_content(&self) -> bool {
        if !self.items.is_empty() {
            return true;
        }
        if !self.preemption_alerts.is_empty() {
            return true;
        }
        if !self.escalating_chains.is_empty() {
            return true;
        }
        self.knowledge_gaps
            .iter()
            .any(|g| g.days_since_last > KNOWLEDGE_GAP_HIGH_URGENCY_DAYS)
    }
}

/// Returns true if the synthesis text is a "nothing to report" abstention.
///
/// Anchored to the canonical abstention opening on the FIRST LINE only, folding
/// every dash variant (the prompt's literal "--", the em/en-dash) and whitespace
/// run into single spaces. Mirrors the frontend `isAbstentionSynthesis`
/// (briefing-synthesis-helpers.ts) so both sides agree on the contract.
///
/// Anchoring matters because this gate also clears the cold-boot snapshot
/// synthesis (`briefing_snapshot::drop_contradictory_abstention`): a lax
/// `contains("no noteworthy")` would strip a legitimate brief that merely uses
/// the phrase mid-prose ("...no noteworthy followups on Tokio yet..."), and a
/// bare `starts_with("low signal")` would misfire on "Low signal strength on the
/// wifi today, but plenty to report."
pub(crate) fn is_abstention_synthesis(text: &str) -> bool {
    let first_line = text.trim_start().lines().next().unwrap_or("");
    // Fold dashes (incl. the literal "--") to spaces so split_whitespace also
    // breaks on them; then collapse to single-spaced lowercase.
    let folded: String = first_line
        .chars()
        .map(|c| {
            if matches!(c, '\u{2010}'..='\u{2015}' | '\u{2212}' | '-') {
                ' '
            } else {
                c
            }
        })
        .flat_map(char::to_lowercase)
        .collect();
    let normalized = folded.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized.starts_with("low signal no noteworthy intelligence")
        || normalized.starts_with("low signal no new intelligence")
}

// ============================================================================
// Enrichment Pipeline
// ============================================================================

/// Build an enriched briefing from raw items.
///
/// Applies the full quality pipeline: quality gate → dedupe → cap → novelty →
/// knowledge gaps → escalating chains → preemption alerts →
/// blind spot score. Called by both the scheduled briefing and manual trigger.
///
/// When `skip_novelty` is true (manual trigger), the novelty filter is skipped
/// so the user always sees content for testing purposes, and no history is
/// recorded to avoid polluting the novelty database.
pub(crate) fn build_enriched_briefing(
    raw_items: Vec<BriefingItem>,
    lang: &str,
    skip_novelty: bool,
) -> BriefingNotification {
    let now = chrono::Local::now();

    // Quality gate — drops garbled/marketing/clickbait titles.
    let quality_filtered: Vec<BriefingItem> = raw_items
        .into_iter()
        .filter(|item| {
            match crate::briefing_quality::is_briefing_worthy(&item.title, &item.source_type) {
                Ok(()) => true,
                Err(reason) => {
                    tracing::debug!(
                        target: "4da::briefing",
                        title = %item.title,
                        source = %item.source_type,
                        reject_reason = reason.as_str(),
                        "briefing-quality gate rejected item"
                    );
                    false
                }
            }
        })
        .collect();

    // Intra-batch fuzzy dedupe — collapses semantic duplicates.
    let deduped = crate::briefing_dedupe::dedupe_briefing_items(quality_filtered);

    // Priority-aware sort: actionable intel (security/breaking) leads, then
    // signal priority, score, corroboration, and finally a deterministic id
    // tiebreak. The briefing is a curated surface — high-priority items MUST
    // lead, and byte-identical scores (the scoring soft-ceiling can pin distinct
    // items to the same value) must never order arbitrarily.
    let mut sorted = deduped;
    sorted.sort_by(briefing_item_cmp);

    // Corroboration detection on the WIDE pre-diversity pool. Clustering must see
    // same-topic duplicates BEFORE diversity slotting spreads items across
    // sources and discards the copies — running it on the final ~8 (as it used
    // to) structurally found almost nothing, so every corroboration_count was 0.
    // The corroboration bonus can shift ordering, so re-sort before selecting.
    let corroboration_available = apply_topic_clustering(&mut sorted);
    sorted.sort_by(briefing_item_cmp);

    // Diversity slots: guarantee at least 1 item per source that produced
    // results, then fill remaining slots from the global ranking. This
    // prevents high-scoring source types (security, fresh news) from
    // dominating the entire briefing.
    let items: Vec<BriefingItem> = apply_diversity_slots(sorted, 8);

    // Pre-load cross-surface intelligence BEFORE the quality gate so that
    // preemption alerts and escalating chains can bypass it. A critical
    // security alert should trigger the briefing even if scored items are weak.
    let escalating_chains = detect_escalating_chains();
    let preemption_alerts: Vec<BriefingPreemptionAlert> =
        match crate::preemption::get_preemption_feed() {
            Ok(feed) => {
                // Build a generous pool (critical first, then high), collapse alerts
                // that share an advisory — one CVE hitting several of your packages
                // (e.g. GHSA-w24r-5266-9c3c on @clerk/clerk-react AND @clerk/shared)
                // should be ONE row listing both packages, not two near-identical
                // rows eating two of the five slots — then cut to five.
                let critical = feed
                    .alerts
                    .iter()
                    .filter(|a| matches!(a.urgency, crate::preemption::AlertUrgency::Critical));
                let high = feed
                    .alerts
                    .iter()
                    .filter(|a| matches!(a.urgency, crate::preemption::AlertUrgency::High));
                let pool: Vec<BriefingPreemptionAlert> = critical
                    .chain(high)
                    .map(BriefingPreemptionAlert::from_preemption_alert)
                    .collect();
                dedupe_alerts_by_advisory(pool)
                    .into_iter()
                    .take(5)
                    .collect()
            }
            Err(_) => Vec::new(),
        };

    // Compute project scope for each alert — distinguishes "your shipping code"
    // from side projects and dev dependencies.
    let project_root = std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().to_lowercase().replace('\\', "/"));
    let preemption_alerts: Vec<BriefingPreemptionAlert> = preemption_alerts
        .into_iter()
        .map(|mut alert| {
            let scope = if alert.is_dev == Some(true) {
                "dev"
            } else if let Some(ref root) = project_root {
                let is_primary = alert.affected_projects.iter().any(|p| {
                    let normalized = p.to_lowercase().replace('\\', "/");
                    normalized.starts_with(root.as_str())
                });
                if is_primary {
                    "primary"
                } else {
                    "external"
                }
            } else {
                "external"
            };
            alert.scope = Some(scope.to_string());
            alert
        })
        .collect();

    // Minimum quality gate: the briefing only fires when it has genuinely
    // valuable content OR cross-surface intelligence (preemption, chains).
    // Commodity content (Tutorial, HelpRequest, Question below 0.45) doesn't
    // count toward the quality minimum.
    let quality_item_count = items
        .iter()
        .filter(|i| i.score >= BRIEFING_SCORE_FLOOR && !is_low_quality_commodity(i))
        .count();
    let has_high_value_single = items
        .iter()
        .any(|i| i.score >= 0.65 && !is_low_quality_commodity(i));
    let has_cross_surface = !preemption_alerts.is_empty() || !escalating_chains.is_empty();
    if quality_item_count < 2 && !has_high_value_single && !has_cross_surface {
        return BriefingNotification {
            title: format!("4DA Intelligence Briefing — {}", now.format("%d %b %Y")),
            items: vec![],
            total_relevant: 0,
            ongoing_topics: vec![],
            knowledge_gaps: vec![],
            escalating_chains: vec![],
            synthesis: None,
            preemption_alerts: vec![],
            blind_spot_score: None,
            labels: Some(build_briefing_labels(lang)),
            personalization_context: None,
            data_freshness: None,
            corroboration_available: false,
            coverage_building: false,
            synthesis_hint: None,
        };
    }

    // Novelty detection: filter out items and preemption alerts shown in the
    // last 14 days. This prevents the briefing from recycling the same content
    // every morning. Stale items/alerts are surfaced as "ongoing topics" in
    // the footer so the user knows they're still tracked.
    let today = now.format("%Y-%m-%d").to_string();
    let (items, mut ongoing_topics) = if skip_novelty {
        (items, vec![])
    } else {
        let (novel, ongoing) = apply_novelty_filter(items, &today);
        (novel, ongoing)
    };

    let preemption_alerts = if skip_novelty {
        preemption_alerts
    } else {
        let (novel_alerts, stale_alert_topics) =
            apply_preemption_novelty_filter(preemption_alerts, &today);
        ongoing_topics.extend(stale_alert_topics);
        ongoing_topics.sort();
        ongoing_topics.dedup();
        novel_alerts
    };

    // Abstention: when novelty filtering removed all items AND all preemption
    // alerts, the briefing has nothing new. Return a "low signal" abstention; the
    // frontend pairs it with the freshness line (data_freshness) so silence
    // reads as proof the system looked, not as a void. Absence lists ("still
    // tracking" / "quiet sources") are deliberately omitted — see the main return.
    if items.is_empty() && preemption_alerts.is_empty() && !ongoing_topics.is_empty() {
        return BriefingNotification {
            title: format!("4DA Intelligence Briefing — {}", now.format("%d %b %Y")),
            items: vec![],
            total_relevant: 0,
            ongoing_topics: vec![],
            knowledge_gaps: vec![],
            escalating_chains,
            synthesis: Some("Low signal — no new intelligence overnight.".to_string()),
            preemption_alerts: vec![],
            blind_spot_score: None,
            labels: Some(build_briefing_labels(lang)),
            personalization_context: None,
            data_freshness: compute_data_freshness(),
            corroboration_available: false,
            coverage_building: false,
            synthesis_hint: None,
        };
    }

    // Corroboration was already computed on the wide pre-diversity pool above;
    // the metadata travels with the surviving items here.

    // Split items into action-first sections.
    // Actions (security, breaking changes, etc.) lead the briefing.
    // Watch items are non-actionable but corroborated and high-scoring.
    // Reading items are everything else — informational, lower confidence.
    let items = apply_section_split(items);

    // Cross-surface dedup: remove scored items that already appear as preemption alerts.
    let items: Vec<BriefingItem> = if preemption_alerts.is_empty() {
        items
    } else {
        let alert_titles: Vec<String> = preemption_alerts
            .iter()
            .map(|a| a.title.to_lowercase())
            .collect();
        // Also dedup by shared advisory id. A scored news item about CVE-X and a
        // preemption alert for CVE-X have completely different titles, so the
        // exact-title match alone never catches them and the same vulnerability
        // appears twice (once as an "action" item, once as an alert).
        let alert_ids: std::collections::HashSet<String> = preemption_alerts
            .iter()
            .flat_map(|a| a.advisory_ids.iter().map(|id| id.to_uppercase()))
            .collect();
        items
            .into_iter()
            .filter(|item| {
                let title_l = item.title.to_lowercase();
                if alert_titles.iter().any(|at| at == &title_l) {
                    return false;
                }
                if !alert_ids.is_empty() {
                    let mut ids = Vec::new();
                    extract_ids_from_text(&item.title, &mut ids);
                    if ids.iter().any(|id| alert_ids.contains(&id.to_uppercase())) {
                        return false;
                    }
                }
                true
            })
            .collect()
    };

    let total_relevant = items.len();

    // escalating_chains and preemption_alerts already loaded before quality gate

    // Collect blind spot score
    let blind_spot_score = crate::blind_spots::generate_blind_spot_report()
        .ok()
        .map(|r| r.overall_score)
        .filter(|&score| score > 0.0);

    // Build translated labels
    let labels = build_briefing_labels(lang);

    // First-briefing personalization: if no previous briefing exists in history,
    // generate a context line showing the user their detected stack + interests.
    let personalization_context = build_first_briefing_context();

    // Cold-start detection: <7 distinct briefing days means the system is still
    // building its coverage model. Prevents false sense of complete coverage.
    let coverage_building = is_coverage_building();

    BriefingNotification {
        title: format!("4DA Intelligence Briefing — {}", now.format("%d %b %Y")),
        items,
        total_relevant,
        // Absence sections ("Still tracking" / "Quiet in your sources") are intentionally
        // not surfaced in the brief — they report what is NOT happening, inform no action,
        // and violate Intelligence Doctrine rule #3 (no vanity metrics). `ongoing_topics` is
        // still computed above for the novelty filter; it just no longer reaches the UI.
        ongoing_topics: vec![],
        knowledge_gaps: vec![],
        escalating_chains,
        synthesis: None,
        preemption_alerts,
        blind_spot_score,
        labels: Some(labels),
        personalization_context,
        data_freshness: compute_data_freshness(),
        corroboration_available,
        coverage_building,
        synthesis_hint: {
            let llm = crate::get_settings_manager().lock().get().llm.clone();
            let is_cloud = matches!(
                llm.provider.as_str(),
                "anthropic" | "openai" | "openai-compatible"
            );
            if !is_cloud || llm.api_key.is_empty() {
                Some("No cloud API key configured — configure Anthropic or OpenAI in Settings to enable intelligence synthesis".to_string())
            } else {
                None
            }
        },
    }
}

// ============================================================================
// Diversity Slot Selection
// ============================================================================

/// Select briefing items with source diversity guarantees.
///
/// Ensures at least one item per source type that produced results, then
/// fills remaining slots from the global ranking (already priority-sorted).
/// This prevents a single dominant source type (e.g. security advisories)
/// from consuming all briefing slots.
///
/// Content types that represent actionable intelligence — things a developer
/// might need to DO something about. These get priority in the brief's hero section.
fn is_actionable_content_type(content_type: Option<&str>) -> bool {
    matches!(
        content_type,
        Some("security_advisory")
            | Some("vulnerability_report")
            | Some("cve")
            | Some("breaking_change")
            | Some("deprecation_notice")
            | Some("release_notes")
            | Some("platform_update")
            | Some("migration_guide")
    )
}

/// Content types that are informational but not actionable — good for awareness
/// but shouldn't displace actionable items in the hero section.
#[cfg(test)]
fn is_optional_reading_type(content_type: Option<&str>) -> bool {
    matches!(
        content_type,
        Some("tutorial")
            | Some("show_and_tell")
            | Some("opinion")
            | Some("blog_post")
            | Some("discussion")
            | Some("general")
            | Some("announcement")
            | Some("comparison")
    ) || content_type.is_none()
}

/// Check if a briefing item is low-quality commodity content.
/// Commodity = Tutorial/HelpRequest/Question/ShowAndTell with score below 0.45,
/// or any item with NULL content_type (cannot be verified as action material).
/// These items don't count toward the minimum quality gate.
fn is_low_quality_commodity(item: &BriefingItem) -> bool {
    // NULL content_type items are unclassified — treat as low confidence
    if item.content_type.is_none() && item.score < 0.55 {
        return true;
    }
    if item.score >= 0.45 {
        return false;
    }
    matches!(
        item.content_type.as_deref(),
        Some("tutorial") | Some("help_request") | Some("question") | Some("show_and_tell")
    )
}

/// Apply topic clustering to briefing items for corroboration detection.
///
/// Loads embeddings from the DB for each item that has an `item_id`, runs
/// the clustering algorithm, then applies corroboration bonuses and populates
/// `alt_sources` / `corroboration_count` on lead items. Best-effort: if the
/// DB is unavailable or items lack embeddings, the items pass through unchanged.
fn apply_topic_clustering(items: &mut Vec<BriefingItem>) -> bool {
    // Collect item IDs for DB lookup
    let ids: Vec<i64> = items.iter().filter_map(|item| item.item_id).collect();

    if ids.is_empty() {
        return false;
    }

    // Load embeddings from the DB
    let embedding_data = match crate::get_database() {
        Ok(db) => match db.get_embeddings_for_ids(&ids) {
            Ok(data) => data,
            Err(e) => {
                tracing::debug!(
                    target: "4da::clustering",
                    error = %e,
                    "Failed to load embeddings for clustering — skipping"
                );
                return false;
            }
        },
        Err(_) => return false,
    };

    if embedding_data.is_empty() {
        tracing::info!(
            target: "4da::clustering",
            item_count = ids.len(),
            "No embeddings available — corroboration detection skipped for this briefing"
        );
        return false;
    }

    // Build cluster candidates by joining briefing items with their embeddings
    let embedding_map: std::collections::HashMap<i64, (Vec<f32>, Option<String>)> = embedding_data
        .into_iter()
        .map(|(id, _title, _source, emb, ct)| (id, (emb, ct)))
        .collect();

    let candidates: Vec<crate::topic_clustering::ClusterCandidate> = items
        .iter()
        .filter_map(|item| {
            let id = item.item_id?;
            let (embedding, content_type) = embedding_map.get(&id)?.clone();
            Some(crate::topic_clustering::ClusterCandidate {
                id,
                score: item.score,
                source_type: item.source_type.clone(),
                embedding,
                title: item.title.clone(),
                content_type,
            })
        })
        .collect();

    if candidates.is_empty() {
        return false;
    }

    let clusters = crate::topic_clustering::cluster_items(&candidates);
    let bonuses = crate::topic_clustering::compute_corroboration_bonuses(&clusters);

    // Apply corroboration bonuses and set metadata on lead items
    for item in items.iter_mut() {
        let Some(id) = item.item_id else { continue };

        // Apply score bonus
        if let Some(&bonus) = bonuses.get(&id) {
            item.score = (item.score + bonus).min(0.95);
        }

        // Set corroboration metadata on lead items
        for cluster in &clusters {
            if cluster.lead_item_id == id && cluster.source_count > 1 {
                item.corroboration_count = cluster.source_count;
                for &member_id in &cluster.member_ids {
                    if member_id != cluster.lead_item_id {
                        if let Some(member) = candidates.iter().find(|c| c.id == member_id) {
                            item.alt_sources.push(crate::topic_clustering::AltSource {
                                source_type: member.source_type.clone(),
                                url: None,
                                title: member.title.clone(),
                            });
                        }
                    }
                }
                break;
            }
        }
    }

    true
}

/// Items are assumed to be pre-sorted by priority then score descending.
/// The final output preserves priority ordering: diversity picks are
/// re-sorted by the same priority-then-score comparator.
fn apply_diversity_slots(items: Vec<BriefingItem>, max_items: usize) -> Vec<BriefingItem> {
    if items.is_empty() || max_items == 0 {
        return vec![];
    }

    // Single source or fits entirely — no diversity logic needed
    let source_count = {
        let mut seen = std::collections::HashSet::new();
        for item in &items {
            seen.insert(item.source_type.clone());
        }
        seen.len()
    };
    if source_count <= 1 || items.len() <= max_items {
        return items.into_iter().take(max_items).collect();
    }

    // Best item per source (first occurrence wins since list is pre-sorted)
    let mut best_per_source: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (idx, item) in items.iter().enumerate() {
        best_per_source
            .entry(item.source_type.clone())
            .or_insert(idx);
    }

    // Collect diversity picks (indices)
    let mut selected_indices: Vec<usize> = best_per_source.into_values().collect();
    selected_indices.sort_unstable(); // preserve original ordering

    // If diversity picks alone exceed budget, keep the best ones by
    // original sort order (priority-then-score).
    if selected_indices.len() >= max_items {
        selected_indices.truncate(max_items);
    } else {
        // Fill remaining slots from the global ranking, skipping already-selected
        let selected_set: std::collections::HashSet<usize> =
            selected_indices.iter().copied().collect();
        for idx in 0..items.len() {
            if selected_indices.len() >= max_items {
                break;
            }
            if !selected_set.contains(&idx) {
                selected_indices.push(idx);
            }
        }
    }

    // Sort selected indices to preserve original priority-then-score ordering
    selected_indices.sort_unstable();

    // Collect items by index (indices are unique and sorted, so no double-moves)
    selected_indices
        .into_iter()
        .map(|idx| items[idx].clone())
        .collect()
}

// ============================================================================
// Section Split (Action → Watch → Reading)
// ============================================================================

/// Split briefing items into action-first sections and recombine.
///
/// The briefing prioritizes actionable intelligence (security advisories,
/// breaking changes, migration guides) above informational content (tutorials,
/// blog posts, discussions). Within each section, items retain their existing
/// priority-then-score ordering from the upstream sort.
///
/// Sections:
/// - **action**: items whose `content_type` is actionable (security, breaking, etc.)
/// - **watch**: non-actionable items with score >= 0.6 AND corroboration from
///   multiple sources (high confidence, worth monitoring)
/// - **reading**: everything else (informational, uncorroborated, lower confidence)
///
/// Each section is capped at 5 items. The final vec is action + watch + reading.
fn apply_section_split(items: Vec<BriefingItem>) -> Vec<BriefingItem> {
    let mut action_items: Vec<BriefingItem> = items
        .iter()
        .filter(|i| is_actionable_content_type(i.content_type.as_deref()))
        .cloned()
        .map(|mut i| {
            i.section = Some("action".into());
            i.triage_reason = Some(triage_reason_for_action(i.content_type.as_deref()));
            i
        })
        .collect();

    let mut watch_items: Vec<BriefingItem> = items
        .iter()
        .filter(|i| {
            !is_actionable_content_type(i.content_type.as_deref())
                && (i.score >= 0.7 || (i.score >= 0.5 && i.corroboration_count > 0))
        })
        .cloned()
        .map(|mut i| {
            i.section = Some("watch".into());
            i.triage_reason = if i.corroboration_count > 0 {
                Some(format!("Seen in {} sources", i.corroboration_count))
            } else if !i.matched_deps.is_empty() {
                let deps: Vec<&str> = i.matched_deps.iter().take(3).map(|s| s.as_str()).collect();
                Some(format!("Relevant to {}", deps.join(", ")))
            } else {
                Some("High relevance match".into())
            };
            i
        })
        .collect();

    let mut reading_items: Vec<BriefingItem> = items
        .iter()
        .filter(|i| {
            !is_actionable_content_type(i.content_type.as_deref())
                && i.score < 0.7
                && (i.score < 0.5 || i.corroboration_count == 0)
        })
        .cloned()
        .map(|mut i| {
            i.section = Some("reading".into());
            i.triage_reason = Some("Background context".into());
            i
        })
        .collect();

    action_items.truncate(5);
    watch_items.truncate(5);
    reading_items.truncate(5);

    let mut final_items = action_items;
    final_items.extend(watch_items);
    final_items.extend(reading_items);
    final_items
}

fn triage_reason_for_action(content_type: Option<&str>) -> String {
    match content_type {
        Some("security_advisory" | "vulnerability_report" | "cve") => {
            "Security advisory — review impact".into()
        }
        Some("breaking_change") => "Breaking change — may affect your code".into(),
        Some("deprecation_notice") => "Deprecation notice — plan migration".into(),
        Some("release_notes") => "New release for your stack".into(),
        Some("platform_update") => "Platform update — check compatibility".into(),
        Some("migration_guide") => "Migration guide available".into(),
        _ => "Requires action".into(),
    }
}

// ============================================================================
// Morning Briefing Check
// ============================================================================

/// Parse "HH:MM" time string, returning (hour, minute). Defaults to (8, 0) on parse failure.
fn priority_rank(p: Option<&str>) -> u8 {
    match p {
        Some("critical") => 0,
        Some("alert") => 1,
        Some("advisory") => 2,
        _ => 3, // "watch" or None
    }
}

/// Canonical briefing ordering. Actionable intel (security advisories, breaking
/// changes) leads regardless of relevance score; then signal priority, then
/// score, then corroboration, then a deterministic `item_id` tiebreak.
///
/// The id tiebreak matters: the scoring soft-ceiling can pin two distinct items
/// to a byte-identical score, and a stable sort would otherwise leave their
/// order to chance (this is exactly how a homelab listicle ended up adjacent to
/// an exploitable CVE in a real brief).
fn briefing_item_cmp(a: &BriefingItem, b: &BriefingItem) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let a_action = is_actionable_content_type(a.content_type.as_deref());
    let b_action = is_actionable_content_type(b.content_type.as_deref());
    b_action
        .cmp(&a_action) // actionable (true) sorts first
        .then_with(|| {
            priority_rank(a.signal_priority.as_deref())
                .cmp(&priority_rank(b.signal_priority.as_deref()))
        })
        .then_with(|| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal))
        .then_with(|| b.corroboration_count.cmp(&a.corroboration_count))
        .then_with(|| a.item_id.cmp(&b.item_id))
}

/// Honest provenance footer appended to the synthesis. Reports item and
/// distinct-source counts, and ONLY claims corroboration when items were
/// actually confirmed by more than one source. The previous "(N signals across
/// <platforms>)" wording conflated total item count with corroboration and
/// implied cross-source confirmation that the data did not support.
fn synthesis_provenance(items: &[BriefingItem]) -> String {
    let item_count = items.len();
    let source_count = {
        let mut s: Vec<&str> = items.iter().map(|i| i.source_type.as_str()).collect();
        s.sort_unstable();
        s.dedup();
        s.len()
    };
    let corroborated = items.iter().filter(|i| i.corroboration_count > 0).count();
    let base = format!(
        "{item_count} item{} from {source_count} source{}",
        if item_count == 1 { "" } else { "s" },
        if source_count == 1 { "" } else { "s" },
    );
    if corroborated > 0 {
        format!("{base}; {corroborated} cross-source corroborated")
    } else {
        base
    }
}

fn parse_briefing_time(time_str: &str) -> (u32, u32) {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() == 2 {
        let hour = parts[0].parse::<u32>().unwrap_or(8);
        let minute = parts[1].parse::<u32>().unwrap_or(0);
        (hour.min(23), minute.min(59))
    } else {
        (8, 0)
    }
}

/// Returns true if the morning briefing is due to fire this tick.
/// Checks enabled, time window, and already-fired guards — but does NOT build the
/// briefing or read analysis results. Used by the scheduler to trigger a fresh
/// fetch+analyze cycle before the briefing reads results.
pub fn is_morning_briefing_due(state: &MonitoringState) -> bool {
    let (enabled, briefing_time_str, persisted_date) = {
        let settings = crate::get_settings_manager().lock();
        let monitoring = &settings.get().monitoring;
        let enabled = monitoring.morning_briefing.unwrap_or(true);
        let time = monitoring
            .briefing_time
            .clone()
            .unwrap_or_else(|| "08:00".to_string());
        let last = monitoring.last_briefing_date.clone();
        (enabled, time, last)
    };

    if !enabled {
        return false;
    }

    let now = chrono::Local::now();
    let today = now.format("%Y-%m-%d").to_string();

    if let Some(ref last) = persisted_date {
        if last == &today {
            return false;
        }
    }
    {
        let last_date = state.last_morning_briefing_date.lock();
        if let Some(ref last) = *last_date {
            if last == &today {
                return false;
            }
        }
    }

    let (target_hour, target_min) = parse_briefing_time(&briefing_time_str);
    let now_mins = now.hour() * 60 + now.minute();
    let target_mins = target_hour * 60 + target_min;
    let mins_since_target = ((now_mins as i32 - target_mins as i32) + 1440) % 1440;

    if mins_since_target > 1410 {
        return false;
    }

    true
}

/// Check if morning briefing should fire and generate notification content.
/// Returns None if disabled, outside the briefing window, or already fired today.
/// The last briefing date is persisted to settings.json so a restart doesn't
/// re-trigger the briefing on the same day.
pub fn check_morning_briefing(state: &MonitoringState) -> Option<BriefingNotification> {
    // 1. Check settings for morning_briefing enabled, briefing_time, and persisted last date
    let (enabled, briefing_time_str, persisted_date) = {
        let settings = crate::get_settings_manager().lock();
        let monitoring = &settings.get().monitoring;
        let enabled = monitoring.morning_briefing.unwrap_or(true);
        let time = monitoring
            .briefing_time
            .clone()
            .unwrap_or_else(|| "08:00".to_string());
        let last = monitoring.last_briefing_date.clone();
        (enabled, time, last)
    };

    if !enabled {
        return None;
    }

    // 2. Check if already fired today — check BEFORE time window to short-circuit early
    let now = chrono::Local::now();
    let today = now.format("%Y-%m-%d").to_string();

    // Check persisted date first (survives restarts)
    if let Some(ref last) = persisted_date {
        if last == &today {
            return None;
        }
    }
    // Also check in-memory state (covers rapid re-checks within same session)
    {
        let last_date = state.last_morning_briefing_date.lock();
        if let Some(ref last) = *last_date {
            if last == &today {
                return None;
            }
        }
    }

    // 3. Time window check with catch-up logic.
    //
    // The briefing fires if EITHER:
    // (a) We're within 30 minutes AFTER the configured time (normal case), OR
    // (b) We're past the configured time and the briefing hasn't fired today (catch-up).
    //
    // Case (b) handles: user opens app at 10:00 but briefing time is 08:00.
    // Without catch-up, they'd miss the briefing entirely.
    let (target_hour, target_min) = parse_briefing_time(&briefing_time_str);
    let now_mins = now.hour() * 60 + now.minute();
    let target_mins = target_hour * 60 + target_min;

    // Minutes elapsed since the target time (with midnight rollover)
    let mins_since_target = ((now_mins as i32 - target_mins as i32) + 1440) % 1440;

    // Not yet reached the target time today (more than 30 min before)
    if mins_since_target > 1410 {
        // We're BEFORE the target time (1410 = 1440-30, meaning we're 30+ min early)
        return None;
    }

    // We're past the target time — the briefing hasn't fired today (checked above).
    // Fire it. The already-fired-today check prevents double delivery.

    // 4. Get top relevant items from in-memory analysis state or DB
    //    Language firewall: only show items matching user's language.
    //    Briefing-quality firewall: reject garbled/marketing/clickbait
    //      titles that made it past the source-fetch gate but would
    //      visibly degrade a featured surface. See briefing_quality.rs.
    //    Intra-batch fuzzy dedupe: collapse near-duplicate titles (e.g.
    //      "React 19.2.3 released" on HN + Reddit) to the highest-scoring
    //      variant so the brief doesn't echo itself.
    let user_lang = crate::i18n::get_user_language();
    let raw_items: Vec<BriefingItem> = {
        let analysis_state = crate::get_analysis_state().lock();
        if let Some(ref results) = analysis_state.results {
            results
                .iter()
                .filter(|r| r.relevant && !r.excluded)
                .filter(|r| r.detected_lang == user_lang) // Language gate
                .filter(|r| r.top_score >= BRIEFING_SCORE_FLOOR)
                // Pull a bigger pool so the briefing-quality + dedupe gates
                // have headroom to filter without starving the brief below
                // its target size. 25 → ~8 after rejections is realistic.
                .take(25)
                .map(|r| BriefingItem {
                    title: r.title.clone(),
                    source_type: r.source_type.clone(),
                    score: r.top_score,
                    signal_type: r.signal_type.clone(),
                    url: r.url.clone(),
                    item_id: Some(r.id as i64),
                    signal_priority: r.signal_priority.clone(),
                    description: r.signal_action.clone(),
                    matched_deps: matched_deps_from_signal_triggers(r.signal_triggers.as_deref()),
                    content_type: r
                        .score_breakdown
                        .as_ref()
                        .and_then(|b| b.content_type.clone()),
                    corroboration_count: 0,
                    alt_sources: vec![],
                    section: None,
                    triage_reason: None,
                })
                .collect()
        } else {
            // Fall back to DB query — now reads real scores and filters by language.
            // The scheduler runs a fresh fetch+analyze before the briefing fires,
            // so this path only triggers if analysis_state.results is still None
            // (e.g. analysis failed). Use a 6-hour window — tight enough to avoid
            // surfacing yesterday's stale content, wide enough to catch items from
            // the most recent overnight fetch cycle.
            if let Ok(db) = crate::get_database() {
                let period_start = chrono::Utc::now() - chrono::Duration::hours(6);
                db.get_relevant_items_since(
                    period_start,
                    BRIEFING_SCORE_FLOOR.into(),
                    25,
                    &user_lang,
                )
                .ok()
                .map(|db_items| {
                    let conn = db.conn.lock();
                    db_items
                        .into_iter()
                        .map(|i| {
                            let matched_deps = load_briefing_matched_deps(&conn, i.id);
                            BriefingItem {
                                title: i.title,
                                source_type: i.source_type,
                                score: i.relevance_score.unwrap_or(0.0) as f32,
                                signal_type: None,
                                url: i.url,
                                item_id: Some(i.id),
                                signal_priority: None,
                                description: None,
                                matched_deps,
                                content_type: i.content_type,
                                corroboration_count: 0,
                                alt_sources: vec![],
                                section: None,
                                triage_reason: None,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default()
            } else {
                vec![]
            }
        }
    };

    // Compute freshness before the quality gate so we can return a stale-data
    // notification even when there's no meaningful content. Without this, stale
    // sources produce silence — the user sees "nothing" and assumes all is well.
    let freshness = compute_data_freshness();

    // Apply quality gate + dedupe + enrichment via shared pipeline
    let briefing = build_enriched_briefing(raw_items, &user_lang, false);

    if !briefing.has_meaningful_content() {
        // If data is stale, surface that explicitly instead of returning None.
        // The frontend can show "No fresh data — sources may need attention."
        if freshness.as_ref().map_or(false, |f| f.is_stale) {
            let now = chrono::Local::now();
            let stale_briefing = BriefingNotification {
                title: format!("4DA Intelligence Briefing — {}", now.format("%d %b %Y")),
                items: vec![],
                total_relevant: 0,
                ongoing_topics: vec![],
                knowledge_gaps: vec![],
                escalating_chains: vec![],
                synthesis: None,
                preemption_alerts: vec![],
                blind_spot_score: None,
                labels: Some(build_briefing_labels(&user_lang)),
                personalization_context: None,
                data_freshness: freshness,
                corroboration_available: false,
                coverage_building: false,
                synthesis_hint: None,
            };
            // Still mark as fired so we don't re-trigger the stale warning all day
            {
                let mut last_date = state.last_morning_briefing_date.lock();
                *last_date = Some(today.clone());
            }
            {
                let mut settings = crate::get_settings_manager().lock();
                settings.get_mut().monitoring.last_briefing_date = Some(today);
                if let Err(e) = settings.save() {
                    warn!(target: "4da::notify", error = %e, "Failed to persist last_briefing_date to settings");
                }
            }
            return Some(stale_briefing);
        }
        return None;
    }

    // 5. Mark as fired today — persist to settings AND in-memory state
    {
        let mut last_date = state.last_morning_briefing_date.lock();
        *last_date = Some(today.clone());
    }
    // Persist to settings.json so restart doesn't re-trigger
    {
        let mut settings = crate::get_settings_manager().lock();
        settings.get_mut().monitoring.last_briefing_date = Some(today);
        if let Err(e) = settings.save() {
            warn!(target: "4da::notify", error = %e, "Failed to persist last_briefing_date to settings");
        }
    }

    Some(briefing)
}

/// Build translated labels for the briefing window.
/// Uses the i18n system to translate UI strings, falling back to English defaults.
pub(crate) fn build_briefing_labels(lang: &str) -> BriefingLabels {
    // Helper: use i18n key if translated, otherwise use the English default.
    // (t() returns the key itself when no translation exists — detect that.)
    let tr = |key: &str, default: &str| -> String {
        let result = crate::i18n::t(key, lang, &[]);
        if result == key {
            default.to_string()
        } else {
            result
        }
    };

    BriefingLabels {
        header: tr("ui:briefing.header", "INTELLIGENCE BRIEFING"),
        escalating: tr("ui:briefing.escalating", "ESCALATING"),
        signals: tr("ui:briefing.signals", "SOURCES"),
        blind_spots: tr("ui:briefing.blind_spots", "QUIET IN YOUR SOURCES"),
        tracking: tr("ui:briefing.tracking", "Still tracking:"),
        gap_days_suffix: tr("ui:briefing.gap_days_suffix", "d since last signal"),
        signals_today_suffix: tr("ui:briefing.signals_today_suffix", " today"),
        empty_state: tr(
            "ui:briefing.empty_state",
            "Your stack is quiet. Nothing new.",
        ),
        preemption: tr("ui:briefing.preemption", "PREEMPTION"),
        blind_spots_score: tr("ui:briefing.blind_spots_score", "BLIND SPOT SCORE"),
    }
}

/// Send a morning briefing via the desktop-level briefing window and OS notification.
///
/// The briefing window is pinned to the desktop level — behind all normal windows,
/// never stealing focus, never interrupting fullscreen applications.
/// A companion OS notification alerts the user that the briefing is ready.
pub fn send_morning_briefing_notification<R: Runtime>(
    app: &AppHandle<R>,
    briefing: &BriefingNotification,
) {
    // Primary: desktop-level briefing window (ambient, non-intrusive)
    crate::briefing_window::show_briefing(app, briefing);

    // Companion OS notification — only if the briefing window isn't showing.
    // When the widget is visible, the OS notification is redundant.
    if crate::briefing_window::is_briefing_visible() {
        info!(
            target: "4da::briefing",
            items = briefing.total_relevant,
            "Briefing delivered via desktop widget (OS notification suppressed)"
        );
        return;
    }

    // Fallback: native OS notification (briefing window failed to show)
    let body = build_notification_summary(briefing);

    #[cfg(target_os = "windows")]
    {
        // Use tauri-winrt-notification directly with our registered AUMID
        // to bypass tauri-plugin-notification's dev-mode guard
        let result = tauri_winrt_notification::Toast::new("com.4da.app")
            .title(&briefing.title)
            .text1(&body)
            .duration(tauri_winrt_notification::Duration::Long)
            .show();
        if let Err(e) = result {
            warn!(target: "4da::briefing", error = %e, "OS notification failed (briefing window is primary)");
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Err(e) = app
            .notification()
            .builder()
            .title(&briefing.title)
            .body(&body)
            .show()
        {
            warn!(target: "4da::briefing", error = %e, "OS notification failed (briefing window is primary)");
        }
    }

    info!(
        target: "4da::briefing",
        items = briefing.total_relevant,
        gaps = briefing.knowledge_gaps.len(),
        chains = briefing.escalating_chains.len(),
        "Intelligence briefing delivered (OS notification fallback)"
    );
}

/// Build a concise, professional summary for the OS notification body.
///
/// Format: "1 escalating chain · 2 critical signals · 5 relevant items"
/// Prioritizes actionable info: chains first, then signals, then gaps.
fn build_notification_summary(briefing: &BriefingNotification) -> String {
    let mut parts: Vec<String> = Vec::new();

    if !briefing.escalating_chains.is_empty() {
        let n = briefing.escalating_chains.len();
        parts.push(format!(
            "{n} escalating chain{}",
            if n != 1 { "s" } else { "" }
        ));
    }

    let critical = briefing
        .items
        .iter()
        .filter(|i| i.signal_priority.as_deref() == Some("critical"))
        .count();
    let alert = briefing
        .items
        .iter()
        .filter(|i| i.signal_priority.as_deref() == Some("alert"))
        .count();
    if critical > 0 {
        parts.push(format!(
            "{critical} critical signal{}",
            if critical != 1 { "s" } else { "" }
        ));
    }
    if alert > 0 {
        parts.push(format!(
            "{alert} alert{}",
            if alert != 1 { "s" } else { "" }
        ));
    }
    if briefing.total_relevant > 0 {
        let n = briefing.total_relevant;
        parts.push(format!(
            "{n} relevant item{}",
            if n != 1 { "s" } else { "" }
        ));
    }
    if !briefing.knowledge_gaps.is_empty() {
        let n = briefing.knowledge_gaps.len();
        parts.push(format!("{n} blind spot{}", if n != 1 { "s" } else { "" }));
    }
    if parts.is_empty() {
        return "No new signals since last check.".to_string();
    }

    let summary = parts.join(" · ");
    if let Some(top) = briefing.items.first() {
        let safe_title = strip_control_chars(&top.title);
        let title = truncate_safe(&safe_title, 80);
        format!("{title}\n{summary}")
    } else {
        summary
    }
}

// ============================================================================
// Novelty Detection
// ============================================================================

/// Extract the most likely topic keyword from a title.
/// Looks for proper nouns (capitalized words 3+ chars) that aren't common English.
fn extract_topic_from_title(title: &str) -> String {
    const STOP_WORDS: &[&str] = &[
        "the", "how", "why", "new", "top", "best", "this", "that", "what", "with", "from", "your",
        "will", "for", "are", "was", "has", "have", "not", "all", "can", "get", "use", "now",
        "just", "more", "into", "out", "about", "than", "been", "its", "our", "but", "who",
        "first", "last", "week", "today", "part", "using", "when", "does", "where", "after",
        "before", "between", "through", "during", "without", "against", "also", "like", "over",
        "under", "only", "very", "most", "some", "every", "each", "both", "many", "much", "still",
        "already", "really", "actually", "here", "there", "then", "replaced", "building", "making",
        "getting", "going", "doing", "running", "creating", "adding", "moving", "changing",
        "breaking", "fixing", "missing", "should", "could", "would", "might", "must",
    ];
    let candidates: Vec<&str> = title
        .split_whitespace()
        .filter(|w| {
            let trimmed = w.trim_end_matches(|c: char| !c.is_alphanumeric());
            trimmed.len() >= 2
                && trimmed.chars().next().is_some_and(|c| c.is_uppercase())
                && !STOP_WORDS.contains(&trimmed.to_lowercase().as_str())
        })
        .take(3)
        .collect();

    let result = candidates.into_iter().take(2).collect::<Vec<_>>().join(" ");
    result
        .trim_end_matches(|c: char| !c.is_alphanumeric())
        .to_string()
}

/// Filter briefing items for novelty — items whose titles appeared in the last 3 days
/// are moved to "ongoing topics" instead of being shown again.
/// Returns (novel_items, ongoing_topic_names).
fn apply_novelty_filter(items: Vec<BriefingItem>, today: &str) -> (Vec<BriefingItem>, Vec<String>) {
    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(_) => return (items, vec![]), // Can't check history, show everything
    };

    // Get titles shown in the last 14 days
    let recent_titles: std::collections::HashSet<String> = {
        let mut stmt = match conn.prepare(
            "SELECT LOWER(item_title) FROM briefing_item_history
             WHERE briefing_date >= date(?1, '-14 days')",
        ) {
            Ok(s) => s,
            Err(_) => return (items, vec![]), // Table might not exist yet
        };

        stmt.query_map(rusqlite::params![today], |row| row.get::<_, String>(0))
            .ok()
            .map(|rows| rows.filter_map(std::result::Result::ok).collect())
            .unwrap_or_default()
    };

    if recent_titles.is_empty() {
        // No history yet — everything is novel. Record items for next time.
        record_briefing_items(&conn, &items, today);
        return (items, vec![]);
    }

    let mut novel = Vec::new();
    let mut ongoing = Vec::new();

    for item in items {
        if recent_titles.contains(&item.title.to_lowercase()) {
            // Extract the actual topic, not the source platform
            let topic = if !item.matched_deps.is_empty() {
                item.matched_deps[0].clone()
            } else {
                extract_topic_from_title(&item.title)
            };
            if !topic.is_empty() {
                ongoing.push(topic);
            }
        } else {
            novel.push(item);
        }
    }

    // Deduplicate ongoing topic names
    ongoing.sort();
    ongoing.dedup();

    // Record novel items for future novelty checks
    record_briefing_items(&conn, &novel, today);

    (novel, ongoing)
}

/// Record briefing items in history for future novelty detection.
fn record_briefing_items(conn: &rusqlite::Connection, items: &[BriefingItem], date: &str) {
    for item in items {
        if let Err(e) = conn.execute(
            "INSERT INTO briefing_item_history (item_title, source_type, briefing_date) VALUES (?1, ?2, ?3)",
            rusqlite::params![item.title, item.source_type, date],
        ) {
            tracing::warn!(target: "4da::monitor", error = %e, title = %item.title, "Failed to record briefing item history");
        }
    }
}

// ============================================================================
// Preemption Alert Novelty Filter
// ============================================================================

/// Filter preemption alerts through the same novelty window as regular items.
///
/// Preemption alerts (vulnerability advisories) are persistent — the same GHSA
/// alerts appear every day until the dependency is updated. Without novelty
/// filtering, the briefing shows identical preemption cards every morning.
///
/// Returns (novel_alerts, stale_topic_names). Novel alerts are shown in the
/// PREEMPTION section; stale topics are surfaced in the "Still tracking:" footer.
/// True for advisories that must re-surface every morning until the user fixes
/// them, regardless of the 14-day novelty window. The preemption feed is
/// recomputed from the user's *installed* dependency versions each cycle, so an
/// alert's presence means the vulnerability is STILL live — an unfixed
/// Critical/High advisory silenced by novelty is a silent exposure, the exact
/// opposite of what the brief is for. Lower-severity advisories still obey
/// novelty so the brief doesn't nag about low-stakes items.
fn is_persistent_security_alert(alert: &BriefingPreemptionAlert) -> bool {
    matches!(alert.urgency.as_str(), "critical" | "high")
}

fn apply_preemption_novelty_filter(
    alerts: Vec<BriefingPreemptionAlert>,
    today: &str,
) -> (Vec<BriefingPreemptionAlert>, Vec<String>) {
    if alerts.is_empty() {
        return (vec![], vec![]);
    }

    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(_) => return (alerts, vec![]),
    };

    let recent_titles: std::collections::HashSet<String> = {
        let mut stmt = match conn.prepare(
            "SELECT LOWER(item_title) FROM briefing_item_history
             WHERE briefing_date >= date(?1, '-14 days') AND source_type = 'preemption'",
        ) {
            Ok(s) => s,
            Err(_) => return (alerts, vec![]),
        };

        stmt.query_map(rusqlite::params![today], |row| row.get::<_, String>(0))
            .ok()
            .map(|rows| rows.filter_map(std::result::Result::ok).collect())
            .unwrap_or_default()
    };

    if recent_titles.is_empty() {
        record_preemption_history(&conn, &alerts, today);
        return (alerts, vec![]);
    }

    let mut novel = Vec::new();
    let mut stale_topics = Vec::new();

    for alert in alerts {
        // Unfixed Critical/High advisories persist every morning — novelty must
        // never silence a live high-severity vulnerability on an installed dep.
        if !is_persistent_security_alert(&alert)
            && recent_titles.contains(&alert.title.to_lowercase())
        {
            let topic = alert
                .package_name
                .clone()
                .unwrap_or_else(|| extract_topic_from_title(&alert.title));
            if !topic.is_empty() {
                stale_topics.push(topic);
            }
        } else {
            novel.push(alert);
        }
    }

    stale_topics.sort();
    stale_topics.dedup();

    record_preemption_history(&conn, &novel, today);

    (novel, stale_topics)
}

fn record_preemption_history(
    conn: &rusqlite::Connection,
    alerts: &[BriefingPreemptionAlert],
    date: &str,
) {
    for alert in alerts {
        if let Err(e) = conn.execute(
            "INSERT INTO briefing_item_history (item_title, source_type, briefing_date) VALUES (?1, ?2, ?3)",
            rusqlite::params![alert.title, "preemption", date],
        ) {
            tracing::warn!(target: "4da::monitor", error = %e, title = %alert.title, "Failed to record preemption history");
        }
    }
}

// ============================================================================
// Escalating Chain Detection
// ============================================================================

/// Detect signal chains in escalating or peak phase for the briefing.
/// These are topics appearing across multiple days with increasing frequency.
fn detect_escalating_chains() -> Vec<ChainSummary> {
    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    match crate::signal_chains::detect_chains(&conn) {
        Ok(chains) => {
            chains
                .into_iter()
                .filter_map(|chain| {
                    let prediction = crate::signal_chains::predict_chain_lifecycle(&chain);
                    let phase_str = match prediction.phase {
                        crate::signal_chains::ChainPhase::Escalating => "escalating",
                        crate::signal_chains::ChainPhase::Peak => "peak",
                        _ => return None, // Only include escalating/peak chains
                    };

                    Some(ChainSummary {
                        name: chain.chain_name,
                        phase: phase_str.to_string(),
                        link_count: chain.links.len(),
                        action: chain.suggested_action,
                        confidence: chain.confidence,
                    })
                })
                .take(3) // Max 3 chains in briefing
                .collect()
        }
        Err(e) => {
            tracing::warn!(target: "4da::briefing", error = %e, "Chain detection failed for briefing");
            vec![]
        }
    }
}

// ============================================================================
// First-Briefing Personalization
// ============================================================================

/// Build a personalization context line for the first briefing.
///
/// Returns `Some("Based on your Rust + TypeScript stack and interest in ...")` if
/// this is the user's first briefing and we have onboarding data to reference.
/// Returns `None` for subsequent briefings or when no profile data exists.
fn build_first_briefing_context() -> Option<String> {
    // Check if this is the first briefing by looking at briefing_item_history.
    // If the table has any rows, the user has already seen a briefing.
    let conn = crate::open_db_connection().ok()?;
    let has_prior: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM briefing_item_history",
            [],
            |row| row.get(0),
        )
        .unwrap_or(true); // If query fails (table missing), assume not first briefing

    if has_prior {
        return None;
    }

    // Gather stack from ACE context
    let ace_ctx = crate::scoring::get_ace_context();
    let tech: Vec<&str> = ace_ctx
        .detected_tech
        .iter()
        .filter(|t| crate::domain_profile::is_display_worthy(&t.to_lowercase()))
        .take(4)
        .map(|s| s.as_str())
        .collect();

    // Gather interests from context engine
    let interest_names: Vec<String> = match crate::get_context_engine() {
        Ok(engine) => engine
            .get_static_identity()
            .ok()
            .map(|id| {
                id.interests
                    .iter()
                    .take(3)
                    .map(|i| i.topic.clone())
                    .collect()
            })
            .unwrap_or_default(),
        Err(_) => vec![],
    };

    if tech.is_empty() && interest_names.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    if !tech.is_empty() {
        parts.push(format!("your {} stack", tech.join(" + ")));
    }
    if !interest_names.is_empty() {
        parts.push(format!("interest in {}", interest_names.join(", ")));
    }

    Some(format!(
        "Based on {}, here's what matters today.",
        parts.join(" and ")
    ))
}

fn is_coverage_building() -> bool {
    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(_) => return true,
    };
    let distinct_days: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT briefing_date) FROM briefing_item_history",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    distinct_days < 7
}

// ============================================================================
// CLI Briefing Generator
// ============================================================================

/// Strip control characters (ANSI escape sequences, etc.) from a string
/// to prevent injection in CLI output.
fn strip_control_chars(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

// ============================================================================
// LLM Morning Brief Synthesis
// ============================================================================

/// Result of LLM synthesis — prose text with optional structured clusters.
pub(crate) struct SynthesisResult {
    pub prose: String,
    pub clusters: Option<Vec<crate::synthesis_schema::SynthesisCluster>>,
    pub provider_used: String,
    pub synthesis_tier: String,
}

/// Short, human-readable label for a project path: the last one or two path
/// components (e.g. "c:/users/.../kairos-mvp/backend" -> "kairos-mvp/backend").
/// Gives the synthesizer a concrete project to name instead of inventing one.
fn project_label(path: &str) -> String {
    let norm = path.replace('\\', "/");
    let parts: Vec<&str> = norm
        .trim_end_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    match parts.len() {
        0 => path.to_string(),
        1 => parts[0].to_string(),
        n => format!("{}/{}", parts[n - 2], parts[n - 1]),
    }
}

/// The exact prose the synthesis gates emit when they reject output (LLM
/// abstention, failed groundedness, or a wrong package version). Used to detect
/// a quality-rejection so the retry wrapper can re-attempt.
const ABSTENTION_PROSE: &str = "Low signal -- no noteworthy intelligence overnight.";

fn is_abstention_prose(prose: &str) -> bool {
    prose.trim_start().starts_with(ABSTENTION_PROSE)
}

/// Synthesize a narrative morning intelligence briefing, retrying on a quality
/// rejection. LLM output is non-deterministic (default temperature), so a brief
/// that gets rejected for a fabricated/transposed version (e.g. "jsonwebtoken to
/// 5.61.6", caught by `check_factual_claims`) usually synthesizes cleanly on a
/// re-attempt. We only retry when the brief HAS content to synthesize — a
/// genuinely quiet day abstains on the first pass without burning extra calls.
pub(crate) async fn synthesize_morning_briefing(
    briefing: &BriefingNotification,
) -> std::result::Result<SynthesisResult, String> {
    const MAX_ATTEMPTS: u32 = 3;
    // A brief with <2 pieces of content has nothing to synthesize; a first-pass
    // abstention there is legitimate, so don't retry (and don't burn LLM calls).
    let has_content = briefing.items.len() + briefing.preemption_alerts.len() >= 2;

    let mut last: Option<SynthesisResult> = None;
    for attempt in 1..=MAX_ATTEMPTS {
        let result = synthesize_morning_briefing_once(briefing).await?;
        if !is_abstention_prose(&result.prose) || !has_content {
            if attempt > 1 {
                tracing::info!(
                    target: "4da::briefing",
                    attempt,
                    "Synthesis accepted on retry after an earlier quality rejection"
                );
            }
            return Ok(result);
        }
        tracing::warn!(
            target: "4da::briefing",
            attempt,
            max = MAX_ATTEMPTS,
            "Synthesis abstained on a content-bearing brief — retrying for an accurate generation"
        );
        last = Some(result);
    }
    tracing::warn!(
        target: "4da::briefing",
        "Synthesis abstained on all attempts — serving the honest quiet state"
    );
    Ok(last.expect("loop runs at least once"))
}

/// One synthesis attempt (LLM call + groundedness + factual gates). Wrapped by
/// `synthesize_morning_briefing`, which retries this on a quality rejection.
async fn synthesize_morning_briefing_once(
    briefing: &BriefingNotification,
) -> std::result::Result<SynthesisResult, String> {
    let configured_settings = {
        let mut guard = crate::get_settings_manager().lock();
        guard.ensure_keys_hydrated();
        guard.get().llm.clone()
    };

    let providers = crate::ollama::resolve_synthesis_providers(&configured_settings).await;
    if providers.is_empty() {
        return Err(
            "No synthesis-capable provider — configure a cloud AI provider (Anthropic or OpenAI) in Settings"
                .into(),
        );
    }

    let (tech_summary, topics_summary) = {
        let ace_ctx = crate::scoring::get_ace_context();
        let tech = if ace_ctx.detected_tech.is_empty() {
            "Not configured".to_string()
        } else {
            ace_ctx
                .detected_tech
                .iter()
                .take(15)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        };
        let topics = if ace_ctx.active_topics.is_empty() {
            "General development".to_string()
        } else {
            ace_ctx
                .active_topics
                .iter()
                .take(10)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        };
        (tech, topics)
    };

    // Load actual installed dependencies so the LLM knows what's in the user's stack
    let deps_summary = match crate::open_db_connection() {
        Ok(conn) => {
            let has_relevance = conn
                .prepare("SELECT project_relevance FROM project_dependencies LIMIT 1")
                .is_ok();
            let query = if has_relevance {
                "SELECT DISTINCT package_name, language FROM project_dependencies \
                 WHERE is_dev = 0 AND project_relevance >= 0.3 \
                 ORDER BY package_name LIMIT 50"
            } else {
                "SELECT DISTINCT package_name, language FROM project_dependencies \
                 WHERE is_dev = 0 \
                 ORDER BY package_name LIMIT 50"
            };
            let deps: Vec<String> = conn
                .prepare(query)
                .ok()
                .and_then(|mut stmt| {
                    stmt.query_map(rusqlite::params![], |row| {
                        let name: String = row.get(0)?;
                        let lang: String = row.get(1)?;
                        Ok(format!("{name} ({lang})"))
                    })
                    .ok()
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
                })
                .unwrap_or_default();
            if deps.is_empty() {
                "None detected".to_string()
            } else {
                deps.join(", ")
            }
        }
        Err(e) => {
            tracing::warn!(target: "4da::briefing", error = %e, "Failed to load deps for briefing");
            "None detected".to_string()
        }
    };

    let items_text = briefing
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let tag = item
                .signal_priority
                .as_deref()
                .map(|p| format!("[{}] ", p.to_uppercase()))
                .unwrap_or_default();
            let desc = item.description.as_deref().unwrap_or("");
            let deps = if item.matched_deps.is_empty() {
                String::new()
            } else {
                format!(", touches: {}", item.matched_deps.join(", "))
            };
            format!(
                "{}. {}{} -- {} (via {}{})",
                i + 1,
                tag,
                item.title,
                desc,
                item.source_type,
                deps
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let chains_text = if briefing.escalating_chains.is_empty() {
        String::new()
    } else {
        let c = briefing
            .escalating_chains
            .iter()
            .map(|c| {
                format!(
                    "- {} ({}, {} signals): {}",
                    c.name, c.phase, c.link_count, c.action
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("\nEscalating chains:\n{c}\n")
    };

    let gaps_text = if briefing.knowledge_gaps.is_empty() {
        String::new()
    } else {
        let g = briefing
            .knowledge_gaps
            .iter()
            .map(|g| format!("- {}: {}d silent", g.topic, g.days_since_last))
            .collect::<Vec<_>>()
            .join("\n");
        format!("\nBlind spots:\n{g}\n")
    };

    // Security/preemption alerts are the highest-priority content (system
    // prompt priority #1: CVEs in the developer's dependencies), but they live
    // in `preemption_alerts`, NOT in `items` — the cross-surface dedup pulls
    // CVE items out of `items` into alerts. Feed them to the synthesizer
    // explicitly, or on a CVE-heavy day it only sees the low-signal leftovers
    // and abstains. See briefing_groundedness for the matching corpus/packages.
    let preemption_text = if briefing.preemption_alerts.is_empty() {
        String::new()
    } else {
        // Order so primary-app issues lead, then side projects, then dev-only:
        // "what's important right now" weights the app you ship above side work.
        let mut ordered: Vec<&BriefingPreemptionAlert> =
            briefing.preemption_alerts.iter().collect();
        ordered.sort_by_key(|a| match a.scope.as_deref() {
            Some("primary") => 0u8,
            Some("dev") => 2,
            _ => 1, // external / unknown
        });
        let p = ordered
            .iter()
            .map(|a| {
                let pkg = a
                    .package_name
                    .as_deref()
                    .map(|p| format!(" [{p}]"))
                    .unwrap_or_default();
                let dep_kind = match a.is_direct {
                    Some(true) => "direct dep",
                    Some(false) => "transitive dep",
                    None => "dependency",
                };
                // Scope + concrete project so the model anchors to real paths
                // instead of inventing a use-case ("webhook flows").
                let scope_label = match a.scope.as_deref() {
                    Some("primary") => "in your PRIMARY app",
                    Some("dev") => "dev-only dependency",
                    Some("external") => "in a SIDE project (not the app you ship)",
                    _ => "scope unknown",
                };
                let projects = if a.affected_projects.is_empty() {
                    String::new()
                } else {
                    let names: Vec<String> = a
                        .affected_projects
                        .iter()
                        .map(|p| project_label(p))
                        .collect();
                    format!(", project: {}", names.join(", "))
                };
                let fix = match (a.installed_version.as_deref(), a.fixed_version.as_deref()) {
                    (Some(cur), Some(fixed)) => format!("; installed {cur}, fix in {fixed}"),
                    (None, Some(fixed)) => format!("; fix in {fixed}"),
                    (Some(cur), None) => format!("; installed {cur}"),
                    (None, None) => String::new(),
                };
                format!(
                    "- [{}] {}{} ({}, {}{}{})",
                    a.urgency.to_uppercase(),
                    a.title,
                    pkg,
                    dep_kind,
                    scope_label,
                    projects,
                    fix
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "\nSECURITY ALERTS in your dependencies (HIGHEST priority -- lead with these):\n{p}\n"
        )
    };

    let system_prompt = r#"You are an intelligence analyst writing a morning brief for a developer's desktop widget.

YOUR JOB: synthesize, don't summarize. Find the 1-2 strongest threads across all signals, explain WHY they matter together, and state what to do. The signal list below handles individual items -- your job is the "so what?" that a senior dev can't see by scanning titles.

PRIORITY ORDER for picking clusters:
1. Security vulnerabilities in the developer's actual dependencies (CVEs, RCEs)
2. Research or patterns that change how the developer should build (architecture shifts)
3. Major version releases of direct dependencies with breaking changes
4. Everything else -- skip unless it compounds with (1) or (2)

WHAT GOOD LOOKS LIKE:
"Tokio has a confirmed RCE via malformed HTTP/2 frames -- patch it today; the alerts list below has the exact fixed version. Upstream's HTTP/2 parser rewrite is still pending, so this is a stopgap.

Embedding fine-tuning research shows significant retrieval gains on domain-specific corpora -- worth prototyping against your scoring pipeline given the current relevance accuracy work."

WHAT BAD LOOKS LIKE:
"Tokio has a CVE. Upgrade tokio." -- Restating the signal title adds zero value. The signal list already says this. You must add context the title doesn't: severity, pattern, compound risk, or architectural implication.

"Async Traits, Hidden Allocs: Profiling Rust Futures highlights hidden allocation issues in async traits." -- NEVER paste a paper/article title as a sentence. Describe the actionable finding: "Rust async profiling reveals hidden allocations in trait objects -- audit hot paths if you use dyn Future."

"memory usage by 2-3x more than expected" -- NEVER invent specific numbers, percentages, or statistics. If the signal doesn't state a number, you can't either. "Hidden allocations" is fine. "2-3x more" when the signal never said that is fabrication.

"GitHub Actions hardening is on the rise. In another domain, researchers are exploring LLM curiosity. Meanwhile, fuzz drivers are gaining traction." -- Summarizing every signal sequentially. That is a for-loop, not intelligence.

BUDGET: maximum 80 words, 1-2 clusters, 2-4 sentences.

STRUCTURE per cluster: pattern/insight + evidence + action.
- Connect dots the signal list can't: "second CVE this quarter", "compounds with your current work on X", "three independent teams converging on Y"
- If two clusters, separate with a line break. Max two.
- Write PROSE PARAGRAPHS. No headers, no numbered lists, no sections, no labels. Just sentences.

QUALITY RULES:
1. Add value beyond the signal titles. If your sentence just restates a title, delete it.
2. State actions for senior devs -- don't explain why fire is hot.
3. Say what papers/research mean for the developer's stack; never paste titles as prose.
4. Every tech name must appear verbatim in the input. Invent nothing.
5. ABSTAIN if <2 items are noteworthy: "Low signal -- no noteworthy intelligence overnight."
6. Use plain ASCII dashes (--) not unicode em dashes.
7. NEVER speculate about implications not stated in the signals. "could impact your X" without evidence is hallucination. Stick to what the signals actually say.
8. If only 1 cluster is strong, write 1 cluster. Don't force a second cluster from weak signals.
9. NEVER invent numbers, percentages, or statistics not explicitly stated in the signals. "2-3x", "35%", "second this quarter" are claims -- they must come from the input, not your imagination.
10. ONLY mention technologies that appear in the developer's installed dependencies list or tech stack. If a signal mentions a package NOT in the developer's dependency list, do not claim it "impacts your stack" or "affects your applications". You may still mention it as industry news, but frame it accordingly: "X released Y" not "X affects your stack".
11. PROJECTS ARE GROUND TRUTH. Each security alert states its project and scope. Reference the actual project by name (e.g. "in kairos-mvp/backend"). NEVER invent a use-case, feature, or flow the input does not state -- do not say "auth flows", "webhook flows", "payment path" etc. unless those exact words appear in the input. The path is data; the use-case is your imagination.
12. NO CROSS-PROJECT COMPOUNDING. Two issues only "compound", "combine", or form a "combined exposure" when they are in the SAME project/path. If alert A is in project X and alert B is in project Y, they are SEPARATE issues -- say so, or cover only the strongest. Never manufacture a combined-attack-surface narrative across different projects.
13. RESPECT SCOPE. An alert marked "in a SIDE project" or "dev-only" is NOT in the app the developer ships. Do not imply it is active core work or that "the attack surface is live" in their main product. State the scope honestly: "in your side project navcal" not "in flows you're actively working on". Lead with PRIMARY-app issues; for side projects, name the project and label it as such.
14. DO NOT WRITE VERSION NUMBERS. Never put a version number (e.g. 1.16.0, 5.61.6, 9.3.1) in your prose. Name the package and the action -- "upgrade jsonwebtoken", "patch axios" -- and let the SECURITY ALERTS list below carry the exact installed/fixed versions. Stating versions yourself reliably transposes them between packages (e.g. attaching Clerk's 5.61.6 to axios); the list owns versions, you own the "so what".

BANNED:
- Inventing a use-case/feature/flow not stated in the input ("auth flows", "webhook flows", "billing path") -- name the project, not an imagined purpose
- Claiming two issues "compound" / "combine" / are a "combined exposure" when they are in different projects -- that is a fabricated narrative
- Implying a side-project or dev-only dependency is active core work or a live threat to the shipped app
- Writing ANY version number in the prose (1.16.0, 5.61.6, 9.3.1, etc.) -- name the package; the alerts list shows the exact version
- Restating signal titles without adding context or connecting dots
- Speculative implications: "could impact", "might affect", "may influence" without evidence in the signals
- Transition padding: "meanwhile", "in another domain", "in a related vein", "additionally", "furthermore"
- Filler: "is crucial", "is important", "this guidance", "it is essential", "it is recommended"
- Pasting paper/article titles as prose sentences -- describe the finding, not the title
- Citation brackets [1] [2], markdown, headers, bullets, numbered lists
- Covering more than 2 clusters -- pick the best, skip the rest
- Horizontal rules (---), separators, or dividers between clusters -- use a blank line only
- Unicode em dashes or special characters -- use plain ASCII only
- Fabricated numbers/percentages/statistics not present in the signals
- Echoing source-type tags like [rss], [hackernews], [arxiv] from the input -- describe findings naturally
- Echoing "(affects: ...)" or "(touches: ...)" metadata from the input -- integrate dependency info naturally
- Section headers or structured labels like "Action Required", "Worth Knowing", "Filtered Out" -- write flowing prose, not a report
- Explaining what you filtered out or why -- the user sees your synthesis, not your process
- Numbered item lists (1. 2. 3.) -- synthesize into prose clusters, don't enumerate
- Referring to signals by their position ("Index 2", "item 3", "the first signal") -- name the finding/project itself; the reader never sees the numbered input list

SOURCE-TYPE CALIBRATION:
Each signal is prefixed with its source type. Calibrate your language accordingly:
- [arxiv] or [papers_with_code]: "research shows", "findings indicate"
- [cve], [osv], [github_advisory]: "confirmed vulnerability", "advisory reports"
- [hackernews], [lobsters], [reddit]: "community discussion suggests", "developers report"
- [dev_to], [medium]: "the article argues", "one analysis suggests"
- [rss], [github]: "the project announced", "the changelog notes"
Never use "research confirms" for blog posts. Never use "developers report" for CVEs."#;

    // Append language instruction for non-English users
    let lang = crate::i18n::get_user_language();
    let language_instruction = if lang != "en" {
        let lang_name = crate::content_translation::lang_display_name(&lang);
        format!(
            "\n\nIMPORTANT: Write this entire briefing in {lang_name}. \
             Keep technical terms (React, Kubernetes, API, TypeScript, etc.) in English. \
             Use natural {lang_name} phrasing — do not translate word-for-word from English."
        )
    } else {
        String::new()
    };
    let full_system_prompt = format!("{system_prompt}{language_instruction}");

    let user_prompt = format!(
        "Developer context:\nTech stack: {tech}\nWorking on: {topics}\n\
         Installed dependencies: {deps}\n{preemption}\n\
         {count} signals:\n{items}\n{chains}{gaps}\n\
         Synthesize my morning intelligence briefing.",
        tech = tech_summary,
        topics = topics_summary,
        deps = deps_summary,
        preemption = preemption_text,
        count = briefing.items.len(),
        items = items_text,
        chains = chains_text,
        gaps = gaps_text,
    );

    let messages = vec![crate::llm::Message {
        role: "user".to_string(),
        content: user_prompt,
    }];

    let mut corpus: Vec<String> = briefing
        .items
        .iter()
        .map(|i| {
            let mut c = i.title.clone();
            if let Some(d) = &i.description {
                c.push(' ');
                c.push_str(d);
            }
            for dep in &i.matched_deps {
                c.push(' ');
                c.push_str(dep);
            }
            c
        })
        .collect();
    // The security alerts now lead the prompt, so the synthesis will reference
    // them — ground those sentences by adding the alert text to the corpus.
    for a in &briefing.preemption_alerts {
        let mut c = a.title.clone();
        if !a.explanation.is_empty() {
            c.push(' ');
            c.push_str(&a.explanation);
        }
        for v in [&a.package_name, &a.installed_version, &a.fixed_version]
            .into_iter()
            .flatten()
        {
            c.push(' ');
            c.push_str(v);
        }
        corpus.push(c);
    }

    // Known package/dependency names for the groundedness allowlist — bare
    // lowercase names ("axios", "jsonwebtoken") the capitalized salient-term
    // extractor cannot see on its own.
    let packages: Vec<String> = {
        let mut s: std::collections::HashSet<String> = std::collections::HashSet::new();
        for a in &briefing.preemption_alerts {
            // `package_name` may be a comma-joined advisory-merged label
            // ("@clerk/clerk-react, @clerk/shared") — split so each real package
            // name lands in the allowlist (package names never contain ", ").
            if let Some(p) = a.package_name.as_deref().filter(|p| !p.is_empty()) {
                for name in p.split(", ") {
                    s.insert(name.to_lowercase());
                }
            }
        }
        for i in &briefing.items {
            for dep in i.matched_deps.iter().filter(|d| !d.is_empty()) {
                s.insert(dep.to_lowercase());
            }
        }
        s.into_iter().collect()
    };

    // Factual ground truth for the deterministic version check: each alert's
    // package and the versions it is legitimate to cite (installed + fix).
    // Packages with no known version are omitted — we can't fault a claim we
    // have no data to contradict.
    let mk_fact = |name: &str, installed: Option<&str>, fixed: Option<&str>| {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        let mut versions = Vec::new();
        if let Some(v) = installed.filter(|v| !v.is_empty()) {
            versions.push(v.to_string());
        }
        if let Some(v) = fixed.filter(|v| !v.is_empty()) {
            versions.push(v.to_string());
        }
        if versions.is_empty() {
            return None;
        }
        Some(crate::briefing_groundedness::PackageFact {
            name: name.to_string(),
            versions,
        })
    };
    let package_facts: Vec<crate::briefing_groundedness::PackageFact> = briefing
        .preemption_alerts
        .iter()
        .flat_map(|a| {
            // Advisory-merged row: emit one fact per underlying package with its
            // OWN versions. The comma-joined `package_name` + single version
            // fields are display-only and lossy for these multi-package rows, so
            // checking them directly would skip every merged package's version.
            if !a.merged_package_versions.is_empty() {
                return a
                    .merged_package_versions
                    .iter()
                    .filter_map(|m| {
                        mk_fact(
                            m.package_name.as_str(),
                            m.installed_version.as_deref(),
                            m.fixed_version.as_deref(),
                        )
                    })
                    .collect::<Vec<_>>();
            }
            // Single-package alert: its own fields are faithful.
            mk_fact(
                a.package_name.as_deref().unwrap_or(""),
                a.installed_version.as_deref(),
                a.fixed_version.as_deref(),
            )
            .into_iter()
            .collect::<Vec<_>>()
        })
        .collect();

    const GROUNDEDNESS_THRESHOLD: f32 = 0.65;

    let mut last_error: Option<String> = None;

    for (idx, llm_settings) in providers.iter().enumerate() {
        let llm_client = crate::llm::LLMClient::new(llm_settings.clone());
        let tier = crate::ollama::synthesis_tier(llm_settings).await;
        let provider_label = format!(
            "{}/{}",
            llm_settings.provider,
            if llm_settings.model.is_empty() {
                "default"
            } else {
                &llm_settings.model
            }
        );

        let active_system_prompt = full_system_prompt.clone();

        // --- Structured output path (Phase 4) -----------------------------------
        // Try JSON mode first. If the provider supports it and the output validates,
        // we skip the entire 200+ line post-processing gauntlet. On parse/validation
        // failure, fall back transparently to the free-text path.
        let structured_mode = crate::llm::StructuredOutputMode::JsonSchema {
            schema: crate::synthesis_schema::SYNTHESIS_JSON_SCHEMA,
        };

        let start = std::time::Instant::now();
        let structured_result = llm_client
            .complete_structured(&active_system_prompt, messages.clone(), &structured_mode)
            .await;

        match structured_result {
            Ok(response) => {
                match serde_json::from_str::<crate::synthesis_schema::SynthesisOutput>(
                    &response.content,
                ) {
                    Ok(output) if output.validate().is_ok() => {
                        tracing::info!(
                            target: "4da::briefing",
                            tokens = response.input_tokens + response.output_tokens,
                            elapsed_ms = start.elapsed().as_millis(),
                            clusters = output.clusters.len(),
                            "Structured synthesis succeeded"
                        );

                        let id_warnings = output.validate_evidence_ids(briefing.items.len());
                        for w in &id_warnings {
                            tracing::warn!(target: "4da::briefing", "{w}");
                        }

                        let prose = output.to_prose();
                        let report =
                            crate::briefing_groundedness::validate_groundedness_with_packages(
                                &prose, &corpus, &packages,
                            );
                        if !report.is_acceptable(GROUNDEDNESS_THRESHOLD) {
                            tracing::warn!(
                                target: "4da::briefing",
                                confidence = report.confidence,
                                total_terms = report.total_terms,
                                ungrounded_count = report.ungrounded_terms.len(),
                                "Structured synthesis failed groundedness — abstaining"
                            );
                            return Ok(SynthesisResult {
                                prose: "Low signal -- no noteworthy intelligence overnight."
                                    .to_string(),
                                clusters: None,
                                provider_used: provider_label.clone(),
                                synthesis_tier: tier.as_str().to_string(),
                            });
                        }

                        // Deterministic factual backstop: reject fabricated
                        // version numbers the fuzzy grounding check can't catch.
                        let fact_violations = crate::briefing_groundedness::check_factual_claims(
                            &prose,
                            &package_facts,
                        );
                        if !fact_violations.is_empty() {
                            tracing::warn!(
                                target: "4da::briefing",
                                violations = ?fact_violations,
                                "Structured synthesis stated wrong package versions — abstaining"
                            );
                            return Ok(SynthesisResult {
                                prose: "Low signal -- no noteworthy intelligence overnight."
                                    .to_string(),
                                clusters: None,
                                provider_used: provider_label.clone(),
                                synthesis_tier: tier.as_str().to_string(),
                            });
                        }

                        tracing::info!(
                            target: "4da::briefing",
                            confidence = report.confidence,
                            claim_confidence = report.claim_confidence(),
                            claim_terms = report.claim_terms,
                            "Structured synthesis passed groundedness + factual checks"
                        );

                        let mut synthesis = prose;
                        synthesis
                            .push_str(&format!("\n\n({})", synthesis_provenance(&briefing.items)));
                        return Ok(SynthesisResult {
                            prose: synthesis,
                            clusters: Some(output.clusters),
                            provider_used: provider_label.clone(),
                            synthesis_tier: tier.as_str().to_string(),
                        });
                    }
                    Ok(_) => {
                        tracing::debug!(
                            target: "4da::briefing",
                            "Structured output failed validation — falling back to free-text"
                        );
                    }
                    Err(e) => {
                        tracing::debug!(
                            target: "4da::briefing",
                            error = %e,
                            "Structured output JSON parse failed — falling back to free-text"
                        );
                    }
                }
            }
            Err(e) => {
                let err_str = format!("{e}");
                if is_provider_error(&err_str) && idx + 1 < providers.len() {
                    tracing::info!(
                        target: "4da::briefing",
                        provider = %llm_settings.provider,
                        error = %e,
                        next = %providers[idx + 1].provider,
                        "Provider failed — trying next candidate"
                    );
                    last_error = Some(err_str);
                    continue;
                }
                tracing::debug!(
                    target: "4da::briefing",
                    error = %e,
                    "Structured completion failed — falling back to free-text"
                );
            }
        }

        // --- Free-text fallback path (legacy) ------------------------------------
        let start = std::time::Instant::now();
        let response = match llm_client
            .complete(&active_system_prompt, messages.clone())
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let err_str = format!("{e}");
                if is_provider_error(&err_str) && idx + 1 < providers.len() {
                    tracing::info!(
                        target: "4da::briefing",
                        provider = %llm_settings.provider,
                        error = %e,
                        next = %providers[idx + 1].provider,
                        "Provider failed on free-text — trying next candidate"
                    );
                    last_error = Some(err_str);
                    continue;
                }
                tracing::warn!(target: "4da::briefing", error = %e, "Morning brief synthesis failed");
                return Err(format!("LLM synthesis failed: {e}"));
            }
        };

        tracing::info!(
            target: "4da::briefing",
            tokens = response.input_tokens + response.output_tokens,
            elapsed_ms = start.elapsed().as_millis(),
            "Free-text synthesis complete (fallback)"
        );

        let mut synthesis = response.content.clone();

        // Strip citation brackets [1], [2][3], etc.
        while let Some(start_bracket) = synthesis.find('[') {
            if let Some(end_bracket) = synthesis[start_bracket..].find(']') {
                let inner = &synthesis[start_bracket + 1..start_bracket + end_bracket];
                if inner.len() <= 10
                    && inner
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == ',' || c == ' ')
                {
                    synthesis.replace_range(start_bracket..=start_bracket + end_bracket, "");
                    continue;
                }
            }
            break;
        }

        while synthesis.contains("**") {
            synthesis = synthesis.replacen("**", "", 2);
        }

        synthesis = synthesis.replace('\u{2014}', "--");
        synthesis = synthesis.replace('\u{2013}', "--");
        synthesis = synthesis.replace('\u{2018}', "'");
        synthesis = synthesis.replace('\u{2019}', "'");
        synthesis = synthesis.replace('\u{201C}', "\"");
        synthesis = synthesis.replace('\u{201D}', "\"");

        synthesis = synthesis
            .lines()
            .map(|line| line.trim_start_matches('#').trim_start())
            .collect::<Vec<_>>()
            .join("\n");

        let label_prefixes = [
            "Top Signal:",
            "Top Signals:",
            "Key Signal:",
            "Key Signals:",
            "Next Steps:",
            "Next Step:",
            "Action:",
            "Actions:",
            "Summary:",
            "Situation:",
            "Priority:",
            "Pattern:",
            "Recommendation:",
            "Recommendations:",
            "Note:",
            "Notes:",
            "Insight:",
            "Insights:",
            "Alert:",
            "Alerts:",
            "Cluster 1:",
            "Cluster 2:",
            "Cluster:",
            "Theme 1:",
            "Theme 2:",
            "Theme:",
            "Strongest Cluster:",
            "Strongest Clusters:",
            "Strongest Clusters",
            "Strongest Signal:",
            "Primary Cluster:",
            "Secondary Cluster:",
            "Top Cluster:",
            "Top Clusters:",
            "Action Required:",
            "Action Required",
            "Worth Knowing:",
            "Worth Knowing",
            "Filtered Out:",
            "Filtered Out",
            "Unresolved System Anomalies:",
            "Unresolved System Anomalies",
            "What to Watch:",
            "What to Watch",
            "Key Takeaway:",
            "Key Takeaways:",
            "Context:",
            "Impact:",
            "Background:",
            "Observation:",
            "Observations:",
        ];
        for prefix in &label_prefixes {
            for line_prefix in [*prefix, &prefix.to_uppercase()] {
                while synthesis.contains(line_prefix) {
                    synthesis = synthesis.replace(line_prefix, "");
                }
            }
        }

        synthesis = synthesis
            .lines()
            .filter(|line| !line.trim().chars().all(|c| c == '-') || line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        let source_type_tags = [
            "arxiv",
            "papers_with_code",
            "cve",
            "osv",
            "github_advisory",
            "hackernews",
            "lobsters",
            "reddit",
            "dev_to",
            "medium",
            "rss",
            "github",
            "npm",
            "crates_io",
            "pypi",
            "huggingface",
            "stackoverflow",
            "bluesky",
            "youtube",
            "producthunt",
            "go_modules",
        ];
        for st in &source_type_tags {
            let patterns = [
                format!("[{}]", st),
                format!("[Source_type: {}]", st),
                format!("[source_type: {}]", st),
                format!("[{}]", st.to_uppercase()),
            ];
            for pat in &patterns {
                synthesis = synthesis.replace(pat, "");
            }
        }

        while let Some(start_idx) = synthesis.find("(affects:") {
            if let Some(end_idx) = synthesis[start_idx..].find(')') {
                synthesis.replace_range(start_idx..=start_idx + end_idx, "");
            } else {
                break;
            }
        }
        while let Some(start_idx) = synthesis.find("[Affecting:") {
            if let Some(end_idx) = synthesis[start_idx..].find(']') {
                synthesis.replace_range(start_idx..=start_idx + end_idx, "");
            } else {
                break;
            }
        }
        while let Some(start_idx) = synthesis.find("[affecting:") {
            if let Some(end_idx) = synthesis[start_idx..].find(']') {
                synthesis.replace_range(start_idx..=start_idx + end_idx, "");
            } else {
                break;
            }
        }

        while synthesis.contains("  ") {
            synthesis = synthesis.replace("  ", " ");
        }

        let words: Vec<&str> = synthesis.split_whitespace().collect();
        if words.len() > 100 {
            tracing::info!(
                target: "4da::briefing",
                word_count = words.len(),
                "Synthesis exceeded 100 words — truncating"
            );
            let truncated = words[..100].join(" ");
            synthesis = if let Some(last_period) = truncated.rfind('.') {
                truncated[..=last_period].to_string()
            } else {
                format!("{truncated}.")
            };
        }

        let response = crate::llm::LLMResponse {
            content: synthesis,
            ..response
        };

        let report = crate::briefing_groundedness::validate_groundedness_with_packages(
            &response.content,
            &corpus,
            &packages,
        );

        if !report.is_acceptable(GROUNDEDNESS_THRESHOLD) {
            tracing::warn!(
                target: "4da::briefing",
                confidence = report.confidence,
                total_terms = report.total_terms,
                ungrounded_count = report.ungrounded_terms.len(),
                ungrounded_sample = ?report.ungrounded_terms.iter().take(5).collect::<Vec<_>>(),
                "Morning brief synthesis failed groundedness check — falling back to abstention"
            );
            return Ok(SynthesisResult {
                prose: "Low signal -- no noteworthy intelligence overnight.".to_string(),
                clusters: None,
                provider_used: provider_label.clone(),
                synthesis_tier: tier.as_str().to_string(),
            });
        }

        let fact_violations =
            crate::briefing_groundedness::check_factual_claims(&response.content, &package_facts);
        if !fact_violations.is_empty() {
            tracing::warn!(
                target: "4da::briefing",
                violations = ?fact_violations,
                "Morning brief synthesis stated wrong package versions — falling back to abstention"
            );
            return Ok(SynthesisResult {
                prose: "Low signal -- no noteworthy intelligence overnight.".to_string(),
                clusters: None,
                provider_used: provider_label.clone(),
                synthesis_tier: tier.as_str().to_string(),
            });
        }

        tracing::info!(
            target: "4da::briefing",
            confidence = report.confidence,
            total_terms = report.total_terms,
            claim_confidence = report.claim_confidence(),
            "Groundedness + factual checks passed"
        );

        let mut synthesis = response.content;
        synthesis.push_str(&format!("\n\n({})", synthesis_provenance(&briefing.items)));

        return Ok(SynthesisResult {
            prose: synthesis,
            clusters: None,
            provider_used: provider_label.clone(),
            synthesis_tier: tier.as_str().to_string(),
        });
    } // end provider loop

    Err(last_error.unwrap_or_else(|| {
        "All synthesis providers failed — check your cloud API key in Settings".into()
    }))
}

fn is_provider_error(err: &str) -> bool {
    let lower = err.to_lowercase();
    lower.contains("401")
        || lower.contains("403")
        || lower.contains("authentication")
        || lower.contains("unauthorized")
        || lower.contains("invalid api key")
        || lower.contains("invalid x-api-key")
        || lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("connect timeout")
        || lower.contains("dns error")
        || lower.contains("no route to host")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn freshness_test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE source_items (
                created_at TEXT NOT NULL,
                last_seen TEXT
            );
            CREATE TABLE feed_health (
                last_success_at TEXT,
                consecutive_failures INTEGER DEFAULT 0,
                circuit_open INTEGER DEFAULT 0
            );",
        )
        .unwrap();
        conn
    }

    fn mk_item(title: &str, source_type: &str, score: f32) -> BriefingItem {
        BriefingItem {
            title: title.into(),
            source_type: source_type.into(),
            score,
            signal_type: None,
            url: None,
            item_id: None,
            signal_priority: None,
            description: None,
            matched_deps: vec![],
            content_type: None,
            corroboration_count: 0,
            alt_sources: vec![],
            section: None,
            triage_reason: None,
        }
    }

    #[test]
    fn briefing_cmp_puts_actionable_first_then_deterministic() {
        // A security advisory must lead even with a LOWER score than a listicle —
        // the exact homelab-listicle-above-CVE failure this fixes.
        let mut cve = mk_item("HTTP/2 Bomb CVE", "mastodon", 0.80);
        cve.content_type = Some("security_advisory".into());
        cve.item_id = Some(10);
        let mut listicle = mk_item("awesome-homelab", "mastodon", 0.95);
        listicle.item_id = Some(20);
        let mut v = vec![listicle, cve];
        v.sort_by(briefing_item_cmp);
        assert_eq!(v[0].title, "HTTP/2 Bomb CVE", "actionable content leads");

        // Byte-identical scores order deterministically by item_id, not by chance.
        let mut a = mk_item("A", "hackernews", 0.9017062);
        a.item_id = Some(2);
        let mut b = mk_item("B", "reddit", 0.9017062);
        b.item_id = Some(1);
        let mut t = vec![a, b];
        t.sort_by(briefing_item_cmp);
        assert_eq!(t[0].title, "B", "lower item_id is the stable tiebreak");
    }

    #[test]
    fn synthesis_provenance_is_honest_about_corroboration() {
        let items = vec![
            mk_item("a", "hackernews", 0.9),
            mk_item("b", "reddit", 0.9),
            mk_item("c", "reddit", 0.9),
        ];
        // No corroboration -> must NOT imply cross-source confirmation.
        let p = synthesis_provenance(&items);
        assert_eq!(p, "3 items from 2 sources");
        assert!(!p.contains("signal"), "must not imply corroboration: {p}");

        // With real corroboration, say so explicitly.
        let mut corro = items.clone();
        corro[0].corroboration_count = 2;
        let p2 = synthesis_provenance(&corro);
        assert!(p2.contains("1 cross-source corroborated"), "got: {p2}");
    }

    #[test]
    fn test_matched_deps_from_signal_triggers_strips_prefix_and_dedupes() {
        let triggers = vec![
            "dep:axios".to_string(),
            "security".to_string(),
            "dep:tokio".to_string(),
            "dep:axios".to_string(),
            "dep: ".to_string(),
        ];

        let deps = matched_deps_from_signal_triggers(Some(triggers.as_slice()));
        assert_eq!(deps, vec!["axios".to_string(), "tokio".to_string()]);
    }

    #[test]
    fn test_load_briefing_matched_deps_filters_weak_links() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE source_item_dependencies (
                source_item_id INTEGER NOT NULL,
                package_name TEXT NOT NULL,
                match_type TEXT NOT NULL
            );
            INSERT INTO source_item_dependencies VALUES (1, 'axios', 'exact_registry');
            INSERT INTO source_item_dependencies VALUES (1, 'leftpad', 'title_heuristic');
            INSERT INTO source_item_dependencies VALUES (1, 'tokio', 'advisory');
            INSERT INTO source_item_dependencies VALUES (1, 'axios', 'exact_registry');
            INSERT INTO source_item_dependencies VALUES (2, 'serde', 'advisory');",
        )
        .unwrap();

        let deps = load_briefing_matched_deps(&conn, 1);
        assert_eq!(deps, vec!["axios".to_string(), "tokio".to_string()]);
    }

    #[test]
    fn test_data_freshness_not_stale_when_sources_checked_recently() {
        let conn = freshness_test_db();
        conn.execute(
            "INSERT INTO source_items (created_at, last_seen)
             VALUES (datetime('now', '-5 days'), datetime('now', '-5 days'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO feed_health (last_success_at, consecutive_failures, circuit_open)
             VALUES (datetime('now'), 0, 0)",
            [],
        )
        .unwrap();

        let freshness = compute_data_freshness_from_conn(&conn);
        assert_eq!(freshness.items_last_72h, 0);
        assert_eq!(freshness.source_checks_last_72h, 1);
        assert!(!freshness.is_stale);
    }

    #[test]
    fn test_data_freshness_stale_when_items_and_source_checks_are_old() {
        let conn = freshness_test_db();
        conn.execute(
            "INSERT INTO source_items (created_at, last_seen)
             VALUES (datetime('now', '-5 days'), datetime('now', '-5 days'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO feed_health (last_success_at, consecutive_failures, circuit_open)
             VALUES (datetime('now', '-5 days'), 0, 0)",
            [],
        )
        .unwrap();

        let freshness = compute_data_freshness_from_conn(&conn);
        assert_eq!(freshness.items_last_72h, 0);
        assert_eq!(freshness.source_checks_last_72h, 0);
        assert_eq!(freshness.stale_sources, 0);
        assert!(freshness.is_stale);
    }

    #[test]
    fn test_data_freshness_reprocessed_items_not_counted_as_fresh() {
        let conn = freshness_test_db();
        // Item created 5 days ago but last_seen updated recently (reprocessing)
        conn.execute(
            "INSERT INTO source_items (created_at, last_seen)
             VALUES (datetime('now', '-5 days'), datetime('now'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO feed_health (last_success_at, consecutive_failures, circuit_open)
             VALUES (datetime('now', '-5 days'), 0, 0)",
            [],
        )
        .unwrap();

        let freshness = compute_data_freshness_from_conn(&conn);
        // items_last_24h must use created_at, NOT last_seen
        assert_eq!(freshness.items_last_24h, 0);
        assert_eq!(freshness.items_last_72h, 0);
        assert!(freshness.is_stale);
        assert!(freshness.no_recent_fetches);
    }

    #[test]
    fn test_data_freshness_no_recent_fetches_amber_warning() {
        let conn = freshness_test_db();
        conn.execute(
            "INSERT INTO source_items (created_at, last_seen)
             VALUES (datetime('now', '-2 days'), datetime('now', '-2 days'))",
            [],
        )
        .unwrap();
        // Source checked 2 days ago (within 72h) but not in last 24h
        conn.execute(
            "INSERT INTO feed_health (last_success_at, consecutive_failures, circuit_open)
             VALUES (datetime('now', '-2 days'), 0, 0)",
            [],
        )
        .unwrap();

        let freshness = compute_data_freshness_from_conn(&conn);
        assert!(!freshness.is_stale);
        assert!(freshness.no_recent_fetches);
    }

    #[test]
    fn test_data_freshness_stale_when_no_checks_72h_despite_items() {
        let conn = freshness_test_db();
        // Items exist in recent window (created recently)
        conn.execute(
            "INSERT INTO source_items (created_at, last_seen)
             VALUES (datetime('now', '-1 hours'), datetime('now'))",
            [],
        )
        .unwrap();
        // But no source checks have succeeded in 72h+
        conn.execute(
            "INSERT INTO feed_health (last_success_at, consecutive_failures, circuit_open)
             VALUES (datetime('now', '-5 days'), 0, 0)",
            [],
        )
        .unwrap();

        let freshness = compute_data_freshness_from_conn(&conn);
        assert!(freshness.items_last_24h > 0);
        assert_eq!(freshness.source_checks_last_72h, 0);
        // Must still be stale — no fetches succeeded even if items exist
        assert!(freshness.is_stale);
    }

    #[test]
    fn test_parse_briefing_time_valid() {
        assert_eq!(parse_briefing_time("08:00"), (8, 0));
        assert_eq!(parse_briefing_time("23:59"), (23, 59));
        assert_eq!(parse_briefing_time("00:00"), (0, 0));
        assert_eq!(parse_briefing_time("12:30"), (12, 30));
    }

    #[test]
    fn test_parse_briefing_time_invalid() {
        // Invalid formats fall back to (8, 0)
        assert_eq!(parse_briefing_time("invalid"), (8, 0));
        assert_eq!(parse_briefing_time(""), (8, 0));
        assert_eq!(parse_briefing_time("8"), (8, 0));
    }

    #[test]
    fn test_parse_briefing_time_clamped() {
        // Hours and minutes clamped to valid range
        assert_eq!(parse_briefing_time("99:99"), (23, 59));
        assert_eq!(parse_briefing_time("30:70"), (23, 59));
    }

    #[test]
    fn test_briefing_item_construction() {
        let item = BriefingItem {
            title: "Rust 2026 Edition announced".to_string(),
            source_type: "hackernews".to_string(),
            score: 0.85,
            signal_type: Some("new_release".to_string()),
            url: Some("https://example.com/rust-2026".to_string()),
            item_id: Some(42),
            signal_priority: Some("alert".to_string()),
            description: Some("Review Rust 2026 changes".to_string()),
            matched_deps: vec!["rust".to_string()],
            content_type: None,
            corroboration_count: 0,
            alt_sources: vec![],
            section: None,
            triage_reason: None,
        };
        assert_eq!(item.title, "Rust 2026 Edition announced");
        assert_eq!(item.source_type, "hackernews");
        assert!(item.score > 0.8);
        assert_eq!(item.signal_type, Some("new_release".to_string()));
        assert!(item.url.is_some());
        assert_eq!(item.item_id, Some(42));
        assert_eq!(item.matched_deps.len(), 1);
    }

    #[test]
    fn test_briefing_notification_construction() {
        let briefing = BriefingNotification {
            title: "4DA Intelligence Briefing — 19 Mar 2026".to_string(),
            items: vec![
                BriefingItem {
                    title: "Critical CVE in tokio".to_string(),
                    source_type: "github".to_string(),
                    score: 0.95,
                    signal_type: Some("security_alert".to_string()),
                    url: Some("https://example.com/cve".to_string()),
                    item_id: Some(1),
                    signal_priority: Some("critical".to_string()),
                    description: Some("Patch tokio immediately".to_string()),
                    matched_deps: vec!["tokio".to_string()],
                    content_type: None,
                    corroboration_count: 0,
                    alt_sources: vec![],
                    section: None,
                    triage_reason: None,
                },
                BriefingItem {
                    title: "Tauri 3.0 beta released".to_string(),
                    source_type: "hackernews".to_string(),
                    score: 0.80,
                    signal_type: Some("new_release".to_string()),
                    url: None,
                    item_id: Some(2),
                    signal_priority: Some("advisory".to_string()),
                    description: None,
                    matched_deps: vec![],
                    content_type: None,
                    corroboration_count: 0,
                    alt_sources: vec![],
                    section: None,
                    triage_reason: None,
                },
            ],
            total_relevant: 2,
            ongoing_topics: vec![],
            knowledge_gaps: vec![KnowledgeGap {
                topic: "sqlite".to_string(),
                days_since_last: 7,
            }],
            escalating_chains: vec![ChainSummary {
                name: "tokio security chain".to_string(),
                phase: "escalating".to_string(),
                link_count: 3,
                action: "Review tokio security patches".to_string(),
                confidence: 0.85,
            }],
            synthesis: None,
            preemption_alerts: vec![],
            blind_spot_score: None,
            labels: None,
            personalization_context: None,
            data_freshness: None,
            corroboration_available: false,
            coverage_building: false,
            synthesis_hint: None,
        };
        assert_eq!(briefing.items.len(), 2);
        assert_eq!(briefing.total_relevant, 2);
        assert!(briefing.title.contains("Intelligence Briefing"));
        assert_eq!(briefing.knowledge_gaps.len(), 1);
        assert_eq!(briefing.knowledge_gaps[0].days_since_last, 7);
        assert_eq!(briefing.escalating_chains.len(), 1);
        assert_eq!(briefing.escalating_chains[0].phase, "escalating");
    }

    #[test]
    fn test_briefing_window_midnight_rollover() {
        // If briefing_time is 23:45 and current time is 00:05,
        // diff should be 20 (within 30-min window)
        let target = 23 * 60 + 45; // 1425
        let now = 5; // 00:05
        let diff = ((now as i32 - target as i32) + 1440) % 1440;
        assert_eq!(diff, 20);
        assert!(diff <= 30);
    }

    #[test]
    fn test_briefing_window_midnight_rollover_outside() {
        // If briefing_time is 23:45 and current time is 00:30,
        // diff should be 45 (outside 30-min window)
        let target = 23 * 60 + 45; // 1425
        let now = 30; // 00:30
        let diff = ((now as i32 - target as i32) + 1440) % 1440;
        assert_eq!(diff, 45);
        assert!(diff > 30);
    }

    #[test]
    fn test_briefing_window_normal_case() {
        // If briefing_time is 08:00 and current time is 08:15,
        // diff should be 15 (within 30-min window)
        let target = 8 * 60; // 480
        let now = 8 * 60 + 15; // 495
        let diff = ((now as i32 - target as i32) + 1440) % 1440;
        assert_eq!(diff, 15);
        assert!(diff <= 30);
    }

    #[test]
    fn test_briefing_window_before_target() {
        // If briefing_time is 08:00 and current time is 07:50,
        // diff should be 1430 (well outside 30-min window — not yet time)
        let target = 8 * 60; // 480
        let now = 7 * 60 + 50; // 470
        let diff = ((now as i32 - target as i32) + 1440) % 1440;
        assert_eq!(diff, 1430);
        assert!(diff > 30);
    }

    #[test]
    fn test_morning_briefing_date_tracking() {
        let state = MonitoringState::new();
        // Initially no date
        assert!(state.last_morning_briefing_date.lock().is_none());

        // Set a date
        {
            let mut guard = state.last_morning_briefing_date.lock();
            *guard = Some("2026-03-19".to_string());
        }

        // Verify it's set
        let guard = state.last_morning_briefing_date.lock();
        assert_eq!(*guard, Some("2026-03-19".to_string()));
    }

    // ══════════════════════════════════════════════════════════════════════
    // Diversity slot tests
    // ══════════════════════════════════════════════════════════════════════

    fn make_briefing_item(title: &str, source: &str, score: f32) -> BriefingItem {
        BriefingItem {
            title: title.to_string(),
            source_type: source.to_string(),
            score,
            signal_type: None,
            url: None,
            item_id: None,
            signal_priority: None,
            description: None,
            matched_deps: vec![],
            content_type: None,
            corroboration_count: 0,
            alt_sources: vec![],
            section: None,
            triage_reason: None,
        }
    }

    fn make_item_with_type(
        title: &str,
        source: &str,
        score: f32,
        content_type: Option<&str>,
    ) -> BriefingItem {
        BriefingItem {
            content_type: content_type.map(String::from),
            ..make_briefing_item(title, source, score)
        }
    }

    #[test]
    fn test_diversity_empty_input() {
        let result = apply_diversity_slots(vec![], 8);
        assert!(result.is_empty());
    }

    #[test]
    fn test_diversity_zero_max() {
        let items = vec![make_briefing_item("A", "hackernews", 0.9)];
        let result = apply_diversity_slots(items, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_diversity_single_source_passthrough() {
        // All items from one source — no diversity logic needed
        let items = vec![
            make_briefing_item("A", "hackernews", 0.9),
            make_briefing_item("B", "hackernews", 0.8),
            make_briefing_item("C", "hackernews", 0.7),
        ];
        let result = apply_diversity_slots(items, 8);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].title, "A");
    }

    #[test]
    fn test_diversity_fits_within_budget() {
        // 3 items, budget of 8 — all should be returned as-is
        let items = vec![
            make_briefing_item("A", "hackernews", 0.9),
            make_briefing_item("B", "reddit", 0.8),
            make_briefing_item("C", "cve", 0.7),
        ];
        let result = apply_diversity_slots(items, 8);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_diversity_guarantees_one_per_source() {
        // 10 items: 8 from hackernews (high scores), 1 reddit, 1 cve (lower scores)
        // Without diversity: top-8 would be all hackernews
        // With diversity: reddit and cve each get a guaranteed slot
        let mut items = Vec::new();
        for i in 0..8 {
            items.push(make_briefing_item(
                &format!("HN-{i}"),
                "hackernews",
                0.90 - (i as f32 * 0.01),
            ));
        }
        items.push(make_briefing_item("Reddit item", "reddit", 0.60));
        items.push(make_briefing_item("CVE item", "cve", 0.55));

        // Pre-sort by score descending (as build_enriched_briefing does)
        items.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let result = apply_diversity_slots(items, 8);
        assert_eq!(result.len(), 8);

        let sources: Vec<&str> = result.iter().map(|i| i.source_type.as_str()).collect();
        assert!(
            sources.contains(&"reddit"),
            "reddit must have a diversity slot; got sources: {:?}",
            sources
        );
        assert!(
            sources.contains(&"cve"),
            "cve must have a diversity slot; got sources: {:?}",
            sources
        );
    }

    #[test]
    fn test_diversity_preserves_priority_ordering() {
        // Items pre-sorted by priority then score
        let items = vec![
            BriefingItem {
                signal_priority: Some("critical".to_string()),
                ..make_briefing_item("Critical CVE", "cve", 0.95)
            },
            make_briefing_item("HN-1", "hackernews", 0.90),
            make_briefing_item("HN-2", "hackernews", 0.85),
            make_briefing_item("HN-3", "hackernews", 0.80),
            make_briefing_item("HN-4", "hackernews", 0.75),
            make_briefing_item("HN-5", "hackernews", 0.70),
            make_briefing_item("HN-6", "hackernews", 0.65),
            make_briefing_item("HN-7", "hackernews", 0.60),
            make_briefing_item("Reddit low", "reddit", 0.50),
        ];

        let result = apply_diversity_slots(items, 8);

        // Critical item must still be first
        assert_eq!(result[0].title, "Critical CVE");
        // Reddit must appear somewhere
        assert!(result.iter().any(|i| i.source_type == "reddit"));
    }

    #[test]
    fn test_diversity_more_sources_than_budget() {
        // 10 different sources, budget of 8 — must truncate diversity picks
        let mut items = Vec::new();
        for i in 0..10 {
            items.push(make_briefing_item(
                &format!("Item-{i}"),
                &format!("source-{i}"),
                0.90 - (i as f32 * 0.05),
            ));
        }
        let result = apply_diversity_slots(items, 8);
        assert_eq!(result.len(), 8);

        // Should keep the top 8 by original sort order
        assert_eq!(result[0].title, "Item-0");
        assert_eq!(result[7].title, "Item-7");
    }

    #[test]
    fn test_diversity_single_item() {
        let items = vec![make_briefing_item("Only one", "hackernews", 0.9)];
        let result = apply_diversity_slots(items, 8);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Only one");
    }

    fn empty_briefing() -> BriefingNotification {
        BriefingNotification {
            title: String::new(),
            items: vec![],
            total_relevant: 0,
            ongoing_topics: vec![],
            knowledge_gaps: vec![],
            escalating_chains: vec![],
            synthesis: None,
            preemption_alerts: vec![],
            blind_spot_score: None,
            labels: None,
            personalization_context: None,
            data_freshness: None,
            corroboration_available: false,
            coverage_building: false,
            synthesis_hint: None,
        }
    }

    fn make_test_preemption_alert(
        title: &str,
        urgency: &str,
        explanation: &str,
    ) -> BriefingPreemptionAlert {
        BriefingPreemptionAlert {
            title: title.into(),
            urgency: urgency.into(),
            explanation: explanation.into(),
            alert_id: None,
            package_name: None,
            ecosystem: None,
            installed_version: None,
            fixed_version: None,
            affected_projects: vec![],
            is_direct: None,
            is_dev: None,
            advisory_ids: vec![],
            source_url: None,
            suggested_actions: vec![],
            scope: None,
            merged_package_versions: Vec::new(),
        }
    }

    #[test]
    fn test_preemption_only_briefing_fires() {
        let mut b = empty_briefing();
        b.preemption_alerts.push(make_test_preemption_alert(
            "axios CVE-2024-1234",
            "critical",
            "RCE in axios < 1.7",
        ));
        b.preemption_alerts.push(make_test_preemption_alert(
            "jsonwebtoken timing attack",
            "high",
            "HMAC comparison not constant-time",
        ));
        assert!(b.has_meaningful_content());
    }

    #[test]
    fn dedupe_collapses_same_advisory_into_one_row_listing_packages() {
        let mut a1 = make_test_preemption_alert("Clerk auth bypass", "high", "x");
        a1.package_name = Some("@clerk/clerk-react".into());
        a1.advisory_ids = vec!["GHSA-w24r-5266-9c3c".into()];
        a1.affected_projects = vec!["navcal".into()];
        let mut a2 = make_test_preemption_alert("Clerk auth bypass", "high", "x");
        a2.package_name = Some("@clerk/shared".into());
        a2.advisory_ids = vec!["GHSA-w24r-5266-9c3c".into()];
        a2.affected_projects = vec!["navcal/vercel-workflow".into()];
        let mut a3 = make_test_preemption_alert("axios advisories", "high", "x");
        a3.package_name = Some("axios".into());
        a3.advisory_ids = vec!["GHSA-aaaa-bbbb-cccc".into()];

        let out = dedupe_alerts_by_advisory(vec![a1, a2, a3]);
        assert_eq!(
            out.len(),
            2,
            "two Clerk rows collapse to one; axios stays distinct"
        );
        assert_eq!(
            out[0].package_name.as_deref(),
            Some("@clerk/clerk-react, @clerk/shared"),
            "merged row lists both affected packages"
        );
        assert!(
            out[0]
                .affected_projects
                .contains(&"navcal/vercel-workflow".to_string()),
            "merged row unions affected projects"
        );
    }

    #[test]
    fn dedupe_keeps_unidentified_alerts_separate() {
        // No advisory id -> we can't prove equality -> both rows are kept.
        let a1 = make_test_preemption_alert("Some alert", "high", "x");
        let a2 = make_test_preemption_alert("Some alert", "high", "x");
        assert_eq!(dedupe_alerts_by_advisory(vec![a1, a2]).len(), 2);
    }

    #[test]
    fn dedupe_does_not_duplicate_a_package_already_listed() {
        let mut a1 = make_test_preemption_alert("dup pkg", "high", "x");
        a1.package_name = Some("axios".into());
        a1.advisory_ids = vec!["GHSA-x".into()];
        let mut a2 = make_test_preemption_alert("dup pkg", "high", "x");
        a2.package_name = Some("axios".into());
        a2.advisory_ids = vec!["GHSA-x".into()];
        let out = dedupe_alerts_by_advisory(vec![a1, a2]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].package_name.as_deref(), Some("axios"));
    }

    #[test]
    fn dedupe_preserves_per_package_versions_for_merged_advisory() {
        // One advisory hitting two packages with DIFFERENT versions must preserve
        // EACH package's real (installed, fix) pair. The comma-joined display
        // `package_name` + single version fields are lossy for merged rows; the
        // factual-version check reads `merged_package_versions` instead so it can
        // validate every package, not just the first merged one (bug_004).
        let mut a1 = make_test_preemption_alert("Clerk auth bypass", "high", "x");
        a1.package_name = Some("@clerk/clerk-react".into());
        a1.installed_version = Some("5.0.0".into());
        a1.fixed_version = Some("5.0.1".into());
        a1.advisory_ids = vec!["GHSA-w24r-5266-9c3c".into()];
        let mut a2 = make_test_preemption_alert("Clerk auth bypass", "high", "x");
        a2.package_name = Some("@clerk/shared".into());
        a2.installed_version = Some("2.0.0".into());
        a2.fixed_version = Some("2.0.1".into());
        a2.advisory_ids = vec!["GHSA-w24r-5266-9c3c".into()];

        let out = dedupe_alerts_by_advisory(vec![a1, a2]);
        assert_eq!(out.len(), 1, "same advisory collapses to one row");
        let merged = &out[0].merged_package_versions;
        assert_eq!(merged.len(), 2, "both packages' versions preserved");
        let react = merged
            .iter()
            .find(|m| m.package_name == "@clerk/clerk-react")
            .expect("react package preserved");
        assert_eq!(react.installed_version.as_deref(), Some("5.0.0"));
        assert_eq!(react.fixed_version.as_deref(), Some("5.0.1"));
        let shared = merged
            .iter()
            .find(|m| m.package_name == "@clerk/shared")
            .expect("shared package preserved");
        assert_eq!(shared.installed_version.as_deref(), Some("2.0.0"));
        assert_eq!(shared.fixed_version.as_deref(), Some("2.0.1"));
    }

    #[test]
    fn dedupe_single_package_alert_has_no_merged_versions() {
        // A non-merged alert keeps merged_package_versions empty — the fact
        // builder then reads its own faithful package_name/version fields.
        let mut a1 = make_test_preemption_alert("axios advisory", "high", "x");
        a1.package_name = Some("axios".into());
        a1.advisory_ids = vec!["GHSA-aaaa-bbbb-cccc".into()];
        let out = dedupe_alerts_by_advisory(vec![a1]);
        assert_eq!(out.len(), 1);
        assert!(out[0].merged_package_versions.is_empty());
    }

    #[test]
    fn is_abstention_synthesis_anchors_on_canonical_first_line() {
        // Canonical abstentions: both dash forms, both wordings, any case, plus a
        // telemetry tail on a later line.
        assert!(is_abstention_synthesis(
            "Low signal -- no noteworthy intelligence overnight."
        ));
        assert!(is_abstention_synthesis(
            "Low signal \u{2014} no new intelligence overnight."
        ));
        assert!(is_abstention_synthesis(
            "Low signal \u{2013} no noteworthy intelligence overnight."
        ));
        assert!(is_abstention_synthesis(
            "LOW SIGNAL - NO NOTEWORTHY INTELLIGENCE OVERNIGHT."
        ));
        assert!(is_abstention_synthesis(
            "Low signal -- no noteworthy intelligence overnight.\n\n(25 items scanned, synthesis skipped)"
        ));
        // NOT abstentions — the two false positives the old lax detector tripped:
        // "no noteworthy" used mid-prose (bug_018) ...
        assert!(!is_abstention_synthesis(
            "PRIORITY: patch axios; no noteworthy followups on Tokio yet. Three things to know."
        ));
        // ... and a real synthesis that merely opens with the words "low signal".
        assert!(!is_abstention_synthesis(
            "Low signal strength on the wifi today, but plenty to report."
        ));
        assert!(!is_abstention_synthesis(
            "Three things to know: patch axios, watch Tokio, ship the brief."
        ));
        assert!(!is_abstention_synthesis(""));
    }

    #[test]
    fn is_abstention_prose_detects_gate_rejections_but_not_real_synthesis() {
        // The exact prose every synthesis gate emits on rejection — must match so
        // the retry wrapper re-attempts instead of shipping a blank brief.
        assert!(is_abstention_prose(ABSTENTION_PROSE));
        assert!(is_abstention_prose(
            "Low signal -- no noteworthy intelligence overnight.\n\n(8 items)"
        ));
        // Real synthesis must NOT read as abstention (else we'd retry good output).
        assert!(!is_abstention_prose(
            "Upgrade jsonwebtoken to 10.3.0 in 4da/relay; it has a type confusion flaw."
        ));
        assert!(!is_abstention_prose(""));
    }

    #[test]
    fn test_truly_empty_briefing_blocked() {
        assert!(!empty_briefing().has_meaningful_content());
    }

    #[test]
    fn test_knowledge_gaps_urgency_threshold() {
        let mut b = empty_briefing();
        b.knowledge_gaps.push(KnowledgeGap {
            topic: "rust".into(),
            days_since_last: 2,
        });
        assert!(!b.has_meaningful_content(), "2-day gap is low urgency");

        b.knowledge_gaps.push(KnowledgeGap {
            topic: "security".into(),
            days_since_last: 10,
        });
        assert!(b.has_meaningful_content(), "10-day gap is high urgency");
    }

    // ══════════════════════════════════════════════════════════════════════
    // Content type classifier tests
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_actionable_content_types() {
        assert!(is_actionable_content_type(Some("security_advisory")));
        assert!(is_actionable_content_type(Some("vulnerability_report")));
        assert!(is_actionable_content_type(Some("cve")));
        assert!(is_actionable_content_type(Some("breaking_change")));
        assert!(is_actionable_content_type(Some("deprecation_notice")));
        assert!(is_actionable_content_type(Some("release_notes")));
        assert!(is_actionable_content_type(Some("platform_update")));
        assert!(is_actionable_content_type(Some("migration_guide")));

        assert!(!is_actionable_content_type(Some("tutorial")));
        assert!(!is_actionable_content_type(Some("blog_post")));
        assert!(!is_actionable_content_type(Some("show_and_tell")));
        assert!(!is_actionable_content_type(None));
    }

    #[test]
    fn test_optional_reading_types() {
        assert!(is_optional_reading_type(Some("tutorial")));
        assert!(is_optional_reading_type(Some("show_and_tell")));
        assert!(is_optional_reading_type(Some("blog_post")));
        assert!(is_optional_reading_type(Some("discussion")));
        assert!(is_optional_reading_type(None));

        assert!(!is_optional_reading_type(Some("security_advisory")));
        assert!(!is_optional_reading_type(Some("cve")));
    }

    // ══════════════════════════════════════════════════════════════════════
    // Section split tests
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_actionable_items_prioritized() {
        // 10 items: 2 security, 1 breaking, 3 tutorials, 4 blog posts
        // After section split, the first items must be security + breaking.
        let items = vec![
            make_item_with_type("CVE in tokio", "github", 0.90, Some("security_advisory")),
            make_item_with_type("CVE in serde", "github", 0.85, Some("cve")),
            make_item_with_type(
                "Breaking: Rust 2027",
                "hackernews",
                0.80,
                Some("breaking_change"),
            ),
            make_item_with_type("Learn Rust in 30 days", "reddit", 0.78, Some("tutorial")),
            make_item_with_type(
                "How to build a web server",
                "reddit",
                0.75,
                Some("tutorial"),
            ),
            make_item_with_type(
                "My first Rust project",
                "hackernews",
                0.72,
                Some("tutorial"),
            ),
            make_item_with_type("Why I switched to Rust", "reddit", 0.70, Some("blog_post")),
            make_item_with_type(
                "Rust vs Go benchmark",
                "hackernews",
                0.68,
                Some("blog_post"),
            ),
            make_item_with_type("Async patterns in Rust", "reddit", 0.65, Some("blog_post")),
            make_item_with_type(
                "Rust adoption survey",
                "hackernews",
                0.60,
                Some("blog_post"),
            ),
        ];

        let result = apply_section_split(items);

        // First 3 items must be the actionable ones (security + breaking)
        assert_eq!(result[0].section.as_deref(), Some("action"));
        assert_eq!(result[1].section.as_deref(), Some("action"));
        assert_eq!(result[2].section.as_deref(), Some("action"));
        assert_eq!(result[0].title, "CVE in tokio");
        assert_eq!(result[1].title, "CVE in serde");
        assert_eq!(result[2].title, "Breaking: Rust 2027");

        // Remaining non-actionable items: score >= 0.7 -> watch, < 0.7 -> reading
        for item in &result[3..] {
            if item.score >= 0.7 {
                assert_eq!(
                    item.section.as_deref(),
                    Some("watch"),
                    "{} (score {}) should be watch",
                    item.title,
                    item.score
                );
            } else {
                assert_eq!(
                    item.section.as_deref(),
                    Some("reading"),
                    "{} (score {}) should be reading",
                    item.title,
                    item.score
                );
            }
        }
    }

    #[test]
    fn test_null_content_type_not_in_actions() {
        // Item with None content_type should NOT be in "action" section
        let items = vec![
            make_item_with_type("Mystery item", "rss", 0.80, None),
            make_item_with_type("CVE-2026-1234", "github", 0.75, Some("cve")),
        ];

        let result = apply_section_split(items);

        // The CVE should be first (action section)
        assert_eq!(result[0].title, "CVE-2026-1234");
        assert_eq!(result[0].section.as_deref(), Some("action"));

        // The None content_type item should be in reading, not action
        let mystery = result.iter().find(|i| i.title == "Mystery item").unwrap();
        assert_ne!(
            mystery.section.as_deref(),
            Some("action"),
            "NULL content_type must not be in action section"
        );
    }

    #[test]
    fn test_section_caps() {
        // 10 actionable items -> only 5 should make it to the final list
        let items: Vec<BriefingItem> = (0..10)
            .map(|i| {
                make_item_with_type(
                    &format!("CVE-{i}"),
                    "github",
                    0.90 - (i as f32 * 0.01),
                    Some("security_advisory"),
                )
            })
            .collect();

        let result = apply_section_split(items);

        // Only 5 action items should survive the cap
        let action_count = result
            .iter()
            .filter(|i| i.section.as_deref() == Some("action"))
            .count();
        assert_eq!(action_count, 5, "action section must be capped at 5");
        assert_eq!(result.len(), 5, "no other sections present, total = 5");

        // The first 5 by score should be kept
        assert_eq!(result[0].title, "CVE-0");
        assert_eq!(result[4].title, "CVE-4");
    }

    #[test]
    fn test_watch_section_requires_corroboration() {
        // Score >= 0.7 goes to watch regardless of corroboration (high confidence)
        let mut high_score_no_corrob =
            make_item_with_type("Interesting post", "hackernews", 0.80, Some("blog_post"));
        high_score_no_corrob.corroboration_count = 0;

        // Score 0.5-0.7 with corroboration -> watch
        let mut mid_score_corrob =
            make_item_with_type("Hot topic", "reddit", 0.55, Some("discussion"));
        mid_score_corrob.corroboration_count = 2;

        // Score 0.5-0.7 without corroboration -> reading
        let mut mid_score_no_corrob =
            make_item_with_type("Lone signal", "devto", 0.60, Some("blog_post"));
        mid_score_no_corrob.corroboration_count = 0;

        let items = vec![high_score_no_corrob, mid_score_corrob, mid_score_no_corrob];
        let result = apply_section_split(items);

        let interesting = result
            .iter()
            .find(|i| i.title == "Interesting post")
            .unwrap();
        assert_eq!(
            interesting.section.as_deref(),
            Some("watch"),
            "high-score (>=0.7) item goes to watch even without corroboration"
        );

        let hot = result.iter().find(|i| i.title == "Hot topic").unwrap();
        assert_eq!(
            hot.section.as_deref(),
            Some("watch"),
            "mid-score corroborated item goes to watch"
        );

        let lone = result.iter().find(|i| i.title == "Lone signal").unwrap();
        assert_eq!(
            lone.section.as_deref(),
            Some("reading"),
            "mid-score uncorroborated item goes to reading"
        );
    }

    #[test]
    fn test_low_quality_commodity_show_and_tell() {
        // show_and_tell below 0.45 should be flagged as commodity
        let show_tell = make_item_with_type("My project", "hn", 0.40, Some("show_and_tell"));
        assert!(
            is_low_quality_commodity(&show_tell),
            "show_and_tell below 0.45 is commodity"
        );

        // show_and_tell above 0.45 is not commodity
        let show_tell_good = make_item_with_type("My project", "hn", 0.50, Some("show_and_tell"));
        assert!(
            !is_low_quality_commodity(&show_tell_good),
            "show_and_tell above 0.45 is not commodity"
        );
    }

    #[test]
    fn test_low_quality_null_content_type() {
        // NULL content_type below 0.55 is treated as low-confidence commodity
        let null_low = make_item_with_type("Unknown item", "rss", 0.50, None);
        assert!(
            is_low_quality_commodity(&null_low),
            "NULL content_type below 0.55 is commodity"
        );

        // NULL content_type at 0.55+ is not commodity
        let null_ok = make_item_with_type("Unknown item", "rss", 0.55, None);
        assert!(
            !is_low_quality_commodity(&null_ok),
            "NULL content_type at 0.55+ is not commodity"
        );
    }

    // ══════════════════════════════════════════════════════════════════════
    // BriefingPreemptionAlert mapping & serialization tests
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_briefing_preemption_from_full_alert() {
        use crate::preemption::{
            AlertEvidence, AlertUrgency, PreemptionAlert, PreemptionType, SuggestedAction,
        };
        let alert = PreemptionAlert {
            id: "osv-pkg-axios-npm".into(),
            alert_type: PreemptionType::SecurityAdvisory,
            title: "axios@1.6.0: 2 known vulnerabilities".into(),
            explanation: "GHSA-wf5p-g6vw-rhxx, CVE-2024-39338 affect axios@1.6.0".into(),
            evidence: vec![
                AlertEvidence {
                    source: "osv".into(),
                    title: "GHSA-wf5p-g6vw-rhxx: Axios SSRF".into(),
                    url: Some("https://github.com/advisories/GHSA-wf5p-g6vw-rhxx".into()),
                    freshness_days: 5.0,
                    relevance_score: 1.0,
                },
                AlertEvidence {
                    source: "osv".into(),
                    title: "CVE-2024-39338: Server-Side Request Forgery".into(),
                    url: Some("https://nvd.nist.gov/vuln/detail/CVE-2024-39338".into()),
                    freshness_days: 12.0,
                    relevance_score: 1.0,
                },
            ],
            affected_projects: vec!["/home/user/my-project".into()],
            affected_dependencies: vec!["axios".into()],
            urgency: AlertUrgency::Critical,
            confidence: 0.95,
            predicted_window: None,
            suggested_actions: vec![
                SuggestedAction {
                    action_type: "investigate".into(),
                    label: "Update axios from 1.6.0 to >= 1.7.4".into(),
                    description: "Review advisories and update.".into(),
                },
                SuggestedAction {
                    action_type: "dismiss".into(),
                    label: "Not affected".into(),
                    description: "Dismiss if confirmed.".into(),
                },
            ],
            created_at: "2026-05-16T00:00:00Z".into(),
            osv_verified: true,
            source_classified: false,
            installed_version: Some("1.6.0".into()),
            fixed_version: Some("1.7.4".into()),
            is_direct: Some(true),
            is_dev: Some(false),
            platform_inactive: false,
        };

        let briefing = BriefingPreemptionAlert::from_preemption_alert(&alert);

        assert_eq!(briefing.title, "axios@1.6.0: 2 known vulnerabilities");
        assert_eq!(briefing.urgency, "critical");
        assert_eq!(briefing.alert_id.as_deref(), Some("osv-pkg-axios-npm"));
        assert_eq!(briefing.package_name.as_deref(), Some("axios"));
        assert_eq!(briefing.ecosystem.as_deref(), Some("npm"));
        assert_eq!(briefing.installed_version.as_deref(), Some("1.6.0"));
        assert_eq!(briefing.fixed_version.as_deref(), Some("1.7.4"));
        assert_eq!(briefing.affected_projects, vec!["/home/user/my-project"]);
        assert_eq!(briefing.is_direct, Some(true));
        assert_eq!(briefing.is_dev, Some(false));
        assert_eq!(
            briefing.source_url.as_deref(),
            Some("https://github.com/advisories/GHSA-wf5p-g6vw-rhxx")
        );
        assert_eq!(
            briefing.suggested_actions,
            vec!["Update axios from 1.6.0 to >= 1.7.4", "Not affected"]
        );
        assert!(briefing
            .advisory_ids
            .contains(&"GHSA-wf5p-g6vw-rhxx".to_string()));
        assert!(briefing
            .advisory_ids
            .contains(&"CVE-2024-39338".to_string()));
    }

    #[test]
    fn test_briefing_preemption_serializes_all_fields() {
        let alert = BriefingPreemptionAlert {
            title: "lodash prototype pollution".into(),
            urgency: "high".into(),
            explanation: "CVE-2020-28500 affects lodash@4.17.20".into(),
            alert_id: Some("osv-pkg-lodash-npm".into()),
            package_name: Some("lodash".into()),
            ecosystem: Some("npm".into()),
            installed_version: Some("4.17.20".into()),
            fixed_version: Some("4.17.21".into()),
            affected_projects: vec!["/app".into()],
            is_direct: Some(true),
            is_dev: Some(false),
            advisory_ids: vec!["CVE-2020-28500".into()],
            source_url: Some("https://nvd.nist.gov/vuln/detail/CVE-2020-28500".into()),
            suggested_actions: vec!["Update lodash to >= 4.17.21".into()],
            scope: None,
            merged_package_versions: Vec::new(),
        };

        let json = serde_json::to_value(&alert).expect("serialize");
        assert_eq!(json["title"], "lodash prototype pollution");
        assert_eq!(json["alert_id"], "osv-pkg-lodash-npm");
        assert_eq!(json["package_name"], "lodash");
        assert_eq!(json["ecosystem"], "npm");
        assert_eq!(json["installed_version"], "4.17.20");
        assert_eq!(json["fixed_version"], "4.17.21");
        assert_eq!(json["affected_projects"][0], "/app");
        assert_eq!(json["is_direct"], true);
        assert_eq!(json["is_dev"], false);
        assert_eq!(json["advisory_ids"][0], "CVE-2020-28500");
        assert_eq!(
            json["source_url"],
            "https://nvd.nist.gov/vuln/detail/CVE-2020-28500"
        );
        assert_eq!(json["suggested_actions"][0], "Update lodash to >= 4.17.21");
    }

    #[test]
    fn test_briefing_preemption_defaults_serialize_cleanly() {
        let alert = make_test_preemption_alert("test alert", "high", "explanation");
        let json = serde_json::to_value(&alert).expect("serialize");

        assert_eq!(json["alert_id"], serde_json::Value::Null);
        assert_eq!(json["package_name"], serde_json::Value::Null);
        assert_eq!(json["affected_projects"], serde_json::json!([]));
        assert_eq!(json["advisory_ids"], serde_json::json!([]));
        assert_eq!(json["suggested_actions"], serde_json::json!([]));
        assert_eq!(json["title"], "test alert");
        assert_eq!(json["urgency"], "high");
    }

    #[test]
    fn test_extract_ecosystem_from_alert_id_variants() {
        assert_eq!(
            extract_ecosystem_from_alert_id("osv-pkg-axios-npm"),
            Some("npm".to_string())
        );
        assert_eq!(
            extract_ecosystem_from_alert_id("osv-pkg-json-web-token-npm"),
            Some("npm".to_string()),
            "package names with hyphens: ecosystem is last segment"
        );
        assert_eq!(
            extract_ecosystem_from_alert_id("osv-pkg-serde-crates.io"),
            Some("crates.io".to_string())
        );
        assert_eq!(
            extract_ecosystem_from_alert_id("signal-chain-123"),
            None,
            "non-osv alert ids return None"
        );
        assert_eq!(
            extract_ecosystem_from_alert_id("osv-pkg-"),
            None,
            "trailing prefix with no content returns None"
        );
    }

    #[test]
    fn test_advisory_id_extraction() {
        use crate::preemption::AlertEvidence;
        // Extract from evidence titles
        let evidence = vec![
            AlertEvidence {
                source: "osv".into(),
                title: "GHSA-wf5p-g6vw-rhxx: Axios SSRF vulnerability".into(),
                url: None,
                freshness_days: 1.0,
                relevance_score: 1.0,
            },
            AlertEvidence {
                source: "osv".into(),
                title: "CVE-2024-39338: Server-Side Request Forgery in Axios".into(),
                url: None,
                freshness_days: 2.0,
                relevance_score: 1.0,
            },
        ];
        let ids = extract_advisory_ids_from_evidence(&evidence);
        assert!(ids.contains(&"GHSA-wf5p-g6vw-rhxx".to_string()));
        assert!(ids.contains(&"CVE-2024-39338".to_string()));
        assert_eq!(ids.len(), 2, "no duplicates");

        // Empty evidence
        let empty_ids = extract_advisory_ids_from_evidence(&[]);
        assert!(empty_ids.is_empty());
    }

    // ══════════════════════════════════════════════════════════════════════
    // T3-3: Morning Brief Edge Case Tests
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_section_split_security_to_action() {
        // security_advisory and cve content types must land in the "action" section
        let items = vec![
            make_item_with_type("GHSA in serde", "github", 0.80, Some("security_advisory")),
            make_item_with_type("CVE-2026-9999 in tokio", "osv", 0.75, Some("cve")),
        ];

        let result = apply_section_split(items);

        assert_eq!(result.len(), 2);
        for item in &result {
            assert_eq!(
                item.section.as_deref(),
                Some("action"),
                "item '{}' with content_type {:?} must be in action section",
                item.title,
                item.content_type
            );
        }
    }

    #[test]
    fn test_persistent_security_alert_exempts_critical_and_high() {
        // Critical/High advisories must bypass the 14-day novelty filter so an
        // unfixed high-severity vulnerability re-surfaces every morning. Lower
        // severities still obey novelty.
        let alert = |urgency: &str| BriefingPreemptionAlert {
            urgency: urgency.to_string(),
            title: "axios has known vulnerabilities".to_string(),
            ..Default::default()
        };
        assert!(is_persistent_security_alert(&alert("critical")));
        assert!(is_persistent_security_alert(&alert("high")));
        assert!(!is_persistent_security_alert(&alert("medium")));
        assert!(!is_persistent_security_alert(&alert("watch")));
        assert!(!is_persistent_security_alert(&alert("low")));
        assert!(!is_persistent_security_alert(&alert("")));
    }

    #[test]
    fn test_section_split_tutorials_to_reading() {
        // tutorial, blog_post, discussion are NOT actionable -- they go to
        // "reading" (when uncorroborated or low-score) not "action"
        let items = vec![
            make_item_with_type("Learn async Rust", "reddit", 0.55, Some("tutorial")),
            make_item_with_type("Why I love Tauri", "hackernews", 0.52, Some("blog_post")),
            make_item_with_type("Is Rust worth it?", "reddit", 0.50, Some("discussion")),
        ];

        let result = apply_section_split(items);

        assert_eq!(result.len(), 3);
        for item in &result {
            assert_eq!(
                item.section.as_deref(),
                Some("reading"),
                "item '{}' with content_type {:?} must be in reading section",
                item.title,
                item.content_type
            );
        }
    }

    #[test]
    fn test_section_split_corroborated_to_watch() {
        // A high-score (>= 0.6) non-actionable item WITH corroboration goes to "watch"
        let mut item = make_item_with_type(
            "Rust 2027 edition discussion",
            "reddit",
            0.75,
            Some("blog_post"),
        );
        item.corroboration_count = 3;

        let result = apply_section_split(vec![item]);

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].section.as_deref(),
            Some("watch"),
            "corroborated high-score non-actionable item must be in watch section"
        );
    }

    #[test]
    fn test_null_content_type_never_hero() {
        // NULL content_type must NEVER end up in "action" regardless of score
        let items = vec![
            make_item_with_type("Unknown high-score item", "rss", 0.95, None),
            make_item_with_type("Another mystery item", "hackernews", 0.88, None),
        ];

        let result = apply_section_split(items);

        for item in &result {
            assert_ne!(
                item.section.as_deref(),
                Some("action"),
                "NULL content_type '{}' must never be in action section",
                item.title
            );
        }
    }

    #[test]
    fn test_section_caps_enforced() {
        // Create 10 actionable items -- only 5 should survive the cap
        let mut items: Vec<BriefingItem> = (0..10)
            .map(|i| {
                make_item_with_type(
                    &format!("Security advisory {i}"),
                    "github",
                    0.90 - (i as f32 * 0.01),
                    Some("security_advisory"),
                )
            })
            .collect();

        // Also add 8 reading items to test that cap
        for i in 0..8 {
            items.push(make_item_with_type(
                &format!("Tutorial {i}"),
                "reddit",
                0.50 - (i as f32 * 0.01),
                Some("tutorial"),
            ));
        }

        // And 7 watch items (high score + corroboration)
        for i in 0..7 {
            let mut watch = make_item_with_type(
                &format!("Hot topic {i}"),
                "hackernews",
                0.70,
                Some("blog_post"),
            );
            watch.corroboration_count = 2;
            items.push(watch);
        }

        let result = apply_section_split(items);

        let action_count = result
            .iter()
            .filter(|i| i.section.as_deref() == Some("action"))
            .count();
        let watch_count = result
            .iter()
            .filter(|i| i.section.as_deref() == Some("watch"))
            .count();
        let reading_count = result
            .iter()
            .filter(|i| i.section.as_deref() == Some("reading"))
            .count();

        assert_eq!(action_count, 5, "action section capped at 5");
        assert_eq!(watch_count, 5, "watch section capped at 5");
        assert_eq!(reading_count, 5, "reading section capped at 5");
        assert_eq!(result.len(), 15, "total = 5 + 5 + 5");
    }
}
