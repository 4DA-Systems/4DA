// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Judge-gated admission: an item from a low-yield editorial source enters
//! the feed only on a project-aware judge's say-so.
//!
//! Measured 2026-09-27 on the SHOWN feed (308 items, 14 days, AI-labelled
//! gold, two blind labellers at kappa 0.78, 20/20 calibration items stable):
//! 12.7% useful. dev.to held 128 of the 308 slots at 4 useful, lobsters 1 of
//! 40, mastodon 7 of 46 and reddit 1 of 15. On those 228 items the pipeline
//! score that decides admission separates useful from noise at AUC 0.593.
//! The project-card judge (`project_cards`, gemma4:26b, one item per call)
//! does it at 0.906. At judge relevance >= 0.5 it keeps 12 of the 13 useful
//! items and 63 of the 228 slots: precision 5.7% -> 19% on these sources.
//! The same test with the thin 10-tech context (2026-09-25) found no usable
//! bar, so only card-aware judgments ([`CARD_PROMPT_VERSIONS`]) count.
//!
//! Sources not listed keep deciding by score. Registry releases, security
//! advisories, HN and RSS are either deterministic (release grading, OSV) or
//! already far more precise.
//!
//! The rule is applied at the verdict persist boundary
//! (`Database::persist_feed_verdicts_with_reasons`), which every admission
//! path shares, and repaired by `Database::reconcile_judge_gate`.
//! - An item without a card-aware judgment waits (`awaiting_judge`). An item
//!   already in the feed keeps its place until it is judged.
//! - Judged below [`GATE_RELEVANCE`], the item is `llm_reject`.
//! - With no judge available at all, the gate is off: the feed must never
//!   starve because no model can judge it.

use crate::db::VerdictReason;

/// Sources whose items need a card-aware judgment to enter the feed.
pub(crate) const GATED_SOURCES: &[&str] = &["devto", "lobsters", "mastodon", "reddit"];

/// Minimum card-aware judge relevance for admission (see module doc).
pub(crate) const GATE_RELEVANCE: f64 = 0.5;

/// Judgments written with project cards as context: the ingest judge and the
/// pending-verdict drain from 2026-09-26 on.
pub(crate) const CARD_PROMPT_VERSIONS: [&str; 2] = [
    crate::llm_judgments::PROMPT_VERSION,
    crate::llm_judge::drain::DRAIN_PROMPT_VERSION,
];

pub(crate) fn is_gated_source(source_type: &str) -> bool {
    GATED_SOURCES.contains(&source_type)
}

/// SQL list literal of [`GATED_SOURCES`], for queries.
pub(crate) fn gated_sources_sql() -> String {
    GATED_SOURCES
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// What the gate makes of a promotion of a gated item. `None` means the
/// promotion stands. `Some(reason)` means it becomes a reasoned rejection.
pub(crate) fn decide(judgment: Option<f64>, already_curated: bool) -> Option<VerdictReason> {
    match judgment {
        Some(r) if r >= GATE_RELEVANCE => None,
        Some(_) => Some(VerdictReason::LlmReject),
        // Kept until judged: the reconcile sweep demotes it if the judgment
        // comes back below the bar.
        None if already_curated => None,
        None => Some(VerdictReason::AwaitingJudge),
    }
}

#[cfg(test)]
thread_local! {
    static TEST_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Tests: switch the gate on for this thread. It is off by default so no
/// existing verdict test depends on the settings of the machine running it.
#[cfg(test)]
pub(crate) fn set_active_for_test(on: bool) {
    TEST_ACTIVE.with(|c| c.set(on));
}

/// Whether a judge can run at all: the judge provider is configured (a key,
/// or Ollama) and holds the feed-judging bar. Read BEFORE taking a database
/// lock, never inside one.
pub(crate) fn active() -> bool {
    #[cfg(test)]
    {
        TEST_ACTIVE.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        let provider = {
            let mgr = crate::get_settings_manager();
            let mut guard = mgr.lock();
            guard.ensure_keys_hydrated();
            crate::llm_judge::judge_provider(&guard.get().llm)
        };
        crate::llm_gate::compute_has_llm(&provider.provider, &provider.api_key)
            && crate::llm_judgments::judge_items_per_call(&provider).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decide_applies_the_bar_and_grandfathers_unjudged_curated_items() {
        assert_eq!(decide(Some(0.8), false), None);
        assert_eq!(decide(Some(0.5), false), None, "the bar is inclusive");
        assert_eq!(decide(Some(0.3), true), Some(VerdictReason::LlmReject));
        assert_eq!(decide(None, false), Some(VerdictReason::AwaitingJudge));
        assert_eq!(
            decide(None, true),
            None,
            "a curated item keeps its place until judged"
        );
    }

    #[test]
    fn only_the_measured_low_yield_sources_are_gated() {
        for s in ["devto", "lobsters", "mastodon", "reddit"] {
            assert!(is_gated_source(s));
        }
        for s in [
            "hackernews",
            "rss",
            "crates_io",
            "npm_registry",
            "osv",
            "github",
        ] {
            assert!(!is_gated_source(s));
        }
    }
}
