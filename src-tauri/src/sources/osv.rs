//! OSV.dev source adapter — aggregated vulnerability intelligence
//!
//! Queries the Open Source Vulnerabilities database for security advisories
//! affecting user's installed dependencies. Covers all major ecosystems:
//! npm, crates.io, PyPI, Go, Maven, NuGet, RubyGems, Packagist, Pub.
//!
//! API docs: <https://osv.dev/docs/>

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::{Source, SourceConfig, SourceError, SourceItem, SourceResult};

// ============================================================================
// OSV API Types
// ============================================================================

#[derive(Debug, Serialize)]
struct OsvQueryRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    package: Option<OsvPackage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
}

#[derive(Debug, Serialize)]
struct OsvBatchRequest {
    queries: Vec<OsvQueryRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OsvPackage {
    name: String,
    ecosystem: String,
}

#[derive(Debug, Deserialize)]
struct OsvQueryResponse {
    vulns: Option<Vec<OsvVulnerability>>,
}

#[derive(Debug, Deserialize)]
struct OsvBatchResponse {
    results: Option<Vec<OsvQueryResponse>>,
}

#[derive(Debug, Deserialize)]
struct OsvVulnerability {
    id: String,
    summary: Option<String>,
    details: Option<String>,
    severity: Option<Vec<OsvSeverity>>,
    affected: Option<Vec<OsvAffected>>,
    references: Option<Vec<OsvReference>>,
    published: Option<String>,
    modified: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OsvSeverity {
    #[serde(rename = "type")]
    severity_type: String,
    score: String,
}

#[derive(Debug, Deserialize)]
struct OsvAffected {
    package: Option<OsvPackage>,
    ranges: Option<Vec<OsvRange>>,
    #[allow(dead_code)]
    versions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct OsvRange {
    #[serde(rename = "type")]
    range_type: String,
    events: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct OsvReference {
    #[serde(rename = "type")]
    ref_type: String,
    url: String,
}

// ============================================================================
// Ecosystem mapping (manifest file -> OSV ecosystem string)
// ============================================================================

/// Map from ACE-detected manifest files to OSV ecosystem identifiers.
#[allow(dead_code)]
pub(crate) const ECOSYSTEM_MAP: &[(&str, &str)] = &[
    ("Cargo.toml", "crates.io"),
    ("package.json", "npm"),
    ("pyproject.toml", "PyPI"),
    ("requirements.txt", "PyPI"),
    ("go.mod", "Go"),
    ("pom.xml", "Maven"),
    ("build.gradle", "Maven"),
    ("Gemfile", "RubyGems"),
    (".csproj", "NuGet"),
    ("composer.json", "Packagist"),
    ("pubspec.yaml", "Pub"),
];

/// Default ecosystems to query for broad developer coverage.
const DEFAULT_ECOSYSTEMS: &[&str] = &["npm", "crates.io", "PyPI", "Go", "Maven"];

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
        // Get packages to check: ACE deps first, then popular defaults
        let ace_packages = crate::source_fetching::load_ace_packages_for_ecosystem(ecosystem);
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
        let packages: Vec<String> = if ace_packages.is_empty() {
            default_packages.iter().map(|s| s.to_string()).collect()
        } else {
            ace_packages.into_iter().take(10).collect()
        };

        let mut all_vulns = Vec::new();
        for pkg_name in &packages {
            let body = OsvQueryRequest {
                package: Some(OsvPackage {
                    name: pkg_name.clone(),
                    ecosystem: ecosystem.to_string(),
                }),
                version: None,
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
        // Build queries with actual package names per ecosystem
        let mut queries = Vec::new();
        for eco in ecosystems {
            let ace_packages = crate::source_fetching::load_ace_packages_for_ecosystem(eco);
            let pkgs: Vec<String> = if ace_packages.is_empty() {
                match *eco {
                    "npm" => vec!["express", "react", "lodash"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                    "crates.io" => vec!["serde", "tokio", "reqwest"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                    "PyPI" => vec!["django", "flask", "requests"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                    _ => continue,
                }
            } else {
                ace_packages.into_iter().take(5).collect()
            };
            for pkg in pkgs {
                queries.push(OsvQueryRequest {
                    package: Some(OsvPackage {
                        name: pkg,
                        ecosystem: eco.to_string(),
                    }),
                    version: None,
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
        if !status.is_success() {
            return Err(SourceError::Network(format!(
                "OSV batch API error: HTTP {}",
                status.as_u16()
            )));
        }

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

        info!("Fetching OSV.dev vulnerabilities (npm + crates.io)");

        // Default fetch: query the two most common developer ecosystems
        let primary_ecosystems = &["npm", "crates.io"];
        let mut all_items = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for eco in primary_ecosystems {
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

        info!("Deep fetching OSV.dev across all default ecosystems");

        // Use the batch endpoint to query all default ecosystems at once
        let vulns = match self.fetch_batch_vulns(DEFAULT_ECOSYSTEMS).await {
            Ok(v) => v,
            Err(e) => {
                warn!(error = ?e, "Batch fetch failed, falling back to sequential");
                // Fallback: query each ecosystem individually
                let mut fallback_vulns = Vec::new();
                for eco in DEFAULT_ECOSYSTEMS {
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
// Conversion helpers
// ============================================================================

/// Convert an OSV vulnerability into a SourceItem for the scoring pipeline.
fn vuln_to_source_item(vuln: &OsvVulnerability) -> SourceItem {
    let summary = vuln.summary.as_deref().unwrap_or("Security advisory");
    let details = vuln.details.as_deref().unwrap_or("");

    // Extract severity info (prefer CVSS_V3 over CVSS_V2)
    let severity_str = vuln
        .severity
        .as_ref()
        .and_then(|s| {
            s.iter()
                .find(|sv| sv.severity_type == "CVSS_V3")
                .or_else(|| s.first())
        })
        .map(|s| format!("{}: {}", s.severity_type, s.score))
        .unwrap_or_else(|| "Unknown".to_string());

    // Extract affected packages
    let affected_pkgs: Vec<String> = vuln
        .affected
        .as_ref()
        .map(|affected| {
            affected
                .iter()
                .filter_map(|a| a.package.as_ref())
                .map(|p| format!("{} ({})", p.name, p.ecosystem))
                .collect()
        })
        .unwrap_or_default();

    // Extract fixed versions from range events
    let fixed_versions: Vec<String> = vuln
        .affected
        .as_ref()
        .map(|affected| {
            affected
                .iter()
                .filter_map(|a| a.ranges.as_ref())
                .flatten()
                .filter_map(|r| r.events.as_ref())
                .flatten()
                .filter_map(|event| {
                    event
                        .as_object()
                        .and_then(|obj| obj.get("fixed"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();

    // Build reference URL (prefer ADVISORY type, then WEB, then fallback)
    let url = vuln
        .references
        .as_ref()
        .and_then(|refs| {
            refs.iter()
                .find(|r| r.ref_type == "ADVISORY")
                .or_else(|| refs.iter().find(|r| r.ref_type == "WEB"))
                .or_else(|| refs.first())
        })
        .map(|r| r.url.clone())
        .unwrap_or_else(|| format!("https://osv.dev/vulnerability/{}", vuln.id));

    let content = format!(
        "{}\n\nSeverity: {}\nAffected: {}\n{}\n{}",
        summary,
        severity_str,
        if affected_pkgs.is_empty() {
            "Unknown".to_string()
        } else {
            affected_pkgs.join(", ")
        },
        if fixed_versions.is_empty() {
            String::new()
        } else {
            format!("Fixed in: {}", fixed_versions.join(", "))
        },
        details
    );

    let mut metadata = serde_json::json!({
        "severity": severity_str,
        "affected_packages": affected_pkgs,
    });
    if !fixed_versions.is_empty() {
        metadata["fixed_versions"] = serde_json::json!(fixed_versions);
    }
    if let Some(published) = &vuln.published {
        metadata["published"] = serde_json::json!(published);
    }
    if let Some(modified) = &vuln.modified {
        metadata["modified"] = serde_json::json!(modified);
    }

    // Extract CVSS numeric score if available
    if let Some(severities) = &vuln.severity {
        if let Some(cvss) = severities.iter().find(|s| s.severity_type == "CVSS_V3") {
            // CVSS vector strings contain the score; try parsing a bare number first
            if let Ok(score) = cvss.score.parse::<f64>() {
                metadata["cvss_score"] = serde_json::json!(score);
            }
        }
    }

    SourceItem::new("osv", &vuln.id, &format!("[{}] {}", vuln.id, summary))
        .with_url(Some(url))
        .with_content(content)
        .with_metadata(metadata)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

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
