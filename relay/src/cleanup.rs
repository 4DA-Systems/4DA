//! Background retention cleanup — deletes old entries and expired invites.

use sqlx::SqlitePool;
use std::time::Duration;
use tracing::info;

/// Default retention: 30 days.
const DEFAULT_RETENTION_DAYS: u64 = 30;

/// Extra buffer beyond invite expiry before we delete the invite row.
const INVITE_CLEANUP_BUFFER_DAYS: u64 = 7;

/// Spawn the periodic cleanup background task.
pub fn spawn_cleanup_task(pool: SqlitePool) {
    let retention_days: u64 = std::env::var("RETENTION_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_RETENTION_DAYS);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        loop {
            interval.tick().await;
            if let Err(e) = run_cleanup(&pool, retention_days).await {
                tracing::error!(target: "relay::cleanup", error = %e, "Cleanup failed");
            }
        }
    });
}

/// Execute a single cleanup pass.
pub async fn run_cleanup(pool: &SqlitePool, retention_days: u64) -> Result<(), sqlx::Error> {
    // Delete old sync entries
    let entries_deleted = sqlx::query(
        "DELETE FROM sync_entries
         WHERE created_at < datetime('now', $1)",
    )
    .bind(format!("-{retention_days} days"))
    .execute(pool)
    .await?
    .rows_affected();

    // Delete expired invites (with buffer)
    let total_buffer = INVITE_CLEANUP_BUFFER_DAYS;
    let invites_deleted = sqlx::query(
        "DELETE FROM team_invites
         WHERE expires_at < datetime('now', $1)",
    )
    .bind(format!("-{total_buffer} days"))
    .execute(pool)
    .await?
    .rows_affected();

    if entries_deleted > 0 || invites_deleted > 0 {
        info!(
            target: "relay::cleanup",
            entries_deleted,
            invites_deleted,
            retention_days,
            "Cleanup pass completed"
        );
    }

    Ok(())
}
