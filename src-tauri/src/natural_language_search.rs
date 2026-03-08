//! Natural Language Search — Pro-gated intelligent query engine.
//!
//! Parses user queries locally (keyword extraction, time range detection,
//! intent classification), then executes SQL text search + sqlite-vec
//! vector similarity. Optional LLM enhancement for better intent parsing.

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::db::embedding_to_blob;
use crate::settings::require_pro_feature;

// ============================================================================
// Types (match existing frontend NaturalLanguageSearch.tsx interface)
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
pub struct QueryResult {
    pub query: String,
    pub intent: String,
    pub items: Vec<QueryResultItem>,
    pub total_count: usize,
    pub execution_ms: u64,
    pub summary: Option<String>,
    pub parsed: ParsedQuery,
}

// ============================================================================
// Stop words for keyword extraction
// ============================================================================

const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for",
    "of", "with", "by", "from", "is", "it", "that", "this", "was", "are",
    "be", "has", "have", "had", "do", "does", "did", "will", "would",
    "could", "should", "may", "might", "can", "shall", "not", "no",
    "so", "if", "then", "than", "when", "where", "what", "which", "who",
    "how", "all", "each", "every", "any", "some", "such", "only", "own",
    "same", "other", "into", "about", "up", "out", "just", "also", "very",
    "my", "me", "i", "we", "you", "your", "our", "they", "them", "their",
    "show", "find", "get", "give", "tell", "list", "display",
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
        types.push("pdf".to_string());
    }
    if q.contains("doc") || q.contains("word") {
        types.push("docx".to_string());
    }
    if q.contains("spreadsheet") || q.contains("excel") || q.contains("xlsx") || q.contains("csv")
    {
        types.push("xlsx".to_string());
    }
    if q.contains("image") || q.contains("photo") || q.contains("screenshot") {
        types.push("image".to_string());
    }
    types
}

// ============================================================================
// Keyword extraction
// ============================================================================

fn extract_keywords(query: &str) -> Vec<String> {
    let stop_set: std::collections::HashSet<&str> = STOP_WORDS.iter().copied().collect();
    query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| w.len() > 2 && !stop_set.contains(w))
        .map(|w| w.to_string())
        .collect()
}

// ============================================================================
// Local query parsing (no LLM required)
// ============================================================================

fn parse_query_local(query: &str) -> ParsedQuery {
    let keywords = extract_keywords(query);
    let time_range = detect_time_range(query);
    let file_types = detect_file_types(query);
    let intent_keywords: Vec<&str> = vec![
        "summarize", "compare", "count", "timeline", "find", "show", "list",
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
// SQL search execution
// ============================================================================

fn execute_text_search(
    conn: &rusqlite::Connection,
    parsed: &ParsedQuery,
    limit: usize,
) -> Result<Vec<QueryResultItem>, String> {
    if parsed.keywords.is_empty() {
        return Ok(Vec::new());
    }

    // Build LIKE conditions for each keyword
    let conditions: Vec<String> = parsed
        .keywords
        .iter()
        .map(|k| format!("(LOWER(s.title) LIKE '%{kw}%' OR LOWER(s.content) LIKE '%{kw}%')", kw = k.replace('\'', "''")))
        .collect();

    let where_clause = conditions.join(" AND ");

    // Source type filter
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

    // Time range filter
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

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("Query error: {e}"))?;
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
                relevance: 0.5, // Text match base score
                source_type,
                timestamp: created_at,
                match_reason: "keyword match".to_string(),
            })
        })
        .map_err(|e| format!("Query error: {e}"))?;

    let mut items: Vec<QueryResultItem> = Vec::new();
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

async fn execute_vector_search(
    parsed: &ParsedQuery,
    limit: usize,
) -> Result<Vec<QueryResultItem>, String> {
    // Reconstruct search text from keywords
    let search_text = parsed.keywords.join(" ");
    if search_text.is_empty() {
        return Ok(Vec::new());
    }

    // Embed the query
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
        .map_err(|e| format!("Vector query error: {e}"))?;

    let rows = stmt
        .query_map(rusqlite::params![blob, limit as i64], |row| {
            let id: i64 = row.get(0)?;
            let source_type: String = row.get(1)?;
            let url: Option<String> = row.get(2)?;
            let title: String = row.get(3)?;
            let content: String = row.get(4)?;
            let created_at: Option<String> = row.get(5)?;
            let distance: f64 = row.get(6)?;

            let relevance = (1.0 - distance).max(0.0).min(1.0);

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
        .map_err(|e| format!("Vector query error: {e}"))?;

    let mut items: Vec<QueryResultItem> = Vec::new();
    for row in rows {
        match row {
            Ok(item) => items.push(item),
            Err(e) => warn!(target: "4da::search", error = %e, "Row parse error"),
        }
    }
    Ok(items)
}

// ============================================================================
// Merge and deduplicate results
// ============================================================================

fn merge_results(
    text_results: Vec<QueryResultItem>,
    vector_results: Vec<QueryResultItem>,
) -> Vec<QueryResultItem> {
    let mut seen_ids = std::collections::HashSet::new();
    let mut merged = Vec::new();

    // Vector results first (higher quality), then text results
    for item in vector_results {
        if seen_ids.insert(item.id) {
            merged.push(item);
        }
    }
    for mut item in text_results {
        if seen_ids.insert(item.id) {
            // Boost text match if it also appeared in vector results conceptually
            item.relevance = 0.4;
            merged.push(item);
        }
    }

    // Sort by relevance descending
    merged.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
    merged.truncate(20);
    merged
}

// ============================================================================
// Tauri Command
// ============================================================================

#[tauri::command]
pub async fn natural_language_query(query_text: String) -> Result<QueryResult, String> {
    require_pro_feature("natural_language_query")?;

    let start = std::time::Instant::now();
    let query_text = query_text.trim().to_string();
    if query_text.is_empty() {
        return Err("Query cannot be empty".to_string());
    }

    // 1. Parse intent locally
    let parsed = parse_query_local(&query_text);
    let intent = classify_intent(&query_text).to_string();

    debug!(
        target: "4da::search",
        query = %query_text,
        intent = %intent,
        keywords = ?parsed.keywords,
        "Processing natural language query"
    );

    // 2. Execute text search
    let conn = crate::open_db_connection()?;
    let text_results = execute_text_search(&conn, &parsed, 20)?;
    drop(conn);

    // 3. Execute vector search (async, may fail gracefully)
    let vector_results = match execute_vector_search(&parsed, 20).await {
        Ok(results) => results,
        Err(e) => {
            debug!(target: "4da::search", error = %e, "Vector search unavailable, using text only");
            Vec::new()
        }
    };

    // 4. Merge and deduplicate
    let items = merge_results(text_results, vector_results);
    let total_count = items.len();

    let execution_ms = start.elapsed().as_millis() as u64;

    Ok(QueryResult {
        query: query_text,
        intent,
        items,
        total_count,
        execution_ms,
        summary: None,
        parsed,
    })
}
