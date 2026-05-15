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

/// A preemption alert included in the morning briefing (critical/high only).
///
/// Carries the rich evidence fields from the backend `PreemptionAlert` so the
/// frontend can render actionable checklist cards (package, versions, projects,
/// advisory IDs) instead of opaque title/explanation blobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    // Priority-aware sort: critical/alert first, then by score descending.
    // The briefing is a curated surface — high-priority items MUST lead.
    let mut sorted = deduped;
    sorted.sort_by(|a, b| {
        let pa = priority_rank(a.signal_priority.as_deref());
        let pb = priority_rank(b.signal_priority.as_deref());
        pa.cmp(&pb).then_with(|| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

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
            Ok(feed) => feed
                .alerts
                .iter()
                .filter(|a| {
                    matches!(
                        a.urgency,
                        crate::preemption::AlertUrgency::Critical
                            | crate::preemption::AlertUrgency::High
                    )
                })
                .take(3)
                .map(|a| BriefingPreemptionAlert::from_preemption_alert(a))
                .collect(),
            Err(_) => Vec::new(),
        };

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
        };
    }

    // Novelty detection: filter items seen in last 3 days, track ongoing topics.
    // If novelty filter removes ALL items, keep the top 3 as "still relevant"
    // rather than showing an empty briefing — a repeat signal beats "nothing new."
    let today = now.format("%Y-%m-%d").to_string();
    let (items, ongoing_topics) = if skip_novelty {
        (items, vec![])
    } else {
        let pre_filter = items.clone();
        let (novel, ongoing) = apply_novelty_filter(items, &today);
        if novel.is_empty() && !pre_filter.is_empty() {
            // All items were seen recently — keep top 3 so the briefing has content
            let fallback: Vec<BriefingItem> = pre_filter.into_iter().take(3).collect();
            (fallback, ongoing)
        } else {
            (novel, ongoing)
        }
    };

    // Topic clustering for corroboration detection.
    // Load embeddings from the DB for the briefing items, then cluster by
    // cosine similarity. Multi-source clusters get a corroboration bonus
    // and alt_sources metadata for the UI badge.
    let mut items = items;
    apply_topic_clustering(&mut items);

    // Split items into action-first sections.
    // Actions (security, breaking changes, etc.) lead the briefing.
    // Watch items are non-actionable but corroborated and high-scoring.
    // Reading items are everything else — informational, lower confidence.
    let items = apply_section_split(items);

    let total_relevant = items.len();

    // Detect knowledge gaps: declared tech with no recent signals
    let knowledge_gaps = detect_knowledge_gaps();

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

    BriefingNotification {
        title: format!("4DA Intelligence Briefing — {}", now.format("%d %b %Y")),
        items,
        total_relevant,
        ongoing_topics,
        knowledge_gaps,
        escalating_chains,
        synthesis: None,
        preemption_alerts,
        blind_spot_score,
        labels: Some(labels),
        personalization_context,
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
fn apply_topic_clustering(items: &mut Vec<BriefingItem>) {
    // Collect item IDs for DB lookup
    let ids: Vec<i64> = items.iter().filter_map(|item| item.item_id).collect();

    if ids.is_empty() {
        return;
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
                return;
            }
        },
        Err(_) => return,
    };

    if embedding_data.is_empty() {
        return;
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
        return;
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
            i
        })
        .collect();

    let mut watch_items: Vec<BriefingItem> = items
        .iter()
        .filter(|i| {
            !is_actionable_content_type(i.content_type.as_deref())
                && i.score >= 0.6
                && i.corroboration_count > 0
        })
        .cloned()
        .map(|mut i| {
            i.section = Some("watch".into());
            i
        })
        .collect();

    let mut reading_items: Vec<BriefingItem> = items
        .iter()
        .filter(|i| {
            !is_actionable_content_type(i.content_type.as_deref())
                && (i.score < 0.6 || i.corroboration_count == 0)
        })
        .cloned()
        .map(|mut i| {
            i.section = Some("reading".into());
            i
        })
        .collect();

    // Cap each section
    action_items.truncate(5);
    watch_items.truncate(5);
    reading_items.truncate(5);

    // Combine: actions first, then watch, then reading
    let mut final_items = action_items;
    final_items.extend(watch_items);
    final_items.extend(reading_items);
    final_items
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
                    matched_deps: r.signal_triggers.clone().unwrap_or_default(),
                    content_type: r
                        .score_breakdown
                        .as_ref()
                        .and_then(|b| b.content_type.clone()),
                    corroboration_count: 0,
                    alt_sources: vec![],
                    section: None,
                })
                .collect()
        } else {
            // Fall back to DB query — now reads real scores and filters by language.
            // Use a 72-hour window (not 24h) so users returning after a weekend
            // or vacation still get a briefing from their last active session.
            if let Ok(db) = crate::get_database() {
                let period_start = chrono::Utc::now() - chrono::Duration::hours(72);
                db.get_relevant_items_since(
                    period_start,
                    BRIEFING_SCORE_FLOOR.into(),
                    25,
                    &user_lang,
                )
                .ok()
                .map(|db_items| {
                    db_items
                        .into_iter()
                        .map(|i| BriefingItem {
                            title: i.title,
                            source_type: i.source_type,
                            score: i.relevance_score.unwrap_or(0.0) as f32,
                            signal_type: None,
                            url: i.url,
                            item_id: Some(i.id),
                            signal_priority: None,
                            description: None,
                            matched_deps: vec![],
                            content_type: None,
                            corroboration_count: 0,
                            alt_sources: vec![],
                            section: None,
                        })
                        .collect()
                })
                .unwrap_or_default()
            } else {
                vec![]
            }
        }
    };

    // Apply quality gate + dedupe + enrichment via shared pipeline
    let briefing = build_enriched_briefing(raw_items, &user_lang, false);

    if !briefing.has_meaningful_content() {
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
        "first", "last", "week", "today", "part",
    ];
    title
        .split_whitespace()
        .find(|w| {
            w.len() >= 3
                && w.chars().next().is_some_and(|c| c.is_uppercase())
                && !STOP_WORDS.contains(&w.to_lowercase().as_str())
        })
        .unwrap_or("")
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

    // Get titles shown in the last 3 days
    let recent_titles: std::collections::HashSet<String> = {
        let mut stmt = match conn.prepare(
            "SELECT LOWER(item_title) FROM briefing_item_history
             WHERE briefing_date >= date(?1, '-3 days')",
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
// Knowledge Gap Detection
// ============================================================================

/// Detect knowledge gaps: declared tech topics with no recent source items.
/// Returns topics the user cares about but hasn't seen intelligence on in 5+ days.
fn detect_knowledge_gaps() -> Vec<KnowledgeGap> {
    let declared_tech: Vec<String> = {
        match crate::get_context_engine() {
            Ok(engine) => {
                if let Ok(identity) = engine.get_static_identity() {
                    identity.tech_stack.clone()
                } else {
                    return vec![];
                }
            }
            Err(_) => return vec![],
        }
    };

    if declared_tech.is_empty() {
        return vec![];
    }

    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut gaps = Vec::new();

    for tech in &declared_tech {
        let tech_lower = tech.to_lowercase();
        // Find the most recent item mentioning this tech
        let last_seen: Option<i64> = conn
            .query_row(
                "SELECT CAST(julianday('now') - julianday(MAX(created_at)) AS INTEGER)
                 FROM source_items
                 WHERE LOWER(title) LIKE ?1",
                rusqlite::params![format!("%{}%", tech_lower)],
                |row| row.get(0),
            )
            .ok();

        if let Some(days) = last_seen {
            if days >= 5 {
                gaps.push(KnowledgeGap {
                    topic: tech.clone(),
                    days_since_last: days,
                });
            }
        }
    }

    // Sort by stalest first
    gaps.sort_by(|a, b| b.days_since_last.cmp(&a.days_since_last));
    gaps.truncate(5); // Max 5 gaps in briefing
    gaps
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

/// Synthesize a narrative morning intelligence briefing using LLM.
pub(crate) async fn synthesize_morning_briefing(
    briefing: &BriefingNotification,
) -> std::result::Result<String, String> {
    let llm_settings = {
        let settings = crate::get_settings_manager().lock();
        settings.get().llm.clone()
    };

    if llm_settings.provider != "ollama" && llm_settings.api_key.is_empty() {
        return Err("No LLM configured".into());
    }

    if !crate::ollama::can_synthesize(&llm_settings).await {
        return Err(
            "Model below synthesis capability threshold (requires 7B+ local or cloud API)".into(),
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
                format!(" (affects: {})", item.matched_deps.join(", "))
            };
            format!("{}. {}{} — {}{}", i + 1, tag, item.title, desc, deps)
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

    let system_prompt = r#"You are an intelligence analyst writing a morning brief for a developer's desktop widget.

YOUR JOB: synthesize, don't summarize. Find the 1-2 strongest threads across all signals, explain WHY they matter together, and state what to do. The signal list below handles individual items -- your job is the "so what?" that a senior dev can't see by scanning titles.

PRIORITY ORDER for picking clusters:
1. Security vulnerabilities in the developer's actual dependencies (CVEs, RCEs)
2. Research or patterns that change how the developer should build (architecture shifts)
3. Major version releases of direct dependencies with breaking changes
4. Everything else -- skip unless it compounds with (1) or (2)

WHAT GOOD LOOKS LIKE:
"Tokio 1.38.x has a confirmed RCE via malformed HTTP/2 frames -- if you're on 1.38.0-1.38.5, upgrade to 1.38.6 today. Upstream says the HTTP/2 parser rewrite lands in 1.39, so this is a stopgap patch.

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

BANNED:
- Restating signal titles without adding context or connecting dots
- Speculative implications: "could impact", "might affect", "may influence" without evidence in the signals
- Transition padding: "meanwhile", "in another domain", "in a related vein", "additionally", "furthermore"
- Filler: "is crucial", "is important", "this guidance", "it is essential", "it is recommended"
- Pasting paper/article titles as prose sentences -- describe the finding, not the title
- Citation brackets [1] [2], markdown, headers, bullets, numbered lists
- Covering more than 2 clusters -- pick the best, skip the rest
- Horizontal rules (---), separators, or dividers between clusters -- use a blank line only
- Unicode em dashes or special characters -- use plain ASCII only
- Fabricated numbers/percentages/statistics not present in the signals"#;

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
        "Developer context:\nTech stack: {tech}\nWorking on: {topics}\n\n\
         {count} signals:\n{items}\n{chains}{gaps}\n\
         Synthesize my morning intelligence briefing.",
        tech = tech_summary,
        topics = topics_summary,
        count = briefing.items.len(),
        items = items_text,
        chains = chains_text,
        gaps = gaps_text,
    );

    let llm_client = crate::llm::LLMClient::new(llm_settings);
    let messages = vec![crate::llm::Message {
        role: "user".to_string(),
        content: user_prompt,
    }];

    let start = std::time::Instant::now();
    let response = match llm_client.complete(&full_system_prompt, messages).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(target: "4da::briefing", error = %e, "Morning brief synthesis failed");
            return Err(format!("LLM synthesis failed: {e}"));
        }
    };

    tracing::info!(
        target: "4da::briefing",
        tokens = response.input_tokens + response.output_tokens,
        elapsed_ms = start.elapsed().as_millis(),
        "Morning brief synthesis complete"
    );

    // --- Post-synthesis cleanup: strip citations and enforce word limit ------
    let mut synthesis = response.content.clone();

    // Strip citation brackets [1], [2][3], etc. — LLMs add them despite being banned
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

    // Strip markdown bold markers **text** → text
    while synthesis.contains("**") {
        synthesis = synthesis.replacen("**", "", 2);
    }

    // Normalize unicode dashes to plain ASCII (prevents encoding artifacts in display)
    synthesis = synthesis.replace('\u{2014}', "--"); // em dash
    synthesis = synthesis.replace('\u{2013}', "--"); // en dash
    synthesis = synthesis.replace('\u{2018}', "'"); // left single quote
    synthesis = synthesis.replace('\u{2019}', "'"); // right single quote
    synthesis = synthesis.replace('\u{201C}', "\""); // left double quote
    synthesis = synthesis.replace('\u{201D}', "\""); // right double quote

    // Strip markdown headers (## Header → Header)
    synthesis = synthesis
        .lines()
        .map(|line| line.trim_start_matches('#').trim_start())
        .collect::<Vec<_>>()
        .join("\n");

    // Strip LLM section labels that leak despite being banned in the prompt
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
    ];
    for prefix in &label_prefixes {
        // Case-insensitive prefix removal at line start
        for line_prefix in [*prefix, &prefix.to_uppercase()] {
            while synthesis.contains(line_prefix) {
                synthesis = synthesis.replace(line_prefix, "");
            }
        }
    }

    // Strip horizontal rules (---) that LLMs use as separators
    synthesis = synthesis
        .lines()
        .filter(|line| !line.trim().chars().all(|c| c == '-') || line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    // Collapse multiple spaces left by cleanup
    while synthesis.contains("  ") {
        synthesis = synthesis.replace("  ", " ");
    }

    // Hard word-limit safety net — caps at ~100 words (generous over 80-word prompt budget)
    let words: Vec<&str> = synthesis.split_whitespace().collect();
    if words.len() > 100 {
        tracing::info!(
            target: "4da::briefing",
            word_count = words.len(),
            "Synthesis exceeded 100 words — truncating"
        );
        let truncated = words[..100].join(" ");
        // Find last sentence boundary within the truncated text
        synthesis = if let Some(last_period) = truncated.rfind('.') {
            truncated[..=last_period].to_string()
        } else {
            format!("{truncated}.")
        };
    }

    // Replace the response content with cleaned version
    let response = crate::llm::LLMResponse {
        content: synthesis,
        ..response
    };

    // --- Post-synthesis groundedness check ---------------------------------
    // Even with strict prompting, LLMs sometimes hallucinate. This
    // validator extracts proper nouns / versions from the output and
    // verifies each appears in the source corpus. If the output is
    // significantly ungrounded, we fall back to a safe abstention.
    //
    // A real production hallucination this catches (Screenshot_1976):
    //   "Recommend update of your strategy for non-test architecture,
    //    including a 5+ year migration from VAR and Stripe"
    // Neither "VAR" nor "Stripe" appeared in any item that day.
    let corpus: Vec<String> = briefing
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

    let report = crate::briefing_groundedness::validate_groundedness(&response.content, &corpus);

    // The threshold is conservative — require at least 35% of salient terms
    // to be grounded. Below this, the LLM likely invented content wholesale.
    const GROUNDEDNESS_THRESHOLD: f32 = 0.35;

    if !report.is_acceptable(GROUNDEDNESS_THRESHOLD) {
        tracing::warn!(
            target: "4da::briefing",
            confidence = report.confidence,
            total_terms = report.total_terms,
            ungrounded_count = report.ungrounded_terms.len(),
            ungrounded_sample = ?report.ungrounded_terms.iter().take(5).collect::<Vec<_>>(),
            "Morning brief synthesis failed groundedness check — falling back to abstention"
        );
        return Ok(format!(
            "Low signal — no noteworthy intelligence overnight.\n\n\
             ({} items scanned, synthesis skipped: {} ungrounded terms detected)",
            briefing.items.len(),
            report.ungrounded_terms.len()
        ));
    }

    tracing::info!(
        target: "4da::briefing",
        confidence = report.confidence,
        total_terms = report.total_terms,
        "Groundedness check passed"
    );

    Ok(response.content)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

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

        // Remaining items should be in reading section (no corroboration = not watch)
        for item in &result[3..] {
            assert_eq!(item.section.as_deref(), Some("reading"));
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
        // High-scoring non-actionable item WITHOUT corroboration -> reading, not watch
        let mut high_score_no_corrob =
            make_item_with_type("Interesting post", "hackernews", 0.80, Some("blog_post"));
        high_score_no_corrob.corroboration_count = 0;

        // High-scoring non-actionable item WITH corroboration -> watch
        let mut high_score_corrob =
            make_item_with_type("Hot topic", "reddit", 0.75, Some("discussion"));
        high_score_corrob.corroboration_count = 2;

        let items = vec![high_score_no_corrob, high_score_corrob];
        let result = apply_section_split(items);

        let interesting = result
            .iter()
            .find(|i| i.title == "Interesting post")
            .unwrap();
        assert_eq!(
            interesting.section.as_deref(),
            Some("reading"),
            "uncorroborated high-score item goes to reading"
        );

        let hot = result.iter().find(|i| i.title == "Hot topic").unwrap();
        assert_eq!(
            hot.section.as_deref(),
            Some("watch"),
            "corroborated high-score item goes to watch"
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
        assert!(briefing.advisory_ids.contains(&"GHSA-wf5p-g6vw-rhxx".to_string()));
        assert!(briefing.advisory_ids.contains(&"CVE-2024-39338".to_string()));
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
}
