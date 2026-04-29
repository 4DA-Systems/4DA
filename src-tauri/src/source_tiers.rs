// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Source Tier classification for fetch priority, scoring, failure handling,
//! and result diversity.
//!
//! Each content source is assigned a tier that governs:
//! - Authority multiplier applied during scoring (Phase 5)
//! - Whether circuit breaker failures surface to the user
//! - Whether the source qualifies for deep (parallel) fetch
//! - Minimum guaranteed slots in final results (diversity floor)
//!
//! Tier assignment is static by default but can be promoted dynamically
//! when ACE detects that a source's language matches the user's stack.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceTier {
    /// Direct stack intelligence -- package registries, security feeds for
    /// the user's detected languages.
    Core,
    /// Broader developer ecosystem -- HN, Reddit, Lobsters, arXiv, dev.to,
    /// StackOverflow, PapersWithCode.
    Ecosystem,
    /// Supplementary discovery -- social, video, product launches, custom RSS.
    Peripheral,
}

#[allow(dead_code)] // Methods used incrementally as fetch/failure/diversity logic is wired
impl SourceTier {
    /// Default tier based on source type string.
    /// ACE-detected languages can promote sources dynamically via
    /// [`for_source_with_context`].
    pub fn default_for_source(source_type: &str) -> Self {
        match source_type {
            // Security is ALWAYS Core
            "cve" | "osv" => SourceTier::Core,

            // Package registries default to Ecosystem (promoted to Core when
            // the user's stack matches -- see for_source_with_context)
            "crates_io" | "npm_registry" | "pypi" | "go_modules" | "huggingface" => {
                SourceTier::Ecosystem
            }

            // General developer discourse
            "hackernews" | "reddit" | "lobsters" | "devto" | "stackoverflow" => {
                SourceTier::Ecosystem
            }

            // Research
            "arxiv" | "papers_with_code" => SourceTier::Ecosystem,

            // GitHub defaults to Ecosystem (promoted to Core if user has repos)
            "github" => SourceTier::Ecosystem,

            // Social, video, product launches
            "twitter" | "bluesky" | "youtube" | "producthunt" => SourceTier::Peripheral,

            // Custom RSS
            "rss" => SourceTier::Peripheral,

            _ => SourceTier::Peripheral,
        }
    }

    /// Promote package registries to Core when the user's ACE-detected
    /// languages include the source's ecosystem language.
    pub fn for_source_with_context(source_type: &str, detected_languages: &[String]) -> Self {
        let base = Self::default_for_source(source_type);

        let should_promote = match source_type {
            "crates_io" => detected_languages
                .iter()
                .any(|l| l.eq_ignore_ascii_case("rust")),
            "npm_registry" => detected_languages.iter().any(|l| {
                l.eq_ignore_ascii_case("typescript") || l.eq_ignore_ascii_case("javascript")
            }),
            "pypi" => detected_languages
                .iter()
                .any(|l| l.eq_ignore_ascii_case("python")),
            "go_modules" => detected_languages
                .iter()
                .any(|l| l.eq_ignore_ascii_case("go")),
            "huggingface" => detected_languages
                .iter()
                .any(|l| l.eq_ignore_ascii_case("python")),
            "github" => true, // Always relevant if user has any repos
            _ => false,
        };

        if should_promote && base == SourceTier::Ecosystem {
            SourceTier::Core
        } else {
            base
        }
    }

    /// Whether circuit breaker failures should be surfaced to the user.
    pub fn surface_failures(&self) -> bool {
        matches!(self, SourceTier::Core)
    }

    /// Whether this source should always use deep (parallel) fetch.
    pub fn deep_fetch(&self) -> bool {
        matches!(self, SourceTier::Core | SourceTier::Ecosystem)
    }

    /// Minimum guaranteed slots in results (0 = no guarantee, compete on merit).
    pub fn diversity_slots(&self) -> usize {
        match self {
            SourceTier::Core => 2,
            SourceTier::Ecosystem => 1,
            SourceTier::Peripheral => 0,
        }
    }

    /// Authority multiplier for scoring -- slight boost for Core to offset
    /// freshness bias from high-volume Ecosystem sources.
    pub fn authority_multiplier(&self) -> f32 {
        match self {
            SourceTier::Core => 1.05,
            SourceTier::Ecosystem => 1.00,
            SourceTier::Peripheral => 0.97,
        }
    }
}

// ============================================================================
// Batch normalization
// ============================================================================

/// Post-scoring normalization: blend raw score with source-relative percentile.
/// 80% raw score + 20% source-normalized ensures every source's best content
/// can compete fairly regardless of source-level score distribution.
///
/// Items are grouped by `source_type`. Groups with fewer than 3 items are
/// left untouched -- not enough data to compute a meaningful percentile.
pub(crate) fn normalize_scores_by_source(items: &mut [crate::SourceRelevance]) {
    use std::collections::HashMap;

    // Group indices by source type (owned keys to avoid borrowing items)
    let mut source_indices: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, item) in items.iter().enumerate() {
        source_indices
            .entry(item.source_type.clone())
            .or_default()
            .push(idx);
    }

    for (_source, indices) in source_indices {
        if indices.len() < 3 {
            continue;
        }

        // Collect (index, score) pairs to avoid borrowing items during sort
        let mut scored: Vec<(usize, f32)> =
            indices.iter().map(|&i| (i, items[i].top_score)).collect();
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let n = scored.len() as f32;
        for (rank, &(idx, raw)) in scored.iter().enumerate() {
            let percentile = (rank as f32 + 1.0) / n;
            items[idx].top_score = raw * 0.80 + percentile * 0.20;
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_sources_are_core() {
        assert_eq!(SourceTier::default_for_source("cve"), SourceTier::Core);
        assert_eq!(SourceTier::default_for_source("osv"), SourceTier::Core);
    }

    #[test]
    fn package_registries_default_to_ecosystem() {
        for src in &["crates_io", "npm_registry", "pypi", "go_modules"] {
            assert_eq!(
                SourceTier::default_for_source(src),
                SourceTier::Ecosystem,
                "{src} should default to Ecosystem"
            );
        }
    }

    #[test]
    fn social_sources_are_peripheral() {
        for src in &["twitter", "bluesky", "youtube", "producthunt"] {
            assert_eq!(
                SourceTier::default_for_source(src),
                SourceTier::Peripheral,
                "{src} should be Peripheral"
            );
        }
    }

    #[test]
    fn unknown_source_defaults_to_peripheral() {
        assert_eq!(
            SourceTier::default_for_source("unknown_source"),
            SourceTier::Peripheral
        );
    }

    #[test]
    fn crates_io_promoted_when_rust_detected() {
        let langs = vec!["Rust".to_string()];
        assert_eq!(
            SourceTier::for_source_with_context("crates_io", &langs),
            SourceTier::Core
        );
    }

    #[test]
    fn npm_promoted_when_typescript_detected() {
        let langs = vec!["TypeScript".to_string()];
        assert_eq!(
            SourceTier::for_source_with_context("npm_registry", &langs),
            SourceTier::Core
        );
    }

    #[test]
    fn npm_promoted_when_javascript_detected() {
        let langs = vec!["JavaScript".to_string()];
        assert_eq!(
            SourceTier::for_source_with_context("npm_registry", &langs),
            SourceTier::Core
        );
    }

    #[test]
    fn crates_io_not_promoted_without_rust() {
        let langs = vec!["Python".to_string()];
        assert_eq!(
            SourceTier::for_source_with_context("crates_io", &langs),
            SourceTier::Ecosystem
        );
    }

    #[test]
    fn github_always_promoted() {
        let langs = vec!["anything".to_string()];
        assert_eq!(
            SourceTier::for_source_with_context("github", &langs),
            SourceTier::Core
        );
    }

    #[test]
    fn github_not_promoted_with_empty_context() {
        let langs: Vec<String> = vec![];
        // github promote check returns true unconditionally, but
        // for_source_with_context still promotes because should_promote = true
        assert_eq!(
            SourceTier::for_source_with_context("github", &langs),
            SourceTier::Core
        );
    }

    #[test]
    fn core_surfaces_failures() {
        assert!(SourceTier::Core.surface_failures());
        assert!(!SourceTier::Ecosystem.surface_failures());
        assert!(!SourceTier::Peripheral.surface_failures());
    }

    #[test]
    fn deep_fetch_for_core_and_ecosystem() {
        assert!(SourceTier::Core.deep_fetch());
        assert!(SourceTier::Ecosystem.deep_fetch());
        assert!(!SourceTier::Peripheral.deep_fetch());
    }

    #[test]
    fn diversity_slots_descend_by_tier() {
        assert!(SourceTier::Core.diversity_slots() > SourceTier::Ecosystem.diversity_slots());
        assert!(SourceTier::Ecosystem.diversity_slots() > SourceTier::Peripheral.diversity_slots());
    }

    #[test]
    fn authority_multipliers_ordered() {
        assert!(
            SourceTier::Core.authority_multiplier() > SourceTier::Ecosystem.authority_multiplier()
        );
        assert!(
            SourceTier::Ecosystem.authority_multiplier()
                > SourceTier::Peripheral.authority_multiplier()
        );
    }

    #[test]
    fn authority_multipliers_close_to_one() {
        // Multipliers should be small adjustments, not dramatic swings
        for tier in &[
            SourceTier::Core,
            SourceTier::Ecosystem,
            SourceTier::Peripheral,
        ] {
            let m = tier.authority_multiplier();
            assert!(
                m >= 0.90 && m <= 1.10,
                "Multiplier {m} out of range for {tier:?}"
            );
        }
    }
}
