// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Deduplication utilities — normalize URLs and titles, then keep first-seen items.

/// Deduplicate items by normalized URL and normalized title.
/// Keeps the first occurrence (usually the oldest/original source).
pub(crate) fn dedup_stored_items(items: &[crate::db::StoredSourceItem]) -> Vec<usize> {
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_titles: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut keep_indices = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        // URL-based dedup (normalized)
        if let Some(ref url) = item.url {
            let normalized = normalize_url(url);
            if !normalized.is_empty() && !seen_urls.insert(normalized) {
                continue; // duplicate URL
            }
        }
        // Title-based dedup (aggressive normalization)
        let title_key = normalize_title_for_dedup(&item.title);
        if !title_key.is_empty() && !seen_titles.insert(title_key) {
            continue; // duplicate title
        }
        keep_indices.push(idx);
    }

    keep_indices
}

/// Normalize a title for dedup: decode entities, strip prefixes, remove punctuation
fn normalize_title_for_dedup(title: &str) -> String {
    // Decode HTML entities first so "&amp;" == "&"
    let decoded = crate::decode_html_entities(title);

    // Strip common source prefixes
    let stripped = decoded
        .trim()
        .trim_start_matches("[HN]")
        .trim_start_matches("Show HN:")
        .trim_start_matches("Ask HN:")
        .trim_start_matches("Tell HN:")
        .trim_start_matches("Launch HN:")
        .trim_start_matches("[D]") // Reddit discussion tag
        .trim_start_matches("[R]")
        .trim_start_matches("[P]")
        .trim();

    // Keep only alphanumeric + whitespace, normalize spaces, lowercase
    stripped
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Normalize a URL for dedup: strip www, trailing slash, query params, fragments, protocol
fn normalize_url(url: &str) -> String {
    let url = url.trim();
    let base = url
        .split('#')
        .next()
        .unwrap_or(url)
        .split('?')
        .next()
        .unwrap_or(url);
    base.replace("http://", "https://")
        .replace("://www.", "://")
        .trim_end_matches('/')
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_url_strips_protocol_www_trailing_slash_query_fragment() {
        assert_eq!(
            normalize_url("http://example.com/page"),
            "https://example.com/page"
        );

        // www stripping
        assert_eq!(
            normalize_url("https://www.example.com/page"),
            "https://example.com/page"
        );

        // Trailing slash removal
        assert_eq!(
            normalize_url("https://example.com/page/"),
            "https://example.com/page"
        );

        // Query parameter stripping
        assert_eq!(
            normalize_url("https://example.com/page?utm_source=twitter&ref=123"),
            "https://example.com/page"
        );

        // Fragment stripping
        assert_eq!(
            normalize_url("https://example.com/page#section-2"),
            "https://example.com/page"
        );

        // All combined: http + www + trailing slash + query + fragment
        assert_eq!(
            normalize_url("http://www.example.com/article/?ref=hn#comments"),
            "https://example.com/article"
        );

        // Lowercase normalization
        assert_eq!(
            normalize_url("HTTPS://Example.COM/Path"),
            "https://example.com/path"
        );

        // Empty and whitespace
        assert_eq!(normalize_url(""), "");
        assert_eq!(
            normalize_url("  https://example.com  "),
            "https://example.com"
        );
    }

    #[test]
    fn test_normalize_title_strips_prefixes_and_normalizes() {
        assert_eq!(
            normalize_title_for_dedup("Show HN: My Cool Project"),
            "my cool project"
        );
        assert_eq!(
            normalize_title_for_dedup("Ask HN: Best Rust framework?"),
            "best rust framework"
        );
        assert_eq!(
            normalize_title_for_dedup("Tell HN: I built a thing"),
            "i built a thing"
        );
        assert_eq!(
            normalize_title_for_dedup("Launch HN: NewStartup"),
            "newstartup"
        );

        // Reddit prefixes
        assert_eq!(
            normalize_title_for_dedup("[D] Discussion about transformers"),
            "discussion about transformers"
        );
        assert_eq!(
            normalize_title_for_dedup("[R] New paper on attention"),
            "new paper on attention"
        );

        // HTML entity decoding (via decode_html_entities)
        assert_eq!(
            normalize_title_for_dedup("Rust &amp; WebAssembly"),
            "rust webassembly"
        );
        assert_eq!(normalize_title_for_dedup("5 &gt; 3 &lt; 10"), "5 3 10");

        // Punctuation removal and whitespace normalization
        assert_eq!(
            normalize_title_for_dedup("  Hello,   World!  (2024)  "),
            "hello world 2024"
        );

        // Empty string
        assert_eq!(normalize_title_for_dedup(""), "");
    }

    #[test]
    fn test_normalize_title_dedup_equivalence() {
        // Two titles that differ only by source prefix should be equal after normalization
        let hn_title = normalize_title_for_dedup("Show HN: Building a Rust CLI tool");
        let raw_title = normalize_title_for_dedup("Building a Rust CLI tool");
        assert_eq!(hn_title, raw_title);

        // Same title with different HTML encoding
        let encoded = normalize_title_for_dedup("React &amp; Next.js Guide");
        let decoded = normalize_title_for_dedup("React & Next.js Guide");
        assert_eq!(encoded, decoded);
    }

    fn make_item(id: i64, title: &str, url: Option<&str>) -> crate::db::StoredSourceItem {
        crate::db::StoredSourceItem {
            id,
            source_type: "hackernews".to_string(),
            source_id: format!("test-{}", id),
            url: url.map(String::from),
            title: title.to_string(),
            content: String::new(),
            content_hash: format!("hash-{}", id),
            embedding: vec![],
            created_at: chrono::Utc::now(),
            last_seen: chrono::Utc::now(),
            detected_lang: "en".to_string(),
            feed_origin: None,
            tags: None,
        }
    }

    #[test]
    fn test_dedup_stored_items_removes_url_duplicates() {
        let items = vec![
            make_item(1, "First Article", Some("https://example.com/article")),
            make_item(
                2,
                "Different Title",
                Some("https://www.example.com/article/"),
            ),
            make_item(3, "Third Article", Some("https://other.com/post")),
        ];

        let kept = dedup_stored_items(&items);
        // Item 2 has the same normalized URL as item 1, so only items 1 and 3 should remain
        assert_eq!(kept, vec![0, 2]);
    }

    #[test]
    fn test_dedup_stored_items_removes_title_duplicates() {
        let items = vec![
            make_item(1, "Show HN: My Cool Tool", None),
            make_item(2, "My Cool Tool", None),
            make_item(3, "Completely Different Article", None),
        ];

        let kept = dedup_stored_items(&items);
        // Item 2 normalizes to same title as item 1 after prefix stripping
        assert_eq!(kept, vec![0, 2]);
    }

    #[test]
    fn test_dedup_stored_items_keeps_unique_items() {
        let items = vec![
            make_item(1, "Rust async runtime", Some("https://blog.com/rust")),
            make_item(2, "Go concurrency patterns", Some("https://blog.com/go")),
            make_item(
                3,
                "Python type hints guide",
                Some("https://blog.com/python"),
            ),
        ];

        let kept = dedup_stored_items(&items);
        assert_eq!(kept, vec![0, 1, 2]);
    }
}
