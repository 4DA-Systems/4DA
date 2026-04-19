// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Market Tracker Sun -- analyzes source_items for revenue-related trends (daily).

use super::SunResult;
use rusqlite::params;

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

    // Count revenue-related items in last 7 days
    let revenue_terms = [
        "freelance",
        "pricing",
        "revenue",
        "saas",
        "subscription",
        "monetize",
        "income",
    ];
    let mut total_mentions: i64 = 0;
    let mut term_counts: Vec<(String, i64)> = Vec::new();

    for term in &revenue_terms {
        let pattern = format!("%{term}%");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM source_items
                 WHERE (LOWER(title) LIKE ?1 OR LOWER(content) LIKE ?1)
                 AND created_at >= datetime('now', '-7 days')",
                params![pattern],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if count > 0 {
            term_counts.push((term.to_string(), count));
            total_mentions += count;
        }
    }

    SunResult {
        success: true,
        message: format!("{total_mentions} revenue-related mentions in last 7 days"),
        data: Some(serde_json::json!({
            "total_mentions": total_mentions,
            "term_counts": term_counts,
        })),
    }
}
