//! Relay database — SQLite storage for encrypted sync entries.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tracing::info;

pub async fn create_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    run_migrations(&pool).await?;

    info!(target: "relay::db", "Database initialized");
    Ok(pool)
}

async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sync_entries (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            team_id     TEXT NOT NULL,
            client_id   TEXT NOT NULL,
            payload     BLOB NOT NULL,
            created_at  TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_sync_team_id
         ON sync_entries(team_id, id DESC)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS team_clients (
            team_id      TEXT NOT NULL,
            client_id    TEXT NOT NULL,
            public_key   BLOB NOT NULL,
            display_name TEXT,
            role         TEXT NOT NULL DEFAULT 'member',
            last_seen    TEXT DEFAULT (datetime('now')),
            PRIMARY KEY (team_id, client_id)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS team_invites (
            code        TEXT PRIMARY KEY,
            team_id     TEXT NOT NULL,
            email       TEXT,
            role        TEXT NOT NULL DEFAULT 'member',
            created_by  TEXT NOT NULL,
            created_at  TEXT DEFAULT (datetime('now')),
            expires_at  TEXT NOT NULL,
            used_at     TEXT,
            used_by     TEXT
        )",
    )
    .execute(pool)
    .await?;

    Ok(())
}
