// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! CVE/NVD feed source adapter for the Developer Immune System.
//!
//! Fetches security advisories from GitHub Advisory Database and NVD.
//! Cross-references against user's installed dependencies to generate
//! targeted vulnerability alerts.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{shared_client, SourceItem};

// Re-export matching functions so existing `cve::X` paths still work
#[allow(unused_imports)]
pub(crate) use super::cve_matching::{cross_reference_advisories, normalize_ecosystem};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CveAdvisory {
    pub cve_id: String,
    pub title: String,
    pub description: String,
    pub severity: String,
    pub cvss_score: Option<f32>,
    pub affected_packages: Vec<AffectedPackage>,
    pub published_at: String,
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AffectedPackage {
    pub name: String,
    pub ecosystem: String,
    pub affected_versions: String,
    pub patched_version: Option<String>,
}

// ============================================================================
// GitHub Advisory Database fetcher
// ============================================================================

/// GitHub Global Security Advisories API page size — the API's `per_page` max.
const GITHUB_ADVISORY_PAGE_SIZE: usize = 100;

/// Pagination cap per fetch: pages x page size = 300 advisories max per call,
/// so one fetch cycle stays bounded even on npm-malware-wave days. Pagination
/// stops early on a short page, so quiet ecosystems cost a single request.
const GITHUB_ADVISORY_MAX_PAGES: usize = 3;

/// Only advisories published inside this window are fetched. Sorted newest-first
/// WITH pagination, the window defines coverage — the previous single
/// `per_page=30` request meant one burst day (e.g. an npm malware wave) evicted
/// everything published before it (scoring audit 2026-08-23, plan item 17).
const GITHUB_ADVISORY_WINDOW_DAYS: i64 = 30;

/// Build the first-page URL for the GitHub Global Security Advisories API.
/// `published_since` (YYYY-MM-DD) becomes a percent-encoded `published=>=DATE`
/// range filter so the API only returns the current window.
fn build_advisories_url(ecosystem: Option<&str>, published_since: &str) -> String {
    let mut url = format!(
        "https://api.github.com/advisories?per_page={GITHUB_ADVISORY_PAGE_SIZE}&sort=published&direction=desc&published=%3E%3D{published_since}"
    );
    if let Some(eco) = ecosystem {
        url.push_str(&format!("&ecosystem={eco}"));
    }
    url
}

/// Extract the `rel="next"` target from a GitHub `Link` header. The global
/// advisories endpoint pages by cursor (`before`/`after`), so following the
/// server-provided next link — which carries the cursor AND the original query
/// filters — is the only correct way to page it.
fn parse_link_next(link_header: &str) -> Option<String> {
    link_header.split(',').find_map(|part| {
        let (target, params) = part.split_once(';')?;
        if !params.contains("rel=\"next\"") {
            return None;
        }
        let target = target.trim().strip_prefix('<')?.strip_suffix('>')?;
        Some(target.to_string())
    })
}

/// Fetch recent advisories from GitHub Advisory Database.
/// This is preferred over NVD because it includes ecosystem-specific package data.
///
/// Windowed to the last [`GITHUB_ADVISORY_WINDOW_DAYS`] days and paginated up to
/// [`GITHUB_ADVISORY_MAX_PAGES`] pages of [`GITHUB_ADVISORY_PAGE_SIZE`], newest
/// first. Once a page has landed, later-page failures degrade to a partial
/// result instead of discarding what was already fetched.
pub(crate) async fn fetch_github_advisories(ecosystem: Option<&str>) -> Result<Vec<CveAdvisory>> {
    let client = shared_client();
    let published_since = (chrono::Utc::now()
        - chrono::Duration::days(GITHUB_ADVISORY_WINDOW_DAYS))
    .format("%Y-%m-%d")
    .to_string();
    let mut next_url = Some(build_advisories_url(ecosystem, &published_since));

    let mut advisories = Vec::new();
    let mut pages_fetched = 0usize;

    while pages_fetched < GITHUB_ADVISORY_MAX_PAGES {
        let Some(url) = next_url.take() else {
            break;
        };

        let response = match client
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "4DA-Developer-OS/1.0")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) if pages_fetched == 0 => return Err(e.into()),
            Err(e) => {
                tracing::warn!(target: "4da::sources", page = pages_fetched, error = %e, "CVE: pagination request failed, keeping advisories fetched so far");
                break;
            }
        };

        if !response.status().is_success() {
            // First page: preserve the empty-result contract; later pages: keep
            // what already landed.
            break;
        }

        // Cursor pagination: read the Link header BEFORE .json() consumes the
        // response.
        let link_next = response
            .headers()
            .get(reqwest::header::LINK)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_link_next);

        let body: serde_json::Value = match response.json().await {
            Ok(b) => b,
            Err(e) if pages_fetched == 0 => return Err(e.into()),
            Err(e) => {
                tracing::warn!(target: "4da::sources", page = pages_fetched, error = %e, "CVE: pagination parse failed, keeping advisories fetched so far");
                break;
            }
        };

        let page_len = body.as_array().map(|a| a.len()).unwrap_or(0);
        if let Some(items) = body.as_array() {
            for item in items {
                if let Some(advisory) = parse_github_advisory(item) {
                    advisories.push(advisory);
                }
            }
        }

        pages_fetched += 1;
        // A short page means the window is exhausted — stop before spending
        // another request (the API is unauthenticated: 60 req/hr budget).
        if page_len < GITHUB_ADVISORY_PAGE_SIZE {
            break;
        }
        next_url = link_next;
    }

    Ok(advisories)
}

fn parse_github_advisory(item: &serde_json::Value) -> Option<CveAdvisory> {
    let ghsa_id = item.get("ghsa_id")?.as_str()?;
    let cve_id = item
        .get("cve_id")
        .and_then(|v| v.as_str())
        .unwrap_or(ghsa_id);
    let summary = item.get("summary")?.as_str()?;
    let description = item
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let severity = item
        .get("severity")
        .and_then(|v| v.as_str())
        .unwrap_or("MEDIUM");
    let cvss_score = item
        .get("cvss")
        .and_then(|v| v.get("score"))
        .and_then(serde_json::Value::as_f64)
        .map(|v| v as f32);
    let published = item
        .get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let url = item.get("html_url").and_then(|v| v.as_str()).unwrap_or("");

    let mut affected_packages = Vec::new();
    if let Some(vulns) = item.get("vulnerabilities").and_then(|v| v.as_array()) {
        for vuln in vulns {
            if let Some(pkg) = vuln.get("package") {
                let name = pkg.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let ecosystem = pkg.get("ecosystem").and_then(|v| v.as_str()).unwrap_or("");
                let range = vuln
                    .get("vulnerable_version_range")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let patched = vuln
                    .get("patched_versions")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                if !name.is_empty() {
                    affected_packages.push(AffectedPackage {
                        name: name.to_string(),
                        ecosystem: ecosystem.to_lowercase(),
                        affected_versions: range.to_string(),
                        patched_version: patched,
                    });
                }
            }
        }
    }

    Some(CveAdvisory {
        cve_id: cve_id.to_string(),
        title: summary.to_string(),
        description: description.to_string(),
        severity: severity.to_uppercase(),
        cvss_score,
        affected_packages,
        published_at: published.to_string(),
        source_url: url.to_string(),
    })
}

// ============================================================================
// Integration with scoring pipeline
// ============================================================================

/// Convert CVE advisories to SourceItems for the PASIFA scoring pipeline.
pub(crate) fn advisories_to_source_items(advisories: &[CveAdvisory]) -> Vec<SourceItem> {
    advisories
        .iter()
        .map(|a| {
            let packages: Vec<String> = a
                .affected_packages
                .iter()
                .map(|p| format!("{} ({})", p.name, p.ecosystem))
                .collect();

            let content = format!(
                "{}\n\nSeverity: {}\nAffected: {}\n{}",
                a.description,
                a.severity,
                packages.join(", "),
                a.cvss_score
                    .map(|s| format!("CVSS: {s:.1}"))
                    .unwrap_or_default()
            );

            SourceItem {
                source_id: a.cve_id.clone(),
                source_type: "cve".to_string(),
                title: format!("[{}] {}", a.cve_id, a.title),
                url: Some(a.source_url.clone()),
                content,
                metadata: {
                    let mut m = serde_json::json!({"source_name": "cve"});
                    if let Ok(pkgs) = serde_json::to_value(&a.affected_packages) {
                        m["affected_packages"] = pkgs;
                    }
                    Some(m)
                },
            }
        })
        .collect()
}

// ============================================================================
// ACE-based ecosystem filtering
// ============================================================================

/// Maps (ACE lookup key, GitHub Advisory DB ecosystem name).
/// ACE uses "rust", "pypi", etc.; GitHub uses "npm", "pip", "rust", "composer",
/// "actions", etc. (the API's `ecosystem` parameter vocabulary).
const GITHUB_ECOSYSTEM_MAP: &[(&str, &str)] = &[
    ("npm", "npm"),
    ("rust", "rust"),
    ("pypi", "pip"),
    ("go", "go"),
    ("maven", "maven"),
    ("nuget", "nuget"),
    ("rubygems", "rubygems"),
    ("packagist", "composer"),
    ("pub", "pub"),
    // ACE has no manifest scanner for Swift (Package.swift) or GitHub Actions
    // workflows yet, so these two never pass the dep filter below today — they
    // are mapped for the day it does, and the deep-scan fallback list already
    // covers them now.
    ("swift", "swift"),
    ("github-actions", "actions"),
];

/// Get the GitHub Advisory DB ecosystem names for which the user has actual
/// runtime dependencies tracked by ACE. Returns an empty vec when no ACE
/// data is available (first run, no projects scanned).
fn get_user_ecosystems() -> Vec<String> {
    GITHUB_ECOSYSTEM_MAP
        .iter()
        .filter(|(ace_key, _)| {
            !crate::source_fetching::load_ace_packages_for_ecosystem(ace_key).is_empty()
        })
        .map(|(_, github_eco)| github_eco.to_string())
        .collect()
}

// ============================================================================
// Source trait implementation
// ============================================================================

use super::{Source, SourceConfig, SourceError, SourceResult};
use async_trait::async_trait;

/// Security advisory source — fetches CVEs from GitHub Advisory Database
pub struct CveSource {
    config: SourceConfig,
}

impl CveSource {
    pub fn new() -> Self {
        Self {
            config: SourceConfig {
                enabled: true,
                max_items: 30,
                fetch_interval_secs: 3600,
                custom: None,
            },
        }
    }
}

impl Default for CveSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Source for CveSource {
    fn source_type(&self) -> &'static str {
        "cve"
    }
    fn name(&self) -> &'static str {
        "Security Advisories"
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
            label: "CVE",
            color_hint: "red",
            min_title_words: 3,
            require_user_language: false,
            require_dev_relevance: false,
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        // Strict manifest mode: the GitHub Advisory feed is a global popular-package flow
        // (ungrounded — the source of the identical cross-stack `cve:*` items). Grounded,
        // version-matched vulnerabilities are surfaced via the OSV matching path instead,
        // so suppress this source entirely.
        if crate::source_fetching::strict_manifest_mode() {
            tracing::info!(target: "4da::sources", "Strict manifest mode: CVE global feed suppressed (vulns routed via OSV matching)");
            return Ok(Vec::new());
        }

        // Filter by user's actual ecosystems when ACE data is available
        let user_ecosystems = get_user_ecosystems();

        if user_ecosystems.is_empty() {
            // Fallback: no ACE data yet, fetch unfiltered (current behavior)
            let advisories = fetch_github_advisories(None)
                .await
                .map_err(|e| SourceError::Network(e.to_string()))?;
            let items = advisories_to_source_items(&advisories);
            tracing::info!(target: "4da::sources", count = items.len(), "CVE: Fetched security advisories (unfiltered, no ACE data)");
            return Ok(items.into_iter().take(self.config.max_items).collect());
        }

        let mut all_items = Vec::new();
        for ecosystem in &user_ecosystems {
            match fetch_github_advisories(Some(ecosystem)).await {
                Ok(advisories) => {
                    let items = advisories_to_source_items(&advisories);
                    tracing::info!(target: "4da::sources", ecosystem = %ecosystem, count = items.len(), "CVE: Fetched advisories for ecosystem");
                    all_items.extend(items);
                }
                Err(e) => {
                    tracing::warn!(target: "4da::sources", ecosystem = %ecosystem, error = %e, "CVE: Failed to fetch advisories for ecosystem");
                }
            }
        }
        tracing::info!(target: "4da::sources", count = all_items.len(), ecosystems = ?user_ecosystems, "CVE: Fetched filtered security advisories");
        Ok(all_items.into_iter().take(self.config.max_items).collect())
    }

    async fn fetch_items_deep(&self, items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        // See fetch_items: suppressed in strict manifest mode.
        if crate::source_fetching::strict_manifest_mode() {
            return Ok(Vec::new());
        }

        let user_ecosystems = get_user_ecosystems();
        let ecosystems: Vec<String> = if user_ecosystems.is_empty() {
            // Fallback: no ACE data yet, use all default ecosystems
            vec![
                "npm", "pip", "go", "rubygems", "maven", "nuget", "rust", "composer", "pub",
                "swift", "actions",
            ]
            .into_iter()
            .map(String::from)
            .collect()
        } else {
            user_ecosystems
        };

        let mut all_items = Vec::new();
        for eco in &ecosystems {
            match fetch_github_advisories(Some(eco)).await {
                Ok(advisories) => {
                    let items = advisories_to_source_items(&advisories);
                    all_items.extend(items.into_iter().take(items_per_category));
                }
                Err(e) => {
                    tracing::warn!(target: "4da::sources", ecosystem = %eco, error = %e, "CVE: Failed for ecosystem");
                }
            }
        }
        tracing::info!(target: "4da::sources", count = all_items.len(), ecosystems = ?ecosystems, "CVE: Deep scan complete");
        Ok(all_items)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_advisory() -> CveAdvisory {
        CveAdvisory {
            cve_id: "CVE-2026-0001".to_string(),
            title: "Prototype pollution in lodash".to_string(),
            description: "A prototype pollution vulnerability exists in lodash".to_string(),
            severity: "HIGH".to_string(),
            cvss_score: Some(7.5),
            affected_packages: vec![AffectedPackage {
                name: "lodash".to_string(),
                ecosystem: "npm".to_string(),
                affected_versions: "< 4.17.21".to_string(),
                patched_version: Some("4.17.21".to_string()),
            }],
            published_at: "2026-03-19T00:00:00Z".to_string(),
            source_url: "https://github.com/advisories/GHSA-test".to_string(),
        }
    }

    #[test]
    fn test_advisory_to_source_item() {
        let items = advisories_to_source_items(&[sample_advisory()]);
        assert_eq!(items.len(), 1);
        assert!(items[0].title.contains("CVE-2026-0001"));
        assert_eq!(items[0].source_type, "cve");
        assert!(items[0].content.contains("HIGH"));
    }

    #[test]
    fn test_parse_github_advisory_valid() {
        let json = serde_json::json!({
            "ghsa_id": "GHSA-test-1234",
            "cve_id": "CVE-2026-9999",
            "summary": "Test vulnerability",
            "description": "A test vulnerability",
            "severity": "high",
            "cvss": { "score": 7.5 },
            "published_at": "2026-03-19T00:00:00Z",
            "html_url": "https://github.com/advisories/GHSA-test-1234",
            "vulnerabilities": [{
                "package": {
                    "name": "test-pkg",
                    "ecosystem": "npm"
                },
                "vulnerable_version_range": "< 2.0.0",
                "patched_versions": "2.0.0"
            }]
        });

        let advisory = parse_github_advisory(&json);
        assert!(advisory.is_some());
        let a = advisory.unwrap();
        assert_eq!(a.cve_id, "CVE-2026-9999");
        assert_eq!(a.affected_packages.len(), 1);
        assert_eq!(a.affected_packages[0].name, "test-pkg");
    }

    #[test]
    fn test_parse_github_advisory_minimal() {
        let json = serde_json::json!({
            "ghsa_id": "GHSA-minimal",
            "summary": "Minimal advisory"
        });

        let advisory = parse_github_advisory(&json);
        assert!(advisory.is_some());
        let a = advisory.unwrap();
        assert_eq!(a.cve_id, "GHSA-minimal");
        assert!(a.affected_packages.is_empty());
    }

    #[test]
    fn test_build_advisories_url_windowed_and_paged() {
        let url = build_advisories_url(Some("npm"), "2026-07-24");
        // Full page size, newest-first, date-windowed, ecosystem-filtered.
        assert!(url.contains("per_page=100"), "{url}");
        assert!(url.contains("sort=published&direction=desc"), "{url}");
        // `>=` percent-encoded — one burst day cannot evict the window.
        assert!(url.contains("published=%3E%3D2026-07-24"), "{url}");
        assert!(url.ends_with("&ecosystem=npm"), "{url}");

        let unfiltered = build_advisories_url(None, "2026-07-24");
        assert!(!unfiltered.contains("ecosystem="), "{unfiltered}");
    }

    #[test]
    fn test_parse_link_next() {
        // Typical GitHub cursor-pagination Link header.
        let header = "<https://api.github.com/advisories?per_page=100&after=Y3Vyc29yOjEwMA%3D%3D>; rel=\"next\", <https://api.github.com/advisories?per_page=100&before=Zmlyc3Q%3D>; rel=\"prev\"";
        assert_eq!(
            parse_link_next(header).as_deref(),
            Some("https://api.github.com/advisories?per_page=100&after=Y3Vyc29yOjEwMA%3D%3D")
        );

        // No next link (last page) -> pagination stops.
        assert_eq!(
            parse_link_next("<https://api.github.com/advisories?before=x>; rel=\"prev\""),
            None
        );
        // Garbage never panics the fetcher.
        assert_eq!(parse_link_next(""), None);
        assert_eq!(parse_link_next("not a link header"), None);
    }

    #[test]
    fn test_github_ecosystem_map_coverage() {
        // The map speaks the GitHub Advisory API's `ecosystem` vocabulary.
        let github_names: Vec<&str> = GITHUB_ECOSYSTEM_MAP.iter().map(|(_, g)| *g).collect();
        for expected in [
            "npm", "rust", "pip", "go", "maven", "nuget", "rubygems",
            // Previously missing (scoring audit 2026-08-23, plan item 17):
            "composer", "pub", "swift", "actions",
        ] {
            assert!(
                github_names.contains(&expected),
                "GitHub ecosystem map missing {expected}"
            );
        }
        // The ACE side of each pair must be an ecosystem key the loaders accept
        // (swift/github-actions are documented forward-mappings ACE cannot
        // populate yet).
        for (ace_key, _) in GITHUB_ECOSYSTEM_MAP {
            assert!(!ace_key.is_empty());
        }
    }
}
