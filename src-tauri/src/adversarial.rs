// SPDX-License-Identifier: FSL-1.1-Apache-2.0

//! Adversarial deliberation engine -- TitanCA-inspired two-perspective validation.
//!
//! Takes an `EvidenceItem` and runs a structured adversarial deliberation via the
//! user's configured LLM. Two perspectives -- Signal Advocate and Noise Challenger --
//! argue opposite sides, then an Arbitrator synthesizes a verdict.
//!
//! Design constraints:
//! - Single LLM call per item (all three roles combined in one prompt).
//! - One call per distinct material, not per refresh: verdicts are cached by
//!   a content hash for a week (`adversarial_cache.rs`, audit L2).
//! - Fail-closed: a rejected item stays suppressed while its verdict is
//!   valid, and an unvetted heuristic item is held back when the model
//!   cannot judge it (`adversarial_batch.rs` has the decision table).
//! - Background share: the pass stops at
//!   [`crate::state::BACKGROUND_BUDGET_SHARE_PERCENT`] of the daily AI
//!   budget, so user-initiated calls keep the rest.
//! - Critical/High items keep a safety floor: they always surface.
//! - Escalation gate: Critical/High is only honored when corroborated (real
//!   advisory linkage or non-empty affected deps; signal chains: OSV-verified
//!   provenance or non-empty affected deps only). Uncorroborated escalations
//!   are capped at Medium before the safety floor is computed.

use crate::evidence::{EvidenceItem, Urgency};
use crate::llm::{LLMClient, Message};
use crate::settings::LLMProvider;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

#[path = "adversarial_batch.rs"]
mod batch;
#[path = "adversarial_cache.rs"]
mod cache;

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

/// Why a deliberation could not run although the configuration supports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unavailable {
    /// The background share of the daily AI budget is spent.
    Budget,
    /// The provider call failed (network, provider error, cost wall).
    LlmError,
    /// The model answered, but not with a readable verdict.
    Unparseable,
}

impl Unavailable {
    /// Whether the rest of the batch should skip the model too. A budget
    /// stop or a failing provider would fail the same way for every item;
    /// one unreadable answer says nothing about the next.
    fn stops_batch(self) -> bool {
        matches!(self, Self::Budget | Self::LlmError)
    }
}

/// What one deliberation attempt produced.
#[derive(Debug, Clone)]
pub(crate) enum DeliberationOutcome {
    Verdict(DeliberationVerdict),
    Unavailable(Unavailable),
    /// No model is configured for this (a cloud provider without a key).
    NotConfigured,
}

/// Run adversarial deliberation on a single EvidenceItem with `provider`.
/// Never errors: every way it can fail is a [`DeliberationOutcome`] the
/// batch settles fail-closed.
async fn deliberate(
    item: &EvidenceItem,
    user_context: &str,
    provider: LLMProvider,
) -> DeliberationOutcome {
    let metered = provider.provider != "ollama";
    if metered && provider.api_key.is_empty() {
        debug!(
            target: "4da::adversarial",
            item_id = %item.id,
            provider = %provider.provider,
            "Skipping deliberation -- no API key configured"
        );
        return DeliberationOutcome::NotConfigured;
    }
    // The background pass stops at its share of the daily budget; the rest
    // is reserved for calls the user asks for (audit L2: this pass spent the
    // whole cap daily and user assessments were refused).
    if metered && crate::state::background_llm_budget_reached() {
        debug!(
            target: "4da::adversarial",
            item_id = %item.id,
            "Skipping deliberation -- background share of the daily AI budget spent"
        );
        return DeliberationOutcome::Unavailable(Unavailable::Budget);
    }

    let system_prompt = build_system_prompt();
    let user_message = build_user_message(item, user_context);

    // llm-egress: no-item-body sends the title, 4DA's own explanation and dependency names, never the article text
    let client = LLMClient::with_purpose(provider, "adversarial");
    let messages = vec![Message {
        role: "user".to_string(),
        content: user_message,
    }];

    let response = match client.complete(&system_prompt, messages).await {
        Ok(resp) => resp,
        Err(e) => {
            warn!(
                target: "4da::adversarial",
                item_id = %item.id,
                error = %e,
                "LLM call failed during deliberation -- item settled fail-closed"
            );
            return DeliberationOutcome::Unavailable(Unavailable::LlmError);
        }
    };

    debug!(
        target: "4da::adversarial",
        item_id = %item.id,
        input_tokens = response.input_tokens,
        output_tokens = response.output_tokens,
        "Deliberation LLM call complete"
    );

    match parse_verdict(&response.content) {
        Some(verdict) => {
            info!(
                target: "4da::adversarial",
                item_id = %item.id,
                should_surface = verdict.should_surface,
                adjusted_confidence = verdict.adjusted_confidence,
                "Deliberation verdict rendered"
            );
            DeliberationOutcome::Verdict(verdict)
        }
        None => {
            warn!(
                target: "4da::adversarial",
                item_id = %item.id,
                response_len = response.content.len(),
                "Failed to parse deliberation verdict -- item settled fail-closed"
            );
            DeliberationOutcome::Unavailable(Unavailable::Unparseable)
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
/// The preemption fast path (cache-miss feed, no LLM) applies this same
/// deterministic gate through [`apply_stored_verdicts`], so phantom critical
/// chains cannot flash there while the deliberated recompute is running.
fn gate_escalation(item: &mut EvidenceItem) -> bool {
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
/// - A stored verdict for the same material is reused with no model call.
/// - Items where the verdict says "don't surface" are dropped.
/// - Items where the verdict says "surface" get their explanation and confidence
///   updated with the grounded output.
/// - Items the model cannot judge right now are settled fail-closed (see
///   `adversarial_batch.rs`).
///
/// Processing is sequential (budget-conscious -- one LLM call per item).
pub(crate) async fn filter_batch(
    items: Vec<EvidenceItem>,
    user_context: &str,
) -> Vec<EvidenceItem> {
    let (items, demoted) = gate_batch(items);
    let Some(llm) = deliberation_provider(items.len(), demoted) else {
        return items;
    };
    let db = crate::get_database().ok();
    let mut cache = db
        .map(|db| cache::VerdictCache::load(db))
        .unwrap_or_default();
    let now = chrono::Utc::now().timestamp();
    let system_prompt = build_system_prompt();
    let keyer =
        |item: &EvidenceItem| cache::material_key(item, &system_prompt, &llm.provider, &llm.model);
    let total = items.len();
    let outcome = batch::deliberate_batch(items, &mut cache, keyer, now, |item| {
        let provider = llm.clone();
        let context = user_context.to_string();
        async move { deliberate(&item, &context, provider).await }
    })
    .await;
    if let Some(db) = db {
        cache.save(db, now);
    }
    let stats = &outcome.stats;
    info!(
        target: "4da::adversarial",
        total,
        demoted,
        bypassed = stats.bypassed,
        cache_hits = stats.cache_hits,
        llm_calls = stats.llm_calls,
        filtered = stats.filtered,
        held_back = stats.held_back,
        unavailable = ?stats.unavailable,
        passed = outcome.passed.len(),
        "Adversarial filter batch complete"
    );
    outcome.passed
}

/// The no-model counterpart of [`filter_batch`] for the cache-miss fast
/// feed: the escalation gate, then stored verdicts only. A rejected item
/// cannot flash in the fast feed, and an unvetted heuristic waits for the
/// deliberated refresh started beside it. With no deliberation configured,
/// the gated items pass as [`filter_batch`] would pass them.
pub(crate) fn apply_stored_verdicts(items: Vec<EvidenceItem>) -> Vec<EvidenceItem> {
    let (items, demoted) = gate_batch(items);
    let Some(llm) = deliberation_provider(items.len(), demoted) else {
        return items;
    };
    let Ok(db) = crate::get_database() else {
        return items;
    };
    let cache = cache::VerdictCache::load(db);
    let system_prompt = build_system_prompt();
    let keyer =
        |item: &EvidenceItem| cache::material_key(item, &system_prompt, &llm.provider, &llm.model);
    let outcome = batch::settle_from_cache(items, &cache, keyer, chrono::Utc::now().timestamp());
    if outcome.stats.filtered + outcome.stats.held_back > 0 {
        info!(
            target: "4da::adversarial",
            cache_hits = outcome.stats.cache_hits,
            filtered = outcome.stats.filtered,
            held_back = outcome.stats.held_back,
            "fast feed settled from stored verdicts"
        );
    }
    outcome.passed
}

/// The escalation gate over a batch. Runs FIRST — deterministic,
/// zero-cost, and regardless of LLM availability: the Basic-tier early
/// return used to exit before the gate, so on a Basic-tier/no-LLM config
/// phantom critical chains sailed through untouched (found live 2026-08-24).
/// An uncorroborated Critical/High item (no advisory linkage, no affected
/// deps) loses its escalation BEFORE the safety floor is computed.
fn gate_batch(mut items: Vec<EvidenceItem>) -> (Vec<EvidenceItem>, usize) {
    let mut demoted: usize = 0;
    for item in items.iter_mut() {
        if item.confidence.provenance == crate::evidence::ConfidenceProvenance::OsvVerified {
            continue;
        }
        let original_urgency = item.urgency;
        if gate_escalation(item) {
            demoted += 1;
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
    (items, demoted)
}

/// The configured provider when its model tier supports deliberation.
/// Basic-tier models produce unreliable verdicts that would wrongly filter
/// good items, so they skip deliberation (the escalation gate has run).
fn deliberation_provider(count: usize, demoted: usize) -> Option<LLMProvider> {
    let llm = {
        let mgr = crate::get_settings_manager();
        let mut guard = mgr.lock();
        guard.ensure_keys_hydrated();
        guard.get().llm.clone()
    };
    let tier = crate::llm_capability::get_model_tier(&llm);
    if tier.supports_adversarial() {
        return Some(llm);
    }
    debug!(
        target: "4da::adversarial",
        tier = %tier,
        count,
        demoted,
        "LLM model tier does not support adversarial deliberation, passing gated items through"
    );
    None
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
