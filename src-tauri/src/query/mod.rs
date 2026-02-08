// Query module - Natural Language Query System
// Phase 2 of 4DA roadmap

mod executor;
mod filters;
mod parser;

// Re-export public API (some items are designed for future use)
#[allow(unused_imports)]
pub use executor::{QueryExecutor, QueryResult, QueryResultItem};
pub use filters::{SentimentFilter, TimeRange};
#[allow(unused_imports)]
pub use parser::{parse_simple, ParsedQuery, QueryIntent, QueryParser};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_intent_display() {
        assert_eq!(format!("{:?}", QueryIntent::Find), "Find");
        assert_eq!(format!("{:?}", QueryIntent::Summarize), "Summarize");
    }
}
