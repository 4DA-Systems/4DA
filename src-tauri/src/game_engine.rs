//! GAME Engine -- achievement tracking and visual feedback for 4DA.
//! Tracks developer milestones and emits celebration events.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::db::Database;

/// An achievement definition with current progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub threshold: u64,
    pub unlocked: bool,
    pub unlocked_at: Option<String>,
    pub progress: u64,
}

/// Event payload when an achievement is unlocked
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementUnlocked {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
}

/// Achievement definitions: (id, title, description, icon, counter_type, threshold)
const ACHIEVEMENTS: &[(&str, &str, &str, &str, &str, u64)] = &[
    (
        "first_scan",
        "First Light",
        "Run your first analysis",
        "sun",
        "scans",
        1,
    ),
    (
        "ten_scans",
        "Pattern Seeker",
        "Run 10 analyses",
        "eye",
        "scans",
        10,
    ),
    (
        "fifty_scans",
        "Signal Hunter",
        "Run 50 analyses",
        "radar",
        "scans",
        50,
    ),
    (
        "first_discovery",
        "Eureka",
        "Find your first high-relevance item",
        "sparkle",
        "discoveries",
        1,
    ),
    (
        "ten_discoveries",
        "Gold Miner",
        "Find 10 high-relevance items",
        "gem",
        "discoveries",
        10,
    ),
    (
        "first_save",
        "Collector",
        "Save your first item",
        "bookmark",
        "saves",
        1,
    ),
    (
        "ten_saves",
        "Curator",
        "Save 10 items",
        "archive",
        "saves",
        10,
    ),
    (
        "first_briefing",
        "Intelligence Brief",
        "Generate your first briefing",
        "scroll",
        "briefings",
        1,
    ),
    (
        "streak_3",
        "Momentum",
        "Use 4DA 3 days in a row",
        "fire",
        "streak",
        3,
    ),
    (
        "streak_7",
        "Discipline",
        "Use 4DA 7 days in a row",
        "flame",
        "streak",
        7,
    ),
    (
        "streak_30",
        "Relentless",
        "Use 4DA 30 days in a row",
        "crown",
        "streak",
        30,
    ),
    (
        "sources_3",
        "Network Builder",
        "Configure 3+ source types",
        "globe",
        "sources",
        3,
    ),
    (
        "context_set",
        "Self Aware",
        "Set up your developer context",
        "brain",
        "context",
        1,
    ),
];

/// Initialize the GAME tables (called from migration)
pub fn create_tables(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS game_counters (
            counter_type TEXT PRIMARY KEY,
            value INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS game_unlocked (
            achievement_id TEXT PRIMARY KEY,
            unlocked_at DATETIME NOT NULL
        );
        CREATE TABLE IF NOT EXISTS game_activity (
            day TEXT PRIMARY KEY
        );",
    )
}

/// Increment a counter and check if any achievements were newly unlocked.
/// Returns a list of newly unlocked achievements.
pub fn increment_counter(
    db: &Arc<Database>,
    counter_type: &str,
    amount: u64,
) -> Vec<AchievementUnlocked> {
    let conn = db.conn.lock();
    let mut newly_unlocked = Vec::new();

    // Upsert the counter (keyed by counter_type)
    if conn
        .execute(
            "INSERT INTO game_counters (counter_type, value) VALUES (?1, ?2)
         ON CONFLICT(counter_type) DO UPDATE SET value = value + ?2",
            rusqlite::params![counter_type, amount as i64],
        )
        .is_err()
    {
        return newly_unlocked;
    }

    // Read current counter value
    let current: i64 = conn
        .query_row(
            "SELECT value FROM game_counters WHERE counter_type = ?1",
            rusqlite::params![counter_type],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Check all achievements that use this counter_type
    for &(id, title, description, icon, ct, threshold) in ACHIEVEMENTS {
        if ct != counter_type {
            continue;
        }
        if (current as u64) < threshold {
            continue;
        }

        // Check if already unlocked
        let already: bool = conn
            .query_row(
                "SELECT 1 FROM game_unlocked WHERE achievement_id = ?1",
                rusqlite::params![id],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !already {
            let _ = conn.execute(
                "INSERT INTO game_unlocked (achievement_id, unlocked_at) VALUES (?1, datetime('now'))",
                rusqlite::params![id],
            );
            info!(target: "4da::game", achievement = id, title, "Achievement unlocked!");
            newly_unlocked.push(AchievementUnlocked {
                id: id.to_string(),
                title: title.to_string(),
                description: description.to_string(),
                icon: icon.to_string(),
            });
        }
    }

    newly_unlocked
}

/// Record today's activity for streak tracking, returns newly unlocked streak achievements
pub fn record_daily_activity(db: &Arc<Database>) -> Vec<AchievementUnlocked> {
    let conn = db.conn.lock();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let _ = conn.execute(
        "INSERT OR IGNORE INTO game_activity (day) VALUES (?1)",
        rusqlite::params![today],
    );

    // Calculate current streak
    let streak = calculate_streak(&conn);

    // Set streak counter to current streak value (absolute, not incremental)
    let _ = conn.execute(
        "INSERT INTO game_counters (counter_type, value) VALUES ('streak', ?1)
         ON CONFLICT(counter_type) DO UPDATE SET value = ?1",
        rusqlite::params![streak as i64],
    );

    let mut unlocked = Vec::new();

    // Check streak achievements
    for &(id, title, description, icon, ct, threshold) in ACHIEVEMENTS {
        if ct != "streak" {
            continue;
        }
        if streak < threshold {
            continue;
        }

        let already: bool = conn
            .query_row(
                "SELECT 1 FROM game_unlocked WHERE achievement_id = ?1",
                rusqlite::params![id],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !already {
            let _ = conn.execute(
                "INSERT INTO game_unlocked (achievement_id, unlocked_at) VALUES (?1, datetime('now'))",
                rusqlite::params![id],
            );
            info!(target: "4da::game", achievement = id, title, "Streak achievement unlocked!");
            unlocked.push(AchievementUnlocked {
                id: id.to_string(),
                title: title.to_string(),
                description: description.to_string(),
                icon: icon.to_string(),
            });
        }
    }

    unlocked
}

fn calculate_streak(conn: &rusqlite::Connection) -> u64 {
    let mut stmt = match conn.prepare("SELECT day FROM game_activity ORDER BY day DESC") {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let days: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    if days.is_empty() {
        return 0;
    }

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut streak = 0u64;
    let mut expected = chrono::Local::now().date_naive();

    // If today isn't in the list, start from yesterday
    if days.first().map(|d| d.as_str()) != Some(today.as_str()) {
        expected = expected.pred_opt().unwrap_or(expected);
    }

    for day_str in &days {
        if let Ok(day) = chrono::NaiveDate::parse_from_str(day_str, "%Y-%m-%d") {
            if day == expected {
                streak += 1;
                expected = expected.pred_opt().unwrap_or(expected);
            } else if day < expected {
                break;
            }
        }
    }

    streak
}

/// Get all achievements with their current progress
pub fn get_all_achievements(db: &Arc<Database>) -> Vec<Achievement> {
    let conn = db.conn.lock();

    ACHIEVEMENTS
        .iter()
        .map(|&(id, title, description, icon, counter_type, threshold)| {
            let progress: i64 = conn
                .query_row(
                    "SELECT value FROM game_counters WHERE counter_type = ?1",
                    rusqlite::params![counter_type],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            let (unlocked, unlocked_at) = conn
                .query_row(
                    "SELECT 1, unlocked_at FROM game_unlocked WHERE achievement_id = ?1",
                    rusqlite::params![id],
                    |row| Ok((true, row.get::<_, String>(1).ok())),
                )
                .unwrap_or((false, None));

            Achievement {
                id: id.to_string(),
                title: title.to_string(),
                description: description.to_string(),
                icon: icon.to_string(),
                threshold,
                unlocked,
                unlocked_at,
                progress: progress as u64,
            }
        })
        .collect()
}

/// Full game state summary for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub total_unlocked: usize,
    pub total_achievements: usize,
    pub current_streak: u64,
    pub achievements: Vec<Achievement>,
}

pub fn get_game_state(db: &Arc<Database>) -> GameState {
    let achievements = get_all_achievements(db);
    let total_unlocked = achievements.iter().filter(|a| a.unlocked).count();
    let conn = db.conn.lock();
    let streak = calculate_streak(&conn);

    GameState {
        total_unlocked,
        total_achievements: ACHIEVEMENTS.len(),
        current_streak: streak,
        achievements,
    }
}
