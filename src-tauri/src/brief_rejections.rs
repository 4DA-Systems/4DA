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

/// Info-string tag of the machine trailer's fenced block. The tag makes the
/// block unambiguous — briefing prose legitimately contains other fenced
/// blocks.
const REJECTS_FENCE_TAG: &str = "rejects";

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

/// If `line` opens or closes a fenced block (3+ backticks at column 0),
/// returns the trimmed info string ("" for a bare opening/closing fence).
fn fence_info(line: &str) -> Option<&str> {
    let line = line.strip_suffix('\r').unwrap_or(line);
    let ticks = line.bytes().take_while(|&b| b == b'`').count();
    if ticks < 3 {
        return None;
    }
    let info = line[ticks..].trim();
    // Backticks in the info string mean inline code, not a fence.
    if info.contains('`') {
        return None;
    }
    Some(info)
}

/// Parse `body` as the expected trailer shape: a JSON array of objects each
/// carrying `idx` + `reason`. Anything else returns None.
fn parse_trailer_shape(body: &str) -> Option<Vec<TrailerReject>> {
    serde_json::from_str::<Vec<TrailerReject>>(body.trim()).ok()
}

/// Detect an UNTAGGED trailer at the very end of `s`: a trailing fenced block
/// (info string empty or `json`) or a trailing bare JSON array, but ONLY when
/// its body provably parses to the reject shape with at least one entry —
/// legit trailing content is never stripped on a guess. Returns
/// `(prefix_without_trailer, trailer_body)`.
fn split_trailing_untagged_trailer(s: &str) -> Option<(String, String)> {
    let trimmed = s.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    let mut line_starts: Vec<usize> = vec![0];
    for (i, b) in trimmed.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push(i + 1);
        }
    }
    let lines: Vec<&str> = trimmed.split('\n').collect();

    // Case A: trailing fenced block. Walk back from the closing fence to the
    // nearest fence line — if it is tagged as anything but ""/"json", the
    // block is ordinary content and stays.
    if lines.len() >= 2 && fence_info(lines[lines.len() - 1]) == Some("") {
        for open_idx in (0..lines.len() - 1).rev() {
            let Some(info) = fence_info(lines[open_idx]) else {
                continue;
            };
            if !(info.is_empty() || info.eq_ignore_ascii_case("json")) {
                return None;
            }
            let body = lines[open_idx + 1..lines.len() - 1].join("\n");
            match parse_trailer_shape(&body) {
                Some(r) if !r.is_empty() => {
                    let prefix = trimmed[..line_starts[open_idx]].trim_end().to_string();
                    return Some((prefix, body));
                }
                _ => return None,
            }
        }
        return None;
    }

    // Case B: trailing bare JSON array (the model dropped the fence
    // entirely). The array must start at a line head and run to EOF.
    if trimmed.ends_with(']') {
        for (idx, line) in lines.iter().enumerate().rev() {
            if !line.trim_start().starts_with('[') {
                continue;
            }
            let body = &trimmed[line_starts[idx]..];
            if parse_trailer_shape(body).is_some_and(|r| !r.is_empty()) {
                let mut prefix = trimmed[..line_starts[idx]].trim_end().to_string();
                // Consume a dangling unterminated fence opener left directly
                // above the array ("```json\n[...]" with no close).
                let last_line_start = prefix.rfind('\n').map_or(0, |p| p + 1);
                if fence_info(&prefix[last_line_start..])
                    .is_some_and(|i| i.is_empty() || i.eq_ignore_ascii_case("json"))
                {
                    prefix = prefix[..last_line_start].trim_end().to_string();
                }
                return Some((prefix, body.to_string()));
            }
        }
    }
    None
}

/// Extract and strip the machine trailer from an LLM briefing response.
///
/// Returns `(stripped_markdown, parsed_rejects)`.
///
/// Stripping (the raw trailer must never reach a render or persistence path):
/// - EVERY `rejects`-tagged fenced block is stripped, wherever it sits —
///   models sometimes emit the trailer twice or mid-response. Fence matching
///   is tolerant: 3+ backticks, optional spaces, case-insensitive tag
///   ("``` rejects", "````REJECTS"). An unterminated final block runs to EOF.
///   Malformed tagged blocks are stripped but contribute no verdicts.
/// - When NO tagged block exists, a fence-degraded trailer is still accepted:
///   a TRAILING ```/```json block or trailing bare JSON array, if and only if
///   its body parses to the expected reject shape. Anything else at the end
///   stays untouched.
///
/// Recording: the LAST candidate whose JSON fully parses to the expected
/// shape wins. Missing/malformed candidates yield zero rejects.
pub(crate) fn extract_rejects_trailer(content: &str) -> (String, Vec<TrailerReject>) {
    // Pass 1: strip every rejects-tagged fenced block.
    let mut segments: Vec<Vec<&str>> = vec![Vec::new()];
    let mut tagged_bodies: Vec<String> = Vec::new();
    let mut block_body: Vec<&str> = Vec::new();
    let mut in_rejects = false;
    for line in content.split('\n') {
        if in_rejects {
            if fence_info(line) == Some("") {
                in_rejects = false;
                tagged_bodies.push(block_body.join("\n"));
                block_body.clear();
                segments.push(Vec::new());
            } else {
                block_body.push(line);
            }
        } else if fence_info(line).is_some_and(|info| info.eq_ignore_ascii_case(REJECTS_FENCE_TAG))
        {
            in_rejects = true;
        } else {
            segments
                .last_mut()
                .expect("segments never empty")
                .push(line);
        }
    }
    if in_rejects {
        // Unterminated final block: stripped to EOF, still a candidate.
        tagged_bodies.push(block_body.join("\n"));
        segments.push(Vec::new());
    }

    // Reassemble kept prose; block boundaries collapse to one blank line.
    let mut remaining = if tagged_bodies.is_empty() {
        content.to_string()
    } else {
        let mut out = String::with_capacity(content.len());
        for seg in &segments {
            let text = seg.join("\n");
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(text);
        }
        out
    };

    // Pass 2: only when the model dropped the tag entirely do we consider an
    // untagged trailing trailer — if a tagged block exists, a trailing
    // ```json block is ordinary content and must survive.
    let mut untagged_body: Option<String> = None;
    if tagged_bodies.is_empty() {
        if let Some((prefix, body)) = split_trailing_untagged_trailer(&remaining) {
            untagged_body = Some(body);
            remaining = prefix;
        }
    }

    if tagged_bodies.is_empty() && untagged_body.is_none() {
        debug!(target: "4da::briefing", "no rejects trailer in briefing response — recording nothing");
        return (remaining, Vec::new());
    }

    // Record from the LAST candidate that fully parses to the expected shape.
    let rejects = tagged_bodies
        .iter()
        .rev()
        .find_map(|body| parse_trailer_shape(body))
        .or_else(|| untagged_body.as_deref().and_then(parse_trailer_shape))
        .unwrap_or_else(|| {
            debug!(target: "4da::briefing", "malformed rejects trailer JSON — recording nothing");
            Vec::new()
        });
    (remaining, rejects)
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

/// Demote (never delete) feed results the Brief rejected, and expire stale
/// demotions whose verdict has aged out.
///
/// Sets `excluded = true` + `excluded_by = "brief:{reason}"` so the canonical
/// `sort_results` pushes them to the bottom of the feed. Scores are NOT
/// modified and nothing is removed. Dep-grounded items are immune: an item
/// with a verified dependency edge into the user's actual stack
/// (`ScoreBreakdown.strongly_grounded`) must never be suppressed by a
/// narration verdict.
///
/// Conversely, an entry still carrying a `brief:*` exclusion whose item is
/// NOT in the current recent-rejections window is CLEARED — a week-old
/// verdict must not suppress an item forever. Only `brief:*` exclusions ever
/// expire this way; user/anti-topic exclusions are never touched. Returns
/// the number of items demoted.
pub(crate) fn apply_brief_rejection_demotions(
    results: &mut [SourceRelevance],
    rejections: &HashMap<i64, String>,
) -> usize {
    let mut demoted = 0usize;
    for r in results.iter_mut() {
        let Some(reason) = rejections.get(&(r.id as i64)) else {
            // No current verdict: expire a stale brief:* demotion left from
            // an earlier run. Never touch other exclusion kinds.
            if r.excluded
                && r.excluded_by
                    .as_deref()
                    .is_some_and(|e| e.starts_with("brief:"))
            {
                r.excluded = false;
                r.excluded_by = None;
            }
            continue;
        };
        if r.excluded {
            continue;
        }
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

    #[test]
    fn duplicate_rejects_blocks_all_stripped_last_used() {
        let content = "Intro.\n\n```rejects\n[{\"idx\": 1, \"reason\": \"first\"}]\n```\n\n\
                       Middle prose.\n\n```rejects\n[{\"idx\": 2, \"reason\": \"second\"}]\n```";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert_eq!(rejects.len(), 1, "last block wins");
        assert_eq!(rejects[0].idx, 2);
        assert_eq!(rejects[0].reason, "second");
        assert_eq!(stripped, "Intro.\n\nMiddle prose.", "BOTH blocks stripped");
    }

    #[test]
    fn last_malformed_block_falls_back_to_earlier_valid_one() {
        let content = "Prose.\n\n```rejects\n[{\"idx\": 1, \"reason\": \"valid\"}]\n```\n\n\
                       ```rejects\n[{oops\n```";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert_eq!(rejects.len(), 1, "last VALID candidate is used");
        assert_eq!(rejects[0].idx, 1);
        assert_eq!(stripped, "Prose.", "malformed block still stripped");
    }

    #[test]
    fn spaced_and_case_insensitive_fences_are_recognized() {
        let content = "Prose.\n\n``` rejects\n[{\"idx\": 1, \"reason\": \"spam\"}]\n```";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert_eq!(rejects.len(), 1);
        assert_eq!(stripped, "Prose.");

        let content = "Prose.\n\n````REJECTS\n[{\"idx\": 2, \"reason\": \"noise\"}]\n````";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].idx, 2);
        assert_eq!(stripped, "Prose.");
    }

    #[test]
    fn untagged_json_trailer_with_reject_shape_is_stripped_and_used() {
        // Fence degradation: the model tagged the trailer ```json instead of
        // ```rejects — the shape proves it is the trailer.
        let content = "Prose.\n\n```json\n[{\"idx\": 2, \"reason\": \"self-promo\"}]\n```";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].idx, 2);
        assert_eq!(stripped, "Prose.");

        // Bare ``` fence works too.
        let content = "Prose.\n\n```\n[{\"idx\": 1, \"reason\": \"spam\"}]\n```";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert_eq!(rejects.len(), 1);
        assert_eq!(stripped, "Prose.");
    }

    #[test]
    fn untagged_trailing_block_with_other_shape_stays_untouched() {
        // NOT the reject shape — legit content must never be stripped.
        let content = "Prose.\n\n```json\n{\"summary\": \"stats\"}\n```";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert!(rejects.is_empty());
        assert_eq!(stripped, content);

        let content = "Prose.\n\n```json\n[1, 2, 3]\n```";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert!(rejects.is_empty());
        assert_eq!(stripped, content);

        // Other info strings are ordinary code blocks.
        let content = "Prose.\n\n```rust\nlet x = [1];\n```";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert!(rejects.is_empty());
        assert_eq!(stripped, content);
    }

    #[test]
    fn prose_after_untagged_block_means_not_a_trailer() {
        let content = "Prose.\n\n```json\n[{\"idx\": 1, \"reason\": \"x\"}]\n```\nMore prose.";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert!(
            rejects.is_empty(),
            "a non-trailing block is never a trailer"
        );
        assert_eq!(stripped, content);
    }

    #[test]
    fn bare_trailing_json_array_with_reject_shape_is_stripped_and_used() {
        let content = "Prose.\n\n[{\"idx\": 3, \"reason\": \"noise\"}]";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].idx, 3);
        assert_eq!(stripped, "Prose.");

        // Multi-line array, and a dangling unterminated fence opener above
        // it is consumed too.
        let content =
            "Prose.\n\n```json\n[\n  {\"idx\": 1, \"reason\": \"spam\"},\n  {\"idx\": 2, \"reason\": \"noise\"}\n]";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert_eq!(rejects.len(), 2);
        assert_eq!(stripped, "Prose.");
    }

    #[test]
    fn bare_trailing_bracket_content_with_other_shape_stays() {
        // Ends with ']' but is prose / wrong shape — untouched.
        let content = "Scores were [1, 2, 3]";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert!(rejects.is_empty());
        assert_eq!(stripped, content);

        let content = "Prose.\n\n[\"a\", \"b\"]";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert!(rejects.is_empty());
        assert_eq!(stripped, content);
    }

    #[test]
    fn tagged_trailer_disables_untagged_scan() {
        // With an explicit ```rejects trailer present, an earlier ```json
        // block is ordinary content — it must survive, and the tagged
        // verdicts win.
        let content = "Prose.\n\n```json\n[{\"idx\": 9, \"reason\": \"example\"}]\n```\n\n\
                       ```rejects\n[{\"idx\": 1, \"reason\": \"spam\"}]\n```";
        let (stripped, rejects) = extract_rejects_trailer(content);
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].idx, 1);
        assert!(stripped.contains("```json"), "legit json block survives");
        assert!(stripped.contains("example"));
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
    fn stale_brief_demotion_expires_when_verdict_ages_out() {
        // Day-9 scenario: the item was demoted by a Brief verdict that has
        // since fallen out of the recent-rejections window — the demotion
        // must be cleared, not persist forever.
        let mut results = vec![make_result(1, 0.9, false)];
        results[0].excluded = true;
        results[0].excluded_by = Some("brief:self-promotional".to_string());
        let rejections: HashMap<i64, String> = HashMap::new();
        let demoted = apply_brief_rejection_demotions(&mut results, &rejections);
        assert_eq!(demoted, 0);
        assert!(!results[0].excluded, "aged-out brief demotion must clear");
        assert!(results[0].excluded_by.is_none());
    }

    #[test]
    fn non_brief_exclusions_are_never_expired() {
        let mut results = vec![make_result(1, 0.9, false)];
        results[0].excluded = true;
        results[0].excluded_by = Some("user-exclusion:crypto".to_string());
        let rejections: HashMap<i64, String> = HashMap::new();
        apply_brief_rejection_demotions(&mut results, &rejections);
        assert!(results[0].excluded, "only brief:* exclusions may expire");
        assert_eq!(
            results[0].excluded_by.as_deref(),
            Some("user-exclusion:crypto")
        );
    }

    #[test]
    fn current_brief_demotion_is_kept() {
        // Verdict still inside the window: the existing demotion stands
        // (idempotent — not re-counted as a new demotion).
        let mut results = vec![make_result(1, 0.9, false)];
        results[0].excluded = true;
        results[0].excluded_by = Some("brief:spam".to_string());
        let rejections: HashMap<i64, String> = [(1i64, "spam".to_string())].into_iter().collect();
        let demoted = apply_brief_rejection_demotions(&mut results, &rejections);
        assert_eq!(demoted, 0);
        assert!(results[0].excluded);
        assert_eq!(results[0].excluded_by.as_deref(), Some("brief:spam"));
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
