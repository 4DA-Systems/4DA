//! 4DA Team Relay — encrypted metadata sync server.
//!
//! A thin relay that stores and routes E2E encrypted blobs.
//! The relay cannot read team metadata — all encryption is client-side.
//!
//! API:
//!   POST /teams                                     -- create a new team
//!   GET  /teams/:team_id                           -- get team info
//!   POST /teams/:team_id/entries                   -- push encrypted entry
//!   GET  /teams/:team_id/entries                   -- pull entries since sequence
//!   GET  /teams/:team_id/clients                   -- list team members
//!   POST /teams/:team_id/clients                   -- register client
//!   DELETE /teams/:team_id/clients/:client_id     -- remove member (admin)
//!   PATCH  /teams/:team_id/clients/:client_id     -- change role (admin)
//!   POST /teams/:team_id/leave                     -- leave team
//!   POST /teams/:team_id/invites                   -- generate invite code (admin)
//!   POST /auth/invite                               -- join team via invite code
//!   GET  /teams/:team_id/stream                    -- SSE notification stream
//!   GET  /health                                    -- health check

mod auth;
mod cleanup;
mod clients;
mod db;
mod entries;
mod error;
mod rate_limit;
mod stream;

#[cfg(test)]
mod tests;

use axum::routing::{delete, get, patch, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "relay=info,tower_http=info".into()),
        )
        .init();

    // Validate required environment variables before starting
    if std::env::var("JWT_SECRET").is_err() {
        eprintln!("FATAL: JWT_SECRET environment variable must be set");
        std::process::exit(1);
    }

    // Database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:relay.db?mode=rwc".to_string());
    let pool = db::create_pool(&database_url)
        .await
        .expect("Failed to create database pool");

    // Spawn background cleanup task
    cleanup::spawn_cleanup_task(pool.clone());

    // Rate limiter (100 req/min general)
    let rate_layer = rate_limit::RateLimitLayer::new(100, 60);
    rate_limit::spawn_rate_limit_cleanup(rate_layer.state());

    // Router
    let app = Router::new()
        // Health
        .route("/health", get(health))
        // Auth
        .route("/auth/invite", post(clients::join_via_invite))
        // Teams
        .route("/teams", post(clients::create_team))
        .route("/teams/:team_id", get(clients::get_team_info))
        // Team entries
        .route("/teams/:team_id/entries", post(entries::push_entry))
        .route("/teams/:team_id/entries", get(entries::pull_entries))
        // Team clients
        .route("/teams/:team_id/clients", get(clients::list_clients))
        .route("/teams/:team_id/clients", post(clients::register_client))
        .route(
            "/teams/:team_id/clients/:client_id",
            delete(clients::remove_member),
        )
        .route(
            "/teams/:team_id/clients/:client_id",
            patch(clients::update_role),
        )
        // Team leave
        .route("/teams/:team_id/leave", post(clients::leave_team))
        // Team invites
        .route("/teams/:team_id/invites", post(clients::create_invite))
        // SSE stream
        .route("/teams/:team_id/stream", get(stream::event_stream))
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                // Tauri WebView sends different Origin headers per platform:
                // - macOS/Linux: tauri://localhost or https://tauri.localhost
                // - Windows: http://tauri.localhost
                // - Dev mode: http://localhost:4444
                .allow_origin([
                    "tauri://localhost".parse().expect("valid origin"),
                    "https://tauri.localhost".parse().expect("valid origin"),
                    "http://tauri.localhost".parse().expect("valid origin"),
                    "http://localhost".parse().expect("valid origin"),
                    "http://localhost:4444".parse().expect("valid origin"),
                ])
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::DELETE,
                    axum::http::Method::PATCH,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers([
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::CONTENT_TYPE,
                ]),
        )
        .layer(rate_layer)
        .with_state(pool);

    // Server
    let port = std::env::var("PORT").unwrap_or_else(|_| "8443".to_string());
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr = format!("{bind_addr}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    info!(target: "relay", addr = %addr, "4DA Relay server started");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
}

async fn health() -> &'static str {
    "ok"
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl+c");
    info!(target: "relay", "Shutdown signal received");
}
