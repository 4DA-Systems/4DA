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
    /// ISO date string when the advisory was withdrawn by the source.
    /// Present only for advisories that have been retracted/withdrawn.
    pub withdrawn: Option<String>,
    /// Other identifiers for the SAME vulnerability (a GHSA row lists its
    /// CVE and RUSTSEC ids, a RUSTSEC row lists the GHSA and CVE). The mirror
    /// stores one row per id, so without these a single quinn-proto bug read
    /// as "2 version-confirmed advisories" on every surface (2026-09-07).
    #[serde(default)]
    pub aliases: Option<Vec<String>>,
    /// Source-specific block. GitHub-reviewed advisories carry a curated
    /// `severity` label here (`CRITICAL`/`HIGH`/`MODERATE`/`LOW`) even when
    /// the CVSS block is a v4 vector the mirror cannot score.
    #[serde(default)]
    pub database_specific: Option<serde_json::Value>,
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
    /// OSV's explicit list of affected versions. Some advisories carry ONLY
    /// this (GHSA-q58r-hwc8-rm9j, bootstrap) or only `GIT` ranges plus this.
    /// It used to be dropped on the belief that the matcher would then "assume
    /// affected" — it did not: the stored ranges were `[]` and an empty
    /// range list reads as "confirmed not affected". Stored as an
    /// [`ENUMERATED_RANGE`] range so every ranges reader sees it.
    #[serde(default)]
    pub versions: Option<Vec<String>>,
}

/// The `type` of the synthetic range that carries an advisory's enumerated
/// `versions` list in the stored ranges JSON (events are the version
/// strings). Readers that only understand `SEMVER`/`ECOSYSTEM` skip it.
pub(crate) const ENUMERATED_RANGE: &str = "ENUMERATED";

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
    /// ISO date string when the advisory was withdrawn by the source.
    /// NULL for active advisories. Rows with a value here are excluded
    /// from active counts but preserved in the database for audit trails.
    pub withdrawn_at: Option<String>,
    pub synced_at: String,
    /// Other ids of the same vulnerability (Phase 120). Empty on rows
    /// synced before the column existed until the next sync refreshes them.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Normalised severity label — `critical` / `high` / `medium` / `low` —
    /// from the source's curated label first, the CVSS band otherwise.
    /// `None` when the source gives neither (RustSec maintenance notices).
    #[serde(default)]
    pub severity_label: Option<String>,
}

/// A version-confirmed copy an advisory does NOT apply to: every source file
/// the advisory names is `cfg`-gated out of that project's build (AD-051,
/// `osv::reachability`). Kept so a surface can say why it is not listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotCompiledMatch {
    pub advisory_id: String,
    pub summary: String,
    pub package_name: String,
    pub ecosystem: String,
    pub project_path: String,
    pub installed_version: Option<String>,
    /// The crate-relative files the advisory names, all gated off.
    pub files: Vec<String>,
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
    /// Machine-wide: the highest line fix across EVERY confirmed copy, in any
    /// project. Right for "is there a fix at all"; wrong as a per-project
    /// upgrade target (navcal's brace-expansion 1.1.12 was told ">= 5.0.12"
    /// because another project held 5.0.9). Per-project consumers read
    /// [`MatchedDependency::fixed_version`] / [`MatchedDependency::clean_version`]
    /// through `osv::fix_target::line_targets`.
    pub fixed_version: Option<String>,
    pub severity_type: Option<String>,
    pub cvss_score: Option<f64>,
    pub source_url: Option<String>,
    pub is_version_confirmed: bool,
    pub project_paths: Vec<String>,
    pub published_at: Option<String>,
    /// Exact affected dependency instances. This prevents project/version/scope
    /// metadata from being reconstructed later from an ambiguous package name.
    pub dependency_instances: Vec<MatchedDependency>,
    /// See [`StoredAdvisory::aliases`].
    #[serde(default)]
    pub aliases: Vec<String>,
    /// See [`StoredAdvisory::severity_label`].
    #[serde(default)]
    pub severity_label: Option<String>,
}

/// Normalise a source severity label to the four-tier vocabulary every
/// surface shares. GitHub says `MODERATE`; the app says `medium`.
pub fn normalize_severity_label(raw: &str) -> Option<String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "critical" => Some("critical".to_string()),
        "high" => Some("high".to_string()),
        "moderate" | "medium" => Some("medium".to_string()),
        "low" => Some("low".to_string()),
        _ => None,
    }
}

/// CVSS base score → the same four-tier label (NVD bands).
pub fn cvss_band(score: f64) -> &'static str {
    if score >= 9.0 {
        "critical"
    } else if score >= 7.0 {
        "high"
    } else if score >= 4.0 {
        "medium"
    } else {
        "low"
    }
}

/// One installed dependency instance affected by an advisory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedDependency {
    pub project_path: String,
    pub installed_version: Option<String>,
    pub is_direct: bool,
    pub is_dev: bool,
    pub is_version_confirmed: bool,
    /// This advisory's fix on the release line THIS copy is on (`None` when
    /// the window has no fix, or the copy could not be placed in a window).
    #[serde(default)]
    pub fixed_version: Option<String>,
    /// The lowest version reachable from this copy by successive fixes that
    /// no known advisory of the package affects — the version that clears
    /// everything, on this copy's own line where one exists. `None` when the
    /// path runs into a window with no fix, or an advisory the matcher cannot
    /// evaluate (no clean version can then be claimed).
    #[serde(default)]
    pub clean_version: Option<String>,
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

/// Metadata for a cached ecosystem ZIP file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMeta {
    pub ecosystem: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub downloaded_at: String,
    pub size_bytes: u64,
    pub advisory_count: usize,
}

/// Result of updating all caches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheUpdateResult {
    pub ecosystems_updated: Vec<String>,
    pub ecosystems_skipped: Vec<String>,
    pub total_advisories: usize,
    pub duration_ms: u64,
    pub errors: Vec<String>,
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
        // OSV usually stores the CVSS VECTOR in `score` (not a bare number); compute the base score
        // per the CVSS v3.1 spec so severity isn't silently dropped for the common case.
        let score = sev.and_then(|s| crate::scoring::cvss::parse_cvss_score(&s.score));
        (sev_type, score)
    }

    /// The severity label the mirror stores: the source's curated label
    /// (`database_specific.severity`, GitHub-reviewed advisories) first —
    /// it is authoritative even when the CVSS block is an unscorable v4
    /// vector — then the band of the best CVSS score. `None` when the
    /// source gives neither.
    pub(crate) fn severity_label(&self) -> Option<String> {
        let curated = self
            .database_specific
            .as_ref()
            .and_then(|d| d.get("severity"))
            .and_then(|v| v.as_str())
            .and_then(normalize_severity_label);
        curated.or_else(|| self.best_cvss().1.map(|s| cvss_band(s).to_string()))
    }

    /// JSON array of alias ids, or `None` when the source lists none.
    pub(crate) fn aliases_json(&self) -> Option<String> {
        let aliases = self.aliases.as_ref()?;
        if aliases.is_empty() {
            return None;
        }
        serde_json::to_string(aliases).ok()
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
