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
    let records = db.get_source_health().map_err(FourDaError::Db)?;

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_health_status_construction() {
        let status = SourceHealthStatus {
            source_type: "hackernews".to_string(),
            status: "healthy".to_string(),
            last_success_relative: Some("5m ago".to_string()),
            items_fetched: 42,
            gap_message: None,
        };
        assert_eq!(status.source_type, "hackernews");
        assert_eq!(status.items_fetched, 42);
        assert!(status.gap_message.is_none());
    }

    #[test]
    fn test_gap_message_healthy_returns_none() {
        assert!(gap_message("hackernews", "healthy").is_none());
        assert!(gap_message("github", "healthy").is_none());
        assert!(gap_message("unknown_source", "healthy").is_none());
    }

    #[test]
    fn test_gap_message_unhealthy_returns_message() {
        assert_eq!(
            gap_message("hackernews", "degraded"),
            Some("HN trends may be missed")
        );
        assert_eq!(
            gap_message("github", "down"),
            Some("Trending repos not tracked")
        );
        assert_eq!(
            gap_message("reddit", "error"),
            Some("Reddit discussions unavailable")
        );
        assert_eq!(
            gap_message("arxiv", "failed"),
            Some("Research papers not updating")
        );
        assert_eq!(gap_message("rss", "timeout"), Some("RSS feeds offline"));
        assert_eq!(
            gap_message("twitter", "rate_limited"),
            Some("X/Twitter feed unavailable")
        );
        assert_eq!(
            gap_message("youtube", "err"),
            Some("YouTube content not tracked")
        );
        assert_eq!(
            gap_message("lobsters", "err"),
            Some("Lobsters discussions offline")
        );
        assert_eq!(
            gap_message("devto", "err"),
            Some("Dev.to articles not updating")
        );
        assert_eq!(
            gap_message("producthunt", "err"),
            Some("Product Hunt launches missed")
        );
        assert_eq!(
            gap_message("unknown", "err"),
            Some("Source data unavailable")
        );
    }

    #[test]
    fn test_relative_time_just_now() {
        // Use a timestamp very close to now
        let now = chrono::Utc::now().naive_utc();
        let iso = now.format("%Y-%m-%d %H:%M:%S").to_string();
        let result = relative_time(&iso);
        assert!(
            result == "just now" || result.ends_with("m ago"),
            "Expected 'just now' or 'Nm ago', got: {}",
            result
        );
    }

    #[test]
    fn test_relative_time_invalid_format() {
        let result = relative_time("not-a-date");
        assert_eq!(result, "not-a-date");
    }
}
