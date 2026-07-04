// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Brief rejection trailer — the structured bridge between the narrated
//! Brief's "Filtered Out" verdicts and the scored feed.
//!
//! The Brief narration prompt requires the LLM to end its response with a
//! fenced machine trailer:
//!
//! ~~~text
//! ```rejects
//! [{"idx": 3, "reason": "self-promotional"}]
//! ```
//! ~~~
//!
//! `idx` is the 1-based `index` attribute of the `<source_item>` blocks in
//! the narration prompt, so each verdict joins back to a real
//! `source_items.id` without any prose interpretation. The trailer is parsed
//! defensively and ALWAYS stripped before the briefing is persisted or
//! rendered — no UI path may show it.
//!
//! Accuracy-first degradation: a missing block, malformed JSON, an empty
//! reason, or ANY out-of-range index records NOTHING. Verdicts are never
//! guessed from prose and never fuzzy-matched by title.

use std::collections::HashMap;

use serde::Deserialize;
use tracing::debug;

use crate::SourceRelevance;

/// Opening fence of the machine trailer. The language tag makes the block
/// unambiguous — briefing prose legitimately contains other fenced blocks.
pub(crate) const REJECTS_FENCE_OPEN: &str = "```rejects";

/// Reasons are short slugs ("self-promotional", "no stack relevance").
/// Anything longer is truncated at a char boundary before storage.
const MAX_REASON_LEN: usize = 80;

/// One entry of the parsed machine trailer (still index-based, not yet
/// joined to item ids).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TrailerReject {
    pub idx: usize,
    pub reason: String,
}

/// Extract and strip the machine trailer from an LLM briefing response.
///
/// Returns `(stripped_markdown, parsed_rejects)`. The block is stripped from
/// the returned markdown whenever the opening fence is present — even when
/// the JSON inside is malformed — because the raw trailer must never reach a
/// render or persistence path. Malformed or missing JSON yields zero rejects.
pub(crate) fn extract_rejects_trailer(content: &str) -> (String, Vec<TrailerReject>) {
    let Some(open) = content.rfind(REJECTS_FENCE_OPEN) else {
        debug!(target: "4da::briefing", "no rejects trailer in briefing response — recording nothing");
        return (content.to_string(), Vec::new());
    };
    let body_start = open + REJECTS_FENCE_OPEN.len();
    let after_open = &content[body_start..];
    // Closing fence: the next ``` after the opening tag. An unterminated
    // block runs to the end of the response (still stripped — never shown).
    let (json_body, block_end) = match after_open.find("```") {
        Some(close) => (&after_open[..close], body_start + close + 3),
        None => (after_open, content.len()),
    };

    let mut stripped = String::with_capacity(content.len());
    stripped.push_str(content[..open].trim_end());
    let tail = content[block_end..].trim();
    if !tail.is_empty() {
        stripped.push_str("\n\n");
        stripped.push_str(tail);
    }

    let rejects = match serde_json::from_str::<Vec<TrailerReject>>(json_body.trim()) {
        Ok(r) => r,
        Err(e) => {
            debug!(target: "4da::briefing", error = %e, "malformed rejects trailer JSON — recording nothing");
            Vec::new()
        }
    };
    (stripped, rejects)
}

/// Map 1-based trailer indices onto the narrated slate's item ids.
///
/// `slate_ids` must be the ids of the items actually shown to the LLM, in
/// prompt order. Any out-of-range index or empty reason means the model
/// broke the contract — the ENTIRE trailer is distrusted and nothing is
/// recorded (accuracy-first: partial trust would misattribute verdicts).
pub(crate) fn map_rejects_to_item_ids(
    rejects: &[TrailerReject],
    slate_ids: &[i64],
) -> Vec<(i64, String)> {
    let mut mapped = Vec::with_capacity(rejects.len());
    for r in rejects {
        let reason: String = r
            .reason
            .trim()
            .chars()
            .take(MAX_REASON_LEN)
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        if reason.is_empty() {
            debug!(target: "4da::briefing", idx = r.idx, "empty rejects reason — recording nothing");
            return Vec::new();
        }
        if r.idx == 0 || r.idx > slate_ids.len() {
            debug!(
                target: "4da::briefing",
                idx = r.idx,
                slate = slate_ids.len(),
                "rejects trailer index out of range — recording nothing"
            );
            return Vec::new();
        }
        mapped.push((slate_ids[r.idx - 1], reason));
    }
    mapped
}

/// Map and persist the trailer verdicts for a saved briefing. Failures are
/// logged and swallowed — rejection persistence must never fail the briefing
/// generation itself.
pub(crate) fn record_rejections(
    db: &crate::db::Database,
    briefing_id: i64,
    rejects: &[TrailerReject],
    slate_ids: &[i64],
) {
    let mapped = map_rejects_to_item_ids(rejects, slate_ids);
    if mapped.is_empty() {
        return;
    }
    match db.save_brief_rejections(briefing_id, &mapped) {
        Ok(n) => {
            tracing::info!(target: "4da::briefing", count = n, "Recorded Brief rejection verdicts")
        }
        Err(e) => {
            tracing::error!(target: "4da::briefing", error = %e, "Failed to persist brief rejections")
        }
    }
}

/// Demote (never delete) feed results the Brief rejected.
///
/// Sets `excluded = true` + `excluded_by = "brief:{reason}"` so the canonical
/// `sort_results` pushes them to the bottom of the feed. Scores are NOT
/// modified and nothing is removed. Dep-grounded items are immune: an item
/// with a verified dependency edge into the user's actual stack
/// (`ScoreBreakdown.strongly_grounded`) must never be suppressed by a
/// narration verdict. Returns the number of items demoted.
pub(crate) fn apply_brief_rejection_demotions(
    results: &mut [SourceRelevance],
    rejections: &HashMap<i64, String>,
) -> usize {
    if rejections.is_empty() {
        return 0;
    }
    let mut demoted = 0usize;
    for r in results.iter_mut() {
        if r.excluded {
            continue;
        }
        let Some(reason) = rejections.get(&(r.id as i64)) else {
            continue;
        };
        let dep_grounded = r
            .score_breakdown
            .as_ref()
            .is_some_and(|b| b.strongly_grounded);
        if dep_grounded {
            continue;
        }
        r.excluded = true;
        r.excluded_by = Some(format!("brief:{reason}"));
        demoted += 1;
    }
    demoted
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Trailer extraction
    // ========================================================================

    #[test]
    fn well_formed_trailer_is_parsed_and_stripped() {
        let content = "## Action Required\nRead the tokio advisory.\n\n\
                       ## Filtered Out\nDropped self-promo.\n\n\
                       ```rejects\n[{\"idx\": 3, \"reason\": \"self-promotional\"}, {\"idx\": 7, \"reason\": \"no stack relevance\"}]\n```";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert_eq!(rejects.len(), 2);
        assert_eq!(rejects[0].idx, 3);
        assert_eq!(rejects[0].reason, "self-promotional");
        assert_eq!(rejects[1].idx, 7);
        assert!(
            !stripped.contains("```rejects"),
            "trailer must be stripped from rendered markdown"
        );
        assert!(!stripped.contains("no stack relevance"));
        assert!(stripped.ends_with("Dropped self-promo."));
        assert!(stripped.starts_with("## Action Required"));
    }

    #[test]
    fn empty_array_trailer_yields_no_rejects_and_is_stripped() {
        let content = "Brief prose.\n\n```rejects\n[]\n```";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert!(rejects.is_empty());
        assert_eq!(stripped, "Brief prose.");
    }

    #[test]
    fn missing_block_leaves_content_unchanged_and_records_nothing() {
        let content = "## Action Required\nNothing urgent today.";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert!(rejects.is_empty());
        assert_eq!(stripped, content);
    }

    #[test]
    fn malformed_json_is_stripped_but_records_nothing() {
        let content = "Prose.\n\n```rejects\n[{\"idx\": oops not json]\n```";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert!(rejects.is_empty(), "malformed JSON must record nothing");
        assert!(
            !stripped.contains("```rejects"),
            "even a malformed trailer must never render"
        );
        assert_eq!(stripped, "Prose.");
    }

    #[test]
    fn unterminated_trailer_is_stripped_to_end() {
        let content = "Prose.\n\n```rejects\n[{\"idx\": 1, \"reason\": \"spam\"}]";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert_eq!(rejects.len(), 1, "unterminated fence still parses the body");
        assert_eq!(stripped, "Prose.");
    }

    #[test]
    fn text_after_trailer_is_preserved() {
        let content = "Prose.\n\n```rejects\n[]\n```\nTrailing note.";
        let (stripped, _) = extract_rejects_trailer(content);
        assert_eq!(stripped, "Prose.\n\nTrailing note.");
    }

    #[test]
    fn other_fenced_blocks_are_untouched() {
        let content =
            "Prose with code:\n```rust\nfn main() {}\n```\nMore prose.\n\n```rejects\n[]\n```";
        let (stripped, _) = extract_rejects_trailer(content);
        assert!(
            stripped.contains("```rust"),
            "non-trailer fences must survive"
        );
        assert!(stripped.contains("fn main() {}"));
        assert!(!stripped.contains("```rejects"));
    }

    // ========================================================================
    // Index -> id mapping
    // ========================================================================

    fn reject(idx: usize, reason: &str) -> TrailerReject {
        TrailerReject {
            idx,
            reason: reason.to_string(),
        }
    }

    #[test]
    fn indices_map_one_based_onto_slate_ids() {
        let slate = vec![100, 200, 300];
        let mapped = map_rejects_to_item_ids(&[reject(1, "spam"), reject(3, "off-stack")], &slate);
        assert_eq!(
            mapped,
            vec![(100, "spam".to_string()), (300, "off-stack".to_string())]
        );
    }

    #[test]
    fn any_out_of_range_index_voids_the_entire_trailer() {
        let slate = vec![100, 200, 300];
        // idx 4 is out of range: even the valid idx 1 must NOT be recorded —
        // a contract-breaking model is distrusted wholesale.
        let mapped = map_rejects_to_item_ids(&[reject(1, "spam"), reject(4, "bad")], &slate);
        assert!(mapped.is_empty());
        // idx 0 is not a valid 1-based index either.
        let mapped = map_rejects_to_item_ids(&[reject(0, "spam")], &slate);
        assert!(mapped.is_empty());
    }

    #[test]
    fn empty_reason_voids_the_entire_trailer() {
        let slate = vec![100, 200];
        let mapped = map_rejects_to_item_ids(&[reject(1, "  "), reject(2, "ok")], &slate);
        assert!(mapped.is_empty());
    }

    #[test]
    fn long_reasons_are_truncated() {
        let slate = vec![100];
        let long = "x".repeat(500);
        let mapped = map_rejects_to_item_ids(&[reject(1, &long)], &slate);
        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0].1.chars().count(), MAX_REASON_LEN);
    }

    // ========================================================================
    // Feed demotion
    // ========================================================================

    /// Minimal SourceRelevance via serde defaults (mirrors analyzer_tests.rs).
    fn make_result(id: u64, score: f32, strongly_grounded: bool) -> SourceRelevance {
        let json = serde_json::json!({
            "id": id,
            "title": format!("item-{id}"),
            "url": null,
            "top_score": score,
            "matches": [],
            "relevant": true,
            "source_type": "test",
        });
        let mut result: SourceRelevance =
            serde_json::from_value(json).expect("SourceRelevance from JSON");
        let breakdown = serde_json::json!({
            "context_score": 0.0,
            "interest_score": 0.0,
            "ace_boost": 0.0,
            "affinity_mult": 1.0,
            "anti_penalty": 0.0,
            "confidence_by_signal": {},
            "strongly_grounded": strongly_grounded,
        });
        result.score_breakdown =
            Some(serde_json::from_value(breakdown).expect("ScoreBreakdown from JSON"));
        result
    }

    #[test]
    fn rejected_item_is_demoted_below_non_rejected_after_sort() {
        let mut results = vec![
            make_result(1, 0.95, false), // rejected by the Brief, ungrounded
            make_result(2, 0.40, false), // untouched
        ];
        let rejections: HashMap<i64, String> = [(1i64, "self-promotional".to_string())]
            .into_iter()
            .collect();
        let demoted = apply_brief_rejection_demotions(&mut results, &rejections);
        assert_eq!(demoted, 1);
        assert!(results[0].excluded);
        assert_eq!(
            results[0].excluded_by.as_deref(),
            Some("brief:self-promotional")
        );
        assert!(
            (results[0].top_score - 0.95).abs() < f32::EPSILON,
            "demotion must not change the score"
        );

        crate::scoring::sort_results(&mut results);
        assert_eq!(
            results.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![2, 1],
            "rejected 0.95 item must sink below the untouched 0.40 item"
        );
    }

    #[test]
    fn dep_grounded_item_is_immune_to_brief_rejection() {
        let mut results = vec![make_result(1, 0.9, true)];
        let rejections: HashMap<i64, String> = [(1i64, "no stack relevance".to_string())]
            .into_iter()
            .collect();
        let demoted = apply_brief_rejection_demotions(&mut results, &rejections);
        assert_eq!(demoted, 0);
        assert!(
            !results[0].excluded,
            "a dep-grounded item must never be suppressed by a narration verdict"
        );
        assert!(results[0].excluded_by.is_none());
    }

    #[test]
    fn already_excluded_items_keep_their_original_exclusion() {
        let mut results = vec![make_result(1, 0.9, false)];
        results[0].excluded = true;
        results[0].excluded_by = Some("anti-topic:crypto".to_string());
        let rejections: HashMap<i64, String> = [(1i64, "spam".to_string())].into_iter().collect();
        let demoted = apply_brief_rejection_demotions(&mut results, &rejections);
        assert_eq!(demoted, 0);
        assert_eq!(results[0].excluded_by.as_deref(), Some("anti-topic:crypto"));
    }

    #[test]
    fn non_rejected_items_are_untouched() {
        let mut results = vec![make_result(5, 0.7, false)];
        let rejections: HashMap<i64, String> = [(1i64, "spam".to_string())].into_iter().collect();
        assert_eq!(
            apply_brief_rejection_demotions(&mut results, &rejections),
            0
        );
        assert!(!results[0].excluded);
    }
}
