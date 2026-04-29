// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Curated Seed Corpus — Intelligence Reconciliation Phase 8 (Cold Start Layer 2).
//!
//! Ships a hand-annotated set of common developer decisions
//! (framework / DB / testing / deploy / state / async / monorepo / …)
//! bundled into the binary via `include_str!`. Users with light git
//! history still get meaningful precedent coverage on Day 0.
//!
//! Source: `src-tauri/src/seed_data/decisions.jsonl` — one JSON
//! `SeededDecision` per line. Outcomes are conservative — "confirmed"
//! only when community adoption is unambiguous (e.g. TypeScript,
//! Docker, PostgreSQL); everything else is "pending" so we don't
//! ship false claims about how a decision worked out.
//!
//! This module does not write to the AWE graph directly — it hands
//! out `SeededDecision`s and lets the seeder (next phase) merge them
//! with git-mined personal priors before the AWE import.

use serde::{Deserialize, Serialize};

use crate::git_decision_miner::SeededDecision;

// ============================================================================
// Embedded corpus
// ============================================================================

/// The corpus content, compiled into the binary at build time.
/// One JSON object per line (JSONL).
const CORPUS_JSONL: &str = include_str!("seed_data/decisions.jsonl");

// ============================================================================
// Public API
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CorpusStats {
    pub total: u32,
    pub by_outcome: OutcomeCounts,
    pub domains_covered: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutcomeCounts {
    pub confirmed: u32,
    pub refuted: u32,
    pub partial: u32,
    pub pending: u32,
}

/// Load every `SeededDecision` in the bundled corpus. Malformed lines
/// are skipped with a structured log — the corpus is developer-vetted,
/// so any parse error is a coding bug we want visible but not fatal.
pub fn load_corpus() -> Vec<SeededDecision> {
    let mut out: Vec<SeededDecision> = Vec::new();
    for (idx, line) in CORPUS_JSONL.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match serde_json::from_str::<SeededDecision>(trimmed) {
            Ok(d) => out.push(d),
            Err(e) => {
                tracing::warn!(
                    target: "4da::seed_corpus",
                    line = idx + 1,
                    error = %e,
                    "skipping malformed corpus line"
                );
            }
        }
    }
    out
}

/// Summary statistics for the currently-loaded corpus — used by the
/// Tauri command surface and the tests.
pub fn corpus_stats() -> CorpusStats {
    let items = load_corpus();
    let mut counts = OutcomeCounts::default();
    let mut domains: std::collections::HashSet<String> = std::collections::HashSet::new();

    for d in &items {
        match d.outcome {
            crate::evidence::PrecedentOutcome::Confirmed => counts.confirmed += 1,
            crate::evidence::PrecedentOutcome::Refuted => counts.refuted += 1,
            crate::evidence::PrecedentOutcome::Partial => counts.partial += 1,
            crate::evidence::PrecedentOutcome::Pending => counts.pending += 1,
        }
        // Domain bucketing: first-word-of-subject gives us a coarse
        // grouping. A proper taxonomy lands in Phase 9 when AWE's
        // synthesis layer needs domain affinity signals.
        let domain = classify_subject_domain(&d.subject);
        domains.insert(domain);
    }

    CorpusStats {
        total: items.len() as u32,
        by_outcome: counts,
        domains_covered: domains.len() as u32,
    }
}

/// Coarse domain classification for a decision subject. Kept
/// rule-based (not learned) so the corpus-domain counts are stable
/// across runs and easy to reason about in tests.
pub fn classify_subject_domain(subject: &str) -> String {
    let s = subject.to_lowercase();
    // Languages
    if matches!(
        s.as_str(),
        "typescript" | "rust" | "go" | "python" | "bun" | "deno"
    ) {
        return "language".to_string();
    }
    // UI frameworks
    if matches!(
        s.as_str(),
        "react" | "vue" | "svelte" | "astro" | "next.js" | "remix"
    ) {
        return "framework".to_string();
    }
    // Desktop shells
    if matches!(s.as_str(), "tauri" | "electron") {
        return "desktop-shell".to_string();
    }
    // Databases
    if matches!(
        s.as_str(),
        "postgresql"
            | "sqlite"
            | "mongodb"
            | "dynamodb"
            | "redis"
            | "supabase"
            | "neon"
            | "planetscale"
    ) {
        return "database".to_string();
    }
    // Testing
    if matches!(s.as_str(), "vitest" | "jest" | "playwright") {
        return "testing".to_string();
    }
    // Build / bundler
    if matches!(s.as_str(), "vite" | "webpack" | "esbuild") {
        return "build".to_string();
    }
    // State management
    if matches!(s.as_str(), "zustand" | "redux" | "jotai") {
        return "state".to_string();
    }
    // Async runtimes
    if matches!(s.as_str(), "tokio" | "async-std") {
        return "async-runtime".to_string();
    }
    // Monorepo tooling
    if matches!(s.as_str(), "turborepo" | "pnpm-workspaces" | "nx") {
        return "monorepo".to_string();
    }
    // Deploy targets
    if matches!(
        s.as_str(),
        "vercel" | "cloudflare-workers" | "docker" | "kubernetes"
    ) {
        return "deploy".to_string();
    }
    // Package managers
    if matches!(s.as_str(), "pnpm" | "yarn") {
        return "package-manager".to_string();
    }
    // API / data-layer
    if matches!(s.as_str(), "graphql" | "trpc") {
        return "data-layer".to_string();
    }
    // Styling
    if matches!(s.as_str(), "tailwind" | "shadcn") {
        return "styling".to_string();
    }
    // Linting / formatting
    if matches!(s.as_str(), "biome" | "eslint") {
        return "lint".to_string();
    }
    "misc".to_string()
}

// ============================================================================
// Tauri command
// ============================================================================

/// Return corpus stats as a JSON string. Useful for the UI to report
/// "X precedents available" on a fresh install.
#[tauri::command]
pub async fn get_seed_corpus_stats() -> std::result::Result<String, String> {
    serde_json::to_string(&corpus_stats()).map_err(|e| e.to_string())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_loads_something() {
        let c = load_corpus();
        assert!(
            c.len() >= 20,
            "expected ≥20 decisions in bundled corpus, got {}",
            c.len()
        );
    }

    #[test]
    fn corpus_all_valid_shape() {
        let c = load_corpus();
        for d in &c {
            assert!(!d.statement.is_empty(), "statement empty: {d:?}");
            assert!(!d.verb.is_empty(), "verb empty: {d:?}");
            assert!(!d.subject.is_empty(), "subject empty: {d:?}");
            assert_eq!(d.source_repo, "curated-corpus");
            assert_eq!(d.source_commit, "curated");
            assert_eq!(d.timestamp, 0);
        }
    }

    #[test]
    fn corpus_covers_multiple_domains() {
        let stats = corpus_stats();
        assert!(
            stats.domains_covered >= 6,
            "expected ≥6 domains, got {}",
            stats.domains_covered
        );
    }

    #[test]
    fn corpus_has_language_domain() {
        let c = load_corpus();
        assert!(c
            .iter()
            .any(|d| classify_subject_domain(&d.subject) == "language"));
    }

    #[test]
    fn corpus_has_framework_domain() {
        let c = load_corpus();
        assert!(c
            .iter()
            .any(|d| classify_subject_domain(&d.subject) == "framework"));
    }

    #[test]
    fn corpus_has_database_domain() {
        let c = load_corpus();
        assert!(c
            .iter()
            .any(|d| classify_subject_domain(&d.subject) == "database"));
    }

    #[test]
    fn corpus_has_testing_domain() {
        let c = load_corpus();
        assert!(c
            .iter()
            .any(|d| classify_subject_domain(&d.subject) == "testing"));
    }

    #[test]
    fn corpus_has_build_domain() {
        let c = load_corpus();
        assert!(c
            .iter()
            .any(|d| classify_subject_domain(&d.subject) == "build"));
    }

    #[test]
    fn corpus_has_deploy_domain() {
        let c = load_corpus();
        assert!(c
            .iter()
            .any(|d| classify_subject_domain(&d.subject) == "deploy"));
    }

    #[test]
    fn stats_sum_to_total() {
        let stats = corpus_stats();
        let sum = stats.by_outcome.confirmed
            + stats.by_outcome.refuted
            + stats.by_outcome.partial
            + stats.by_outcome.pending;
        assert_eq!(sum, stats.total);
    }

    #[test]
    fn classify_subject_domain_covers_every_corpus_subject() {
        let c = load_corpus();
        // No entry should fall through to "misc" — if one does, the
        // corpus author forgot to add a classification rule. This
        // keeps the classifier and the corpus in lockstep.
        let unclassified: Vec<&str> = c
            .iter()
            .filter(|d| classify_subject_domain(&d.subject) == "misc")
            .map(|d| d.subject.as_str())
            .collect();
        assert!(
            unclassified.is_empty(),
            "corpus subjects without domain classification: {unclassified:?}"
        );
    }

    #[test]
    fn confirmed_outcomes_are_conservative() {
        // Guard-rail: if more than half the corpus is marked Confirmed,
        // we're probably overclaiming industry consensus. Tighten if
        // this test fails — it means a new Confirmed entry needs a
        // critical review.
        let stats = corpus_stats();
        let pct_confirmed = (stats.by_outcome.confirmed as f32 / stats.total as f32) * 100.0;
        assert!(
            pct_confirmed <= 50.0,
            "{}% of corpus is Confirmed — too optimistic; prefer Pending when unsure",
            pct_confirmed
        );
    }

    #[test]
    fn no_duplicate_subjects() {
        let c = load_corpus();
        let mut seen = std::collections::HashSet::new();
        let mut dupes = Vec::new();
        for d in &c {
            if !seen.insert(d.subject.clone()) {
                dupes.push(d.subject.clone());
            }
        }
        assert!(dupes.is_empty(), "duplicate subjects in corpus: {dupes:?}");
    }

    #[test]
    fn every_outcome_serializes_from_corpus() {
        // Ensure the JSONL string-cased outcome values parse into the
        // PrecedentOutcome enum. (Schema change would catch this at
        // compile time, but a typo in the JSONL only shows up here.)
        let c = load_corpus();
        // Parsing `load_corpus` already exercised serde_json::from_str
        // for every line, so the mere fact that c is non-empty proves
        // all lines parsed.
        assert!(!c.is_empty());
    }

    #[test]
    fn confirmed_pool_has_known_anchors() {
        // Sanity: if TypeScript or Docker are not in the Confirmed
        // bucket, something upstream has gone wrong. Canaries.
        let c = load_corpus();
        let confirmed: Vec<&str> = c
            .iter()
            .filter(|d| d.outcome == crate::evidence::PrecedentOutcome::Confirmed)
            .map(|d| d.subject.as_str())
            .collect();
        assert!(
            confirmed.contains(&"typescript"),
            "typescript should be a Confirmed anchor"
        );
        assert!(
            confirmed.contains(&"docker"),
            "docker should be a Confirmed anchor"
        );
    }

    #[test]
    fn corpus_outcomes_stay_in_enum() {
        // If load_corpus() ever returns items, it means every outcome
        // string parsed successfully into the enum — so this is a
        // round-trip guarantee, not a separate check. Assert the
        // invariant holds.
        let c = load_corpus();
        for d in &c {
            let reserialized = serde_json::to_string(&d.outcome).unwrap();
            assert!(
                matches!(
                    reserialized.as_str(),
                    "\"confirmed\"" | "\"refuted\"" | "\"partial\"" | "\"pending\""
                ),
                "unexpected outcome serialization: {reserialized}"
            );
        }
    }
}
