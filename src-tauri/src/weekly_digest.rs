//! Weekly Intelligence Digest — Pro-gated aggregated intelligence report.
//!
//! Combines attention report, signal chains, knowledge gaps, and top items
//! from the past week into a single structured digest. All data sourced
//! locally from SQLite — no external API calls required.

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::settings::require_pro_feature;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyDigest {
    pub generated_at: String,
    pub period_start: String,
    pub period_end: String,
    pub highlights: Vec<DigestHighlight>,
    pub top_topics: Vec<DigestTopic>,
    pub active_signals: Vec<DigestSignal>,
    pub knowledge_gaps: Vec<DigestGap>,
    pub stats: DigestStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestHighlight {
    pub title: String,
    pub source_type: String,
    pub score: f64,
    pub url: Option<String>,
    pub discovered_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestTopic {
    pub topic: String,
    pub interactions: u32,
    pub trend: String, // "rising", "stable", "declining"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestSignal {
    pub chain_name: String,
    pub priority: String,
    pub link_count: usize,
    pub suggested_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestGap {
    pub dependency: String,
    pub severity: String,
    pub days_stale: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestStats {
    pub total_items_analyzed: u32,
    pub relevant_items: u32,
    pub new_sources_discovered: u32,
    pub avg_relevance_score: f64,
}

// ============================================================================
// Data collection helpers
// ============================================================================

fn collect_top_items(conn: &rusqlite::Connection, days: i64) -> Vec<DigestHighlight> {
    let cutoff = (Utc::now() - Duration::days(days))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    // source_items ordered by recency (most recently seen = most active)
    let sql = "SELECT title, source_type, url, created_at
               FROM source_items
               WHERE created_at >= ?1
               ORDER BY last_seen DESC
               LIMIT 10";

    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::digest", error = %e, "Failed to query top items");
            return Vec::new();
        }
    };

    let rows = match stmt.query_map(rusqlite::params![cutoff], |row| {
        Ok(DigestHighlight {
            title: row.get::<_, String>(0).unwrap_or_default(),
            source_type: row.get::<_, String>(1).unwrap_or_default(),
            url: row.get(2).ok(),
            discovered_at: row.get::<_, String>(3).unwrap_or_default(),
            score: 0.0, // Score computed in-memory during analysis, not stored
        })
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::digest", error = %e, "Failed to map top items");
            return Vec::new();
        }
    };

    rows.filter_map(|r| r.ok()).collect()
}

fn collect_stats(conn: &rusqlite::Connection, days: i64) -> DigestStats {
    let cutoff = (Utc::now() - Duration::days(days))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let total: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM source_items WHERE created_at >= ?1",
            rusqlite::params![cutoff],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Count distinct source types active this period
    let active_sources: u32 = conn
        .query_row(
            "SELECT COUNT(DISTINCT source_type) FROM source_items WHERE created_at >= ?1",
            rusqlite::params![cutoff],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Count items with embeddings (proxy for "analyzed" items)
    let analyzed: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM source_items
             WHERE created_at >= ?1 AND LENGTH(embedding) > 0",
            rusqlite::params![cutoff],
            |row| row.get(0),
        )
        .unwrap_or(0);

    DigestStats {
        total_items_analyzed: total,
        relevant_items: analyzed,
        new_sources_discovered: active_sources,
        avg_relevance_score: 0.0, // Relevance computed in-memory, not persisted
    }
}

fn collect_topics(conn: &rusqlite::Connection) -> Vec<DigestTopic> {
    // Use ACE topic affinities if available
    let sql = "SELECT topic, score FROM ace_topic_affinities ORDER BY score DESC LIMIT 10";
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let rows = match stmt.query_map([], |row| {
        let topic: String = row.get(0)?;
        let score: f64 = row.get(1)?;
        let interactions = (score * 100.0).round() as u32;
        let trend = if score > 0.7 {
            "rising"
        } else if score > 0.3 {
            "stable"
        } else {
            "declining"
        };
        Ok(DigestTopic {
            topic,
            interactions,
            trend: trend.to_string(),
        })
    }) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    rows.filter_map(|r| r.ok()).collect()
}

// ============================================================================
// Tauri Command
// ============================================================================

#[tauri::command]
pub async fn generate_weekly_digest() -> Result<WeeklyDigest, String> {
    require_pro_feature("generate_weekly_digest")?;

    let now = Utc::now();
    let week_ago = now - Duration::days(7);

    debug!(target: "4da::digest", "Generating weekly intelligence digest");

    let conn = crate::open_db_connection()?;

    // 1. Top items from the week
    let highlights = collect_top_items(&conn, 7);

    // 2. Stats
    let stats = collect_stats(&conn, 7);

    // 3. Topics
    let top_topics = collect_topics(&conn);

    // 4. Signal chains (reuse existing detection)
    let active_signals = match crate::signal_chains::detect_chains(&conn) {
        Ok(chains) => chains
            .into_iter()
            .take(5)
            .map(|c| DigestSignal {
                chain_name: c.chain_name,
                priority: c.overall_priority,
                link_count: c.links.len(),
                suggested_action: c.suggested_action,
            })
            .collect(),
        Err(e) => {
            debug!(target: "4da::digest", error = %e, "Signal chains unavailable");
            Vec::new()
        }
    };

    // 5. Knowledge gaps (reuse existing detection)
    let knowledge_gaps = match crate::knowledge_decay::detect_knowledge_gaps(&conn) {
        Ok(gaps) => gaps
            .into_iter()
            .take(5)
            .map(|g| DigestGap {
                dependency: g.dependency,
                severity: serde_json::to_string(&g.gap_severity)
                    .unwrap_or_else(|_| format!("{:?}", g.gap_severity))
                    .trim_matches('"')
                    .to_string(),
                days_stale: g.days_since_last_engagement,
            })
            .collect(),
        Err(e) => {
            debug!(target: "4da::digest", error = %e, "Knowledge gaps unavailable");
            Vec::new()
        }
    };

    Ok(WeeklyDigest {
        generated_at: now.to_rfc3339(),
        period_start: week_ago.format("%Y-%m-%d").to_string(),
        period_end: now.format("%Y-%m-%d").to_string(),
        highlights,
        top_topics,
        active_signals,
        knowledge_gaps,
        stats,
    })
}
