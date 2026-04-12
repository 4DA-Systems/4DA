//! SSE notification stream for real-time sync updates.
//!
//! NOTE: This stream endpoint is implemented and authenticated but is NOT
//! currently consumed by the 4DA desktop client. The client uses interval
//! polling (30s) via team_sync_scheduler.rs instead. Wiring the SSE
//! consumer is tracked as future work.
//! See: .ai/COMPREHENSIVE-AUDIT-AND-PREVENTION-2026-04-12.md G-P1-14

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::{self, Stream};
use sqlx::SqlitePool;
use std::convert::Infallible;
use std::time::Duration;

use crate::auth::AuthTeam;
use crate::error::RelayError;

/// GET /teams/{team_id}/stream -- SSE notification stream.
/// Sends "new_entry" events with the latest sequence number.
pub async fn event_stream(
    AuthTeam(claims): AuthTeam,
    Path(team_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, RelayError> {
    if claims.team_id != team_id {
        return Err(RelayError::Auth("Team ID mismatch".to_string()));
    }

    // Get current max sequence as starting point
    let current_max: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(id), 0) FROM sync_entries WHERE team_id = $1")
            .bind(&team_id)
            .fetch_one(&pool)
            .await
            .unwrap_or(0);

    let team_id_owned = team_id.clone();
    let stream = stream::unfold(
        (pool, team_id_owned, current_max),
        |(pool, team_id, mut last_seq)| async move {
            // Poll every 2 seconds for new entries
            tokio::time::sleep(Duration::from_secs(2)).await;

            let new_max: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(id), 0) FROM sync_entries WHERE team_id = $1",
            )
            .bind(&team_id)
            .fetch_one(&pool)
            .await
            .unwrap_or(last_seq);

            if new_max > last_seq {
                last_seq = new_max;
                let event = Event::default()
                    .event("new_entry")
                    .data(new_max.to_string())
                    .id(new_max.to_string());
                Some((Ok(event), (pool, team_id, last_seq)))
            } else {
                // Send a comment as keepalive (won't be a real event)
                let event = Event::default().comment("keepalive");
                Some((Ok(event), (pool, team_id, last_seq)))
            }
        },
    );

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    ))
}
