//! Source health commands — exposes circuit breaker state + intelligence gap messages.

use crate::error::{FourDaError, Result};
use crate::get_database;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SourceHealthStatus {
    pub source_type: String,
    pub status: String,
    pub last_success_relative: Option<String>,
    pub items_fetched: i64,
    pub gap_message: Option<&'static str>,
}

fn relative_time(iso: &str) -> String {
    // Parse "YYYY-MM-DD HH:MM:SS" from SQLite datetime()
    let now = chrono::Utc::now().naive_utc();
    if let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(iso, "%Y-%m-%d %H:%M:%S") {
        let diff = now - parsed;
        let mins = diff.num_minutes();
        if mins < 1 {
            return "just now".to_string();
        }
        if mins < 60 {
            return format!("{}m ago", mins);
        }
        let hours = diff.num_hours();
        if hours < 24 {
            return format!("{}h ago", hours);
        }
        let days = diff.num_days();
        return format!("{}d ago", days);
    }
    iso.to_string()
}

fn gap_message(source: &str, status: &str) -> Option<&'static str> {
    if status == "healthy" {
        return None;
    }
    match source {
        "hackernews" => Some("HN trends may be missed"),
        "github" => Some("Trending repos not tracked"),
        "reddit" => Some("Reddit discussions unavailable"),
        "arxiv" => Some("Research papers not updating"),
        "rss" => Some("RSS feeds offline"),
        "twitter" => Some("X/Twitter feed unavailable"),
        "youtube" => Some("YouTube content not tracked"),
        "lobsters" => Some("Lobsters discussions offline"),
        "devto" => Some("Dev.to articles not updating"),
        "producthunt" => Some("Product Hunt launches missed"),
        _ => Some("Source data unavailable"),
    }
}

#[tauri::command]
pub async fn get_source_health_status() -> Result<Vec<SourceHealthStatus>> {
    let db = get_database().map_err(FourDaError::Internal)?;
    let records = db.get_source_health().map_err(|e| FourDaError::Db(e))?;

    let statuses = records
        .into_iter()
        .map(|r| SourceHealthStatus {
            gap_message: gap_message(&r.source_type, &r.status),
            last_success_relative: r.last_success.as_deref().map(relative_time),
            source_type: r.source_type,
            status: r.status,
            items_fetched: r.items_fetched,
        })
        .collect();

    Ok(statuses)
}
