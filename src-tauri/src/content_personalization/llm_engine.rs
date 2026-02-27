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
             the developer's different data sources. Show connections they haven't noticed."
        }
        "recommendation" => {
            "You are a strategic advisor. Generate a specific, actionable recommendation \
             based on the developer's profile. Reference their actual tech stack, hardware, \
             and progress. 3-4 sentences max."
        }
        _ => "You are a developer advisor. Generate a concise, personalized insight.",
    };

    format!(
        "{}\n\n## Developer Context\n```json\n{}\n```\n\n## Card Data\nType: {:?}\nTitle: {}\nData Points:\n{}",
        role,
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
pub async fn generate_insight_prose(
    card: &SovereignInsightCard,
    ctx: &PersonalizationContext,
    session_type: &str,
) -> Option<InsightContent> {
    let client = crate::coach_context::get_llm_client().ok()?;
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
            let (total_tokens, cost_cents) =
                crate::coach_context::record_llm_usage(&client, &response);

            info!(
                target: "4da::personalize",
                tokens = total_tokens,
                cost = cost_cents,
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
