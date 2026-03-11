//! 4DA Team Relay — encrypted metadata sync server.
//!
//! A thin relay that stores and routes E2E encrypted blobs.
//! The relay cannot read team metadata — all encryption is client-side.
//!
//! API:
//!   POST /teams                         -- create a new team
//!   POST /teams/{team_id}/entries       -- push encrypted entry
//!   GET  /teams/{team_id}/entries       -- pull entries since sequence
//!   GET  /teams/{team_id}/clients       -- list team members
//!   POST /teams/{team_id}/clients       -- register client
//!   POST /teams/{team_id}/invites       -- generate invite code (admin)
//!   POST /auth/invite                   -- join team via invite code
//!   GET  /teams/{team_id}/stream        -- SSE notification stream
//!   GET  /health                        -- health check

mod auth;
mod clients;
mod db;
mod entries;
mod error;
mod stream;

use axum::routing::{get, post};
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

    // Database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:relay.db?mode=rwc".to_string());
    let pool = db::create_pool(&database_url)
        .await
        .expect("Failed to create database pool");

    // Router
    let app = Router::new()
        // Health
        .route("/health", get(health))
        // Auth
        .route("/auth/invite", post(clients::join_via_invite))
        // Teams
        .route("/teams", post(clients::create_team))
        // Team entries
        .route("/teams/{team_id}/entries", post(entries::push_entry))
        .route("/teams/{team_id}/entries", get(entries::pull_entries))
        // Team clients
        .route("/teams/{team_id}/clients", get(clients::list_clients))
        .route("/teams/{team_id}/clients", post(clients::register_client))
        // Team invites
        .route("/teams/{team_id}/invites", post(clients::create_invite))
        // SSE stream
        .route("/teams/{team_id}/stream", get(stream::event_stream))
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        )
        .with_state(pool);

    // Server
    let port = std::env::var("PORT").unwrap_or_else(|_| "8443".to_string());
    let addr = format!("0.0.0.0:{port}");
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
