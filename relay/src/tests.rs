//! Integration tests for the relay server.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::routing::{delete, get, patch, post};
use axum::Router;
use http_body_util::BodyExt;
use sqlx::SqlitePool;
use tower::ServiceExt;

use crate::{clients, entries, health};

/// Create an in-memory SQLite pool for testing.
async fn test_pool() -> SqlitePool {
    // Ensure JWT_SECRET is set for tests (safe test-only value)
    std::env::set_var(
        "JWT_SECRET",
        "test-only-jwt-secret-do-not-use-in-production",
    );
    crate::db::create_pool("sqlite::memory:").await.unwrap()
}

/// Build the test router with all routes.
fn test_router(pool: SqlitePool) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/auth/invite", post(clients::join_via_invite))
        .route("/teams", post(clients::create_team))
        .route("/teams/:team_id", get(clients::get_team_info))
        .route("/teams/:team_id/entries", post(entries::push_entry))
        .route("/teams/:team_id/entries", get(entries::pull_entries))
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
        .route("/teams/:team_id/leave", post(clients::leave_team))
        .route("/teams/:team_id/invites", post(clients::create_invite))
        .with_state(pool)
}

/// Helper: create a team and return (router, team_id, admin_token).
async fn setup_team() -> (Router, String, String) {
    let pool = test_pool().await;
    let app = test_router(pool);
    let team_id = uuid::Uuid::new_v4().to_string();

    let body = serde_json::json!({
        "team_id": team_id,
        "client_id": "admin-client-1",
        "display_name": "Admin User",
        "public_key": [1, 2, 3, 4],
        "license_key_hash": "testhash123"
    });

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/teams")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = json["token"].as_str().unwrap().to_string();

    (app, team_id, token)
}

/// Helper: add a second member via invite flow, return their token.
async fn add_member(app: &Router, team_id: &str, admin_token: &str) -> String {
    // Create invite
    let invite_body = serde_json::json!({
        "role": "member"
    });
    let uri = format!("/teams/{}/invites", team_id);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(&uri)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", admin_token))
                .body(Body::from(serde_json::to_vec(&invite_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let invite_code = json["code"].as_str().unwrap().to_string();

    // Join via invite
    let join_body = serde_json::json!({
        "invite_code": invite_code,
        "client_id": "member-client-2",
        "display_name": "Member User",
        "public_key": [5, 6, 7, 8]
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/auth/invite")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&join_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    json["token"].as_str().unwrap().to_string()
}

// ---------- Tests ----------

#[tokio::test]
async fn test_health_endpoint() {
    let pool = test_pool().await;
    let app = test_router(pool);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"ok");
}

#[tokio::test]
async fn test_create_team() {
    let (app, team_id, token) = setup_team().await;

    // Verify we can use the token to list clients
    let uri = format!("/teams/{}/clients", team_id);
    let resp = app
        .oneshot(
            Request::builder()
                .uri(&uri)
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let clients: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0]["role"], "admin");
}

#[tokio::test]
async fn test_push_pull_entries() {
    let (app, team_id, token) = setup_team().await;

    // Push an entry
    let push_body = serde_json::json!({
        "client_id": "admin-client-1",
        "payload": [10, 20, 30, 40, 50]
    });
    let uri = format!("/teams/{}/entries", team_id);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(&uri)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(serde_json::to_vec(&push_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let seq = json["relay_seq"].as_i64().unwrap();
    assert!(seq > 0);

    // Pull entries
    let pull_uri = format!("/teams/{}/entries?since=0", team_id);
    let resp = app
        .oneshot(
            Request::builder()
                .uri(&pull_uri)
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let entries = json["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
}

#[tokio::test]
async fn test_invite_flow() {
    let (app, team_id, token) = setup_team().await;
    let member_token = add_member(&app, &team_id, &token).await;

    // Verify the member can list clients
    let uri = format!("/teams/{}/clients", team_id);
    let resp = app
        .oneshot(
            Request::builder()
                .uri(&uri)
                .header("authorization", format!("Bearer {}", member_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let clients: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(clients.len(), 2);
}

#[tokio::test]
async fn test_remove_member_admin_only() {
    let (app, team_id, token) = setup_team().await;
    let member_token = add_member(&app, &team_id, &token).await;

    // Member tries to remove admin — should fail
    let uri = format!("/teams/{}/clients/admin-client-1", team_id);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri(&uri)
                .header("authorization", format!("Bearer {}", member_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Admin removes member — should succeed
    let uri = format!("/teams/{}/clients/member-client-2", team_id);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri(&uri)
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    // Verify member count is now 1
    let uri = format!("/teams/{}/clients", team_id);
    let resp = app
        .oneshot(
            Request::builder()
                .uri(&uri)
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let clients: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(clients.len(), 1);
}

#[tokio::test]
async fn test_role_change() {
    let (app, team_id, token) = setup_team().await;
    let _member_token = add_member(&app, &team_id, &token).await;

    // Promote member to admin
    let body = serde_json::json!({ "role": "admin" });
    let uri = format!("/teams/{}/clients/member-client-2", team_id);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri(&uri)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["role"], "admin");
}

#[tokio::test]
async fn test_leave_team() {
    let (app, team_id, token) = setup_team().await;
    let member_token = add_member(&app, &team_id, &token).await;

    // Member leaves
    let uri = format!("/teams/{}/leave", team_id);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(&uri)
                .header("authorization", format!("Bearer {}", member_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    // Verify only admin remains
    let uri = format!("/teams/{}/clients", team_id);
    let resp = app
        .oneshot(
            Request::builder()
                .uri(&uri)
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let clients: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(clients.len(), 1);
}

#[tokio::test]
async fn test_payload_size_limit() {
    let (app, team_id, token) = setup_team().await;

    // Create a payload larger than 64KB
    let big_payload: Vec<u8> = vec![0u8; 70_000];
    let push_body = serde_json::json!({
        "client_id": "admin-client-1",
        "payload": big_payload
    });
    let uri = format!("/teams/{}/entries", team_id);
    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(&uri)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(serde_json::to_vec(&push_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_expired_invite_rejected() {
    let pool = test_pool().await;
    let app = test_router(pool.clone());
    let team_id = uuid::Uuid::new_v4().to_string();

    // Create team
    let body = serde_json::json!({
        "team_id": team_id,
        "client_id": "admin-1",
        "display_name": "Admin",
        "public_key": [1, 2, 3],
        "license_key_hash": "hash"
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/teams")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let _token = json["token"].as_str().unwrap().to_string();

    // Insert an already-expired invite directly into DB
    sqlx::query(
        "INSERT INTO team_invites (code, team_id, role, created_by, expires_at)
         VALUES ($1, $2, 'member', 'admin-1', datetime('now', '-1 hour'))",
    )
    .bind("EXPIRED-CODE")
    .bind(&team_id)
    .execute(&pool)
    .await
    .unwrap();

    // Try to join with expired invite
    let join_body = serde_json::json!({
        "invite_code": "EXPIRED-CODE",
        "client_id": "new-member",
        "display_name": "New Member",
        "public_key": [5, 6, 7]
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/auth/invite")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&join_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_team_id_mismatch() {
    let (app, _team_id, token) = setup_team().await;

    // Try to access a different team with the token
    let other_team = uuid::Uuid::new_v4().to_string();
    let uri = format!("/teams/{}/clients", other_team);
    let resp = app
        .oneshot(
            Request::builder()
                .uri(&uri)
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
