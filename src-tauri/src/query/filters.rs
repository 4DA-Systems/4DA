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
        let start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
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
        let start = yesterday.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let end = yesterday.date_naive().and_hms_opt(23, 59, 59).unwrap();
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
    #[allow(dead_code)]
    pub fn custom(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self {
            start,
            end,
            relative: None,
        }
    }

    /// Check if a datetime falls within this range
    #[allow(dead_code)]
    pub fn contains(&self, dt: &DateTime<Utc>) -> bool {
        dt >= &self.start && dt <= &self.end
    }

    /// Check if a naive datetime (assumed UTC) falls within this range
    #[allow(dead_code)]
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

/// Entity filter for queries (people, places, projects)
/// Used when NER (Named Entity Recognition) is implemented
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityFilter {
    /// Entity name or pattern
    pub name: String,
    /// Entity type (optional)
    pub entity_type: Option<EntityType>,
    /// Whether to match exactly or fuzzy
    pub exact_match: bool,
}

/// Entity types for filtering (used with NER)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EntityType {
    Person,
    Organization,
    Project,
    Location,
    Topic,
    Unknown,
}

#[allow(dead_code)]
impl EntityFilter {
    pub fn person(name: &str) -> Self {
        Self {
            name: name.to_string(),
            entity_type: Some(EntityType::Person),
            exact_match: false,
        }
    }

    pub fn project(name: &str) -> Self {
        Self {
            name: name.to_string(),
            entity_type: Some(EntityType::Project),
            exact_match: false,
        }
    }

    pub fn topic(name: &str) -> Self {
        Self {
            name: name.to_string(),
            entity_type: Some(EntityType::Topic),
            exact_match: false,
        }
    }

    /// Get SQL LIKE pattern for this entity
    pub fn to_sql_pattern(&self) -> String {
        if self.exact_match {
            self.name.clone()
        } else {
            format!("%{}%", self.name)
        }
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

#[allow(dead_code)]
impl SentimentFilter {
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

/// Combined filter set for a query (builder pattern)
/// Designed for future advanced query building
#[allow(dead_code)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryFilters {
    pub time_range: Option<TimeRange>,
    pub entities: Vec<EntityFilter>,
    pub sentiment: Option<SentimentFilter>,
    pub file_types: Vec<String>,
    pub min_confidence: Option<f32>,
}

#[allow(dead_code)]
impl QueryFilters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_time_range(mut self, time_range: TimeRange) -> Self {
        self.time_range = Some(time_range);
        self
    }

    pub fn with_entity(mut self, entity: EntityFilter) -> Self {
        self.entities.push(entity);
        self
    }

    pub fn with_sentiment(mut self, sentiment: SentimentFilter) -> Self {
        self.sentiment = Some(sentiment);
        self
    }

    pub fn with_file_type(mut self, file_type: &str) -> Self {
        self.file_types.push(file_type.to_string());
        self
    }

    /// Check if any filters are set
    pub fn is_empty(&self) -> bool {
        self.time_range.is_none()
            && self.entities.is_empty()
            && self.sentiment.is_none()
            && self.file_types.is_empty()
            && self.min_confidence.is_none()
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

    #[test]
    fn test_entity_filter() {
        let filter = EntityFilter::person("John");
        assert_eq!(filter.entity_type, Some(EntityType::Person));
        assert_eq!(filter.to_sql_pattern(), "%John%");
    }
}
