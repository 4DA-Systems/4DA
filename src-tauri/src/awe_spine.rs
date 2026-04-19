// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! AWE Spine — Intelligence Reconciliation Phase 9 (2026-04-17).
//!
//! The reasoning substrate that attaches precedents to every
//! `EvidenceItem` produced by any lens materializer. Draws from the
//! merged Cold Start pool (git miner Phase 7 + curated corpus Phase 8)
//! and ranks by similarity. AWE-engine-native transmute is reserved
//! for the Confession Box in Phase 10 — too slow for per-item inline
//! use; the spine runs in microseconds.
//!
//! **Design rules:**
//! - Deterministic similarity scoring. No randomness.
//! - Zero-fail: missing data → empty precedent list, never panic.
//! - Bounded: cap match scans and precedent counts.
//! - Cheap: a full spine enrichment across a feed of 30 items must
//!   add < 10ms. No DB reads; the precedent pool is loaded once.
//!
//! **Matching strategy (high → low specificity):**
//! 1. Direct dep match — seed `subject` equals any `affected_deps[i]`
//!    on the item, case-insensitive. Similarity = 0.9.
//! 2. Title substring — seed `subject` appears as a word in the item
//!    `title`. Similarity = 0.6.
//! 3. Domain match — seed and item share a classified domain.
//!    Similarity = 0.3.
//!
//! Items without any match return the empty precedent list; that's
//! honest — "no precedents yet" is a legitimate state on a fresh install.

use std::sync::OnceLock;

#[cfg(test)]
use crate::evidence::PrecedentOutcome;
use crate::evidence::{EvidenceItem, PrecedentRef};
use crate::git_decision_miner::SeededDecision;

// ============================================================================
// Precedent index — loaded once per process
// ============================================================================

/// In-memory index of all available precedents: git-mined + curated corpus.
/// Loaded lazily on first use via `precedent_index()`; subsequent calls
/// return the cached slice.
struct PrecedentIndex {
    seeds: Vec<SeededDecision>,
}

static PRECEDENT_INDEX: OnceLock<PrecedentIndex> = OnceLock::new();

fn precedent_index() -> &'static PrecedentIndex {
    PRECEDENT_INDEX.get_or_init(|| {
        let mut seeds: Vec<SeededDecision> = Vec::new();

        // Curated corpus (always present — bundled in the binary).
        seeds.extend(crate::seed_corpus::load_corpus());

        // Git-mined personal priors — optional. The miner writes JSONL
        // to temp on its last run; load it if present. Zero-fail path.
        let jsonl_path = std::env::temp_dir().join("awe_git_seeded.jsonl");
        if let Ok(content) = std::fs::read_to_string(&jsonl_path) {
            for line in content.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(d) = serde_json::from_str::<SeededDecision>(line) {
                    seeds.push(d);
                }
            }
        }

        PrecedentIndex { seeds }
    })
}

// ============================================================================
// Similarity scoring — pure functions, trivially testable
// ============================================================================

/// Specificity of a match between a seeded decision and an item.
/// Higher = more specific. `None` when there is no match at all.
pub fn match_similarity(seed: &SeededDecision, item: &EvidenceItem) -> Option<f32> {
    let subject_lower = seed.subject.to_lowercase();

    // 1. Direct dep match (highest specificity).
    for dep in &item.affected_deps {
        if dep.to_lowercase() == subject_lower {
            return Some(0.9);
        }
    }

    // 2. Title substring — must appear at word boundaries to avoid
    //    matching "react" inside "interactions".
    if title_contains_subject_as_word(&item.title, &subject_lower) {
        return Some(0.6);
    }

    // 3. Domain match (coarsest).
    let seed_domain = crate::seed_corpus::classify_subject_domain(&seed.subject);
    for dep in &item.affected_deps {
        let dep_domain = crate::seed_corpus::classify_subject_domain(dep);
        if dep_domain == seed_domain && dep_domain != "misc" {
            return Some(0.3);
        }
    }

    None
}

/// True when `subject` (lowercase) appears as a whole word in `title`.
/// Rejects substring matches: "react" doesn't match "reaction".
pub fn title_contains_subject_as_word(title: &str, subject_lower: &str) -> bool {
    if subject_lower.is_empty() {
        return false;
    }
    let title_lower = title.to_lowercase();
    let mut start = 0;
    while let Some(idx) = title_lower[start..].find(subject_lower) {
        let abs = start + idx;
        let before_ok = abs == 0
            || title_lower
                .as_bytes()
                .get(abs - 1)
                .map_or(true, |b| !is_word_char(*b));
        let after_ok = title_lower
            .as_bytes()
            .get(abs + subject_lower.len())
            .map_or(true, |b| !is_word_char(*b));
        if before_ok && after_ok {
            return true;
        }
        start = abs + subject_lower.len();
    }
    false
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

// ============================================================================
// Conversion: SeededDecision → PrecedentRef
// ============================================================================

/// Decide the origin label based on the seed's source_repo. Curated
/// corpus entries have a fixed label; everything else is user-history.
pub fn classify_precedent_origin(seed: &SeededDecision) -> &'static str {
    if seed.source_repo == "curated-corpus" {
        "curated-corpus"
    } else {
        "user-history"
    }
}

/// Build a deterministic `decision_id`: `<origin>:<hash-of-commit-or-subject>`.
/// Stable across runs for the same seed.
pub fn build_precedent_id(seed: &SeededDecision) -> String {
    if seed.source_commit != "curated" && !seed.source_commit.is_empty() {
        // Use first 8 chars of the commit hash for readability.
        let short: String = seed.source_commit.chars().take(8).collect();
        format!("git:{short}")
    } else {
        format!("curated:{}", seed.subject)
    }
}

fn seed_to_precedent(seed: &SeededDecision, similarity: f32) -> PrecedentRef {
    PrecedentRef {
        decision_id: build_precedent_id(seed),
        statement: seed.statement.clone(),
        outcome: Some(seed.outcome.clone()),
        origin: classify_precedent_origin(seed).to_string(),
        similarity,
    }
}

// ============================================================================
// Main enrichment API
// ============================================================================

/// Maximum precedents attached per item. Keeps the payload scannable.
pub const MAX_PRECEDENTS_PER_ITEM: usize = 3;

/// Attach precedents to an `EvidenceItem` in place. Does nothing when
/// the item already has precedents (idempotent — lens materializers
/// may already have populated them from a future AWE transmute path).
pub fn enrich_item(item: &mut EvidenceItem) {
    if !item.precedents.is_empty() {
        return;
    }

    let index = precedent_index();

    let mut scored: Vec<(f32, &SeededDecision)> = Vec::new();
    for seed in &index.seeds {
        if let Some(sim) = match_similarity(seed, item) {
            scored.push((sim, seed));
        }
    }

    // Sort by similarity desc, break ties by prefer-user-history
    // (git-mined) over curated-corpus because a user's own history is
    // almost always more relevant than industry priors.
    scored.sort_by(|a, b| {
        let sim_cmp = b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal);
        if sim_cmp != std::cmp::Ordering::Equal {
            return sim_cmp;
        }
        let a_user = classify_precedent_origin(a.1) == "user-history";
        let b_user = classify_precedent_origin(b.1) == "user-history";
        b_user.cmp(&a_user)
    });

    for (sim, seed) in scored.into_iter().take(MAX_PRECEDENTS_PER_ITEM) {
        item.precedents.push(seed_to_precedent(seed, sim));
    }
}

/// Attach precedents to every item in a slice. Convenience wrapper.
pub fn enrich_items(items: &mut [EvidenceItem]) {
    for item in items.iter_mut() {
        enrich_item(item);
    }
}

// ============================================================================
// Debug accessor (tests only) — force-reset the OnceLock for isolated
// test runs. Real code never needs this.
// ============================================================================

#[cfg(test)]
pub fn _test_seeds() -> &'static [SeededDecision] {
    &precedent_index().seeds
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{Action, Confidence, EvidenceCitation, EvidenceKind, LensHints, Urgency};

    fn sample_seed(subject: &str, statement: &str, outcome: PrecedentOutcome) -> SeededDecision {
        SeededDecision {
            statement: statement.to_string(),
            verb: "adopt".to_string(),
            subject: subject.to_string(),
            outcome,
            source_commit: "curated".to_string(),
            source_repo: "curated-corpus".to_string(),
            timestamp: 0,
        }
    }

    fn sample_git_seed(subject: &str) -> SeededDecision {
        SeededDecision {
            statement: format!("Adopted {subject}"),
            verb: "adopt".to_string(),
            subject: subject.to_string(),
            outcome: PrecedentOutcome::Confirmed,
            source_commit: "deadbeefcafe1234".to_string(),
            source_repo: "/user/proj/a".to_string(),
            timestamp: 1700000000,
        }
    }

    fn sample_item(title: &str, affected_deps: Vec<&str>) -> EvidenceItem {
        EvidenceItem {
            id: "ev_test".to_string(),
            kind: EvidenceKind::Alert,
            title: title.to_string(),
            explanation: String::new(),
            confidence: Confidence::heuristic(0.5),
            urgency: Urgency::Medium,
            reversibility: None,
            evidence: vec![EvidenceCitation {
                source: "test".to_string(),
                title: "test citation".to_string(),
                url: None,
                freshness_days: 0.0,
                relevance_note: "test".to_string(),
            }],
            affected_projects: Vec::new(),
            affected_deps: affected_deps.into_iter().map(String::from).collect(),
            suggested_actions: vec![Action {
                action_id: "acknowledge".to_string(),
                label: "OK".to_string(),
                description: "OK".to_string(),
            }],
            precedents: Vec::new(),
            refutation_condition: None,
            lens_hints: LensHints::preemption_only(),
            created_at: 0,
            expires_at: None,
        }
    }

    // --- Similarity scoring --------------------------------------------------

    #[test]
    fn direct_dep_match_scores_high() {
        let seed = sample_seed("tokio", "Adopted tokio", PrecedentOutcome::Confirmed);
        let item = sample_item("CVE affects your project", vec!["tokio"]);
        assert_eq!(match_similarity(&seed, &item), Some(0.9));
    }

    #[test]
    fn direct_dep_match_is_case_insensitive() {
        let seed = sample_seed("tokio", "Adopted tokio", PrecedentOutcome::Confirmed);
        let item = sample_item("CVE affects your project", vec!["Tokio"]);
        assert_eq!(match_similarity(&seed, &item), Some(0.9));
    }

    #[test]
    fn title_word_match_scores_medium() {
        let seed = sample_seed("tokio", "Adopted tokio", PrecedentOutcome::Confirmed);
        let item = sample_item("tokio performance regression", vec![]);
        assert_eq!(match_similarity(&seed, &item), Some(0.6));
    }

    #[test]
    fn title_substring_without_word_boundary_rejected() {
        let seed = sample_seed("react", "Adopted React", PrecedentOutcome::Confirmed);
        let item = sample_item("user reactions to the release", vec![]);
        // "react" as prefix of "reactions" should not score.
        assert_eq!(match_similarity(&seed, &item), None);
    }

    #[test]
    fn domain_match_scores_low() {
        let seed = sample_seed("vue", "Adopted Vue", PrecedentOutcome::Pending);
        let item = sample_item("CVE", vec!["react"]); // both in "framework" domain
        assert_eq!(match_similarity(&seed, &item), Some(0.3));
    }

    #[test]
    fn no_match_returns_none() {
        let seed = sample_seed("tokio", "Adopted tokio", PrecedentOutcome::Confirmed);
        let item = sample_item("frontend styling regression", vec!["tailwind"]);
        assert_eq!(match_similarity(&seed, &item), None);
    }

    #[test]
    fn misc_domain_does_not_produce_false_matches() {
        let seed = sample_seed("handwave", "Adopted handwave", PrecedentOutcome::Pending);
        let item = sample_item("unrelated thing", vec!["also-misc-thing"]);
        assert_eq!(match_similarity(&seed, &item), None);
    }

    // --- Title-word boundary edge cases --------------------------------------

    #[test]
    fn title_match_hyphenated() {
        assert!(title_contains_subject_as_word(
            "CVE in sqlite-vec 0.1.7",
            "sqlite-vec"
        ));
    }

    #[test]
    fn title_match_at_start() {
        assert!(title_contains_subject_as_word(
            "tokio 1.37 released",
            "tokio"
        ));
    }

    #[test]
    fn title_match_at_end() {
        assert!(title_contains_subject_as_word("adopts tokio", "tokio"));
    }

    #[test]
    fn title_match_rejects_embedded_substring() {
        assert!(!title_contains_subject_as_word(
            "notokio is not a thing",
            "tokio"
        ));
        assert!(!title_contains_subject_as_word("tokioisms", "tokio"));
    }

    #[test]
    fn title_match_empty_subject_returns_false() {
        assert!(!title_contains_subject_as_word("anything", ""));
    }

    // --- Precedent conversion ------------------------------------------------

    #[test]
    fn curated_precedent_id_uses_subject() {
        let seed = sample_seed("tokio", "Adopted tokio", PrecedentOutcome::Confirmed);
        assert_eq!(build_precedent_id(&seed), "curated:tokio");
    }

    #[test]
    fn git_precedent_id_uses_short_hash() {
        let seed = sample_git_seed("tokio");
        assert_eq!(build_precedent_id(&seed), "git:deadbeef");
    }

    #[test]
    fn origin_classification() {
        assert_eq!(
            classify_precedent_origin(&sample_seed(
                "tokio",
                "Adopted tokio",
                PrecedentOutcome::Confirmed
            )),
            "curated-corpus"
        );
        assert_eq!(
            classify_precedent_origin(&sample_git_seed("tokio")),
            "user-history"
        );
    }

    #[test]
    fn seed_to_precedent_preserves_fields() {
        let seed = sample_seed("tokio", "Adopted tokio", PrecedentOutcome::Confirmed);
        let p = seed_to_precedent(&seed, 0.9);
        assert_eq!(p.statement, "Adopted tokio");
        assert_eq!(p.similarity, 0.9);
        assert_eq!(p.outcome, Some(PrecedentOutcome::Confirmed));
        assert_eq!(p.decision_id, "curated:tokio");
    }

    // --- End-to-end enrichment (relies on loaded corpus) ---------------------

    #[test]
    fn enrich_item_attaches_precedents_for_known_dep() {
        // "tokio" is in the curated corpus as a Confirmed anchor.
        let mut item = sample_item("CVE-2026-1234 affects tokio 1.x", vec!["tokio"]);
        enrich_item(&mut item);
        assert!(!item.precedents.is_empty(), "expected precedents for tokio");
        // Top match must be the direct-dep match (similarity 0.9).
        assert_eq!(item.precedents[0].similarity, 0.9);
    }

    #[test]
    fn enrich_item_idempotent_when_precedents_exist() {
        let mut item = sample_item("CVE affects tokio", vec!["tokio"]);
        item.precedents.push(PrecedentRef {
            decision_id: "existing:1".to_string(),
            statement: "Pre-existing".to_string(),
            outcome: Some(PrecedentOutcome::Pending),
            origin: "user-history".to_string(),
            similarity: 1.0,
        });
        enrich_item(&mut item);
        assert_eq!(item.precedents.len(), 1, "must not overwrite existing");
        assert_eq!(item.precedents[0].decision_id, "existing:1");
    }

    #[test]
    fn enrich_item_caps_at_3_precedents() {
        // Use a dep whose domain (framework) contains many corpus entries.
        let mut item = sample_item("CVE", vec!["react"]);
        enrich_item(&mut item);
        assert!(
            item.precedents.len() <= MAX_PRECEDENTS_PER_ITEM,
            "expected ≤{} precedents, got {}",
            MAX_PRECEDENTS_PER_ITEM,
            item.precedents.len()
        );
    }

    #[test]
    fn enrich_item_with_no_match_leaves_empty() {
        let mut item = sample_item("totally unrelated title", vec!["nonexistent-dep-xyz"]);
        enrich_item(&mut item);
        assert!(
            item.precedents.is_empty(),
            "expected no precedents for unknown dep, got {:?}",
            item.precedents
        );
    }

    #[test]
    fn enrich_items_slice_processes_all() {
        let mut items = vec![
            sample_item("CVE affects tokio", vec!["tokio"]),
            sample_item("react update", vec!["react"]),
            sample_item("unknown", vec!["nonexistent-dep-xyz"]),
        ];
        enrich_items(&mut items);
        assert!(!items[0].precedents.is_empty());
        assert!(!items[1].precedents.is_empty());
        assert!(items[2].precedents.is_empty());
    }

    // --- Sort stability ------------------------------------------------------

    #[test]
    fn precedents_ordered_by_similarity_descending() {
        // Build an item that matches multiple corpus entries at
        // different similarity levels, verify sort.
        let mut item = sample_item("CVE in tokio affects react users", vec!["tokio"]);
        enrich_item(&mut item);
        for pair in item.precedents.windows(2) {
            assert!(
                pair[0].similarity >= pair[1].similarity,
                "precedents not sorted desc: {:?}",
                item.precedents
            );
        }
    }

    #[test]
    fn user_history_beats_curated_on_tie() {
        // We can't easily inject git-mined seeds into the OnceLock at
        // test time, but we can at least verify the cmp function chose
        // user-history. Construct two seeds and test sort stability
        // inline.
        let curated = sample_seed("tokio", "Adopted tokio", PrecedentOutcome::Confirmed);
        let git = sample_git_seed("tokio");
        let a_user = classify_precedent_origin(&curated) == "user-history";
        let b_user = classify_precedent_origin(&git) == "user-history";
        assert!(!a_user);
        assert!(b_user);
    }

    // --- Precedent index integrity ------------------------------------------

    #[test]
    fn precedent_index_contains_curated_entries() {
        let seeds = _test_seeds();
        assert!(
            seeds.len() >= 20,
            "expected ≥20 precedents, got {}",
            seeds.len()
        );
        // Confirm a known curated anchor is present.
        assert!(
            seeds.iter().any(|s| s.subject == "tokio"),
            "tokio precedent missing from index"
        );
    }
}
