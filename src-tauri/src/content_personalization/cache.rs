//! Cache management for the Sovereign Content Engine.
//!
//! Handles pruning of stale personalization cache entries and read state cleanup.
//! Content is cached in `content_personalization_cache` keyed by context_hash.
//! Old entries are pruned to keep only the most recent versions per lesson.

use rusqlite::Connection;
use tracing::{debug, info};

/// Maximum number of cache versions to keep per (module_id, lesson_idx, block_type, block_id).
const MAX_CACHE_VERSIONS: u32 = 3;

/// Maximum age in days for cache entries before they're eligible for pruning.
const MAX_CACHE_AGE_DAYS: u32 = 30;

/// Maximum age in days for read state entries.
const MAX_READ_STATE_AGE_DAYS: u32 = 90;

/// Prune stale cache entries. Keeps only the most recent `MAX_CACHE_VERSIONS` per slot
/// and removes entries older than `MAX_CACHE_AGE_DAYS`.
///
/// Returns the number of rows deleted.
pub fn prune_cache(conn: &Connection) -> usize {
    let mut total_deleted = 0usize;

    // 1. Remove entries older than MAX_CACHE_AGE_DAYS
    match conn.execute(
        "DELETE FROM content_personalization_cache
         WHERE generated_at < datetime('now', ?1)",
        [format!("-{MAX_CACHE_AGE_DAYS} days")],
    ) {
        Ok(count) => {
            total_deleted += count;
            if count > 0 {
                debug!(
                    target: "4da::cache",
                    deleted = count,
                    "Pruned old cache entries (>{} days)",
                    MAX_CACHE_AGE_DAYS
                );
            }
        }
        Err(e) => {
            debug!(target: "4da::cache", error = %e, "Failed to prune old cache entries");
        }
    }

    // 2. Keep only MAX_CACHE_VERSIONS per slot — delete older ones
    match conn.execute(
        "DELETE FROM content_personalization_cache
         WHERE id NOT IN (
             SELECT id FROM (
                 SELECT id,
                     ROW_NUMBER() OVER (
                         PARTITION BY module_id, lesson_idx, block_type, block_id
                         ORDER BY generated_at DESC
                     ) AS rn
                 FROM content_personalization_cache
             ) WHERE rn <= ?1
         )",
        [MAX_CACHE_VERSIONS],
    ) {
        Ok(count) => {
            total_deleted += count;
            if count > 0 {
                debug!(
                    target: "4da::cache",
                    deleted = count,
                    "Pruned excess cache versions (>{} per slot)",
                    MAX_CACHE_VERSIONS
                );
            }
        }
        Err(e) => {
            debug!(target: "4da::cache", error = %e, "Failed to prune excess cache versions");
        }
    }

    // 3. Clean old read state entries
    match conn.execute(
        "DELETE FROM content_read_state
         WHERE read_at < datetime('now', ?1)",
        [format!("-{MAX_READ_STATE_AGE_DAYS} days")],
    ) {
        Ok(count) => {
            if count > 0 {
                debug!(
                    target: "4da::cache",
                    deleted = count,
                    "Pruned old read state entries (>{} days)",
                    MAX_READ_STATE_AGE_DAYS
                );
            }
        }
        Err(e) => {
            debug!(target: "4da::cache", error = %e, "Failed to prune old read state entries");
        }
    }

    if total_deleted > 0 {
        info!(
            target: "4da::cache",
            total_deleted,
            "Cache pruning complete"
        );
    }

    total_deleted
}

/// Get cache statistics for diagnostics.
pub fn cache_stats(conn: &Connection) -> CacheStats {
    let cache_count: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM content_personalization_cache",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let read_state_count: u32 = conn
        .query_row("SELECT COUNT(*) FROM content_read_state", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    let cache_size_bytes: u64 = conn
        .query_row(
            "SELECT COALESCE(SUM(LENGTH(content_json)), 0) FROM content_personalization_cache",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let unique_slots: u32 = conn
        .query_row(
            "SELECT COUNT(DISTINCT module_id || ':' || lesson_idx || ':' || block_type || ':' || block_id)
             FROM content_personalization_cache",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    CacheStats {
        cache_entries: cache_count,
        read_state_entries: read_state_count,
        cache_size_bytes,
        unique_slots,
    }
}

/// Cache statistics for diagnostics.
#[derive(Debug, serde::Serialize)]
pub struct CacheStats {
    pub cache_entries: u32,
    pub read_state_entries: u32,
    pub cache_size_bytes: u64,
    pub unique_slots: u32,
}
