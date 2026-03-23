// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

use super::text::MAX_CONTENT_LENGTH;

// ============================================================================
// Content Scraping
// ============================================================================

/// Scrape article content from a URL
pub(crate) async fn scrape_article_content(url: &str) -> Option<String> {
    use scraper::{Html, Selector};

    // Skip non-HTTP URLs and known problematic domains
    if !url.starts_with("http") {
        return None;
    }

    // Use shared client with per-request timeout
    let client = crate::sources::shared_client();

    // Fetch the page
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let html = response.text().await.ok()?;
    let document = Html::parse_document(&html);

    // Try multiple content selectors in order of preference
    let selectors = [
        "article",
        "main",
        "[role='main']",
        ".post-content",
        ".article-content",
        ".entry-content",
        ".content",
        "body",
    ];

    for selector_str in selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(element) = document.select(&selector).next() {
                let text: String = element
                    .text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");

                // Only use if we got meaningful content (at least 100 chars)
                if text.len() > 100 {
                    // Truncate to max length
                    let truncated = if text.len() > MAX_CONTENT_LENGTH {
                        text.chars().take(MAX_CONTENT_LENGTH).collect()
                    } else {
                        text
                    };
                    return Some(truncated);
                }
            }
        }
    }

    None
}
