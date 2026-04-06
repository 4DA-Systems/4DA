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
    pub gap_message: Option<String>,
}

fn relative_time(iso: &str) -> String {
    let lang = crate::i18n::get_user_language();
    // Parse "YYYY-MM-DD HH:MM:SS" from SQLite datetime()
    let now = chrono::Utc::now().naive_utc();
    if let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(iso, "%Y-%m-%d %H:%M:%S") {
        let diff = now - parsed;
        let mins = diff.num_minutes();
        if mins < 1 {
            return crate::i18n::t("ui:health.justNow", &lang, &[]);
        }
        if mins < 60 {
            return crate::i18n::t(
                "ui:health.minutesAgo",
                &lang,
                &[("count", &mins.to_string())],
            );
        }
        let hours = diff.num_hours();
        if hours < 24 {
            return crate::i18n::t(
                "ui:health.hoursAgo",
                &lang,
                &[("count", &hours.to_string())],
            );
        }
        let days = diff.num_days();
        return crate::i18n::t("ui:health.daysAgo", &lang, &[("count", &days.to_string())]);
    }
    iso.to_string()
}

fn gap_message(source: &str, status: &str) -> Option<String> {
    if status == "healthy" {
        return None;
    }
    let lang = crate::i18n::get_user_language();
    let key = match source {
        "hackernews" => "ui:health.hnMissed",
        "github" => "ui:health.githubMissed",
        "reddit" => "ui:health.redditMissed",
        "arxiv" => "ui:health.arxivMissed",
        "rss" => "ui:health.rssMissed",
        "twitter" => "ui:health.twitterMissed",
        "youtube" => "ui:health.youtubeMissed",
        "lobsters" => "ui:health.lobstersMissed",
        "devto" => "ui:health.devtoMissed",
        "producthunt" => "ui:health.phMissed",
        _ => "ui:health.sourceMissed",
    };
    Some(crate::i18n::t(key, &lang, &[]))
}

#[tauri::command]
pub async fn get_source_health_status() -> Result<Vec<SourceHealthStatus>> {
    let db = get_database()?;
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
// Source Quality Analysis
// ============================================================================

/// Returns per-source relevance quality ratios.
/// Sources below 5% relevance ratio are flagged for potential replacement.
#[tauri::command]
pub async fn get_source_quality() -> Result<Vec<crate::health::SourceQualityReport>> {
    let conn = crate::state::open_db_connection()?;
    Ok(crate::health::compute_source_quality(&conn, 30))
}

/// Reset circuit breaker for a specific source, allowing it to be retried.
#[tauri::command]
pub async fn reset_source_circuit_breaker(source_type: String) -> Result<String> {
    if source_type.len() > 50 {
        return Err("Source type too long".into());
    }
    let db = get_database()?;
    let conn = db.conn.lock();
    conn.execute(
        "UPDATE source_health SET consecutive_failures = 0, status = 'healthy' WHERE source_type = ?1",
        rusqlite::params![source_type],
    )
    .map_err(FourDaError::Db)?;
    tracing::info!(target: "4da::sources", source = %source_type, "Circuit breaker manually reset");
    let lang = crate::i18n::get_user_language();
    Ok(crate::i18n::t(
        "ui:health.circuitReset",
        &lang,
        &[("source", &source_type)],
    ))
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
        // gap_message now returns Option<String> via i18n; in test env (no locale
        // files loaded) the t() function falls back to returning the key itself.
        // We verify that unhealthy sources always return Some(non-empty).
        let cases = [
            ("hackernews", "degraded"),
            ("github", "down"),
            ("reddit", "error"),
            ("arxiv", "failed"),
            ("rss", "timeout"),
            ("twitter", "rate_limited"),
            ("youtube", "err"),
            ("lobsters", "err"),
            ("devto", "err"),
            ("producthunt", "err"),
            ("unknown", "err"),
        ];
        for (source, status) in &cases {
            let msg = gap_message(source, status);
            assert!(
                msg.is_some(),
                "Expected Some for source={source}, status={status}"
            );
            assert!(
                !msg.as_ref().unwrap().is_empty(),
                "Expected non-empty message for source={source}"
            );
        }
    }

    #[test]
    fn test_relative_time_just_now() {
        // Use a timestamp very close to now
        let now = chrono::Utc::now().naive_utc();
        let iso = now.format("%Y-%m-%d %H:%M:%S").to_string();
        let result = relative_time(&iso);
        // The result is locale-dependent, so compare against i18n outputs
        let lang = crate::i18n::get_user_language();
        let just_now = crate::i18n::t("ui:health.justNow", &lang, &[]);
        let one_min_ago = crate::i18n::t("ui:health.minutesAgo", &lang, &[("count", "1")]);
        // Timestamp is essentially "now", so we should get either "just now" or "1m ago"
        // (in whatever locale is active). Must NOT be the raw ISO fallback.
        assert!(
            result == just_now || result == one_min_ago,
            "Expected '{}' or '{}', got: {}",
            just_now,
            one_min_ago,
            result
        );
    }

    #[test]
    fn test_relative_time_invalid_format() {
        let result = relative_time("not-a-date");
        assert_eq!(result, "not-a-date");
    }
}
