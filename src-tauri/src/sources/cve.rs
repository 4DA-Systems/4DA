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

/// Fetch recent advisories from GitHub Advisory Database.
/// This is preferred over NVD because it includes ecosystem-specific package data.
pub(crate) async fn fetch_github_advisories(ecosystem: Option<&str>) -> Result<Vec<CveAdvisory>> {
    let client = shared_client();
    let mut url =
        "https://api.github.com/advisories?per_page=30&sort=published&direction=desc".to_string();
    if let Some(eco) = ecosystem {
        url.push_str(&format!("&ecosystem={eco}"));
    }

    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "4DA-Developer-OS/1.0")
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(vec![]);
    }

    let body: serde_json::Value = response.json().await?;
    let mut advisories = Vec::new();

    if let Some(items) = body.as_array() {
        for item in items {
            if let Some(advisory) = parse_github_advisory(item) {
                advisories.push(advisory);
            }
        }
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
#[allow(dead_code)]
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
                metadata: serde_json::to_value(&a.affected_packages).ok(),
            }
        })
        .collect()
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
}
