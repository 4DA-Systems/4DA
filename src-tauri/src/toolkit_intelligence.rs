//! Toolkit Intelligence — Backend commands for 4DA-connected intelligence tools
//!
//! Provides two Tauri commands:
//! - `toolkit_test_feed` — Test any RSS/Atom URL and see what 4DA extracts
//! - `toolkit_score_sandbox` — Score a title against your interest profile
//!
//! Export pack generation lives in `toolkit_export.rs`.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedTestResult {
    pub feed_title: Option<String>,
    pub format: String,
    pub item_count: usize,
    pub items: Vec<FeedTestItem>,
    pub fetch_duration_ms: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedTestItem {
    pub title: String,
    pub url: String,
    pub published_at: Option<String>,
    pub content_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxScoreResult {
    pub score: f32,
    pub relevant: bool,
    pub breakdown: SandboxBreakdown,
    pub matched_interests: Vec<String>,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxBreakdown {
    pub keyword_score: f32,
    pub interest_score: f32,
    pub ace_boost: f32,
    pub affinity_mult: f32,
    pub domain_relevance: f32,
    pub content_quality: f32,
}

// ============================================================================
// Command 1: Test Feed
// ============================================================================

#[tauri::command]
pub async fn toolkit_test_feed(url: String) -> Result<FeedTestResult> {
    info!(target: "4da::toolkit", url = %url, "Testing feed URL");

    // Validate URL
    validate_feed_url(&url)?;

    let client = crate::sources::shared_client();
    let start = std::time::Instant::now();

    // Fetch with timeout
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .header("User-Agent", "4DA/1.0 Feed Tester")
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "HTTP {}: {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown")
        )
        .into());
    }

    let xml = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let fetch_duration_ms = start.elapsed().as_millis() as u64;

    // Detect format
    let format = detect_feed_format(&xml).to_string();

    // Parse using RssSource's parse_feed
    let rss_source = crate::sources::rss::RssSource::new();
    let entries = rss_source.parse_feed(&xml, &url);

    let mut errors = Vec::new();
    if entries.is_empty() && format == "Unknown" {
        errors.push(
            "Could not detect feed format. Ensure the URL points to a valid RSS or Atom feed."
                .to_string(),
        );
    }

    let feed_title = entries.first().map(|e| e.feed_title.clone());
    let item_count = entries.len();

    // Take first 10 items with content preview
    let items: Vec<FeedTestItem> = entries
        .into_iter()
        .take(10)
        .map(|e| {
            let preview = if e.description.len() > 200 {
                let truncated = crate::truncate_utf8(&e.description, 200);
                format!("{}...", truncated)
            } else {
                e.description.clone()
            };
            FeedTestItem {
                title: e.title,
                url: e.link,
                published_at: e.pub_date,
                content_preview: preview,
            }
        })
        .collect();

    info!(target: "4da::toolkit", format = %format, items = item_count, duration_ms = fetch_duration_ms, "Feed test complete");

    Ok(FeedTestResult {
        feed_title,
        format,
        item_count,
        items,
        fetch_duration_ms,
        errors,
    })
}

// ============================================================================
// Command 2: Scoring Sandbox
// ============================================================================

#[tauri::command]
pub async fn toolkit_score_sandbox(
    title: String,
    content: Option<String>,
    source_type: Option<String>,
) -> Result<SandboxScoreResult> {
    info!(target: "4da::toolkit", title = %title, "Scoring sandbox request");

    let db = crate::get_database()?;
    let ctx = crate::scoring::build_scoring_context(db).await?;

    let content_str = content.unwrap_or_default();
    let src_type = source_type.unwrap_or_else(|| "sandbox".to_string());
    let empty_embedding: Vec<f32> = vec![];

    let input = crate::scoring::ScoringInput {
        id: 0,
        title: &title,
        url: None,
        content: &content_str,
        source_type: &src_type,
        embedding: &empty_embedding,
        created_at: None,
    };

    let options = crate::scoring::ScoringOptions {
        apply_freshness: false,
        apply_signals: false,
    };

    let result = crate::scoring::score_item(&input, &ctx, db, &options, None);

    // Extract breakdown details
    let breakdown = if let Some(ref bd) = result.score_breakdown {
        SandboxBreakdown {
            keyword_score: bd.keyword_score,
            interest_score: bd.interest_score,
            ace_boost: bd.ace_boost,
            affinity_mult: bd.affinity_mult,
            domain_relevance: bd.context_score,
            content_quality: bd.source_quality_boost,
        }
    } else {
        SandboxBreakdown {
            keyword_score: 0.0,
            interest_score: result.interest_score,
            ace_boost: 0.0,
            affinity_mult: 1.0,
            domain_relevance: result.context_score,
            content_quality: 0.0,
        }
    };

    // Extract matched interests from matches
    let matched_interests: Vec<String> = result
        .matches
        .iter()
        .map(|m| m.matched_text.clone())
        .collect();

    info!(target: "4da::toolkit", score = result.top_score, relevant = result.relevant, "Sandbox scoring complete");

    Ok(SandboxScoreResult {
        score: result.top_score,
        relevant: result.relevant,
        breakdown,
        matched_interests,
        explanation: result.explanation,
    })
}

// ============================================================================
// Helper (extracted from toolkit_test_feed for testability)
// ============================================================================

/// Detect whether raw XML is Atom, RSS 2.0, or Unknown.
pub(crate) fn detect_feed_format(xml: &str) -> &'static str {
    if (xml.contains("<feed") && xml.contains("xmlns=\"http://www.w3.org/2005/Atom\""))
        || (xml.contains("<entry>") && !xml.contains("<item>"))
    {
        "Atom"
    } else if xml.contains("<rss") || xml.contains("<item>") {
        "RSS 2.0"
    } else {
        "Unknown"
    }
}

/// Validate a feed URL (must start with http:// or https://).
pub(crate) fn validate_feed_url(url: &str) -> Result<()> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URL must start with http:// or https://".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------
    // 1. FeedTestResult construction & serde round-trip
    // ----------------------------------------------------------------
    #[test]
    fn feed_test_result_serde_roundtrip() {
        let result = FeedTestResult {
            feed_title: Some("My Feed".into()),
            format: "RSS 2.0".into(),
            item_count: 3,
            items: vec![FeedTestItem {
                title: "Post 1".into(),
                url: "https://example.com/1".into(),
                published_at: Some("2025-01-01T00:00:00Z".into()),
                content_preview: "Hello world".into(),
            }],
            fetch_duration_ms: 123,
            errors: vec![],
        };

        let json = serde_json::to_string(&result).expect("serialize");
        let decoded: FeedTestResult = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.feed_title, Some("My Feed".into()));
        assert_eq!(decoded.format, "RSS 2.0");
        assert_eq!(decoded.item_count, 3);
        assert_eq!(decoded.items.len(), 1);
        assert_eq!(decoded.items[0].title, "Post 1");
        assert_eq!(decoded.fetch_duration_ms, 123);
        assert!(decoded.errors.is_empty());
    }

    // ----------------------------------------------------------------
    // 2. FeedTestResult with no title and errors
    // ----------------------------------------------------------------
    #[test]
    fn feed_test_result_with_errors() {
        let result = FeedTestResult {
            feed_title: None,
            format: "Unknown".into(),
            item_count: 0,
            items: vec![],
            fetch_duration_ms: 50,
            errors: vec!["Could not detect feed format.".into()],
        };

        assert!(result.feed_title.is_none());
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].contains("Could not detect"));
    }

    // ----------------------------------------------------------------
    // 3. FeedTestItem clone and debug
    // ----------------------------------------------------------------
    #[test]
    fn feed_test_item_clone_and_debug() {
        let item = FeedTestItem {
            title: "Rust 1.80 Released".into(),
            url: "https://blog.rust-lang.org/1.80".into(),
            published_at: None,
            content_preview: "Major release with...".into(),
        };

        let cloned = item.clone();
        assert_eq!(cloned.title, item.title);
        assert_eq!(cloned.url, item.url);
        assert_eq!(cloned.published_at, item.published_at);

        // Debug trait should produce non-empty output
        let debug_str = format!("{:?}", item);
        assert!(debug_str.contains("Rust 1.80 Released"));
    }

    // ----------------------------------------------------------------
    // 4. SandboxBreakdown construction & serde
    // ----------------------------------------------------------------
    #[test]
    fn sandbox_breakdown_serde_roundtrip() {
        let breakdown = SandboxBreakdown {
            keyword_score: 0.75,
            interest_score: 0.5,
            ace_boost: 0.1,
            affinity_mult: 1.2,
            domain_relevance: 0.8,
            content_quality: 0.3,
        };

        let json = serde_json::to_string(&breakdown).expect("serialize");
        let decoded: SandboxBreakdown = serde_json::from_str(&json).expect("deserialize");

        assert!((decoded.keyword_score - 0.75).abs() < f32::EPSILON);
        assert!((decoded.interest_score - 0.5).abs() < f32::EPSILON);
        assert!((decoded.ace_boost - 0.1).abs() < f32::EPSILON);
        assert!((decoded.affinity_mult - 1.2).abs() < f32::EPSILON);
        assert!((decoded.domain_relevance - 0.8).abs() < f32::EPSILON);
        assert!((decoded.content_quality - 0.3).abs() < f32::EPSILON);
    }

    // ----------------------------------------------------------------
    // 5. SandboxBreakdown default/fallback values
    // ----------------------------------------------------------------
    #[test]
    fn sandbox_breakdown_fallback_defaults() {
        let fallback = SandboxBreakdown {
            keyword_score: 0.0,
            interest_score: 0.0,
            ace_boost: 0.0,
            affinity_mult: 1.0,
            domain_relevance: 0.0,
            content_quality: 0.0,
        };

        assert!((fallback.affinity_mult - 1.0).abs() < f32::EPSILON);
        assert!((fallback.keyword_score).abs() < f32::EPSILON);
        assert!((fallback.ace_boost).abs() < f32::EPSILON);
    }

    // ----------------------------------------------------------------
    // 6. SandboxScoreResult construction & serde
    // ----------------------------------------------------------------
    #[test]
    fn sandbox_score_result_serde_roundtrip() {
        let result = SandboxScoreResult {
            score: 0.85,
            relevant: true,
            breakdown: SandboxBreakdown {
                keyword_score: 0.6,
                interest_score: 0.4,
                ace_boost: 0.05,
                affinity_mult: 1.1,
                domain_relevance: 0.7,
                content_quality: 0.2,
            },
            matched_interests: vec!["rust".into(), "tauri".into()],
            explanation: Some("Strong keyword match".into()),
        };

        let json = serde_json::to_string(&result).expect("serialize");
        let decoded: SandboxScoreResult = serde_json::from_str(&json).expect("deserialize");

        assert!((decoded.score - 0.85).abs() < f32::EPSILON);
        assert!(decoded.relevant);
        assert_eq!(decoded.matched_interests, vec!["rust", "tauri"]);
        assert_eq!(decoded.explanation, Some("Strong keyword match".into()));
    }

    // ----------------------------------------------------------------
    // 7. SandboxScoreResult with no matches and no explanation
    // ----------------------------------------------------------------
    #[test]
    fn sandbox_score_result_empty_matches() {
        let result = SandboxScoreResult {
            score: 0.1,
            relevant: false,
            breakdown: SandboxBreakdown {
                keyword_score: 0.0,
                interest_score: 0.0,
                ace_boost: 0.0,
                affinity_mult: 1.0,
                domain_relevance: 0.0,
                content_quality: 0.0,
            },
            matched_interests: vec![],
            explanation: None,
        };

        assert!(!result.relevant);
        assert!(result.matched_interests.is_empty());
        assert!(result.explanation.is_none());
    }

    // ----------------------------------------------------------------
    // 8. detect_feed_format — RSS detection
    // ----------------------------------------------------------------
    #[test]
    fn detect_feed_format_rss() {
        let xml = r#"<?xml version="1.0"?><rss version="2.0"><channel><item><title>Hello</title></item></channel></rss>"#;
        assert_eq!(detect_feed_format(xml), "RSS 2.0");
    }

    // ----------------------------------------------------------------
    // 10. detect_feed_format — Atom detection (xmlns)
    // ----------------------------------------------------------------
    #[test]
    fn detect_feed_format_atom_xmlns() {
        let xml = r#"<?xml version="1.0"?><feed xmlns="http://www.w3.org/2005/Atom"><entry><title>Hello</title></entry></feed>"#;
        assert_eq!(detect_feed_format(xml), "Atom");
    }

    // ----------------------------------------------------------------
    // 11. detect_feed_format — Atom detection (entry without item)
    // ----------------------------------------------------------------
    #[test]
    fn detect_feed_format_atom_entry_only() {
        // Has <entry> but no <item> → Atom even without xmlns
        let xml = r#"<feed><entry><title>Test</title></entry></feed>"#;
        assert_eq!(detect_feed_format(xml), "Atom");
    }

    // ----------------------------------------------------------------
    // 12. detect_feed_format — Unknown format
    // ----------------------------------------------------------------
    #[test]
    fn detect_feed_format_unknown() {
        let xml = "<html><body>Not a feed</body></html>";
        assert_eq!(detect_feed_format(xml), "Unknown");
    }

    // ----------------------------------------------------------------
    // 13. detect_feed_format — item-only (RSS)
    // ----------------------------------------------------------------
    #[test]
    fn detect_feed_format_item_only_rss() {
        // Has <item> without <rss> tag — still detected as RSS 2.0
        let xml = "<channel><item><title>Test</title></item></channel>";
        assert_eq!(detect_feed_format(xml), "RSS 2.0");
    }

    // ----------------------------------------------------------------
    // 14. validate_feed_url — valid URLs
    // ----------------------------------------------------------------
    #[test]
    fn validate_feed_url_valid() {
        assert!(validate_feed_url("https://example.com/feed.xml").is_ok());
        assert!(validate_feed_url("http://example.com/rss").is_ok());
        assert!(validate_feed_url("https://feeds.feedburner.com/example").is_ok());
    }

    // ----------------------------------------------------------------
    // 15. validate_feed_url — invalid URLs
    // ----------------------------------------------------------------
    #[test]
    fn validate_feed_url_invalid() {
        assert!(validate_feed_url("ftp://example.com").is_err());
        assert!(validate_feed_url("example.com/feed").is_err());
        assert!(validate_feed_url("").is_err());
        assert!(validate_feed_url("file:///etc/passwd").is_err());

        let err = validate_feed_url("ftp://x").unwrap_err();
        assert!(err.to_string().contains("http://"));
    }
}
