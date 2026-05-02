// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Background content enrichment for ambiguous-zone items.
//!
//! Items that arrive with empty or very short content (title-only) and score
//! in the ambiguous zone (0.20–0.55) cannot be reliably classified from the
//! title alone. This module fetches the page body, extracts readable text,
//! and updates the DB so the next scoring cycle can re-evaluate with richer
//! signal. Re-embedding is queued automatically via `embedding_status = 'pending'`.

use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::db::Database;

/// Domains that are known to block scraping or require authentication.
const UNFETCHABLE_DOMAINS: &[&str] = &[
    "twitter.com",
    "x.com",
    "facebook.com",
    "instagram.com",
    "linkedin.com",
    "t.co",
];

/// Maximum concurrent HTTP requests during enrichment.
const MAX_CONCURRENT_REQUESTS: usize = 5;

/// Maximum items to enrich per cycle.
const MAX_ENRICHMENT_CANDIDATES: usize = 20;

/// Minimum extracted text length to accept as enrichment (chars).
const MIN_ENRICHED_LENGTH: usize = 100;

// ============================================================================
// Public API
// ============================================================================

/// Fetch body content for ambiguous-zone items with empty/short content.
///
/// Returns the number of items successfully enriched. Designed to run as a
/// fire-and-forget background task after scoring completes — errors are logged
/// and swallowed, never bubbled to the caller.
pub(crate) async fn enrich_ambiguous_items(db: &Database) -> usize {
    let candidates = match db.get_enrichment_candidates(MAX_ENRICHMENT_CANDIDATES) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                target: "4da::enrichment",
                error = %e,
                "Failed to query enrichment candidates"
            );
            return 0;
        }
    };

    if candidates.is_empty() {
        return 0;
    }

    tracing::info!(
        target: "4da::enrichment",
        candidates = candidates.len(),
        "Starting background content enrichment"
    );

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let mut handles = Vec::with_capacity(candidates.len());

    for (id, url) in candidates {
        let permit = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = match permit.acquire().await {
                Ok(p) => p,
                Err(_) => return None,
            };
            fetch_and_extract(id, &url).await
        }));
    }

    let mut enriched_count = 0usize;
    for handle in handles {
        if let Ok(Some((id, content))) = handle.await {
            // Fetch the title so we can compute embed_text and content_hash
            let title = db
                .get_item_title(id)
                .ok()
                .flatten()
                .unwrap_or_default();

            let content_hash = crate::db::hash_content_parts(&[&title, &content]);
            let embed_text = format!("{}\n\n{}", title, content);

            if let Err(e) = db.update_enriched_content(id, &content, &content_hash, &embed_text) {
                tracing::warn!(
                    target: "4da::enrichment",
                    item_id = id,
                    error = %e,
                    "Failed to persist enriched content"
                );
            } else {
                enriched_count += 1;
            }
        }
    }

    if enriched_count > 0 {
        tracing::info!(
            target: "4da::enrichment",
            enriched = enriched_count,
            "Background enrichment cycle complete"
        );
    }

    enriched_count
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Check whether a URL points to a domain we know cannot be scraped.
fn is_unfetchable(url: &str) -> bool {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return true;
    }

    // Extract hostname from URL
    let host = url
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("");

    UNFETCHABLE_DOMAINS
        .iter()
        .any(|blocked| host == *blocked || host.ends_with(&format!(".{blocked}")))
}

/// Fetch a URL and extract readable body text.
///
/// Returns `Some((id, cleaned_text))` on success, `None` on any failure.
async fn fetch_and_extract(id: i64, url: &str) -> Option<(i64, String)> {
    if is_unfetchable(url) {
        return None;
    }

    // SSRF prevention: block internal/private network addresses
    if crate::url_validation::validate_not_internal(url).is_err() {
        tracing::warn!(
            target: "4da::enrichment",
            url = %url,
            "Blocked enrichment fetch to internal address (SSRF prevention)"
        );
        return None;
    }

    // Reuse the existing scrape_article_content which already handles:
    // - Semantic selectors (article, main, [role='main'], etc.)
    // - Body fallback
    // - Script/style stripping (via scraper text extraction)
    // - 100-char minimum
    // - 5000-char cap (MAX_CONTENT_LENGTH)
    // - 10s timeout
    // - Shared HTTP client
    let raw_content = crate::utils::scrape_article_content(url).await?;

    // Clean through preprocess_content to normalize whitespace, strip HTML entities, etc.
    let cleaned = crate::utils::preprocess_content(&raw_content);

    if cleaned.len() < MIN_ENRICHED_LENGTH {
        return None;
    }

    Some((id, cleaned))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_unfetchable_domain() {
        // Blocked domains
        assert!(is_unfetchable("https://twitter.com/user/status/123"));
        assert!(is_unfetchable("https://x.com/user/status/123"));
        assert!(is_unfetchable("https://facebook.com/post/456"));
        assert!(is_unfetchable("https://www.instagram.com/p/abc"));
        assert!(is_unfetchable("https://linkedin.com/in/someone"));
        assert!(is_unfetchable("https://t.co/abc123"));

        // Subdomains of blocked domains
        assert!(is_unfetchable("https://mobile.twitter.com/user"));
        assert!(is_unfetchable("https://www.facebook.com/post"));

        // Non-HTTP URLs
        assert!(is_unfetchable("ftp://example.com/file"));
        assert!(is_unfetchable("mailto:user@example.com"));
        assert!(is_unfetchable(""));

        // Allowed domains
        assert!(!is_unfetchable("https://example.com/article"));
        assert!(!is_unfetchable("https://blog.rust-lang.org/post"));
        assert!(!is_unfetchable("https://github.com/repo/issues/1"));
        assert!(!is_unfetchable("http://news.ycombinator.com/item?id=123"));
    }

    #[test]
    fn test_extract_body_content() {
        // Verify that scraper-based extraction would work with semantic selectors.
        // We test the HTML parsing logic directly since fetch_and_extract is async
        // and requires network access.
        use scraper::{Html, Selector};

        let html = r#"
        <html>
        <head><title>Test</title></head>
        <body>
            <nav>Navigation stuff</nav>
            <article>
                <h1>Great Article Title</h1>
                <p>This is the main article content that should be extracted by the scraper.
                It contains meaningful text that would help with scoring and classification
                of the item in the PASIFA pipeline. We need at least 100 characters.</p>
            </article>
            <footer>Footer stuff</footer>
        </body>
        </html>
        "#;

        let document = Html::parse_document(html);
        let selector = Selector::parse("article").expect("valid selector");
        let element = document.select(&selector).next().expect("article found");
        let text: String = element
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        assert!(text.len() > 100, "Article text should be >100 chars");
        assert!(
            text.contains("main article content"),
            "Should contain article body"
        );
        assert!(
            !text.contains("Navigation stuff"),
            "Should not contain nav"
        );
        assert!(
            !text.contains("Footer stuff"),
            "Should not contain footer"
        );
    }

    #[test]
    fn test_extract_body_content_fallback() {
        // When no semantic element exists, body should be the fallback
        use scraper::{Html, Selector};

        let html = r#"
        <html>
        <body>
            <div>
                <p>This page has no article or main element. The content lives directly
                in a div inside the body. The scraper should fall back to the body selector
                and still extract this text for enrichment purposes. Need enough chars here.</p>
            </div>
        </body>
        </html>
        "#;

        let document = Html::parse_document(html);

        // article should not match
        let article_sel = Selector::parse("article").expect("valid");
        assert!(
            document.select(&article_sel).next().is_none(),
            "No article element"
        );

        // body should match and have content
        let body_sel = Selector::parse("body").expect("valid");
        let element = document.select(&body_sel).next().expect("body found");
        let text: String = element
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        assert!(text.len() > 100, "Body text should be >100 chars");
        assert!(
            text.contains("no article or main element"),
            "Should contain body text"
        );
    }

    #[test]
    fn test_short_content_skipped() {
        // Content shorter than MIN_ENRICHED_LENGTH should be rejected
        use scraper::{Html, Selector};

        let html = r#"
        <html>
        <body>
            <article><p>Too short.</p></article>
        </body>
        </html>
        "#;

        let document = Html::parse_document(html);
        let selector = Selector::parse("article").expect("valid");
        let element = document.select(&selector).next().expect("article found");
        let text: String = element
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        assert!(
            text.len() < MIN_ENRICHED_LENGTH,
            "Short content should be below threshold (got {} chars)",
            text.len()
        );
    }
}
