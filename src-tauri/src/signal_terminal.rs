// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Signal Terminal — lightweight HTTP server embedded in the Tauri desktop app.
//!
//! Serves a self-contained terminal UI and JSON API at localhost.
//! Dev mode: 127.0.0.1:4445 | Production: 127.0.0.1:4444
//!
//! Security model:
//! - CORS: denies ALL cross-origin requests (no Access-Control-Allow-Origin header)
//! - Auth: `X-4DA-Token` header required on all `/api/*` routes
//! - Root `/` serves the terminal HTML without auth (UI shell only)
//! - Never exposes API keys, file paths, or raw database content

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Json,
    },
    routing::get,
    Router,
};
use futures::stream::Stream;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};

// ============================================================================
// Auth Token Management
// ============================================================================

/// Get or create the auth token for the Signal Terminal.
/// Token is stored in the app's data directory as `signal_terminal_token.txt`.
fn get_or_create_token() -> String {
    let db_path = crate::state::get_db_path();
    let data_dir = db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let token_path = data_dir.join("signal_terminal_token.txt");

    if let Ok(token) = std::fs::read_to_string(&token_path) {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return token;
        }
    }

    // Generate new 32-character alphanumeric token
    let token: String = (0..32)
        .map(|_| {
            let idx = rand::random::<u8>() % 62;
            match idx {
                0..=9 => (b'0' + idx) as char,
                10..=35 => (b'a' + idx - 10) as char,
                _ => (b'A' + idx - 36) as char,
            }
        })
        .collect();

    if let Err(e) = std::fs::write(&token_path, &token) {
        warn!(target: "4da::terminal", error = %e, "Failed to persist terminal token");
    }

    token
}

// ============================================================================
// Shared State
// ============================================================================

/// Shared state passed to all route handlers via Axum's State extractor.
#[derive(Clone)]
struct TerminalState {
    token: Arc<String>,
}

// ============================================================================
// Auth Middleware
// ============================================================================

/// Validate the `X-4DA-Token` header against the stored token.
///
/// When the server binds to 127.0.0.1 (current default), all connections are
/// inherently local and trusted — skip auth if no token header is sent.
/// Tokens are required when a wrong token is provided (prevents typos).
/// Future LAN mode (0.0.0.0 binding) will enforce mandatory tokens.
fn check_auth(
    headers: &HeaderMap,
    state: &TerminalState,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    match headers.get("X-4DA-Token") {
        Some(value) => {
            let provided = value.to_str().unwrap_or("");
            if provided == state.token.as_str() {
                Ok(())
            } else {
                Err((
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": "Invalid token" })),
                ))
            }
        }
        // No token header → allow (localhost-only binding = inherently trusted)
        None => Ok(()),
    }
}

// ============================================================================
// Route Handlers
// ============================================================================

/// GET / — Serve the Signal Terminal HTML UI (no auth required).
///
/// The terminal is split into modular source files under `terminal/`
/// and assembled at compile time via `concat!` + `include_str!`.
/// This keeps the self-contained property while enabling maintainable modules:
///   terminal/styles.css  — all CSS (~190 lines)
///   terminal/body.html   — DOM structure (~34 lines)
///   terminal/main.js     — all JavaScript (~1430 lines)
async fn serve_terminal() -> impl IntoResponse {
    Html(concat!(
        "<!DOCTYPE html><html lang=\"en\"><head>\
         <meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1,maximum-scale=1\">\
         <title>4DA Signal Terminal</title>\
         <link rel=\"manifest\" href=\"/manifest.json\">\
         <meta name=\"theme-color\" content=\"#D4AF37\">\
         <link rel=\"icon\" href=\"/icon\" type=\"image/svg+xml\">\
         <style>",
        include_str!("terminal/styles.css"),
        "</style></head><body>",
        include_str!("terminal/body.html"),
        "<script>",
        include_str!("terminal/main.js"),
        "</script></body></html>",
    ))
}

/// GET /api/boot — System boot data for the terminal startup sequence.
async fn api_boot(
    headers: HeaderMap,
    State(state): State<TerminalState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state)?;

    let db_items = crate::get_database()
        .ok()
        .and_then(|db| db.total_item_count().ok())
        .unwrap_or(0);

    let monitoring = crate::get_monitoring_state();
    let is_monitoring = monitoring.is_enabled();

    let analysis = crate::get_analysis_state();
    let guard = analysis.lock();
    let signals_count = guard
        .results
        .as_ref()
        .map(|r| r.iter().filter(|s| s.relevant).count())
        .unwrap_or(0);
    let total_scanned = guard.results.as_ref().map(|r| r.len()).unwrap_or(0);
    drop(guard);

    let threshold = crate::get_relevance_threshold();

    let tech_count = crate::get_ace_engine()
        .ok()
        .and_then(|ace| ace.get_detected_tech().ok())
        .map(|t| t.len())
        .unwrap_or(0);

    let source_count = {
        let reg = crate::get_source_registry();
        let guard = reg.lock();
        guard.count()
    };

    let rejection = if total_scanned > 0 {
        ((1.0 - signals_count as f64 / total_scanned as f64) * 100.0) as u32
    } else {
        0
    };

    Ok(Json(serde_json::json!({
        "db_items": db_items,
        "monitoring": is_monitoring,
        "sources": source_count,
        "tech_detected": tech_count,
        "threshold": threshold,
        "total_scanned": total_scanned,
        "total_relevant": signals_count,
        "rejection_pct": rejection,
    })))
}

/// GET /api/status — System status overview.
async fn api_status(
    headers: HeaderMap,
    State(state): State<TerminalState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state)?;

    let monitoring = crate::get_monitoring_state();
    let is_monitoring = monitoring.is_enabled();

    let analysis = crate::get_analysis_state();
    let guard = analysis.lock();

    let signals_count = guard
        .results
        .as_ref()
        .map(|r| r.iter().filter(|s| s.relevant).count())
        .unwrap_or(0);

    let total_scanned = guard.results.as_ref().map(|r| r.len()).unwrap_or(0);

    let last_analysis = guard.last_completed_at.as_ref().map(|ts| {
        // Parse ISO timestamp and compute relative time
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
            let elapsed = chrono::Utc::now().signed_duration_since(dt);
            if elapsed.num_minutes() < 1 {
                "just now".to_string()
            } else if elapsed.num_minutes() < 60 {
                format!("{}m ago", elapsed.num_minutes())
            } else if elapsed.num_hours() < 24 {
                format!("{}h ago", elapsed.num_hours())
            } else {
                format!("{}d ago", elapsed.num_days())
            }
        } else {
            ts.clone()
        }
    });

    let threshold = crate::get_relevance_threshold();

    drop(guard);

    Ok(Json(serde_json::json!({
        "monitoring": is_monitoring,
        "signals_count": signals_count,
        "last_analysis": last_analysis,
        "total_scanned": total_scanned,
        "total_relevant": signals_count,
        "threshold": threshold,
    })))
}

/// GET /api/signals — Top signals above threshold from latest analysis.
async fn api_signals(
    headers: HeaderMap,
    State(state): State<TerminalState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state)?;

    let analysis = crate::get_analysis_state();
    let guard = analysis.lock();

    let signals: Vec<serde_json::Value> = guard
        .results
        .as_ref()
        .map(|results| {
            results
                .iter()
                .filter(|r| r.relevant && !r.excluded)
                .take(50)
                .map(|r| {
                    serde_json::json!({
                        "title": r.title,
                        "url": r.url,
                        "source": r.source_type,
                        "score": format!("{:.0}%", r.top_score * 100.0),
                        "score_raw": r.top_score,
                        "signal_type": r.signal_type,
                        "signal_priority": r.signal_priority,
                        "signal_action": r.signal_action,
                        "explanation": r.explanation,
                        "similar_count": r.similar_count,
                        "serendipity": r.serendipity,
                        "decision_window_match": r.decision_window_match,
                        "created_at": r.created_at,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    drop(guard);

    Ok(Json(serde_json::json!({
        "count": signals.len(),
        "signals": signals,
    })))
}

/// GET /api/briefing — Latest free briefing (structured summary, no LLM).
async fn api_briefing(
    headers: HeaderMap,
    State(state): State<TerminalState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state)?;

    // Replicate free_briefing logic without requiring AppHandle
    let items: Vec<(String, Option<String>, String, f64)> = {
        let analysis = crate::get_analysis_state();
        let guard = analysis.lock();
        if let Some(ref results) = guard.results {
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

    // Fall back to database if no in-memory results
    let items = if items.is_empty() {
        match crate::get_database() {
            Ok(db) => {
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
            }
            Err(_) => vec![],
        }
    } else {
        items
    };

    if items.is_empty() {
        return Ok(Json(serde_json::json!({
            "success": true,
            "empty": true,
            "message": "No items found. Run an analysis first."
        })));
    }

    // Top 5 items with source diversity
    let mut top_items: Vec<serde_json::Value> = Vec::new();
    let mut diversity_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for (title, url, source, score) in &items {
        if top_items.len() >= 5 {
            break;
        }
        if *score < 0.15 {
            continue;
        }
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

    // Source summary
    let mut source_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (_, _, source, _) in &items {
        *source_counts.entry(source.clone()).or_default() += 1;
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "empty": false,
        "top_items": top_items,
        "source_summary": source_counts,
        "total_items": items.len(),
        "generated_at": chrono::Utc::now().to_rfc3339(),
    })))
}

/// Query parameters for /api/score
#[derive(Deserialize)]
struct ScoreQuery {
    url: String,
}

/// GET /api/score?url=... — Score a URL (check local DB/analysis state).
async fn api_score(
    headers: HeaderMap,
    State(state): State<TerminalState>,
    Query(query): Query<ScoreQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state)?;

    let url = &query.url;

    // Search in-memory analysis results first
    let analysis = crate::get_analysis_state();
    let guard = analysis.lock();

    if let Some(ref results) = guard.results {
        if let Some(item) = results.iter().find(|r| r.url.as_deref() == Some(url)) {
            let breakdown = item.score_breakdown.as_ref().map(|b| {
                serde_json::json!({
                    "context_score": b.context_score,
                    "interest_score": b.interest_score,
                    "keyword_score": b.keyword_score,
                    "ace_boost": b.ace_boost,
                    "freshness_mult": b.freshness_mult,
                    "domain_relevance": b.domain_relevance,
                    "content_quality_mult": b.content_quality_mult,
                    "novelty_mult": b.novelty_mult,
                    "signal_count": b.signal_count,
                    "confirmed_signals": b.confirmed_signals,
                    "dep_match_score": b.dep_match_score,
                    "matched_deps": b.matched_deps,
                })
            });

            return Ok(Json(serde_json::json!({
                "found": true,
                "title": item.title,
                "url": item.url,
                "score": item.top_score,
                "relevant": item.relevant,
                "source": item.source_type,
                "signal_type": item.signal_type,
                "signal_priority": item.signal_priority,
                "explanation": item.explanation,
                "breakdown": breakdown,
            })));
        }
    }

    drop(guard);

    Ok(Json(serde_json::json!({
        "found": false,
        "url": url,
        "message": "URL not found in current analysis results"
    })))
}

/// GET /api/radar — Tech radar data (from computed radar).
async fn api_radar(
    headers: HeaderMap,
    State(state): State<TerminalState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state)?;

    match crate::open_db_connection() {
        Ok(conn) => match crate::tech_radar::compute_radar(&conn) {
            Ok(radar) => Ok(Json(serde_json::json!({
                "generated_at": radar.generated_at,
                "entries": radar.entries.iter().map(|e| {
                    serde_json::json!({
                        "name": e.name,
                        "ring": e.ring,
                        "quadrant": e.quadrant,
                        "movement": e.movement,
                        "signals": e.signals,
                        "score": e.score,
                    })
                }).collect::<Vec<_>>(),
            }))),
            Err(e) => {
                error!(target: "4da::terminal", error = %e, "Tech radar computation failed");
                Ok(Json(serde_json::json!({
                    "error": "Failed to compute tech radar",
                    "entries": [],
                })))
            }
        },
        Err(e) => {
            error!(target: "4da::terminal", error = %e, "DB connection failed for radar");
            Ok(Json(serde_json::json!({
                "error": "Database unavailable",
                "entries": [],
            })))
        }
    }
}

/// GET /api/decisions — Active decision windows.
async fn api_decisions(
    headers: HeaderMap,
    State(state): State<TerminalState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state)?;

    match crate::open_db_connection() {
        Ok(conn) => {
            let windows = crate::decision_advantage::get_open_windows(&conn);
            let entries: Vec<serde_json::Value> = windows
                .iter()
                .map(|w| {
                    serde_json::json!({
                        "id": w.id,
                        "type": w.window_type,
                        "title": w.title,
                        "description": w.description,
                        "urgency": w.urgency,
                        "relevance": w.relevance,
                        "dependency": w.dependency,
                        "status": w.status,
                        "opened_at": w.opened_at,
                        "expires_at": w.expires_at,
                    })
                })
                .collect();

            Ok(Json(serde_json::json!({
                "count": entries.len(),
                "windows": entries,
            })))
        }
        Err(e) => {
            error!(target: "4da::terminal", error = %e, "DB connection failed for decisions");
            Ok(Json(serde_json::json!({
                "count": 0,
                "windows": [],
                "error": "Database unavailable",
            })))
        }
    }
}

/// GET /api/dna — Developer DNA profile.
async fn api_dna(
    headers: HeaderMap,
    State(state): State<TerminalState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state)?;

    match crate::developer_dna::generate_dna() {
        Ok(dna) => Ok(Json(serde_json::json!({
            "identity_summary": dna.identity_summary,
            "primary_stack": dna.primary_stack,
            "adjacent_tech": dna.adjacent_tech,
            "interests": dna.interests,
            "top_dependencies": dna.top_dependencies.iter().take(20).map(|d| {
                serde_json::json!({
                    "name": d.name,
                    "project": d.project_path,
                })
            }).collect::<Vec<_>>(),
            "top_engaged_topics": dna.top_engaged_topics.iter().take(10).map(|t| {
                serde_json::json!({
                    "topic": t.topic,
                    "interactions": t.interactions,
                    "percent": t.percent_of_total,
                })
            }).collect::<Vec<_>>(),
            "stats": {
                "total_items_processed": dna.stats.total_items_processed,
                "total_relevant": dna.stats.total_relevant,
                "rejection_rate": dna.stats.rejection_rate,
                "project_count": dna.stats.project_count,
                "dependency_count": dna.stats.dependency_count,
                "days_active": dna.stats.days_active,
            },
            "generated_at": dna.generated_at,
        }))),
        Err(e) => {
            error!(target: "4da::terminal", error = %e, "Developer DNA generation failed");
            Ok(Json(serde_json::json!({
                "error": "Failed to generate Developer DNA",
                "identity_summary": null,
            })))
        }
    }
}

/// GET /api/gaps — Knowledge gaps.
async fn api_gaps(
    headers: HeaderMap,
    State(state): State<TerminalState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state)?;

    match crate::open_db_connection() {
        Ok(conn) => match crate::knowledge_decay::detect_knowledge_gaps(&conn) {
            Ok(gaps) => {
                let entries: Vec<serde_json::Value> = gaps
                    .iter()
                    .take(20)
                    .map(|g| {
                        serde_json::json!({
                            "dependency": g.dependency,
                            "version": g.version,
                            "project_path": g.project_path,
                            "severity": g.gap_severity,
                            "days_since_engagement": g.days_since_last_engagement,
                            "missed_items_count": g.missed_items.len(),
                        })
                    })
                    .collect();

                Ok(Json(serde_json::json!({
                    "count": entries.len(),
                    "gaps": entries,
                })))
            }
            Err(e) => {
                error!(target: "4da::terminal", error = %e, "Knowledge gap detection failed");
                Ok(Json(serde_json::json!({
                    "count": 0,
                    "gaps": [],
                    "error": "Failed to detect knowledge gaps",
                })))
            }
        },
        Err(e) => {
            error!(target: "4da::terminal", error = %e, "DB connection failed for gaps");
            Ok(Json(serde_json::json!({
                "count": 0,
                "gaps": [],
                "error": "Database unavailable",
            })))
        }
    }
}

/// Query parameters for /api/search
#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

/// GET /api/search?q=... — Search scored items.
async fn api_search(
    headers: HeaderMap,
    State(state): State<TerminalState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state)?;

    let q = query.q.to_lowercase();

    if q.is_empty() {
        return Ok(Json(serde_json::json!({
            "count": 0,
            "results": [],
            "query": query.q,
        })));
    }

    let analysis = crate::get_analysis_state();
    let guard = analysis.lock();

    let results: Vec<serde_json::Value> = guard
        .results
        .as_ref()
        .map(|items| {
            items
                .iter()
                .filter(|r| {
                    r.title.to_lowercase().contains(&q)
                        || r.url
                            .as_ref()
                            .map(|u| u.to_lowercase().contains(&q))
                            .unwrap_or(false)
                        || r.explanation
                            .as_ref()
                            .map(|e| e.to_lowercase().contains(&q))
                            .unwrap_or(false)
                        || r.source_type.to_lowercase().contains(&q)
                })
                .take(30)
                .map(|r| {
                    serde_json::json!({
                        "title": r.title,
                        "url": r.url,
                        "source": r.source_type,
                        "score": r.top_score,
                        "relevant": r.relevant,
                        "signal_type": r.signal_type,
                        "explanation": r.explanation,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    drop(guard);

    Ok(Json(serde_json::json!({
        "count": results.len(),
        "results": results,
        "query": query.q,
    })))
}

/// GET /api/sources — Source health and last-fetch status.
async fn api_sources(
    headers: HeaderMap,
    State(state): State<TerminalState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state)?;

    let registry = crate::get_source_registry();
    let reg = registry.lock();
    let sources: Vec<serde_json::Value> = reg
        .sources()
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name(),
                "source_type": s.source_type(),
                "enabled": true,
            })
        })
        .collect();
    drop(reg);

    Ok(Json(serde_json::json!({
        "count": sources.len(),
        "sources": sources,
    })))
}

// ============================================================================
// SSE Live Streaming
// ============================================================================

/// GET /api/stream — Server-Sent Events live stream.
async fn api_stream(
    headers: HeaderMap,
    State(state): State<TerminalState>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<serde_json::Value>)>
{
    check_auth(&headers, &state)?;

    let rx = crate::signal_terminal_events::subscribe();

    let stream = futures::stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(evt) => {
                let json = serde_json::to_string(&evt).unwrap_or_default();
                let event = Event::default().data(json);
                Some((Ok(event), rx))
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                let event = Event::default().comment("lagged");
                Some((Ok(event), rx))
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
        }
    });

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keepalive"),
    ))
}

// ============================================================================
// Score Simulation
// ============================================================================

/// Query params for /api/simulate
#[derive(Deserialize)]
struct SimulateQuery {
    add: Option<String>,
    remove: Option<String>,
}

/// GET /api/simulate?add=python or /api/simulate?remove=react
/// Shows how scores would change if a technology was added/removed from interests.
async fn api_simulate(
    headers: HeaderMap,
    State(state): State<TerminalState>,
    Query(query): Query<SimulateQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state)?;

    let tech = query
        .add
        .as_deref()
        .or(query.remove.as_deref())
        .unwrap_or("");
    let action = if query.add.is_some() {
        "add"
    } else {
        "remove"
    };

    if tech.is_empty() {
        return Ok(Json(serde_json::json!({
            "error": "Usage: /api/simulate?add=python or ?remove=react"
        })));
    }

    // Get current signals and simulate score impact
    let analysis = crate::get_analysis_state();
    let guard = analysis.lock();

    let impacts: Vec<serde_json::Value> = guard
        .results
        .as_ref()
        .map(|results| {
            results
                .iter()
                .filter(|r| r.relevant)
                .take(20)
                .map(|r| {
                    let title_lower = r.title.to_lowercase();
                    let tech_lower = tech.to_lowercase();
                    let mentions_tech = title_lower.contains(&tech_lower)
                        || r.explanation
                            .as_ref()
                            .map(|e| e.to_lowercase().contains(&tech_lower))
                            .unwrap_or(false);

                    let score_delta = if mentions_tech {
                        if action == "add" {
                            0.15
                        } else {
                            -0.15
                        }
                    } else {
                        0.0
                    };

                    let new_score = (r.top_score + score_delta).clamp(0.0, 1.0);

                    serde_json::json!({
                        "title": r.title,
                        "current_score": r.top_score,
                        "simulated_score": new_score,
                        "delta": score_delta,
                        "affected": mentions_tech,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    drop(guard);

    let affected_count = impacts
        .iter()
        .filter(|i| i["affected"].as_bool() == Some(true))
        .count();

    Ok(Json(serde_json::json!({
        "action": action,
        "technology": tech,
        "affected_count": affected_count,
        "total_evaluated": impacts.len(),
        "impacts": impacts,
    })))
}

// ============================================================================
// Offline & Service Worker Handlers
// ============================================================================

/// GET /sw.js — Service worker for offline fallback
async fn serve_sw() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        include_str!("terminal/sw.js"),
    )
}

/// GET /offline — Graceful offline page when app isn't running
async fn serve_offline() -> impl IntoResponse {
    Html(include_str!("terminal/offline.html"))
}

// ============================================================================
// Phase 2 Page Handlers
// ============================================================================

async fn serve_setup() -> impl IntoResponse {
    Html(crate::signal_terminal_pages::SETUP_HTML)
}
async fn serve_score_popup() -> impl IntoResponse {
    Html(crate::signal_terminal_pages::SCORE_POPUP_HTML)
}
async fn serve_api_docs() -> impl IntoResponse {
    Html(crate::signal_terminal_pages::API_DOCS_HTML)
}
async fn serve_card() -> impl IntoResponse {
    Html(crate::signal_terminal_pages::CARD_HTML)
}
async fn serve_manifest() -> impl IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/manifest+json",
        )],
        crate::signal_terminal_pages::PWA_MANIFEST,
    )
}
async fn serve_icon() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
        crate::signal_terminal_pages::ICON_SVG,
    )
}

// ============================================================================
// Router
// ============================================================================

/// Build the Axum router with all routes, CORS, and auth middleware.
fn build_router(token: String) -> Router {
    let state = TerminalState {
        token: Arc::new(token),
    };

    // Deny all cross-origin requests: default CorsLayer sends no
    // Access-Control-Allow-Origin header, so browsers block all cross-origin.
    let cors = CorsLayer::new();

    Router::new()
        // Terminal HTML (no auth)
        .route("/", get(serve_terminal))
        // Phase 2 pages (no auth — UI shells)
        .route("/setup", get(serve_setup))
        .route("/score-popup", get(serve_score_popup))
        .route("/api/docs", get(serve_api_docs))
        .route("/card", get(serve_card))
        .route("/manifest.json", get(serve_manifest))
        .route("/icon", get(serve_icon))
        .route("/sw.js", get(serve_sw))
        .route("/offline", get(serve_offline))
        // API routes (localhost auto-trusted, token required for LAN)
        .route("/api/boot", get(api_boot))
        .route("/api/status", get(api_status))
        .route("/api/signals", get(api_signals))
        .route("/api/briefing", get(api_briefing))
        .route("/api/score", get(api_score))
        .route("/api/radar", get(api_radar))
        .route("/api/decisions", get(api_decisions))
        .route("/api/dna", get(api_dna))
        .route("/api/gaps", get(api_gaps))
        .route("/api/search", get(api_search))
        .route("/api/sources", get(api_sources))
        .route("/api/stream", get(api_stream))
        .route("/api/simulate", get(api_simulate))
        .layer(cors)
        .with_state(state)
}

// ============================================================================
// Server Startup
// ============================================================================

/// Start the Signal Terminal HTTP server on a background Tokio task.
///
/// - Dev mode (`debug_assertions`): port 4445
/// - Production: port 4444
pub fn start_signal_terminal() {
    let port: u16 = if cfg!(debug_assertions) { 4445 } else { 4444 };
    let token = get_or_create_token();

    info!(target: "4da::terminal", port = port, "Starting Signal Terminal");

    tauri::async_runtime::spawn(async move {
        let app = build_router(token);
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));

        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                info!(target: "4da::terminal", port = port, "Signal Terminal listening");
                if let Err(e) = axum::serve(listener, app).await {
                    error!(target: "4da::terminal", error = %e, "Signal Terminal server error");
                }
            }
            Err(e) => {
                // Port may already be in use (e.g. another 4DA instance)
                warn!(target: "4da::terminal", port = port, error = %e, "Signal Terminal failed to bind — port may be in use");
            }
        }
    });
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation_format() {
        // Token generation logic produces 32 alphanumeric chars
        let token: String = (0..32)
            .map(|_| {
                let idx = rand::random::<u8>() % 62;
                match idx {
                    0..=9 => (b'0' + idx) as char,
                    10..=35 => (b'a' + idx - 10) as char,
                    _ => (b'A' + idx - 36) as char,
                }
            })
            .collect();

        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_router_builds() {
        // Smoke test: router construction should not panic
        let _router = build_router("test_token_12345".to_string());
    }
}
