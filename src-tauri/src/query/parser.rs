// Query parser - Natural language to structured query
use serde::{Deserialize, Serialize};

/// The intent of a query
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum QueryIntent {
    #[default]
    Find, // Find files/content matching criteria
    Summarize, // Summarize content about a topic
    Compare,   // Compare multiple items
    Timeline,  // Show chronological progression
    Count,     // Count items matching criteria
}

/// A structured query parsed from natural language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedQuery {
    /// Original natural language input
    pub original: String,
    /// Detected intent
    pub intent: QueryIntent,
    /// Extracted entities (people, places, projects, topics)
    pub entities: Vec<String>,
    /// Keywords for search
    pub keywords: Vec<String>,
    /// Time range filter
    pub time_range: Option<super::TimeRange>,
    /// Sentiment filter
    pub sentiment: Option<super::SentimentFilter>,
    /// File type filter
    pub file_types: Vec<String>,
    /// Confidence score of the parsing (0.0-1.0)
    pub confidence: f32,
}

impl Default for ParsedQuery {
    fn default() -> Self {
        Self {
            original: String::new(),
            intent: QueryIntent::Find,
            entities: Vec::new(),
            keywords: Vec::new(),
            time_range: None,
            sentiment: None,
            file_types: Vec::new(),
            confidence: 0.0,
        }
    }
}

/// Query parser (simple keyword-based, no LLM required)
/// For simple usage, prefer `parse_simple()` function directly
#[allow(dead_code)]
pub struct QueryParser;

#[allow(dead_code)]
impl QueryParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse natural language query into structured form
    pub fn parse(&self, query: &str) -> ParsedQuery {
        parse_simple(query)
    }
}

impl Default for QueryParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple keyword-based parser (no LLM required)
pub fn parse_simple(query: &str) -> ParsedQuery {
    let lower = query.to_lowercase();

    // Detect intent
    let intent = if lower.starts_with("summarize")
        || lower.starts_with("summary")
        || lower.contains("summarize")
    {
        QueryIntent::Summarize
    } else if lower.starts_with("compare") || lower.contains("vs") || lower.contains("versus") {
        QueryIntent::Compare
    } else if lower.starts_with("how many") || lower.starts_with("count") {
        QueryIntent::Count
    } else if lower.contains("timeline") || lower.contains("history of") {
        QueryIntent::Timeline
    } else {
        QueryIntent::Find
    };

    // Extract keywords (remove stopwords)
    let stopwords = [
        "show", "me", "find", "files", "about", "the", "a", "an", "in", "on", "at", "to", "for",
        "of", "with", "by", "from", "where", "what", "when", "how", "did", "do", "does", "i", "my",
        "was", "were", "is", "are", "that", "which", "who", "whom", "this", "these", "all", "any",
        "can", "could", "would", "should", "have", "has", "had", "get", "got",
    ];

    let keywords: Vec<String> = query
        .split_whitespace()
        .filter(|w| {
            let word = w.to_lowercase();
            word.len() > 2 && !stopwords.contains(&word.as_str())
        })
        .map(|s| s.to_string())
        .collect();

    // Detect time range
    let time_range = if lower.contains("last week") || lower.contains("this week") {
        Some(super::TimeRange::last_week())
    } else if lower.contains("last month") || lower.contains("this month") {
        Some(super::TimeRange::last_month())
    } else if lower.contains("yesterday") {
        Some(super::TimeRange::yesterday())
    } else if lower.contains("today") {
        Some(super::TimeRange::today())
    } else if lower.contains("last year") || lower.contains("this year") {
        Some(super::TimeRange::last_year())
    } else {
        None
    };

    // Detect sentiment
    let sentiment = if lower.contains("stress") {
        Some(super::SentimentFilter::Stressed)
    } else if lower.contains("happy") || lower.contains("excited") {
        Some(super::SentimentFilter::Happy)
    } else if lower.contains("frustrated") || lower.contains("angry") {
        Some(super::SentimentFilter::Frustrated)
    } else if lower.contains("worried") || lower.contains("anxious") {
        Some(super::SentimentFilter::Anxious)
    } else {
        None
    };

    // Detect file types
    let mut file_types = Vec::new();
    if lower.contains("pdf") {
        file_types.push("pdf".to_string());
    }
    if lower.contains("word") || lower.contains("doc") || lower.contains("docx") {
        file_types.push("docx".to_string());
    }
    if lower.contains("excel") || lower.contains("spreadsheet") || lower.contains("xlsx") {
        file_types.push("xlsx".to_string());
    }
    if lower.contains("image") || lower.contains("photo") || lower.contains("picture") {
        file_types.push("image".to_string());
    }

    ParsedQuery {
        original: query.to_string(),
        intent,
        entities: Vec::new(), // Entities require NER (Phase 2, Month 5)
        keywords,
        time_range,
        sentiment,
        file_types,
        confidence: 0.6, // Keyword-based parsing has moderate confidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_find() {
        let parsed = parse_simple("show me files about rust");
        assert_eq!(parsed.intent, QueryIntent::Find);
        assert!(parsed.keywords.contains(&"rust".to_string()));
    }

    #[test]
    fn test_parse_simple_summarize() {
        let parsed = parse_simple("summarize my notes on machine learning");
        assert_eq!(parsed.intent, QueryIntent::Summarize);
        assert!(parsed.keywords.contains(&"machine".to_string()));
        assert!(parsed.keywords.contains(&"learning".to_string()));
    }

    #[test]
    fn test_parse_simple_time_range() {
        let parsed = parse_simple("what did I work on last week");
        assert_eq!(parsed.intent, QueryIntent::Find);
        assert!(parsed.time_range.is_some());
    }

    #[test]
    fn test_parse_simple_file_type() {
        let parsed = parse_simple("find all pdf files about taxes");
        assert!(parsed.file_types.contains(&"pdf".to_string()));
    }

    #[test]
    fn test_parse_simple_sentiment() {
        let parsed = parse_simple("when was I stressed about deadlines");
        assert_eq!(
            parsed.sentiment,
            Some(super::super::SentimentFilter::Stressed)
        );
    }
}
