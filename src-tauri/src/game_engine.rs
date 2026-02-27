//! GAME Engine — Achievement tracking for 4DA
//!
//! Tracks user activity counters and unlocks achievements
//! when thresholds are reached. Stores state in SQLite.

use crate::db::Database;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Achievement definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub counter_type: String,
    pub threshold: u64,
}

/// Achievement unlock event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementUnlocked {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub unlocked_at: String,
}

/// Game state returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub counters: Vec<CounterState>,
    pub achievements: Vec<AchievementState>,
    pub streak: u32,
    pub last_active: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterState {
    pub counter_type: String,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementState {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub counter_type: String,
    pub threshold: u64,
    pub current: u64,
    pub unlocked: bool,
    pub unlocked_at: Option<String>,
}

/// All 13 achievements
fn all_achievements() -> Vec<Achievement> {
    vec![
        Achievement {
            id: "first_scan".into(),
            name: "First Light".into(),
            description: "Run your first content scan".into(),
            icon: "telescope".into(),
            counter_type: "scans".into(),
            threshold: 1,
        },
        Achievement {
            id: "ten_scans".into(),
            name: "Radar Operator".into(),
            description: "Run 10 content scans".into(),
            icon: "satellite".into(),
            counter_type: "scans".into(),
            threshold: 10,
        },
        Achievement {
            id: "fifty_scans".into(),
            name: "Signal Hunter".into(),
            description: "Run 50 content scans".into(),
            icon: "radar".into(),
            counter_type: "scans".into(),
            threshold: 50,
        },
        Achievement {
            id: "first_discovery".into(),
            name: "Eureka".into(),
            description: "Find your first relevant item".into(),
            icon: "lightbulb".into(),
            counter_type: "discoveries".into(),
            threshold: 1,
        },
        Achievement {
            id: "ten_discoveries".into(),
            name: "Pattern Spotter".into(),
            description: "Find 10 relevant items".into(),
            icon: "eye".into(),
            counter_type: "discoveries".into(),
            threshold: 10,
        },
        Achievement {
            id: "hundred_discoveries".into(),
            name: "Intelligence Analyst".into(),
            description: "Find 100 relevant items".into(),
            icon: "brain".into(),
            counter_type: "discoveries".into(),
            threshold: 100,
        },
        Achievement {
            id: "first_save".into(),
            name: "Collector".into(),
            description: "Save your first item".into(),
            icon: "bookmark".into(),
            counter_type: "saves".into(),
            threshold: 1,
        },
        Achievement {
            id: "first_briefing".into(),
            name: "Briefed".into(),
            description: "Generate your first briefing".into(),
            icon: "newspaper".into(),
            counter_type: "briefings".into(),
            threshold: 1,
        },
        Achievement {
            id: "three_sources".into(),
            name: "Multi-Source".into(),
            description: "Discover items from 3+ sources".into(),
            icon: "antenna".into(),
            counter_type: "sources".into(),
            threshold: 3,
        },
        Achievement {
            id: "five_sources".into(),
            name: "Intel Network".into(),
            description: "Discover items from 5+ sources".into(),
            icon: "globe".into(),
            counter_type: "sources".into(),
            threshold: 5,
        },
        Achievement {
            id: "context_builder".into(),
            name: "Context Builder".into(),
            description: "Set up 3 context items (role, tech, interests)".into(),
            icon: "puzzle".into(),
            counter_type: "context".into(),
            threshold: 3,
        },
        Achievement {
            id: "streak_three".into(),
            name: "Consistent".into(),
            description: "Use 4DA 3 days in a row".into(),
            icon: "flame".into(),
            counter_type: "streak".into(),
            threshold: 3,
        },
        Achievement {
            id: "streak_seven".into(),
            name: "Dedicated".into(),
            description: "Use 4DA 7 days in a row".into(),
            icon: "fire".into(),
            counter_type: "streak".into(),
            threshold: 7,
        },
    ]
}

/// Create game tables in the database
pub fn create_tables(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS game_counters (
            counter_type TEXT PRIMARY KEY,
            value INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS game_achievements (
            id TEXT PRIMARY KEY,
            unlocked_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS game_streak (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            current_streak INTEGER NOT NULL DEFAULT 0,
            last_active_date TEXT,
            longest_streak INTEGER NOT NULL DEFAULT 0
        );
        INSERT OR IGNORE INTO game_streak (id, current_streak, last_active_date, longest_streak)
            VALUES (1, 0, NULL, 0);",
    )?;
    Ok(())
}

/// Increment a counter and return any newly unlocked achievements
pub fn increment_counter(
    db: &Database,
    counter_type: &str,
    amount: u64,
) -> Vec<AchievementUnlocked> {
    let conn = db.conn.lock();
    let mut unlocked = Vec::new();

    // Update counter
    let new_value: u64 = match conn.query_row(
        "INSERT INTO game_counters (counter_type, value, updated_at) VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(counter_type) DO UPDATE SET value = value + ?2, updated_at = datetime('now')
         RETURNING value",
        rusqlite::params![counter_type, amount],
        |row| row.get(0),
    ) {
        Ok(v) => v,
        Err(e) => {
            debug!(target: "4da::game", error = %e, "Failed to update game counter");
            return unlocked;
        }
    };

    debug!(target: "4da::game", counter_type = %counter_type, new_value = new_value, "Counter incremented");

    // Update streak if this is a scan (daily activity indicator)
    if counter_type == "scans" {
        update_streak(&conn);
    }

    // Check for newly unlocked achievements
    for achievement in all_achievements() {
        if achievement.counter_type != counter_type {
            continue;
        }

        // For streak achievements, use streak value not counter
        let check_value = if counter_type == "streak" {
            get_current_streak(&conn) as u64
        } else {
            new_value
        };

        if check_value >= achievement.threshold {
            // Check if already unlocked
            let already: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM game_achievements WHERE id = ?1",
                    rusqlite::params![achievement.id],
                    |row| row.get(0),
                )
                .unwrap_or(true);

            if !already {
                let now = chrono::Utc::now().to_rfc3339();
                if let Err(e) = conn.execute(
                    "INSERT INTO game_achievements (id, unlocked_at) VALUES (?1, ?2)",
                    rusqlite::params![achievement.id, now],
                ) {
                    debug!(target: "4da::game", error = %e, "Failed to record achievement");
                    continue;
                }
                info!(target: "4da::game", id = %achievement.id, name = %achievement.name, "Achievement unlocked!");
                unlocked.push(AchievementUnlocked {
                    id: achievement.id.clone(),
                    name: achievement.name.clone(),
                    description: achievement.description.clone(),
                    icon: achievement.icon.clone(),
                    unlocked_at: now,
                });
            }
        }
    }

    unlocked
}

fn update_streak(conn: &rusqlite::Connection) {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let last_active: Option<String> = conn
        .query_row(
            "SELECT last_active_date FROM game_streak WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    match last_active {
        Some(ref last) if last == &today => {
            // Already active today, nothing to do
        }
        Some(ref last) => {
            // Check if yesterday
            if let Ok(last_date) = chrono::NaiveDate::parse_from_str(last, "%Y-%m-%d") {
                if let Ok(today_date) = chrono::NaiveDate::parse_from_str(&today, "%Y-%m-%d") {
                    let diff = (today_date - last_date).num_days();
                    if diff == 1 {
                        // Consecutive day — increment streak
                        let _ = conn.execute(
                            "UPDATE game_streak SET current_streak = current_streak + 1, last_active_date = ?1,
                             longest_streak = MAX(longest_streak, current_streak + 1) WHERE id = 1",
                            rusqlite::params![today],
                        );
                    } else {
                        // Streak broken — reset to 1
                        let _ = conn.execute(
                            "UPDATE game_streak SET current_streak = 1, last_active_date = ?1 WHERE id = 1",
                            rusqlite::params![today],
                        );
                    }
                }
            }
        }
        None => {
            // First ever activity
            let _ = conn.execute(
                "UPDATE game_streak SET current_streak = 1, last_active_date = ?1, longest_streak = 1 WHERE id = 1",
                rusqlite::params![today],
            );
        }
    }

    // Check streak achievements
    let streak = get_current_streak(conn);
    for achievement in all_achievements() {
        if achievement.counter_type != "streak" {
            continue;
        }
        if streak >= achievement.threshold as u32 {
            let already: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM game_achievements WHERE id = ?1",
                    rusqlite::params![achievement.id],
                    |row| row.get(0),
                )
                .unwrap_or(true);
            if !already {
                let now = chrono::Utc::now().to_rfc3339();
                let _ = conn.execute(
                    "INSERT INTO game_achievements (id, unlocked_at) VALUES (?1, ?2)",
                    rusqlite::params![achievement.id, now],
                );
                info!(target: "4da::game", id = %achievement.id, "Streak achievement unlocked!");
            }
        }
    }
}

fn get_current_streak(conn: &rusqlite::Connection) -> u32 {
    conn.query_row(
        "SELECT current_streak FROM game_streak WHERE id = 1",
        [],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// Get the full game state
pub fn get_game_state(db: &Database) -> GameState {
    let conn = db.conn.lock();
    let achievements_def = all_achievements();

    // Get all counters
    let mut counters = Vec::new();
    if let Ok(mut stmt) = conn.prepare("SELECT counter_type, value FROM game_counters") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok(CounterState {
                counter_type: row.get(0)?,
                value: row.get(1)?,
            })
        }) {
            for row in rows.flatten() {
                counters.push(row);
            }
        }
    }

    // Get unlocked achievement IDs
    let mut unlocked_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    if let Ok(mut stmt) = conn.prepare("SELECT id, unlocked_at FROM game_achievements") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for row in rows.flatten() {
                unlocked_map.insert(row.0, row.1);
            }
        }
    }

    // Build counter lookup
    let counter_lookup: std::collections::HashMap<&str, u64> = counters
        .iter()
        .map(|c| (c.counter_type.as_str(), c.value))
        .collect();

    let streak = get_current_streak(&conn);

    // Build achievement states
    let achievements: Vec<AchievementState> = achievements_def
        .iter()
        .map(|a| {
            let current = if a.counter_type == "streak" {
                streak as u64
            } else {
                counter_lookup
                    .get(a.counter_type.as_str())
                    .copied()
                    .unwrap_or(0)
            };
            let unlocked_at = unlocked_map.get(&a.id).cloned();
            AchievementState {
                id: a.id.clone(),
                name: a.name.clone(),
                description: a.description.clone(),
                icon: a.icon.clone(),
                counter_type: a.counter_type.clone(),
                threshold: a.threshold,
                current,
                unlocked: unlocked_at.is_some(),
                unlocked_at,
            }
        })
        .collect();

    let last_active: Option<String> = conn
        .query_row(
            "SELECT last_active_date FROM game_streak WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    GameState {
        counters,
        achievements,
        streak,
        last_active,
    }
}

/// Get just the list of unlocked achievements
#[allow(dead_code)] // Called from game_commands::get_achievements (reserved for future frontend use)
pub fn get_achievements(db: &Database) -> Vec<AchievementUnlocked> {
    let conn = db.conn.lock();
    let achievements_def = all_achievements();
    let mut result = Vec::new();

    if let Ok(mut stmt) = conn.prepare("SELECT id, unlocked_at FROM game_achievements") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for row in rows.flatten() {
                if let Some(a) = achievements_def.iter().find(|a| a.id == row.0) {
                    result.push(AchievementUnlocked {
                        id: a.id.clone(),
                        name: a.name.clone(),
                        description: a.description.clone(),
                        icon: a.icon.clone(),
                        unlocked_at: row.1,
                    });
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_achievements_count() {
        let achievements = all_achievements();
        assert_eq!(
            achievements.len(),
            13,
            "Should have exactly 13 achievements"
        );
    }

    #[test]
    fn test_all_achievements_unique_ids() {
        let achievements = all_achievements();
        let mut ids: Vec<&str> = achievements.iter().map(|a| a.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 13, "All achievement IDs should be unique");
    }

    #[test]
    fn test_all_achievements_have_required_fields() {
        for a in all_achievements() {
            assert!(!a.id.is_empty(), "Achievement ID should not be empty");
            assert!(!a.name.is_empty(), "Achievement name should not be empty");
            assert!(
                !a.description.is_empty(),
                "Achievement description should not be empty"
            );
            assert!(!a.icon.is_empty(), "Achievement icon should not be empty");
            assert!(
                !a.counter_type.is_empty(),
                "Achievement counter_type should not be empty"
            );
            assert!(a.threshold > 0, "Achievement threshold should be positive");
        }
    }

    #[test]
    fn test_all_achievements_counter_types() {
        let achievements = all_achievements();
        let valid_types = [
            "scans",
            "discoveries",
            "saves",
            "briefings",
            "sources",
            "context",
            "streak",
        ];
        for a in &achievements {
            assert!(
                valid_types.contains(&a.counter_type.as_str()),
                "Unknown counter type '{}' in achievement '{}'",
                a.counter_type,
                a.id
            );
        }
    }

    #[test]
    fn test_scan_achievements_ordered_thresholds() {
        let achievements = all_achievements();
        let scan_thresholds: Vec<u64> = achievements
            .iter()
            .filter(|a| a.counter_type == "scans")
            .map(|a| a.threshold)
            .collect();
        // Scan achievements: 1, 10, 50
        assert_eq!(scan_thresholds, vec![1, 10, 50]);
    }

    #[test]
    fn test_discovery_achievements_ordered_thresholds() {
        let achievements = all_achievements();
        let thresholds: Vec<u64> = achievements
            .iter()
            .filter(|a| a.counter_type == "discoveries")
            .map(|a| a.threshold)
            .collect();
        assert_eq!(thresholds, vec![1, 10, 100]);
    }

    #[test]
    fn test_streak_achievements_ordered_thresholds() {
        let achievements = all_achievements();
        let thresholds: Vec<u64> = achievements
            .iter()
            .filter(|a| a.counter_type == "streak")
            .map(|a| a.threshold)
            .collect();
        assert_eq!(thresholds, vec![3, 7]);
    }

    #[test]
    fn test_source_achievements_ordered_thresholds() {
        let achievements = all_achievements();
        let thresholds: Vec<u64> = achievements
            .iter()
            .filter(|a| a.counter_type == "sources")
            .map(|a| a.threshold)
            .collect();
        assert_eq!(thresholds, vec![3, 5]);
    }

    #[test]
    fn test_first_scan_achievement() {
        let achievements = all_achievements();
        let first = achievements.iter().find(|a| a.id == "first_scan").unwrap();
        assert_eq!(first.threshold, 1);
        assert_eq!(first.counter_type, "scans");
        assert_eq!(first.name, "First Light");
    }

    #[test]
    fn test_context_builder_achievement() {
        let achievements = all_achievements();
        let ctx = achievements
            .iter()
            .find(|a| a.id == "context_builder")
            .unwrap();
        assert_eq!(ctx.threshold, 3);
        assert_eq!(ctx.counter_type, "context");
    }
}
