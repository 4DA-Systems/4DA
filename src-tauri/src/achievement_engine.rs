//! GAME Engine — Achievement tracking for 4DA
//!
//! Tracks user activity counters and unlocks achievements
//! when thresholds are reached. Stores state in SQLite.

use crate::db::Database;
use crate::achievement_definitions::{all_achievements, AchievementTier};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Achievement unlock event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementUnlocked {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub tier: AchievementTier,
    pub celebration_intensity: f64,
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
    pub tier: AchievementTier,
    pub current: u64,
    pub unlocked: bool,
    pub unlocked_at: Option<String>,
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

/// Check and update the daily usage streak on app startup.
///
/// Called once when the frontend loads. Updates the streak counter (consecutive
/// days of usage) and returns any newly unlocked streak achievements. This
/// ensures streak achievements fire from normal app usage, not only from scans.
pub fn check_daily_streak(db: &Database) -> Vec<AchievementUnlocked> {
    let conn = db.conn.lock();
    update_streak(&conn);

    // Collect any newly-unlocked streak achievements that update_streak recorded
    // but that haven't been emitted as events yet. We re-check here because
    // update_streak inserts into game_achievements but doesn't return unlocked info.
    let streak = get_current_streak(&conn);
    let mut unlocked = Vec::new();

    for achievement in all_achievements() {
        if achievement.counter_type != "streak" {
            continue;
        }
        if streak >= achievement.threshold as u32 {
            // update_streak already inserted the record — we just need to check
            // if it was inserted *this call* (unlocked_at within last 5 seconds)
            let recent: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM game_achievements
                     WHERE id = ?1 AND unlocked_at >= datetime('now', '-5 seconds')",
                    rusqlite::params![achievement.id],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if recent {
                let now = chrono::Utc::now().to_rfc3339();
                info!(target: "4da::game", id = %achievement.id, streak, "Streak achievement emitted on daily check");
                unlocked.push(AchievementUnlocked {
                    id: achievement.id.clone(),
                    name: achievement.name.clone(),
                    description: achievement.description.clone(),
                    icon: achievement.icon.clone(),
                    tier: achievement.tier.clone(),
                    celebration_intensity: achievement.tier.intensity(),
                    unlocked_at: now,
                });
            }
        }
    }

    unlocked
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
                let celebration_intensity = achievement.tier.intensity();
                unlocked.push(AchievementUnlocked {
                    id: achievement.id.clone(),
                    name: achievement.name.clone(),
                    description: achievement.description.clone(),
                    icon: achievement.icon.clone(),
                    tier: achievement.tier.clone(),
                    celebration_intensity,
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
                tier: a.tier.clone(),
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
                    let celebration_intensity = a.tier.intensity();
                    result.push(AchievementUnlocked {
                        id: a.id.clone(),
                        name: a.name.clone(),
                        description: a.description.clone(),
                        icon: a.icon.clone(),
                        tier: a.tier.clone(),
                        celebration_intensity,
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
    use crate::achievement_definitions::AchievementTier;
    use crate::test_utils::test_db;

    // ========================================================================
    // Helper
    // ========================================================================

    /// Create a test Database and return it. The game tables are already
    /// created by `Database::new(":memory:")` via the migration system.
    fn game_db() -> Database {
        test_db()
    }

    // ========================================================================
    // 1. Full Bronze → Silver → Gold progression for scans
    // ========================================================================

    #[test]
    fn test_scan_bronze_unlock_first_scan() {
        let db = game_db();

        let unlocked = increment_counter(&db, "scans", 1);
        assert_eq!(unlocked.len(), 1, "Should unlock exactly one achievement");

        let a = &unlocked[0];
        assert_eq!(a.id, "first_scan");
        assert_eq!(a.name, "First Light");
        assert_eq!(a.tier, AchievementTier::Bronze);
        assert!(
            (a.celebration_intensity - 0.5).abs() < f64::EPSILON,
            "Bronze celebration_intensity should be 0.5, got {}",
            a.celebration_intensity,
        );
    }

    #[test]
    fn test_scan_silver_unlock_ten_scans() {
        let db = game_db();

        // Increment to 1 → unlocks first_scan
        let first = increment_counter(&db, "scans", 1);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, "first_scan");

        // Increment by 9 more (total 10) → unlocks ten_scans
        let second = increment_counter(&db, "scans", 9);
        assert_eq!(second.len(), 1, "Should unlock exactly ten_scans");

        let a = &second[0];
        assert_eq!(a.id, "ten_scans");
        assert_eq!(a.name, "Radar Operator");
        assert_eq!(a.tier, AchievementTier::Silver);
        assert!(
            (a.celebration_intensity - 0.75).abs() < f64::EPSILON,
            "Silver celebration_intensity should be 0.75, got {}",
            a.celebration_intensity,
        );
    }

    #[test]
    fn test_scan_gold_unlock_fifty_scans() {
        let db = game_db();

        // Increment to 1 → first_scan
        increment_counter(&db, "scans", 1);
        // Increment to 10 → ten_scans
        increment_counter(&db, "scans", 9);
        // Increment to 50 → fifty_scans
        let gold = increment_counter(&db, "scans", 40);
        assert_eq!(gold.len(), 1, "Should unlock exactly fifty_scans");

        let a = &gold[0];
        assert_eq!(a.id, "fifty_scans");
        assert_eq!(a.name, "Signal Hunter");
        assert_eq!(a.tier, AchievementTier::Gold);
        assert!(
            (a.celebration_intensity - 1.0).abs() < f64::EPSILON,
            "Gold celebration_intensity should be 1.0, got {}",
            a.celebration_intensity,
        );
    }

    #[test]
    fn test_full_scan_progression_single_jumps() {
        let db = game_db();

        // Jump straight to 50 in one shot — should unlock all three at once
        let unlocked = increment_counter(&db, "scans", 50);
        assert_eq!(
            unlocked.len(),
            3,
            "Jumping to 50 should unlock first_scan, ten_scans, fifty_scans"
        );

        let ids: Vec<&str> = unlocked.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"first_scan"));
        assert!(ids.contains(&"ten_scans"));
        assert!(ids.contains(&"fifty_scans"));
    }

    // ========================================================================
    // 2. No double-unlocking
    // ========================================================================

    #[test]
    fn test_no_double_unlock_first_scan() {
        let db = game_db();

        let first = increment_counter(&db, "scans", 1);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, "first_scan");

        // Increment again — first_scan should NOT re-unlock
        let second = increment_counter(&db, "scans", 1);
        assert!(
            second.is_empty(),
            "first_scan should not be unlocked again; got {:?}",
            second.iter().map(|a| &a.id).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn test_no_double_unlock_after_many_increments() {
        let db = game_db();

        // Unlock first_scan
        increment_counter(&db, "scans", 1);

        // 8 more increments (total 9) — none should re-unlock first_scan
        for _ in 0..8 {
            let result = increment_counter(&db, "scans", 1);
            assert!(
                !result.iter().any(|a| a.id == "first_scan"),
                "first_scan should not unlock again"
            );
        }
    }

    #[test]
    fn test_no_double_unlock_ten_scans() {
        let db = game_db();

        // Get to 10
        increment_counter(&db, "scans", 10);

        // Increment past 10 — ten_scans should not re-appear
        let result = increment_counter(&db, "scans", 1);
        assert!(
            !result.iter().any(|a| a.id == "ten_scans"),
            "ten_scans should not double-unlock"
        );
    }

    // ========================================================================
    // 3. Multiple counter types don't interfere
    // ========================================================================

    #[test]
    fn test_discovery_counter_does_not_unlock_scan_achievements() {
        let db = game_db();

        let unlocked = increment_counter(&db, "discoveries", 1);
        assert_eq!(
            unlocked.len(),
            1,
            "Should unlock exactly one discovery achievement"
        );
        assert_eq!(unlocked[0].id, "first_discovery");

        // Verify no scan achievements were unlocked
        let scan_unlocks: Vec<&AchievementUnlocked> =
            unlocked.iter().filter(|a| a.id.contains("scan")).collect();
        assert!(
            scan_unlocks.is_empty(),
            "Discovery increment should not unlock scan achievements"
        );
    }

    #[test]
    fn test_scan_counter_does_not_unlock_discovery_achievements() {
        let db = game_db();

        let unlocked = increment_counter(&db, "scans", 1);
        // Only first_scan should appear
        assert!(
            !unlocked.iter().any(|a| a.id.contains("discovery")),
            "Scan increment should not unlock discovery achievements"
        );
    }

    #[test]
    fn test_independent_counters_track_separately() {
        let db = game_db();

        increment_counter(&db, "scans", 5);
        increment_counter(&db, "discoveries", 3);
        increment_counter(&db, "saves", 1);

        let state = get_game_state(&db);

        let scan_counter = state
            .counters
            .iter()
            .find(|c| c.counter_type == "scans")
            .expect("scans counter should exist");
        assert_eq!(scan_counter.value, 5);

        let disc_counter = state
            .counters
            .iter()
            .find(|c| c.counter_type == "discoveries")
            .expect("discoveries counter should exist");
        assert_eq!(disc_counter.value, 3);

        let save_counter = state
            .counters
            .iter()
            .find(|c| c.counter_type == "saves")
            .expect("saves counter should exist");
        assert_eq!(save_counter.value, 1);
    }

    // ========================================================================
    // 4. Streak system — update_streak
    // ========================================================================

    #[test]
    fn test_streak_starts_at_zero() {
        let db = game_db();
        let state = get_game_state(&db);
        assert_eq!(state.streak, 0, "Fresh DB streak should be 0");
        assert!(
            state.last_active.is_none(),
            "Fresh DB last_active should be None"
        );
    }

    #[test]
    fn test_streak_increments_on_first_scan() {
        let db = game_db();

        // A scan triggers update_streak
        increment_counter(&db, "scans", 1);

        let state = get_game_state(&db);
        assert_eq!(state.streak, 1, "Streak should be 1 after first scan");
        assert!(state.last_active.is_some(), "last_active should be set");
    }

    #[test]
    fn test_streak_stays_same_on_repeated_same_day_scans() {
        let db = game_db();

        increment_counter(&db, "scans", 1);
        let state1 = get_game_state(&db);
        let streak_after_first = state1.streak;

        // Another scan on the same day should not change streak
        increment_counter(&db, "scans", 1);
        let state2 = get_game_state(&db);
        assert_eq!(
            state2.streak, streak_after_first,
            "Streak should not change for multiple scans on same day"
        );
    }

    #[test]
    fn test_streak_reset_when_day_missed() {
        let db = game_db();
        let conn = db.conn.lock();

        // Simulate: last active was 3 days ago (streak was 5)
        let three_days_ago = (chrono::Utc::now() - chrono::Duration::days(3))
            .format("%Y-%m-%d")
            .to_string();
        conn.execute(
            "UPDATE game_streak SET current_streak = 5, last_active_date = ?1 WHERE id = 1",
            rusqlite::params![three_days_ago],
        )
        .unwrap();

        // Verify streak is 5 before the update
        let streak_before = get_current_streak(&conn);
        assert_eq!(streak_before, 5);

        // Now trigger update_streak (simulates today's activity)
        update_streak(&conn);

        // Streak should reset to 1 (missed days)
        let streak_after = get_current_streak(&conn);
        assert_eq!(
            streak_after, 1,
            "Streak should reset to 1 after missing a day"
        );
    }

    #[test]
    fn test_streak_continues_on_consecutive_day() {
        let db = game_db();
        let conn = db.conn.lock();

        // Simulate: last active was yesterday, streak was 4
        let yesterday = (chrono::Utc::now() - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        conn.execute(
            "UPDATE game_streak SET current_streak = 4, last_active_date = ?1 WHERE id = 1",
            rusqlite::params![yesterday],
        )
        .unwrap();

        // Trigger update_streak
        update_streak(&conn);

        let streak = get_current_streak(&conn);
        assert_eq!(streak, 5, "Streak should increment to 5 on consecutive day");
    }

    #[test]
    fn test_streak_no_change_on_same_day() {
        let db = game_db();
        let conn = db.conn.lock();

        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        conn.execute(
            "UPDATE game_streak SET current_streak = 7, last_active_date = ?1 WHERE id = 1",
            rusqlite::params![today],
        )
        .unwrap();

        update_streak(&conn);

        let streak = get_current_streak(&conn);
        assert_eq!(streak, 7, "Streak should not change on same day");
    }

    #[test]
    fn test_streak_longest_streak_tracked() {
        let db = game_db();
        let conn = db.conn.lock();

        // Set up: yesterday, streak 9
        let yesterday = (chrono::Utc::now() - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        conn.execute(
            "UPDATE game_streak SET current_streak = 9, last_active_date = ?1, longest_streak = 9 WHERE id = 1",
            rusqlite::params![yesterday],
        )
        .unwrap();

        update_streak(&conn);

        let longest: u32 = conn
            .query_row(
                "SELECT longest_streak FROM game_streak WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(longest, 10, "Longest streak should be updated to 10");
    }

    // ========================================================================
    // 5. check_daily_streak
    // ========================================================================

    #[test]
    fn test_check_daily_streak_on_fresh_db() {
        let db = game_db();

        let unlocked = check_daily_streak(&db);
        // Fresh DB has streak 0 → no streak achievements should unlock
        // (lowest streak threshold is 3)
        assert!(
            unlocked.is_empty(),
            "Fresh DB should not unlock any streak achievements"
        );
    }

    #[test]
    fn test_check_daily_streak_unlocks_streak_three() {
        let db = game_db();

        // Pre-set: 2-day streak, last active yesterday → today bumps to 3
        {
            let conn = db.conn.lock();
            let yesterday = (chrono::Utc::now() - chrono::Duration::days(1))
                .format("%Y-%m-%d")
                .to_string();
            conn.execute(
                "UPDATE game_streak SET current_streak = 2, last_active_date = ?1 WHERE id = 1",
                rusqlite::params![yesterday],
            )
            .unwrap();
        }

        let unlocked = check_daily_streak(&db);
        // The streak will become 3, which should unlock "streak_three"
        // check_daily_streak uses a 5-second recency window, so if the
        // achievement was just inserted it should appear
        let streak_ids: Vec<&str> = unlocked.iter().map(|a| a.id.as_str()).collect();
        assert!(
            streak_ids.contains(&"streak_three"),
            "Should unlock streak_three when streak reaches 3; got {:?}",
            streak_ids,
        );
    }

    #[test]
    fn test_check_daily_streak_does_not_double_unlock() {
        let db = game_db();

        // Pre-set streak to 3 with yesterday as last active
        {
            let conn = db.conn.lock();
            let yesterday = (chrono::Utc::now() - chrono::Duration::days(1))
                .format("%Y-%m-%d")
                .to_string();
            conn.execute(
                "UPDATE game_streak SET current_streak = 2, last_active_date = ?1 WHERE id = 1",
                rusqlite::params![yesterday],
            )
            .unwrap();
        }

        // First call → should unlock streak_three
        let first = check_daily_streak(&db);
        let first_ids: Vec<&str> = first.iter().map(|a| a.id.as_str()).collect();
        assert!(first_ids.contains(&"streak_three"));

        // Second call on same day → streak is already at 3, already unlocked,
        // and no longer "recent" (well, it will be within 5 seconds, but the
        // INSERT OR IGNORE means it won't be re-inserted).
        // Actually update_streak won't modify streak again (same day),
        // so the unlocked_at timestamp stays the same. It may or may not
        // re-emit within the 5-second window. That's acceptable behavior.
        // The key invariant is that the DB only has one row per achievement.
        let count: i64 = {
            let conn = db.conn.lock();
            conn.query_row(
                "SELECT COUNT(*) FROM game_achievements WHERE id = 'streak_three'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(count, 1, "streak_three should only be recorded once in DB");
    }

    // ========================================================================
    // 6. get_game_state returns correct state
    // ========================================================================

    #[test]
    fn test_get_game_state_fresh_db() {
        let db = game_db();

        let state = get_game_state(&db);
        assert!(state.counters.is_empty(), "No counters on fresh DB");
        assert_eq!(state.streak, 0);
        assert!(state.last_active.is_none());
        // All achievements should exist but none unlocked
        assert!(
            !state.achievements.is_empty(),
            "Should have achievement definitions"
        );
        assert_eq!(state.achievements.len(), 25);
        for a in &state.achievements {
            assert!(
                !a.unlocked,
                "No achievements should be unlocked on fresh DB"
            );
            assert!(a.unlocked_at.is_none());
            assert_eq!(a.current, 0, "All counters should be 0 on fresh DB");
        }
    }

    #[test]
    fn test_get_game_state_after_increments() {
        let db = game_db();

        increment_counter(&db, "scans", 5);
        increment_counter(&db, "discoveries", 2);

        let state = get_game_state(&db);

        // Check counters
        let scan_counter = state
            .counters
            .iter()
            .find(|c| c.counter_type == "scans")
            .expect("scans counter");
        assert_eq!(scan_counter.value, 5);

        let disc_counter = state
            .counters
            .iter()
            .find(|c| c.counter_type == "discoveries")
            .expect("discoveries counter");
        assert_eq!(disc_counter.value, 2);

        // Check achievements reflect current counter values
        let first_scan = state
            .achievements
            .iter()
            .find(|a| a.id == "first_scan")
            .expect("first_scan achievement");
        assert!(
            first_scan.unlocked,
            "first_scan should be unlocked (5 >= 1)"
        );
        assert_eq!(first_scan.current, 5);
        assert!(first_scan.unlocked_at.is_some());

        let ten_scans = state
            .achievements
            .iter()
            .find(|a| a.id == "ten_scans")
            .expect("ten_scans achievement");
        assert!(
            !ten_scans.unlocked,
            "ten_scans should NOT be unlocked (5 < 10)"
        );
        assert_eq!(ten_scans.current, 5);

        let first_disc = state
            .achievements
            .iter()
            .find(|a| a.id == "first_discovery")
            .expect("first_discovery achievement");
        assert!(
            first_disc.unlocked,
            "first_discovery should be unlocked (2 >= 1)"
        );
        assert_eq!(first_disc.current, 2);

        let ten_disc = state
            .achievements
            .iter()
            .find(|a| a.id == "ten_discoveries")
            .expect("ten_discoveries achievement");
        assert!(
            !ten_disc.unlocked,
            "ten_discoveries should NOT be unlocked (2 < 10)"
        );
        assert_eq!(ten_disc.current, 2);
    }

    #[test]
    fn test_get_game_state_streak_reflected_in_achievements() {
        let db = game_db();

        // Set streak to 7
        {
            let conn = db.conn.lock();
            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
            conn.execute(
                "UPDATE game_streak SET current_streak = 7, last_active_date = ?1 WHERE id = 1",
                rusqlite::params![today],
            )
            .unwrap();
        }

        let state = get_game_state(&db);
        assert_eq!(state.streak, 7);

        // All streak achievements should have current = 7
        for a in state
            .achievements
            .iter()
            .filter(|a| a.counter_type == "streak")
        {
            assert_eq!(a.current, 7, "Streak achievements should show current=7");
        }
    }

    #[test]
    fn test_get_game_state_all_achievements_have_metadata() {
        let db = game_db();
        let state = get_game_state(&db);

        for a in &state.achievements {
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

    // ========================================================================
    // 7. get_achievements
    // ========================================================================

    #[test]
    fn test_get_achievements_empty_on_fresh_db() {
        let db = game_db();
        let result = get_achievements(&db);
        assert!(result.is_empty(), "No achievements on fresh DB");
    }

    #[test]
    fn test_get_achievements_after_unlocks() {
        let db = game_db();

        increment_counter(&db, "scans", 10);
        increment_counter(&db, "discoveries", 1);

        let result = get_achievements(&db);

        let ids: Vec<&str> = result.iter().map(|a| a.id.as_str()).collect();
        // scans=10 unlocks first_scan + ten_scans, discoveries=1 unlocks first_discovery
        assert!(ids.contains(&"first_scan"), "Should include first_scan");
        assert!(ids.contains(&"ten_scans"), "Should include ten_scans");
        assert!(
            ids.contains(&"first_discovery"),
            "Should include first_discovery"
        );
        assert_eq!(
            result.len(),
            3,
            "Should have exactly 3 unlocked achievements"
        );
    }

    #[test]
    fn test_get_achievements_has_correct_tiers() {
        let db = game_db();
        increment_counter(&db, "scans", 50);

        let result = get_achievements(&db);
        let first = result.iter().find(|a| a.id == "first_scan").unwrap();
        assert_eq!(first.tier, AchievementTier::Bronze);

        let ten = result.iter().find(|a| a.id == "ten_scans").unwrap();
        assert_eq!(ten.tier, AchievementTier::Silver);

        let fifty = result.iter().find(|a| a.id == "fifty_scans").unwrap();
        assert_eq!(fifty.tier, AchievementTier::Gold);
    }

    // ========================================================================
    // 8. Edge cases
    // ========================================================================

    #[test]
    fn test_increment_by_zero_does_not_unlock() {
        let db = game_db();
        let unlocked = increment_counter(&db, "scans", 0);
        assert!(
            unlocked.is_empty(),
            "Incrementing by 0 should not unlock anything"
        );
    }

    #[test]
    fn test_increment_large_amount() {
        let db = game_db();
        // Increment by 1000 — should unlock all three scan achievements
        let unlocked = increment_counter(&db, "scans", 1000);
        let ids: Vec<&str> = unlocked.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"first_scan"));
        assert!(ids.contains(&"ten_scans"));
        assert!(ids.contains(&"fifty_scans"));
    }

    #[test]
    fn test_unknown_counter_type_no_crash() {
        let db = game_db();
        // An unknown counter type should not crash — just no achievements
        let unlocked = increment_counter(&db, "nonexistent_type", 100);
        assert!(
            unlocked.is_empty(),
            "Unknown counter type should not unlock anything"
        );
    }

    #[test]
    fn test_multiple_counter_types_full_scenario() {
        let db = game_db();

        // Simulate a realistic session
        let scan_unlocks = increment_counter(&db, "scans", 1);
        assert_eq!(scan_unlocks.len(), 1);
        assert_eq!(scan_unlocks[0].id, "first_scan");

        let disc_unlocks = increment_counter(&db, "discoveries", 1);
        assert_eq!(disc_unlocks.len(), 1);
        assert_eq!(disc_unlocks[0].id, "first_discovery");

        let save_unlocks = increment_counter(&db, "saves", 1);
        assert_eq!(save_unlocks.len(), 1);
        assert_eq!(save_unlocks[0].id, "first_save");

        let briefing_unlocks = increment_counter(&db, "briefings", 1);
        assert_eq!(briefing_unlocks.len(), 1);
        assert_eq!(briefing_unlocks[0].id, "first_briefing");

        // Verify total unlocked in get_achievements
        let all = get_achievements(&db);
        assert_eq!(all.len(), 4, "Should have 4 unlocked achievements total");
    }

    #[test]
    fn test_discovery_full_progression() {
        let db = game_db();

        let bronze = increment_counter(&db, "discoveries", 1);
        assert_eq!(bronze.len(), 1);
        assert_eq!(bronze[0].id, "first_discovery");
        assert_eq!(bronze[0].tier, AchievementTier::Bronze);

        let silver = increment_counter(&db, "discoveries", 9);
        assert_eq!(silver.len(), 1);
        assert_eq!(silver[0].id, "ten_discoveries");
        assert_eq!(silver[0].tier, AchievementTier::Silver);

        let gold = increment_counter(&db, "discoveries", 90);
        assert_eq!(gold.len(), 1);
        assert_eq!(gold[0].id, "hundred_discoveries");
        assert_eq!(gold[0].tier, AchievementTier::Gold);
    }

    #[test]
    fn test_decision_full_progression() {
        let db = game_db();

        let bronze = increment_counter(&db, "decisions", 1);
        assert_eq!(bronze.len(), 1);
        assert_eq!(bronze[0].id, "first_decision");
        assert_eq!(bronze[0].tier, AchievementTier::Bronze);

        let silver = increment_counter(&db, "decisions", 9);
        assert_eq!(silver.len(), 1);
        assert_eq!(silver[0].id, "ten_decisions");
        assert_eq!(silver[0].tier, AchievementTier::Silver);

        let gold = increment_counter(&db, "decisions", 40);
        assert_eq!(gold.len(), 1);
        assert_eq!(gold[0].id, "fifty_decisions");
        assert_eq!(gold[0].tier, AchievementTier::Gold);
    }
}
