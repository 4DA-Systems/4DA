// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV API types, constants, and conversion helpers.
//!
//! Split from `osv.rs` to keep both files under the 700-line warning threshold.

use serde::{Deserialize, Serialize};

use super::SourceItem;

// ============================================================================
// OSV API Types
// ============================================================================

#[derive(Debug, Serialize)]
pub(super) struct OsvQueryRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<OsvPackage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct OsvBatchRequest {
    pub queries: Vec<OsvQueryRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct OsvPackage {
    pub name: String,
    pub ecosystem: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct OsvQueryResponse {
    pub vulns: Option<Vec<OsvVulnerability>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct OsvBatchResponse {
    pub results: Option<Vec<OsvQueryResponse>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct OsvVulnerability {
    pub id: String,
    pub summary: Option<String>,
    pub details: Option<String>,
    pub severity: Option<Vec<OsvSeverity>>,
    pub affected: Option<Vec<OsvAffected>>,
    pub references: Option<Vec<OsvReference>>,
    pub published: Option<String>,
    pub modified: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct OsvSeverity {
    #[serde(rename = "type")]
    pub severity_type: String,
    pub score: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct OsvAffected {
    pub package: Option<OsvPackage>,
    pub ranges: Option<Vec<OsvRange>>,
    // REMOVE BY 2026-11-10: serde-deserialized field, wire into vulnerability detail view or drop
    #[allow(dead_code)]
    pub versions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // REMOVE BY 2026-11-10: serde-deserialized struct, wire into version range display or drop
pub(super) struct OsvRange {
    #[serde(rename = "type")]
    pub range_type: String,
    pub events: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct OsvReference {
    #[serde(rename = "type")]
    pub ref_type: String,
    pub url: String,
}

// ============================================================================
// Ecosystem mapping (manifest file -> OSV ecosystem string)
// ============================================================================

#[cfg(test)]
pub(super) const ECOSYSTEM_MAP: &[(&str, &str)] = &[
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
pub(super) const DEFAULT_ECOSYSTEMS: &[&str] = &["npm", "crates.io", "PyPI", "Go", "Maven"];

// ============================================================================
// Conversion helpers
// ============================================================================

/// Convert an OSV vulnerability into a SourceItem for the scoring pipeline.
pub(super) fn vuln_to_source_item(vuln: &OsvVulnerability) -> SourceItem {
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
        "source_name": "osv",
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
