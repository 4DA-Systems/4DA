//! Edge Detector Sun -- detects emerging trends in user's tech stack (daily).

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

    // Get user's tech stack from detected_tech
    let mut tech_stmt = conn
        .prepare("SELECT DISTINCT name FROM detected_tech LIMIT 20")
        .ok();

    let user_techs: Vec<String> = tech_stmt
        .as_mut()
        .and_then(|s| {
            s.query_map([], |row| row.get::<_, String>(0))
                .ok()
                .map(|rows| rows.flatten().collect())
        })
        .unwrap_or_default();

    if user_techs.is_empty() {
        return SunResult {
            success: true,
            message: "No tech stack detected yet -- run ACE scan first".into(),
            data: Some(serde_json::json!({ "tech_count": 0, "trends": [] })),
        };
    }

    // For each tech, compare 7-day vs 30-day mention frequency
    let mut trends: Vec<serde_json::Value> = Vec::new();
    let mut rising_count = 0;

    for tech in &user_techs {
        let pattern = format!("%{}%", tech.to_lowercase());

        let recent: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items
                 WHERE (LOWER(title) LIKE ?1 OR LOWER(content) LIKE ?1)
                 AND created_at >= datetime('now', '-7 days')",
                params![pattern],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let older: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items
                 WHERE (LOWER(title) LIKE ?1 OR LOWER(content) LIKE ?1)
                 AND created_at >= datetime('now', '-30 days')
                 AND created_at < datetime('now', '-7 days')",
                params![pattern],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Normalize older to 7-day rate (older covers 23 days)
        let older_weekly = if older > 0 {
            older as f64 * 7.0 / 23.0
        } else {
            0.0
        };
        let trend = if older_weekly > 0.0 {
            ((recent as f64 - older_weekly) / older_weekly * 100.0).round()
        } else if recent > 0 {
            100.0
        } else {
            0.0
        };

        if trend > 20.0 {
            rising_count += 1;
            trends.push(serde_json::json!({
                "tech": tech,
                "recent_7d": recent,
                "baseline_weekly": older_weekly.round(),
                "trend_pct": trend,
            }));
        }
    }

    // Sort by trend magnitude
    trends.sort_by(|a, b| {
        b.get("trend_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            .partial_cmp(&a.get("trend_pct").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    SunResult {
        success: true,
        message: format!(
            "{} rising trends detected across {} tracked technologies",
            rising_count,
            user_techs.len()
        ),
        data: Some(serde_json::json!({
            "tech_count": user_techs.len(),
            "rising_count": rising_count,
            "trends": trends,
        })),
    }
}
