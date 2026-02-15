//! Void Engine Tauri commands for heartbeat signal visualization.

use crate::error::Result;
use crate::void_engine;
use crate::{get_database, get_monitoring_state};

// ============================================================================
// Void Engine Commands
// ============================================================================

/// Get the current void signal state (for initial mount)
#[tauri::command]
pub fn get_void_signal() -> Result<void_engine::VoidSignal> {
    let db = get_database()?;
    let monitoring = get_monitoring_state();
    Ok(void_engine::compute_signal(db, monitoring))
}
