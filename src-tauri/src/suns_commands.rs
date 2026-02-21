//! Suns Tauri Commands -- frontend API for the Suns dashboard.

use crate::error::{FourDaError, Result};
use crate::suns::{SunAlert, SunResult, SunStatus};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rusqlite::params;

// ============================================================================
// Global Sun Registry
// ============================================================================

static SUN_REGISTRY: Lazy<Mutex<crate::suns::SunRegistry>> =
    Lazy::new(|| Mutex::new(crate::suns::SunRegistry::new()));

pub fn get_sun_registry() -> parking_lot::MutexGuard<'static, crate::suns::SunRegistry> {
    SUN_REGISTRY.lock()
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn get_sun_statuses() -> Result<Vec<SunStatus>> {
    let registry = get_sun_registry();
    Ok(registry.get_statuses())
}

#[tauri::command]
pub async fn toggle_sun(sun_id: String, enabled: bool) -> Result<()> {
    let mut registry = get_sun_registry();
    registry.set_enabled(&sun_id, enabled);
    Ok(())
}

#[tauri::command]
pub async fn get_sun_alerts() -> Result<Vec<SunAlert>> {
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;

    let mut stmt = conn
        .prepare(
            "SELECT id, sun_id, alert_type, message, acknowledged, created_at
             FROM sun_alerts
             WHERE acknowledged = 0
             ORDER BY created_at DESC
             LIMIT 50",
        )
        .map_err(FourDaError::Db)?;

    let alerts = stmt
        .query_map([], |row| {
            Ok(SunAlert {
                id: row.get(0)?,
                sun_id: row.get(1)?,
                alert_type: row.get(2)?,
                message: row.get(3)?,
                acknowledged: row.get::<_, i32>(4)? != 0,
                created_at: row.get(5)?,
            })
        })
        .map_err(FourDaError::Db)?
        .filter_map(|r| r.ok())
        .collect();

    Ok(alerts)
}

#[tauri::command]
pub async fn acknowledge_sun_alert(alert_id: i64) -> Result<()> {
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;

    conn.execute(
        "UPDATE sun_alerts SET acknowledged = 1 WHERE id = ?1",
        params![alert_id],
    )
    .map_err(FourDaError::Db)?;

    Ok(())
}

#[tauri::command]
pub async fn trigger_sun_manually(sun_id: String) -> Result<SunResult> {
    let mut registry = get_sun_registry();
    registry
        .execute_one(&sun_id)
        .ok_or_else(|| FourDaError::Internal(format!("Sun '{}' not found", sun_id)))
}
