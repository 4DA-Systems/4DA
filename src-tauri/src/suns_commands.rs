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

#[tauri::command]
pub async fn get_street_health() -> Result<crate::suns::StreetHealthScore> {
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;

    let module_defs = [
        ("S", "Sovereign Setup"),
        ("T", "Technical Moats"),
        ("R", "Revenue Engines"),
        ("E1", "Execution Playbook"),
        ("E2", "Evolving Edge"),
        ("T2", "Tactical Automation"),
        ("S2", "Stacking Streams"),
    ];

    // Get sun counts per module from registry
    let sun_counts = {
        let registry = get_sun_registry();
        registry.get_module_sun_counts()
    };

    let mut module_scores: Vec<crate::suns::ModuleHealth> = Vec::new();

    for (module_id, module_name) in &module_defs {
        // Sun success rate in last 7 days
        let total_runs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sun_runs WHERE module_id = ?1 AND created_at >= datetime('now', '-7 days')",
                params![module_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let successful_runs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sun_runs WHERE module_id = ?1 AND success = 1 AND created_at >= datetime('now', '-7 days')",
                params![module_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let success_rate = if total_runs > 0 {
            successful_runs as f32 / total_runs as f32
        } else {
            0.0
        };

        // Playbook progress for this module
        let lessons_completed: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM playbook_progress WHERE module_id = ?1",
                params![module_id],
                |row| row.get::<_, i64>(0).map(|v| v as usize),
            )
            .unwrap_or(0);

        // Total lessons from content files
        let total_lessons = get_module_lesson_count(module_id);

        // Last activity timestamp
        let last_activity: Option<String> = conn
            .query_row(
                "SELECT MAX(created_at) FROM sun_runs WHERE module_id = ?1",
                params![module_id],
                |row| row.get(0),
            )
            .unwrap_or(None);

        // Composite score: 40% sun success rate + 40% playbook progress + 20% recency
        let progress_score = if total_lessons > 0 {
            (lessons_completed as f32 / total_lessons as f32).min(1.0)
        } else {
            0.0
        };

        let recency_score = last_activity
            .as_ref()
            .and_then(|ts| chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S").ok())
            .map(|dt| {
                let days_ago = (chrono::Utc::now().naive_utc() - dt).num_days();
                if days_ago <= 1 {
                    1.0f32
                } else if days_ago <= 7 {
                    0.7
                } else if days_ago <= 30 {
                    0.4
                } else {
                    0.1
                }
            })
            .unwrap_or(0.0);

        let score = (success_rate * 0.4 + progress_score * 0.4 + recency_score * 0.2).min(1.0);

        module_scores.push(crate::suns::ModuleHealth {
            module_id: module_id.to_string(),
            module_name: module_name.to_string(),
            score,
            sun_count: *sun_counts.get(*module_id).unwrap_or(&0),
            success_rate,
            lessons_completed,
            total_lessons,
            last_activity,
        });
    }

    // Overall score: weighted average of module scores
    let overall = if module_scores.is_empty() {
        0.0
    } else {
        module_scores.iter().map(|m| m.score).sum::<f32>() / module_scores.len() as f32
    };

    // Trend: compare last 7 days vs previous 7 days
    let recent_successes: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sun_runs WHERE success = 1 AND created_at >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let older_successes: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sun_runs WHERE success = 1 AND created_at >= datetime('now', '-14 days') AND created_at < datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let trend = if recent_successes > older_successes + 2 {
        "improving".to_string()
    } else if older_successes > recent_successes + 2 {
        "declining".to_string()
    } else {
        "stable".to_string()
    };

    // Top action: find the weakest module and suggest what to do
    let top_action = module_scores
        .iter()
        .min_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|weakest| {
            if weakest.lessons_completed == 0 {
                format!("Start {} -- begin your first lesson", weakest.module_name)
            } else if weakest.success_rate < 0.5 {
                format!("Check {} suns -- some are failing", weakest.module_name)
            } else {
                format!(
                    "Continue {} -- complete more lessons to strengthen this module",
                    weakest.module_name
                )
            }
        })
        .unwrap_or_else(|| "Start the STREETS playbook to build your health score".to_string());

    Ok(crate::suns::StreetHealthScore {
        overall,
        module_scores,
        trend,
        top_action,
    })
}

/// Get approximate lesson count for a module (from content files if available, else default).
fn get_module_lesson_count(module_id: &str) -> usize {
    if let Some(filename) = crate::playbook_commands::module_id_to_filename(module_id) {
        let content_dir = crate::playbook_commands::get_content_dir();
        let path = content_dir.join(filename);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                return crate::playbook_commands::parse_lessons(&content).len();
            }
        }
    }
    0
}
