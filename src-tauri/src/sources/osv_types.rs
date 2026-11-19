// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV API types for strict manifest mode's live query (`osv_live`).

use serde::{Deserialize, Serialize};

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
    pub results: Option<Vec<OsvBatchResult>>,
}

/// One result of `/v1/querybatch`, in query order. OSV leaves `vulns` out
/// when no advisory affects that query; a `vulns` that is present and `null`
/// is not that answer (`Some(None)` here), and is read as malformed.
#[derive(Debug, Deserialize)]
pub(super) struct OsvBatchResult {
    #[serde(default, deserialize_with = "present_or_null")]
    pub vulns: Option<Option<Vec<OsvVulnerability>>>,
}

/// A field that is present (`Some`), with its value or `null` (`Some(None)`);
/// `#[serde(default)]` makes an absent one `None`.
fn present_or_null<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
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
    /// Empty when the entry carries none (passed over, the record still read).
    #[serde(default)]
    pub score: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct OsvAffected {
    pub package: Option<OsvPackage>,
    pub ranges: Option<Vec<OsvRange>>,
    // The advisory's enumerated `versions` list is not deserialized: matching
    // runs on `ranges` only, and serde skips unknown fields.
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct OsvRange {
    #[serde(rename = "type")]
    pub range_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
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
