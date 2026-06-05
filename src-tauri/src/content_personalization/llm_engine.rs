// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! LLM Prose Engine — upgrades no-LLM cards with rich personalized prose.
//!
//! When an LLM is configured, generates contextual prose for insight blocks
//! and emits Tauri events for frontend hydration. Respects existing cost limits.

use tracing::{info, warn};

use super::context::PersonalizationContext;
use super::{InsightContent, SovereignInsightCard};

// ============================================================================
// Prompt Construction
// ============================================================================

/// Build a system prompt for personalized insight generation.
pub fn build_insight_prompt(
    card: &SovereignInsightCard,
    ctx: &PersonalizationContext,
    session_type: &str,
) -> String {
    let ctx_json = serde_json::to_string_pretty(ctx).unwrap_or_else(|_| "{}".into());

    let role = match session_type {
        "insight" => {
            "You are a concise technical advisor. Generate a 2-3 sentence personalized insight \
             based on the data card and developer profile below. Be specific to THIS developer's \
             actual hardware, stack, and situation. No generic advice."
        }
        "mirror" => {
            "You are a pattern analyst. Generate a 2-3 sentence cross-data insight connecting \
             the developer's different data sources. Surface only connections that are actually \
             present in the data below -- do not invent correlations to seem insightful."
        }
        "recommendation" => {
            "You are a strategic advisor. Generate a specific, actionable recommendation \
             based on the developer's profile. Reference their actual tech stack, hardware, \
             and progress. 3-4 sentences max."
        }
        _ => "You are a developer advisor. Generate a concise, personalized insight.",
    };

    // Grounding rule (shared by every role): the personalization mandate above must
    // not become license to fabricate. Every claim has to trace to the data below, and
    // impact may only be asserted for technology that actually appears in the context.
    // Prevents the false-attribution class (e.g. welding an unrelated package onto the
    // user's stack). See the brief-grounding immune pass.
    const GROUNDING: &str = "\n\nGrounding (do not violate): every claim must trace to a Data Point \
        or a field present in the Developer Context. Do not assert impact on any technology, hardware, \
        or project that does not appear there, and do not cross ecosystems (an npm package does not \
        affect a Rust backend). Invent no numbers, connections, or urgency. If the data does not support \
        a specific insight, give a brief factual observation instead of a fabricated one.";

    format!(
        "{}{}\n\n## Developer Context\n```json\n{}\n```\n\n## Card Data\nType: {:?}\nTitle: {}\nData Points:\n{}",
        role,
        GROUNDING,
        ctx_json,
        card.card_type,
        card.title,
        card.data_points
            .iter()
            .map(|dp| format!("- {}: {}", dp.label, dp.value))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// Generate LLM prose for an insight card. Returns None if LLM is unavailable.
///
/// This function is designed to be called asynchronously AFTER the no-LLM
/// response has been returned to the frontend. The frontend subscribes to
/// Tauri events for the upgraded prose.
/// Get an LLM client from current settings, if configured.
fn get_llm_client() -> Option<crate::llm::LLMClient> {
    let manager = crate::get_settings_manager();
    let mut guard = manager.lock();
    guard.ensure_keys_hydrated();
    let provider = guard.get().llm.clone();
    if provider.api_key.is_empty() && provider.provider != "ollama" {
        return None;
    }
    Some(crate::llm::LLMClient::new(provider))
}

pub async fn generate_insight_prose(
    card: &SovereignInsightCard,
    ctx: &PersonalizationContext,
    session_type: &str,
) -> Option<InsightContent> {
    let client = get_llm_client()?;
    let system_prompt = build_insight_prompt(card, ctx, session_type);

    let user_message = format!(
        "Generate a personalized insight for the '{}' card. \
         Be specific to my actual data — no generic advice.",
        card.title
    );

    let messages = vec![crate::llm::Message {
        role: "user".into(),
        content: user_message,
    }];

    match client.complete(&system_prompt, messages).await {
        Ok(response) => {
            let total_tokens = response.input_tokens + response.output_tokens;

            info!(
                target: "4da::personalize",
                tokens = total_tokens,
                "LLM insight generated"
            );

            Some(InsightContent::Prose {
                text: response.content,
                model: session_type.to_string(),
            })
        }
        Err(e) => {
            warn!(target: "4da::personalize", error = %e, "LLM insight generation failed");
            None
        }
    }
}
