// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Query filters - Time, entity, sentiment filters
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

/// Time range filter for queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// Human-readable relative description (e.g., "last_week")
    pub relative: Option<String>,
}

impl TimeRange {
    /// Create a time range for today
    pub fn today() -> Self {
        let now = Utc::now();
        let start = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap_or_else(|| now.naive_utc());
        Self {
            start: start.and_utc(),
            end: now,
            relative: Some("today".to_string()),
        }
    }

    /// Create a time range for yesterday
    pub fn yesterday() -> Self {
        let now = Utc::now();
        let yesterday = now - Duration::days(1);
        let start = yesterday
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap_or_else(|| yesterday.naive_utc());
        let end = yesterday
            .date_naive()
            .and_hms_opt(23, 59, 59)
            .unwrap_or_else(|| yesterday.naive_utc());
        Self {
            start: start.and_utc(),
            end: end.and_utc(),
            relative: Some("yesterday".to_string()),
        }
    }

    /// Create a time range for the last 7 days
    pub fn last_week() -> Self {
        let now = Utc::now();
        let start = now - Duration::days(7);
        Self {
            start,
            end: now,
            relative: Some("last_week".to_string()),
        }
    }

    /// Create a time range for the last 30 days
    pub fn last_month() -> Self {
        let now = Utc::now();
        let start = now - Duration::days(30);
        Self {
            start,
            end: now,
            relative: Some("last_month".to_string()),
        }
    }

    /// Create a time range for the last 365 days
    pub fn last_year() -> Self {
        let now = Utc::now();
        let start = now - Duration::days(365);
        Self {
            start,
            end: now,
            relative: Some("last_year".to_string()),
        }
    }

    /// Create a custom time range
    pub fn custom(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self {
            start,
            end,
            relative: None,
        }
    }

    /// Check if a datetime falls within this range
    pub fn contains(&self, dt: &DateTime<Utc>) -> bool {
        dt >= &self.start && dt <= &self.end
    }

    /// Check if a naive datetime (assumed UTC) falls within this range
    pub fn contains_naive(&self, dt: &NaiveDateTime) -> bool {
        let utc_dt = dt.and_utc();
        self.contains(&utc_dt)
    }

    /// Get SQL WHERE clause fragment for this time range
    pub fn to_sql_clause(&self, column: &str) -> String {
        format!(
            "{} >= '{}' AND {} <= '{}'",
            column,
            self.start.format("%Y-%m-%d %H:%M:%S"),
            column,
            self.end.format("%Y-%m-%d %H:%M:%S")
        )
    }
}

/// Sentiment filter for queries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SentimentFilter {
    Positive,
    Negative,
    Neutral,
    Stressed,
    Excited,
    Anxious,
    Happy,
    Frustrated,
}

impl SentimentFilter {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "positive" | "good" | "great" => Self::Positive,
            "negative" | "bad" => Self::Negative,
            "neutral" => Self::Neutral,
            "stressed" | "stress" => Self::Stressed,
            "excited" | "excitement" => Self::Excited,
            "anxious" | "anxiety" | "worried" => Self::Anxious,
            "happy" | "happiness" | "joy" => Self::Happy,
            "frustrated" | "frustration" | "angry" => Self::Frustrated,
            _ => Self::Neutral,
        }
    }

    /// Get sentiment keywords for text matching
    pub fn to_keywords(&self) -> Vec<&'static str> {
        match self {
            Self::Positive => vec![
                "great",
                "excellent",
                "good",
                "happy",
                "success",
                "accomplished",
                "progress",
            ],
            Self::Negative => vec![
                "bad", "terrible", "failed", "problem", "issue", "error", "broken",
            ],
            Self::Neutral => vec![],
            Self::Stressed => vec![
                "stressed",
                "pressure",
                "deadline",
                "urgent",
                "overwhelmed",
                "behind",
            ],
            Self::Excited => vec!["excited", "amazing", "awesome", "can't wait", "thrilled"],
            Self::Anxious => vec!["worried", "anxious", "concerned", "nervous", "uncertain"],
            Self::Happy => vec!["happy", "joy", "delighted", "pleased", "satisfied"],
            Self::Frustrated => vec!["frustrated", "annoying", "stuck", "blocked", "impossible"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_range_today() {
        let tr = TimeRange::today();
        // The start should be today at 00:00:00
        assert_eq!(tr.relative, Some("today".to_string()));
        // Check that an hour ago today is contained
        let hour_ago = Utc::now() - Duration::hours(1);
        if hour_ago >= tr.start {
            assert!(tr.contains(&hour_ago));
        }
    }

    #[test]
    fn test_time_range_last_week() {
        let tr = TimeRange::last_week();
        // A time 3 days ago should definitely be within last week
        let three_days_ago = Utc::now() - Duration::days(3);
        assert!(tr.contains(&three_days_ago));
    }

    #[test]
    fn test_time_range_sql_clause() {
        let tr = TimeRange::last_week();
        let sql = tr.to_sql_clause("created_at");
        assert!(sql.contains("created_at >="));
        assert!(sql.contains("AND created_at <="));
    }

    #[test]
    fn test_sentiment_from_str() {
        assert_eq!(
            SentimentFilter::from_str("stressed"),
            SentimentFilter::Stressed
        );
        assert_eq!(SentimentFilter::from_str("HAPPY"), SentimentFilter::Happy);
        assert_eq!(
            SentimentFilter::from_str("unknown"),
            SentimentFilter::Neutral
        );
    }

    #[test]
    fn test_sentiment_keywords() {
        let stressed = SentimentFilter::Stressed;
        let keywords = stressed.to_keywords();
        assert!(keywords.contains(&"deadline"));
        assert!(keywords.contains(&"pressure"));
    }
}
