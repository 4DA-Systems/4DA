use crate::error::Result;
use crate::get_database;
use tauri::AppHandle;

#[tauri::command]
pub fn get_game_state() -> Result<serde_json::Value> {
    let db = get_database()?;
    let state = crate::game_engine::get_game_state(db);
    Ok(serde_json::to_value(state).unwrap_or_default())
}

#[tauri::command]
pub fn get_achievements() -> Result<serde_json::Value> {
    let db = get_database()?;
    let achievements = crate::game_engine::get_achievements(db);
    Ok(serde_json::to_value(achievements).unwrap_or_default())
}

/// Check daily streak on app startup. Returns any newly unlocked streak achievements.
#[tauri::command]
pub fn check_daily_streak(app: AppHandle) -> Result<serde_json::Value> {
    let db = get_database()?;
    let unlocked = crate::game_engine::check_daily_streak(db);
    for a in &unlocked {
        crate::events::emit_achievement_unlocked(&app, a);
    }
    Ok(serde_json::to_value(&unlocked).unwrap_or_default())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::game_achievements::AchievementTier;
    use crate::game_engine::{AchievementState, CounterState, GameState};

    #[test]
    fn test_game_state_serialization() {
        let state = GameState {
            counters: vec![CounterState {
                counter_type: "scans".to_string(),
                value: 5,
            }],
            achievements: vec![AchievementState {
                id: "first_scan".to_string(),
                name: "First Light".to_string(),
                description: "Run your first content scan".to_string(),
                icon: "telescope".to_string(),
                counter_type: "scans".to_string(),
                threshold: 1,
                tier: AchievementTier::Bronze,
                current: 5,
                unlocked: true,
                unlocked_at: Some("2025-01-15 10:00:00".to_string()),
            }],
            streak: 3,
            last_active: Some("2025-06-01".to_string()),
        };
        let json = serde_json::to_value(&state).expect("serialize");
        assert_eq!(json["streak"], 3);
        assert_eq!(json["counters"][0]["value"], 5);
        assert_eq!(json["achievements"][0]["unlocked"], true);
    }

    #[test]
    fn test_game_state_empty() {
        let state = GameState {
            counters: vec![],
            achievements: vec![],
            streak: 0,
            last_active: None,
        };
        let json = serde_json::to_value(&state).expect("serialize");
        assert_eq!(json["streak"], 0);
        assert!(json["last_active"].is_null());
        assert_eq!(json["counters"].as_array().expect("array").len(), 0);
    }
}
