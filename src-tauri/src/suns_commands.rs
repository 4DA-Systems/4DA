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

#[cfg(test)]
mod tests {
    use crate::suns::{ModuleHealth, StreetHealthScore, SunAlert, SunResult, SunStatus};

    // ---- SunResult construction & serialization ----

    #[test]
    fn test_sun_result_success() {
        let result = SunResult {
            success: true,
            message: "All checks passed".to_string(),
            data: None,
        };
        assert!(result.success);
        let json = serde_json::to_value(&result).expect("serialize");
        assert_eq!(json["success"], true);
        assert_eq!(json["message"], "All checks passed");
        assert!(json["data"].is_null());
    }

    #[test]
    fn test_sun_result_failure_with_data() {
        let data = serde_json::json!({"error_code": 42, "detail": "timeout"});
        let result = SunResult {
            success: false,
            message: "Check failed".to_string(),
            data: Some(data.clone()),
        };
        assert!(!result.success);
        let json = serde_json::to_value(&result).expect("serialize");
        assert_eq!(json["data"]["error_code"], 42);
    }

    // ---- SunStatus construction & serialization ----

    #[test]
    fn test_sun_status_construction() {
        let status = SunStatus {
            id: "hardware_monitor".to_string(),
            name: "Hardware Monitor".to_string(),
            module_id: "S".to_string(),
            enabled: true,
            interval_secs: 86400,
            last_run: Some("2025-12-01 10:00:00".to_string()),
            next_run_in_secs: Some(3600),
            last_result: Some("OK".to_string()),
            run_count: 10,
        };
        let json = serde_json::to_value(&status).expect("serialize");
        assert_eq!(json["id"], "hardware_monitor");
        assert_eq!(json["module_id"], "S");
        assert_eq!(json["enabled"], true);
        assert_eq!(json["interval_secs"], 86400);
        assert_eq!(json["run_count"], 10);
    }

    #[test]
    fn test_sun_status_with_none_fields() {
        let status = SunStatus {
            id: "test_sun".to_string(),
            name: "Test Sun".to_string(),
            module_id: "T".to_string(),
            enabled: false,
            interval_secs: 3600,
            last_run: None,
            next_run_in_secs: None,
            last_result: None,
            run_count: 0,
        };
        let json = serde_json::to_value(&status).expect("serialize");
        assert!(json["last_run"].is_null());
        assert!(json["next_run_in_secs"].is_null());
        assert!(json["last_result"].is_null());
    }

    // ---- SunAlert construction & serialization ----

    #[test]
    fn test_sun_alert_serialization() {
        let alert = SunAlert {
            id: 1,
            sun_id: "price_tracker".to_string(),
            alert_type: "warning".to_string(),
            message: "Price change detected".to_string(),
            acknowledged: false,
            created_at: "2025-12-01 12:00:00".to_string(),
        };
        let json = serde_json::to_value(&alert).expect("serialize");
        assert_eq!(json["id"], 1);
        assert_eq!(json["sun_id"], "price_tracker");
        assert_eq!(json["acknowledged"], false);
    }

    #[test]
    fn test_sun_alert_deserialization() {
        let json_str = r#"{
            "id": 5,
            "sun_id": "uptime_monitor",
            "alert_type": "critical",
            "message": "Service down",
            "acknowledged": true,
            "created_at": "2025-12-01 14:00:00"
        }"#;
        let alert: SunAlert = serde_json::from_str(json_str).expect("deserialize");
        assert_eq!(alert.id, 5);
        assert_eq!(alert.sun_id, "uptime_monitor");
        assert!(alert.acknowledged);
    }

    // ---- ModuleHealth construction ----

    #[test]
    fn test_module_health_construction() {
        let health = ModuleHealth {
            module_id: "S".to_string(),
            module_name: "Sovereign Setup".to_string(),
            score: 0.75,
            sun_count: 2,
            success_rate: 0.9,
            lessons_completed: 3,
            total_lessons: 5,
            last_activity: Some("2025-12-01 10:00:00".to_string()),
        };
        assert!(health.score >= 0.0 && health.score <= 1.0);
        assert!(health.success_rate >= 0.0 && health.success_rate <= 1.0);
        assert!(health.lessons_completed <= health.total_lessons);
    }

    // ---- StreetHealthScore construction ----

    #[test]
    fn test_street_health_score_serialization() {
        let score = StreetHealthScore {
            overall: 0.65,
            module_scores: vec![
                ModuleHealth {
                    module_id: "S".to_string(),
                    module_name: "Sovereign Setup".to_string(),
                    score: 0.8,
                    sun_count: 2,
                    success_rate: 1.0,
                    lessons_completed: 4,
                    total_lessons: 5,
                    last_activity: Some("2025-12-01".to_string()),
                },
                ModuleHealth {
                    module_id: "T".to_string(),
                    module_name: "Technical Moats".to_string(),
                    score: 0.5,
                    sun_count: 1,
                    success_rate: 0.5,
                    lessons_completed: 1,
                    total_lessons: 4,
                    last_activity: None,
                },
            ],
            trend: "improving".to_string(),
            top_action: "Continue Sovereign Setup".to_string(),
        };
        let json = serde_json::to_value(&score).expect("serialize");
        let overall = json["overall"].as_f64().expect("f64");
        assert!(
            (overall - 0.65).abs() < 1e-5,
            "overall should be ~0.65, got {}",
            overall
        );
        assert_eq!(json["trend"], "improving");
        assert_eq!(json["module_scores"].as_array().expect("array").len(), 2);
    }

    // ---- get_module_lesson_count ----

    #[test]
    fn test_get_module_lesson_count_unknown_module() {
        let count = super::get_module_lesson_count("UNKNOWN");
        assert_eq!(count, 0);
    }
}
