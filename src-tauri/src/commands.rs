// Copyright (c) 2025-2026 Antony Lawrence Kiddie Pasifa. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use tracing::info;

use crate::error::Result;
use crate::{
    anomaly, developer_dna, extract_topics, get_ace_engine, get_analysis_state, get_context_engine,
    get_database, get_relevance_threshold, get_source_registry, health, scoring,
    set_relevance_threshold, SourceRelevance,
};

// ============================================================================
// Commands
// ============================================================================

// get_hn_top_stories + compute_relevance removed: shadow scoring pipeline
// that duplicated scoring::score_item() but missed calibration, content quality,
// novelty, content DNA, competing tech, domain relevance, intent boost, dependency intelligence.
// Onboarding now uses run_cached_analysis + get_analysis_status (unified pipeline).
// ============================================================================
// Background Job Functions (called by monitoring scheduler)
// ============================================================================

/// Run background health check - called every 5 minutes by scheduler
pub async fn run_background_health_check() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn().lock();
    let report = health::check_all_components(&conn)?;

    info!(
        target: "4da::health",
        status = ?report.overall_status,
        quality = ?report.context_quality,
        fallback = report.fallback_level,
        "Background health check complete"
    );

    Ok(serde_json::to_value(&report)?)
}

/// Run background anomaly detection - called every hour by scheduler
pub async fn run_background_anomaly_detection() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;
    let conn = ace.get_conn().lock();
    let anomalies = anomaly::detect_all(&conn)?;

    // Store any new anomalies
    let mut new_count = 0;
    for a in &anomalies {
        if anomaly::store_anomaly(&conn, a).is_ok() {
            new_count += 1;
        }
    }

    info!(target: "4da::anomaly", found = anomalies.len(), stored = new_count, "Background anomaly detection complete");

    Ok(serde_json::json!({
        "anomalies_found": anomalies.len(),
        "new_stored": new_count,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Run background anomaly detection and return the anomaly objects.
/// Used by monitoring_jobs to process anomalies (bridge to notifications).
pub async fn run_background_anomaly_detection_with_results() -> Result<Vec<crate::anomaly::Anomaly>>
{
    let ace = get_ace_engine()?;
    let conn = ace.get_conn().lock();
    let anomalies = anomaly::detect_all(&conn)?;

    let mut new_count = 0;
    for a in &anomalies {
        if anomaly::store_anomaly(&conn, a).is_ok() {
            new_count += 1;
        }
    }

    info!(target: "4da::anomaly", found = anomalies.len(), stored = new_count, "Anomaly detection with results complete");
    Ok(anomalies)
}

/// Run background behavior decay - called daily by scheduler
pub async fn run_background_behavior_decay() -> Result<serde_json::Value> {
    let ace = get_ace_engine()?;

    // Apply decay to behavior signals
    let decayed_count = ace.apply_behavior_decay()?;

    info!(
        target: "4da::decay",
        signals_decayed = decayed_count,
        "Background behavior decay applied"
    );

    // Auto-tune relevance threshold based on engagement rate
    let threshold_adjusted = {
        let current = get_relevance_threshold();
        if let Some(new_threshold) = ace.compute_threshold_adjustment(current) {
            set_relevance_threshold(new_threshold);
            ace.store_threshold(new_threshold);
            info!(
                target: "4da::threshold",
                old = current,
                new = new_threshold,
                "Auto-tuned relevance threshold"
            );
            Some(new_threshold)
        } else {
            None
        }
    };

    Ok(serde_json::json!({
        "signals_decayed": decayed_count,
        "threshold_adjusted": threshold_adjusted,
        "current_threshold": get_relevance_threshold(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

// Update, Digest, and AI Briefing commands are in digest_commands.rs

// ============================================================================
// MCP Score Autopsy Command
// ============================================================================

/// Execute score autopsy - native implementation using AnalysisState data
/// Provides deep breakdown of why an item scored the way it did
#[tauri::command]
pub(crate) async fn mcp_score_autopsy(
    item_id: u64,
    source_type: String,
    _synthesize: bool,
    _compact: bool,
) -> Result<serde_json::Value> {
    info!(
        target: "4da::autopsy",
        item_id = item_id,
        source_type = %source_type,
        "Score autopsy requested"
    );

    // Find the item in analysis results
    let state = get_analysis_state();
    let guard = state.lock();
    let results = guard
        .results
        .as_ref()
        .ok_or("No analysis results available. Run an analysis first.")?;

    let item = results
        .iter()
        .find(|r| r.id == item_id)
        .ok_or_else(|| format!("Item {} not found in analysis results", item_id))?;

    // Get item metadata from DB
    let db = get_database()?;
    let db_item = db.get_source_item_by_id(item_id as i64).ok().flatten();

    let age_hours = db_item
        .as_ref()
        .map(|i| (chrono::Utc::now() - i.created_at).num_minutes() as f64 / 60.0)
        .unwrap_or(0.0);

    let created_at = db_item
        .as_ref()
        .map(|i| i.created_at.to_rfc3339())
        .unwrap_or_default();

    // Build component breakdown from ScoreBreakdown
    let mut components = Vec::new();
    if let Some(ref bd) = item.score_breakdown {
        components.push(serde_json::json!({
            "name": "Context Match",
            "raw_value": bd.context_score,
            "weight": 0.5,
            "contribution": bd.context_score * 0.5,
            "explanation": if bd.context_score > 0.2 {
                format!("Strong match with your project files ({:.0}% similarity)", bd.context_score * 100.0)
            } else if bd.context_score > 0.05 {
                format!("Weak match with project context ({:.0}%)", bd.context_score * 100.0)
            } else {
                "No significant match with indexed project files".to_string()
            }
        }));

        components.push(serde_json::json!({
            "name": "Interest Match",
            "raw_value": bd.interest_score,
            "weight": 0.5,
            "contribution": bd.interest_score * 0.5,
            "explanation": if bd.interest_score > 0.3 {
                format!("Closely matches your declared interests ({:.0}%)", bd.interest_score * 100.0)
            } else if bd.interest_score > 0.1 {
                format!("Partial interest match ({:.0}%)", bd.interest_score * 100.0)
            } else {
                "Low alignment with declared interests".to_string()
            }
        }));

        if bd.ace_boost > 0.01 {
            components.push(serde_json::json!({
                "name": "ACE Semantic Boost",
                "raw_value": bd.ace_boost,
                "weight": 1.0,
                "contribution": bd.ace_boost,
                "explanation": format!("Boosted by ACE context engine topics/tech (+{:.0}%)", bd.ace_boost * 100.0)
            }));
        }

        if (bd.affinity_mult - 1.0).abs() > 0.01 {
            let direction = if bd.affinity_mult > 1.0 {
                "boosted"
            } else {
                "reduced"
            };
            components.push(serde_json::json!({
                "name": "Learned Affinity",
                "raw_value": bd.affinity_mult,
                "weight": 1.0,
                "contribution": bd.affinity_mult - 1.0,
                "explanation": format!("Score {} by learned topic preferences (x{:.2})", direction, bd.affinity_mult)
            }));
        }

        if bd.anti_penalty > 0.01 {
            components.push(serde_json::json!({
                "name": "Anti-Topic Penalty",
                "raw_value": bd.anti_penalty,
                "weight": 1.0,
                "contribution": -bd.anti_penalty,
                "explanation": format!("Penalized by anti-topic filter (-{:.0}%)", bd.anti_penalty * 100.0)
            }));
        }

        if (bd.freshness_mult - 1.0).abs() > 0.01 {
            let label = if bd.freshness_mult > 1.0 {
                "Freshness bonus"
            } else {
                "Staleness decay"
            };
            components.push(serde_json::json!({
                "name": "Temporal Freshness",
                "raw_value": bd.freshness_mult,
                "weight": 1.0,
                "contribution": bd.freshness_mult - 1.0,
                "explanation": format!("{}: item is {:.0}h old (x{:.2})", label, age_hours, bd.freshness_mult)
            }));
        }
    }

    // Build matching context from ACE
    let ace_ctx = scoring::get_ace_context();
    let topics = extract_topics(&item.title, "");

    let matching_interests: Vec<String> = {
        let ctx_engine = get_context_engine().ok();
        ctx_engine
            .and_then(|ce| ce.get_static_identity().ok())
            .map(|id| {
                id.interests
                    .iter()
                    .filter(|i| {
                        let int_lower = i.topic.to_lowercase();
                        topics.iter().any(|t| {
                            let tl = t.to_lowercase();
                            tl.contains(&int_lower) || int_lower.contains(&tl)
                        })
                    })
                    .map(|i| i.topic.clone())
                    .collect()
            })
            .unwrap_or_default()
    };

    let matching_tech: Vec<String> = ace_ctx
        .detected_tech
        .iter()
        .filter(|t| {
            let tl = t.to_lowercase();
            topics.iter().any(|topic| {
                let topic_lower = topic.to_lowercase();
                topic_lower.contains(&tl) || tl.contains(&topic_lower)
            })
        })
        .cloned()
        .collect();

    let matching_active: Vec<String> = ace_ctx
        .active_topics
        .iter()
        .filter(|t| {
            topics.iter().any(|topic| {
                let topic_lower = topic.to_lowercase();
                topic_lower.contains(t.as_str()) || t.contains(&topic_lower)
            })
        })
        .cloned()
        .collect();

    let matching_affinities: Vec<String> = ace_ctx
        .topic_affinities
        .iter()
        .filter(|(_, (score, _))| *score > 0.3)
        .filter(|(topic, _)| {
            topics.iter().any(|t| {
                let tl = t.to_lowercase();
                tl.contains(topic.as_str()) || topic.contains(&tl)
            })
        })
        .map(|(topic, (score, _))| format!("{} ({:+.0}%)", topic, score * 100.0))
        .collect();

    // Find similar items for comparison (items with close scores)
    let similar_items: Vec<serde_json::Value> = results
        .iter()
        .filter(|r| r.id != item_id && r.relevant)
        .map(|r| {
            let diff = r.top_score - item.top_score;
            let key_diff = if diff.abs() < 0.05 {
                "Very similar score - different content matched".to_string()
            } else if diff > 0.0 {
                if r.context_score > item.context_score + 0.1 {
                    "Higher context match with your project files".to_string()
                } else if r.interest_score > item.interest_score + 0.1 {
                    "Better alignment with declared interests".to_string()
                } else {
                    "Stronger overall relevance signals".to_string()
                }
            } else if item.context_score > r.context_score + 0.1 {
                "This item has stronger project context match".to_string()
            } else {
                "This item has stronger interest alignment".to_string()
            };
            (r, diff, key_diff)
        })
        .take(3)
        .map(|(r, diff, key_diff)| {
            serde_json::json!({
                "id": r.id,
                "title": r.title,
                "score": r.top_score,
                "score_difference": diff,
                "key_difference": key_diff
            })
        })
        .collect();

    // Generate recommendations
    let mut recommendations = Vec::new();
    if item.context_score < 0.1 {
        recommendations.push("Index more project files to improve context matching. Add directories in Settings > Context.".to_string());
    }
    if item.interest_score < 0.1 {
        recommendations.push(
            "Add more interests in Settings > Interests to improve matching for this topic area."
                .to_string(),
        );
    }
    if matching_tech.is_empty() {
        recommendations.push("This item doesn't match your detected tech stack. If it's relevant, the ACE engine will learn from your interaction.".to_string());
    }
    if item.top_score < 0.35 && !item.relevant {
        recommendations.push("This item fell below the relevance threshold. Save items like this to train the system to surface similar content.".to_string());
    }

    // Build narrative
    let narrative = build_autopsy_narrative(item, &matching_tech, &matching_active, age_hours);

    Ok(serde_json::json!({
        "item": {
            "id": item.id,
            "title": item.title,
            "url": item.url,
            "source_type": item.source_type,
            "created_at": created_at,
            "age_hours": age_hours
        },
        "final_score": item.top_score,
        "components": components,
        "matching_context": {
            "interests": matching_interests,
            "tech_stack": matching_tech,
            "active_topics": matching_active,
            "learned_affinities": matching_affinities,
            "exclusions_hit": item.excluded_by.as_ref().map(|e| vec![e.clone()]).unwrap_or_else(Vec::<String>::new)
        },
        "similar_items": similar_items,
        "recommendations": recommendations,
        "narrative": narrative
    }))
}

/// Build a human-readable narrative for the score autopsy
fn build_autopsy_narrative(
    item: &SourceRelevance,
    matching_tech: &[String],
    matching_active: &[String],
    age_hours: f64,
) -> String {
    let mut parts = Vec::new();

    // Score assessment
    let score_pct = (item.top_score * 100.0) as u32;
    if item.top_score >= 0.6 {
        parts.push(format!("This item scored {}% - a strong match.", score_pct));
    } else if item.top_score >= 0.35 {
        parts.push(format!(
            "This item scored {}% - above the relevance threshold.",
            score_pct
        ));
    } else {
        parts.push(format!(
            "This item scored {}% - below the relevance threshold of 35%.",
            score_pct
        ));
    }

    // Context explanation
    if item.context_score > 0.3 {
        parts.push("It closely matches code you're actively working on.".to_string());
    } else if item.context_score > 0.1 {
        parts.push("It has some overlap with your project files.".to_string());
    }

    // Interest explanation
    if item.interest_score > 0.3 {
        parts.push("It strongly aligns with your declared interests.".to_string());
    } else if item.interest_score > 0.1 {
        parts.push("It partially matches your interests.".to_string());
    }

    // Tech stack
    if !matching_tech.is_empty() {
        parts.push(format!(
            "It mentions {} which is in your tech stack.",
            matching_tech.join(", ")
        ));
    }

    // Active topics
    if !matching_active.is_empty() {
        parts.push(format!(
            "It relates to topics you've been active in: {}.",
            matching_active.join(", ")
        ));
    }

    // Freshness
    if age_hours < 2.0 {
        parts.push("It was discovered very recently and received a freshness boost.".to_string());
    } else if age_hours > 36.0 {
        parts.push(format!(
            "It's {:.0} hours old, so its score was slightly reduced for staleness.",
            age_hours
        ));
    }

    // Signal info
    if let Some(ref sig) = item.signal_type {
        let label = match sig.as_str() {
            "security_alert" => "a security alert",
            "breaking_change" => "a breaking change notification",
            "tool_discovery" => "a new tool/library discovery",
            "tech_trend" => "a technology trend",
            "learning" => "a learning resource",
            "competitive_intel" => "competitive intelligence",
            _ => "a classified signal",
        };
        parts.push(format!(
            "It was classified as {} with {} priority.",
            label,
            item.signal_priority.as_deref().unwrap_or("unknown")
        ));
    }

    parts.join(" ")
}

// ============================================================================
// Product Hardening Commands
// ============================================================================

/// Get registered sources
#[tauri::command]
pub(crate) async fn get_sources() -> Result<Vec<serde_json::Value>> {
    let registry = get_source_registry();
    let guard = registry.lock();

    let sources: Vec<serde_json::Value> = guard
        .sources()
        .iter()
        .map(|s| {
            serde_json::json!({
                "type": s.source_type(),
                "name": s.name(),
                "enabled": s.config().enabled,
                "max_items": s.config().max_items,
                "fetch_interval_secs": s.config().fetch_interval_secs
            })
        })
        .collect();

    Ok(sources)
}
// Analysis functions (start_background_analysis, run_multi_source_analysis, etc.) are in analysis.rs
// Settings and Context Engine commands are in settings_commands.rs
// ACE commands, PASIFA helpers, and auto-seeding are in ace_commands.rs
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Export current analysis results in specified format
#[tauri::command]
pub(crate) async fn export_results(format: String) -> Result<String> {
    let state = get_analysis_state();
    let guard = state.lock();

    let results = match &guard.results {
        Some(r) => r,
        None => return Err("No analysis results to export".into()),
    };

    let relevant: Vec<&SourceRelevance> = results.iter().filter(|r| r.relevant).collect();

    match format.as_str() {
        "markdown" => {
            let mut md = String::from("# 4DA Analysis Results\n\n");
            md.push_str(&format!(
                "**Generated:** {}\n",
                chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
            ));
            md.push_str(&format!(
                "**Total items:** {} ({} relevant)\n\n",
                results.len(),
                relevant.len()
            ));
            md.push_str("---\n\n");
            for item in &relevant {
                let score_pct = (item.top_score * 100.0) as u32;
                md.push_str(&format!("### {} ({}%)\n", item.title, score_pct));
                if let Some(ref url) = item.url {
                    md.push_str(&format!("- **URL:** {}\n", url));
                }
                md.push_str(&format!("- **Source:** {}\n", item.source_type));
                if let Some(ref explanation) = item.explanation {
                    md.push_str(&format!("- **Why:** {}\n", explanation));
                }
                md.push('\n');
            }
            Ok(md)
        }
        "text" => {
            let mut text = format!(
                "4DA Analysis Results ({})\n",
                chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
            );
            text.push_str(&format!(
                "{} items, {} relevant\n\n",
                results.len(),
                relevant.len()
            ));
            for item in &relevant {
                let score_pct = (item.top_score * 100.0) as u32;
                text.push_str(&format!(
                    "[{}%] {} ({})\n",
                    score_pct, item.title, item.source_type
                ));
                if let Some(ref url) = item.url {
                    text.push_str(&format!("  {}\n", url));
                }
            }
            Ok(text)
        }
        "html" => {
            let mut html = String::from("<html><head><title>4DA Analysis Results</title></head><body style='font-family:sans-serif;background:#0A0A0A;color:#fff;padding:2rem'>");
            html.push_str(&format!(
                "<h1>4DA Analysis Results</h1><p>{} items, {} relevant</p><hr>",
                results.len(),
                relevant.len()
            ));
            for item in &relevant {
                let score_pct = (item.top_score * 100.0) as u32;
                html.push_str("<div style='margin:1rem 0;padding:1rem;background:#141414;border-radius:8px;border:1px solid #2A2A2A'>");
                html.push_str(&format!("<strong>{}%</strong> ", score_pct));
                if let Some(ref url) = item.url {
                    html.push_str(&format!(
                        "<a href='{}' style='color:#D4AF37'>{}</a>",
                        escape_html(url),
                        escape_html(&item.title)
                    ));
                } else {
                    html.push_str(&escape_html(&item.title));
                }
                html.push_str(&format!(
                    " <span style='color:#666'>({})</span>",
                    escape_html(&item.source_type)
                ));
                html.push_str("</div>");
            }
            html.push_str("</body></html>");
            Ok(html)
        }
        "digest" => {
            // Rich shareable digest with Developer DNA context
            let dna = developer_dna::generate_dna().ok();
            let now = chrono::Utc::now();
            let date_str = now.format("%B %d, %Y").to_string();

            let mut md = format!("# 4DA Intelligence Digest — {}\n\n", date_str);

            // Developer identity header
            if let Some(ref dna) = dna {
                md.push_str(&format!("> **{}**", dna.identity_summary));
                if !dna.primary_stack.is_empty() {
                    md.push_str(&format!(" | Stack: {}", dna.primary_stack.join(", ")));
                }
                md.push('\n');
            }
            md.push('\n');

            // Stats bar
            let high_signal = relevant.iter().filter(|r| r.top_score >= 0.5).count();
            let rejection_pct = if !results.is_empty() {
                ((1.0 - relevant.len() as f64 / results.len() as f64) * 100.0) as u32
            } else {
                0
            };
            md.push_str(&format!(
                "**{}** items scored | **{}** relevant | **{}** high-signal | **{}%** filtered as noise\n\n",
                results.len(), relevant.len(), high_signal, rejection_pct
            ));
            md.push_str("---\n\n");

            // High-signal section
            let high: Vec<&&SourceRelevance> =
                relevant.iter().filter(|r| r.top_score >= 0.5).collect();
            if !high.is_empty() {
                md.push_str("## High-Signal\n\n");
                for item in &high {
                    let score_pct = (item.top_score * 100.0) as u32;
                    if let Some(ref url) = item.url {
                        md.push_str(&format!("- **[{}]({})** — {}%", item.title, url, score_pct));
                    } else {
                        md.push_str(&format!("- **{}** — {}%", item.title, score_pct));
                    }
                    if let Some(ref explanation) = item.explanation {
                        md.push_str(&format!(" — {}", explanation));
                    }
                    md.push('\n');
                }
                md.push('\n');
            }

            // Group remaining by source
            let mut by_source: std::collections::HashMap<&str, Vec<&&SourceRelevance>> =
                std::collections::HashMap::new();
            for item in &relevant {
                if item.top_score < 0.5 {
                    by_source
                        .entry(item.source_type.as_str())
                        .or_default()
                        .push(item);
                }
            }

            if !by_source.is_empty() {
                fn source_label(s: &str) -> &str {
                    match s {
                        "hackernews" => "Hacker News",
                        "reddit" => "Reddit",
                        "arxiv" => "arXiv",
                        "github" => "GitHub",
                        "producthunt" => "Product Hunt",
                        "youtube" => "YouTube",
                        "twitter" => "Twitter/X",
                        "rss" => "RSS",
                        "devto" => "Dev.to",
                        "lobsters" => "Lobsters",
                        other => other,
                    }
                }

                // Sort sources by item count descending
                let mut sources: Vec<_> = by_source.into_iter().collect();
                sources.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

                for (source, items) in &sources {
                    md.push_str(&format!(
                        "## {} ({})\n\n",
                        source_label(source),
                        items.len()
                    ));
                    for item in items.iter().take(8) {
                        let score_pct = (item.top_score * 100.0) as u32;
                        if let Some(ref url) = item.url {
                            md.push_str(&format!("- [{}]({}) — {}%\n", item.title, url, score_pct));
                        } else {
                            md.push_str(&format!("- {} — {}%\n", item.title, score_pct));
                        }
                    }
                    if items.len() > 8 {
                        md.push_str(&format!("- *...+{} more*\n", items.len() - 8));
                    }
                    md.push('\n');
                }
            }

            // Footer
            md.push_str("---\n\n");
            md.push_str("*Generated by [4DA](https://github.com/4da-dev/4da-home) — The internet, scored for you.*\n");

            Ok(md)
        }
        _ => Err(format!(
            "Unknown format: {}. Use 'markdown', 'text', 'html', or 'digest'",
            format
        )
        .into()),
    }
}

// ============================================================================
// Diagnostics
// ============================================================================

#[tauri::command]
pub(crate) async fn get_diagnostics() -> Result<crate::diagnostics::DiagnosticsSnapshot> {
    let db = get_database()?;
    let db_path = db.db_path().to_path_buf();
    Ok(crate::diagnostics::collect_diagnostics(db, &db_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SourceRelevance;

    fn make_item(score: f32, context: f32, interest: f32) -> SourceRelevance {
        SourceRelevance {
            id: 1,
            title: "Test Item".to_string(),
            url: Some("https://example.com".to_string()),
            top_score: score,
            matches: vec![],
            relevant: score >= 0.35,
            context_score: context,
            interest_score: interest,
            excluded: false,
            excluded_by: None,
            source_type: "hackernews".to_string(),
            explanation: None,
            confidence: None,
            score_breakdown: None,
            signal_type: None,
            signal_priority: None,
            signal_action: None,
            signal_triggers: None,
            signal_horizon: None,
            similar_count: 0,
            similar_titles: vec![],
            serendipity: false,
            streets_engine: None,
            decision_window_match: None,
            decision_boost_applied: 0.0,
        }
    }

    #[test]
    fn escape_html_handles_ampersand() {
        assert_eq!(escape_html("a & b"), "a &amp; b");
    }

    #[test]
    fn escape_html_handles_angle_brackets() {
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
    }

    #[test]
    fn escape_html_handles_quotes() {
        assert_eq!(escape_html(r#"say "hello""#), "say &quot;hello&quot;");
    }

    #[test]
    fn escape_html_handles_single_quotes() {
        assert_eq!(escape_html("it's"), "it&#x27;s");
    }

    #[test]
    fn escape_html_preserves_safe_text() {
        assert_eq!(escape_html("hello world"), "hello world");
    }

    #[test]
    fn escape_html_handles_all_special_chars() {
        assert_eq!(
            escape_html("<div class=\"a\" data-x='b'>&</div>"),
            "&lt;div class=&quot;a&quot; data-x=&#x27;b&#x27;&gt;&amp;&lt;/div&gt;"
        );
    }

    #[test]
    fn narrative_strong_match_above_60_percent() {
        let item = make_item(0.75, 0.0, 0.0);
        let result = build_autopsy_narrative(&item, &[], &[], 10.0);
        assert!(result.contains("strong match"));
        assert!(result.contains("75%"));
    }

    #[test]
    fn narrative_above_threshold() {
        let item = make_item(0.45, 0.0, 0.0);
        let result = build_autopsy_narrative(&item, &[], &[], 10.0);
        assert!(result.contains("above the relevance threshold"));
    }

    #[test]
    fn narrative_below_threshold() {
        let item = make_item(0.20, 0.0, 0.0);
        let result = build_autopsy_narrative(&item, &[], &[], 10.0);
        assert!(result.contains("below the relevance threshold"));
    }

    #[test]
    fn narrative_includes_tech_stack() {
        let item = make_item(0.50, 0.0, 0.0);
        let tech = vec!["Rust".to_string(), "TypeScript".to_string()];
        let result = build_autopsy_narrative(&item, &tech, &[], 10.0);
        assert!(result.contains("Rust, TypeScript"));
        assert!(result.contains("tech stack"));
    }

    #[test]
    fn narrative_includes_active_topics() {
        let item = make_item(0.50, 0.0, 0.0);
        let active = vec!["WebAssembly".to_string()];
        let result = build_autopsy_narrative(&item, &[], &active, 10.0);
        assert!(result.contains("WebAssembly"));
        assert!(result.contains("active in"));
    }

    #[test]
    fn narrative_freshness_boost() {
        let item = make_item(0.50, 0.0, 0.0);
        let result = build_autopsy_narrative(&item, &[], &[], 1.0);
        assert!(result.contains("freshness boost"));
    }

    #[test]
    fn narrative_staleness_penalty() {
        let item = make_item(0.50, 0.0, 0.0);
        let result = build_autopsy_narrative(&item, &[], &[], 48.0);
        assert!(result.contains("staleness"));
    }

    #[test]
    fn narrative_context_match() {
        let item = make_item(0.50, 0.5, 0.0);
        let result = build_autopsy_narrative(&item, &[], &[], 10.0);
        assert!(result.contains("actively working on"));
    }

    #[test]
    fn narrative_interest_match() {
        let item = make_item(0.50, 0.0, 0.5);
        let result = build_autopsy_narrative(&item, &[], &[], 10.0);
        assert!(result.contains("declared interests"));
    }
}
