//! Natural Language Search — Intelligence Console query engine.
//!
//! Tiered search: free users get 3 results + ghost preview,
//! Pro users get full results + decision/gap cross-referencing.
//! Stack-aware boosting prioritises results matching the user's tech.

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::db::embedding_to_blob;
use crate::error::{Result, ResultExt};

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
        .map(|w| w.to_string())
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

fn boost_for_stack(items: &mut [QueryResultItem], stack: &[StackContextEntry]) {
    let stack_names: Vec<String> = stack.iter().map(|s| s.name.to_lowercase()).collect();
    if stack_names.is_empty() {
        return;
    }
    for item in items.iter_mut() {
        let title_lower = item.file_name.as_deref().unwrap_or("").to_lowercase();
        let preview_lower = item.preview.to_lowercase();
        let stack_matches: usize = stack_names
            .iter()
            .filter(|s| title_lower.contains(s.as_str()) || preview_lower.contains(s.as_str()))
            .count();
        if stack_matches > 0 {
            item.relevance += 0.15 * (stack_matches as f64).min(3.0);
            item.relevance = item.relevance.min(1.0);
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
// Knowledge gap detection for query
// ============================================================================

fn find_query_gaps(conn: &rusqlite::Connection, keywords: &[String]) -> Vec<QueryGap> {
    let gaps = match crate::knowledge_decay::detect_knowledge_gaps(conn) {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };

    let mut query_gaps = Vec::new();
    for gap in &gaps {
        let dep_lower = gap.dependency.to_lowercase();
        if keywords
            .iter()
            .any(|k| dep_lower.contains(k.as_str()) || k.contains(dep_lower.as_str()))
        {
            query_gaps.push(QueryGap {
                technology: gap.dependency.clone(),
                days_stale: gap.days_since_last_engagement,
                severity: serde_json::to_string(&gap.gap_severity)
                    .unwrap_or_else(|_| format!("{:?}", gap.gap_severity))
                    .trim_matches('"')
                    .to_string(),
            });
        }
    }
    query_gaps.truncate(5);
    query_gaps
}

// ============================================================================
// SQL text search
// ============================================================================

fn execute_text_search(
    conn: &rusqlite::Connection,
    parsed: &ParsedQuery,
    limit: usize,
) -> Result<Vec<QueryResultItem>> {
    if parsed.keywords.is_empty() {
        return Ok(Vec::new());
    }

    let conditions: Vec<String> = parsed
        .keywords
        .iter()
        .map(|k| {
            format!(
                "(LOWER(s.title) LIKE '%{kw}%' OR LOWER(s.content) LIKE '%{kw}%')",
                kw = k.replace('\'', "''")
            )
        })
        .collect();

    let where_clause = conditions.join(" AND ");

    let type_filter = if !parsed.file_types.is_empty() {
        let types: Vec<String> = parsed
            .file_types
            .iter()
            .map(|t| format!("'{}'", t.replace('\'', "''")))
            .collect();
        format!(" AND s.source_type IN ({})", types.join(","))
    } else {
        String::new()
    };

    let time_filter = if let Some(ref tr) = parsed.time_range {
        format!(" AND s.created_at >= '{}'", tr.start)
    } else {
        String::new()
    };

    let sql = format!(
        "SELECT s.id, s.source_type, s.url, s.title, s.content, s.created_at
         FROM source_items s
         WHERE ({where_clause}){type_filter}{time_filter}
         ORDER BY s.last_seen DESC
         LIMIT {limit}"
    );

    debug!(target: "4da::search", sql = %sql, "Executing text search");

    let mut stmt = conn.prepare(&sql).context("Query error")?;
    let rows = stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            let source_type: String = row.get(1)?;
            let url: Option<String> = row.get(2)?;
            let title: String = row.get(3)?;
            let content: String = row.get(4)?;
            let created_at: Option<String> = row.get(5)?;

            let preview = if content.len() > 200 {
                format!("{}...", &content[..200])
            } else {
                content
            };

            Ok(QueryResultItem {
                id,
                file_path: url.clone(),
                file_name: Some(title),
                preview,
                relevance: 0.5,
                source_type,
                timestamp: created_at,
                match_reason: "keyword match".to_string(),
            })
        })
        .context("Query error")?;

    let mut items = Vec::new();
    for row in rows {
        match row {
            Ok(item) => items.push(item),
            Err(e) => warn!(target: "4da::search", error = %e, "Row parse error"),
        }
    }
    Ok(items)
}

// ============================================================================
// Vector similarity search
// ============================================================================

async fn execute_vector_search(parsed: &ParsedQuery, limit: usize) -> Result<Vec<QueryResultItem>> {
    let search_text = crate::utils::preprocess_content(&parsed.keywords.join(" "));
    if search_text.is_empty() {
        return Ok(Vec::new());
    }

    let embeddings = crate::embeddings::embed_texts(&[search_text]).await?;
    if embeddings.is_empty() || embeddings[0].iter().all(|&v| v == 0.0) {
        debug!(target: "4da::search", "No embedding available, skipping vector search");
        return Ok(Vec::new());
    }

    let query_embedding = &embeddings[0];
    let conn = crate::open_db_connection()?;
    let blob = embedding_to_blob(query_embedding);

    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.source_type, s.url, s.title, s.content, s.created_at, v.distance
             FROM source_vec v
             JOIN source_items s ON s.id = v.rowid
             WHERE v.embedding MATCH ?1 AND k = ?2
             ORDER BY v.distance",
        )
        .context("Vector query error")?;

    let rows = stmt
        .query_map(rusqlite::params![blob, limit as i64], |row| {
            let id: i64 = row.get(0)?;
            let source_type: String = row.get(1)?;
            let url: Option<String> = row.get(2)?;
            let title: String = row.get(3)?;
            let content: String = row.get(4)?;
            let created_at: Option<String> = row.get(5)?;
            let distance: f64 = row.get(6)?;
            let relevance = (1.0 - distance).clamp(0.0, 1.0);

            let preview = if content.len() > 200 {
                format!("{}...", &content[..200])
            } else {
                content
            };

            Ok(QueryResultItem {
                id,
                file_path: url.clone(),
                file_name: Some(title),
                preview,
                relevance,
                source_type,
                timestamp: created_at,
                match_reason: format!("semantic similarity ({:.0}%)", relevance * 100.0),
            })
        })
        .context("Vector query error")?;

    let mut items = Vec::new();
    for row in rows {
        match row {
            Ok(item) => items.push(item),
            Err(e) => warn!(target: "4da::search", error = %e, "Row parse error"),
        }
    }
    Ok(items)
}

// ============================================================================
// Merge and deduplicate
// ============================================================================

fn merge_results(
    text_results: Vec<QueryResultItem>,
    vector_results: Vec<QueryResultItem>,
) -> Vec<QueryResultItem> {
    let mut seen_ids = std::collections::HashSet::new();
    let mut merged = Vec::new();

    for item in vector_results {
        if seen_ids.insert(item.id) {
            merged.push(item);
        }
    }
    for mut item in text_results {
        if seen_ids.insert(item.id) {
            item.relevance = 0.4;
            merged.push(item);
        }
    }

    merged.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    merged.truncate(30);
    merged
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
// Tauri Command — tiered response (no hard Pro gate)
// ============================================================================

const FREE_RESULT_LIMIT: usize = 3;

#[tauri::command]
pub async fn natural_language_query(query_text: String) -> Result<QueryResult> {
    let start = std::time::Instant::now();
    let query_text = query_text.trim().to_string();
    if query_text.is_empty() {
        return Err("Query cannot be empty".into());
    }

    let is_pro = crate::settings::is_pro();

    // 1. Parse intent locally
    let parsed = parse_query_local(&query_text);
    let intent = classify_intent(&query_text).to_string();

    debug!(
        target: "4da::search",
        query = %query_text, intent = %intent, is_pro,
        keywords = ?parsed.keywords,
        "Processing natural language query"
    );

    // 2. Stack context (available to all users)
    let conn = crate::open_db_connection()?;
    let stack_context = build_stack_context(&conn, &parsed.keywords);

    // 3. Execute text search
    let text_results = execute_text_search(&conn, &parsed, 30)?;
    drop(conn);

    // 4. Execute vector search
    let vector_results = match execute_vector_search(&parsed, 20).await {
        Ok(results) => results,
        Err(e) => {
            debug!(target: "4da::search", error = %e, "Vector search unavailable");
            Vec::new()
        }
    };

    // 5. Merge, deduplicate, boost for stack
    let mut all_items = merge_results(text_results, vector_results);
    boost_for_stack(&mut all_items, &stack_context);
    all_items.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total_count = all_items.len();

    // 6. Pro-only intelligence: decisions + gaps
    let (related_decisions, knowledge_gaps) = if is_pro {
        let conn = crate::open_db_connection()?;
        let decisions = find_related_decisions(&conn, &parsed.keywords);
        let gaps = find_query_gaps(&conn, &parsed.keywords);
        (decisions, gaps)
    } else {
        (Vec::new(), Vec::new())
    };

    // 7. Count Pro intelligence for ghost preview (even if user is Pro, compute for response)
    let decision_count = if is_pro {
        related_decisions.len()
    } else {
        let conn = crate::open_db_connection()?;
        find_related_decisions(&conn, &parsed.keywords).len()
    };
    let gap_count = if is_pro {
        knowledge_gaps.len()
    } else {
        let conn = crate::open_db_connection()?;
        find_query_gaps(&conn, &parsed.keywords).len()
    };

    // 8. Build ghost preview (free users only)
    let ghost_preview = if !is_pro && total_count > FREE_RESULT_LIMIT {
        Some(GhostPreview {
            total_results: total_count,
            hidden_results: total_count.saturating_sub(FREE_RESULT_LIMIT),
            decision_count,
            gap_count,
            synthesis_available: is_llm_configured(),
        })
    } else if !is_pro {
        // Even with few results, show ghost if there's Pro intelligence
        if decision_count > 0 || gap_count > 0 || is_llm_configured() {
            Some(GhostPreview {
                total_results: total_count,
                hidden_results: 0,
                decision_count,
                gap_count,
                synthesis_available: is_llm_configured(),
            })
        } else {
            None
        }
    } else {
        None
    };

    // 9. Truncate items for free users
    let items = if !is_pro && all_items.len() > FREE_RESULT_LIMIT {
        all_items[..FREE_RESULT_LIMIT].to_vec()
    } else {
        all_items
    };

    let execution_ms = start.elapsed().as_millis() as u64;

    Ok(QueryResult {
        query: query_text,
        intent,
        items,
        total_count,
        execution_ms,
        summary: None,
        parsed,
        stack_context,
        related_decisions,
        knowledge_gaps,
        ghost_preview,
        is_pro,
    })
}
