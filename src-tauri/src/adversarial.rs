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
//! - Escalation gate: Critical/High is only honored when corroborated (real
//!   advisory linkage or non-empty affected deps; signal chains: OSV-verified
//!   provenance or non-empty affected deps only). Uncorroborated escalations
//!   are capped at Medium before the safety floor is computed.

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

    let provider = {
        let mgr = crate::get_settings_manager();
        let mut guard = mgr.lock();
        guard.ensure_keys_hydrated();
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

/// How a deliberation verdict is applied to an item. Extracted as data so the
/// decision is testable without an LLM (`filter_batch` itself needs one).
#[derive(Debug, PartialEq, Eq)]
enum VerdictApplication {
    /// Verdict agrees the item should surface — adopt the grounded explanation
    /// and adjusted confidence.
    SurfaceUpdated,
    /// Safety floor: the verdict argued AGAINST surfacing, but Critical/High
    /// urgency mandates surfacing anyway. The item surfaces UNCHANGED — its
    /// original explanation and confidence stand. Adopting the verdict here
    /// ships an alert that argues against itself (observed live 2026-08-22:
    /// a Critical chain-alert whose own explanation read "incorrectly
    /// escalated" at 92% displayed confidence).
    SurfaceUnchanged,
    /// Verdict says don't surface and no safety floor applies — drop the item.
    Filter,
}

/// The one decision table for applying a verdict. `must_surface` is the
/// Critical/High safety floor; `should_surface` is the LLM's judgment.
fn apply_verdict(must_surface: bool, should_surface: bool) -> VerdictApplication {
    match (should_surface, must_surface) {
        (true, _) => VerdictApplication::SurfaceUpdated,
        (false, true) => VerdictApplication::SurfaceUnchanged,
        (false, false) => VerdictApplication::Filter,
    }
}

// ============================================================================
// Escalation corroboration
// ============================================================================

/// Advisory-id families that count as real advisory linkage. Every family
/// except GHSA is followed by a numeric segment ("CVE-2026-1234",
/// "RUSTSEC-2026-0001", "GO-2026-5781"); GHSA ids use base32 segments, so
/// GHSA is matched on the boundary-checked prefix alone.
const ADVISORY_ID_PREFIXES: &[&str] =
    &["CVE-", "GHSA-", "RUSTSEC-", "OSV-", "PYSEC-", "MAL-", "GO-"];

/// Urgency cap applied to an escalated item whose escalation is
/// uncorroborated. Medium ("act within the month") keeps the item visible
/// and deliberation-eligible without granting it the Critical/High safety
/// floor; Watch stays reserved for chains that preemption itself already
/// classifies as ungrounded ecosystem awareness.
const UNCORROBORATED_ESCALATION_CAP: Urgency = Urgency::Medium;

/// True when `text` contains a token that looks like a real advisory id.
///
/// A prefix only counts when it starts at a token boundary (start of string
/// or after a non-alphanumeric byte), so prose like "normal-2026" never
/// matches "MAL-". Numeric families additionally require a digit right after
/// the prefix, so "go-to-definition" never matches "GO-". ASCII-only case
/// folding keeps byte offsets stable (all prefixes are pure ASCII).
///
/// `pub(crate)`: preemption's `chain_to_alert` uses this pure check to keep
/// its explanation copy honest ("Includes a published advisory." vs "No
/// advisory issued.") — display copy only, never an escalation input there.
pub(crate) fn contains_advisory_id(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    ADVISORY_ID_PREFIXES.iter().any(|prefix| {
        let p = prefix.as_bytes();
        bytes
            .windows(p.len())
            .enumerate()
            .filter(|(_, window)| *window == p)
            .any(|(pos, _)| {
                let at_boundary = pos == 0 || !bytes[pos - 1].is_ascii_alphanumeric();
                let next = bytes.get(pos + p.len());
                let follow_ok = if *prefix == "GHSA-" {
                    next.is_some_and(|b| b.is_ascii_alphanumeric())
                } else {
                    next.is_some_and(|b| b.is_ascii_digit())
                };
                at_boundary && follow_ok
            })
    })
}

/// Does this item carry real advisory linkage? Machine-verified OSV
/// provenance, or an advisory id in its title or structured evidence
/// citations. Deliberately never inspects `explanation` — a previous fix
/// round (R3) rewrote explanation copy, and the escalation decision must
/// rest on structured evidence, not prose.
///
/// SIGNAL CHAINS (`chain-*` ids): the ONLY advisory linkage a chain can earn
/// here is machine-verified OSV provenance — advisory ids in a chain's title
/// or citations never count. This is the single place the chain rule lives.
///
/// Why neither text arm works for chains: a chain AGGREGATES co-tokened
/// items, so its evidence list routinely includes advisory-titled neighbors
/// that merely share the chain's token — live post-activation proof
/// (2026-08-24): all three phantom single-token chains
/// ("table"/"sandbox"/"next") survived the gate through the citation arm, the
/// "table" chain citing an unrelated XWiki "Live Table" CVE. And a chain
/// alert's TITLE is just its first link's title (`chain_to_alert`), i.e. the
/// same aggregated-neighbor text — measured live 2026-08-25: two critical
/// chains with empty affected deps rode their own "[CVE-...]" first-link
/// titles through the former own-title arm. An off-stack advisory in a
/// chain's text is ecosystem awareness, not the user's exposure. A chain
/// whose topic IS a verified installed dep carries that dep in
/// `affected_deps` (`chain_to_alert` propagates `SignalChain::verified_dep`)
/// and passes the escalation gate through its deps arm instead.
fn has_advisory_linkage(item: &EvidenceItem) -> bool {
    if item.confidence.provenance == crate::evidence::ConfidenceProvenance::OsvVerified {
        return true;
    }
    if item.id.starts_with("chain-") {
        return false;
    }
    contains_advisory_id(&item.title)
        || item.evidence.iter().any(|citation| {
            contains_advisory_id(&citation.title)
                || citation.url.as_deref().is_some_and(contains_advisory_id)
        })
}

/// The escalation gate at the deliberation boundary. A Critical/High item
/// keeps its urgency — and thereby earns the must-surface safety floor —
/// only when the escalation is corroborated: real advisory linkage, or
/// non-empty affected deps confirmed by an upstream materializer.
///
/// For `chain-*` items, corroboration means OSV-verified provenance or
/// non-empty affected deps ONLY (see [`has_advisory_linkage`] for the chain
/// rule and its live evidence). This should be a pure safety net:
/// `chain_policy` only mints an escalation-capable priority for dep-grounded
/// chains, and `chain_to_alert` propagates `SignalChain::verified_dep` into
/// affected deps — so a legitimately escalated chain arrives carrying its
/// dep. The gate catches drift (stale persisted chains predating
/// `verified_dep`, future policy changes) and demotes any chain that arrives
/// escalated with neither, instead of letting it bypass deliberation as a
/// critical alert.
///
/// Returns `true` when the item was demoted (the caller logs the demotion).
///
/// `pub(crate)`: the preemption fast path (cache-miss feed, no LLM) applies
/// this same deterministic gate so phantom critical chains cannot flash
/// there while the deliberated recompute is still running.
pub(crate) fn gate_escalation(item: &mut EvidenceItem) -> bool {
    let escalated = item.urgency == Urgency::Critical || item.urgency == Urgency::High;
    if !escalated {
        return false;
    }
    if has_advisory_linkage(item) || !item.affected_deps.is_empty() {
        return false;
    }
    item.urgency = UNCORROBORATED_ESCALATION_CAP;
    true
}

/// Run adversarial deliberation on a batch of items, filtering out items that
/// don't pass deliberation.
///
/// - Critical and High urgency items always surface (never filter
///   safety-critical intelligence) — but a dissenting verdict never rewrites
///   them (see [`VerdictApplication::SurfaceUnchanged`]), and the floor is
///   only granted to corroborated escalations (see [`gate_escalation`]):
///   an escalated item with no advisory linkage and no affected deps is
///   capped at Medium first and deliberated like any other Medium item.
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
    // Escalation gate FIRST — deterministic, zero-cost, and it must run
    // regardless of LLM availability: the Basic-tier early return below used
    // to exit before the gate, so on a Basic-tier/no-LLM config phantom
    // critical chains sailed through the deliberated path untouched (found
    // live 2026-08-24 during post-activation verification). An uncorroborated
    // Critical/High item (no advisory linkage, no affected deps) loses its
    // escalation BEFORE the safety floor is computed. R3 fixed the
    // explanation copy of phantom chain alerts ("No advisory issued"); this
    // fixes the escalation itself.
    let mut items = items;
    let mut demoted_count: usize = 0;
    for item in items.iter_mut() {
        if item.confidence.provenance == crate::evidence::ConfidenceProvenance::OsvVerified {
            continue;
        }
        let original_urgency = item.urgency;
        if gate_escalation(item) {
            demoted_count += 1;
            warn!(
                target: "4da::adversarial",
                item_id = %item.id,
                title = %item.title,
                from = ?original_urgency,
                to = ?item.urgency,
                "Escalated item has no advisory linkage and no affected deps; demoted below the Critical/High floor"
            );
        }
    }

    // Gate: skip adversarial deliberation for Basic-tier models. Small models
    // produce unreliable verdicts that would incorrectly filter good items.
    // (The escalation gate above has already run — this skips only the LLM.)
    let llm_settings = {
        let mgr = crate::get_settings_manager();
        let guard = mgr.lock();
        guard.get().llm.clone()
    };
    let tier = crate::llm_capability::get_model_tier(&llm_settings);
    if !tier.supports_adversarial() {
        debug!(
            target: "4da::adversarial",
            tier = %tier,
            count = items.len(),
            demoted = demoted_count,
            "LLM model tier does not support adversarial deliberation, passing gated items through"
        );
        return items;
    }

    let total = items.len();
    let mut passed = Vec::with_capacity(total);
    let mut filtered_count: usize = 0;
    let mut bypass_count: usize = 0;
    let mut delib_count: usize = 0;

    for item in items {
        // OSV-verified items are machine-confirmed (semver range check against
        // installed version). No LLM deliberation needed — pass through as-is.
        if item.confidence.provenance == crate::evidence::ConfidenceProvenance::OsvVerified {
            bypass_count += 1;
            passed.push(item);
            continue;
        }

        let must_surface = item.urgency == Urgency::Critical || item.urgency == Urgency::High;

        delib_count += 1;
        if must_surface {
            bypass_count += 1;
        }

        match deliberate(&item, user_context).await {
            Ok(Some(verdict)) => match apply_verdict(must_surface, verdict.should_surface) {
                VerdictApplication::SurfaceUpdated => {
                    let mut updated = item;
                    updated.explanation = verdict.grounded_explanation;
                    updated.confidence =
                        Confidence::llm_assessed(verdict.adjusted_confidence.clamp(0.0, 1.0));
                    passed.push(updated);
                }
                VerdictApplication::SurfaceUnchanged => {
                    // Surfaced by the safety floor over LLM dissent. Keep the
                    // item's own explanation/confidence — the dissent is an
                    // input to filtering, not a rewrite of the evidence.
                    warn!(
                        target: "4da::adversarial",
                        item_id = %item.id,
                        title = %item.title,
                        adjusted_confidence = verdict.adjusted_confidence,
                        "Critical/High item surfaced despite dissenting verdict; original explanation kept"
                    );
                    passed.push(item);
                }
                VerdictApplication::Filter => {
                    filtered_count += 1;
                    debug!(
                        target: "4da::adversarial",
                        item_id = %item.id,
                        title = %item.title,
                        "Item filtered by adversarial deliberation"
                    );
                }
            },
            Ok(None) => {
                // LLM unavailable -- pass through unchanged
                passed.push(item);
            }
            Err(e) => {
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
        demoted = demoted_count,
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
    let has_connector = CAUSAL_CONNECTORS.iter().any(|conn| lower.contains(conn));

    if !has_connector {
        return false;
    }

    // Check 3 is title-independent; we only have the explanation here.
    // The caller can do title-overlap checking externally if needed.
    // We check that the explanation isn't trivially short after stripping
    // connectors, which catches "X because X" type restatements.
    true
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

    // title / explanation / deps / projects can derive from untrusted scraped content —
    // sanitize before sending to the deliberation LLM so a crafted item field can't inject
    // instructions (defense-only; does not change the deliberation verdict).
    use crate::prompt_safety::sanitize_untrusted;
    let deps = if item.affected_deps.is_empty() {
        "none".to_string()
    } else {
        sanitize_untrusted(&item.affected_deps.join(", "))
    };

    let projects = if item.affected_projects.is_empty() {
        "none".to_string()
    } else {
        sanitize_untrusted(&item.affected_projects.join(", "))
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
        title = sanitize_untrusted(&item.title),
        kind = kind_str.trim_matches('"'),
        urgency = urgency_str.trim_matches('"'),
        explanation = sanitize_untrusted(&item.explanation),
        deps = deps,
        projects = projects,
        context = sanitize_untrusted(user_context),
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
        grounded_explanation: parsed.grounded_explanation.unwrap_or_default(),
        signal_argument: parsed.signal_argument.unwrap_or_default(),
        noise_argument: parsed.noise_argument.unwrap_or_default(),
        reasoning_chain: ReasoningChain {
            claim: chain.and_then(|c| c.claim.clone()).unwrap_or_default(),
            evidence_points: chain
                .and_then(|c| c.evidence_points.clone())
                .unwrap_or_default(),
            connection: chain.and_then(|c| c.connection.clone()).unwrap_or_default(),
            conclusion: chain.and_then(|c| c.conclusion.clone()).unwrap_or_default(),
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
        let content_start = after_fence.find('\n').map(|i| i + 1).unwrap_or(0);
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
            let top_tech: Vec<&str> = tech.iter().take(10).map(|t| t.name.as_str()).collect();
            if !top_tech.is_empty() {
                parts.push(format!("Tech stack: {}", top_tech.join(", ")));
            }
        }
        if let Ok(topics) = ace.get_active_topics() {
            let top_topics: Vec<&str> = topics.iter().take(5).map(|t| t.topic.as_str()).collect();
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

#[cfg(test)]
#[path = "adversarial_tests.rs"]
mod tests;
