// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Natural Language Search — Intelligence Console query engine.
//!
//! Tiered search: free users get 3 results + ghost preview,
//! Signal users get full results + decision/gap cross-referencing.
//! Stack-aware boosting prioritises results matching the user's tech.

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::Result;

#[path = "natural_language_search_engine.rs"]
mod natural_language_search_engine;
pub(crate) use natural_language_search_engine::*;

#[path = "natural_language_search_timing.rs"]
mod natural_language_search_timing;
use natural_language_search_timing::{EmbedSource, SearchTimings, StageClock};

#[path = "natural_language_search_warm.rs"]
pub(crate) mod natural_language_search_warm;
// Glob so `#[tauri::command]`'s companion `__cmd__warm_search` comes along.
pub(crate) use natural_language_search_warm::*;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResultItem {
    pub id: i64,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub preview: String,
    pub relevance: f64,
    pub source_type: String,
    pub timestamp: Option<String>,
    pub match_reason: String,
    /// Title contains the query as a whole word/phrase; pinned above every other hit.
    #[serde(default)]
    pub exact_match: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedQuery {
    pub keywords: Vec<String>,
    pub entities: Vec<String>,
    pub time_range: Option<TimeRange>,
    pub file_types: Vec<String>,
    pub sentiment: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: String,
    pub end: String,
    pub relative: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackContextEntry {
    pub name: String,
    pub category: String,
    pub relevant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedDecision {
    pub id: i64,
    pub subject: String,
    pub decision: String,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryGap {
    pub technology: String,
    pub days_stale: u32,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostPreview {
    pub total_results: usize,
    pub hidden_results: usize,
    pub decision_count: usize,
    pub gap_count: usize,
    pub synthesis_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub query: String,
    pub intent: String,
    pub items: Vec<QueryResultItem>,
    pub total_count: usize,
    pub execution_ms: u64,
    pub summary: Option<String>,
    pub parsed: ParsedQuery,
    pub stack_context: Vec<StackContextEntry>,
    pub related_decisions: Vec<RelatedDecision>,
    pub knowledge_gaps: Vec<QueryGap>,
    pub ghost_preview: Option<GhostPreview>,
    pub is_pro: bool,
    /// The query embedder was not ready, so these results come from the keyword
    /// legs only. The embed finishes in the background; asking again shortly
    /// returns the full hybrid ranking.
    #[serde(default)]
    pub semantic_pending: bool,
}

// ============================================================================
// Stop words
// ============================================================================

pub(crate) const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
    "from", "is", "it", "that", "this", "was", "are", "be", "has", "have", "had", "do", "does",
    "did", "will", "would", "could", "should", "may", "might", "can", "shall", "not", "no", "so",
    "if", "then", "than", "when", "where", "what", "which", "who", "how", "all", "each", "every",
    "any", "some", "such", "only", "own", "same", "other", "into", "about", "up", "out", "just",
    "also", "very", "my", "me", "i", "we", "you", "your", "our", "they", "them", "their", "show",
    "find", "get", "give", "tell", "list", "display",
];

// ============================================================================
// Intent classification
// ============================================================================

fn classify_intent(query: &str) -> &'static str {
    let q = query.to_lowercase();
    if q.starts_with("summarize") || q.starts_with("summary") || q.contains("summarize") {
        "Summarize"
    } else if q.starts_with("compare") || q.contains("versus") || q.contains(" vs ") {
        "Compare"
    } else if q.starts_with("how many") || q.starts_with("count") || q.contains("how much") {
        "Count"
    } else if q.contains("timeline") || q.contains("history of") || q.contains("over time") {
        "Timeline"
    } else {
        "Find"
    }
}

// ============================================================================
// Time range detection
// ============================================================================

fn detect_time_range(query: &str) -> Option<TimeRange> {
    let q = query.to_lowercase();
    let now = Utc::now();

    let (duration, label) = if q.contains("today") {
        (Duration::hours(24), "today")
    } else if q.contains("yesterday") {
        (Duration::hours(48), "yesterday")
    } else if q.contains("last week") || q.contains("past week") || q.contains("this week") {
        (Duration::days(7), "last week")
    } else if q.contains("last month") || q.contains("past month") || q.contains("this month") {
        (Duration::days(30), "last month")
    } else if q.contains("last 3 months") || q.contains("past 3 months") {
        (Duration::days(90), "last 3 months")
    } else if q.contains("last year") || q.contains("past year") || q.contains("this year") {
        (Duration::days(365), "last year")
    } else if q.contains("recent") || q.contains("lately") {
        (Duration::days(14), "recently")
    } else {
        return None;
    };

    let start = now - duration;
    Some(TimeRange {
        start: start.format("%Y-%m-%d %H:%M:%S").to_string(),
        end: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        relative: Some(label.to_string()),
    })
}

// ============================================================================
// File type detection
// ============================================================================

fn detect_file_types(query: &str) -> Vec<String> {
    let q = query.to_lowercase();
    let mut types = Vec::new();
    if q.contains("pdf") {
        types.push("pdf".into());
    }
    if q.contains("doc") || q.contains("word") {
        types.push("docx".into());
    }
    if q.contains("spreadsheet") || q.contains("excel") || q.contains("xlsx") || q.contains("csv") {
        types.push("xlsx".into());
    }
    if q.contains("image") || q.contains("photo") || q.contains("screenshot") {
        types.push("image".into());
    }
    types
}

// ============================================================================
// Keyword extraction (pub(crate) so standing_queries can reuse)
// ============================================================================

pub(crate) fn extract_keywords(query: &str) -> Vec<String> {
    let stop_set: std::collections::HashSet<&str> = STOP_WORDS.iter().copied().collect();
    query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| w.len() > 2 && !stop_set.contains(w))
        .map(std::string::ToString::to_string)
        .collect()
}

// ============================================================================
// Local query parsing
// ============================================================================

fn parse_query_local(query: &str) -> ParsedQuery {
    let keywords = extract_keywords(query);
    let time_range = detect_time_range(query);
    let file_types = detect_file_types(query);
    let intent_keywords: Vec<&str> = vec![
        "summarize",
        "compare",
        "count",
        "timeline",
        "find",
        "show",
        "list",
    ];
    let entities: Vec<String> = keywords
        .iter()
        .filter(|k| !intent_keywords.contains(&k.as_str()))
        .cloned()
        .collect();

    let confidence = if keywords.is_empty() {
        0.3
    } else if time_range.is_some() || !file_types.is_empty() {
        0.85
    } else {
        0.65
    };

    ParsedQuery {
        keywords,
        entities,
        time_range,
        file_types,
        sentiment: None,
        confidence,
    }
}

// ============================================================================
// Stack context — detected technologies relevant to query
// ============================================================================

fn build_stack_context(conn: &rusqlite::Connection, keywords: &[String]) -> Vec<StackContextEntry> {
    let sql =
        "SELECT name, category FROM detected_tech WHERE confidence >= 0.5 ORDER BY confidence DESC";
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let rows = match stmt.query_map([], |row| {
        let name: String = row.get(0)?;
        let category: String = row.get(1)?;
        Ok((name, category))
    }) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut entries = Vec::new();
    for row in rows.flatten() {
        let (name, category) = row;
        let name_lower = name.to_lowercase();
        let relevant = keywords
            .iter()
            .any(|k| name_lower.contains(k.as_str()) || k.contains(name_lower.as_str()));
        entries.push(StackContextEntry {
            name,
            category,
            relevant,
        });
    }
    entries
}

// ============================================================================
// Stack-aware relevance boosting
// ============================================================================

const NON_EXACT_RELEVANCE_CAP: f64 = 0.99;

fn boost_for_stack(items: &mut [QueryResultItem], stack: &[StackContextEntry]) {
    let stack_names: Vec<String> = stack.iter().map(|s| s.name.to_lowercase()).collect();
    if stack_names.is_empty() {
        return;
    }
    // Exact-title hits already sit at the top of the scale; a stack boost never
    // lifts any other hit to (or past) an exact-title score.
    for item in items.iter_mut().filter(|i| !i.exact_match) {
        let title_lower = item.file_name.as_deref().unwrap_or("").to_lowercase();
        let preview_lower = item.preview.to_lowercase();
        let stack_matches: usize = stack_names
            .iter()
            .filter(|s| title_lower.contains(s.as_str()) || preview_lower.contains(s.as_str()))
            .count();
        if stack_matches > 0 {
            item.relevance += 0.15 * (stack_matches as f64).min(3.0);
            item.relevance = item.relevance.min(NON_EXACT_RELEVANCE_CAP);
            if !item.match_reason.contains("stack") {
                item.match_reason = format!("{} + stack match", item.match_reason);
            }
        }
    }
}

// ============================================================================
// Decision cross-referencing
// ============================================================================

fn find_related_decisions(
    conn: &rusqlite::Connection,
    keywords: &[String],
) -> Vec<RelatedDecision> {
    let decisions = match crate::decisions::list_decisions(conn, None, None, 50) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut related = Vec::new();
    for dec in &decisions {
        if dec.status == crate::decisions::DecisionStatus::Superseded {
            continue;
        }
        let subject_lower = dec.subject.to_lowercase();
        let tags_lower: Vec<String> = dec.context_tags.iter().map(|t| t.to_lowercase()).collect();

        let matches = keywords.iter().any(|k| {
            subject_lower.contains(k.as_str()) || tags_lower.iter().any(|t| t.contains(k.as_str()))
        });

        if matches {
            related.push(RelatedDecision {
                id: dec.id,
                subject: dec.subject.clone(),
                decision: dec.decision.clone(),
                relation: "related".to_string(),
            });
        }
    }
    related.truncate(5);
    related
}

// ============================================================================
// LLM availability check
// ============================================================================

fn is_llm_configured() -> bool {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let llm = &guard.get().llm;
    !llm.provider.is_empty() && llm.provider != "none"
}

// ============================================================================
// Tauri Command — Signal-gated
// ============================================================================

const FREE_RESULT_LIMIT: usize = 3;

/// Text for the keyword (BM25 / exact-title) legs. Non-English queries are
/// translated for keyword matching; vector embeddings handle cross-lingual
/// matching natively.
async fn keyword_search_text(query_text: &str, query_lang: &str) -> String {
    if query_lang == "en" {
        return query_text.to_string();
    }
    let request = crate::content_translation::TranslationRequest {
        id: "search_query".to_string(),
        text: query_text.to_string(),
        source_lang: query_lang.to_string(),
    };
    let result = crate::content_translation::translate_content(&request, "en").await;
    if result.provider == "none" {
        return query_text.to_string();
    }
    debug!(
        target: "4da::search",
        original = %query_text,
        translated = %result.translated,
        "Translated search query for BM25 keyword matching"
    );
    result.translated
}

/// Ghost preview for free users: what Signal would add to this search.
fn build_ghost_preview(
    is_pro: bool,
    total_count: usize,
    decision_count: usize,
) -> Option<GhostPreview> {
    if is_pro {
        return None;
    }
    let synthesis_available = is_llm_configured();
    let hidden_results = total_count.saturating_sub(FREE_RESULT_LIMIT);
    if hidden_results == 0 && decision_count == 0 && !synthesis_available {
        return None;
    }
    Some(GhostPreview {
        total_results: total_count,
        hidden_results,
        decision_count,
        gap_count: 0,
        synthesis_available,
    })
}

/// Header search box entry point.
///
/// Knowledge gaps are deliberately NOT computed here: gap detection cost 25,669 ms
/// of a 30,456 ms "rusqlite" search (measured 2026-10-07) only to attach at most
/// five gap chips. `knowledge_gaps` stays in the response shape and is empty.
/// All SQLite work runs on the blocking pool, never on a tokio worker.
#[tauri::command]
pub async fn natural_language_query(query_text: String) -> Result<QueryResult> {
    let query_text = query_text.trim().to_string();
    if query_text.is_empty() {
        return Err("Query cannot be empty".into());
    }
    if query_text.len() > 5000 {
        return Err("Query too long (maximum 5000 characters)".into());
    }

    crate::settings::require_signal_feature("natural_language_query")?;
    let is_pro = crate::settings::is_signal();
    run_search(query_text, is_pro)
        .await
        .map(|(result, _)| result)
}

/// The search itself, after validation and the entitlement gate, with the time
/// each stage took. Logged on the "Search completed" line.
pub(crate) async fn run_search(
    query_text: String,
    is_pro: bool,
) -> Result<(QueryResult, SearchTimings)> {
    let start = std::time::Instant::now();
    let mut timings = SearchTimings::default();
    let mut clock = StageClock::start();
    let query_lang = crate::language_detect::detect_language(&query_text);
    let parsed = parse_query_local(&query_text);
    let intent = classify_intent(&query_text).to_string();
    debug!(
        target: "4da::search",
        query = %query_text, intent = %intent, is_pro, lang = %query_lang,
        keywords = ?parsed.keywords,
        "Processing natural language query"
    );
    timings.parse_ms = clock.lap();

    let keywords = parsed.keywords.clone();
    let (stack_context, related) = run_blocking(move || {
        let conn = crate::open_db_connection()?;
        // Decisions serve both tiers: shown to Signal, counted in the free ghost preview.
        Ok((
            build_stack_context(&conn, &keywords),
            find_related_decisions(&conn, &keywords),
        ))
    })
    .await?;
    timings.context_ms = clock.lap();

    let search_text = keyword_search_text(&query_text, &query_lang).await;
    timings.translate_ms = clock.lap();
    let mut all_items = execute_hybrid_search(&search_text, &parsed, 30, &mut timings)
        .await
        .unwrap_or_default();
    clock.lap();
    boost_for_stack(&mut all_items, &stack_context);
    sort_items(&mut all_items);
    timings.rank_ms = clock.lap();

    let total_count = all_items.len();
    let ghost_preview = build_ghost_preview(is_pro, total_count, related.len());
    let related_decisions = if is_pro { related } else { Vec::new() };
    if !is_pro {
        all_items.truncate(FREE_RESULT_LIMIT);
    }

    let execution_ms = start.elapsed().as_millis() as u64;
    let h = &timings.hybrid;
    info!(
        target: "4da::search",
        query_lang = %query_lang, results = all_items.len(), total = total_count,
        ms = execution_ms,
        parse_ms = timings.parse_ms, context_ms = timings.context_ms,
        translate_ms = timings.translate_ms, embed_ms = timings.embed_ms,
        embed_load_ms = ?timings.embed_load_ms, embed = timings.embed_source.as_str(),
        ace_ms = timings.ace_ms, conn_wait_ms = h.conn_wait_ms, exact_ms = h.exact_ms,
        fts_ms = h.fts_ms, knn_ms = h.knn_ms, fuse_ms = h.fuse_ms, rank_ms = timings.rank_ms,
        "Search completed"
    );

    let semantic_pending = timings.embed_source == EmbedSource::TimedOut;
    let result = QueryResult {
        query: query_text,
        intent,
        items: all_items,
        total_count,
        execution_ms,
        summary: None,
        parsed,
        stack_context,
        related_decisions,
        knowledge_gaps: Vec::new(),
        ghost_preview,
        is_pro,
        semantic_pending,
    };
    Ok((result, timings))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i64, relevance: f64, exact: bool, title: &str) -> QueryResultItem {
        QueryResultItem {
            id,
            file_path: None,
            file_name: Some(title.to_string()),
            preview: String::new(),
            relevance,
            source_type: "test".to_string(),
            timestamp: None,
            match_reason: String::new(),
            exact_match: exact,
        }
    }

    /// Gap detection cost 25.7 s of a 30.5 s search. Guard the request path at the
    /// source level: neither the command nor its engine may call it again.
    #[test]
    fn search_path_never_calls_gap_detection() {
        let needle = concat!("detect_knowledge", "_gaps");
        for (file, src) in [
            (
                "natural_language_search.rs",
                include_str!("natural_language_search.rs"),
            ),
            (
                "natural_language_search_engine.rs",
                include_str!("natural_language_search_engine.rs"),
            ),
        ] {
            assert!(!src.contains(needle), "{file} must not call gap detection");
        }
    }

    #[test]
    fn free_ghost_preview_reports_no_gaps() {
        let ghost = build_ghost_preview(false, 10, 2).expect("hidden results => ghost");
        assert_eq!(ghost.hidden_results, 7);
        assert_eq!(ghost.gap_count, 0);
        assert!(build_ghost_preview(true, 10, 2).is_none());
    }

    #[test]
    fn exact_matches_stay_on_top_after_stack_boost() {
        let mut items = vec![
            item(1, 0.80, false, "tokio rusqlite axum"),
            item(2, 0.999, true, "crates.io: rusqlite v0.40.2"),
        ];
        let stack: Vec<StackContextEntry> = ["tokio", "rusqlite", "axum"]
            .iter()
            .map(|n| StackContextEntry {
                name: (*n).to_string(),
                category: "lib".to_string(),
                relevant: true,
            })
            .collect();
        boost_for_stack(&mut items, &stack);
        sort_items(&mut items);
        assert_eq!(items[0].id, 2);
        assert!(items[1].relevance <= NON_EXACT_RELEVANCE_CAP);
        assert!(items[0].relevance > items[1].relevance);
    }
}
