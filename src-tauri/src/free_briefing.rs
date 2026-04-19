// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
    crate::ipc_rate_limit::check_rate_limit("generate_free_briefing", 10)?;

    info!(target: "4da::briefing", "Generating free-tier briefing");

    let user_lang = crate::i18n::get_user_language();

    // Try in-memory results first, fall back to DB
    let items: Vec<(String, Option<String>, String, f64)> = {
        let state = get_analysis_state().lock();
        if let Some(ref results) = state.results {
            results
                .iter()
                .filter(|r| r.relevant && !r.excluded)
                .filter(|r| r.detected_lang == user_lang)
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
        db.get_relevant_items_since(period_start, 0.1, 30, &user_lang)
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
        let lang = crate::i18n::get_user_language();
        return Ok(serde_json::json!({
            "success": true,
            "empty": true,
            "message": crate::i18n::t("errors:briefing.empty", &lang, &[])
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

    // Single lock: extract both priority map and signal counts
    let (priority_map, signal_priorities): (HashMap<String, String>, HashMap<String, usize>) = {
        let state = get_analysis_state().lock();
        if let Some(ref results) = state.results {
            let pmap: HashMap<String, String> = results
                .iter()
                .filter_map(|r| {
                    r.signal_priority
                        .as_ref()
                        .map(|p| (r.title.clone(), p.clone()))
                })
                .collect();
            let mut scounts: HashMap<String, usize> = HashMap::new();
            for r in results
                .iter()
                .filter(|r| r.relevant && !r.excluded)
                .filter(|r| r.detected_lang == user_lang)
            {
                if let Some(ref priority) = r.signal_priority {
                    *scounts.entry(priority.clone()).or_insert(0) += 1;
                }
            }
            (pmap, scounts)
        } else {
            (HashMap::new(), HashMap::new())
        }
    };

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

        let priority = priority_map.get(title).cloned();
        top_items.push(serde_json::json!({
            "title": title,
            "url": url,
            "source": source,
            "score": format!("{:.0}%", score * 100.0),
            "signal_priority": priority,
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

    // Knowledge gaps: tech with no recent signals
    let knowledge_gaps: Vec<serde_json::Value> = {
        let mut gaps = Vec::new();
        if let Ok(conn) = crate::open_db_connection() {
            for tech in &ace_ctx.detected_tech {
                let tech_lower = tech.to_lowercase();
                if let Ok(days) = conn.query_row(
                    "SELECT CAST(julianday('now') - julianday(MAX(created_at)) AS INTEGER) FROM source_items WHERE LOWER(title) LIKE ?1",
                    rusqlite::params![format!("%{}%", tech_lower)],
                    |row| row.get::<_, i64>(0),
                ) {
                    if days >= 5 {
                        gaps.push(serde_json::json!({ "topic": tech, "days_since_last": days }));
                    }
                }
            }
        }
        gaps.sort_by(|a, b| {
            b["days_since_last"]
                .as_i64()
                .unwrap_or(0)
                .cmp(&a["days_since_last"].as_i64().unwrap_or(0))
        });
        gaps.truncate(5);
        gaps
    };

    // Wisdom signals from AWE
    let wisdom_signals: Vec<serde_json::Value> = {
        if let Some(path) = crate::context_commands::find_awe_binary() {
            if let Ok(output) = crate::context_commands::run_awe_with_timeout(
                std::process::Command::new(&path).args([
                    "wisdom",
                    "--domain",
                    "software-engineering",
                ]),
                10,
            ) {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut signals = Vec::new();
                let mut current_type = "";
                for line in stdout.lines() {
                    let trimmed = line.trim();
                    if trimmed.contains("VALIDATED PRINCIPLES") {
                        current_type = "principle";
                    } else if trimmed.contains("ANTI-PATTERNS") {
                        current_type = "anti-pattern";
                    } else if trimmed.starts_with('[') && !current_type.is_empty() {
                        if let Some(end) = trimmed.find(']') {
                            let conf = trimmed[1..end]
                                .trim_end_matches('%')
                                .parse::<f64>()
                                .unwrap_or(0.0)
                                / 100.0;
                            let text = trimmed[end + 1..].trim();
                            if !text.is_empty() && conf > 0.0 {
                                signals.push(serde_json::json!({ "text": text, "confidence": conf, "signal_type": current_type }));
                            }
                        }
                    }
                }
                signals.sort_by(|a, b| {
                    b["confidence"]
                        .as_f64()
                        .unwrap_or(0.0)
                        .partial_cmp(&a["confidence"].as_f64().unwrap_or(0.0))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                signals.truncate(3);
                signals
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    };

    // GAME: track briefing generation
    if let Ok(db) = crate::get_database() {
        for a in crate::achievement_engine::increment_counter(db, "briefings", 1) {
            crate::events::emit_achievement_unlocked(&app, &a);
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "empty": false,
        "top_items": top_items,
        "stack_alerts": stack_alerts,
        "source_summary": source_counts,
        "signal_priorities": signal_priorities,
        "knowledge_gaps": knowledge_gaps,
        "wisdom_signals": wisdom_signals,
        "total_items": items.len(),
        "generated_at": chrono::Utc::now().to_rfc3339(),
    }))
}
