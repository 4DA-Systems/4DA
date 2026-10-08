// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! AI relevance assessment ("Assess with AI" — Phase B).
//!
//! On-demand LLM triage of the surfaced coverage-gap blind spots: ONE batched
//! call decides which gaps warrant attention and gives a one-line next step.
//! Signal-gated; degrades to the `no_llm_configured` hint without a model.
//!
//! **Cache (audit 2026-10-07, wave 4c).** The verdict persists in `kv_store`
//! keyed on a STABLE set — the dependencies that carry security or
//! breaking-change evidence, plus a fingerprint of those evidence rows. The
//! old in-memory cache was keyed on every surfaced dependency, a set that
//! changes daily as routine releases enter and leave the 14-day window, and
//! died with the process; the view then re-ran the model on every tab open
//! (19 Sonnet calls in five weeks). Routine release churn no longer reaches
//! the key, so "it only uses your model when something actually changes" is
//! true. A manual click passes `force` and always re-assesses.
//!
//! **Evidence, not labels.** The model is sent evidence types and counts
//! ("1 new release (you're on 0.8.21); never engaged"), never a risk label —
//! the engagement-derived `risk=critical` it used to receive turned one
//! routine schemars release into "Critical risk signal, review it now".

use std::future::Future;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    breakdown_total, correct_fabricated_signal_counts, gap_row_breakdown,
    generate_blind_spot_report, installed_note, now_millis, parse_why_signal_count, truncate_note,
    DepSignalBreakdown, UncoveredDep,
};
use crate::db::Database;

/// Per-dependency AI verdict. `dep_name` is the DISPLAY name ("libc (crates.io)")
/// so the frontend can join it back to the rendered row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DepAssessment {
    pub dep_name: String,
    pub worth_reviewing: bool,
    pub recommendation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct BlindSpotAssessment {
    pub assessments: Vec<DepAssessment>,
    pub model: String,
    pub assessed_at: i64,
    pub from_cache: bool,
    /// True when this is a persisted verdict whose stable key no longer
    /// matches the current evidence — the view may re-assess. Always false
    /// on a fresh or key-matching result.
    #[serde(default)]
    pub stale: bool,
}

/// One dependency as the model sees it.
#[derive(Debug, Clone)]
pub(super) struct AssessInput {
    pub name: String,
    /// Evidence summary sent to the model: counts by type + engagement.
    pub why: String,
    /// Security/breaking evidence exists — the verdict can never be "fine".
    pub force_worth: bool,
    /// An advisory affecting the installed version exists — the only
    /// evidence that licenses "critical"/"high-risk" wording.
    pub has_advisory: bool,
    /// The dependency has any reviewable signal at all.
    pub has_signals: bool,
}

/// `kv_store` record of the last assessment and the key it was made for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PersistedAssessment {
    pub version: u32,
    pub key: String,
    pub assessment: BlindSpotAssessment,
}

const KV_KEY: &str = "blind_spot_ai_assessment";
const PERSIST_VERSION: u32 = 1;

/// SplitMix64 finaliser: a stable (cross-process, cross-toolchain) mix for
/// evidence-row fingerprints. `DefaultHasher` is not stable across Rust
/// releases, and this key is persisted.
pub(super) fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The cache key: dependencies WITH security/breaking evidence and a
/// fingerprint of those evidence rows. Release-only and zero-signal deps
/// never enter it — their daily churn is exactly what kept invalidating
/// the old key.
pub(super) fn stable_assessment_key(rows: &[(String, Option<DepSignalBreakdown>)]) -> String {
    let mut lines: Vec<String> = rows
        .iter()
        .filter_map(|(name, b)| {
            let b = b.as_ref().filter(|b| b.security > 0)?;
            Some(format!(
                "{name}|{}|{}|{:016x}",
                b.security, b.advisories, b.security_fingerprint
            ))
        })
        .collect();
    lines.sort();
    lines.dedup();
    format!("v{PERSIST_VERSION};{}", lines.join(";"))
}

fn plural(n: u32, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The evidence line for one dependency: counts by type and engagement —
/// never a risk label.
pub(super) fn describe_evidence(
    d: &UncoveredDep,
    breakdown: Option<DepSignalBreakdown>,
    installed: &[String],
) -> String {
    let engagement = if d.days_since_last_signal >= 999 {
        "never engaged".to_string()
    } else {
        format!(
            "last opened an item about it {} days ago",
            d.days_since_last_signal
        )
    };
    let Some(b) = breakdown.filter(|b| breakdown_total(*b) > 0) else {
        let coverage = d
            .coverage_reason
            .clone()
            .unwrap_or_else(|| "no confirmed source coverage".to_string());
        return format!("no signals ({coverage}); {engagement}");
    };
    let ver = installed_note(installed);
    let breaking = b.security - b.advisories;
    let mut parts: Vec<String> = Vec::new();
    if b.advisories > 0 {
        let n = plural(b.advisories, "advisory", "advisories");
        parts.push(format!("{n} affecting your installed version{ver}"));
    }
    if breaking > 0 {
        parts.push(plural(
            breaking,
            "breaking-change signal",
            "breaking-change signals",
        ));
    }
    if b.releases > 0 {
        let n = plural(b.releases, "new release", "new releases");
        parts.push(format!("{n}{ver}"));
    }
    if b.analyses > 0 {
        parts.push(plural(
            b.analyses,
            "expert analysis article",
            "expert analysis articles",
        ));
    }
    if b.other > 0 {
        parts.push(plural(b.other, "discussion item", "discussion items"));
    }
    let total = breakdown_total(b);
    format!(
        "{} unreviewed signal{}: {}; {engagement}",
        total,
        if total == 1 { "" } else { "s" },
        parts.join(", ")
    )
}

/// Build the model inputs and the stable cache key from the current report.
/// Blocking (report + per-dep breakdowns) — call off the async runtime.
fn gather_inputs() -> std::result::Result<(Vec<AssessInput>, String), String> {
    let report = generate_blind_spot_report().map_err(|e| e.to_string())?;
    let mut inputs = Vec::new();
    let mut key_rows = Vec::new();
    for d in &report.uncovered_dependencies {
        let (installed, breakdown) = gap_row_breakdown(d);
        let security = breakdown.map_or(0, |b| b.security);
        inputs.push(AssessInput {
            name: d.name.clone(),
            why: describe_evidence(d, breakdown, &installed),
            force_worth: security > 0,
            has_advisory: breakdown.is_some_and(|b| b.advisories > 0),
            has_signals: breakdown.is_some_and(|b| breakdown_total(b) > 0),
        });
        key_rows.push((d.name.clone(), breakdown));
    }
    Ok((inputs, stable_assessment_key(&key_rows)))
}

pub(super) const BS_ASSESS_SYSTEM_PROMPT: &str = r#"You are triaging dependency "blind spots" for a specific developer's stack. Each item is a dependency in the developer's own project manifest, followed by its EVIDENCE: counts by type (advisories affecting the installed version, breaking-change signals, new releases, expert analysis articles, discussion items) and whether the developer has ever opened an item about it. There is no risk label — judge from the evidence. Decide which genuinely warrant the developer's attention RIGHT NOW, and give one short, concrete sentence on why + what to do.

Be strict — most stable, low-churn libraries are NOT worth reviewing. Mark `worth_reviewing` true ONLY when there is a real reason to act: an advisory affecting the installed version, a breaking change, a new MAJOR version the developer should evaluate, or a maintenance/abandonment risk for a load-bearing dependency. A dependency with no signals is usually a quiet, mature library — mark it false ("Stable library, no action needed.").

Routine releases are maintenance, not risk. A dependency whose only evidence is new releases is at most "skim the changelog before your next upgrade". "never engaged" only means the developer has not opened an item about it — it is not a risk.

Wording rules: NEVER describe a dependency as "critical", "high-risk", "urgent" or a "risk signal" unless its evidence lists an advisory affecting the installed version. If you mark an item false, do not tell the developer to review it now.

The `recommendation` must be ONE short sentence: the reason plus a concrete next step (e.g. "Review the 19.x breaking changes before upgrading" or "Mature crypto primitive, no action needed").

Do NOT state, restate, or invent any signal counts, quantities, or totals in the recommendation — the UI already shows the exact numbers. Speak qualitatively ("has an unreviewed advisory", "a new release to skim"). You MAY cite a specific VERSION number (e.g. "the 19.x line").

Output a JSON array ONLY, one object per numbered dependency:
[{"id": <number>, "worth_reviewing": <true|false>, "recommendation": "<one short sentence>"}]"#;

/// Risk vocabulary only an advisory licenses.
const RISK_WORDS: [&str; 5] = [
    "critical",
    "high-risk",
    "high risk",
    "urgent",
    "risk signal",
];

/// A verdict-neutral, evidence-true recommendation used when the model's
/// sentence contradicts the evidence.
fn evidence_recommendation(input: &AssessInput) -> String {
    if input.has_advisory {
        "An advisory affects your installed version — review it before your next upgrade."
    } else if input.force_worth {
        "Unreviewed breaking-change signal — read it before your next upgrade."
    } else if input.has_signals {
        "Routine release activity — skim the changelog before your next upgrade."
    } else {
        "No confirmed source coverage — watch its repository if it is load-bearing."
    }
    .to_string()
}

/// Replace a recommendation that claims risk the evidence does not show.
pub(super) fn enforce_risk_vocabulary(rec: &str, input: &AssessInput) -> String {
    let lower = rec.to_lowercase();
    if !input.has_advisory && RISK_WORDS.iter().any(|w| lower.contains(w)) {
        evidence_recommendation(input)
    } else {
        rec.to_string()
    }
}

/// Parse the model's `[{id, worth_reviewing, recommendation}]` array, joining
/// each entry back to its dep by 1-based index. Tolerant: a parse failure
/// yields an empty Vec (the UI then shows "couldn't assess"), never a panic.
/// `force_worth` (security/breaking evidence ONLY — never an engagement-based
/// risk level) keeps a dep worth reviewing whatever the model says, and then
/// the sentence must not read "no action needed".
pub(super) fn parse_dep_assessments(response: &str, inputs: &[AssessInput]) -> Vec<DepAssessment> {
    let json_str = match (response.find('['), response.rfind(']')) {
        (Some(s), Some(e)) if e >= s => &response[s..=e],
        _ => response,
    };
    let parsed: Vec<serde_json::Value> = serde_json::from_str(json_str).unwrap_or_default();
    let mut out = Vec::new();
    for v in parsed {
        let id = v["id"]
            .as_u64()
            .or_else(|| v["id"].as_i64().map(|n| n.max(0) as u64))
            .unwrap_or(0);
        if id == 0 || (id as usize) > inputs.len() {
            continue;
        }
        let input = &inputs[id as usize - 1];
        let model_worth = v["worth_reviewing"].as_bool().unwrap_or(false);
        // Correct any hallucinated signal count against the true figure we
        // fed the model (the leading number in `why`), THEN police wording.
        let corrected = correct_fabricated_signal_counts(
            v["recommendation"].as_str().unwrap_or(""),
            parse_why_signal_count(&input.why),
        );
        let recommendation = if input.force_worth && !model_worth {
            evidence_recommendation(input)
        } else {
            enforce_risk_vocabulary(&corrected, input)
        };
        out.push(DepAssessment {
            dep_name: input.name.clone(),
            worth_reviewing: model_worth || input.force_worth,
            recommendation: truncate_note(&recommendation),
        });
    }
    out
}

/// Numbered item lines for the user message.
fn render_items(inputs: &[AssessInput]) -> String {
    inputs
        .iter()
        .enumerate()
        .map(|(i, d)| format!("{}. {} — {}", i + 1, d.name, d.why))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What one assessment produced, and the record to persist (None when the
/// result came from the cache or the model's answer was unusable).
pub(super) struct AssessOutcome {
    pub assessment: BlindSpotAssessment,
    pub persist: Option<PersistedAssessment>,
}

/// The cache-or-model decision, with the model call injected so tests can
/// count it. A persisted verdict whose key matches is returned with ZERO
/// model calls unless `force`. `call_llm` gets the numbered item lines and
/// returns `(response_text, model_name)`.
pub(super) async fn assess_core<F, Fut>(
    inputs: &[AssessInput],
    key: &str,
    cached: Option<PersistedAssessment>,
    force: bool,
    call_llm: F,
) -> std::result::Result<AssessOutcome, String>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = std::result::Result<(String, String), String>>,
{
    if !force {
        if let Some(hit) = cached.filter(|c| c.key == key) {
            let mut assessment = hit.assessment;
            assessment.from_cache = true;
            assessment.stale = false;
            return Ok(AssessOutcome {
                assessment,
                persist: None,
            });
        }
    }
    let (content, model) = call_llm(render_items(inputs)).await?;
    let assessment = BlindSpotAssessment {
        assessments: parse_dep_assessments(&content, inputs),
        model,
        assessed_at: now_millis(),
        from_cache: false,
        stale: false,
    };
    let persist = (!assessment.assessments.is_empty()).then(|| PersistedAssessment {
        version: PERSIST_VERSION,
        key: key.to_string(),
        assessment: assessment.clone(),
    });
    Ok(AssessOutcome {
        assessment,
        persist,
    })
}

pub(super) fn load_persisted_from(db: &Database) -> Option<PersistedAssessment> {
    let raw = db.get_kv(KV_KEY).ok().flatten()?;
    serde_json::from_str::<PersistedAssessment>(&raw)
        .ok()
        .filter(|p| p.version == PERSIST_VERSION)
}

pub(super) fn store_persisted_in(db: &Database, p: &PersistedAssessment) {
    match serde_json::to_string(p) {
        Ok(json) => {
            if let Err(e) = db.set_kv(KV_KEY, &json) {
                tracing::warn!(target: "4da::blind_spots", "persist AI assessment failed: {e}");
            }
        }
        Err(e) => tracing::warn!(target: "4da::blind_spots", "encode AI assessment failed: {e}"),
    }
}

fn load_persisted() -> Option<PersistedAssessment> {
    crate::get_database()
        .ok()
        .and_then(|db| load_persisted_from(db))
}

/// A persisted verdict as the view sees it: `stale` when the current stable
/// key is known and differs. An unknown current key is NOT stale — a failed
/// report must never trigger a paid re-run.
pub(super) fn cached_view(
    p: PersistedAssessment,
    current_key: Option<&str>,
) -> BlindSpotAssessment {
    let mut a = p.assessment;
    a.from_cache = true;
    a.stale = current_key.is_some_and(|k| k != p.key);
    a
}

/// LLM provider from settings, or `None` when nothing usable is configured
/// (hosted provider with no API key). Mirrors `llm_judgments::get_llm_settings`.
fn assessment_llm_provider() -> Option<crate::settings::LLMProvider> {
    let mgr = crate::get_settings_manager();
    let mut guard = mgr.lock();
    guard.ensure_keys_hydrated();
    let provider = guard.get().llm.clone();
    if provider.provider != "ollama" && provider.api_key.is_empty() {
        return None;
    }
    Some(provider)
}

/// The real model call: one batched completion, no guards held across it.
async fn call_assessment_llm(items_text: String) -> std::result::Result<(String, String), String> {
    let provider = assessment_llm_provider().ok_or_else(|| "no_llm_configured".to_string())?;
    let model = provider.model.clone();
    let context = crate::adversarial::build_user_context_summary();
    let user_message = format!(
        "## Developer context\n{context}\n\n## Surfaced dependency blind spots\n{items_text}\n\nTriage each numbered dependency per the rubric. Output the JSON array only:"
    );
    // llm-egress: no-item-body sends dependency names and evidence counts from the user's own manifests
    let client = crate::llm::LLMClient::with_purpose(provider, "blind_spots");
    let response = client
        .complete(
            BS_ASSESS_SYSTEM_PROMPT,
            vec![crate::llm::Message {
                role: "user".to_string(),
                content: user_message,
            }],
        )
        .await
        .map_err(|e| format!("AI assessment failed: {e}"))?;
    Ok((response.content, model))
}

/// AI triage of the surfaced blind spots. Returns the persisted verdict at
/// zero model cost while the stable evidence key is unchanged; `force`
/// (the manual button) always re-assesses.
#[tauri::command]
pub async fn assess_blind_spots_with_ai(
    force: Option<bool>,
) -> std::result::Result<BlindSpotAssessment, String> {
    crate::settings::require_signal_feature("assess_blind_spots_with_ai")
        .map_err(|e| e.to_string())?;
    let (inputs, key) = tauri::async_runtime::spawn_blocking(gather_inputs)
        .await
        .map_err(|e| format!("blind spot assessment task failed: {e}"))??;
    if inputs.is_empty() {
        return Ok(BlindSpotAssessment {
            assessments: Vec::new(),
            model: String::new(),
            assessed_at: now_millis(),
            from_cache: false,
            stale: false,
        });
    }
    let cached = load_persisted();
    let outcome = assess_core(
        &inputs,
        &key,
        cached,
        force.unwrap_or(false),
        call_assessment_llm,
    )
    .await?;
    if let (Some(p), Ok(db)) = (outcome.persist.as_ref(), crate::get_database()) {
        store_persisted_in(db, p);
    }
    Ok(outcome.assessment)
}

/// The persisted AI assessment WITHOUT calling the LLM — survives restarts.
/// The view calls this first; `stale` tells it whether the evidence moved
/// since (the only case an automatic re-assessment is allowed).
#[tauri::command]
pub async fn get_cached_blind_spot_assessment(
) -> std::result::Result<Option<BlindSpotAssessment>, String> {
    crate::settings::require_signal_feature("get_cached_blind_spot_assessment")
        .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(|| {
        load_persisted().map(|p| {
            let current = gather_inputs().ok().map(|(_, key)| key);
            cached_view(p, current.as_deref())
        })
    })
    .await
    .map_err(|e| format!("blind spot cache task failed: {e}"))
}

#[cfg(test)]
#[path = "assess_tests.rs"]
mod tests;
