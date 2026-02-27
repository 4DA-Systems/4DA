use crate::error::Result;
use crate::get_database;

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
