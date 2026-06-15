// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV.dev source adapter — aggregated vulnerability intelligence
//!
//! Queries the Open Source Vulnerabilities database for security advisories
//! affecting user's installed dependencies. Covers all major ecosystems:
//! npm, crates.io, PyPI, Go, Maven, NuGet, RubyGems, Packagist, Pub.
//!
//! API docs: <https://osv.dev/docs/>
//!
//! Types, constants, and conversion helpers live in `osv_types`.

use super::osv_types::*;

use async_trait::async_trait;
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// OSV Source
// ============================================================================

/// OSV.dev source — fetches aggregated open-source vulnerability data
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

    /// Fetch vulnerabilities for popular packages in an ecosystem.
    ///
    /// The OSV API requires a package name — cannot query by ecosystem alone.
    /// Uses well-known packages per ecosystem, augmented by ACE deps when available.
    async fn fetch_ecosystem_vulns(&self, ecosystem: &str) -> SourceResult<Vec<OsvVulnerability>> {
        // Get packages to check: ACE deps with versions when available
        let ace_packages = crate::source_fetching::load_ace_packages_with_versions(ecosystem);
        let packages: Vec<(String, Option<String>)> = if !ace_packages.is_empty() {
            ace_packages.into_iter().take(15).collect()
        } else {
            let default_packages: Vec<&str> = match ecosystem {
                "npm" => vec!["express", "react", "next", "lodash", "axios", "webpack"],
                "crates.io" => vec!["serde", "tokio", "reqwest", "axum", "clap", "anyhow"],
                "PyPI" => vec![
                    "django", "flask", "requests", "numpy", "fastapi", "pydantic",
                ],
                "Go" => vec![
                    "golang.org/x/net",
                    "golang.org/x/crypto",
                    "github.com/gin-gonic/gin",
                ],
                "Maven" => vec![
                    "org.apache.logging.log4j:log4j-core",
                    "com.google.guava:guava",
                ],
                _ => vec![],
            };
            default_packages
                .iter()
                .map(|s| (s.to_string(), None))
                .collect()
        };

        let mut all_vulns = Vec::new();
        for (pkg_name, pkg_version) in &packages {
            let body = OsvQueryRequest {
                package: Some(OsvPackage {
                    name: pkg_name.clone(),
                    ecosystem: ecosystem.to_string(),
                }),
                version: pkg_version.clone(),
            };

            let response = match self
                .client
                .post("https://api.osv.dev/v1/query")
                .json(&body)
                .header("User-Agent", "4DA-Developer-OS/1.0")
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    warn!(target: "4da::sources", package = %pkg_name, error = %e, "OSV: network error");
                    continue;
                }
            };

            let status = response.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                break; // Stop querying this ecosystem
            }
            if !status.is_success() {
                continue; // Skip this package
            }

            if let Ok(result) = response.json::<OsvQueryResponse>().await {
                all_vulns.extend(result.vulns.unwrap_or_default());
            }

            // Rate limit between per-package queries
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        Ok(all_vulns)
    }

    /// Fetch vulnerabilities across multiple ecosystems using the batch endpoint.
    async fn fetch_batch_vulns(&self, ecosystems: &[&str]) -> SourceResult<Vec<OsvVulnerability>> {
        // Build queries with actual package names + versions per ecosystem
        let mut queries = Vec::new();
        for eco in ecosystems {
            let ace_packages = crate::source_fetching::load_ace_packages_with_versions(eco);
            let pkgs: Vec<(String, Option<String>)> = if ace_packages.is_empty() {
                match *eco {
                    "npm" => vec!["express", "react", "lodash"]
                        .into_iter()
                        .map(|s| (String::from(s), None))
                        .collect(),
                    "crates.io" => vec!["serde", "tokio", "reqwest"]
                        .into_iter()
                        .map(|s| (String::from(s), None))
                        .collect(),
                    "PyPI" => vec!["django", "flask", "requests"]
                        .into_iter()
                        .map(|s| (String::from(s), None))
                        .collect(),
                    _ => continue,
                }
            } else {
                ace_packages.into_iter().take(5).collect()
            };
            for (pkg, version) in pkgs {
                queries.push(OsvQueryRequest {
                    package: Some(OsvPackage {
                        name: pkg,
                        ecosystem: eco.to_string(),
                    }),
                    version,
                });
            }
        }

        let body = OsvBatchRequest { queries };

        let response = self
            .client
            .post("https://api.osv.dev/v1/querybatch")
            .json(&body)
            .header("User-Agent", "4DA-Developer-OS/1.0")
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SourceError::RateLimited(
                "OSV batch API rate limited (HTTP 429)".to_string(),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SourceError::Forbidden(
                "OSV batch API forbidden (HTTP 403)".to_string(),
            ));
        }
        super::check_http_status(status, "OSV batch API")?;

        let result: OsvBatchResponse = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let mut all_vulns = Vec::new();
        if let Some(results) = result.results {
            for resp in results {
                if let Some(vulns) = resp.vulns {
                    all_vulns.extend(vulns);
                }
            }
        }

        Ok(all_vulns)
    }
}

impl Default for OsvSource {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ACE-based ecosystem filtering
// ============================================================================

/// Get the OSV ecosystem names for which the user has actual runtime
/// dependencies tracked by ACE. Returns an empty vec when no ACE data
/// is available (first run, no projects scanned).
fn get_active_osv_ecosystems() -> Vec<String> {
    // Maps (ACE lookup key, OSV ecosystem name).
    // ACE uses "npm", "rust", "pypi", etc.; OSV uses "npm", "crates.io", "PyPI", etc.
    let ecosystem_map: &[(&str, &str)] = &[
        ("npm", "npm"),
        ("rust", "crates.io"),
        ("pypi", "PyPI"),
        ("go", "Go"),
        ("maven", "Maven"),
        ("nuget", "NuGet"),
        ("rubygems", "RubyGems"),
        ("packagist", "Packagist"),
    ];

    ecosystem_map
        .iter()
        .filter(|(ace_key, _)| {
            !crate::source_fetching::load_ace_packages_for_ecosystem(ace_key).is_empty()
        })
        .map(|(_, osv_eco)| osv_eco.to_string())
        .collect()
}

// ============================================================================
// Strict manifest mode — dependency-matched advisories
// ============================================================================

/// Strict manifest mode: surface only vulnerabilities that are **version-matched to the
/// stack's pinned dependencies**, via `osv::matching::get_matched_advisories`, instead of
/// the global popular-package query flow above. The advisory mirror is synced first when
/// stale so a single `--once` cycle can surface grounded vulns (the headless step-3 sync's
/// freshness gate then skips the just-synced mirror — no double download).
async fn matched_advisories_as_items() -> Vec<SourceItem> {
    let db = match crate::get_database() {
        Ok(db) => db,
        Err(e) => {
            warn!(target: "4da::sources", error = %e, "OSV strict mode: database unavailable");
            return Vec::new();
        }
    };

    let needs_sync = crate::osv::sync::needs_sync(&db, crate::osv::sync::osv_sync_max_age_hours())
        .unwrap_or(true);
    if needs_sync {
        if let Err(e) = crate::osv::sync::sync(&db).await {
            warn!(target: "4da::sources", error = %e, "OSV strict mode: advisory sync failed — matching against existing mirror");
        }
    }

    let matched = crate::osv::matching::get_matched_advisories(&db).unwrap_or_default();
    let items: Vec<SourceItem> = matched
        .iter()
        .map(matched_advisory_to_source_item)
        .collect();
    info!(
        target: "4da::sources",
        count = items.len(),
        "OSV strict mode: surfaced manifest-matched advisories"
    );
    items
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
        }
    }

    async fn fetch_items(&self) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        // Strict manifest mode: route through deterministic dependency matching and
        // suppress the global popular-package query flow entirely.
        if crate::source_fetching::strict_manifest_mode() {
            return Ok(matched_advisories_as_items().await);
        }

        // Determine which ecosystems the user actually has dependencies in
        let active_ecosystems = get_active_osv_ecosystems();
        let ecosystems: Vec<&str> = if active_ecosystems.is_empty() {
            // Fallback: no ACE data yet, query the two most common ecosystems
            vec!["npm", "crates.io"]
        } else {
            active_ecosystems.iter().map(|s| s.as_str()).collect()
        };

        info!(ecosystems = ?ecosystems, "Fetching OSV.dev vulnerabilities");

        let mut all_items = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for eco in &ecosystems {
            match self.fetch_ecosystem_vulns(eco).await {
                Ok(vulns) => {
                    info!(ecosystem = eco, count = vulns.len(), "Fetched OSV vulns");
                    for vuln in &vulns {
                        if seen_ids.insert(vuln.id.clone()) {
                            all_items.push(vuln_to_source_item(vuln));
                        }
                    }
                }
                Err(e) => {
                    warn!(ecosystem = eco, error = ?e, "Failed to fetch OSV vulns");
                }
            }
        }

        // Respect max_items limit
        all_items.truncate(self.config.max_items);

        info!(total = all_items.len(), "Total OSV items after dedup");
        Ok(all_items)
    }

    async fn fetch_items_deep(&self, _items_per_category: usize) -> SourceResult<Vec<SourceItem>> {
        if !self.config.enabled {
            return Err(SourceError::Disabled);
        }

        // Strict manifest mode: deterministic dependency matching (same as the shallow
        // path); the global batch query is suppressed.
        if crate::source_fetching::strict_manifest_mode() {
            return Ok(matched_advisories_as_items().await);
        }

        // Only query ecosystems the user has dependencies in
        let active_ecosystems = get_active_osv_ecosystems();
        let ecosystems: Vec<&str> = if active_ecosystems.is_empty() {
            // Fallback: no ACE data yet, use all defaults
            DEFAULT_ECOSYSTEMS.to_vec()
        } else {
            active_ecosystems.iter().map(|s| s.as_str()).collect()
        };

        info!(ecosystems = ?ecosystems, "Deep fetching OSV.dev vulnerabilities");

        // Use the batch endpoint to query ecosystems at once
        let vulns = match self.fetch_batch_vulns(&ecosystems).await {
            Ok(v) => v,
            Err(e) => {
                warn!(error = ?e, "Batch fetch failed, falling back to sequential");
                // Fallback: query each ecosystem individually
                let mut fallback_vulns = Vec::new();
                for eco in &ecosystems {
                    match self.fetch_ecosystem_vulns(eco).await {
                        Ok(v) => fallback_vulns.extend(v),
                        Err(e) => {
                            warn!(ecosystem = eco, error = ?e, "Failed to fetch OSV ecosystem");
                        }
                    }
                }
                fallback_vulns
            }
        };

        // Dedup by vuln ID and convert
        let mut seen_ids = std::collections::HashSet::new();
        let items: Vec<SourceItem> = vulns
            .iter()
            .filter(|v| seen_ids.insert(v.id.clone()))
            .map(vuln_to_source_item)
            .collect();

        info!(total = items.len(), "Total deep OSV items after dedup");
        Ok(items)
    }

    async fn scrape_content(&self, item: &SourceItem) -> SourceResult<String> {
        // OSV items already have full content from the API response
        if !item.content.is_empty() {
            return Ok(item.content.clone());
        }

        // If content is somehow empty, try fetching the individual vuln
        let vuln_url = format!("https://api.osv.dev/v1/vulns/{}", item.source_id);

        let response = self
            .client
            .get(&vuln_url)
            .header("User-Agent", "4DA-Developer-OS/1.0")
            .send()
            .await
            .map_err(|e| SourceError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Ok(item.content.clone());
        }

        let vuln: OsvVulnerability = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let enriched = vuln_to_source_item(&vuln);
        Ok(enriched.content)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matched_advisory_title_is_grounding_compatible() {
        // The ledger's grounding gate grounds a vulnerability on the LEADING title token
        // after stripping a `[id]` prefix. This test pins that contract: title must be
        // `[<advisory_id>] <package_name>: <summary>` and the item must carry source_type
        // "osv" with source_id = advisory_id.
        let m = crate::osv::types::MatchedAdvisory {
            advisory_id: "GHSA-xxxx-yyyy-zzzz".to_string(),
            summary: "SSRF via crafted URL".to_string(),
            details: Some("Long details".to_string()),
            package_name: "axios".to_string(),
            ecosystem: "npm".to_string(),
            installed_version: Some("1.6.0".to_string()),
            fixed_version: Some("1.6.8".to_string()),
            severity_type: Some("CVSS_V3".to_string()),
            cvss_score: Some(7.5),
            source_url: Some("https://github.com/advisories/GHSA-xxxx-yyyy-zzzz".to_string()),
            is_version_confirmed: true,
            project_paths: vec!["/stack".to_string()],
            published_at: Some("2026-01-01T00:00:00Z".to_string()),
            dependency_instances: vec![],
        };

        let item = matched_advisory_to_source_item(&m);
        assert_eq!(item.source_type, "osv");
        assert_eq!(item.source_id, "GHSA-xxxx-yyyy-zzzz");
        assert_eq!(
            item.title,
            "[GHSA-xxxx-yyyy-zzzz] axios: SSRF via crafted URL"
        );

        // Replicate the ledger's grounding extraction (grounding.mjs isGrounded, vuln branch):
        // strip the leading `[...]` id prefix, take the first token before whitespace/colon.
        let body = item.title.trim_start_matches('[');
        let after_id = body.splitn(2, ']').nth(1).unwrap().trim();
        let leading = after_id.split([' ', ':']).next().unwrap();
        assert_eq!(
            leading, "axios",
            "leading title token must be the pinned package"
        );

        // Content names the package+ecosystem and the fix, so the receipt is self-describing.
        assert!(item.content.contains("axios (npm)"));
        assert!(item.content.contains("Fixed in: 1.6.8"));
    }

    #[test]
    fn test_osv_source_creation() {
        let source = OsvSource::new();
        assert_eq!(source.source_type(), "osv");
        assert_eq!(source.name(), "OSV.dev");
        assert!(source.config().enabled);
        assert_eq!(source.config().max_items, 50);
        assert_eq!(source.config().fetch_interval_secs, 3600);
    }

    #[test]
    fn test_osv_source_default() {
        let source = OsvSource::default();
        assert_eq!(source.source_type(), "osv");
    }

    #[test]
    fn test_vuln_to_source_item_full() {
        let vuln = OsvVulnerability {
            id: "GHSA-xxxx-yyyy-zzzz".to_string(),
            summary: Some("XSS in React Router".to_string()),
            details: Some("A cross-site scripting vulnerability exists in...".to_string()),
            severity: Some(vec![OsvSeverity {
                severity_type: "CVSS_V3".to_string(),
                score: "7.5".to_string(),
            }]),
            affected: Some(vec![OsvAffected {
                package: Some(OsvPackage {
                    name: "react-router".to_string(),
                    ecosystem: "npm".to_string(),
                }),
                ranges: Some(vec![OsvRange {
                    range_type: "SEMVER".to_string(),
                    events: Some(vec![
                        serde_json::json!({"introduced": "0"}),
                        serde_json::json!({"fixed": "6.4.5"}),
                    ]),
                }]),
                versions: None,
            }]),
            references: Some(vec![
                OsvReference {
                    ref_type: "ADVISORY".to_string(),
                    url: "https://github.com/advisories/GHSA-xxxx-yyyy-zzzz".to_string(),
                },
                OsvReference {
                    ref_type: "WEB".to_string(),
                    url: "https://example.com/blog".to_string(),
                },
            ]),
            published: Some("2026-03-15T10:00:00Z".to_string()),
            modified: Some("2026-03-20T12:00:00Z".to_string()),
        };

        let item = vuln_to_source_item(&vuln);

        assert_eq!(item.source_type, "osv");
        assert_eq!(item.source_id, "GHSA-xxxx-yyyy-zzzz");
        assert_eq!(item.title, "[GHSA-xxxx-yyyy-zzzz] XSS in React Router");
        assert_eq!(
            item.url,
            Some("https://github.com/advisories/GHSA-xxxx-yyyy-zzzz".to_string())
        );
        assert!(item.content.contains("XSS in React Router"));
        assert!(item.content.contains("CVSS_V3: 7.5"));
        assert!(item.content.contains("react-router (npm)"));
        assert!(item.content.contains("Fixed in: 6.4.5"));

        let metadata = item.metadata.unwrap();
        assert_eq!(metadata["severity"], "CVSS_V3: 7.5");
        assert_eq!(metadata["cvss_score"], 7.5);
        assert_eq!(metadata["published"], "2026-03-15T10:00:00Z");
        assert_eq!(metadata["modified"], "2026-03-20T12:00:00Z");
        assert_eq!(metadata["fixed_versions"], serde_json::json!(["6.4.5"]));
    }

    #[test]
    fn test_vuln_to_source_item_minimal() {
        let vuln = OsvVulnerability {
            id: "OSV-2026-1234".to_string(),
            summary: None,
            details: None,
            severity: None,
            affected: None,
            references: None,
            published: None,
            modified: None,
        };

        let item = vuln_to_source_item(&vuln);

        assert_eq!(item.source_id, "OSV-2026-1234");
        assert_eq!(item.title, "[OSV-2026-1234] Security advisory");
        assert_eq!(
            item.url,
            Some("https://osv.dev/vulnerability/OSV-2026-1234".to_string())
        );
        assert!(item.content.contains("Severity: Unknown"));
        assert!(item.content.contains("Affected: Unknown"));
    }

    #[test]
    fn test_vuln_to_source_item_prefers_advisory_url() {
        let vuln = OsvVulnerability {
            id: "TEST-001".to_string(),
            summary: Some("Test".to_string()),
            details: None,
            severity: None,
            affected: None,
            references: Some(vec![
                OsvReference {
                    ref_type: "WEB".to_string(),
                    url: "https://web.example.com".to_string(),
                },
                OsvReference {
                    ref_type: "ADVISORY".to_string(),
                    url: "https://advisory.example.com".to_string(),
                },
            ]),
            published: None,
            modified: None,
        };

        let item = vuln_to_source_item(&vuln);
        assert_eq!(item.url, Some("https://advisory.example.com".to_string()));
    }

    #[test]
    fn test_vuln_to_source_item_prefers_cvss_v3() {
        let vuln = OsvVulnerability {
            id: "TEST-002".to_string(),
            summary: Some("Test".to_string()),
            details: None,
            severity: Some(vec![
                OsvSeverity {
                    severity_type: "CVSS_V2".to_string(),
                    score: "5.0".to_string(),
                },
                OsvSeverity {
                    severity_type: "CVSS_V3".to_string(),
                    score: "8.1".to_string(),
                },
            ]),
            affected: None,
            references: None,
            published: None,
            modified: None,
        };

        let item = vuln_to_source_item(&vuln);
        assert!(item.content.contains("CVSS_V3: 8.1"));
    }

    #[test]
    fn test_osv_json_parsing() {
        let json = r#"{
            "vulns": [
                {
                    "id": "GHSA-test-0001",
                    "summary": "SQL injection in ORM",
                    "details": "A SQL injection vulnerability...",
                    "severity": [
                        { "type": "CVSS_V3", "score": "9.8" }
                    ],
                    "affected": [
                        {
                            "package": { "name": "some-orm", "ecosystem": "npm" },
                            "ranges": [
                                {
                                    "type": "SEMVER",
                                    "events": [
                                        { "introduced": "0" },
                                        { "fixed": "3.2.1" }
                                    ]
                                }
                            ]
                        }
                    ],
                    "references": [
                        { "type": "ADVISORY", "url": "https://github.com/advisories/GHSA-test-0001" }
                    ],
                    "published": "2026-03-10T00:00:00Z",
                    "modified": "2026-03-12T00:00:00Z"
                }
            ]
        }"#;

        let response: OsvQueryResponse = serde_json::from_str(json).unwrap();
        let vulns = response.vulns.unwrap();
        assert_eq!(vulns.len(), 1);
        assert_eq!(vulns[0].id, "GHSA-test-0001");
        assert_eq!(vulns[0].summary.as_deref(), Some("SQL injection in ORM"));

        let severity = vulns[0].severity.as_ref().unwrap();
        assert_eq!(severity[0].severity_type, "CVSS_V3");
        assert_eq!(severity[0].score, "9.8");

        let affected = vulns[0].affected.as_ref().unwrap();
        assert_eq!(affected[0].package.as_ref().unwrap().ecosystem, "npm");
    }

    #[test]
    fn test_osv_batch_response_parsing() {
        let json = r#"{
            "results": [
                {
                    "vulns": [
                        { "id": "VULN-A", "summary": "Vuln A" }
                    ]
                },
                {
                    "vulns": [
                        { "id": "VULN-B", "summary": "Vuln B" },
                        { "id": "VULN-C", "summary": "Vuln C" }
                    ]
                },
                {
                    "vulns": null
                }
            ]
        }"#;

        let response: OsvBatchResponse = serde_json::from_str(json).unwrap();
        let results = response.results.unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].vulns.as_ref().unwrap().len(), 1);
        assert_eq!(results[1].vulns.as_ref().unwrap().len(), 2);
        assert!(results[2].vulns.is_none());
    }

    #[test]
    fn test_ecosystem_map_coverage() {
        // Verify all expected manifest files are mapped
        let manifests: Vec<&str> = ECOSYSTEM_MAP.iter().map(|(m, _)| *m).collect();
        assert!(manifests.contains(&"Cargo.toml"));
        assert!(manifests.contains(&"package.json"));
        assert!(manifests.contains(&"pyproject.toml"));
        assert!(manifests.contains(&"requirements.txt"));
        assert!(manifests.contains(&"go.mod"));
        assert!(manifests.contains(&"pom.xml"));
        assert!(manifests.contains(&"build.gradle"));
        assert!(manifests.contains(&"Gemfile"));
        assert!(manifests.contains(&".csproj"));
        assert!(manifests.contains(&"composer.json"));
        assert!(manifests.contains(&"pubspec.yaml"));
    }

    #[test]
    fn test_multiple_affected_packages() {
        let vuln = OsvVulnerability {
            id: "MULTI-001".to_string(),
            summary: Some("Cross-ecosystem vuln".to_string()),
            details: None,
            severity: None,
            affected: Some(vec![
                OsvAffected {
                    package: Some(OsvPackage {
                        name: "pkg-a".to_string(),
                        ecosystem: "npm".to_string(),
                    }),
                    ranges: None,
                    versions: None,
                },
                OsvAffected {
                    package: Some(OsvPackage {
                        name: "pkg-b".to_string(),
                        ecosystem: "PyPI".to_string(),
                    }),
                    ranges: None,
                    versions: None,
                },
            ]),
            references: None,
            published: None,
            modified: None,
        };

        let item = vuln_to_source_item(&vuln);
        assert!(item.content.contains("pkg-a (npm)"));
        assert!(item.content.contains("pkg-b (PyPI)"));

        let metadata = item.metadata.unwrap();
        let pkgs = metadata["affected_packages"].as_array().unwrap();
        assert_eq!(pkgs.len(), 2);
    }
}
