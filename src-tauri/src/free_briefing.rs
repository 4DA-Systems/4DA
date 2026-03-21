//! Template briefing -- structured summary without LLM.
//!
//! Gives all users a useful daily overview using only scored data.
//! No API calls, no cost, instant response.

use crate::error::Result;
use crate::scoring::{get_ace_context, has_word_boundary_match};
use crate::{get_analysis_state, get_database};
use std::collections::HashMap;
use tracing::info;

/// Template briefing (no LLM required)
#[tauri::command]
pub async fn generate_free_briefing(app: tauri::AppHandle) -> Result<serde_json::Value> {
    info!(target: "4da::briefing", "Generating free-tier briefing");

    // Try in-memory results first, fall back to DB
    let items: Vec<(String, Option<String>, String, f64)> = {
        let state = get_analysis_state().lock();
        if let Some(ref results) = state.results {
            results
                .iter()
                .filter(|r| r.relevant && !r.excluded)
                .take(30)
                .map(|r| {
                    (
                        r.title.clone(),
                        r.url.clone(),
                        r.source_type.clone(),
                        r.top_score as f64,
                    )
                })
                .collect()
        } else {
            vec![]
        }
    };

    let items = if items.is_empty() {
        // Fall back to database query
        let db = get_database()?;
        let period_start = chrono::Utc::now() - chrono::Duration::hours(72);
        db.get_relevant_items_since(period_start, 0.1, 30)
            .map(|db_items| {
                db_items
                    .into_iter()
                    .map(|i| {
                        (
                            i.title,
                            i.url,
                            i.source_type,
                            i.relevance_score.unwrap_or(0.0),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    } else {
        items
    };

    if items.is_empty() {
        return Ok(serde_json::json!({
            "success": true,
            "empty": true,
            "message": "No items found. Run an analysis first."
        }));
    }

    // Noise prefixes to filter out from briefing items
    const NOISE_PREFIXES: &[&str] = &[
        "show hn:",
        "ask hn:",
        "poll:",
        "who is hiring",
        "who wants to be hired",
        "freelancer? seeking",
    ];

    // Top 5 items with filtering and source diversity
    let mut top_items: Vec<serde_json::Value> = Vec::new();
    let mut diversity_counts: HashMap<String, usize> = HashMap::new();

    for (title, url, source, score) in &items {
        if top_items.len() >= 5 {
            break;
        }

        // Filter out low-score items
        if *score < 0.15 {
            continue;
        }

        // Filter out noise patterns (case-insensitive prefix match)
        let title_lower = title.to_lowercase();
        if NOISE_PREFIXES
            .iter()
            .any(|prefix| title_lower.starts_with(prefix))
        {
            continue;
        }

        // Source diversity: max 2 items per source_type
        let count = diversity_counts.entry(source.clone()).or_default();
        if *count >= 2 {
            continue;
        }
        *count += 1;

        top_items.push(serde_json::json!({
            "title": title,
            "url": url,
            "source": source,
            "score": format!("{:.0}%", score * 100.0),
        }));
    }

    // Stack alerts: items that mention detected tech
    let ace_ctx = get_ace_context();
    let tech_lower: Vec<String> = ace_ctx
        .detected_tech
        .iter()
        .map(|t| t.to_lowercase())
        .collect();
    let stack_alerts: Vec<serde_json::Value> = items
        .iter()
        .filter(|(title, _, _, _)| {
            let t = title.to_lowercase();
            tech_lower
                .iter()
                .any(|tech| has_word_boundary_match(&t, tech))
        })
        .take(3)
        .map(|(title, url, source, _)| {
            serde_json::json!({
                "title": title,
                "url": url,
                "source": source,
            })
        })
        .collect();

    // Source summary
    let mut source_counts: HashMap<String, usize> = HashMap::new();
    for (_, _, source, _) in &items {
        *source_counts.entry(source.clone()).or_default() += 1;
    }

    // GAME: track briefing generation
    if let Ok(db) = crate::get_database() {
        for a in crate::game_engine::increment_counter(db, "briefings", 1) {
            crate::events::emit_achievement_unlocked(&app, &a);
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "empty": false,
        "top_items": top_items,
        "stack_alerts": stack_alerts,
        "source_summary": source_counts,
        "total_items": items.len(),
        "generated_at": chrono::Utc::now().to_rfc3339(),
    }))
}
