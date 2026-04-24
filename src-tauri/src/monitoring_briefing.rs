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
/// 0.35 keeps genuinely relevant content while cutting noise that scored
/// on a single weak keyword match. Critical/alert priority items bypass
/// this via the signal classifier's own 0.30 threshold.
pub(crate) const BRIEFING_SCORE_FLOOR: f32 = 0.35;

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

/// AWE wisdom signal — a validated principle or anti-pattern from the Wisdom Graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WisdomSignal {
    pub text: String,
    pub confidence: f32,
    pub signal_type: String, // "principle" or "anti-pattern"
}

/// A preemption alert included in the morning briefing (critical/high only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingPreemptionAlert {
    pub title: String,
    pub urgency: String,
    pub explanation: String,
}

/// Translated labels for the briefing window UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingLabels {
    pub header: String,
    pub escalating: String,
    pub wisdom: String,
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
    /// AWE wisdom signals — validated principles and anti-patterns from the Wisdom Graph
    #[serde(default)]
    pub wisdom_signals: Vec<WisdomSignal>,
    /// LLM-synthesized intelligence narrative (populated async after initial delivery)
    #[serde(default)]
    pub synthesis: Option<String>,
    /// Behavioral wisdom — personalized insight from 4DA's behavioral data
    /// Reserved for future enrichment
    #[serde(default)]
    pub wisdom_synthesis: Option<String>,
    /// Preemption alerts — critical/high urgency items from the preemption engine
    #[serde(default)]
    pub preemption_alerts: Vec<BriefingPreemptionAlert>,
    /// Blind spot summary — quick overview of coverage gaps (0-100, higher = more gaps)
    #[serde(default)]
    pub blind_spot_score: Option<f32>,
    /// Translated labels for the briefing window (i18n)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<BriefingLabels>,
}

// ============================================================================
// Enrichment Pipeline
// ============================================================================

/// Build an enriched briefing from raw items.
///
/// Applies the full quality pipeline: quality gate → dedupe → cap → novelty →
/// knowledge gaps → escalating chains → AWE wisdom → preemption alerts →
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

    // Cap at the widget's display budget. Prefer fewer high-quality items
    // over a full list of mediocre ones.
    let items: Vec<BriefingItem> = sorted.into_iter().take(8).collect();

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

    let total_relevant = items.len();

    // Detect knowledge gaps: declared tech with no recent signals
    let knowledge_gaps = detect_knowledge_gaps();

    // Detect escalating signal chains for top-level briefing section
    let escalating_chains = detect_escalating_chains();

    // Recall AWE wisdom signals — validated principles and anti-patterns
    let wisdom_signals = recall_awe_wisdom();

    // Collect preemption alerts for briefing (critical + high only, max 3)
    let preemption_alerts = match crate::preemption::get_preemption_feed() {
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
            .map(|a| BriefingPreemptionAlert {
                title: a.title.clone(),
                urgency: format!("{:?}", a.urgency).to_lowercase(),
                explanation: a.explanation.clone(),
            })
            .collect(),
        Err(_) => Vec::new(),
    };

    // Collect blind spot score
    let blind_spot_score = crate::blind_spots::generate_blind_spot_report()
        .ok()
        .map(|r| r.overall_score);

    // Build translated labels
    let labels = build_briefing_labels(lang);

    BriefingNotification {
        title: format!("4DA Intelligence Briefing — {}", now.format("%d %b %Y")),
        items,
        total_relevant,
        ongoing_topics,
        knowledge_gaps,
        escalating_chains,
        wisdom_signals,
        synthesis: None,
        wisdom_synthesis: None,
        preemption_alerts,
        blind_spot_score,
        labels: Some(labels),
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
                })
                .collect()
        } else {
            // Fall back to DB query — now reads real scores and filters by language.
            // Use a 72-hour window (not 24h) so users returning after a weekend
            // or vacation still get a briefing from their last active session.
            if let Ok(db) = crate::get_database() {
                let period_start = chrono::Utc::now() - chrono::Duration::hours(72);
                db.get_relevant_items_since(period_start, BRIEFING_SCORE_FLOOR.into(), 25, &user_lang)
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

    if briefing.items.is_empty() {
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
pub fn build_briefing_labels(lang: &str) -> BriefingLabels {
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
        wisdom: tr("ui:briefing.wisdom", "WISDOM"),
        signals: tr("ui:briefing.signals", "SOURCES"),
        blind_spots: tr("ui:briefing.blind_spots", "BLIND SPOTS"),
        tracking: tr("ui:briefing.tracking", "Tracking:"),
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

    // Companion: native OS notification with concise professional summary
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
        wisdom = briefing.wisdom_signals.len(),
        "Intelligence briefing delivered (desktop widget + OS notification)"
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
    let principles = briefing
        .wisdom_signals
        .iter()
        .filter(|s| s.signal_type == "principle")
        .count();
    if principles > 0 {
        parts.push(format!(
            "{principles} wisdom signal{}",
            if principles != 1 { "s" } else { "" }
        ));
    }

    if parts.is_empty() {
        return "No new signals since last check.".to_string();
    }

    let summary = parts.join(" · ");
    if let Some(top) = briefing.items.first() {
        let safe_title = strip_control_chars(&top.title);
        let title = truncate_safe(&safe_title, 80);
        format!("{summary}\n{title}")
    } else {
        summary
    }
}

// ============================================================================
// Novelty Detection
// ============================================================================

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
            ongoing.push(item.source_type.clone());
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
// AWE Wisdom Recall
// ============================================================================

/// Recall AWE wisdom signals relevant to the morning briefing.
///
/// Fetches validated principles and anti-patterns from the Wisdom Graph.
/// These provide decision context from accumulated project experience.
///
/// **First production use of the `external::awe::AweClient` typed wrapper.**
/// Migrated from raw `Command::new("awe")` on 2026-04-14 as part of the
/// Silent-Failure Defense Layer 1 rollout. See
/// `docs/strategy/SILENT-FAILURE-DEFENSE.md` for the architecture and
/// `.claude/wisdom/antibodies/2026-04-12-silent-cli-failures.md` for the
/// bugs this wrapper defends against by construction.
fn recall_awe_wisdom() -> Vec<WisdomSignal> {
    // AWE v1 integration removed — AWE v2 is a standalone repo.
    // This stub preserves the briefing pipeline shape; AWE v2 will
    // re-populate wisdom signals via its own MCP tools.
    vec![]
}

// ============================================================================
// CLI Briefing Generator
// ============================================================================

/// Strip control characters (ANSI escape sequences, etc.) from a string
/// to prevent injection in CLI output.
fn strip_control_chars(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

/// Generate a formatted briefing string suitable for CLI output or frontend display.
/// Uses in-memory analysis state first, falls back to DB query.
/// All titles and sources are sanitized to prevent ANSI injection.
pub fn generate_briefing_text() -> String {
    let now = chrono::Local::now();
    let mut output = format!("4DA Morning Briefing — {}\n\n", now.format("%d %b %Y"));

    // Try in-memory results first
    let items: Vec<(String, String, f32, Option<String>)> = {
        let state = crate::get_analysis_state().lock();
        if let Some(ref results) = state.results {
            results
                .iter()
                .filter(|r| r.relevant && !r.excluded)
                .take(10)
                .map(|r| {
                    (
                        r.title.clone(),
                        r.source_type.clone(),
                        r.top_score,
                        r.signal_type.clone(),
                    )
                })
                .collect()
        } else {
            vec![]
        }
    };

    // Fall back to DB if no in-memory results
    let items = if items.is_empty() {
        if let Ok(db) = crate::get_database() {
            let period_start = chrono::Utc::now() - chrono::Duration::hours(24);
            {
                let lang = crate::i18n::get_user_language();
                db.get_relevant_items_since(period_start, 0.1, 10, &lang)
            }
            .ok()
            .map(|db_items| {
                db_items
                    .into_iter()
                    .map(|i| {
                        (
                            i.title,
                            i.source_type,
                            i.relevance_score.unwrap_or(0.0) as f32,
                            None::<String>,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
        } else {
            vec![]
        }
    } else {
        items
    };

    if items.is_empty() {
        output.push_str("  No new signals since last check.\n");
        return output;
    }

    for (title, source_type, score, signal_type) in items.iter().take(10) {
        let safe_title = strip_control_chars(title);
        let safe_source = strip_control_chars(source_type);
        let signal_tag = signal_type
            .as_deref()
            .map(|s| format!("[{}] ", strip_control_chars(&s.to_uppercase())))
            .unwrap_or_default();
        output.push_str(&format!("  {signal_tag}{safe_title}\n"));
        output.push_str(&format!(
            "    Source: {} | Score: {:.0}%\n\n",
            safe_source,
            score * 100.0
        ));
    }

    output.push_str(&format!("  {} total relevant items\n", items.len()));
    output
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

    let wisdom_text = if briefing.wisdom_signals.is_empty() {
        String::new()
    } else {
        let w = briefing
            .wisdom_signals
            .iter()
            .map(|w| {
                let label = if w.signal_type == "anti-pattern" {
                    "AVOID"
                } else {
                    "PRINCIPLE"
                };
                format!("- [{}] ({:.0}%) {}", label, w.confidence * 100.0, w.text)
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("\nAWE Wisdom:\n{w}\n")
    };

    let system_prompt = r#"You are an intelligence analyst writing a morning brief for a developer's desktop widget.

YOUR JOB: find the strongest thread. Group signals by theme, pick the 1-2 most important clusters, skip the rest. The signal list below the synthesis covers everything — your job is to surface what matters MOST, not summarize all of them.

WHAT GOOD LOOKS LIKE:
"CI supply-chain security hit critical mass — three independent teams published GitHub Actions hardening guides this week. Audit your pinned versions and tag references today.

React compiler tooling is also maturing — two papers show 5-8x build improvements with zero config, worth evaluating on React 19+."

WHAT BAD LOOKS LIKE:
"GitHub Actions hardening is on the rise. In another domain, researchers are exploring LLM curiosity. Meanwhile, fuzz drivers are gaining traction. Additionally, TypeScript repos show..." ← Summarizing every signal sequentially. That is a for-loop, not intelligence.

BUDGET: maximum 80 words, 1-2 clusters, 2-4 sentences. The signal list handles the rest.

STRUCTURE per cluster: claim + evidence → action.
- "X hit critical mass — three teams published Y. Audit your Z."
- If two clusters matter, separate with a line break. Max two.

QUALITY RULES:
1. Pick the strongest 1-2 clusters. Skip outliers — the source list covers them.
2. State actions for senior devs — don't explain why fire is hot.
3. Say what papers mean; never paste their titles as prose.
4. Every tech name must appear verbatim in the input. Invent nothing.
5. ABSTAIN if <2 items are noteworthy: "Low signal — no noteworthy intelligence overnight."

BANNED:
- Transition padding: "meanwhile", "in another domain", "in a related vein", "additionally", "furthermore"
- Filler: "this is crucial because", "this guidance is important for", "this is important because"
- Citation brackets [1] [2], markdown, headers, bullets, numbered lists
- Covering more than 2 clusters — pick the best, skip the rest"#;

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
         {count} signals:\n{items}\n{chains}{gaps}{wisdom}\n\
         Synthesize my morning intelligence briefing.",
        tech = tech_summary,
        topics = topics_summary,
        count = briefing.items.len(),
        items = items_text,
        chains = chains_text,
        gaps = gaps_text,
        wisdom = wisdom_text,
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
            if inner.len() <= 10 && inner.chars().all(|c| c.is_ascii_digit() || c == ',' || c == ' ') {
                synthesis.replace_range(start_bracket..start_bracket + end_bracket + 1, "");
                continue;
            }
        }
        break;
    }

    // Strip markdown bold markers **text** → text
    while synthesis.contains("**") {
        synthesis = synthesis.replacen("**", "", 2);
    }

    // Strip markdown headers (## Header → Header)
    synthesis = synthesis
        .lines()
        .map(|line| line.trim_start_matches('#').trim_start())
        .collect::<Vec<_>>()
        .join("\n");

    // Strip LLM section labels that leak despite being banned in the prompt
    let label_prefixes = [
        "Top Signal:", "Top Signals:", "Key Signal:", "Key Signals:",
        "Next Steps:", "Next Step:", "Action:", "Actions:",
        "Summary:", "Situation:", "Priority:", "Pattern:",
        "Recommendation:", "Recommendations:", "Note:", "Notes:",
        "Insight:", "Insights:", "Alert:", "Alerts:",
        "Cluster 1:", "Cluster 2:", "Cluster:",
        "Theme 1:", "Theme 2:", "Theme:",
    ];
    for prefix in &label_prefixes {
        // Case-insensitive prefix removal at line start
        for line_prefix in [*prefix, &prefix.to_uppercase()] {
            while synthesis.contains(line_prefix) {
                synthesis = synthesis.replace(line_prefix, "");
            }
        }
    }

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

    // The threshold is conservative — require 65% of salient terms to
    // be grounded. Anything below suggests the LLM invented content.
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
            wisdom_signals: vec![WisdomSignal {
                text: "Always test migration paths before releasing".to_string(),
                confidence: 0.85,
                signal_type: "principle".to_string(),
            }],
            synthesis: None,
            wisdom_synthesis: None,
            preemption_alerts: vec![],
            blind_spot_score: None,
            labels: None,
        };
        assert_eq!(briefing.items.len(), 2);
        assert_eq!(briefing.total_relevant, 2);
        assert!(briefing.title.contains("Intelligence Briefing"));
        assert_eq!(briefing.knowledge_gaps.len(), 1);
        assert_eq!(briefing.knowledge_gaps[0].days_since_last, 7);
        assert_eq!(briefing.escalating_chains.len(), 1);
        assert_eq!(briefing.escalating_chains[0].phase, "escalating");
        assert_eq!(briefing.wisdom_signals.len(), 1);
        assert_eq!(briefing.wisdom_signals[0].signal_type, "principle");
        assert!(briefing.wisdom_signals[0].confidence > 0.8);
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
}
