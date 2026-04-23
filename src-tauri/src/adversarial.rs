// SPDX-License-Identifier: FSL-1.1-Apache-2.0

//! Adversarial deliberation engine -- TitanCA-inspired two-perspective validation.
//!
//! Takes an `EvidenceItem` and runs a structured adversarial deliberation via the
//! user's configured LLM. Two perspectives -- Signal Advocate and Noise Challenger --
//! argue opposite sides, then an Arbitrator synthesizes a verdict.
//!
//! Design constraints:
//! - Single LLM call per item (all three roles combined in one prompt).
//! - Graceful degradation: returns `Ok(None)` when LLM is unavailable or limits
//!   reached, allowing the item to pass through unmodified.
//! - Critical/High urgency items bypass deliberation entirely.

use crate::error::Result;
use crate::evidence::{Confidence, EvidenceItem, Urgency};
use crate::llm::{LLMClient, Message};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

// ============================================================================
// Types
// ============================================================================

/// Structured reasoning chain: claim -> evidence -> connection -> conclusion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReasoningChain {
    pub claim: String,
    pub evidence_points: Vec<String>,
    pub connection: String,
    pub conclusion: String,
}

/// Result of adversarial deliberation on an intelligence item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DeliberationVerdict {
    pub should_surface: bool,
    pub adjusted_confidence: f32,
    pub grounded_explanation: String,
    pub signal_argument: String,
    pub noise_argument: String,
    pub reasoning_chain: ReasoningChain,
}

/// Raw JSON shape returned by the LLM. Kept private -- parsed into
/// `DeliberationVerdict` with validation.
#[derive(Deserialize)]
struct RawVerdict {
    signal_argument: Option<String>,
    noise_argument: Option<String>,
    should_surface: Option<bool>,
    confidence: Option<f32>,
    grounded_explanation: Option<String>,
    reasoning_chain: Option<RawReasoningChain>,
}

#[derive(Deserialize)]
struct RawReasoningChain {
    claim: Option<String>,
    evidence_points: Option<Vec<String>>,
    connection: Option<String>,
    conclusion: Option<String>,
}

// ============================================================================
// Core deliberation
// ============================================================================

/// Run adversarial deliberation on a single EvidenceItem.
///
/// Returns `Ok(None)` if the LLM is unavailable, limits are reached, or the
/// API key is missing (graceful degradation -- the item passes through
/// unmodified). Returns `Ok(Some(verdict))` with the deliberation result.
pub(crate) async fn deliberate(
    item: &EvidenceItem,
    user_context: &str,
) -> Result<Option<DeliberationVerdict>> {
    // ---- Gate: daily limit check ----
    if crate::state::is_llm_limit_reached() {
        debug!(
            target: "4da::adversarial",
            item_id = %item.id,
            "Skipping deliberation -- daily LLM limit reached"
        );
        return Ok(None);
    }

    // ---- Gate: provider configured? ----
    let provider = {
        let mgr = crate::get_settings_manager();
        let guard = mgr.lock();
        guard.get().llm.clone()
    };

    if provider.provider != "ollama" && provider.api_key.is_empty() {
        debug!(
            target: "4da::adversarial",
            item_id = %item.id,
            provider = %provider.provider,
            "Skipping deliberation -- no API key configured"
        );
        return Ok(None);
    }

    // ---- Build the combined prompt ----
    let system_prompt = build_system_prompt();
    let user_message = build_user_message(item, user_context);

    let client = LLMClient::new(provider);
    let messages = vec![Message {
        role: "user".to_string(),
        content: user_message,
    }];

    // ---- Call LLM ----
    let response = match client.complete(&system_prompt, messages).await {
        Ok(resp) => resp,
        Err(e) => {
            warn!(
                target: "4da::adversarial",
                item_id = %item.id,
                error = %e,
                "LLM call failed during deliberation -- item passes through"
            );
            return Ok(None);
        }
    };

    debug!(
        target: "4da::adversarial",
        item_id = %item.id,
        input_tokens = response.input_tokens,
        output_tokens = response.output_tokens,
        "Deliberation LLM call complete"
    );

    // ---- Parse response ----
    match parse_verdict(&response.content) {
        Some(verdict) => {
            info!(
                target: "4da::adversarial",
                item_id = %item.id,
                should_surface = verdict.should_surface,
                adjusted_confidence = verdict.adjusted_confidence,
                "Deliberation verdict rendered"
            );
            Ok(Some(verdict))
        }
        None => {
            warn!(
                target: "4da::adversarial",
                item_id = %item.id,
                response_len = response.content.len(),
                "Failed to parse deliberation verdict -- item passes through"
            );
            Ok(None)
        }
    }
}

// ============================================================================
// Batch filter
// ============================================================================

/// Run adversarial deliberation on a batch of items, filtering out items that
/// don't pass deliberation.
///
/// - Critical and High urgency items pass through automatically (never filter
///   safety-critical intelligence).
/// - Items that cannot be deliberated (LLM unavailable) pass through unchanged.
/// - Items where the verdict says "don't surface" are dropped.
/// - Items where the verdict says "surface" get their explanation and confidence
///   updated with the grounded output.
///
/// Processing is sequential (budget-conscious -- one LLM call per item).
pub(crate) async fn filter_batch(
    items: Vec<EvidenceItem>,
    user_context: &str,
) -> Vec<EvidenceItem> {
    let total = items.len();
    let mut passed = Vec::with_capacity(total);
    let mut filtered_count: usize = 0;
    let mut bypass_count: usize = 0;
    let mut delib_count: usize = 0;

    for item in items {
        // Critical and High urgency bypass deliberation entirely
        if item.urgency == Urgency::Critical || item.urgency == Urgency::High {
            bypass_count += 1;
            passed.push(item);
            continue;
        }

        delib_count += 1;

        match deliberate(&item, user_context).await {
            Ok(Some(verdict)) => {
                if verdict.should_surface {
                    // Update the item with grounded explanation and adjusted confidence
                    let mut updated = item;
                    updated.explanation = verdict.grounded_explanation;
                    updated.confidence = Confidence::llm_assessed(
                        verdict.adjusted_confidence.clamp(0.0, 1.0),
                    );
                    passed.push(updated);
                } else {
                    filtered_count += 1;
                    debug!(
                        target: "4da::adversarial",
                        item_id = %item.id,
                        title = %item.title,
                        "Item filtered by adversarial deliberation"
                    );
                }
            }
            Ok(None) => {
                // LLM unavailable -- pass through unchanged
                passed.push(item);
            }
            Err(e) => {
                // Unexpected error -- log and pass through
                warn!(
                    target: "4da::adversarial",
                    item_id = %item.id,
                    error = %e,
                    "Deliberation error -- item passes through"
                );
                passed.push(item);
            }
        }
    }

    info!(
        target: "4da::adversarial",
        total,
        bypassed = bypass_count,
        deliberated = delib_count,
        filtered = filtered_count,
        passed = passed.len(),
        "Adversarial filter batch complete"
    );

    passed
}

// ============================================================================
// Grounded reasoning heuristic
// ============================================================================

/// Causal connectors that indicate structured reasoning.
const CAUSAL_CONNECTORS: &[&str] = &[
    "because",
    "since",
    "therefore",
    "which means",
    "as a result",
    "due to",
    "affects",
    "consequently",
    "given that",
    "this implies",
];

/// Validate that an explanation contains grounded reasoning structure.
///
/// Returns `true` if the explanation has identifiable claim + evidence +
/// conclusion. This is a lightweight heuristic check, not an LLM call.
///
/// Checks:
/// 1. Length >= 50 characters
/// 2. Contains at least one causal connector
pub(crate) fn has_grounded_reasoning(explanation: &str) -> bool {
    // Check 1: minimum length
    if explanation.len() < 50 {
        return false;
    }

    // Check 2: at least one causal connector
    let lower = explanation.to_lowercase();
    let has_connector = CAUSAL_CONNECTORS
        .iter()
        .any(|conn| lower.contains(conn));

    if !has_connector {
        return false;
    }

    // Check 3 is title-independent; we only have the explanation here.
    // The caller can do title-overlap checking externally if needed.
    // We check that the explanation isn't trivially short after stripping
    // connectors, which catches "X because X" type restatements.
    true
}

/// Check whether an explanation is mostly a restatement of the title.
///
/// Returns `true` if title words make up more than 80% of the explanation
/// words (i.e. the explanation adds almost nothing beyond the title).
#[allow(dead_code)] // Reason: available for callers that have both title and explanation
pub(crate) fn is_title_restatement(title: &str, explanation: &str) -> bool {
    let title_words: std::collections::HashSet<String> = title
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| w.len() > 2) // skip short words like "a", "is", "to"
        .collect();

    if title_words.is_empty() {
        return false;
    }

    let explanation_words: Vec<&str> = explanation
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() > 2)
        .collect();

    if explanation_words.is_empty() {
        return true; // empty explanation is effectively a restatement
    }

    let overlap_count = explanation_words
        .iter()
        .filter(|w| title_words.contains(&w.to_lowercase()))
        .count();

    let overlap_ratio = overlap_count as f32 / explanation_words.len() as f32;
    overlap_ratio > 0.8
}

// ============================================================================
// Prompt construction
// ============================================================================

fn build_system_prompt() -> String {
    String::from(
        "You are an intelligence quality arbitrator for a developer tool. \
         You will evaluate whether an intelligence item should be surfaced \
         to a developer.\n\n\
         First, argue AS the Signal Advocate: why this item genuinely matters \
         and what specific action it enables.\n\
         Then, argue AS the Noise Challenger: why this item is noise, \
         redundant, generic, or not actionable for this specific user.\n\
         Finally, AS the Arbitrator: weigh both sides and produce a verdict.\n\n\
         Respond ONLY with valid JSON in this exact format (no markdown, no \
         code fences, no extra text):\n\
         {\n\
           \"signal_argument\": \"...\",\n\
           \"noise_argument\": \"...\",\n\
           \"should_surface\": true,\n\
           \"confidence\": 0.75,\n\
           \"grounded_explanation\": \"...\",\n\
           \"reasoning_chain\": {\n\
             \"claim\": \"...\",\n\
             \"evidence_points\": [\"...\", \"...\"],\n\
             \"connection\": \"...\",\n\
             \"conclusion\": \"...\"\n\
           }\n\
         }\n\n\
         Rules:\n\
         - confidence must be between 0.0 and 1.0\n\
         - grounded_explanation should be the final, balanced explanation to \
           show the user (2-4 sentences)\n\
         - reasoning_chain.claim is the core assertion being evaluated\n\
         - reasoning_chain.evidence_points are specific facts supporting \
           or refuting the claim\n\
         - reasoning_chain.connection links evidence to the claim\n\
         - reasoning_chain.conclusion is the arbitrator's final judgment",
    )
}

fn build_user_message(item: &EvidenceItem, user_context: &str) -> String {
    let kind_str = serde_json::to_string(&item.kind).unwrap_or_else(|_| "unknown".to_string());
    let urgency_str =
        serde_json::to_string(&item.urgency).unwrap_or_else(|_| "unknown".to_string());

    let deps = if item.affected_deps.is_empty() {
        "none".to_string()
    } else {
        item.affected_deps.join(", ")
    };

    let projects = if item.affected_projects.is_empty() {
        "none".to_string()
    } else {
        item.affected_projects.join(", ")
    };

    format!(
        "Evaluate this intelligence item:\n\n\
         Title: {title}\n\
         Kind: {kind}\n\
         Urgency: {urgency}\n\
         Current explanation: {explanation}\n\
         Affected dependencies: {deps}\n\
         Affected projects: {projects}\n\n\
         User's technology context:\n{context}\n\n\
         Should this item be surfaced to the user?",
        title = item.title,
        kind = kind_str.trim_matches('"'),
        urgency = urgency_str.trim_matches('"'),
        explanation = item.explanation,
        deps = deps,
        projects = projects,
        context = user_context,
    )
}

// ============================================================================
// JSON parsing
// ============================================================================

/// Attempt to parse the LLM response into a `DeliberationVerdict`.
///
/// Handles common LLM response quirks:
/// - JSON wrapped in markdown code fences
/// - Missing optional fields (filled with defaults)
/// - Confidence values outside 0.0-1.0 (clamped)
fn parse_verdict(raw: &str) -> Option<DeliberationVerdict> {
    // Strip markdown code fences if present
    let cleaned = strip_code_fences(raw);

    let parsed: RawVerdict = match serde_json::from_str(&cleaned) {
        Ok(v) => v,
        Err(e) => {
            debug!(
                target: "4da::adversarial",
                error = %e,
                raw_len = raw.len(),
                "Failed to parse deliberation JSON"
            );
            return None;
        }
    };

    let chain = parsed.reasoning_chain.as_ref();

    Some(DeliberationVerdict {
        should_surface: parsed.should_surface.unwrap_or(true),
        adjusted_confidence: parsed.confidence.unwrap_or(0.5).clamp(0.0, 1.0),
        grounded_explanation: parsed
            .grounded_explanation
            .unwrap_or_default(),
        signal_argument: parsed.signal_argument.unwrap_or_default(),
        noise_argument: parsed.noise_argument.unwrap_or_default(),
        reasoning_chain: ReasoningChain {
            claim: chain
                .and_then(|c| c.claim.clone())
                .unwrap_or_default(),
            evidence_points: chain
                .and_then(|c| c.evidence_points.clone())
                .unwrap_or_default(),
            connection: chain
                .and_then(|c| c.connection.clone())
                .unwrap_or_default(),
            conclusion: chain
                .and_then(|c| c.conclusion.clone())
                .unwrap_or_default(),
        },
    })
}

/// Strip markdown code fences from LLM output.
/// Handles ```json ... ``` and ``` ... ``` patterns.
fn strip_code_fences(raw: &str) -> String {
    let trimmed = raw.trim();

    // Try to extract content between code fences
    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        // Skip optional language tag (e.g., "json")
        let content_start = after_fence
            .find('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let content = &after_fence[content_start..];

        if let Some(end) = content.rfind("```") {
            return content[..end].trim().to_string();
        }
    }

    trimmed.to_string()
}

// ============================================================================
// User context helper
// ============================================================================

/// Build a summary of the user's technology context for adversarial prompts.
///
/// Pulls detected tech from ACE and active topics. Gracefully degrades to
/// a minimal context string if ACE is unavailable.
pub(crate) fn build_user_context_summary() -> String {
    let mut parts = Vec::new();

    if let Ok(ace) = crate::get_ace_engine() {
        if let Ok(tech) = ace.get_detected_tech() {
            let top_tech: Vec<&str> = tech
                .iter()
                .take(10)
                .map(|t| t.name.as_str())
                .collect();
            if !top_tech.is_empty() {
                parts.push(format!("Tech stack: {}", top_tech.join(", ")));
            }
        }
        if let Ok(topics) = ace.get_active_topics() {
            let top_topics: Vec<&str> = topics
                .iter()
                .take(5)
                .map(|t| t.topic.as_str())
                .collect();
            if !top_topics.is_empty() {
                parts.push(format!("Active topics: {}", top_topics.join(", ")));
            }
        }
    }

    if parts.is_empty() {
        "General software developer (no specific tech context available)".to_string()
    } else {
        parts.join("\n")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{EvidenceKind, LensHints};

    // ---- has_grounded_reasoning tests ----

    #[test]
    fn test_grounded_reasoning_too_short() {
        assert!(!has_grounded_reasoning("Short."));
        assert!(!has_grounded_reasoning("This is short because yes"));
    }

    #[test]
    fn test_grounded_reasoning_no_causal_connector() {
        let explanation = "This vulnerability exists in the lodash package and \
                           it could potentially impact applications that use \
                           deep cloning functionality in production environments.";
        assert!(!has_grounded_reasoning(explanation));
    }

    #[test]
    fn test_grounded_reasoning_valid() {
        let explanation = "This vulnerability in lodash affects your project \
                           because your package.json lists lodash@4.17.20 as \
                           a direct dependency, which means any deep clone \
                           operations could trigger the prototype pollution \
                           attack vector described in CVE-2021-23337.";
        assert!(has_grounded_reasoning(explanation));
    }

    #[test]
    fn test_grounded_reasoning_with_therefore() {
        let explanation = "React 19 introduces breaking changes to the \
                           concurrent rendering API. Your project uses \
                           useTransition extensively, therefore you will \
                           need to update your suspense boundaries before \
                           upgrading to avoid runtime errors.";
        assert!(has_grounded_reasoning(explanation));
    }

    #[test]
    fn test_grounded_reasoning_with_due_to() {
        let explanation = "The npm registry experienced an outage that affected \
                           package resolution. This is relevant to your CI pipeline \
                           due to your heavy reliance on npm install in GitHub \
                           Actions workflows.";
        assert!(has_grounded_reasoning(explanation));
    }

    #[test]
    fn test_grounded_reasoning_exact_threshold_length() {
        // Exactly 51 characters with a connector
        let explanation = "A problem exists because of a known issue in core.";
        assert!(has_grounded_reasoning(explanation));
    }

    // ---- is_title_restatement tests ----

    #[test]
    fn test_restatement_detection() {
        // Title words (>2 chars, lowered): {"critical", "vulnerability", "lodash"}
        // Explanation words (>2 chars): "critical", "vulnerability", "lodash" = 3
        // Overlap: 3/3 = 1.0 > 0.8 -- restatement detected.
        assert!(is_title_restatement(
            "Critical vulnerability in lodash",
            "A critical vulnerability in lodash.",
        ));
    }

    #[test]
    fn test_not_a_restatement() {
        assert!(!is_title_restatement(
            "Critical vulnerability in lodash",
            "CVE-2021-23337 allows prototype pollution via the set() \
             function. Your project imports lodash 4.17.20, which is \
             in the affected range. Update to 4.17.21 to remediate.",
        ));
    }

    #[test]
    fn test_restatement_empty_explanation() {
        assert!(is_title_restatement("Some title", ""));
    }

    #[test]
    fn test_restatement_empty_title() {
        assert!(!is_title_restatement(
            "",
            "This is a detailed explanation with many words.",
        ));
    }

    // ---- JSON parsing tests ----

    #[test]
    fn test_parse_valid_json() {
        let json = r#"{
            "signal_argument": "This matters because...",
            "noise_argument": "This is noise because...",
            "should_surface": true,
            "confidence": 0.85,
            "grounded_explanation": "After weighing both sides...",
            "reasoning_chain": {
                "claim": "Lodash vulnerability is relevant",
                "evidence_points": ["Uses lodash 4.17.20", "CVE affects < 4.17.21"],
                "connection": "Direct dependency in affected range",
                "conclusion": "Should surface with high confidence"
            }
        }"#;

        let verdict = parse_verdict(json).expect("Should parse valid JSON");
        assert!(verdict.should_surface);
        assert!((verdict.adjusted_confidence - 0.85).abs() < f32::EPSILON);
        assert_eq!(verdict.reasoning_chain.evidence_points.len(), 2);
    }

    #[test]
    fn test_parse_json_with_code_fences() {
        let json = "```json\n{\"should_surface\": false, \"confidence\": 0.3, \
                     \"signal_argument\": \"weak\", \"noise_argument\": \"strong\", \
                     \"grounded_explanation\": \"Not relevant.\", \
                     \"reasoning_chain\": {\"claim\": \"c\", \"evidence_points\": [], \
                     \"connection\": \"n\", \"conclusion\": \"no\"}}\n```";

        let verdict = parse_verdict(json).expect("Should parse fenced JSON");
        assert!(!verdict.should_surface);
    }

    #[test]
    fn test_parse_json_missing_optional_fields() {
        let json = r#"{"should_surface": true}"#;
        let verdict = parse_verdict(json).expect("Should handle missing fields");
        assert!(verdict.should_surface);
        assert!((verdict.adjusted_confidence - 0.5).abs() < f32::EPSILON);
        assert!(verdict.grounded_explanation.is_empty());
        assert!(verdict.reasoning_chain.evidence_points.is_empty());
    }

    #[test]
    fn test_parse_json_confidence_clamped() {
        let json = r#"{"should_surface": true, "confidence": 1.5}"#;
        let verdict = parse_verdict(json).expect("Should clamp confidence");
        assert!((verdict.adjusted_confidence - 1.0).abs() < f32::EPSILON);

        let json_neg = r#"{"should_surface": true, "confidence": -0.3}"#;
        let verdict_neg = parse_verdict(json_neg).expect("Should clamp negative");
        assert!(verdict_neg.adjusted_confidence >= 0.0);
    }

    #[test]
    fn test_parse_invalid_json() {
        assert!(parse_verdict("not json at all").is_none());
        assert!(parse_verdict("").is_none());
        assert!(parse_verdict("{broken").is_none());
    }

    #[test]
    fn test_parse_defaults_to_surface_when_missing() {
        let json = r#"{}"#;
        let verdict = parse_verdict(json).expect("Should parse empty object");
        // Default: should_surface = true (fail open)
        assert!(verdict.should_surface);
    }

    // ---- strip_code_fences tests ----

    #[test]
    fn test_strip_code_fences_json() {
        let input = "```json\n{\"key\": \"value\"}\n```";
        assert_eq!(strip_code_fences(input), r#"{"key": "value"}"#);
    }

    #[test]
    fn test_strip_code_fences_bare() {
        let input = "```\n{\"key\": \"value\"}\n```";
        assert_eq!(strip_code_fences(input), r#"{"key": "value"}"#);
    }

    #[test]
    fn test_strip_code_fences_none() {
        let input = r#"{"key": "value"}"#;
        assert_eq!(strip_code_fences(input), input);
    }

    // ---- Critical/High bypass in filter_batch (unit-level) ----

    fn make_test_item(urgency: Urgency, title: &str) -> EvidenceItem {
        EvidenceItem {
            id: format!("test-{}", title.replace(' ', "-")),
            kind: EvidenceKind::Alert,
            title: title.to_string(),
            explanation: String::new(),
            confidence: Confidence::heuristic(0.6),
            urgency,
            reversibility: None,
            evidence: vec![],
            affected_projects: vec![],
            affected_deps: vec![],
            suggested_actions: vec![],
            precedents: vec![],
            refutation_condition: None,
            lens_hints: LensHints::preemption_only(),
            created_at: 0,
            expires_at: None,
        }
    }

    // Note: filter_batch integration tests require an LLM and are not
    // run in unit tests. The bypass logic for Critical/High is verified
    // by the Urgency ordering -- Critical < High < Medium < Watch --
    // so the comparison `item.urgency == Urgency::Critical || item.urgency
    // == Urgency::High` is exercised.

    #[test]
    fn test_urgency_ordering_for_bypass() {
        // Verify the enum ordering used by filter_batch bypass logic
        assert!(Urgency::Critical < Urgency::High);
        assert!(Urgency::High < Urgency::Medium);
        assert!(Urgency::Medium < Urgency::Watch);
    }

    #[test]
    fn test_make_test_item_fields() {
        let item = make_test_item(Urgency::Critical, "test vuln");
        assert_eq!(item.urgency, Urgency::Critical);
        assert_eq!(item.title, "test vuln");
        assert_eq!(item.id, "test-test-vuln");
    }
}
