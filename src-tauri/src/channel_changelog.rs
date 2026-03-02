//! Changelog computation for channel renders.
//!
//! Compares two render versions at the paragraph level using word-level Jaccard
//! similarity and produces a human-readable diff summary.

use std::collections::HashSet;

use crate::channels::{ChannelChangelog, ChannelRender};

/// Compute a changelog between two render versions using paragraph-level diff.
pub(crate) fn compute_changelog(
    channel_id: i64,
    old_render: &ChannelRender,
    new_render: &ChannelRender,
) -> ChannelChangelog {
    let old_paragraphs: Vec<&str> = old_render
        .content_markdown
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    let new_paragraphs: Vec<&str> = new_render
        .content_markdown
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();

    // Find paragraphs in new that don't match any in old (added)
    for np in &new_paragraphs {
        let best_match = old_paragraphs
            .iter()
            .map(|op| word_jaccard(op, np))
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
        if best_match < 0.3 {
            added.push(truncate_paragraph(np));
        }
    }

    // Find paragraphs in old that don't match any in new (removed)
    for op in &old_paragraphs {
        let best_match = new_paragraphs
            .iter()
            .map(|np| word_jaccard(op, np))
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
        if best_match < 0.3 {
            removed.push(truncate_paragraph(op));
        }
    }

    let summary = format!(
        "{} added, {} removed since v{}",
        added.len(),
        removed.len(),
        old_render.version
    );

    ChannelChangelog {
        channel_id,
        from_version: old_render.version,
        to_version: new_render.version,
        summary,
        added_lines: added,
        removed_lines: removed,
        changed_at: new_render.rendered_at.clone(),
    }
}

/// Word-level Jaccard similarity between two text blocks.
fn word_jaccard(a: &str, b: &str) -> f64 {
    let words_a: HashSet<&str> = a.split_whitespace().collect();
    let words_b: HashSet<&str> = b.split_whitespace().collect();
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Truncate a paragraph for changelog display.
fn truncate_paragraph(p: &str) -> String {
    if p.len() > 150 {
        format!("{}...", &p[..150])
    } else {
        p.to_string()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_jaccard_identical() {
        let score = word_jaccard("hello world foo", "hello world foo");
        assert!((score - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_word_jaccard_partial_overlap() {
        // "hello world foo" has 3 words, "hello world bar" has 3 words
        // intersection = {hello, world} = 2, union = {hello, world, foo, bar} = 4
        let score = word_jaccard("hello world foo", "hello world bar");
        assert!((score - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_word_jaccard_no_overlap() {
        let score = word_jaccard("abc def", "xyz uvw");
        assert!((score - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_changelog() {
        let old = ChannelRender {
            id: 1,
            channel_id: 1,
            version: 1,
            content_markdown:
                "## Current State\n\nGPUs are expensive.\n\nNVIDIA dominates the market."
                    .to_string(),
            content_hash: "old".to_string(),
            source_item_ids: vec![],
            model: None,
            tokens_used: None,
            latency_ms: None,
            rendered_at: "2024-01-01 00:00:00".to_string(),
        };
        let new = ChannelRender {
            id: 2,
            channel_id: 1,
            version: 2,
            content_markdown:
                "## Current State\n\nGPUs are getting cheaper.\n\nAMD is gaining ground."
                    .to_string(),
            content_hash: "new".to_string(),
            source_item_ids: vec![],
            model: None,
            tokens_used: None,
            latency_ms: None,
            rendered_at: "2024-01-02 00:00:00".to_string(),
        };

        let changelog = compute_changelog(1, &old, &new);
        assert_eq!(changelog.from_version, 1);
        assert_eq!(changelog.to_version, 2);
        assert!(!changelog.added_lines.is_empty() || !changelog.removed_lines.is_empty());
    }
}
