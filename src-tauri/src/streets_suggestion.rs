//! STREETS Contextual Discovery — surfaces relevant STREETS modules based on
//! user behavior (saved items, topic affinities) from the last 7 days.
//!
//! Maps topic clusters to STREETS modules and respects frequency caps
//! (max 1/day, 3/week) and completed-module exclusions.

use crate::error::Result;
use crate::state::open_db_connection;
use serde::Serialize;
use tracing::debug;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct StreetsSuggestion {
    pub module_id: String,
    pub module_title: String,
    pub reason: String,
    pub match_strength: f32,
}

// ============================================================================
// Module-to-Topic Mapping
// ============================================================================

/// Each STREETS module and its associated topic keywords.
const MODULE_TOPICS: &[(&str, &str, &[&str])] = &[
    (
        "S",
        "Sovereign Setup",
        &[
            "local-first",
            "privacy",
            "self-hosting",
            "infrastructure",
            "ollama",
            "llm",
            "gpu",
            "local",
            "homelab",
            "server",
            "docker",
        ],
    ),
    (
        "T",
        "Technical Moats",
        &[
            "brand",
            "credibility",
            "portfolio",
            "open source",
            "community",
            "moat",
            "competitive advantage",
            "unique",
            "differentiation",
        ],
    ),
    (
        "R",
        "Revenue Engines",
        &[
            "pricing",
            "monetization",
            "saas",
            "consulting",
            "templates",
            "revenue",
            "income",
            "freelance",
            "subscription",
            "mrr",
            "billing",
        ],
    ),
    (
        "E1",
        "Execution Playbook",
        &[
            "productivity",
            "automation",
            "workflows",
            "ci/cd",
            "tools",
            "shipping",
            "mvp",
            "launch",
            "deploy",
            "pipeline",
        ],
    ),
    (
        "E2",
        "Evolving Edge",
        &[
            "scaling",
            "growth",
            "hiring",
            "delegation",
            "business expansion",
            "market",
            "trends",
            "adaptation",
        ],
    ),
    (
        "T2",
        "Tactical Automation",
        &[
            "career",
            "learning",
            "skills",
            "adaptation",
            "industry trends",
            "upskilling",
            "transformation",
            "ai tools",
        ],
    ),
    (
        "S2",
        "Stacking Streams",
        &[
            "systems",
            "processes",
            "team management",
            "enterprise",
            "architecture",
            "resilience",
            "diversification",
            "multiple streams",
        ],
    ),
];

// ============================================================================
// Suggestion Logic
// ============================================================================

/// Get a contextual STREETS suggestion based on recent saved items' topics.
///
/// Returns `None` if:
/// - No saved items match any module topics (last 7 days)
/// - Fewer than 3 matching saves for the best module
/// - The best module is already completed
/// - Frequency cap exceeded (shown within last 24h, or 3+ in last 7 days)
#[tauri::command]
pub async fn get_streets_suggestion() -> Result<Option<StreetsSuggestion>> {
    let conn = open_db_connection()?;

    // 1. Get completed modules (never suggest these)
    // A module is considered "complete" when 6+ lessons are done (standard module size).
    let completed_modules: Vec<String> = conn
        .prepare(
            "SELECT module_id FROM playbook_progress
             GROUP BY module_id
             HAVING COUNT(*) >= 6",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    // 2. Get recent saved item topics (last 7 days)
    let recent_topics: Vec<String> = conn
        .prepare(
            "SELECT item_topics FROM interactions
             WHERE action_type = 'save'
             AND timestamp > datetime('now', '-7 days')
             ORDER BY timestamp DESC
             LIMIT 200",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    // Parse JSON topic arrays and flatten
    let all_topics: Vec<String> = recent_topics
        .iter()
        .filter_map(|t| serde_json::from_str::<Vec<String>>(t).ok())
        .flatten()
        .map(|t| t.to_lowercase())
        .collect();

    if all_topics.is_empty() {
        debug!(target: "4da::streets_suggest", "No recent saved topics found");
        return Ok(None);
    }

    // 3. Score each module by topic match count
    let mut best_module: Option<(&str, &str, usize)> = None;

    for (module_id, module_title, keywords) in MODULE_TOPICS {
        // Skip completed modules
        if completed_modules.iter().any(|m| m == *module_id) {
            continue;
        }

        let match_count = all_topics
            .iter()
            .filter(|topic| {
                keywords
                    .iter()
                    .any(|kw| topic.contains(kw) || kw.contains(topic.as_str()))
            })
            .count();

        if match_count >= 3 && (best_module.is_none() || match_count > best_module.unwrap().2) {
            best_module = Some((module_id, module_title, match_count));
        }
    }

    let (module_id, module_title, match_count) = match best_module {
        Some(m) => m,
        None => {
            debug!(target: "4da::streets_suggest", "No module reached 3+ topic matches");
            return Ok(None);
        }
    };

    // 4. Check frequency caps using kv_store
    let now_epoch: i64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Check last suggestion time (24h cap)
    let last_shown: Option<i64> = conn
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM kv_store WHERE key = 'streets_suggest_last'",
            [],
            |row| row.get(0),
        )
        .ok();

    if let Some(last) = last_shown {
        if now_epoch - last < 86400 {
            debug!(target: "4da::streets_suggest", "Frequency cap: shown within last 24h");
            return Ok(None);
        }
    }

    // Check weekly count (max 3/week)
    let weekly_count: i64 = conn
        .query_row(
            "SELECT CAST(COALESCE(value, '0') AS INTEGER) FROM kv_store WHERE key = 'streets_suggest_weekly_count'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let weekly_reset: i64 = conn
        .query_row(
            "SELECT CAST(COALESCE(value, '0') AS INTEGER) FROM kv_store WHERE key = 'streets_suggest_weekly_reset'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Reset weekly counter if more than 7 days since last reset
    let effective_weekly = if now_epoch - weekly_reset > 604800 {
        0
    } else {
        weekly_count
    };

    if effective_weekly >= 3 {
        debug!(target: "4da::streets_suggest", "Frequency cap: 3+ suggestions this week");
        return Ok(None);
    }

    // 5. Record this suggestion (kv_store.value is REAL)
    conn.execute(
        "INSERT OR REPLACE INTO kv_store (key, value, updated_at) VALUES ('streets_suggest_last', ?1, datetime('now'))",
        rusqlite::params![now_epoch as f64],
    ).ok();

    let new_weekly = if now_epoch - weekly_reset > 604800 {
        // Reset the week
        conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value, updated_at) VALUES ('streets_suggest_weekly_reset', ?1, datetime('now'))",
            rusqlite::params![now_epoch as f64],
        ).ok();
        1
    } else {
        effective_weekly + 1
    };

    conn.execute(
        "INSERT OR REPLACE INTO kv_store (key, value, updated_at) VALUES ('streets_suggest_weekly_count', ?1, datetime('now'))",
        rusqlite::params![new_weekly as f64],
    ).ok();

    let match_strength = (match_count as f32 / 10.0).min(1.0);
    let reason = format!(
        "You saved {} items about topics related to this module in the last week.",
        match_count
    );

    debug!(target: "4da::streets_suggest",
        module = module_id,
        matches = match_count,
        strength = match_strength,
        "Generated STREETS suggestion"
    );

    Ok(Some(StreetsSuggestion {
        module_id: module_id.to_string(),
        module_title: module_title.to_string(),
        reason,
        match_strength,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_topics_coverage() {
        // All 7 STREETS modules should be defined
        assert_eq!(MODULE_TOPICS.len(), 7);
    }

    #[test]
    fn test_module_ids_are_valid() {
        let valid_ids = ["S", "T", "R", "E1", "E2", "T2", "S2"];
        for (id, _, _) in MODULE_TOPICS {
            assert!(valid_ids.contains(id), "Invalid module ID: {}", id);
        }
    }

    #[test]
    fn test_each_module_has_keywords() {
        for (id, _, keywords) in MODULE_TOPICS {
            assert!(!keywords.is_empty(), "Module {} has no keywords", id);
        }
    }

    #[test]
    fn test_suggestion_type_serialization() {
        let suggestion = StreetsSuggestion {
            module_id: "R".to_string(),
            module_title: "Revenue Engines".to_string(),
            reason: "You saved 4 items about pricing".to_string(),
            match_strength: 0.4,
        };
        let json = serde_json::to_string(&suggestion).unwrap();
        assert!(json.contains("Revenue Engines"));
        assert!(json.contains("match_strength"));
    }
}
