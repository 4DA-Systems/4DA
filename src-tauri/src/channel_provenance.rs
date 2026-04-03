//! Provenance extraction for channel renders.
//!
//! Parses [S1], [S2] source markers from LLM-generated markdown and maps them
//! back to the source items that were provided as context, producing a chain of
//! evidence (claim -> source) for every factual statement.

use std::collections::HashSet;

use crate::channels::RenderProvenance;
use crate::db::StoredSourceItem;

/// Parse source markers like [S1], [S2] from text without regex.
/// Returns a list of 1-based source indices found in the text.
pub(crate) fn parse_source_markers(text: &str) -> Vec<usize> {
    let mut markers = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Look for [S followed by digits followed by ]
        if bytes[i] == b'[' && i + 2 < len && bytes[i + 1] == b'S' {
            let start = i + 2;
            let mut end = start;
            while end < len && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end > start && end < len && bytes[end] == b']' {
                if let Ok(num) = text[start..end].parse::<usize>() {
                    markers.push(num);
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }

    markers
}

/// Remove source markers [S1], [S2] etc. from text.
pub(crate) fn strip_source_markers(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'[' && i + 2 < len && bytes[i + 1] == b'S' {
            let start = i + 2;
            let mut end = start;
            while end < len && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end > start && end < len && bytes[end] == b']' {
                // Skip this marker
                i = end + 1;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }

    result.trim().to_string()
}

/// Extract provenance from rendered markdown by parsing [S1], [S2] etc. markers.
pub(crate) fn extract_provenance(
    render_id: i64,
    content: &str,
    items: &[(StoredSourceItem, f64)],
) -> Vec<RenderProvenance> {
    let mut provenance: Vec<RenderProvenance> = Vec::new();
    let mut seen_claims: HashSet<usize> = HashSet::new();

    for (claim_idx, line) in content.lines().enumerate() {
        let markers = parse_source_markers(line);

        if markers.is_empty() {
            continue;
        }

        if seen_claims.contains(&claim_idx) {
            continue;
        }
        seen_claims.insert(claim_idx);

        let mut source_ids = Vec::new();
        let mut source_titles = Vec::new();
        let mut source_urls = Vec::new();

        for marker in &markers {
            let idx = marker.saturating_sub(1); // S1 = index 0
            if let Some((item, _)) = items.get(idx) {
                source_ids.push(item.id);
                source_titles.push(item.title.clone());
                source_urls.push(item.url.clone().unwrap_or_default());
            }
        }

        if !source_ids.is_empty() {
            let clean_text = strip_source_markers(line.trim());
            provenance.push(RenderProvenance {
                render_id,
                claim_index: claim_idx as i64,
                claim_text: clean_text,
                source_item_ids: source_ids,
                source_titles,
                source_urls,
            });
        }
    }

    provenance
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_source_markers() {
        assert_eq!(parse_source_markers("Claim [S1] here"), vec![1]);
        assert_eq!(parse_source_markers("[S1][S2]"), vec![1, 2]);
        assert_eq!(parse_source_markers("No markers"), Vec::<usize>::new());
        assert_eq!(parse_source_markers("[S12] big number"), vec![12]);
        assert_eq!(parse_source_markers("[S] no number"), Vec::<usize>::new());
    }

    #[test]
    fn test_strip_source_markers() {
        assert_eq!(strip_source_markers("Claim [S1] here [S2]"), "Claim  here");
        assert_eq!(strip_source_markers("No markers"), "No markers");
    }

    #[test]
    fn test_extract_provenance() {
        use chrono::Utc;

        let items: Vec<(StoredSourceItem, f64)> = vec![
            (
                StoredSourceItem {
                    id: 1,
                    source_type: "hn".to_string(),
                    source_id: "123".to_string(),
                    url: Some("https://example.com/1".to_string()),
                    title: "Article One".to_string(),
                    content: "Content one".to_string(),
                    content_hash: "hash1".to_string(),
                    embedding: vec![],
                    created_at: Utc::now(),
                    last_seen: Utc::now(),
                    detected_lang: "en".to_string(),
                },
                0.9,
            ),
            (
                StoredSourceItem {
                    id: 2,
                    source_type: "reddit".to_string(),
                    source_id: "456".to_string(),
                    url: Some("https://example.com/2".to_string()),
                    title: "Article Two".to_string(),
                    content: "Content two".to_string(),
                    content_hash: "hash2".to_string(),
                    embedding: vec![],
                    created_at: Utc::now(),
                    last_seen: Utc::now(),
                    detected_lang: "en".to_string(),
                },
                0.8,
            ),
        ];

        let content =
            "Some claim about GPUs [S1]\nAnother claim [S2]\nNo marker here\nBoth sources [S1][S2]";
        let prov = extract_provenance(100, content, &items);

        // Lines 0, 1, 3 have markers (line 2 has none)
        assert_eq!(prov.len(), 3);
        assert_eq!(prov[0].source_item_ids, vec![1]);
        assert_eq!(prov[1].source_item_ids, vec![2]);
        assert_eq!(prov[2].source_item_ids, vec![1, 2]);
    }
}
