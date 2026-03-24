//! Tech Moat Scanner Sun -- analyzes detected tech stack for competitive moats (daily).

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

    // Count distinct technologies detected by ACE
    let tech_count: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT name) FROM detected_tech",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Look for uncommon/niche technologies that could be moat indicators.
    // Technologies with fewer mentions in source_items are potentially more unique.
    let mut stmt = conn
        .prepare(
            "SELECT dt.name, COUNT(si.id) as mention_count
             FROM detected_tech dt
             LEFT JOIN source_items si
               ON LOWER(si.title || ' ' || COALESCE(si.content, '')) LIKE '%' || LOWER(dt.name) || '%'
               AND si.created_at >= datetime('now', '-30 days')
             GROUP BY dt.name
             ORDER BY mention_count ASC
             LIMIT 10",
        )
        .ok();

    let mut unique_techs: Vec<(String, i64)> = Vec::new();
    let mut moat_candidates = 0;

    if let Some(ref mut s) = stmt {
        if let Ok(rows) = s.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        }) {
            for row in rows.flatten() {
                if row.1 <= 5 {
                    moat_candidates += 1;
                }
                unique_techs.push(row);
            }
        }
    }

    SunResult {
        success: true,
        message: format!(
            "{tech_count} technologies detected, {moat_candidates} potential moat candidates"
        ),
        data: Some(serde_json::json!({
            "tech_count": tech_count,
            "moat_candidates": moat_candidates,
            "unique_techs": unique_techs,
        })),
    }
}
