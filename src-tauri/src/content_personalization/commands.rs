//! Tauri commands for the Sovereign Content Engine.

use crate::error::{FourDaError, Result};
use tracing::{info, warn};

use super::cache;
use super::context::{assemble_personalization_context, context_hash};
use super::llm_engine;
use super::nollm_engine;
use super::template_processor;
use super::temporal;
use super::{InsightContent, PersonalizationDepth, PersonalizedLesson};

/// Get a personalized lesson with all 5 levels of personalization applied.
///
/// The pipeline:
/// 1. Load raw markdown from disk
/// 2. Assemble PersonalizationContext from all data sources
/// 3. L1 interpolation + L2 conditionals (sync, <10ms)
/// 4. L3-L5 compute insight cards, mirrors, temporal blocks
/// 5. Return PersonalizedLesson for frontend rendering
#[tauri::command]
pub async fn get_personalized_lesson(
    module_id: String,
    lesson_idx: u32,
) -> Result<PersonalizedLesson> {
    let start = std::time::Instant::now();

    // Step 1: Load raw lesson content using existing playbook system
    let content = crate::playbook_commands::get_playbook_content(module_id.clone(), None)
        .map_err(FourDaError::Internal)?;

    let lesson = content.lessons.get(lesson_idx as usize).ok_or_else(|| {
        FourDaError::Internal(format!(
            "Lesson {} not found in module {}",
            lesson_idx, module_id
        ))
    })?;

    // Step 2: Assemble context (sync — all local data)
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;
    let ctx = assemble_personalization_context(&conn);
    let hash = context_hash(&ctx);

    // Step 3: L1/L2 template processing (sync, fast)
    let processed = template_processor::process_template(&lesson.content, &ctx);

    // Step 4: L3 insight cards from injection markers
    let insight_blocks = nollm_engine::compute_insight_blocks(
        &processed.injection_markers,
        &ctx,
        &module_id,
        lesson_idx,
    );

    // Step 5: L4 mirror/connection blocks
    let mirror_blocks = nollm_engine::compute_mirror_blocks(&ctx, &module_id, lesson_idx);

    // Step 6: L5 temporal blocks (diff ribbons, feed echoes, progressive reveals)
    let temporal_blocks = temporal::compute_temporal_blocks(&conn, &ctx, &module_id, lesson_idx);

    // Step 7: Update read state for next visit's temporal diff
    temporal::update_read_state(&conn, &ctx, &module_id, lesson_idx);

    let depth = PersonalizationDepth {
        l1_resolved: processed.l1_resolved,
        l1_fallbacks: processed.l1_fallbacks,
        l2_evaluated: processed.l2_evaluated,
        l3_cards: insight_blocks.len() as u32,
        l4_connections: mirror_blocks.len() as u32,
        l5_temporal: temporal_blocks.len() as u32,
        llm_pending: ctx.settings.has_llm,
    };

    let elapsed = start.elapsed();
    info!(
        target: "4da::personalize",
        module = %module_id,
        lesson = lesson_idx,
        l1 = depth.l1_resolved,
        l2 = depth.l2_evaluated,
        l3 = depth.l3_cards,
        l4 = depth.l4_connections,
        l5 = depth.l5_temporal,
        elapsed_ms = elapsed.as_millis() as u64,
        "Personalized lesson ready"
    );

    Ok(PersonalizedLesson {
        content: processed.content,
        insight_blocks,
        mirror_blocks,
        temporal_blocks,
        depth,
        context_hash: hash,
    })
}

/// Get multiple personalized lessons in a single IPC call.
///
/// Shares one DB connection and one PersonalizationContext assembly across all
/// requested lessons, eliminating the N+1 overhead of calling
/// `get_personalized_lesson` individually for each lesson in a module.
#[tauri::command]
pub async fn get_personalized_lessons_batch(
    requests: Vec<(String, u32)>,
) -> Result<Vec<PersonalizedLesson>> {
    let start = std::time::Instant::now();

    // Shared connection + context (assembled once)
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;
    let ctx = assemble_personalization_context(&conn);
    let hash = context_hash(&ctx);

    let mut results = Vec::with_capacity(requests.len());

    for (module_id, lesson_idx) in &requests {
        let content = crate::playbook_commands::get_playbook_content(module_id.clone(), None)
            .map_err(FourDaError::Internal)?;

        let lesson = match content.lessons.get(*lesson_idx as usize) {
            Some(l) => l,
            None => {
                warn!(
                    target: "4da::personalize",
                    module = %module_id,
                    lesson = lesson_idx,
                    "Lesson not found in batch, skipping"
                );
                continue;
            }
        };

        let processed = template_processor::process_template(&lesson.content, &ctx);
        let insight_blocks = nollm_engine::compute_insight_blocks(
            &processed.injection_markers,
            &ctx,
            module_id,
            *lesson_idx,
        );
        let mirror_blocks = nollm_engine::compute_mirror_blocks(&ctx, module_id, *lesson_idx);
        let temporal_blocks =
            temporal::compute_temporal_blocks(&conn, &ctx, module_id, *lesson_idx);
        temporal::update_read_state(&conn, &ctx, module_id, *lesson_idx);

        let depth = PersonalizationDepth {
            l1_resolved: processed.l1_resolved,
            l1_fallbacks: processed.l1_fallbacks,
            l2_evaluated: processed.l2_evaluated,
            l3_cards: insight_blocks.len() as u32,
            l4_connections: mirror_blocks.len() as u32,
            l5_temporal: temporal_blocks.len() as u32,
            llm_pending: ctx.settings.has_llm,
        };

        results.push(PersonalizedLesson {
            content: processed.content,
            insight_blocks,
            mirror_blocks,
            temporal_blocks,
            depth,
            context_hash: hash.clone(),
        });
    }

    let elapsed = start.elapsed();
    info!(
        target: "4da::personalize",
        count = results.len(),
        elapsed_ms = elapsed.as_millis() as u64,
        "Batch personalization complete"
    );

    Ok(results)
}

/// Get a lightweight summary of the current personalization context.
/// Used by the frontend to check data availability before rendering.
#[tauri::command]
pub async fn get_personalization_context_summary() -> Result<serde_json::Value> {
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;
    let ctx = assemble_personalization_context(&conn);

    Ok(serde_json::json!({
        "profile_completeness": ctx.computed.profile_completeness,
        "has_llm": ctx.settings.has_llm,
        "llm_tier": ctx.computed.llm_tier,
        "gpu_tier": ctx.computed.gpu_tier,
        "os_family": ctx.computed.os_family,
        "stack_count": ctx.stack.primary.len(),
        "radar_adopt_count": ctx.radar.adopt.len(),
        "completed_modules": ctx.progress.completed_modules,
        "completed_lessons": ctx.progress.completed_lesson_count,
        "regional_country": ctx.regional.country,
        "dna_available": ctx.dna.is_full,
        "context_hash": context_hash(&ctx),
    }))
}

/// Prune stale cache entries. Called on app startup and periodically.
#[tauri::command]
pub async fn prune_personalization_cache() -> Result<serde_json::Value> {
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;
    let deleted = cache::prune_cache(&conn);
    let stats = cache::cache_stats(&conn);
    Ok(serde_json::json!({
        "deleted": deleted,
        "remaining": stats.cache_entries,
        "read_states": stats.read_state_entries,
        "cache_size_bytes": stats.cache_size_bytes,
    }))
}

/// Asynchronously upgrade insight blocks with LLM prose.
///
/// Called by the frontend AFTER the no-LLM response renders. Iterates over the
/// lesson's insight blocks, generates LLM prose for each, and emits a Tauri event
/// (`personalization-llm-upgrade`) for each upgrade so the frontend can hydrate.
#[tauri::command]
pub async fn hydrate_lesson_with_llm(
    app: tauri::AppHandle,
    module_id: String,
    lesson_idx: u32,
) -> Result<serde_json::Value> {
    use tauri::Emitter;

    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;
    let ctx = assemble_personalization_context(&conn);

    if !ctx.settings.has_llm {
        return Ok(serde_json::json!({ "upgraded": 0, "reason": "no_llm" }));
    }

    // Re-compute the no-LLM insight blocks so we know what to upgrade
    let content = crate::playbook_commands::get_playbook_content(module_id.clone(), None)
        .map_err(FourDaError::Internal)?;

    let lesson = content.lessons.get(lesson_idx as usize).ok_or_else(|| {
        FourDaError::Internal(format!(
            "Lesson {} not found in module {}",
            lesson_idx, module_id
        ))
    })?;

    let processed = template_processor::process_template(&lesson.content, &ctx);
    let insight_blocks = nollm_engine::compute_insight_blocks(
        &processed.injection_markers,
        &ctx,
        &module_id,
        lesson_idx,
    );

    let mut upgraded = 0u32;

    for block in &insight_blocks {
        // Extract the card from the block content
        let card = match &block.content {
            InsightContent::Card(c) => c,
            InsightContent::Prose { .. } => continue, // Already prose
        };

        // Determine session type from card type
        let session_type = match block.source_labels.first().map(|s| s.as_str()) {
            Some("mirror") => "mirror",
            Some("recommendation") => "recommendation",
            _ => "insight",
        };

        match llm_engine::generate_insight_prose(card, &ctx, session_type).await {
            Some(prose) => {
                // Emit event for frontend hydration
                let payload = serde_json::json!({
                    "module_id": module_id,
                    "lesson_idx": lesson_idx,
                    "block_id": block.block_id,
                    "content": prose,
                });
                if let Err(e) = app.emit("personalization-llm-upgrade", &payload) {
                    warn!(
                        target: "4da::personalize",
                        error = %e,
                        block_id = %block.block_id,
                        "Failed to emit LLM upgrade event"
                    );
                }
                upgraded += 1;
            }
            None => {
                warn!(
                    target: "4da::personalize",
                    block_id = %block.block_id,
                    "LLM prose generation returned None"
                );
            }
        }
    }

    info!(
        target: "4da::personalize",
        module = %module_id,
        lesson = lesson_idx,
        upgraded,
        total = insight_blocks.len(),
        "LLM hydration complete"
    );

    Ok(serde_json::json!({
        "upgraded": upgraded,
        "total_blocks": insight_blocks.len(),
    }))
}
