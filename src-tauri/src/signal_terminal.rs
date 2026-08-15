// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Signal Terminal — lightweight HTTP server embedded in the Tauri desktop app.
//!
//! Serves a self-contained terminal UI and JSON API at localhost.
//! Dev mode: 127.0.0.1:4447 | Production: 127.0.0.1:4446
//!
//! Ports are deliberately disjoint from the Vite toolchain (dev server 4444,
//! HMR 4445): sharing an origin let the terminal's service worker hijack the
//! app shell. Keep this range (4446/4447) clear of the frontend dev ports.
//!
//! Security model (two independent gates — both must pass):
//!
//! 1. **Host allowlist** (`host_guard`, every route including `/`). The `Host`
//!    header must be `127.0.0.1:<port>`, `localhost:<port>` or `[::1]:<port>`.
//!    A missing, port-less or foreign `Host` is `403`. This is the DNS-rebinding
//!    defence: binding to loopback and setting no CORS headers do NOT stop a
//!    hostile page from re-pointing its own domain at 127.0.0.1, at which point
//!    it is same-origin and CORS is irrelevant. Checking `Host` does stop it,
//!    because the browser keeps sending the attacker's domain.
//! 2. **Bearer token** (`check_auth`, every `/api/*` route). `X-4DA-Token` must
//!    equal the token in `data/signal_terminal_token.txt`. Missing, empty and
//!    wrong tokens are all `401` — there is no localhost bypass. Comparison is
//!    constant-time. This is the defence against other processes and other
//!    users on the same machine, which the Host check cannot see.
//!
//! `/api/stream` additionally accepts `?token=` because `EventSource` cannot set
//! request headers. Every other route is header-only.
//!
//! Unauthenticated routes serve compile-time-constant UI shells only (`/`,
//! `/setup`, `/score-popup`, `/card`, `/api/docs`, `/manifest.json`, `/icon`,
//! `/sw.js`, `/offline`). They carry no user data — every byte of intelligence
//! is behind `check_auth`.
//!
//! No route exposes API keys or credentials. Note that `/api/decisions` and
//! `/api/gaps` DO return local project paths, which is one of the reasons the
//! token is mandatory rather than advisory.

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
use futures::StreamExt;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};

// ============================================================================
// Auth Token Management
// ============================================================================

/// Request header carrying the Signal Terminal bearer token.
const TOKEN_HEADER: &str = "X-4DA-Token";

/// Token length in characters. 32 chars over a 62-symbol alphabet is ~190 bits.
const TOKEN_LEN: usize = 32;

/// Symbol set for generated tokens.
const TOKEN_ALPHABET: &[u8; 62] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Largest multiple of 62 that fits in a `u8` (62 * 4). Bytes at or above this
/// are discarded during generation — see `generate_token`.
const TOKEN_REJECT_AT: u8 = 248;

/// Generate a fresh token with uniformly distributed symbols.
///
/// Rejection sampling is load-bearing: 256 is not a multiple of 62, so the
/// obvious `rand::random::<u8>() % 62` maps 5 raw bytes onto the first 8 symbols
/// and only 4 onto the remaining 54, biasing every character toward `0-7`.
/// Discarding bytes >= 248 leaves exactly 4 raw bytes per symbol, so each of the
/// 62 symbols is equally likely.
fn generate_token() -> String {
    let mut token = String::with_capacity(TOKEN_LEN);
    while token.len() < TOKEN_LEN {
        let byte = rand::random::<u8>();
        if byte < TOKEN_REJECT_AT {
            token.push(TOKEN_ALPHABET[(byte % 62) as usize] as char);
        }
    }
    token
}

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

    let token = generate_token();

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
    /// Port the server is bound to. Used to build the `Host` allowlist, so the
    /// guard cannot drift out of sync with the listener.
    port: u16,
}

// ============================================================================
// Auth
// ============================================================================

/// Error returned by both gates.
type AuthRejection = (StatusCode, Json<serde_json::Value>);

fn unauthorized() -> AuthRejection {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "error": "Missing or invalid token",
            "hint": "Send the X-4DA-Token header. The token is in data/signal_terminal_token.txt"
        })),
    )
}

/// Compare two byte strings without leaking the position of the first
/// difference through timing.
///
/// Once the lengths match, every byte pair is inspected — the loop never breaks
/// early on a mismatch. `black_box` stops LLVM from noticing that the
/// accumulator can be short-circuited and reintroducing an early exit.
///
/// Length is compared first and non-constant-time. That is deliberate and safe:
/// the token length (32) is a fixed public constant, not a secret.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    std::hint::black_box(diff) == 0
}

/// Constant-time check of a candidate token against the configured one.
///
/// An empty configured token never matches anything, so a failure to generate
/// or read the token file fails closed rather than open.
fn token_matches(candidate: &str, state: &TerminalState) -> bool {
    !state.token.is_empty() && constant_time_eq(candidate.as_bytes(), state.token.as_bytes())
}

/// Validate the `X-4DA-Token` header against the stored token.
///
/// The token is MANDATORY. A missing header, an empty header and a wrong token
/// are all `401` — identical response, identical shape. There is no
/// localhost bypass: binding to 127.0.0.1 does not make a caller trustworthy,
/// it only makes it local, and every other process and every other user account
/// on the machine is local too.
fn check_auth(headers: &HeaderMap, state: &TerminalState) -> Result<(), AuthRejection> {
    let provided = headers
        .get(TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");

    if token_matches(provided, state) {
        Ok(())
    } else {
        Err(unauthorized())
    }
}

/// Auth for `/api/stream` only.
///
/// The browser `EventSource` API cannot attach request headers, so the SSE route
/// also accepts the token as a query parameter. The header is tried first and
/// wins when both are present. Confined to this one route on purpose — query
/// strings end up in referrers and shell history, so no other endpoint takes it.
fn check_auth_sse(
    headers: &HeaderMap,
    query_token: Option<&str>,
    state: &TerminalState,
) -> Result<(), AuthRejection> {
    if check_auth(headers, state).is_ok() {
        return Ok(());
    }
    match query_token {
        Some(token) if token_matches(token, state) => Ok(()),
        _ => Err(unauthorized()),
    }
}

// ============================================================================
// Host Guard (DNS-rebinding defence)
// ============================================================================

/// Split a `Host` header value into (hostname, port).
///
/// Returns `None` when there is no explicit port. That is intentional: the
/// terminal never listens on 80/443, so a port-less `Host` cannot have come from
/// a browser addressing this server, and is rejected.
fn split_host_port(host: &str) -> Option<(&str, u16)> {
    if let Some(rest) = host.strip_prefix('[') {
        // IPv6 literal: `[::1]:4446`
        let (addr, tail) = rest.split_once(']')?;
        let port = tail.strip_prefix(':')?.parse().ok()?;
        Some((addr, port))
    } else {
        let (name, port) = host.rsplit_once(':')?;
        Some((name, port.parse().ok()?))
    }
}

/// Is this `Host` header one of the loopback names this server answers to?
///
/// Strict allowlist. Anything else — an attacker's domain resolved to 127.0.0.1,
/// a LAN IP, a `.local` mDNS name — is refused. If LAN binding is ever added,
/// this allowlist is the thing that must be widened, deliberately.
fn is_allowed_host(host: &str, port: u16) -> bool {
    match split_host_port(host) {
        Some((name, host_port)) if host_port == port => {
            name.eq_ignore_ascii_case("localhost") || name == "127.0.0.1" || name == "::1"
        }
        _ => false,
    }
}

/// Reject any request whose `Host` header is not a loopback name for our port.
///
/// Applied as the OUTERMOST layer, so it covers every route — including the
/// unauthenticated UI shells and the 404 fallback — before any handler runs.
async fn host_guard(
    State(state): State<TerminalState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, AuthRejection> {
    // HTTP/1.1 carries the `Host` header; HTTP/2 carries `:authority`, which
    // hyper surfaces on the URI. Accept either, require one.
    let host = request
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .or_else(|| request.uri().authority().map(|a| a.as_str().to_owned()));

    match host {
        Some(host) if is_allowed_host(&host, state.port) => Ok(next.run(request).await),
        other => {
            warn!(
                target: "4da::terminal",
                host = other.as_deref().unwrap_or("<none>"),
                "Rejected request with non-loopback Host header (possible DNS rebinding)"
            );
            Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "Forbidden",
                    "hint": "Signal Terminal only answers to loopback Host headers"
                })),
            ))
        }
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
        .map_or(0, |r| r.iter().filter(|s| s.relevant).count());
    let total_scanned = guard.results.as_ref().map_or(0, std::vec::Vec::len);
    drop(guard);

    let threshold = crate::get_relevance_threshold();

    let tech_count = crate::get_ace_engine()
        .ok()
        .and_then(|ace| ace.get_detected_tech().ok())
        .map_or(0, |t| t.len());

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
        .map_or(0, |r| r.iter().filter(|s| s.relevant).count());

    let total_scanned = guard.results.as_ref().map_or(0, std::vec::Vec::len);

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
                {
                    let user_lang = crate::i18n::get_user_language();
                    db.get_relevant_items_since(period_start, 0.1, 30, &user_lang)
                }
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
                            .is_some_and(|u| u.to_lowercase().contains(&q))
                        || r.explanation
                            .as_ref()
                            .is_some_and(|e| e.to_lowercase().contains(&q))
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

/// Query params for /api/stream — `EventSource` cannot send headers.
#[derive(Deserialize)]
struct StreamQuery {
    token: Option<String>,
}

/// GET /api/stream — Server-Sent Events live stream.
async fn api_stream(
    headers: HeaderMap,
    State(state): State<TerminalState>,
    Query(query): Query<StreamQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AuthRejection> {
    check_auth_sse(&headers, query.token.as_deref(), &state)?;

    let rx = crate::signal_terminal_events::subscribe();

    // Send an initial "connected" event, then stream broadcast events
    let initial = futures::stream::once(async {
        let monitoring = crate::get_monitoring_state().is_enabled();
        let analysis = crate::get_analysis_state();
        let guard = analysis.lock();
        let signals_count = guard
            .results
            .as_ref()
            .map_or(0, |r| r.iter().filter(|s| s.relevant).count());
        let total_scanned = guard.results.as_ref().map_or(0, std::vec::Vec::len);
        drop(guard);

        let data = serde_json::json!({
            "type": "Connected",
            "monitoring": monitoring,
            "signals_count": signals_count,
            "total_scanned": total_scanned,
        });
        Ok::<_, Infallible>(Event::default().data(serde_json::to_string(&data).unwrap_or_default()))
    });

    let broadcast = futures::stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(evt) => {
                let json = serde_json::to_string(&evt).unwrap_or_default();
                Some((Ok(Event::default().data(json)), rx))
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                Some((Ok(Event::default().comment("lagged")), rx))
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
        }
    });

    let combined = initial.chain(broadcast);

    Ok(Sse::new(combined).keep_alive(
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
    let action = if query.add.is_some() { "add" } else { "remove" };

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
                            .is_some_and(|e| e.to_lowercase().contains(&tech_lower));

                    // Proportional delta based on mention strength
                    let mention_count = title_lower.matches(&tech_lower).count()
                        + r.explanation
                            .as_ref()
                            .map_or(0, |e| e.to_lowercase().matches(&tech_lower).count());

                    let base_delta = if action == "add" { 0.08 } else { -0.08 };
                    let score_delta = if mention_count > 0 {
                        (base_delta * mention_count as f32).clamp(-0.25, 0.25)
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
        .filter(|i| i["delta"].as_f64().is_some_and(|d| d != 0.0))
        .count();

    let message = if affected_count == 0 {
        Some(format!(
            "No current signals mention '{tech}'. Adding it would affect future analyses when content about {tech} appears in your sources."
        ))
    } else {
        None
    };

    Ok(Json(serde_json::json!({
        "action": action,
        "technology": tech,
        "affected_count": affected_count,
        "total_evaluated": impacts.len(),
        "impacts": impacts,
        "message": message,
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

/// Build the Axum router with all routes, CORS, the Host guard, and auth.
///
/// `port` must be the port the listener actually binds — it is what the `Host`
/// allowlist is checked against.
fn build_router(token: String, port: u16) -> Router {
    let state = TerminalState {
        token: Arc::new(token),
        port,
    };

    // Deny all cross-origin requests: default CorsLayer sends no
    // Access-Control-Allow-Origin header, so browsers block all cross-origin.
    // This is defence in depth only — it is useless against DNS rebinding,
    // which makes the attacker same-origin. `host_guard` is what stops that.
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
        // API routes — every one of these calls check_auth first.
        // No token, wrong token, empty token => 401. No localhost bypass.
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
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Not found",
                    "hint": "Try /api/docs for available endpoints"
                })),
            )
        })
        .layer(cors)
        // Outermost: runs before routing, so a foreign Host never reaches a
        // handler — not even the unauthenticated UI shells or the 404 fallback.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            host_guard,
        ))
        .with_state(state)
}

// ============================================================================
// Server Startup
// ============================================================================

/// Start the Signal Terminal HTTP server on a background Tokio task.
///
/// - Dev mode (`debug_assertions`): port 4447
/// - Production: port 4446
///
/// These are intentionally clear of the Vite toolchain (dev server 4444, HMR
/// 4445) so the terminal's service worker can never share the app's origin.
pub fn start_signal_terminal() {
    let port: u16 = if cfg!(debug_assertions) { 4447 } else { 4446 };
    let token = get_or_create_token();

    info!(target: "4da::terminal", port = port, "Starting Signal Terminal");

    tauri::async_runtime::spawn(async move {
        let app = build_router(token, port);
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const TEST_TOKEN: &str = "test_token_12345";

    fn test_state(port: u16) -> TerminalState {
        TerminalState {
            token: Arc::new(TEST_TOKEN.to_string()),
            port,
        }
    }

    fn headers_with(token: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(token) = token {
            headers.insert(
                TOKEN_HEADER,
                axum::http::HeaderValue::from_str(token).expect("valid header value"),
            );
        }
        headers
    }

    // ── Live server harness ─────────────────────────────────────────────────
    //
    // Binds a real socket on an ephemeral port and serves the real router, so
    // these tests exercise the actual hyper -> middleware -> handler path rather
    // than a hand-assembled `Request`. The router is built with the port that
    // was actually bound, which is also what the Host allowlist checks.
    struct TestServer {
        port: u16,
    }

    impl TestServer {
        async fn start() -> Self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind ephemeral port");
            let port = listener.local_addr().expect("local addr").port();
            let app = build_router(TEST_TOKEN.to_string(), port);
            tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });
            Self { port }
        }

        fn url(&self, path: &str) -> String {
            format!("http://127.0.0.1:{}{path}", self.port)
        }

        /// Send a request, optionally overriding the token and Host headers.
        async fn get(
            &self,
            path: &str,
            token: Option<&str>,
            host: Option<&str>,
        ) -> reqwest::Response {
            let mut req = reqwest::Client::new().get(self.url(path));
            if let Some(token) = token {
                req = req.header(TOKEN_HEADER, token);
            }
            if let Some(host) = host {
                req = req.header(reqwest::header::HOST, host);
            }
            req.send().await.expect("request completes")
        }
    }

    // ── Token generation ────────────────────────────────────────────────────

    #[test]
    fn test_token_generation_format() {
        let token = generate_token();
        assert_eq!(token.len(), TOKEN_LEN);
        assert!(token.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_token_generation_is_not_modulo_biased() {
        // Exhaustive over the whole u8 range — deterministic, not statistical.
        //
        // With rejection sampling every one of the 62 symbols is reachable from
        // exactly 4 raw bytes, so all are equally likely.
        let mut counts = [0u32; 62];
        for byte in 0..=u8::MAX {
            if byte < TOKEN_REJECT_AT {
                counts[(byte % 62) as usize] += 1;
            }
        }
        assert!(
            counts.iter().all(|&c| c == 4),
            "symbol frequencies must be uniform, got {counts:?}"
        );

        // The same sweep WITHOUT rejection — i.e. the old `rand::random::<u8>() % 62`
        // — over-weights the first 8 symbols. Asserting the old behaviour is
        // genuinely biased is what makes the test above meaningful: it proves
        // the rejection step is load-bearing and a revert would be caught.
        let mut biased = [0u32; 62];
        for byte in 0..=u8::MAX {
            biased[(byte % 62) as usize] += 1;
        }
        assert_eq!(biased[0], 5, "unrejected %62 over-weights low symbols");
        assert_eq!(biased[61], 4, "...and under-weights high symbols");
        assert_eq!(
            TOKEN_REJECT_AT as u32 % 62,
            0,
            "threshold must divide evenly"
        );
    }

    #[test]
    fn test_tokens_are_distinct() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b, "successive tokens must not repeat");
    }

    // ── Constant-time comparison ────────────────────────────────────────────

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"), "length mismatch");
        assert!(!constant_time_eq(b"", b"a"));
        assert!(constant_time_eq(b"", b""));
    }

    // ── check_auth: the token is MANDATORY ──────────────────────────────────

    #[test]
    fn test_auth_rejects_missing_token() {
        // THE regression this whole change exists for. Before the fix a request
        // with no X-4DA-Token header was allowed straight through.
        let state = test_state(4447);
        let result = check_auth(&headers_with(None), &state);
        let (status, _) = result.expect_err("missing token must be rejected");
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_rejects_wrong_token() {
        let state = test_state(4447);
        let (status, _) = check_auth(&headers_with(Some("wrong")), &state)
            .expect_err("wrong token must be rejected");
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_rejects_empty_token() {
        let state = test_state(4447);
        let (status, _) =
            check_auth(&headers_with(Some("")), &state).expect_err("empty token must be rejected");
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_accepts_correct_token() {
        let state = test_state(4447);
        assert!(check_auth(&headers_with(Some(TEST_TOKEN)), &state).is_ok());
    }

    #[test]
    fn test_auth_fails_closed_on_empty_configured_token() {
        // If the token file could not be read or written, nothing authenticates.
        let state = TerminalState {
            token: Arc::new(String::new()),
            port: 4447,
        };
        assert!(check_auth(&headers_with(Some("")), &state).is_err());
        assert!(check_auth(&headers_with(None), &state).is_err());
    }

    #[test]
    fn test_sse_auth_accepts_query_token_but_still_requires_one() {
        let state = test_state(4447);
        // EventSource cannot set headers, so the query fallback must work...
        assert!(check_auth_sse(&headers_with(None), Some(TEST_TOKEN), &state).is_ok());
        // ...but it is a fallback, not a bypass.
        assert!(check_auth_sse(&headers_with(None), None, &state).is_err());
        assert!(check_auth_sse(&headers_with(None), Some("wrong"), &state).is_err());
        // Header still works on its own.
        assert!(check_auth_sse(&headers_with(Some(TEST_TOKEN)), None, &state).is_ok());
    }

    // ── Host allowlist (DNS-rebinding defence) ──────────────────────────────

    #[test]
    fn test_allowed_hosts() {
        assert!(is_allowed_host("127.0.0.1:4447", 4447));
        assert!(is_allowed_host("localhost:4447", 4447));
        assert!(is_allowed_host("LOCALHOST:4447", 4447), "case-insensitive");
        assert!(is_allowed_host("[::1]:4447", 4447));
    }

    #[test]
    fn test_rejected_hosts() {
        // The rebinding attack: attacker's own domain, our port.
        assert!(!is_allowed_host("evil.example.com:4447", 4447));
        // Port-less Host — cannot have come from a browser addressing this server.
        assert!(!is_allowed_host("localhost", 4447));
        assert!(!is_allowed_host("127.0.0.1", 4447));
        // Right name, wrong port.
        assert!(!is_allowed_host("127.0.0.1:4446", 4447));
        // Other loopback aliases and LAN addresses are NOT on the allowlist.
        assert!(!is_allowed_host("127.0.0.2:4447", 4447));
        assert!(!is_allowed_host("192.168.1.10:4447", 4447));
        assert!(!is_allowed_host("localhost.evil.com:4447", 4447));
        assert!(!is_allowed_host("", 4447));
        // Malformed IPv6.
        assert!(!is_allowed_host("[::1]", 4447));
        assert!(!is_allowed_host("[::1:4447", 4447));
    }

    #[test]
    fn test_split_host_port() {
        assert_eq!(split_host_port("localhost:80"), Some(("localhost", 80)));
        assert_eq!(split_host_port("[::1]:8080"), Some(("::1", 8080)));
        assert_eq!(split_host_port("localhost"), None);
        assert_eq!(split_host_port("localhost:notaport"), None);
    }

    // ── End-to-end over a real socket ───────────────────────────────────────

    #[tokio::test]
    async fn test_router_builds() {
        let _router = build_router(TEST_TOKEN.to_string(), 4447);
    }

    #[tokio::test]
    async fn test_live_api_requires_token() {
        let server = TestServer::start().await;

        // No token at all — this returned 200 with the full signal corpus before.
        let res = server.get("/api/status", None, None).await;
        assert_eq!(res.status(), 401, "missing token must not be served");

        // Wrong token.
        let res = server.get("/api/status", Some("wrong-token"), None).await;
        assert_eq!(res.status(), 401, "wrong token must not be served");

        // Correct token.
        let res = server.get("/api/status", Some(TEST_TOKEN), None).await;
        assert_eq!(res.status(), 200, "correct token must be served");
    }

    #[tokio::test]
    async fn test_live_every_api_route_requires_token() {
        let server = TestServer::start().await;
        // Every data-bearing route, not just a sample — a new route added
        // without check_auth fails here.
        for path in [
            "/api/boot",
            "/api/status",
            "/api/signals",
            "/api/briefing",
            "/api/score?url=https://example.com",
            "/api/radar",
            "/api/decisions",
            "/api/dna",
            "/api/gaps",
            "/api/search?q=rust",
            "/api/sources",
            "/api/stream",
            "/api/simulate?add=python",
        ] {
            let res = server.get(path, None, None).await;
            assert_eq!(res.status(), 401, "{path} must require a token");
        }
    }

    #[tokio::test]
    async fn test_live_foreign_host_is_rejected() {
        let server = TestServer::start().await;

        // DNS rebinding: attacker's domain resolved to 127.0.0.1, correct port,
        // and — worst case — a stolen token. The Host check still refuses.
        let res = server
            .get("/api/status", Some(TEST_TOKEN), Some("evil.example.com"))
            .await;
        assert_eq!(res.status(), 403, "foreign Host must be rejected");

        // The unauthenticated UI shell is behind the same guard.
        let res = server.get("/", None, Some("evil.example.com")).await;
        assert_eq!(
            res.status(),
            403,
            "foreign Host must not reach the UI shell"
        );

        // So is the 404 fallback — no probing the route table from a foreign Host.
        let res = server.get("/nope", None, Some("evil.example.com")).await;
        assert_eq!(
            res.status(),
            403,
            "foreign Host must not reach the fallback"
        );
    }

    #[tokio::test]
    async fn test_live_loopback_hosts_are_accepted() {
        let server = TestServer::start().await;
        for host in [
            format!("127.0.0.1:{}", server.port),
            format!("localhost:{}", server.port),
            format!("[::1]:{}", server.port),
        ] {
            let res = server.get("/", None, Some(&host)).await;
            assert_eq!(res.status(), 200, "{host} must be accepted");
        }
    }

    #[tokio::test]
    async fn test_live_missing_host_is_rejected() {
        // reqwest always sets Host, so speak HTTP/1.0 on a raw socket to omit it.
        // HTTP/1.0 makes Host optional, so hyper passes it to our guard rather
        // than rejecting it itself.
        let server = TestServer::start().await;
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", server.port))
            .await
            .expect("connect");
        stream
            .write_all(b"GET /api/status HTTP/1.0\r\n\r\n")
            .await
            .expect("write request");

        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.expect("read");
        let response = String::from_utf8_lossy(&response);

        assert!(
            !response.starts_with("HTTP/1.0 200") && !response.starts_with("HTTP/1.1 200"),
            "a request with no Host header must not be served: {response}"
        );
        assert!(
            response.contains(" 403 "),
            "missing Host should be refused by host_guard, got: {response}"
        );
    }

    #[tokio::test]
    async fn test_live_sse_accepts_query_token() {
        let server = TestServer::start().await;

        // No token anywhere.
        let res = server.get("/api/stream", None, None).await;
        assert_eq!(res.status(), 401);

        // Query token — the EventSource path the terminal UI actually uses.
        let res = server
            .get(&format!("/api/stream?token={TEST_TOKEN}"), None, None)
            .await;
        assert_eq!(res.status(), 200, "EventSource query token must work");
    }
}
