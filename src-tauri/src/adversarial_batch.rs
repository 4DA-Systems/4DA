// SPDX-License-Identifier: FSL-1.1-Apache-2.0

//! Applying deliberation to a batch: cached verdicts first, the model only
//! for material it has not judged, and fail-closed when it cannot judge.
//!
//! Decision table per item (OSV-verified items bypass all of it):
//!
//! | Verdict source            | Result                                          |
//! |---------------------------|-------------------------------------------------|
//! | cached or fresh, surface  | surfaces with the grounded explanation          |
//! | cached or fresh, reject   | dropped (Critical/High: surfaces unchanged)     |
//! | none, model unavailable   | unvetted heuristic items are HELD BACK;         |
//! |                           | LLM-assessed and Critical/High items surface    |
//! | none, no model configured | surfaces as before (deterministic gate only)    |
//!
//! "Unavailable" covers the background budget share being spent, a failed
//! provider call and an unreadable answer. A rejected item therefore stays
//! suppressed while its verdict is valid, whatever the budget or provider is
//! doing (audit L2: rejected chains came back once the budget ran out).

use std::future::Future;

use tracing::{debug, warn};

use super::cache::VerdictCache;
use super::{
    apply_verdict, DeliberationOutcome, DeliberationVerdict, Unavailable, VerdictApplication,
};
use crate::evidence::{Confidence, ConfidenceProvenance, EvidenceItem, Urgency};

/// What one batch did, for the summary log line and tests.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct BatchStats {
    /// OSV-verified items, never deliberated.
    pub bypassed: usize,
    /// Items decided by a stored verdict, with no model call.
    pub cache_hits: usize,
    /// Model calls made.
    pub llm_calls: usize,
    /// Items dropped by a verdict (cached or fresh).
    pub filtered: usize,
    /// Unjudged items held back because the model could not judge them.
    pub held_back: usize,
    /// Why the model could not judge, when it could not.
    pub unavailable: Option<Unavailable>,
}

#[derive(Debug, Default)]
pub(crate) struct BatchOutcome {
    pub passed: Vec<EvidenceItem>,
    pub stats: BatchStats,
}

/// An item is only "must surface" when its Critical/High escalation survived
/// the corroboration gate (which runs before any batch).
fn must_surface(item: &EvidenceItem) -> bool {
    item.urgency == Urgency::Critical || item.urgency == Urgency::High
}

/// Fail closed for unvetted heuristics: an item whose only backing is a
/// weighted formula or keyword match (in Preemption, a signal chain) is not
/// shown until the model has vetted it. Doctrine principle 5, accurate
/// first: never show intelligence the system cannot stand behind. LLM-judged
/// items were already vetted by a model upstream, and a corroborated
/// Critical/High item keeps its safety floor, so neither is held back.
pub(crate) fn holds_back_unjudged(item: &EvidenceItem) -> bool {
    !must_surface(item)
        && matches!(
            item.confidence.provenance,
            ConfidenceProvenance::Heuristic | ConfidenceProvenance::Checklist
        )
}

impl BatchOutcome {
    fn apply(&mut self, item: EvidenceItem, verdict: &DeliberationVerdict) {
        match apply_verdict(must_surface(&item), verdict.should_surface) {
            VerdictApplication::SurfaceUpdated => {
                let mut updated = item;
                updated.explanation = verdict.grounded_explanation.clone();
                updated.confidence =
                    Confidence::llm_assessed(verdict.adjusted_confidence.clamp(0.0, 1.0));
                self.passed.push(updated);
            }
            VerdictApplication::SurfaceUnchanged => {
                // Surfaced by the safety floor over the model's dissent: the
                // dissent filters, it never rewrites the evidence.
                warn!(
                    target: "4da::adversarial",
                    item_id = %item.id,
                    title = %item.title,
                    adjusted_confidence = verdict.adjusted_confidence,
                    "Critical/High item surfaced despite dissenting verdict; original explanation kept"
                );
                self.passed.push(item);
            }
            VerdictApplication::Filter => {
                self.stats.filtered += 1;
                debug!(
                    target: "4da::adversarial",
                    item_id = %item.id,
                    title = %item.title,
                    "Item filtered by adversarial deliberation"
                );
            }
        }
    }

    fn unjudged(&mut self, item: EvidenceItem) {
        if holds_back_unjudged(&item) {
            self.stats.held_back += 1;
            debug!(
                target: "4da::adversarial",
                item_id = %item.id,
                title = %item.title,
                "Unjudged heuristic item held back until the model can vet it"
            );
        } else {
            self.passed.push(item);
        }
    }

    /// Settle `item` without the model if possible. Returns the item and
    /// its cache key when it still needs a live verdict.
    fn settle_cached<K>(
        &mut self,
        item: EvidenceItem,
        cache: &VerdictCache,
        keyer: &K,
        now: i64,
    ) -> Option<(EvidenceItem, String)>
    where
        K: Fn(&EvidenceItem) -> String,
    {
        if item.confidence.provenance == ConfidenceProvenance::OsvVerified {
            self.stats.bypassed += 1;
            self.passed.push(item);
            return None;
        }
        let key = keyer(&item);
        if let Some(cached) = cache.fresh(&key, now) {
            self.stats.cache_hits += 1;
            let verdict = cached.verdict.clone();
            self.apply(item, &verdict);
            return None;
        }
        Some((item, key))
    }
}

/// Deliberate a batch: stored verdicts first, `live` for the rest. After a
/// budget stop or a provider failure the rest of the batch is not sent (one
/// failure is not retried per item) and is settled fail-closed.
pub(crate) async fn deliberate_batch<K, F, Fut>(
    items: Vec<EvidenceItem>,
    cache: &mut VerdictCache,
    keyer: K,
    now: i64,
    mut live: F,
) -> BatchOutcome
where
    K: Fn(&EvidenceItem) -> String,
    F: FnMut(EvidenceItem) -> Fut,
    Fut: Future<Output = DeliberationOutcome>,
{
    let mut out = BatchOutcome::default();
    let mut stopped: Option<Unavailable> = None;
    for item in items {
        let Some((item, key)) = out.settle_cached(item, cache, &keyer, now) else {
            continue;
        };
        let outcome = match stopped {
            Some(reason) => DeliberationOutcome::Unavailable(reason),
            None => {
                out.stats.llm_calls += 1;
                live(item.clone()).await
            }
        };
        match outcome {
            DeliberationOutcome::Verdict(verdict) => {
                cache.insert(key, verdict.clone(), now);
                out.apply(item, &verdict);
            }
            DeliberationOutcome::Unavailable(reason) => {
                if reason.stops_batch() {
                    stopped = Some(reason);
                }
                out.stats.unavailable = Some(reason);
                out.unjudged(item);
            }
            DeliberationOutcome::NotConfigured => out.passed.push(item),
        }
    }
    out
}

/// The no-model path (the cache-miss fast feed): stored verdicts apply,
/// and anything not yet judged is settled as if the model were unavailable.
/// The deliberated refresh started beside it adds what the model approves.
pub(crate) fn settle_from_cache<K>(
    items: Vec<EvidenceItem>,
    cache: &VerdictCache,
    keyer: K,
    now: i64,
) -> BatchOutcome
where
    K: Fn(&EvidenceItem) -> String,
{
    let mut out = BatchOutcome::default();
    for item in items {
        if let Some((item, _)) = out.settle_cached(item, cache, &keyer, now) {
            out.unjudged(item);
        }
    }
    out
}
