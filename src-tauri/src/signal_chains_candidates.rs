// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Source item loading for signal-chain detection.

use rusqlite::params;

use crate::error::Result;

use super::{SIGNAL_CHAIN_MAX_ITEMS, SIGNAL_CHAIN_PER_SOURCE_DAY, SIGNAL_CHAIN_WINDOW_DAYS};

pub(super) type ChainCandidateItem = (i64, String, String, String, String, Vec<String>);

pub(super) fn load_recent_chain_candidate_items(
    conn: &rusqlite::Connection,
) -> Result<Vec<ChainCandidateItem>> {
    let columns = SourceItemColumns::read(conn);
    let signal_at_expr = columns.signal_at_expr();
    let tags_expr = columns.tags_expr();
    let relevance_expr = columns.relevance_expr();
    let embedding_filter = columns.embedding_filter();
    let window = format!("-{SIGNAL_CHAIN_WINDOW_DAYS} days");

    // Balance by source and day. The old newest-200 slice frequently contained
    // only the current burst/day, making the multi-day chain gate impossible.
    let sql = format!(
        "WITH eligible AS (
             SELECT si.id,
                    COALESCE(si.title, '') AS title,
                    COALESCE(si.source_type, 'unknown') AS source_type,
                    {signal_at_expr} AS signal_at,
                    substr(COALESCE(si.content, ''), 1, 500) AS content,
                    {tags_expr} AS tags,
                    {relevance_expr} AS chain_rank_score
             FROM source_items si
             WHERE {signal_at_expr} >= datetime('now', ?1)
             {embedding_filter}
         ),
         ranked AS (
             SELECT *,
                    ROW_NUMBER() OVER (
                        PARTITION BY source_type, DATE(signal_at)
                        ORDER BY chain_rank_score DESC, signal_at DESC, id DESC
                    ) AS source_day_rank
             FROM eligible
         )
         SELECT id, title, source_type, signal_at, content, tags
         FROM ranked
         WHERE source_day_rank <= ?2
         ORDER BY signal_at DESC, id DESC
         LIMIT ?3"
    );

    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(
            params![
                window,
                SIGNAL_CHAIN_PER_SOURCE_DAY as i64,
                SIGNAL_CHAIN_MAX_ITEMS as i64
            ],
            map_candidate_row,
        )?
        .filter_map(valid_candidate_row)
        .collect();

    Ok(items)
}

pub(super) fn load_chain_candidate_items_by_id(
    conn: &rusqlite::Connection,
    item_ids: &[i64],
) -> Result<Vec<ChainCandidateItem>> {
    let columns = SourceItemColumns::read(conn);
    let signal_at_expr = columns.signal_at_expr();
    let tags_expr = columns.tags_expr();
    let mut items: Vec<ChainCandidateItem> = Vec::new();

    for chunk in item_ids.chunks(500) {
        let placeholders = vec!["?"; chunk.len()].join(",");
        let sql = format!(
            "SELECT si.id,
                    COALESCE(si.title, ''),
                    COALESCE(si.source_type, 'unknown'),
                    {signal_at_expr} AS signal_at,
                    substr(COALESCE(si.content, ''), 1, 500),
                    {tags_expr}
                 FROM source_items si
                 WHERE si.id IN ({placeholders})"
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> = chunk
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt.query_map(params.as_slice(), map_candidate_row)?;
        items.extend(rows.filter_map(valid_candidate_row));
    }

    items.sort_by_key(|(id, ..)| *id);
    Ok(items)
}

fn map_candidate_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChainCandidateItem> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get::<_, String>(4).unwrap_or_default(),
        parse_source_tags(&row.get::<_, String>(5).unwrap_or_default()),
    ))
}

fn valid_candidate_row(row: rusqlite::Result<ChainCandidateItem>) -> Option<ChainCandidateItem> {
    match row {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("Row processing failed in signal_chains: {e}");
            None
        }
    }
}

struct SourceItemColumns {
    published_at: bool,
    tags: bool,
    relevance_score: bool,
    embedding_status: bool,
}

impl SourceItemColumns {
    fn read(conn: &rusqlite::Connection) -> Self {
        Self {
            published_at: source_items_has_column(conn, "published_at"),
            tags: source_items_has_column(conn, "tags"),
            relevance_score: source_items_has_column(conn, "relevance_score"),
            embedding_status: source_items_has_column(conn, "embedding_status"),
        }
    }

    fn signal_at_expr(&self) -> &'static str {
        if self.published_at {
            "COALESCE(datetime(si.published_at), datetime(si.created_at), si.created_at)"
        } else {
            "COALESCE(datetime(si.created_at), si.created_at)"
        }
    }

    fn tags_expr(&self) -> &'static str {
        if self.tags {
            "COALESCE(si.tags, '')"
        } else {
            "''"
        }
    }

    fn relevance_expr(&self) -> &'static str {
        if self.relevance_score {
            "COALESCE(si.relevance_score, 0.0)"
        } else {
            "0.0"
        }
    }

    fn embedding_filter(&self) -> &'static str {
        if self.embedding_status {
            "AND (si.embedding_status IS NULL OR si.embedding_status = 'complete')"
        } else {
            ""
        }
    }
}

fn source_items_has_column(conn: &rusqlite::Connection, column: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('source_items') WHERE name = ?1",
        params![column],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

fn parse_source_tags(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if let Ok(tags) = serde_json::from_str::<Vec<String>>(trimmed) {
        return tags;
    }
    trimmed
        .split([',', ';', '|'])
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}
