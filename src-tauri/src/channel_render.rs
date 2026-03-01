//! Channel render pipeline — source gathering, LLM synthesis, provenance extraction, changelog.
//!
//! This module orchestrates the full render cycle for an Information Channel:
//! 1. Gather source items matching the channel's topic query
//! 2. Build system/user prompts for LLM synthesis
//! 3. Call LLM (or generate fallback if unavailable)
//! 4. Extract provenance (source citations) from the rendered markdown
//! 5. Compute changelog against the previous render version

use std::collections::HashSet;
use tracing::{error, info, warn};

use crate::channels::{Channel, ChannelChangelog, ChannelRender, RenderProvenance};
use crate::db::{Database, StoredSourceItem};
use crate::extract_topics;
use crate::scoring::{compute_affinity_multiplier, get_ace_context};

// ============================================================================
// Source Gathering
// ============================================================================

/// Gather source items that match a channel's topic query.
/// Returns matched items with their match scores, sorted by score descending.
pub(crate) fn gather_channel_sources(
    db: &Database,
    channel: &Channel,
) -> Result<Vec<(StoredSourceItem, f64)>, String> {
    let channel_topics: Vec<String> = channel
        .topic_query
        .iter()
        .map(|t| t.to_lowercase())
        .collect();

    if channel_topics.is_empty() {
        return Ok(vec![]);
    }

    // Get recent source items (last 30 days, up to 500)
    let items = db
        .get_items_since_hours(30 * 24, 500)
        .map_err(|e| e.to_string())?;

    let mut scored: Vec<(StoredSourceItem, f64)> = Vec::new();
    let ace_ctx = get_ace_context();

    for item in items {
        let item_topics = extract_topics(&item.title, &item.content);
        let item_topics_lower: Vec<String> = item_topics.iter().map(|t| t.to_lowercase()).collect();

        // Score by keyword overlap between channel topics and item topics/title
        let matched = channel_topics
            .iter()
            .filter(|ct| {
                item_topics_lower.iter().any(|it| it.contains(ct.as_str()))
                    || item.title.to_lowercase().contains(ct.as_str())
            })
            .count();

        if matched > 0 {
            let base_score = matched as f64 / channel_topics.len() as f64;
            // Apply affinity boost from learned topic preferences
            let affinity_mult = compute_affinity_multiplier(&item_topics_lower, &ace_ctx);
            let score = (base_score * affinity_mult as f64).min(1.0);
            scored.push((item, score));
        }
    }

    // Sort by score descending, take top 30
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(30);

    // Persist matches in DB for the channel sources view
    for (item, score) in &scored {
        if let Err(e) = db.upsert_channel_source_match(channel.id, item.id, *score) {
            warn!(target: "4da::channels", error = %e, "Failed to store source match");
        }
    }

    Ok(scored)
}

/// Lightweight preview: count matching sources for given topics without persisting.
/// Returns (count, top 3 titles).
pub(crate) fn preview_channel_sources(
    db: &Database,
    topics: &[String],
) -> Result<(usize, Vec<String>), String> {
    let topics_lower: Vec<String> = topics.iter().map(|t| t.to_lowercase()).collect();
    if topics_lower.is_empty() {
        return Ok((0, vec![]));
    }

    let items = db
        .get_items_since_hours(30 * 24, 500)
        .map_err(|e| e.to_string())?;

    let mut matched_titles: Vec<(String, f64)> = Vec::new();

    for item in items {
        let item_topics = extract_topics(&item.title, &item.content);
        let item_topics_lower: Vec<String> = item_topics.iter().map(|t| t.to_lowercase()).collect();

        let hit_count = topics_lower
            .iter()
            .filter(|ct| {
                item_topics_lower.iter().any(|it| it.contains(ct.as_str()))
                    || item.title.to_lowercase().contains(ct.as_str())
            })
            .count();

        if hit_count > 0 {
            let score = hit_count as f64 / topics_lower.len() as f64;
            matched_titles.push((item.title.clone(), score));
        }
    }

    matched_titles.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let count = matched_titles.len();
    let top_titles: Vec<String> = matched_titles.into_iter().take(3).map(|(t, _)| t).collect();

    Ok((count, top_titles))
}

// ============================================================================
// LLM Prompt Building
// ============================================================================

/// Build system and user prompts for channel rendering.
fn build_channel_prompt(
    channel: &Channel,
    items: &[(StoredSourceItem, f64)],
    tech_stack: &str,
    active_topics: &str,
    sovereign_profile: &str,
    affinity_summary: &str,
    previous_render: Option<&ChannelRender>,
) -> (String, String) {
    let system_prompt = format!(
        "You are a senior technical analyst maintaining a live intelligence channel on: {title}\n\
         \n\
         {description}\n\
         \n\
         Write a concise reference document (max 800 words) covering the current state of this \
         topic based on the provided sources. Structure as:\n\
         \n\
         ## Current State\n\
         [2-3 paragraphs on where things stand right now]\n\
         \n\
         ## Key Developments\n\
         [3-5 bullet points on the most significant recent developments, each with [S{{n}}] source markers]\n\
         \n\
         ## What to Watch\n\
         [2-3 items worth monitoring going forward]\n\
         \n\
         Rules:\n\
         - After every factual claim, add a source marker like [S1], [S2] etc. referencing the source number\n\
         - Reference the user's tech stack when applicable: {tech}\n\
         - Be specific -- cite versions, benchmarks, dates from the sources\n\
         - If sources conflict, note the disagreement\n\
         - Do not fabricate information not present in the sources",
        title = channel.title,
        description = channel.description,
        tech = tech_stack,
    );

    let mut items_text = String::new();
    for (i, (item, score)) in items.iter().enumerate().take(20) {
        items_text.push_str(&format!(
            "S{}: [{}] {} (match: {:.0}%)\n  URL: {}\n  Content: {}\n\n",
            i + 1,
            item.source_type,
            item.title,
            score * 100.0,
            item.url.as_deref().unwrap_or("N/A"),
            crate::truncate_utf8(&item.content, 300),
        ));
    }

    let previous_section = match previous_render {
        Some(render) => format!(
            "\n\nPrevious version (v{}):\n{}\n\n\
             Focus on what has CHANGED since this version. Note new developments.",
            render.version,
            crate::truncate_utf8(&render.content_markdown, 1500),
        ),
        None => String::new(),
    };

    let user_prompt = format!(
        "User's tech stack: {tech}\n\
         User's active topics: {topics}\n\
         User's learned preferences: {affinities}\n\
         User's system profile: {sovereign}\n\n\
         {count} sources for channel \"{title}\":\n\n\
         {items}{previous}\n\n\
         Generate the intelligence document.",
        tech = tech_stack,
        topics = active_topics,
        affinities = affinity_summary,
        sovereign = sovereign_profile,
        count = items.len().min(20),
        title = channel.title,
        items = items_text,
        previous = previous_section,
    );

    (system_prompt, user_prompt)
}

// ============================================================================
// Provenance Extraction
// ============================================================================

/// Parse source markers like [S1], [S2] from text without regex.
/// Returns a list of 1-based source indices found in the text.
fn parse_source_markers(text: &str) -> Vec<usize> {
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
fn strip_source_markers(text: &str) -> String {
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
fn extract_provenance(
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
// Changelog Computation
// ============================================================================

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
// Fallback Content
// ============================================================================

/// Generate structured source listing when no LLM is available.
pub(crate) fn generate_fallback_content(
    channel: &Channel,
    items: &[(StoredSourceItem, f64)],
) -> String {
    let mut content = format!(
        "# {}\n\n> Source listing -- configure an LLM for prose synthesis\n\n",
        channel.title
    );

    if items.is_empty() {
        content.push_str("No matching sources found. Run an analysis to fetch content.\n");
        return content;
    }

    content.push_str(&format!("## {} Matching Sources\n\n", items.len().min(10)));

    for (i, (item, score)) in items.iter().enumerate().take(10) {
        let url_display = item.url.as_deref().unwrap_or("No URL");
        content.push_str(&format!(
            "{}. **{}** ({})\n   - Source: {} | Match: {:.0}%\n   - {}\n\n",
            i + 1,
            item.title,
            item.source_type,
            item.created_at.format("%Y-%m-%d"),
            score * 100.0,
            url_display,
        ));
    }

    content
}

// ============================================================================
// Main Render Entry Point
// ============================================================================

/// Render (or re-render) a channel. This is the main entry point.
///
/// Flow: load channel -> gather sources -> check LLM -> build prompt -> call LLM
///       -> extract provenance -> save render -> compute changelog
pub(crate) async fn render_channel(channel_id: i64) -> Result<ChannelRender, String> {
    let db = crate::get_database().map_err(|e| e.to_string())?;

    // Load channel
    let channel = db
        .get_channel(channel_id)
        .map_err(|e| format!("Channel not found: {}", e))?;

    // Gather sources (sync, no await)
    let items = gather_channel_sources(db, &channel)?;

    info!(
        target: "4da::channels",
        channel = %channel.slug,
        sources = items.len(),
        "Gathered sources for channel"
    );

    if items.is_empty() {
        let fallback = generate_fallback_content(&channel, &items);
        let render = db
            .save_channel_render(channel_id, &fallback, &[], None, None, None)
            .map_err(|e| e.to_string())?;
        return Ok(render);
    }

    // Check LLM availability -- clone settings out of lock before any await
    let llm_settings = {
        let settings = crate::get_settings_manager().lock();
        settings.get().llm.clone()
    };

    let has_llm = !llm_settings.provider.is_empty()
        && llm_settings.provider != "none"
        && (llm_settings.provider == "ollama" || !llm_settings.api_key.is_empty());

    if !has_llm {
        let fallback = generate_fallback_content(&channel, &items);
        let source_ids: Vec<i64> = items.iter().map(|(item, _)| item.id).collect();
        let render = db
            .save_channel_render(channel_id, &fallback, &source_ids, None, None, None)
            .map_err(|e| e.to_string())?;
        return Ok(render);
    }

    // Build ACE context (sync, no lock issues)
    let ace_ctx = get_ace_context();
    let tech_summary = if ace_ctx.detected_tech.is_empty() {
        "Not detected".to_string()
    } else {
        ace_ctx
            .detected_tech
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    let topics_summary = if ace_ctx.active_topics.is_empty() {
        "None active".to_string()
    } else {
        ace_ctx
            .active_topics
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };

    // Build affinity summary from learned topic preferences (confidence > 0.15, top 5)
    let affinity_summary = {
        let mut affinities: Vec<(&String, &(f32, f32))> = ace_ctx
            .topic_affinities
            .iter()
            .filter(|(_, (_, confidence))| *confidence > 0.15)
            .collect();
        affinities.sort_by(|a, b| {
            b.1 .0
                .abs()
                .partial_cmp(&a.1 .0.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let top: Vec<String> = affinities
            .iter()
            .take(5)
            .map(|(topic, (score, _))| format!("{}: {:.0}%", topic, score * 100.0))
            .collect();
        if top.is_empty() {
            "Not yet learned".to_string()
        } else {
            top.join(", ")
        }
    };

    // Load sovereign profile for hardware/environment context
    let sovereign_summary = {
        let conn = crate::open_db_connection().ok();
        match conn {
            Some(c) => {
                let mut facts = Vec::new();
                if let Ok(mut stmt) = c.prepare(
                    "SELECT category, key, value FROM sovereign_profile ORDER BY category LIMIT 20",
                ) {
                    let _ = stmt
                        .query_map([], |row| {
                            let cat: String = row.get(0)?;
                            let key: String = row.get(1)?;
                            let val: String = row.get(2)?;
                            Ok(format!("{}/{}: {}", cat, key, val))
                        })
                        .map(|rows| {
                            for row in rows.flatten() {
                                facts.push(row);
                            }
                        });
                }
                if facts.is_empty() {
                    "Not available".to_string()
                } else {
                    facts.join(", ")
                }
            }
            None => "Not available".to_string(),
        }
    };

    // Get previous render for diff context (sync)
    let previous_render = db.get_latest_render(channel_id).ok().flatten();

    // Build prompt
    let (system_prompt, user_prompt) = build_channel_prompt(
        &channel,
        &items,
        &tech_summary,
        &topics_summary,
        &sovereign_summary,
        &affinity_summary,
        previous_render.as_ref(),
    );

    // Call LLM (async -- no locks held at this point)
    let llm_client = crate::llm::LLMClient::new(llm_settings.clone());
    let messages = vec![crate::llm::Message {
        role: "user".to_string(),
        content: user_prompt,
    }];
    let start = std::time::Instant::now();

    match llm_client.complete(&system_prompt, messages).await {
        Ok(response) => {
            let elapsed = start.elapsed();
            let total_tokens = (response.input_tokens + response.output_tokens) as i64;

            info!(
                target: "4da::channels",
                channel = %channel.slug,
                tokens = total_tokens,
                elapsed_ms = elapsed.as_millis() as u64,
                "Channel rendered via LLM"
            );

            let source_ids: Vec<i64> = items.iter().map(|(item, _)| item.id).collect();

            // Save render
            let render = db
                .save_channel_render(
                    channel_id,
                    &response.content,
                    &source_ids,
                    Some(&llm_settings.model),
                    Some(total_tokens),
                    Some(elapsed.as_millis() as i64),
                )
                .map_err(|e| e.to_string())?;

            // Extract and save provenance
            let provenance = extract_provenance(render.id, &response.content, &items);
            if !provenance.is_empty() {
                if let Err(e) = db.save_render_provenance(&provenance) {
                    error!(target: "4da::channels", error = %e, "Failed to save provenance");
                }
            }

            Ok(render)
        }
        Err(e) => {
            warn!(
                target: "4da::channels",
                error = %e,
                channel = %channel.slug,
                "LLM render failed, using fallback"
            );
            let fallback = generate_fallback_content(&channel, &items);
            let source_ids: Vec<i64> = items.iter().map(|(item, _)| item.id).collect();
            let render = db
                .save_channel_render(channel_id, &fallback, &source_ids, None, None, None)
                .map_err(|e| e.to_string())?;
            Ok(render)
        }
    }
}

// ============================================================================
// Auto-Render Stale Channels
// ============================================================================

/// Render all channels that are stale or never rendered.
/// Iterates through every active channel, checks freshness, and renders
/// any that need updating. Logs each attempt and continues on failure.
pub(crate) async fn auto_render_stale_channels() -> Result<(), String> {
    let db = crate::get_database().map_err(|e| e.to_string())?;
    let channels = db.list_channels().map_err(|e| e.to_string())?;

    let stale: Vec<_> = channels
        .iter()
        .filter(|ch| {
            matches!(
                ch.freshness,
                crate::channels::ChannelFreshness::NeverRendered
                    | crate::channels::ChannelFreshness::Stale
            )
        })
        .collect();

    info!(
        target: "4da::channels",
        total = channels.len(),
        stale = stale.len(),
        "Auto-rendering stale channels"
    );

    for ch in &stale {
        match render_channel(ch.id).await {
            Ok(render) => {
                info!(
                    target: "4da::channels",
                    channel = %ch.slug,
                    version = render.version,
                    "Auto-rendered channel"
                );
            }
            Err(e) => {
                warn!(
                    target: "4da::channels",
                    channel = %ch.slug,
                    error = %e,
                    "Auto-render failed for channel, continuing"
                );
            }
        }
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::ChannelStatus;

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

    #[test]
    fn test_fallback_content_empty() {
        let channel = Channel {
            id: 1,
            slug: "test".to_string(),
            title: "Test Channel".to_string(),
            description: "A test".to_string(),
            topic_query: vec![],
            status: ChannelStatus::Active,
            source_count: 0,
            render_count: 0,
            last_rendered_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let content = generate_fallback_content(&channel, &[]);
        assert!(content.contains("Test Channel"));
        assert!(content.contains("Source listing"));
        assert!(content.contains("No matching sources"));
    }

    #[test]
    fn test_fallback_content_with_items() {
        use chrono::Utc;

        let channel = Channel {
            id: 1,
            slug: "test".to_string(),
            title: "GPU Intel".to_string(),
            description: "GPU stuff".to_string(),
            topic_query: vec!["gpu".to_string()],
            status: ChannelStatus::Active,
            source_count: 1,
            render_count: 0,
            last_rendered_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        };

        let items: Vec<(StoredSourceItem, f64)> = vec![(
            StoredSourceItem {
                id: 1,
                source_type: "hn".to_string(),
                source_id: "1".to_string(),
                url: Some("https://example.com".to_string()),
                title: "NVIDIA RTX 5090".to_string(),
                content: "New GPU release".to_string(),
                content_hash: "h".to_string(),
                embedding: vec![],
                created_at: Utc::now(),
                last_seen: Utc::now(),
            },
            0.9,
        )];

        let content = generate_fallback_content(&channel, &items);
        assert!(content.contains("GPU Intel"));
        assert!(content.contains("NVIDIA RTX 5090"));
        assert!(content.contains("90%"));
    }
}
