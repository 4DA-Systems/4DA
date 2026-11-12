// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV.dev source adapter — the user's dependency advisories as feed items.
//!
//! On the desktop this adapter sends nothing: it reads the advisories the
//! local mirror (`crate::osv`) matched against the installed versions, which
//! come from each ecosystem's public archive download. Until 2026-11-12 it
//! also POSTed every dependency name, with its installed version, to
//! `api.osv.dev` (`/v1/query` one by one, `/v1/querybatch` on the deep
//! fetch) every hour, for items the mirror already had (audit P2).
//!
//! Strict manifest mode (`FOURDA_STRICT_MANIFEST=1`, set only by the 4DA
//! receipts ledger for its fixture stacks) still queries OSV per version:
//! see `osv_live`.
//!
//! API docs: <https://osv.dev/docs/>

use async_trait::async_trait;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// OSV Source
// ============================================================================

/// OSV.dev source — the user's matched dependency advisories.
pub struct OsvSource {
    config: SourceConfig,
    client: reqwest::Client,
}

impl OsvSource {
    /// Create a new OSV source with default config
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 50,
                fetch_interval_secs: 3600, // 1 hour
                custom: None,
            },
            client: super::shared_client(),
        }
    }
}

impl Default for OsvSource {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Mirror-matched advisories
// ============================================================================

/// The advisories the local mirror matched against installed versions, as
/// items, one per advisory. Reads the database only: no request is made, and
/// the mirror itself is kept current by the sync step, never from here (a
/// source fetch runs under a 90-second timeout; npm's archive is ~200 MB).
pub(super) fn mirror_matched_items() -> Vec<SourceItem> {
    let db = match crate::get_database() {
        Ok(db) => db,
        Err(e) => {
            warn!(target: "4da::sources", error = %e, "OSV: database unavailable");
            return Vec::new();
        }
    };
    let matched = crate::osv::matching::get_matched_advisories(&db).unwrap_or_default();
    let items = super::osv_collapse::one_item_per_advisory(
        matched
            .iter()
            .map(matched_advisory_to_source_item)
            .collect(),
    );
    info!(
        target: "4da::sources",
        count = items.len(),
        "OSV: surfaced mirror-matched advisories"
    );
    items
}

/// Strict manifest mode's fallback: sync the mirror first when it is stale,
/// so a single `--once` cycle can surface grounded vulns (the headless
/// step-3 sync's freshness gate then skips the just-synced mirror — no
/// double download), then read it.
pub(super) async fn matched_advisories_as_items() -> Vec<SourceItem> {
    if let Ok(db) = crate::get_database() {
        let needs_sync =
            crate::osv::sync::needs_sync(&db, crate::osv::sync::osv_sync_max_age_hours())
                .unwrap_or(true);
        if needs_sync {
            if let Err(e) = crate::osv::sync::sync(&db).await {
                warn!(target: "4da::sources", error = %e, "OSV strict mode: advisory sync failed — matching against existing mirror");
            }
        }
    }
    mirror_matched_items()
}

/// Build a `SourceItem` from a dependency-matched advisory. The title LEADS with the
/// affected package name (after the `[advisory-id]` prefix) so the ledger's grounding gate
/// — which grounds a vulnerability on the leading title token — verifies it names a pinned
/// dependency. e.g. `[GHSA-xxxx-yyyy-zzzz] axios: SSRF via crafted URL`.
fn matched_advisory_to_source_item(m: &crate::osv::types::MatchedAdvisory) -> SourceItem {
    let title = format!("[{}] {}: {}", m.advisory_id, m.package_name, m.summary);
    let url = m
        .source_url
        .clone()
        .or_else(|| Some(format!("https://osv.dev/vulnerability/{}", m.advisory_id)));

    let mut content_parts = vec![
        format!("{} ({})", m.package_name, m.ecosystem),
        m.summary.clone(),
    ];
    if let Some(details) = &m.details {
        content_parts.push(details.clone());
    }
    // The line the scoring pipeline reads a CVSS score from
    // (`scoring::pipeline_v2::extract_cvss_from_content`, "Severity: TYPE: score").
    if let Some(score) = m.cvss_score {
        content_parts.push(format!(
            "Severity: {}: {score:.1}",
            m.severity_type.as_deref().unwrap_or("CVSS")
        ));
    }
    if let Some(installed) = &m.installed_version {
        content_parts.push(format!("Installed: {installed}"));
    }
    if let Some(fixed) = &m.fixed_version {
        content_parts.push(format!("Fixed in: {fixed}"));
    }
    let content = content_parts.join("\n");

    let metadata = serde_json::json!({
        "ecosystem": m.ecosystem,
        "package": m.package_name,
        "advisory_id": m.advisory_id,
        "installed_version": m.installed_version,
        "fixed_version": m.fixed_version,
        "cvss_score": m.cvss_score,
        "severity": m.severity_type,
        "is_version_confirmed": m.is_version_confirmed,
        "manifest_grounded": true,
        "source_name": "osv",
    });

    SourceItem::new("osv", &m.advisory_id, &title)
        .with_url(url)
        .with_content(content)
        .with_metadata(metadata)
}

// ============================================================================
// Source Trait Implementation
// ============================================================================

#[async_trait]
impl Source for OsvSource {
    fn source_type(&self) -> &'static str {
        "osv"
    }

    fn name(&self) -> &'static str {
        "OSV.dev"
    }

    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn set_config(&mut self, config: SourceConfig) {
        self.config = config;
    }

    fn manifest(&self) -> super::SourceManifest {
        super::SourceManifest {
            category: super::SourceCategory::Security,
            default_content_type: "security_advisory",
            default_multiplier: 1.30,
            label: "OSV",
            color_hint: "red",
            min_title_words: 3,
            require_user_language: false,
            require_dev_relevance: false,
            max_item_age_days: None,
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }
        if crate::source_fetching::strict_manifest_mode() {
            return Ok(super::osv_live::live_matched_advisories_as_items(&self.client).await);
        }
        let mut items = mirror_matched_items();
        items.truncate(self.config.max_items);
        Ok(items)
    }

    async fn fetch_items_deep(&self, items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }
        if crate::source_fetching::strict_manifest_mode() {
            return Ok(super::osv_live::live_matched_advisories_as_items(&self.client).await);
        }
        let mut items = mirror_matched_items();
        items.truncate(items_per_category.clamp(1, self.config.max_items));
        Ok(items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // Mirror items carry the advisory text already; nothing is fetched.
        Ok(item.content.clone())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "osv_tests.rs"]
mod tests;
