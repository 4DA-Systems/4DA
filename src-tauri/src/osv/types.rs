// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! OSV mirror types — API response types and local storage types.

use serde::{Deserialize, Serialize};

// ============================================================================
// OSV API Response Types (for JSON deserialization from batch API)
// ============================================================================

#[derive(Debug, Serialize)]
pub(crate) struct BatchRequest {
    pub queries: Vec<BatchQuery>,
}

#[derive(Debug, Serialize)]
pub(crate) struct BatchQuery {
    pub package: PackageRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct PackageRef {
    pub name: String,
    pub ecosystem: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BatchResponse {
    pub results: Option<Vec<QueryResult>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QueryResult {
    pub vulns: Option<Vec<Vulnerability>>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Vulnerability {
    pub id: String,
    pub summary: Option<String>,
    pub details: Option<String>,
    pub severity: Option<Vec<Severity>>,
    pub affected: Option<Vec<Affected>>,
    pub references: Option<Vec<Reference>>,
    pub published: Option<String>,
    pub modified: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Severity {
    #[serde(rename = "type")]
    pub severity_type: String,
    pub score: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Affected {
    pub package: Option<PackageRef>,
    pub ranges: Option<Vec<Range>>,
    #[allow(dead_code)]
    pub versions: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct Range {
    #[serde(rename = "type")]
    pub range_type: String,
    pub events: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Reference {
    #[serde(rename = "type")]
    pub ref_type: String,
    pub url: String,
}

// ============================================================================
// Local Storage Types
// ============================================================================

/// An advisory stored in the local osv_advisories table.
/// One row per (advisory_id, package_name, ecosystem) combination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAdvisory {
    pub id: i64,
    pub advisory_id: String,
    pub summary: String,
    pub details: Option<String>,
    pub package_name: String,
    pub ecosystem: String,
    pub affected_ranges: Option<String>,
    pub fixed_versions: Option<String>,
    pub severity_type: Option<String>,
    pub cvss_score: Option<f64>,
    pub source_url: Option<String>,
    pub published_at: Option<String>,
    pub modified_at: Option<String>,
    pub synced_at: String,
}

/// An advisory matched to a user dependency with version verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedAdvisory {
    pub advisory_id: String,
    pub summary: String,
    pub details: Option<String>,
    pub package_name: String,
    pub ecosystem: String,
    pub installed_version: Option<String>,
    pub fixed_version: Option<String>,
    pub severity_type: Option<String>,
    pub cvss_score: Option<f64>,
    pub source_url: Option<String>,
    pub is_version_confirmed: bool,
    pub project_paths: Vec<String>,
    pub published_at: Option<String>,
}

/// Result of a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub ecosystems_synced: Vec<String>,
    pub advisories_stored: usize,
    pub advisories_matched: usize,
    pub duration_ms: u64,
    pub errors: Vec<String>,
}

/// Per-ecosystem sync status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub ecosystem: String,
    pub last_synced_at: Option<String>,
    pub advisory_count: i64,
    pub error: Option<String>,
}

// ============================================================================
// Helpers
// ============================================================================

impl Vulnerability {
    pub(crate) fn best_cvss(&self) -> (Option<String>, Option<f64>) {
        let sev = self.severity.as_ref().and_then(|s| {
            s.iter()
                .find(|sv| sv.severity_type == "CVSS_V3")
                .or_else(|| s.first())
        });
        let sev_type = sev.map(|s| s.severity_type.clone());
        let score = sev.and_then(|s| s.score.parse::<f64>().ok());
        (sev_type, score)
    }

    pub(crate) fn best_url(&self) -> Option<String> {
        self.references.as_ref().and_then(|refs| {
            refs.iter()
                .find(|r| r.ref_type == "ADVISORY")
                .or_else(|| refs.iter().find(|r| r.ref_type == "WEB"))
                .or_else(|| refs.first())
                .map(|r| r.url.clone())
        })
    }
}
