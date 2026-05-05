// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Curated Feed Registry — pre-vetted developer intelligence sources
//!
//! Loads the curated feed catalog from embedded JSON at startup.
//! Provides O(1) lookup by feed URL during scoring (Phase 5) to override
//! generic RSS defaults with calibrated per-feed manifests.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::content_dna::ContentType;
use crate::source_tiers::SourceTier;

static REGISTRY: OnceLock<CuratedFeedRegistry> = OnceLock::new();

const CATALOG_JSON: &str = include_str!("../data/curated_feeds.json");

#[derive(Debug, Clone, Deserialize)]
pub struct CuratedFeedManifest {
    pub id: String,
    pub name: String,
    pub url: String,
    pub homepage: String,
    pub description: String,
    pub domains: Vec<String>,
    pub content_type: String,
    pub tier: String,
    pub editorial_model: String,
    pub expected_frequency_days: u32,
    pub color_hint: String,
}

impl CuratedFeedManifest {
    pub fn resolved_tier(&self) -> SourceTier {
        match self.tier.as_str() {
            "core" => SourceTier::Core,
            "ecosystem" => SourceTier::Ecosystem,
            _ => SourceTier::Peripheral,
        }
    }

    pub fn resolved_content_type(&self) -> Option<ContentType> {
        ContentType::from_slug(&self.content_type)
    }

    pub fn content_multiplier(&self) -> f32 {
        self.resolved_content_type()
            .map(|ct| ct.multiplier())
            .unwrap_or(1.0)
    }
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
    #[allow(dead_code)]
    schema_version: u32,
    feeds: Vec<CuratedFeedManifest>,
}

#[derive(Debug)]
pub struct CuratedFeedRegistry {
    by_url: HashMap<String, CuratedFeedManifest>,
    by_id: HashMap<String, CuratedFeedManifest>,
    all_feeds: Vec<CuratedFeedManifest>,
}

impl CuratedFeedRegistry {
    fn load() -> Self {
        let catalog: CatalogFile =
            serde_json::from_str(CATALOG_JSON).expect("curated_feeds.json must be valid JSON");

        let mut by_url = HashMap::with_capacity(catalog.feeds.len());
        let mut by_id = HashMap::with_capacity(catalog.feeds.len());

        for feed in &catalog.feeds {
            by_url.insert(feed.url.clone(), feed.clone());
            by_id.insert(feed.id.clone(), feed.clone());
        }

        Self {
            by_url,
            by_id,
            all_feeds: catalog.feeds,
        }
    }

    pub fn get_by_url(&self, url: &str) -> Option<&CuratedFeedManifest> {
        self.by_url.get(url)
    }

    pub fn get_by_id(&self, id: &str) -> Option<&CuratedFeedManifest> {
        self.by_id.get(id)
    }

    pub fn all_feeds(&self) -> &[CuratedFeedManifest] {
        &self.all_feeds
    }

    pub fn feed_count(&self) -> usize {
        self.all_feeds.len()
    }

    pub fn feeds_for_domain(&self, domain: &str) -> Vec<&CuratedFeedManifest> {
        self.all_feeds
            .iter()
            .filter(|f| f.domains.iter().any(|d| d == domain))
            .collect()
    }

    pub fn all_urls(&self) -> Vec<&str> {
        self.all_feeds.iter().map(|f| f.url.as_str()).collect()
    }

    pub fn all_domains(&self) -> Vec<&str> {
        let mut domains: Vec<&str> = self
            .all_feeds
            .iter()
            .flat_map(|f| f.domains.iter().map(|d| d.as_str()))
            .collect();
        domains.sort_unstable();
        domains.dedup();
        domains
    }
}

pub fn get_curated_registry() -> &'static CuratedFeedRegistry {
    REGISTRY.get_or_init(CuratedFeedRegistry::load)
}

pub fn is_curated_feed(url: &str) -> bool {
    get_curated_registry().get_by_url(url).is_some()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_loads_successfully() {
        let registry = get_curated_registry();
        assert!(
            registry.feed_count() >= 80,
            "Expected at least 80 curated feeds, got {}",
            registry.feed_count()
        );
    }

    #[test]
    fn no_duplicate_urls() {
        let registry = get_curated_registry();
        let mut seen = std::collections::HashSet::new();
        for feed in registry.all_feeds() {
            assert!(seen.insert(&feed.url), "Duplicate feed URL: {}", feed.url);
        }
    }

    #[test]
    fn no_duplicate_ids() {
        let registry = get_curated_registry();
        let mut seen = std::collections::HashSet::new();
        for feed in registry.all_feeds() {
            assert!(seen.insert(&feed.id), "Duplicate feed ID: {}", feed.id);
        }
    }

    #[test]
    fn all_content_types_resolve() {
        let registry = get_curated_registry();
        for feed in registry.all_feeds() {
            assert!(
                ContentType::from_slug(&feed.content_type).is_some(),
                "Feed '{}' has invalid content_type: '{}'",
                feed.id,
                feed.content_type
            );
        }
    }

    #[test]
    fn all_tiers_are_valid() {
        let registry = get_curated_registry();
        for feed in registry.all_feeds() {
            assert!(
                matches!(feed.tier.as_str(), "core" | "ecosystem" | "peripheral"),
                "Feed '{}' has invalid tier: '{}'",
                feed.id,
                feed.tier
            );
        }
    }

    #[test]
    fn lookup_by_url_works() {
        let registry = get_curated_registry();
        let result = registry.get_by_url("https://blog.rust-lang.org/feed.xml");
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, "rust-blog-official");
    }

    #[test]
    fn lookup_by_id_works() {
        let registry = get_curated_registry();
        let result = registry.get_by_id("cloudflare-blog");
        assert!(result.is_some());
        assert_eq!(result.unwrap().url, "https://blog.cloudflare.com/rss");
    }

    #[test]
    fn domain_filtering_works() {
        let registry = get_curated_registry();
        let rust_feeds = registry.feeds_for_domain("rust");
        assert!(
            rust_feeds.len() >= 5,
            "Expected at least 5 rust-domain feeds, got {}",
            rust_feeds.len()
        );
    }

    #[test]
    fn core_tier_feeds_exist() {
        let registry = get_curated_registry();
        let core_feeds: Vec<_> = registry
            .all_feeds()
            .iter()
            .filter(|f| f.tier == "core")
            .collect();
        assert!(
            core_feeds.len() >= 5,
            "Expected at least 5 core-tier feeds, got {}",
            core_feeds.len()
        );
    }

    #[test]
    fn curated_multipliers_are_higher_than_generic_rss() {
        let registry = get_curated_registry();
        let generic_rss_baseline = 0.97 * 1.0; // Peripheral tier × Discussion content

        for feed in registry.all_feeds() {
            let tier_mult = feed.resolved_tier().authority_multiplier();
            let content_mult = feed.content_multiplier();
            let combined = tier_mult * content_mult;

            assert!(
                combined >= generic_rss_baseline,
                "Feed '{}' scores lower ({:.3}) than generic RSS ({:.3})",
                feed.id,
                combined,
                generic_rss_baseline
            );
        }
    }

    #[test]
    fn all_feeds_have_at_least_one_domain() {
        let registry = get_curated_registry();
        for feed in registry.all_feeds() {
            assert!(
                !feed.domains.is_empty(),
                "Feed '{}' has no domains",
                feed.id
            );
        }
    }

    #[test]
    fn editorial_models_are_valid() {
        let registry = get_curated_registry();
        let valid = ["official", "single-expert", "editorial-team", "community"];
        for feed in registry.all_feeds() {
            assert!(
                valid.contains(&feed.editorial_model.as_str()),
                "Feed '{}' has invalid editorial_model: '{}'",
                feed.id,
                feed.editorial_model
            );
        }
    }
}
