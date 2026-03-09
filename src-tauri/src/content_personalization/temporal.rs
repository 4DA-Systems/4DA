//! L5 Temporal Evolution Engine — diff ribbons, feed echoes, progressive reveals.
//!
//! Detects what changed between lesson reads to show personalized temporal context:
//! - DiffRibbon: profile changes since last read
//! - FeedEcho: feed items matching lesson topics since last read
//! - ProgressiveReveal: newly completed upstream modules

use rusqlite::Connection;
use tracing::{debug, warn};

use super::context::PersonalizationContext;
use super::{DiffChange, FeedEchoItem, TemporalBlock, TemporalBlockType};

// ============================================================================
// Lesson → Topic Mapping
// ============================================================================

/// Map module/lesson to relevant topics for feed echo matching.
fn lesson_topics(module_id: &str, _lesson_idx: u32) -> Vec<&'static str> {
    match module_id {
        "S" => vec![
            "hardware",
            "gpu",
            "cpu",
            "llm",
            "ollama",
            "electricity",
            "infrastructure",
        ],
        "T" => vec!["moat", "niche", "rate", "specialization", "expertise"],
        "R" => vec![
            "revenue",
            "saas",
            "freelance",
            "consulting",
            "open-source",
            "course",
            "api",
        ],
        "E1" => vec!["launch", "ship", "mvp", "pricing", "marketing", "sprint"],
        "E2" => vec!["trend", "opportunity", "ai", "emerging", "github trending"],
        "T2" => vec!["automation", "bot", "pipeline", "ci/cd", "workflow"],
        "S2" => vec![
            "diversify",
            "stream",
            "passive income",
            "portfolio",
            "compound",
        ],
        _ => vec![],
    }
}

/// Map module dependencies — which modules must be completed before this one.
fn upstream_modules(module_id: &str) -> Vec<&'static str> {
    match module_id {
        "T" => vec!["S"],
        "R" => vec!["S", "T"],
        "E1" => vec!["S", "T", "R"],
        "E2" => vec!["S"],
        "T2" => vec!["S", "R", "E1"],
        "S2" => vec!["R", "E1"],
        _ => vec![],
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Compute all temporal blocks for a lesson.
pub fn compute_temporal_blocks(
    conn: &Connection,
    ctx: &PersonalizationContext,
    module_id: &str,
    lesson_idx: u32,
) -> Vec<TemporalBlock> {
    let mut blocks = Vec::new();

    // L5a: Diff ribbon — profile changes since last read
    if let Some(block) = compute_diff_ribbon(conn, ctx, module_id, lesson_idx) {
        blocks.push(block);
    }

    // L5b: Progressive reveal — newly completed upstream modules
    if let Some(block) = compute_progressive_reveal(conn, ctx, module_id, lesson_idx) {
        blocks.push(block);
    }

    // L5c: Feed echoes — matching items since last read
    if let Some(block) = compute_feed_echoes(conn, module_id, lesson_idx) {
        blocks.push(block);
    }

    blocks
}

/// Update the read state snapshot for this lesson.
pub fn update_read_state(
    conn: &Connection,
    ctx: &PersonalizationContext,
    module_id: &str,
    lesson_idx: u32,
) {
    let hash = super::context::context_hash(ctx);
    let snapshot = serde_json::to_string(ctx).unwrap_or_default();

    let result = conn.execute(
        "INSERT INTO content_read_state (module_id, lesson_idx, context_hash, profile_snapshot)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(module_id, lesson_idx) DO UPDATE SET
           context_hash = excluded.context_hash,
           profile_snapshot = excluded.profile_snapshot,
           read_at = datetime('now')",
        rusqlite::params![module_id, lesson_idx, hash, snapshot],
    );

    if let Err(e) = result {
        warn!(target: "4da::personalize", error = %e, "Failed to update read state");
    }
}

// ============================================================================
// Diff Ribbon
// ============================================================================

/// Compare current context with the snapshot from last read.
/// Returns a DiffRibbon block if changes were detected.
fn compute_diff_ribbon(
    conn: &Connection,
    ctx: &PersonalizationContext,
    module_id: &str,
    lesson_idx: u32,
) -> Option<TemporalBlock> {
    // Load previous snapshot
    let old_snapshot: String = conn
        .query_row(
            "SELECT profile_snapshot FROM content_read_state
             WHERE module_id = ?1 AND lesson_idx = ?2",
            rusqlite::params![module_id, lesson_idx],
            |row| row.get(0),
        )
        .ok()?;

    let old_ctx: PersonalizationContext = serde_json::from_str(&old_snapshot).ok()?;

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();

    // Compare stack
    for tech in &ctx.stack.primary {
        if !old_ctx.stack.primary.contains(tech) {
            added.push(format!("Stack: {}", tech));
        }
    }
    for tech in &old_ctx.stack.primary {
        if !ctx.stack.primary.contains(tech) {
            removed.push(format!("Stack: {}", tech));
        }
    }

    // Compare radar adopt ring
    for tech in &ctx.radar.adopt {
        if !old_ctx.radar.adopt.contains(tech) {
            added.push(format!("Radar Adopt: {}", tech));
        }
    }

    // Compare GPU
    let old_gpu = old_ctx.computed.gpu_tier.clone();
    let new_gpu = ctx.computed.gpu_tier.clone();
    if old_gpu != new_gpu {
        changed.push(DiffChange {
            field: "GPU Tier".into(),
            old_value: old_gpu,
            new_value: new_gpu,
        });
    }

    // Compare LLM tier
    if old_ctx.computed.llm_tier != ctx.computed.llm_tier {
        changed.push(DiffChange {
            field: "LLM Tier".into(),
            old_value: old_ctx.computed.llm_tier.clone(),
            new_value: ctx.computed.llm_tier.clone(),
        });
    }

    // Compare completed lessons
    if ctx.progress.completed_lesson_count > old_ctx.progress.completed_lesson_count {
        let delta = ctx.progress.completed_lesson_count - old_ctx.progress.completed_lesson_count;
        added.push(format!("{} more lessons completed", delta));
    }

    // Compare profile completeness
    if (ctx.computed.profile_completeness - old_ctx.computed.profile_completeness).abs() > 5.0 {
        changed.push(DiffChange {
            field: "Profile Completeness".into(),
            old_value: format!("{:.0}%", old_ctx.computed.profile_completeness),
            new_value: format!("{:.0}%", ctx.computed.profile_completeness),
        });
    }

    // Only return if something actually changed
    if added.is_empty() && removed.is_empty() && changed.is_empty() {
        return None;
    }

    debug!(
        target: "4da::personalize",
        module = module_id,
        lesson = lesson_idx,
        added = added.len(),
        removed = removed.len(),
        changed = changed.len(),
        "Diff ribbon computed"
    );

    Some(TemporalBlock {
        block_id: "diff_ribbon".into(),
        block_type: TemporalBlockType::DiffRibbon {
            added,
            removed,
            changed,
        },
    })
}

// ============================================================================
// Progressive Reveal
// ============================================================================

/// Check if upstream modules were completed since last read.
fn compute_progressive_reveal(
    conn: &Connection,
    ctx: &PersonalizationContext,
    module_id: &str,
    lesson_idx: u32,
) -> Option<TemporalBlock> {
    let upstream = upstream_modules(module_id);
    if upstream.is_empty() {
        return None;
    }

    // Load previous snapshot to check what was completed then
    let old_completed: Vec<String> = conn
        .query_row(
            "SELECT profile_snapshot FROM content_read_state
             WHERE module_id = ?1 AND lesson_idx = ?2",
            rusqlite::params![module_id, lesson_idx],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|snap| serde_json::from_str::<PersonalizationContext>(&snap).ok())
        .map(|old| old.progress.completed_modules)
        .unwrap_or_default();

    // Find newly completed upstream modules
    let newly_completed: Vec<String> = upstream
        .iter()
        .filter(|mid| {
            ctx.progress.completed_modules.contains(&mid.to_string())
                && !old_completed.contains(&mid.to_string())
        })
        .map(|mid| mid.to_string())
        .collect();

    if newly_completed.is_empty() {
        return None;
    }

    let unlocked_content: Vec<String> = newly_completed
        .iter()
        .map(|mid| match mid.as_str() {
            "S" => "Sovereign Profile data now available for personalization".into(),
            "T" => "Technical Moats analysis unlocked".into(),
            "R" => "Revenue Engine rankings now computed".into(),
            "E1" => "Execution metrics available".into(),
            _ => format!("Module {} insights now available", mid),
        })
        .collect();

    Some(TemporalBlock {
        block_id: "progressive_reveal".into(),
        block_type: TemporalBlockType::ProgressiveReveal {
            newly_completed,
            unlocked_content,
        },
    })
}

// ============================================================================
// Feed Echoes
// ============================================================================

/// Find feed items matching this lesson's topics since last read.
fn compute_feed_echoes(
    conn: &Connection,
    module_id: &str,
    lesson_idx: u32,
) -> Option<TemporalBlock> {
    let topics = lesson_topics(module_id, lesson_idx);
    if topics.is_empty() {
        return None;
    }

    // Get last read time for this lesson
    let last_read: Option<String> = conn
        .query_row(
            "SELECT read_at FROM content_read_state
             WHERE module_id = ?1 AND lesson_idx = ?2",
            rusqlite::params![module_id, lesson_idx],
            |row| row.get(0),
        )
        .ok();

    // Build parameterized LIKE conditions — each topic gets two params
    // (one for title LIKE, one for content LIKE)
    let like_patterns: Vec<String> = topics.iter().map(|t| format!("%{}%", t)).collect();

    // Param numbering: ?1..?N for title/content LIKE pairs,
    // then ?N+1 for the optional read_at timestamp
    let topic_conditions: Vec<String> = (0..topics.len())
        .map(|i| {
            let title_idx = i * 2 + 1;
            let content_idx = i * 2 + 2;
            format!(
                "(si.title LIKE ?{} OR si.content LIKE ?{})",
                title_idx, content_idx
            )
        })
        .collect();
    let topic_filter = topic_conditions.join(" OR ");
    let time_param_idx = topics.len() * 2 + 1;

    let query = if last_read.is_some() {
        format!(
            "SELECT si.title, si.source, si.url, si.fetched_at
             FROM source_items si
             WHERE ({}) AND si.fetched_at > ?{}
             ORDER BY si.fetched_at DESC
             LIMIT 5",
            topic_filter, time_param_idx
        )
    } else {
        format!(
            "SELECT si.title, si.source, si.url, si.fetched_at
             FROM source_items si
             WHERE ({}) AND si.fetched_at > datetime('now', '-7 days')
             ORDER BY si.fetched_at DESC
             LIMIT 3",
            topic_filter
        )
    };

    let mut stmt = conn.prepare(&query).ok()?;

    let map_row = |row: &rusqlite::Row| -> rusqlite::Result<FeedEchoItem> {
        Ok(FeedEchoItem {
            title: row.get(0)?,
            source: row.get(1)?,
            url: row.get(2)?,
            matched_topic: String::new(),
            fetched_at: row.get(3)?,
        })
    };

    // Build parameter slice: [pattern1_title, pattern1_content, pattern2_title, ...]
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    for pat in &like_patterns {
        params.push(Box::new(pat.clone()));
        params.push(Box::new(pat.clone()));
    }
    if let Some(ref read_at) = last_read {
        params.push(Box::new(read_at.clone()));
    }
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let raw_items: Vec<FeedEchoItem> = match stmt.query_map(param_refs.as_slice(), map_row) {
        Ok(rows) => rows
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(
                        "Row processing failed in content_personalization_temporal: {e}"
                    );
                    None
                }
            })
            .collect(),
        Err(_) => return None,
    };

    let mut items: Vec<FeedEchoItem> = raw_items
        .into_iter()
        .map(|mut item| {
            for topic in &topics {
                if item.title.to_lowercase().contains(topic) {
                    item.matched_topic = topic.to_string();
                    break;
                }
            }
            item
        })
        .collect();

    // Also include channel-sourced items matching lesson topics
    let channel_query = "SELECT si.title, c.title, si.url, csm.matched_at
         FROM channel_source_matches csm
         JOIN source_items si ON si.id = csm.source_item_id
         JOIN channels c ON c.id = csm.channel_id
         WHERE (si.title LIKE ?1 OR si.content LIKE ?1)
         ORDER BY csm.match_score DESC
         LIMIT 3";

    for topic in &topics {
        let pattern = format!("%{}%", topic);
        if let Ok(mut cstmt) = conn.prepare(channel_query) {
            if let Ok(rows) = cstmt.query_map(rusqlite::params![pattern], |row| {
                Ok(FeedEchoItem {
                    title: row.get(0)?,
                    source: format!("Channel: {}", row.get::<_, String>(1)?),
                    url: row.get(2)?,
                    matched_topic: topic.to_string(),
                    fetched_at: row.get(3)?,
                })
            }) {
                for row in rows.flatten() {
                    items.push(row);
                }
            }
        }
    }

    if items.is_empty() {
        return None;
    }

    debug!(
        target: "4da::personalize",
        module = module_id,
        lesson = lesson_idx,
        count = items.len(),
        "Feed echoes found"
    );

    Some(TemporalBlock {
        block_id: "feed_echo".into(),
        block_type: TemporalBlockType::FeedEcho { items },
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lesson_topics_coverage() {
        // Every module should have topics
        for mid in &["S", "T", "R", "E1", "E2", "T2", "S2"] {
            let topics = lesson_topics(mid, 0);
            assert!(
                !topics.is_empty(),
                "Module {} should have topic mappings",
                mid
            );
        }
    }

    #[test]
    fn test_upstream_modules() {
        assert!(upstream_modules("S").is_empty()); // S has no prerequisites
        assert_eq!(upstream_modules("T"), vec!["S"]);
        assert_eq!(upstream_modules("R"), vec!["S", "T"]);
        assert_eq!(upstream_modules("E1"), vec!["S", "T", "R"]);
    }
}
