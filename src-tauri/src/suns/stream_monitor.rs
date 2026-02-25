//! Stream Monitor Sun -- measures revenue stream diversity (6h).

use super::SunResult;
use rusqlite::params;

pub fn execute() -> SunResult {
    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(e) => {
            return SunResult {
                success: false,
                message: format!("DB unavailable: {}", e),
                data: None,
            }
        }
    };

    // Aggregate data from revenue-related sun runs
    let mut module_health: Vec<serde_json::Value> = Vec::new();
    let mut active_modules = 0;

    // Check each STREETS module for recent activity
    let modules = ["S", "T", "R", "E1", "E2", "T2", "S2"];
    for module_id in &modules {
        let recent_runs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sun_runs
                 WHERE module_id = ?1 AND success = 1
                 AND created_at >= datetime('now', '-7 days')",
                params![module_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let lesson_progress: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM playbook_progress WHERE module_id = ?1",
                params![module_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let is_active = recent_runs > 0 || lesson_progress > 0;
        if is_active {
            active_modules += 1;
        }

        module_health.push(serde_json::json!({
            "module": module_id,
            "recent_sun_runs": recent_runs,
            "lessons_completed": lesson_progress,
            "active": is_active,
        }));
    }

    // Revenue stream diversity: how many distinct revenue engine types appear in profile
    let stream_count: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT value) FROM sovereign_profile
             WHERE category = 'revenue' OR key LIKE '%engine%'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let diversity_score = (active_modules as f32 / modules.len() as f32 * 0.6)
        + (stream_count.min(5) as f32 / 5.0 * 0.4);

    let diversity_label = if diversity_score >= 0.7 {
        "diversified"
    } else if diversity_score >= 0.4 {
        "growing"
    } else {
        "concentrated"
    };

    SunResult {
        success: true,
        message: format!(
            "Stream diversity: {} ({}/{} modules active, {} revenue streams)",
            diversity_label,
            active_modules,
            modules.len(),
            stream_count
        ),
        data: Some(serde_json::json!({
            "diversity_score": diversity_score,
            "diversity_label": diversity_label,
            "active_modules": active_modules,
            "total_modules": modules.len(),
            "stream_count": stream_count,
            "module_health": module_health,
        })),
    }
}
