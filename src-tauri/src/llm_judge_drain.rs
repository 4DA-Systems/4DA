// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Pending-verdict drain — the retry lane for starved deferred flips.
//!
//! Phase 109's damper defers unreasoned verdict flips into
//! `source_items.feed_verdict_pending` and waits for a second judging run.
//! That second run only ever comes from the analysis cycle's recency-bounded
//! working set — so a deferred flip on an item that ages out of the window
//! waits forever. Measured live 2026-08-31 on the founder's 59k-item
//! instance: **175 pending markers, 135 pending more than 2 days, 47 more
//! than 7 days, the oldest deferred 2026-08-12 and never revisited once.**
//!
//! The drain runs after every post-cycle judge pass and works the backlog
//! OLDEST first (`created_at ASC` — the exact inversion of the freshness
//! preference that caused the starvation):
//!
//! 1. **Terminal resolution (no LLM, no budget needed)** — a marker that has
//!    used its whole retry budget ([`MAX_DRAIN_ATTEMPTS`] visits spread over
//!    more than [`MIN_EXHAUST_AGE_DAYS`]) resolves demote-only:
//!    `feed_relevant = 0`, `feed_verdict_source = 'fallback_exhausted'`,
//!    reason `pending_retries_exhausted`. A pending PROMOTE exhausts to the
//!    standing 0 — **never promote without a real verdict**. Corrupt markers
//!    are cleared (Phase-109 doctrine: rewritten, never trusted).
//! 2. **Re-judge slice** — up to [`DRAIN_SLICE`] of the oldest still-pending
//!    items (20% of the fresh judge lane's 40-item selection) get one cheap
//!    LLM read on the judge sibling model ([`crate::llm_judge::judge_provider`]).
//!    A confident REJECT resolves the flip as a real `llm_reject` demotion; a
//!    confident RELEVANT read that disputes a pending demote clears the marker
//!    (the standing curated verdict is re-affirmed); anything ambiguous
//!    escalates the marker's attempt count and waits for the next cycle.
//!
//! Deliberately untouched: the serendipity paths
//! (`scoring::dedup::compute_serendipity_candidates`, the deep-scan 0.45
//! injection) — their verdicts never carry pending markers in the first place
//! (`VerdictSource::Serendipity` writes are immediate at the boundary) — and
//! scoring values themselves: this lane changes retry SCHEDULING and terminal
//! bookkeeping on the `feed_verdict_*` columns only, so no `PIPELINE_VERSION`
//! bump.

use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::db::{Database, PendingVerdictRow, VerdictReason, VerdictSource};
use crate::llm::{LLMClient, Message};
use crate::settings::LLMProvider;

/// Items per drain pass that get an LLM re-judgment: 20% of the fresh judge
/// lane's selection (`llm_judgments::BATCH_SIZE * 4` = 40). A true CARVE-OUT,
/// not an add-on: whenever a backlog exists, the fresh lane cedes this many
/// selection slots (`llm_judgments::drain_reserve`), so the cycle's total
/// judge volume stays constant. Runs on the cheap sibling model behind the
/// same budget gates as every other judge call.
pub(crate) const DRAIN_SLICE: usize = 8;

/// Drain visits a marker may consume before it resolves terminally…
pub(crate) const MAX_DRAIN_ATTEMPTS: u32 = 8;

/// …provided the flip has ALSO been pending this long. Both gates together:
/// a young marker cannot burn through its attempts in an afternoon and get
/// terminally resolved before the analysis cycle ever had a fair chance to
/// confirm it.
pub(crate) const MIN_EXHAUST_AGE_DAYS: i64 = 7;

/// How deep the terminal-resolution scan reads the backlog each pass. Live
/// backlog is 175; this bounds the scan without ever leaving exhausted
/// markers waiting more than a couple of cycles.
const EXHAUST_SCAN_LIMIT: usize = 512;

/// Judgments this drain writes are stamped with their own prompt version so
/// the main lane's demotion gate (which filters on ITS prompt version) can
/// never double-count them, and post-hoc analysis can see the cohort.
const DRAIN_PROMPT_VERSION: &str = "drain_v1";

/// A drain judgment DISPUTES a pending demote only at or above this judged
/// relevance (plus the shared confidence bar). Between the reject line and
/// this line the judge is shrugging — that escalates, never resolves.
const CONFIRM_RELEVANCE_MIN: f64 = 0.5;

/// What one drain visit decided for one item.
#[derive(Debug, PartialEq, Eq)]
enum DrainAction {
    /// Real verdict: confident reject → demote via the persist boundary.
    Demote,
    /// Real verdict the other way, against a pending DEMOTE: the standing
    /// curated verdict is re-affirmed, the deferred flip dies.
    ClearPending,
    /// No usable evidence this visit — count it and wait for the next cycle.
    Escalate,
}

/// Outcome of one drain pass. Log/test consumption only — no UI surface
/// (Intelligence Doctrine rule 3).
#[derive(Debug, Default)]
pub(crate) struct DrainSummary {
    pub judged: usize,
    pub demoted: usize,
    pub confirmed: usize,
    pub escalated: usize,
    pub exhausted: usize,
    pub corrupt_cleared: usize,
    pub skipped: Option<&'static str>,
}

/// Scheduled/headless entry point, run right after the post-cycle judge pass.
pub(crate) async fn run_pending_verdict_drain(db: &Database) -> DrainSummary {
    run_drain_with(db, crate::state::is_llm_limit_reached(), drain_provider()).await
}

/// Same BYOK gate as the main judge lane (`llm_judgments::get_llm_settings`,
/// which is private to its claimed module): configured provider, key present
/// unless Ollama, bulk work routed to the cheap judge sibling.
fn drain_provider() -> Option<LLMProvider> {
    let mgr = crate::get_settings_manager();
    let mut guard = mgr.lock();
    guard.ensure_keys_hydrated();
    let provider = crate::llm_judge::judge_provider(&guard.get().llm);
    if provider.provider != "ollama" && provider.api_key.is_empty() {
        return None;
    }
    Some(provider)
}

/// Gate-injectable inner pass (mirrors `llm_judgments::run_post_cycle_with`):
/// hermetic tests drive the budget/BYOK gates directly instead of mutating
/// process-global counters.
async fn run_drain_with(
    db: &Database,
    llm_limit_reached: bool,
    provider: Option<LLMProvider>,
) -> DrainSummary {
    let mut summary = DrainSummary::default();

    let backlog = match db.get_pending_verdict_backlog(EXHAUST_SCAN_LIMIT) {
        Ok(rows) => rows,
        Err(e) => {
            warn!(target: "4da::verdict_drain", error = %e, "Failed to read pending-verdict backlog");
            return summary;
        }
    };
    if backlog.is_empty() {
        return summary;
    }

    // Phase A — terminal resolution + corrupt-marker hygiene. Pure DB work:
    // runs even with no provider or an exhausted budget, because a backlog
    // must keep draining precisely when the LLM lane is unavailable.
    let now = chrono::Utc::now();
    let corrupt: Vec<i64> = backlog
        .iter()
        .filter(|r| r.marker.is_none())
        .map(|r| r.id)
        .collect();
    let exhausted: Vec<i64> = backlog
        .iter()
        .filter(|r| {
            r.marker.is_some_and(|m| {
                m.attempts >= MAX_DRAIN_ATTEMPTS
                    && (now - m.first_seen).num_days() >= MIN_EXHAUST_AGE_DAYS
            })
        })
        .map(|r| r.id)
        .collect();

    match db.clear_pending_markers(&corrupt) {
        Ok(n) => summary.corrupt_cleared = n,
        Err(e) => {
            warn!(target: "4da::verdict_drain", error = %e, "Failed to clear corrupt pending markers")
        }
    }
    if !exhausted.is_empty() {
        let verdicts: Vec<(i64, bool, VerdictSource, Option<VerdictReason>)> = exhausted
            .iter()
            .map(|&id| {
                (
                    id,
                    false,
                    VerdictSource::FallbackExhausted,
                    Some(VerdictReason::PendingRetriesExhausted),
                )
            })
            .collect();
        match db.persist_feed_verdicts_with_reasons(&verdicts, crate::scoring::PIPELINE_VERSION) {
            Ok(n) => summary.exhausted = n,
            Err(e) => {
                warn!(target: "4da::verdict_drain", error = %e, "Terminal resolution failed")
            }
        }
    }

    // Phase B — one cheap batched re-judgment for the oldest workable slice.
    if llm_limit_reached {
        summary.skipped = Some("llm_budget_reached");
        log_summary(db, &summary);
        return summary;
    }
    let Some(provider) = provider else {
        summary.skipped = Some("no_llm_provider");
        log_summary(db, &summary);
        return summary;
    };

    let slice: Vec<&PendingVerdictRow> = backlog
        .iter()
        .filter(|r| {
            // Corrupt and exhausted rows were resolved above; markers already
            // at max attempts are only waiting out the age gate — burning
            // another call on them cannot change anything.
            r.marker.is_some_and(|m| m.attempts < MAX_DRAIN_ATTEMPTS)
        })
        .take(DRAIN_SLICE)
        .collect();
    if slice.is_empty() {
        log_summary(db, &summary);
        return summary;
    }

    let ids: Vec<i64> = slice.iter().map(|r| r.id).collect();
    let items = match load_items(db, &ids) {
        Ok(items) => items,
        Err(e) => {
            warn!(target: "4da::verdict_drain", error = %e, "Failed to load drain items");
            log_summary(db, &summary);
            return summary;
        }
    };

    let model_name = provider.model.clone();
    let client = LLMClient::with_purpose(provider, "verdict_drain");
    let judgments = match judge_items(&client, &items).await {
        Ok(j) => j,
        Err(e) => {
            // A failed call consumes no attempt: no evidence was obtained, so
            // the markers are left exactly as found for the next cycle.
            warn!(target: "4da::verdict_drain", error = %e, "Drain re-judgment call failed — no attempts consumed");
            log_summary(db, &summary);
            return summary;
        }
    };

    let mut demote: Vec<(i64, bool, VerdictSource, Option<VerdictReason>)> = Vec::new();
    let mut clear: Vec<i64> = Vec::new();
    let mut escalate: Vec<i64> = Vec::new();
    for row in &slice {
        let Some(judged) = judgments.iter().find(|j| j.id == Some(row.id)) else {
            // The model dropped this item from its reply: no evidence, no
            // attempt consumed.
            continue;
        };
        summary.judged += 1;
        let relevance = judged.relevance.unwrap_or(0.0).clamp(0.0, 1.0);
        let confidence = judged.confidence.unwrap_or(0.0).clamp(0.0, 1.0);
        if let Err(e) = db.upsert_llm_judgment(
            row.id,
            relevance,
            judged.reason.as_deref().unwrap_or_default(),
            None,
            confidence,
            &model_name,
            DRAIN_PROMPT_VERSION,
        ) {
            warn!(target: "4da::verdict_drain", error = %e, item_id = row.id, "Failed to store drain judgment");
        }
        let direction = row.marker.map(|m| m.direction);
        match resolve_action(direction, relevance, confidence) {
            DrainAction::Demote => demote.push((
                row.id,
                false,
                VerdictSource::Score,
                Some(VerdictReason::LlmReject),
            )),
            DrainAction::ClearPending => clear.push(row.id),
            DrainAction::Escalate => escalate.push(row.id),
        }
    }

    match db.persist_feed_verdicts_with_reasons(&demote, crate::scoring::PIPELINE_VERSION) {
        Ok(n) => summary.demoted = n,
        Err(e) => warn!(target: "4da::verdict_drain", error = %e, "Drain demotions failed"),
    }
    match db.clear_pending_markers(&clear) {
        Ok(n) => summary.confirmed = n,
        Err(e) => warn!(target: "4da::verdict_drain", error = %e, "Drain confirmations failed"),
    }
    match db.escalate_pending_attempts(&escalate) {
        Ok(n) => summary.escalated = n,
        Err(e) => warn!(target: "4da::verdict_drain", error = %e, "Drain escalation failed"),
    }

    log_summary(db, &summary);
    summary
}

/// One decision table, pure so the safety boundary is unit-testable:
///
/// - Confident reject (shares the main lane's `DEMOTION_*` bars) → a real
///   demote verdict, whichever direction was pending.
/// - Confident relevant AND the pending flip was a DEMOTE → the flip is
///   disputed; the standing curated verdict survives and the marker dies.
///   A pending PROMOTE never resolves upward here — promotion needs a full
///   run's dedup/diversity/rerank context — so a confident-relevant read on
///   one merely escalates.
/// - Anything else → escalate.
fn resolve_action(pending_direction: Option<bool>, relevance: f64, confidence: f64) -> DrainAction {
    let confident = confidence >= crate::llm_judgments::DEMOTION_CONFIDENCE_MIN;
    if confident && relevance < crate::llm_judgments::DEMOTION_RELEVANCE_BELOW {
        return DrainAction::Demote;
    }
    if confident && relevance >= CONFIRM_RELEVANCE_MIN && pending_direction == Some(false) {
        return DrainAction::ClearPending;
    }
    DrainAction::Escalate
}

fn log_summary(db: &Database, summary: &DrainSummary) {
    let remaining = db.count_pending_verdicts().unwrap_or(-1);
    if summary.judged > 0
        || summary.exhausted > 0
        || summary.corrupt_cleared > 0
        || summary.skipped.is_some()
    {
        info!(
            target: "4da::verdict_drain",
            judged = summary.judged,
            demoted = summary.demoted,
            confirmed = summary.confirmed,
            escalated = summary.escalated,
            exhausted = summary.exhausted,
            corrupt_cleared = summary.corrupt_cleared,
            skipped = summary.skipped.unwrap_or("none"),
            backlog_remaining = remaining,
            "Pending-verdict drain pass complete"
        );
    } else {
        debug!(target: "4da::verdict_drain", backlog_remaining = remaining, "Pending-verdict drain: nothing to do");
    }
}

// ============================================================================
// Item loading + the re-judge call
// ============================================================================

struct DrainItem {
    id: i64,
    title: String,
    content: Option<String>,
    source_type: String,
}

fn load_items(db: &Database, ids: &[i64]) -> rusqlite::Result<Vec<DrainItem>> {
    let conn = db.conn.lock();
    let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT id, title, content, source_type FROM source_items WHERE id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::types::ToSql> = ids
        .iter()
        .map(|id| id as &dyn rusqlite::types::ToSql)
        .collect();
    let rows = stmt.query_map(params.as_slice(), |row| {
        Ok(DrainItem {
            id: row.get(0)?,
            title: row.get(1)?,
            content: row.get(2)?,
            source_type: row.get(3)?,
        })
    })?;
    rows.collect()
}

/// The drain judges on the same cheap sibling model as the ingest lane, and
/// #559 caught that model live quoting numbers (`"id": "60146"`) — strict
/// serde then failed the WHOLE batch with the budget already spent. The
/// lenient deserializers are shared with `llm_judgments`, never forked.
#[derive(Debug, Deserialize)]
struct DrainJudgment {
    /// Optional so one id-less element degrades to a drop, not a parse
    /// failure for the whole batch (same doctrine as the main judge lane).
    #[serde(default, deserialize_with = "crate::llm_judgments::de_i64_lenient")]
    id: Option<i64>,
    #[serde(default, deserialize_with = "crate::llm_judgments::de_f64_lenient")]
    relevance: Option<f64>,
    #[serde(default, deserialize_with = "crate::llm_judgments::de_f64_lenient")]
    confidence: Option<f64>,
    reason: Option<String>,
}

/// Lean re-judge: relevance + confidence + a short reason, nothing else — the
/// drain needs a verdict, not a content analysis. Untrusted item text goes
/// through the same `<source_item>` framing as every other judge prompt.
async fn judge_items(
    client: &LLMClient,
    items: &[DrainItem],
) -> crate::error::Result<Vec<DrainJudgment>> {
    let user_context = crate::adversarial::build_user_context_summary();
    let items_block: String = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let preview: String = item
                .content
                .as_deref()
                .unwrap_or_default()
                .chars()
                .take(400)
                .collect();
            crate::prompt_safety::wrap_untrusted_item(
                i + 1,
                &format!("{} ({})", item.id, item.source_type),
                &item.title,
                &preview,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let system_prompt = format!(
        "{defense}\n\n\
         You are re-evaluating BACKLOGGED items for a developer-intelligence \
         tool — items whose earlier relevance verdict is in doubt.\n\n\
         {user_context}\n\n\
         For each item respond with a JSON array element:\n\
         - \"id\": the numeric id from the item's id attribute (the number before the parenthesis)\n\
         - \"relevance\": 0.0-1.0 for THIS user specifically\n\
         - \"confidence\": 0.0-1.0 in your assessment\n\
         - \"reason\": one sentence, MAX 15 words\n\n\
         Rules:\n\
         - If the item has no clear connection to the user's stack/topics, relevance < 0.3\n\
         - A package only affects the user if it is actually in their stack; never claim \
           cross-ecosystem impact. Unconfirmed -> relevance < 0.3\n\
         - Return ONLY a valid JSON array, no other text",
        defense = crate::prompt_safety::UNTRUSTED_CONTENT_DEFENSE_CLAUSE,
    );
    let messages = vec![Message {
        role: "user".to_string(),
        content: format!("Re-evaluate these items:\n\n{items_block}"),
    }];

    let response = client.complete(&system_prompt, messages).await?;
    parse_drain_response(&response.content)
}

/// Parse the model's reply. Tolerates a markdown fence; elements with a
/// non-numeric or missing id are dropped (nothing to attach them to); a reply
/// that is not a JSON array at all is an error the caller treats as
/// "no attempts consumed".
fn parse_drain_response(text: &str) -> crate::error::Result<Vec<DrainJudgment>> {
    let trimmed = text.trim();
    let json_text = if trimmed.starts_with("```") {
        trimmed
            .lines()
            .skip(1)
            .take_while(|l| !l.starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        trimmed.to_string()
    };
    let parsed: Vec<DrainJudgment> = serde_json::from_str(&json_text)?;
    Ok(parsed)
}

#[cfg(test)]
#[path = "llm_judge_drain_tests.rs"]
mod tests;
