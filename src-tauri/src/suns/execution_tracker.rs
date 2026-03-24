//! Execution Tracker Sun -- measures shipping velocity via playbook progress (12h).

use super::SunResult;

pub fn execute() -> SunResult {
    let conn = match crate::open_db_connection() {
        Ok(c) => c,
        Err(e) => {
            return SunResult {
                success: false,
                message: format!("DB unavailable: {e}"),
                data: None,
            }
        }
    };

    // Lessons completed in last 7 days
    let recent_lessons: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM playbook_progress
             WHERE completed_at >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Total lessons completed
    let total_lessons: i64 = conn
        .query_row("SELECT COUNT(*) FROM playbook_progress", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    // Command executions in last 7 days (STREETS commands)
    let recent_commands: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM command_execution_log
             WHERE success = 1 AND executed_at >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Coach sessions in last 7 days
    let recent_sessions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM coach_sessions
             WHERE updated_at >= datetime('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let velocity_score = (recent_lessons * 3 + recent_commands + recent_sessions) as f32 / 10.0;
    let velocity_label = if velocity_score >= 3.0 {
        "high"
    } else if velocity_score >= 1.0 {
        "moderate"
    } else {
        "low"
    };

    SunResult {
        success: true,
        message: format!(
            "Execution velocity: {velocity_label} ({recent_lessons} lessons, {recent_commands} commands this week)"
        ),
        data: Some(serde_json::json!({
            "recent_lessons": recent_lessons,
            "total_lessons": total_lessons,
            "recent_commands": recent_commands,
            "recent_sessions": recent_sessions,
            "velocity_score": velocity_score,
            "velocity_label": velocity_label,
        })),
    }
}
