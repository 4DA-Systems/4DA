// Copyright (c) 2025-2026 4DA Systems. All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Video curriculum management for STREETS Cohort members.
//!
//! Videos are drip-released based on days since license activation.
//! Cohort members unlock new lessons over an 8-week schedule, with
//! progress tracking persisted in the local SQLite database.

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoLesson {
    pub id: i64,
    pub video_id: String,
    pub title: String,
    pub duration_seconds: i64,
    pub drip_day: i64,
    pub watched: bool,
    pub watch_progress_seconds: i64,
    pub unlocked: bool,
    pub unlocked_at: Option<String>,
    pub watched_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoCurriculumStatus {
    pub total_videos: usize,
    pub unlocked_count: usize,
    pub watched_count: usize,
    pub total_duration_seconds: i64,
    pub watched_duration_seconds: i64,
    pub days_since_activation: i64,
}

// ============================================================================
// Video Manifest (hardcoded 8-week curriculum)
// ============================================================================

/// (video_id, title, duration_seconds, drip_day)
const VIDEO_MANIFEST: &[(&str, &str, i64, i64)] = &[
    (
        "streets-w1-01",
        "Week 1: Sovereign Setup Deep Dive",
        2700,
        0,
    ),
    (
        "streets-w1-02",
        "Week 1: Infrastructure Audit Workshop",
        1800,
        2,
    ),
    (
        "streets-w2-01",
        "Week 2: Technical Moats Masterclass",
        2700,
        7,
    ),
    ("streets-w2-02", "Week 2: Positioning Workshop", 1800, 9),
    (
        "streets-w3-01",
        "Week 3: Revenue Engine Selection",
        2700,
        14,
    ),
    (
        "streets-w3-02",
        "Week 3: 48-Hour Validation Framework",
        1800,
        16,
    ),
    ("streets-w4-01", "Week 4: Execution Blueprint", 2700, 21),
    ("streets-w4-02", "Week 4: Time-Boxing Mastery", 1800, 23),
    ("streets-w5-01", "Week 5: First Engine Launch", 2700, 28),
    (
        "streets-w5-02",
        "Week 5: Pricing Strategy Session",
        1800,
        30,
    ),
    ("streets-w6-01", "Week 6: Evolving Edge Workshop", 2700, 35),
    ("streets-w6-02", "Week 6: Trend Detection System", 1800, 37),
    ("streets-w7-01", "Week 7: Automation Deep Dive", 2700, 42),
    ("streets-w7-02", "Week 7: Monitoring Stack Setup", 1800, 44),
    (
        "streets-w8-01",
        "Week 8: Stacking Streams Strategy",
        2700,
        49,
    ),
    (
        "streets-w8-02",
        "Week 8: $10K/Month Roadmap Workshop",
        1800,
        51,
    ),
];

// ============================================================================
// Helpers
// ============================================================================

/// Calculate the number of days since the license was activated.
/// Returns 0 if no activation date is found or parsing fails.
fn get_days_since_activation() -> Result<i64, String> {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let license = &guard.get().license;

    match &license.activated_at {
        Some(activated) => {
            let start = chrono::DateTime::parse_from_rfc3339(activated)
                .map_err(|e| format!("Failed to parse activation date: {}", e))?;
            let elapsed = chrono::Utc::now().signed_duration_since(start);
            Ok(elapsed.num_days().max(0))
        }
        None => Ok(0),
    }
}

/// Ensure the video_curriculum table exists and seed it from the manifest if empty.
fn seed_curriculum_if_needed(conn: &rusqlite::Connection) -> Result<(), String> {
    // Create table if it doesn't exist
    conn.execute(
        "CREATE TABLE IF NOT EXISTS video_curriculum (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            video_id TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            duration_seconds INTEGER NOT NULL DEFAULT 0,
            drip_day INTEGER NOT NULL DEFAULT 0,
            watched INTEGER NOT NULL DEFAULT 0,
            watch_progress_seconds INTEGER NOT NULL DEFAULT 0,
            unlocked_at TEXT,
            watched_at TEXT
        )",
        [],
    )
    .map_err(|e| format!("Failed to create video_curriculum table: {}", e))?;

    // Check if table already has rows
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM video_curriculum", [], |row| {
            row.get(0)
        })
        .map_err(|e| format!("Failed to count video_curriculum rows: {}", e))?;

    if count > 0 {
        debug!(target: "4da::video", count = count, "Video curriculum already seeded");
        return Ok(());
    }

    // Seed from manifest
    info!(target: "4da::video", videos = VIDEO_MANIFEST.len(), "Seeding video curriculum");

    let mut stmt = conn
        .prepare(
            "INSERT INTO video_curriculum (video_id, title, duration_seconds, drip_day)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .map_err(|e| format!("Failed to prepare seed statement: {}", e))?;

    for (video_id, title, duration, drip_day) in VIDEO_MANIFEST {
        stmt.execute(rusqlite::params![video_id, title, duration, drip_day])
            .map_err(|e| format!("Failed to seed video '{}': {}", video_id, e))?;
    }

    info!(target: "4da::video", "Video curriculum seeded successfully");
    Ok(())
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get the full video curriculum with unlock status and progress summary.
/// Gate: requires streets_cohort membership.
#[tauri::command]
pub fn get_video_curriculum() -> Result<(Vec<VideoLesson>, VideoCurriculumStatus), String> {
    crate::settings::require_streets_feature("streets_cohort")?;

    let conn = crate::open_db_connection()?;
    seed_curriculum_if_needed(&conn)?;

    let days = get_days_since_activation()?;
    debug!(target: "4da::video", days_since_activation = days, "Fetching video curriculum");

    let mut stmt = conn
        .prepare(
            "SELECT id, video_id, title, duration_seconds, drip_day,
                    watched, watch_progress_seconds, unlocked_at, watched_at
             FROM video_curriculum
             ORDER BY drip_day ASC, id ASC",
        )
        .map_err(|e| format!("Failed to query video_curriculum: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, bool>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
            ))
        })
        .map_err(|e| format!("Failed to read video_curriculum: {}", e))?;

    let mut videos = Vec::new();
    let mut total_duration: i64 = 0;
    let mut watched_duration: i64 = 0;
    let mut watched_count: usize = 0;
    let mut unlocked_count: usize = 0;

    for row in rows {
        let (id, video_id, title, duration, drip_day, watched, progress, unlocked_at, watched_at) =
            row.map_err(|e| format!("Failed to parse video row: {}", e))?;

        let unlocked = drip_day <= days;

        if unlocked {
            unlocked_count += 1;
        }
        if watched {
            watched_count += 1;
            watched_duration += duration;
        }
        total_duration += duration;

        videos.push(VideoLesson {
            id,
            video_id,
            title,
            duration_seconds: duration,
            drip_day,
            watched,
            watch_progress_seconds: progress,
            unlocked,
            unlocked_at,
            watched_at,
        });
    }

    let status = VideoCurriculumStatus {
        total_videos: videos.len(),
        unlocked_count,
        watched_count,
        total_duration_seconds: total_duration,
        watched_duration_seconds: watched_duration,
        days_since_activation: days,
    };

    info!(
        target: "4da::video",
        total = status.total_videos,
        unlocked = status.unlocked_count,
        watched = status.watched_count,
        "Video curriculum loaded"
    );

    Ok((videos, status))
}

/// Update watch progress for a video (partial watch, resume later).
/// Gate: requires streets_cohort membership.
#[tauri::command]
pub fn mark_video_progress(video_id: String, progress_seconds: i64) -> Result<(), String> {
    crate::settings::require_streets_feature("streets_cohort")?;

    let conn = crate::open_db_connection()?;
    seed_curriculum_if_needed(&conn)?;

    let updated = conn
        .execute(
            "UPDATE video_curriculum SET watch_progress_seconds = ?1 WHERE video_id = ?2",
            rusqlite::params![progress_seconds, video_id],
        )
        .map_err(|e| format!("Failed to update video progress: {}", e))?;

    if updated == 0 {
        return Err(format!("Video not found: {}", video_id));
    }

    debug!(
        target: "4da::video",
        video_id = %video_id,
        progress = progress_seconds,
        "Video progress updated"
    );

    Ok(())
}

/// Mark a video as fully watched.
/// Gate: requires streets_cohort membership.
#[tauri::command]
pub fn mark_video_complete(video_id: String) -> Result<(), String> {
    crate::settings::require_streets_feature("streets_cohort")?;

    let conn = crate::open_db_connection()?;
    seed_curriculum_if_needed(&conn)?;

    let updated = conn
        .execute(
            "UPDATE video_curriculum
             SET watched = 1, watched_at = datetime('now')
             WHERE video_id = ?1",
            rusqlite::params![video_id],
        )
        .map_err(|e| format!("Failed to mark video complete: {}", e))?;

    if updated == 0 {
        return Err(format!("Video not found: {}", video_id));
    }

    info!(
        target: "4da::video",
        video_id = %video_id,
        "Video marked complete"
    );

    Ok(())
}
