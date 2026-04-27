// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Channel render pipeline — source gathering, LLM synthesis, fallback content.
//!
//! This module orchestrates the full render cycle for an Information Channel:
//! 1. Gather source items matching the channel's topic query
//! 2. Build system/user prompts for LLM synthesis
//! 3. Call LLM (or generate fallback if unavailable)
//! 4. Extract provenance (delegated to `channel_provenance`)
//!
//! Changelog computation lives in `channel_changelog`.
//! Provenance extraction lives in `channel_provenance`.

use tracing::{error, info, warn};

use crate::channel_provenance::extract_provenance;
use crate::channels::{Channel, ChannelRender};
use crate::db::{Database, StoredSourceItem};
use crate::error::{Result, ResultExt};
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
) -> Result<Vec<(StoredSourceItem, f64)>> {
    let channel_topics: Vec<String> = channel
        .topic_query
        .iter()
        .map(|t| t.to_lowercase())
        .collect();

    if channel_topics.is_empty() {
        return Ok(vec![]);
    }

    // Get recent source items (last 30 days, up to 500)
    let items = db.get_items_tiered(30 * 24, 500)?;

    let mut scored: Vec<(StoredSourceItem, f64)> = Vec::new();
    let ace_ctx = get_ace_context();

    for item in items {
        let item_topics = extract_topics(&item.title, &item.content, &[]);
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
            let base_score = matched as f64 / channel_topics.len().max(1) as f64;
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
) -> Result<(usize, Vec<String>)> {
    let topics_lower: Vec<String> = topics.iter().map(|t| t.to_lowercase()).collect();
    if topics_lower.is_empty() {
        return Ok((0, vec![]));
    }

    let items = db.get_items_tiered(30 * 24, 500)?;

    let mut matched_titles: Vec<(String, f64)> = Vec::new();

    for item in items {
        let item_topics = extract_topics(&item.title, &item.content, &[]);
        let item_topics_lower: Vec<String> = item_topics.iter().map(|t| t.to_lowercase()).collect();

        let hit_count = topics_lower
            .iter()
            .filter(|ct| {
                item_topics_lower.iter().any(|it| it.contains(ct.as_str()))
                    || item.title.to_lowercase().contains(ct.as_str())
            })
            .count();

        if hit_count > 0 {
            let score = hit_count as f64 / topics_lower.len().max(1) as f64;
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
pub(crate) async fn render_channel(channel_id: i64) -> Result<ChannelRender> {
    let db = crate::get_database()?;

    // Load channel
    let channel = db.get_channel(channel_id).context("Channel not found")?;

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
        let render = db.save_channel_render(channel_id, &fallback, &[], None, None, None)?;
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
        let render =
            db.save_channel_render(channel_id, &fallback, &source_ids, None, None, None)?;
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
                            Ok(format!("{cat}/{key}: {val}"))
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
            let render = db.save_channel_render(
                channel_id,
                &response.content,
                &source_ids,
                Some(&llm_settings.model),
                Some(total_tokens),
                Some(elapsed.as_millis() as i64),
            )?;

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
            let render =
                db.save_channel_render(channel_id, &fallback, &source_ids, None, None, None)?;
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
pub(crate) async fn auto_render_stale_channels() -> Result<()> {
    let db = crate::get_database()?;
    let channels = db.list_channels()?;

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
                detected_lang: "en".to_string(),
                feed_origin: None,
                tags: None,
            },
            0.9,
        )];

        let content = generate_fallback_content(&channel, &items);
        assert!(content.contains("GPU Intel"));
        assert!(content.contains("NVIDIA RTX 5090"));
        assert!(content.contains("90%"));
    }
}
